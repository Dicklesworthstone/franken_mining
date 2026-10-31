//! DESIGN (BD-333.S2): source-cited, descriptive gold-site bridges between the same quarter in adjacent years.
//! A release is eligible only for its own quarter. Later comparatives, hidden sheets, and aggregate scope
//! never supply a missing site input. Arithmetic closure does not identify a cause.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::{Encoder, blake3_hash, hash::Digest32};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::xlsx::{CellValue, Sheet, col_index_to_letters, parse_a1_ref};
use fmine_metrics::lmdi::{
    Bar, BasisTag, Factor, PrintedQty, WATERFALL_NO_CLAIM, WATERFALL_RULES_VERSION,
    WaterfallPeriod, decompose_aisc_waterfall,
};
pub use fmine_metrics::lmdi::{BarPoint, Bridge};
use fmine_numerics::exact_feas::Rat;

use crate::aisc::{self, AiscClaimIndex, AiscTable, IdentityInstance};
use crate::key::{self, KeyedCell, OwnershipBasis, Period, PeriodKind, PublicValue, Scope};
use crate::mine_ledger::{MineLedger, rat_text};
use crate::mine_stats::{MineStatsRead, MineStatsValue, View};
use crate::sources::{IssuerProfile, SourceKind};
use crate::{LoadedSource, js};

/// A classified candidate, including inputs retained when a later validation refuses the bridge.
#[derive(Debug, Clone)]
pub struct PairOutcome {
    pub subject: String,
    pub from: Period,
    pub to: Period,
    pub scope: &'static str,
    pub inputs: Vec<KeyedCell>,
    /// Live title, denominator and ownership-label citations, including their source digests.
    pub context: Vec<(String, String)>,
    pub outcome: Result<Bridge, Refusal>,
}

impl PairOutcome {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self.outcome {
            Ok(Bridge::Lmdi { .. }) => "lmdi",
            Ok(Bridge::AdditiveFallback { .. }) => "additive_fallback",
            Err(_) => "refused",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WaterfallReport {
    pub pairs: Vec<PairOutcome>,
}

impl WaterfallReport {
    /// The frame binds exact reconciliations, all citations/refusals, and the IEEE bits of approximate bars.
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        let mut enc = Encoder::new_frame(*b"WFLW", 1);
        enc.put_str(WATERFALL_RULES_VERSION);
        enc.put_u64(self.pairs.len() as u64);
        for p in &self.pairs {
            for s in [&p.subject, &p.from.code(), &p.to.code(), p.scope] {
                enc.put_str(s);
            }
            enc.put_fixed(&key::claims_digest(&p.inputs).0);
            enc.put_u64(p.context.len() as u64);
            for (at, sha) in &p.context {
                enc.put_str(at);
                enc.put_str(sha);
            }
            enc.put_str(p.kind());
            let bridge = match &p.outcome {
                Err(r) => {
                    enc.put_str(r.code);
                    continue;
                }
                Ok(b) => b,
            };
            let g = bridge.gap();
            for r in [
                g.aisc_0,
                g.aisc_1,
                g.printed_aisc_0,
                g.printed_aisc_1,
                g.total_delta,
                g.printed_rate_gap_0,
                g.printed_rate_gap_1,
                g.printed_rate_gap,
            ] {
                enc.put_str(&rat_text(r));
            }
            encode_strings(&mut enc, &g.inputs);
            enc.put_u64(bridge.bars().len() as u64);
            for b in bridge.bars() {
                enc.put_str(&b.id);
                enc.put_str(b.rule);
                match b.point {
                    BarPoint::Exact(r) => {
                        enc.put_u8(0);
                        enc.put_str(&rat_text(r));
                    }
                    BarPoint::Approx(v) => {
                        enc.put_u8(1);
                        enc.put_u64(v.to_bits());
                    }
                }
                for bound in b.enclosure {
                    enc.put_u64(bound.to_bits());
                }
                enc.put_u8(u8::from(b.sign_not_determined));
                encode_strings(&mut enc, &b.inputs);
            }
            if let Bridge::Lmdi {
                closure_residual, ..
            } = bridge
            {
                enc.put_u64(closure_residual.to_bits());
            }
        }
        blake3_hash(&enc.finish())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let mut counts = BTreeMap::<String, usize>::new();
        let mut bars = 0;
        let mut sign_nd = 0;
        for p in &self.pairs {
            *counts.entry(p.kind().to_string()).or_default() += 1;
            *counts.entry(format!("{}_pairs", p.scope)).or_default() += 1;
            if let Ok(b) = &p.outcome {
                bars += b.bars().len();
                sign_nd += b.bars().iter().filter(|b| b.sign_not_determined).count();
            }
        }
        for name in [
            "lmdi",
            "additive_fallback",
            "refused",
            "gold_site_pairs",
            "copper_site_pairs",
            "region_pairs",
            "company_pairs",
        ] {
            counts.entry(name.to_string()).or_default();
        }
        let counts = counts
            .iter()
            .map(|(k, n)| format!("{}:{n}", js(k)))
            .collect::<Vec<_>>()
            .join(",");
        let pairs = self
            .pairs
            .iter()
            .map(pair_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.waterfall.v1\",\"rules_version\":{},\"descriptive_only\":true,\
             \"identifying\":false,\"interpretation\":\"non_identifying\",\"interpretation_note\":{},\
             \"vintage_policy\":\"registered release equals the observed quarter; no later comparative substitution\",\
             \"intervals\":\"conservative displayed-rounding enclosures; not confidence intervals\",\
             \"additive_frontier\":\"reported total cash costs (aggregate) plus signed block-2 AISC adjustments\",\
             \"counts\":{{\"pairs\":{},\"bars\":{bars},\"sign_not_determined\":{sign_nd},{counts}}},\"pairs\":[{pairs}]}}",
            js(WATERFALL_RULES_VERSION),
            js(WATERFALL_NO_CLAIM),
            self.pairs.len()
        )
    }
}

fn encode_strings(enc: &mut Encoder, values: &[String]) {
    enc.put_u64(values.len() as u64);
    for value in values {
        enc.put_str(value);
    }
}

fn strings(values: &[String]) -> String {
    values.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
}

fn derived(v: Rat, rule: &str, inputs: &[String]) -> String {
    format!(
        "{{\"v\":{},\"exact\":{},\"rule\":{},\"rules_version\":{},\"in\":[{}]}}",
        v.to_f64(),
        js(&rat_text(v)),
        js(rule),
        js(WATERFALL_RULES_VERSION),
        strings(inputs)
    )
}

fn bar_json(b: &Bar) -> String {
    let exact = match b.point {
        BarPoint::Exact(r) => js(&rat_text(r)),
        BarPoint::Approx(_) => "null".to_string(),
    };
    format!(
        "{{\"id\":{},\"v\":{},\"point_kind\":\"midpoint\",\"exact\":{exact},\"rule\":{},\
         \"rules_version\":{},\"in\":[{}],\"interval\":[{},{}],\"sign_not_determined\":{}}}",
        js(&b.id),
        b.point.as_f64(),
        js(b.rule),
        js(WATERFALL_RULES_VERSION),
        strings(&b.inputs),
        b.enclosure[0],
        b.enclosure[1],
        b.sign_not_determined
    )
}

fn pair_json(p: &PairOutcome) -> String {
    let inputs = p
        .inputs
        .iter()
        .map(KeyedCell::to_json)
        .collect::<Vec<_>>()
        .join(",");
    let context = p
        .context
        .iter()
        .map(|(at, sha)| format!("{{\"at\":{},\"sha256\":{}}}", js(at), js(sha)))
        .collect::<Vec<_>>()
        .join(",");
    let (reason, refusal, detail) = match &p.outcome {
        Err(r) => (
            js(&r.what),
            format!(
                "{{\"code\":{},\"what\":{},\"fix\":{}}}",
                js(r.code),
                js(&r.what),
                js(r.fix)
            ),
            "\"bars\":[],\"printed_rate_gap\":null".to_string(),
        ),
        Ok(bridge) => {
            let reason = match bridge {
                Bridge::AdditiveFallback { reason, .. } => js(reason),
                Bridge::Lmdi { .. } => "null".to_string(),
            };
            let g = bridge.gap();
            let endpoints = [
                ("aisc_0", g.aisc_0),
                ("aisc_1", g.aisc_1),
                ("printed_aisc_0", g.printed_aisc_0),
                ("printed_aisc_1", g.printed_aisc_1),
                ("total_delta", g.total_delta),
                ("printed_rate_gap_0", g.printed_rate_gap_0),
                ("printed_rate_gap_1", g.printed_rate_gap_1),
                ("printed_rate_gap", g.printed_rate_gap),
            ]
            .into_iter()
            .map(|(name, value)| {
                format!(
                    "{}:{}",
                    js(name),
                    derived(value, "waterfall.rate_reconciliation", &g.inputs)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
            let residual = match bridge {
                Bridge::Lmdi {
                    closure_residual, ..
                } => *closure_residual,
                Bridge::AdditiveFallback { .. } => 0.0,
            };
            let closure_inputs = bridge
                .bars()
                .iter()
                .flat_map(|b| &b.inputs)
                .chain(&g.inputs)
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let bars = bridge
                .bars()
                .iter()
                .map(bar_json)
                .collect::<Vec<_>>()
                .join(",");
            (
                reason,
                "null".to_string(),
                format!(
                    "\"bars\":[{bars}],{endpoints},\"closure_residual\":{{\"v\":{residual},\
                 \"rule\":\"waterfall.closure\",\"rules_version\":{},\"in\":[{}]}}",
                    js(WATERFALL_RULES_VERSION),
                    strings(&closure_inputs)
                ),
            )
        }
    };
    format!(
        "{{\"subject\":{},\"from\":{},\"to\":{},\"scope\":{},\"kind\":{},\"reason\":{reason},\
        \"refusal\":{refusal},\"inputs\":[{inputs}],\"context\":[{context}],{detail}}}",
        js(&p.subject),
        js(&p.from.code()),
        js(&p.to.code()),
        js(p.scope),
        js(p.kind())
    )
}

struct TableColumn<'a> {
    source: &'a LoadedSource,
    sheet: &'a Sheet,
    table: AiscTable,
    col: u32,
}

#[derive(Default)]
struct Endpoint<'a> {
    aisc: Vec<TableColumn<'a>>,
    physical: Vec<(&'a LoadedSource, &'a MineStatsValue)>,
}

struct Context<'a> {
    profile: &'a IssuerProfile,
    sources: &'a [LoadedSource],
    claims: &'a AiscClaimIndex,
    reads: &'a [(String, MineStatsRead)],
    aisc_instances: &'a [IdentityInstance],
    mine_ledger: &'a MineLedger,
}

#[derive(Default)]
struct Inputs {
    cells: BTreeMap<String, KeyedCell>,
    context: BTreeMap<String, String>,
}

fn refuse(reason: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::NonIdentifiableAttribution104, reason)
}

fn basis_refusal(reason: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::BasisMismatch105, reason)
}

fn sha(source: &LoadedSource) -> String {
    Digest32(source.workbook.sha256).to_hex()
}

fn cited_at(source: &LoadedSource, sheet: &str, cell: &str) -> String {
    format!("{}:{sheet}!{cell}", source.file_name)
}

fn normalized(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// All candidate site pairs remain visible, including missing first-release reconciliations. Copper and
/// aggregate candidates have separate scope counts; they never inflate the gold-site bridge denominator.
#[must_use]
pub fn pair_sites(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    reads: &[(String, MineStatsRead)],
    claims: &AiscClaimIndex,
    aisc_instances: &[IdentityInstance],
    mine_ledger: &MineLedger,
) -> WaterfallReport {
    let cx = Context {
        profile,
        sources,
        claims,
        reads,
        aisc_instances,
        mine_ledger,
    };
    let mut endpoints: BTreeMap<(String, Period), Endpoint<'_>> = BTreeMap::new();
    for source in sources
        .iter()
        .filter(|s| s.kind == SourceKind::AiscReconciliation)
    {
        for sheet in &source.workbook.sheets {
            let Ok(period) = key::parse_sheet_period(&sheet.name) else {
                continue;
            };
            if !matches!(period.kind, PeriodKind::Quarter(_)) || period.code() != source.quarter {
                continue;
            }
            let Ok(Some(table)) = aisc::locate_table(sheet, &profile.subjects) else {
                continue;
            };
            for cell in sheet.row_cells(table.header_row) {
                let CellValue::Text(header) = &cell.value else {
                    continue;
                };
                let header = header.split_whitespace().collect::<Vec<_>>().join(" ");
                let Some(subject) = aisc::match_subject(&header, &profile.subjects) else {
                    continue;
                };
                endpoints
                    .entry((subject.to_string(), period))
                    .or_default()
                    .aisc
                    .push(TableColumn {
                        source,
                        sheet,
                        table: table.clone(),
                        col: cell.col,
                    });
            }
        }
    }
    for (file, read) in reads {
        let Some(source) = sources
            .iter()
            .find(|s| s.file_name == *file && s.kind == SourceKind::MineStats)
        else {
            continue;
        };
        for v in read.values.iter().filter(|v| {
            v.months == 3
                && v.period == source.quarter
                && (v.view == View::Minestats
                    || profile.regions.contains(&v.site)
                    || matches!(v.site.as_str(), "Total Gold" | "Total Copper"))
        }) {
            if source.workbook.sheet(&v.sheet).is_none() {
                continue;
            }
            let Some(period) = key::parse_period_code(&v.period) else {
                continue;
            };
            let subject = profile
                .shares
                .iter()
                .find(|s| s.label == v.site)
                .map_or(v.site.as_str(), |s| s.subject.as_str());
            endpoints
                .entry((subject.to_string(), period))
                .or_default()
                .physical
                .push((source, v));
        }
    }
    let mut pairs = Vec::new();
    for ((subject, to), later) in &endpoints {
        let Some(year) = to.year.checked_sub(1) else {
            continue;
        };
        let from = Period {
            year,
            kind: to.kind,
        };
        let Some(earlier) = endpoints.get(&(subject.clone(), from)) else {
            continue;
        };
        pairs.push(pair(&cx, subject, from, *to, earlier, later));
    }
    WaterfallReport { pairs }
}

fn pair(
    cx: &Context<'_>,
    subject: &str,
    from: Period,
    to: Period,
    a: &Endpoint<'_>,
    b: &Endpoint<'_>,
) -> PairOutcome {
    let mut inputs = Inputs::default();
    for e in [a, b] {
        for t in &e.aisc {
            inputs
                .context
                .insert(cited_at(t.source, &t.sheet.name, "A1"), sha(t.source));
        }
        for (s, v) in &e.physical {
            inputs.context.insert(cited_at(s, &v.sheet, "A1"), sha(s));
        }
    }
    let scopes = [
        key::scope_for(cx.profile, subject, &from.code()),
        key::scope_for(cx.profile, subject, &to.code()),
    ];
    let copper = [a, b].into_iter().flat_map(|e| &e.physical).any(|(s, v)| {
        s.workbook
            .sheet(&v.sheet)
            .and_then(|s| aisc::cell_text(s, 1, 1))
            .is_some_and(|s| s.contains("(Copper)"))
    });
    let scope = match &scopes[0] {
        Scope::Region { .. } => "region",
        Scope::Company => "company",
        _ if copper => "copper_site",
        _ => "gold_site",
    };
    let outcome = if scopes[0] != scopes[1] {
        Err(basis_refusal(format!(
            "cross_map_region: {subject} changes registered scope between {} and {}",
            from.code(),
            to.code()
        )))
    } else if !matches!(scopes[0], Scope::Site) {
        Err(basis_refusal(format!(
            "unsupported_aggregate_scope: {subject}; only site bridges are supported"
        )))
    } else if copper {
        Err(basis_refusal(
            "copper_site: public gold waterfall does not convert pounds or copper tonnes to gold ounces",
        ))
    } else {
        prepare(cx, subject, a, &mut inputs).and_then(|p0| {
            prepare(cx, subject, b, &mut inputs).and_then(|p1| {
                let mut bridge = decompose_aisc_waterfall(&p0, &p1)?;
                let bars = match &mut bridge {
                    Bridge::Lmdi { bars, .. } | Bridge::AdditiveFallback { bars, .. } => bars,
                };
                for bar in bars {
                    // C's component-sum half-width depends on its signed lines, even for an LMDI bar.
                    for p in [&p0, &p1] {
                        if bar.inputs.contains(&p.cost.at) {
                            bar.inputs.extend(
                                inputs
                                    .cells
                                    .values()
                                    .filter(|c| c.key.unit == "usd_m")
                                    .map(|c| c.locator.clone()),
                            );
                        }
                    }
                    bar.inputs.extend(inputs.context.keys().cloned());
                    bar.inputs.sort();
                    bar.inputs.dedup();
                }
                Ok(bridge)
            })
        })
    };
    PairOutcome {
        subject: subject.to_string(),
        from,
        to,
        scope,
        inputs: inputs.cells.into_values().collect(),
        context: inputs.context.into_iter().collect(),
        outcome,
    }
}

fn prepare(
    cx: &Context<'_>,
    subject: &str,
    endpoint: &Endpoint<'_>,
    inputs: &mut Inputs,
) -> Result<WaterfallPeriod, Refusal> {
    let [t] = endpoint.aisc.as_slice() else {
        return Err(refuse(format!(
            "first_published_aisc_{}: {subject}",
            if endpoint.aisc.is_empty() {
                "missing"
            } else {
                "ambiguous"
            }
        )));
    };
    if endpoint.physical.is_empty() {
        return Err(refuse(format!(
            "same_release_site_sheet_missing: {subject}"
        )));
    }
    if endpoint
        .physical
        .iter()
        .map(|(s, v)| (&s.file_name, &v.sheet))
        .collect::<BTreeSet<_>>()
        .len()
        != 1
    {
        return Err(refuse(format!("ambiguous_site_sheet: {subject}")));
    }
    let mut bases = BTreeSet::<key::SeriesOwnershipBasis>::new();
    for (source, v) in &endpoint.physical {
        if let Some(c) = cx.claims.cells.get(&cited_at(source, &v.sheet, &v.cell)) {
            bases.insert(c.key.ownership_basis.into());
        }
    }
    if bases.len() != 1 {
        return Err(basis_refusal(format!(
            "ambiguous_physical_basis: {subject}"
        )));
    }
    let physical_key = endpoint
        .physical
        .iter()
        .find_map(|(s, v)| cx.claims.cells.get(&cited_at(s, &v.sheet, &v.cell)))
        .ok_or_else(|| refuse(format!("physical_claim_missing: {subject}")))?;
    let confirmed = aisc::bridge_basis(
        cx.profile,
        cx.sources,
        t.source,
        t.sheet,
        &t.table,
        subject,
        physical_key.key.ownership_basis,
    )
    .map_err(basis_refusal)?;
    for at in confirmed.evidence {
        if let Some(source) = cx
            .sources
            .iter()
            .find(|s| at.starts_with(&format!("{}:", s.file_name)))
        {
            inputs.context.insert(at, sha(source));
        }
    }
    let basis = match confirmed.basis {
        OwnershipBasis::Consolidated100 => BasisTag::Consolidated100,
        OwnershipBasis::Attributable { num, den, .. } => BasisTag::Attributable {
            share: Rat::new(i128::from(num), i128::from(den))
                .map_err(|_| basis_refusal("invalid attributable share"))?,
        },
        OwnershipBasis::Unprinted => BasisTag::Unprinted,
    };
    let quantity = |row, zero, inputs: &mut Inputs| {
        let cell = format!("{}{row}", col_index_to_letters(t.col));
        read_quantity(cx, t.source, t.sheet, &cell, "usd_m", zero, inputs)
    };
    let mut lines = Vec::new();
    let mut counted = 0;
    let mut block1_count = 0;
    for row in t.table.block1.iter().chain(&t.table.block2) {
        let q = quantity(*row, true, inputs)?;
        counted += usize::from(q.half_step != Rat::from_integer(0));
        block1_count +=
            usize::from(t.table.block1.contains(row) && q.half_step != Rat::from_integer(0));
        if t.table.block2.contains(row) {
            let c = inputs
                .cells
                .get(&q.at)
                .ok_or_else(|| refuse("component Claim absent"))?;
            lines.push((format!("{}#{}", c.key.line.label, c.key.line.occurrence), q));
        }
    }
    let mut tcc = quantity(t.table.tcc_row, false, inputs)?;
    tcc.half_step =
        Rat::new(block1_count as i128, 2).map_err(|_| refuse("TCC half-width overflow"))?;
    lines.push(("total_cash_costs_aggregate".to_string(), tcc));
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    let mut cost = quantity(t.table.aisc_row, false, inputs)?;
    cost.half_step =
        Rat::new(counted as i128, 2).map_err(|_| refuse("component half-width overflow"))?;
    let mut at_row = |row, unit| {
        read_quantity(
            cx,
            t.source,
            t.sheet,
            &format!("{}{row}", col_index_to_letters(t.col)),
            unit,
            false,
            inputs,
        )
    };
    let oz_sold = at_row(t.table.ounces_row, "koz")?;
    let printed_rate = at_row(t.table.aisc_rate_row, "usd_per_oz")?;
    let mut physical = |name, label, section, unit| {
        physical_factor(cx, endpoint, name, label, section, unit, inputs)
    };
    let tonnes = physical("tonnes", "tonnes processed (thousands)", None, "kt")?;
    let grade = physical(
        "grade",
        "processed",
        Some("average grade (grams per tonne)"),
        "g_per_t",
    )?;
    let recovery = physical("recovery", "recovery rate (percent)", None, "recovery")?;
    let produced = physical("produced", "production (thousands of ounces)", None, "koz")?;
    guard_states(cx, subject, &t.source.quarter, inputs)?;
    Ok(WaterfallPeriod {
        cost,
        lines,
        oz_sold,
        printed_rate,
        tonnes,
        grade,
        recovery,
        produced,
        basis,
    })
}

fn physical_factor(
    cx: &Context<'_>,
    endpoint: &Endpoint<'_>,
    name: &str,
    label: &str,
    section: Option<&str>,
    unit: &str,
    inputs: &mut Inputs,
) -> Result<Factor, Refusal> {
    let selected: Vec<_> = endpoint
        .physical
        .iter()
        .filter(|(_, v)| {
            normalized(&v.label) == label
                && v.section.as_deref().map(normalized).as_deref() == section
        })
        .collect();
    let [selected] = selected.as_slice() else {
        if !selected.is_empty() {
            return Err(refuse(format!("ambiguous_physical_row: {name}")));
        }
        let Some((source, v)) = endpoint.physical.first() else {
            return Err(refuse("site sheet missing"));
        };
        return Ok(Factor::NotPublished {
            token: format!("{name} row absent"),
            at: cited_at(source, &v.sheet, "A1"),
        });
    };
    let (source, v) = **selected;
    let sheet = source
        .workbook
        .sheet(&v.sheet)
        .ok_or_else(|| refuse("site sheet missing"))?;
    let at = cited_at(source, &v.sheet, &v.cell);
    let (row, col) =
        parse_a1_ref(&v.cell).ok_or_else(|| refuse(format!("invalid_locator: {at}")))?;
    if cx.reads.iter().any(|(file, read)| {
        file == &source.file_name
            && read
                .repeated
                .contains(&format!("{}:{}!A{row}", source.quarter, v.sheet))
    }) {
        return Err(refuse(format!("repeated_physical_label: {at}")));
    }
    let c = claim_at(cx, source, &at, inputs)?;
    if let Some(Ok(d)) = sheet.cell(row, col).and_then(|c| c.displayed())
        && let Some(token) = d.token
    {
        return Ok(Factor::NotPublished { token, at });
    }
    if let PublicValue::Number {
        displayed: Ok(_), ..
    } = &c.claim.value
    {
        return read_quantity(cx, source, sheet, &v.cell, unit, false, inputs).map(Factor::Value);
    }
    let token = match &c.claim.value {
        PublicValue::Number {
            displayed: Err(code),
            ..
        } => format!(
            "display refused ({code}): {}",
            key::raw_token(&c.claim.value)
        ),
        _ => key::raw_token(&c.claim.value).to_string(),
    };
    Ok(Factor::NotPublished { token, at })
}

fn claim_at<'a>(
    cx: &'a Context<'_>,
    source: &LoadedSource,
    at: &str,
    inputs: &mut Inputs,
) -> Result<&'a KeyedCell, Refusal> {
    let c = cx
        .claims
        .cells
        .get(at)
        .ok_or_else(|| refuse(format!("unkeyed_input: {at}")))?;
    if c.locator != at
        || c.key.valid_period.code() != source.quarter
        || c.key.reporting_vintage.release != source.quarter
        || c.key.reporting_vintage.file_sha256 != sha(source)
    {
        return Err(refuse(format!("source_binding_mismatch: {at}")));
    }
    inputs.cells.insert(at.to_string(), c.clone());
    Ok(c)
}

fn read_quantity(
    cx: &Context<'_>,
    source: &LoadedSource,
    sheet: &Sheet,
    cell: &str,
    unit: &str,
    zero_token: bool,
    inputs: &mut Inputs,
) -> Result<PrintedQty, Refusal> {
    let at = cited_at(source, &sheet.name, cell);
    let c = claim_at(cx, source, &at, inputs)?;
    let actual = c.key.unit.as_str();
    if (unit == "recovery" && !matches!(actual, "mass_fraction" | "percent"))
        || (unit != "recovery" && actual != unit)
    {
        return Err(basis_refusal(format!(
            "unit_mismatch: {at}, expected {unit}, found {actual}"
        )));
    }
    let (row, col) = parse_a1_ref(cell).ok_or_else(|| refuse(format!("invalid_locator: {at}")))?;
    let extracted = sheet
        .cell(row, col)
        .ok_or_else(|| refuse(format!("missing_input: {at}")))?;
    let shown = extracted.displayed();
    let dash = matches!(&extracted.value, CellValue::Placeholder(t) if matches!(t.trim(), "—" | "–" | "-"));
    if zero_token && dash {
        return Ok(PrintedQty {
            value: Rat::from_integer(0),
            half_step: Rat::from_integer(0),
            at,
        });
    }
    if source.kind == SourceKind::AiscReconciliation
        && matches!(extracted.value, CellValue::Number(_))
        && shown
            .as_ref()
            .is_some_and(|d| d.as_ref().is_ok_and(|d| d.token.is_some()))
    {
        return Ok(PrintedQty {
            value: Rat::from_integer(0),
            half_step: Rat::new(1, 2).map_err(|_| refuse("half-step overflow"))?,
            at,
        });
    }
    let PublicValue::Number {
        displayed: Ok(text),
        ..
    } = &c.claim.value
    else {
        return Err(refuse(format!("not_numeric: {at}")));
    };
    let displayed = shown
        .and_then(Result::ok)
        .filter(|d| d.token.is_none() && d.text() == *text)
        .ok_or_else(|| refuse(format!("display_unavailable: {at}")))?;
    if displayed.percent && unit != "recovery" {
        return Err(basis_refusal(format!("unexpected_percent: {at}")));
    }
    let mut scale = 10_i128
        .checked_pow(u32::from(displayed.d))
        .ok_or_else(|| refuse("display scale overflow"))?;
    if unit == "recovery" && (displayed.percent || actual == "percent") {
        scale = scale
            .checked_mul(100)
            .ok_or_else(|| refuse("percent scale overflow"))?;
    }
    let value =
        Rat::new(displayed.scaled, scale).map_err(|_| refuse("display rational overflow"))?;
    let half_step = Rat::new(
        1,
        scale
            .checked_mul(2)
            .ok_or_else(|| refuse("half-step overflow"))?,
    )
    .map_err(|_| refuse("half-step overflow"))?;
    Ok(PrintedQty {
        value,
        half_step,
        at,
    })
}

fn guard_states(
    cx: &Context<'_>,
    subject: &str,
    release: &str,
    inputs: &Inputs,
) -> Result<(), Refusal> {
    for i in cx
        .aisc_instances
        .iter()
        .filter(|i| i.subject == subject && matches!(i.state.code(), "iii" | "v"))
    {
        if !cx
            .sources
            .iter()
            .any(|s| s.file_name == i.file && s.quarter == release)
            || key::parse_sheet_period(&i.sheet).is_ok_and(|p| p.code() != release)
        {
            continue;
        }
        if let Some(o) = i
            .operands
            .iter()
            .find(|o| inputs.cells.contains_key(&crate::citation::locator(o)))
        {
            return Err(refuse(format!(
                "operand_state_{}: {} in {}",
                i.state.code(),
                crate::citation::locator(o),
                i.family.as_str()
            )));
        }
    }
    for i in cx.mine_ledger.instances.iter().filter(|i| {
        i.subject == subject
            && i.anchor.starts_with(&format!("{release}:"))
            && matches!(i.state.code(), "iii" | "v")
    }) {
        for at in i.cells.iter().flatten() {
            if let Some((_, suffix)) = at.split_once(':')
                && let Some(c) = inputs.cells.values().find(|c| {
                    c.key.reporting_vintage.release == release
                        && c.locator.ends_with(&format!(":{suffix}"))
                })
            {
                return Err(refuse(format!(
                    "operand_state_{}: {} in {} anchored {}",
                    i.state.code(),
                    c.locator,
                    i.family.as_str(),
                    i.anchor
                )));
            }
        }
    }
    Ok(())
}
