//! Public lane (`GATE-BULL-01`, `BD-055`, `BD-058`, `BD-066`, `BD-321`, `BD-322`): loads the registered public
//! workbooks of an issuer, refusing unless every file is present with its registered SHA-256; measures
//! extraction (cell census and numeric-cell digest per workbook); builds the five-state identity ledger over
//! the issuer's AISC reconciliation tables; and commits the whole result to a state root.
//!
//! Nothing here is a stand-in: a missing or unreadable input is a typed refusal (`FMINE-REF-506`, exit 5) and
//! an altered one `FMINE-REF-505` (exit 8), each naming the file. Synthetic workbooks exist only in the
//! synthetic lane, which says so and is never gate-eligible.

pub mod aisc;
pub mod citation;
pub mod consistency;
pub mod guidance;
pub mod html;
pub mod key;
pub mod lint;
pub mod live;
pub mod mine_ledger;
pub mod mine_stats;
pub mod recovery;
pub mod sources;
pub mod synthetic;
pub mod transcribe;
pub mod waterfall;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_io::xlsx::{
    CellCensus, CellValue, ExtractedCell, QuarantineItem, Workbook, parse_a1_ref,
};
use fmine_numerics::exact_feas::discover::{ShareRefusal, UnitDiscovery, UnitFinding};
use fmine_numerics::exact_feas::{self as xf, Rat};

use crate::aisc::{
    CitedCell, ConsistentBasis, DEFAULT_MISSING_FACT, ExplanationBasis, FootingDiscovery,
    IdentityFamily, IdentityInstance, IdentityState, MemberWeighting, OneCellCheck, OneCellOutcome,
    Operand, OperandRole, OperandValue, RateMiss, ReconstructionCheck, ShareBasis, StatedShare,
    Undecided, UnregisteredHeader, cell_text, confirm_share_label, find_qtd_sheet,
    footing_discovery, ledger_for_table, locate_table, one_cell_checks, operand_at,
    reconstruction_holds,
};
use crate::mine_ledger::{LedgerFile, LedgerRules, MineLedger, mine_ledger};
use crate::mine_stats::{CellRefusal, LabelVocabulary, MineStatsRead, read_mine_stats};
pub use crate::sources::load_issuer_profile;
use crate::sources::{
    ExpectedRoot, IssuerProfile, RegisteredMissingFact, RegisteredReconstruction, SourceKind,
};

/// Converts an extracted cell into a public-basis `Claim<Option<f64>>` (`BD-058`): a number is the issuer's
/// assertion, a placeholder is a known unknown, and text or a formula error is refused (`FMINE-REF-208`,
/// source cell unusable; never 401, which §8.4 reserves for unknown reactive ground).
pub fn cell_to_public_claim(
    workbook: &Workbook,
    sheet_name: &str,
    metric_key: &str,
    cell: &ExtractedCell,
    additivity: AdditivityClass,
) -> Result<Claim<Option<f64>>, Refusal> {
    let anchor = workbook.make_anchor(sheet_name, &cell.cell_ref);
    let at = format!("{sheet_name}!{}", cell.cell_ref);
    let unusable = |why: String| Err(Refusal::new(RefusalCode::SourceCellUnusable208, why));
    match &cell.value {
        CellValue::Number(v) => Ok(Claim::new(
            metric_key,
            Some(*v),
            &format!("{v}"),
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::ISSUER,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![anchor],
        )),
        CellValue::Placeholder(tok) => Ok(Claim::new(
            metric_key,
            None,
            tok,
            Representation::Placeholder { token: tok.clone() },
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Unknown,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![anchor],
        )),
        CellValue::FormulaError(err_tok) => {
            unusable(format!("quarantined formula error '{err_tok}' at {at}"))
        }
        CellValue::Text(txt) => unusable(format!(
            "expected a number or placeholder at {at}, found text '{txt}'"
        )),
    }
}

/// Which lane produced a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    /// Registered public filings, verified by digest. Eligible for `GATE-BULL-01`.
    Public,
    /// Invented workbooks with seeded answers. Never gate-eligible.
    Synthetic,
}

impl Lane {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Synthetic => "synthetic",
        }
    }
}

/// One loaded input workbook with its provenance and extraction census.
#[derive(Debug, Clone)]
pub struct LoadedSource {
    pub file_name: String,
    pub kind: SourceKind,
    pub quarter: String,
    pub url: String,
    pub url_basis: String,
    pub bytes: u64,
    pub workbook: Workbook,
    pub census: CellCensus,
}

fn hex(bytes: &[u8; 32]) -> String {
    Digest32(*bytes).to_hex()
}

/// Loads every registered workbook of `profile` from `<root>/<corpus_dir>/`, checking byte size and SHA-256
/// against the registry. Refuses naming every missing, altered or unparseable file: `FMINE-REF-506` (exit 5)
/// when any file is missing or unreadable, otherwise `FMINE-REF-505` (exit 8) for digest mismatches alone.
/// A profile without registered workbooks refuses with `FMINE-REF-201`; documents use their own loader.
pub fn load_registered_sources(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<Vec<LoadedSource>, Refusal> {
    profile
        .sources
        .first()
        .ok_or_else(|| Refusal::new(RefusalCode::NoClaimInDomain201, "no registered workbooks"))?;
    let dir = root.join(&profile.corpus_dir);
    let mut loaded = Vec::with_capacity(profile.sources.len());
    let mut missing = Vec::new();
    let mut altered = Vec::new();
    let mut unreadable = Vec::new();
    for src in &profile.sources {
        let bytes = match std::fs::read(dir.join(&src.file_name)) {
            Ok(b) => b,
            Err(e) => {
                missing.push(format!("{} ({e})", src.file_name));
                continue;
            }
        };
        let digest = sha256(&bytes).to_hex();
        if bytes.len() as u64 != src.bytes || digest != src.sha256_hex {
            altered.push(format!(
                "{} (registered {} bytes sha256 {}, found {} bytes sha256 {digest})",
                src.file_name,
                src.bytes,
                src.sha256_hex,
                bytes.len()
            ));
            continue;
        }
        let rel = format!("{}/{}", profile.corpus_dir, src.file_name);
        match Workbook::from_bytes(rel, &bytes) {
            Ok(workbook) => {
                let census = workbook.cell_census();
                loaded.push(LoadedSource {
                    file_name: src.file_name.clone(),
                    kind: src.kind,
                    quarter: src.quarter.clone(),
                    url: src.url.clone(),
                    url_basis: src.url_basis.clone(),
                    bytes: src.bytes,
                    workbook,
                    census,
                });
            }
            Err(e) => unreadable.push(format!("{} ({e})", src.file_name)),
        }
    }
    if missing.is_empty() && altered.is_empty() && unreadable.is_empty() {
        return Ok(loaded);
    }
    let mut detail = format!(
        "{} of {} registered {} workbooks unusable under {}",
        missing.len() + altered.len() + unreadable.len(),
        profile.sources.len(),
        profile.issuer,
        dir.display()
    );
    for (label, list) in [
        ("missing", &missing),
        ("digest mismatch", &altered),
        ("unreadable", &unreadable),
    ] {
        if !list.is_empty() {
            let _ = write!(detail, "; {label}: {}", list.join(", "));
        }
    }
    Err(files_refusal(&missing, &unreadable, detail))
}

/// `FMINE-REF-506` (exit 5): a registered input is missing or cannot be read.
pub(crate) fn input_refusal(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, detail)
}

/// §30.1: a missing or unreadable registered input is foundation I/O (`FMINE-REF-506`, exit 5), never a
/// digest-mismatch code; only when every unusable file is present and parses but has other bytes is it
/// `FMINE-REF-505` (exit 8). Every file is named either way.
pub(crate) fn files_refusal(missing: &[String], unreadable: &[String], detail: String) -> Refusal {
    if missing.is_empty() && unreadable.is_empty() {
        Refusal::new(RefusalCode::DigestMismatch505, detail)
    } else {
        input_refusal(detail)
    }
}

/// A sheet that holds a cost-of-sales row but whose table structure could not be located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRefusal {
    pub file: String,
    pub sheet: String,
    pub reason: String,
}

/// Report of one assurance run (`fmine assure public`).
#[derive(Debug, Clone)]
pub struct PublicAssuranceReport {
    pub issuer: String,
    pub lane: Lane,
    pub retrieved_utc: String,
    /// The `--through YYYYQn` bound the run applied, if any. Not in the state root: the sources it kept are.
    pub through: Option<String>,
    pub sources: Vec<LoadedSource>,
    pub aisc_tables: usize,
    pub table_refusals: Vec<TableRefusal>,
    pub instances: Vec<IdentityInstance>,
    pub unregistered_headers: Vec<UnregisteredHeader>,
    /// The label-anchored read of each registered Mine Stats workbook, by file name (bd-gwf1.1). The read itself
    /// is not in the state root (it enters with the Mine Stats claims, BD-332.S3); the ledger over it is.
    pub mine_stats: Vec<(String, MineStatsRead)>,
    /// Source-anchored, keyed public figures from AISC operands and Mine Stats. Their separate projection
    /// digest binds status/basis interpretations without changing the established identity-ledger root.
    pub public_claims: aisc::AiscClaimIndex,
    /// Earliest registered AISC identities and cited changes across all visible registered vintages.
    pub issuer_consistency: consistency::AiscConsistency,
    /// A census of Mine Stats Claims using the same canonical keys and IDs as `public_claims`.
    pub mine_stats_claims: key::mine_stats::KeyedClaims,
    /// Exact displayed values and typed tokens across Mine Stats releases, with registered disclosure
    /// coverage checked against the source text. No comparison selects a true vintage.
    pub mine_stats_vintages: key::consistency::VintageReport,
    /// The same comparison rules over visible AISC arithmetic operands.
    pub aisc_vintages: key::consistency::VintageReport,
    pub disclosures: key::disclosure::VerifiedDisclosureRegistry,
    /// Same-quarter year-on-year public AISC bridges, with cited inputs, rounding enclosures and explicit
    /// fallbacks or refusals. Their separate digest is bound into the M0AS v8 assurance root.
    pub waterfall: waterfall::WaterfallReport,
    /// The Mine Stats identity ledger (BD-331.S4): ownership and quarter sums on the summary sheets. Bound into
    /// the state root in its own section; counted apart from `instances` and the totals.
    pub mine_ledger: MineLedger,
    /// The same families run over hidden worksheets (BD-331.S2). Bound into the state root in its own section
    /// and never counted in `instances`, the totals or `GATE-BULL-01`.
    pub hidden_scope: HiddenScope,
    /// Appendix K.4's one-cell cost-of-sales check on every registered-subject column of the visible AISC
    /// tables (bd-prmn.8). A diagnostic: it changes no instance's state and is not in the state root.
    pub one_cell: Vec<OneCellCheck>,
    pub ledger_anchor: LedgerAnchor,
    /// Values transcribed from registered PDF documents and the tie discovery over them (bd-plx1), when the
    /// workspace has a transcription registry and the run has no `--through` bound. Report-only; not in the
    /// state root.
    pub transcriptions: Option<transcribe::TranscriptionReport>,
    /// Optional retrospective recovery comparison from agreed historical PDF rows and source Claims.
    /// Its own digest binds the model and results; it is outside the M0 identity-ledger certificate.
    pub recovery: Option<recovery::RecoveryReport>,
}

/// How every hidden-scope instance is labelled.
pub const HIDDEN_SCOPE: &str = "sheet not displayed to readers";

/// The identity ledger over the AISC tables on hidden worksheets: reported, bound into the root, never counted.
#[derive(Debug, Clone, Default)]
pub struct HiddenScope {
    pub aisc_tables: usize,
    pub instances: Vec<IdentityInstance>,
    pub unregistered_headers: Vec<UnregisteredHeader>,
    pub table_refusals: Vec<TableRefusal>,
}

/// Counts of the alternative construction (subtotal rounded once), reported and never scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternateCounts {
    /// Footing instances in states (i) or (iii) that the alternative construction accepts.
    pub footing_consistent: usize,
    /// Per-ounce instances consistent under the declared construction that the alternative also accepts.
    pub rate_consistent: usize,
}

impl PublicAssuranceReport {
    /// Only a public-lane run can count toward `GATE-BULL-01`.
    #[must_use]
    pub fn gate_eligible(&self) -> bool {
        self.lane == Lane::Public
    }

    /// Instance counts keyed by `(family, state code)`.
    #[must_use]
    pub fn state_counts(&self) -> BTreeMap<(IdentityFamily, &'static str), usize> {
        let mut m = BTreeMap::new();
        for i in &self.instances {
            *m.entry((i.family, i.state.code())).or_insert(0) += 1;
        }
        m
    }

    /// Instance count in one state code across all families.
    #[must_use]
    pub fn count_state(&self, code: &str) -> usize {
        self.instances
            .iter()
            .filter(|i| i.state.code() == code)
            .count()
    }

    /// (i) instances whose consistency is only a relaxation that does not refute them (BD-331.S4: a region not
    /// refuted by its members' weighted mean). They show no conflict and prove no value.
    #[must_use]
    pub fn relaxation_only(&self) -> usize {
        self.instances
            .iter()
            .filter(|i| {
                matches!(
                    i.state,
                    IdentityState::Consistent {
                        basis: Some(ConsistentBasis::Members(_)),
                        ..
                    }
                )
            })
            .count()
    }

    #[must_use]
    pub fn alternate_counts(&self) -> AlternateCounts {
        let mut a = AlternateCounts {
            footing_consistent: 0,
            rate_consistent: 0,
        };
        for i in &self.instances {
            let footing = matches!(
                i.family,
                IdentityFamily::TccFooting | IdentityFamily::AiscFooting
            );
            match &i.state {
                IdentityState::Consistent {
                    alternate_consistent: true,
                    ..
                } if footing => a.footing_consistent += 1,
                IdentityState::IssuerDiscrepancy(c) if c.alternate_consistent => {
                    a.footing_consistent += 1
                }
                IdentityState::Consistent {
                    alternate_consistent: true,
                    ..
                } => a.rate_consistent += 1,
                _ => {}
            }
        }
        a
    }

    /// The `mine_stats_ledger` section (BD-331.S4): counts per family and state (refusals by reason), every (iii)
    /// instance, and every jointly infeasible component with its minimum correction sets and implied ranges.
    fn mine_ledger_json(&self, o: &mut String) {
        use mine_ledger::{Family, State};
        let l = &self.mine_ledger;
        let list = |cells: &[String]| cells.iter().map(|c| js(c)).collect::<Vec<_>>().join(",");
        o.push_str("\"mine_stats_ledger\":{\"construction\":\"rounded_once\",\"families\":[");
        for (n, family) in Family::ALL.into_iter().enumerate() {
            let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
            for i in l.instances.iter().filter(|i| i.family == family) {
                if let State::Refused { reason, .. } = &i.state {
                    *by_reason.entry(reason.code()).or_insert(0) += 1;
                }
            }
            let _ = write!(
                o,
                "{}{{\"family\":{},\"n\":{}",
                if n == 0 { "" } else { "," },
                js(family.as_str()),
                l.instances.iter().filter(|i| i.family == family).count()
            );
            for code in ["i", "iii", "iv", "v", "np"] {
                let _ = write!(o, ",{}:{}", js(code), l.count(family, code));
            }
            let reasons: Vec<String> = by_reason
                .iter()
                .map(|(r, k)| format!("{}:{k}", js(r)))
                .collect();
            let _ = write!(o, ",\"v_by_reason\":{{{}}}}}", reasons.join(","));
        }
        let _ = write!(
            o,
            "],\"components\":{},\"infeasible_components\":{},\"joint_only\":{},\"indeterminate\":[{}],\
             \"discrepancies\":[",
            l.components,
            l.infeasible.len(),
            l.infeasible.iter().filter(|c| c.joint_only).count(),
            l.indeterminate
                .iter()
                .map(|(id, why)| js(&format!("{id}: {why}")))
                .collect::<Vec<_>>()
                .join(",")
        );
        let iii: Vec<String> = l
            .instances
            .iter()
            .filter(|i| i.state == State::Discrepancy)
            .map(|i| {
                let cells: Vec<String> = i.cells.iter().flatten().cloned().collect();
                format!(
                    "{{\"family\":{},\"subject\":{},\"field\":{},\"group\":{},\"period\":{},\"anchor\":{},\
                     \"cells\":[{}],\"share\":{},\"printed_share\":{},\"certificate\":{},\"component\":{}}}",
                    js(i.family.as_str()),
                    js(&i.subject),
                    js(&i.field),
                    js(i.group.as_str()),
                    js(&i.period.code()),
                    js(&i.anchor),
                    list(&cells),
                    i.share
                        .map_or_else(|| "null".to_string(), |s| js(&mine_ledger::rat_text(s))),
                    i.printed_share.as_deref().map_or_else(|| "null".to_string(), js),
                    if i.nil_token { "\"nil_token\"" } else { "null" },
                    i.component.as_deref().map_or_else(|| "null".to_string(), js)
                )
            })
            .collect();
        o.push_str(&iii.join(","));
        o.push_str("],\"unresolved\":[");
        let iv: Vec<String> = l
            .instances
            .iter()
            .filter_map(|i| match &i.state {
                State::Unresolved(need) => Some((i, need)),
                _ => None,
            })
            .map(|(i, need)| {
                let cells: Vec<String> = i.cells.iter().flatten().cloned().collect();
                format!(
                    "{{\"family\":{},\"subject\":{},\"field\":{},\"group\":{},\"period\":{},\"anchor\":{},\
                     \"cells\":[{}],\"needs\":{},\"certificate\":{}}}",
                    js(i.family.as_str()),
                    js(&i.subject),
                    js(&i.field),
                    js(i.group.as_str()),
                    js(&i.period.code()),
                    js(&i.anchor),
                    list(&cells),
                    js(need.code()),
                    if i.nil_token { "\"nil_token\"" } else { "null" },
                )
            })
            .collect();
        o.push_str(&iv.join(","));
        o.push_str("],\"infeasible\":[");
        let comps: Vec<String> = l
            .infeasible
            .iter()
            .map(|c| {
                let anchors: Vec<String> = c
                    .instances
                    .iter()
                    .filter_map(|&k| l.instances.get(k).map(|i| i.anchor.clone()))
                    .collect();
                let sets: Vec<String> = c
                    .corrections
                    .iter()
                    .map(|s| {
                        let implied: Vec<String> = s
                            .implied
                            .iter()
                            .map(|(cell, r)| format!("{}:{}", js(cell), js(&mine_ledger::range_text(r))))
                            .collect();
                        format!(
                            "{{\"cells\":[{}],\"implied\":{{{}}}}}",
                            list(&s.cells),
                            implied.join(",")
                        )
                    })
                    .collect();
                format!(
                    "{{\"component\":{},\"cells\":[{}],\"anchors\":[{}],\"joint_only\":{},\"minimum\":{},\
                     \"corrections\":[{}]}}",
                    js(&c.id),
                    list(&c.cells),
                    list(&anchors),
                    c.joint_only,
                    c.minimum,
                    sets.join(",")
                )
            })
            .collect();
        o.push_str(&comps.join(","));
        o.push_str("]},");
    }

    /// The `one_cell_cos` section (bd-prmn.8): counts over every checked column, then each failing column.
    fn one_cell_json(&self, o: &mut String) {
        let count = |state: &str| self.one_cell.iter().filter(|c| c.state() == state).count();
        let _ = write!(
            o,
            "\"one_cell_cos\":{{\"diagnostic\":\"Appendix K.4 one-cell cost-of-sales check; not a ledger state, \
             not in the state root\",\"columns\":{},\"pass\":{},\"fail\":{},\"refused\":{},\"failing\":[",
            self.one_cell.len(),
            count("pass"),
            count("fail"),
            count("refused")
        );
        let failing = self.one_cell.iter().filter_map(|c| match &c.outcome {
            OneCellOutcome::Checked {
                basis: (a, b),
                pass: false,
                at_full,
                at_share,
                implied_oz: (lo, hi),
            } => Some(format!(
                "{{\"file\":{},\"sheet\":{},\"column\":{},\"subject\":{},\"basis\":\"{a}/{b}\",\"at_full\":{at_full},\
                 \"at_share\":{},\"implied_oz\":[{},{}]}}",
                js(&c.file),
                js(&c.sheet),
                js(&c.column),
                js(&c.subject),
                at_share.map_or_else(|| "null".to_string(), |s| s.to_string()),
                js(lo),
                js(hi)
            )),
            _ => None,
        });
        o.push_str(&failing.collect::<Vec<_>>().join(","));
        o.push_str("]},");
    }

    /// Which rounding construction the TCC and AISC footings fit (bd-4wvl.4, BD-340.S4 part 1): the visible
    /// scope, then the hidden one. A report: it changes no state and is not in the state root.
    pub fn construction_discovery(
        &self,
    ) -> Result<Vec<(&'static str, FootingDiscovery)>, fmine_units::UnitsError> {
        let mut out = Vec::new();
        for (scope, instances) in [
            ("visible", &self.instances),
            ("hidden", &self.hidden_scope.instances),
        ] {
            out.extend(
                footing_discovery(instances)?
                    .into_iter()
                    .map(|d| (scope, d)),
            );
        }
        Ok(out)
    }

    /// E463 (bd-4wvl.4 part 5): which unit the `Minestats*` copper production and sales rows labelled "(millions
    /// of pounds)" are in, from their site-sheet pairs alone. The registered alias (W1-DR-02) only names the row;
    /// this asks the numbers, kt (the summary's unit) against Mlb. A report: no state, not in the state root.
    pub fn copper_unit_discovery(&self) -> Result<UnitDiscovery, ShareRefusal> {
        use fmine_numerics::exact_feas::discover::{Factor, UnitPair, discover_unit};
        let mut at: BTreeMap<&str, (&mine_stats::MineStatsValue, &Workbook)> = BTreeMap::new();
        for (file, read) in &self.mine_stats {
            let Some(source) = self.sources.iter().find(|s| &s.file_name == file) else {
                continue;
            };
            for v in &read.values {
                at.insert(v.locator.as_str(), (v, &source.workbook));
            }
        }
        let number = |locator: &Option<String>| {
            let (v, workbook) = at.get(locator.as_deref()?)?;
            let (printed, step) = mine_ledger::displayed_number(v, workbook)?;
            Some((v, Factor { printed, step }))
        };
        let mut pairs = Vec::new();
        for i in &self.mine_ledger.instances {
            if i.family != mine_ledger::Family::SiteSheet
                || !matches!(i.field.as_str(), "cu_prod_attr_kt" | "cu_sold_attr_kt")
            {
                continue;
            }
            let [summary, site] = i.cells.as_slice() else {
                continue;
            };
            if let (Some((_, reference)), Some((v, observed))) = (number(summary), number(site))
                && v.label_as_printed.contains("(millions of pounds)")
            {
                pairs.push(UnitPair {
                    observed,
                    reference,
                });
            }
        }
        let kt_per_mlb = Rat::new(
            fmine_units::AVDP_LB_TO_GRAMS_NUM,
            fmine_units::AVDP_LB_TO_GRAMS_DEN * 1000,
        )?;
        let candidates = [
            ("kt".to_string(), Rat::from_integer(1)),
            ("Mlb".to_string(), kt_per_mlb),
        ];
        discover_unit(&pairs, &candidates, xf::Convention::Closed)
    }

    /// The `unit_discovery` section (bd-4wvl.4 part 5): counts only.
    fn unit_discovery_json(&self, o: &mut String) {
        o.push_str(
            "\"unit_discovery\":{\"report_only\":\"which unit the copper rows labelled (millions of pounds) are \
             in, from their site-sheet pairs; never a state, not in the state root\",",
        );
        match self.copper_unit_discovery() {
            Err(e) => {
                let _ = write!(o, "\"refused\":{}}},", js(&format!("{e:?}")));
            }
            Ok(u) => {
                let fits: Vec<String> = u
                    .fits
                    .iter()
                    .map(|(name, n)| format!("{}:{n}", js(name)))
                    .collect();
                let finding = match &u.finding {
                    UnitFinding::Identified(name) => name.clone(),
                    UnitFinding::Ambiguous(_) => "ambiguous".to_string(),
                    UnitFinding::NoFit => "no_fit".to_string(),
                };
                let _ = write!(
                    o,
                    "\"pairs\":{},\"fits\":{{{}}},\"finding\":{},\"reason_code\":{}}},",
                    u.pairs,
                    fits.join(","),
                    js(&finding),
                    u.finding
                        .reason_code()
                        .map_or_else(|| "null".to_string(), js)
                );
            }
        }
    }

    /// The `construction_discovery` section (bd-4wvl.4): one entry per footing family and scope.
    fn construction_discovery_json(&self, o: &mut String) {
        let _ = write!(
            o,
            "\"construction_discovery\":{{\"report_only\":\"which rounding construction the decided footings fit; \
             never a state, not in the state root\",\"alpha\":{:e},\"null\":\"rounded_once: every printed value \
             is its true value rounded once, independently\",",
            fmine_numerics::exact_feas::discover::ALPHA
        );
        match self.construction_discovery() {
            Err(e) => {
                let _ = write!(o, "\"refused\":{}}},", js(&e.to_string()));
            }
            Ok(found) => {
                let entries: Vec<String> = found
                    .iter()
                    .map(|(scope, d)| discovery_json(scope, d))
                    .collect();
                let _ = write!(o, "\"families\":[{}]}},", entries.join(","));
            }
        }
    }

    /// One cited identity instance as a JSON object; `scope` labels a hidden-scope instance. Report surfaces
    /// use this projection to retain the ledger's exact arithmetic and source locators in review packets.
    pub fn exception_json(&self, i: &IdentityInstance, scope: Option<&str>) -> String {
        let mut o = format!(
            "{{\"state\":{},\"label\":{},\"file\":{},\"sha256\":{},\"sheet\":{},\"cell\":{},\
             \"subject\":{},\"family\":{},\"detail\":{}",
            js(i.state.code()),
            js(i.state.label()),
            js(&i.file),
            js(&self.source_sha(&i.file)),
            js(&i.sheet),
            js(&i.cell),
            js(&i.subject),
            js(i.family.as_str()),
            js(&i.state.detail())
        );
        if let Some(code) = i.state.reason_code() {
            let _ = write!(o, ",\"reason_code\":{}", js(code));
        }
        if let Some(scope) = scope {
            let _ = write!(o, ",\"scope\":{}", js(scope));
        }
        let sha_of = |f: &str| self.source_sha(f);
        let derived = citation::derived(i);
        match &i.state {
            IdentityState::IssuerDiscrepancy(c) => {
                let lines: Vec<String> = c.line_cells.iter().map(|l| js(l)).collect();
                // The three numbers as citations (bd-sjrh.1): the printed total as read, the sum and the residual
                // with their rules and inputs.
                let printed = i
                    .operands
                    .iter()
                    .find(|o| o.role == OperandRole::Total)
                    .map_or_else(
                        || "null".to_string(),
                        |o| citation::extracted(o, &sha_of).to_json(),
                    );
                let by_name = |name: &str| {
                    derived
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map_or_else(|| "null".to_string(), |(_, d)| d.to_json())
                };
                let _ = write!(
                    o,
                    ",\"certificate\":{{\"printed\":{},\"component_sum\":{},\"residual\":{},\"total_cell\":{},\
                     \"line_cells\":[{}],\"sign_flip_witness\":{},\"alternate_consistent\":{}}}",
                    printed,
                    by_name("component_sum"),
                    by_name("residual"),
                    js(&c.total_cell),
                    lines.join(","),
                    c.sign_flip_witness
                        .as_deref()
                        .map_or_else(|| "null".to_string(), js),
                    c.alternate_consistent
                );
            }
            IdentityState::Explained { rule, .. } => {
                let _ = write!(o, ",\"rule\":{}", js(rule));
            }
            IdentityState::Undecidable {
                missing_fact,
                closing_document,
                ..
            } => {
                let _ = write!(
                    o,
                    ",\"missing_fact\":{},\"closing_document\":{}",
                    js(missing_fact),
                    closing_document
                        .as_deref()
                        .map_or_else(|| "null".to_string(), js)
                );
            }
            _ => {}
        }
        o.push_str(",\"evidence\":\"Assertion\",\"source\":\"issuer\"");
        let _ = write!(o, ",\"cited\":{}", citation::cited_json(i, &sha_of));
        o.push('}');
        o
    }

    /// One compact record per counted instance (bd-sjrh.1): its cell, family, state and cited numbers.
    fn instance_json(&self, i: &IdentityInstance) -> String {
        format!(
            "{{\"at\":{},\"family\":{},\"state\":{},\"evidence\":\"Assertion\",\"source\":\"issuer\",\"cited\":{}}}",
            js(&format!("{}:{}!{}", i.file, i.sheet, i.cell)),
            js(i.family.as_str()),
            js(i.state.code()),
            citation::cited_json(i, &|f| self.source_sha(f))
        )
    }

    /// sha256 over the sorted `"{file}\t{sha256}\n"` lines of the sources this run read (BD-339.S1).
    #[must_use]
    pub fn registry_epoch(&self) -> String {
        let mut lines: Vec<String> = self
            .sources
            .iter()
            .map(|s| format!("{}\t{}\n", s.file_name, hex(&s.workbook.sha256)))
            .collect();
        lines.sort();
        sha256(lines.concat().as_bytes()).to_hex()
    }

    /// sha256 over `"{file}\t{numeric_digest}\n"` in registry order: one value an independent reader of the
    /// cells can reproduce (BD-339.S1).
    #[must_use]
    pub fn census_digest_of_digests(&self) -> String {
        let lines: String = self
            .sources
            .iter()
            .map(|s| format!("{}\t{}\n", s.file_name, hex(&s.census.numeric_digest)))
            .collect();
        sha256(lines.as_bytes()).to_hex()
    }

    /// Compares this run with a row of the expected-roots file: root, anchor, totals, registry epoch and digest
    /// of digests must all be equal. The error names every field that differs.
    pub fn check_expected(&self, expected: &ExpectedRoot) -> Result<(), String> {
        let totals = STATE_CODES.map(|c| self.count_state(c) as u64);
        let checks = [
            (
                "state_root",
                self.ledger_anchor.state_root.to_hex(),
                expected.state_root.clone(),
            ),
            (
                "anchor_id",
                self.ledger_anchor.anchor_id().to_string(),
                expected.anchor_id.clone(),
            ),
            (
                "totals",
                format!("{totals:?}"),
                format!("{:?}", expected.totals),
            ),
            (
                "registry_epoch",
                self.registry_epoch(),
                expected.registry_epoch.clone(),
            ),
            (
                "census_digest_of_digests",
                self.census_digest_of_digests(),
                expected.census_digest_of_digests.clone(),
            ),
        ];
        let differ: Vec<String> = checks
            .iter()
            .filter(|(_, got, want)| got != want)
            .map(|(name, got, want)| format!("{name}: run {got}, expected {want}"))
            .collect();
        if differ.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "run differs from the expected row of commit {} (line {}): {}",
                expected.commit,
                expected.line,
                differ.join("; ")
            ))
        }
    }

    fn source_sha(&self, file: &str) -> String {
        self.sources
            .iter()
            .find(|s| s.file_name == file)
            .map(|s| hex(&s.workbook.sha256))
            .unwrap_or_default()
    }

    /// Canonical JSON payload for the CLI envelope.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut o = String::new();
        // The no-claim line, the four flags and the next actions are envelope fields (fmine-cli), not payload.
        let _ = write!(
            o,
            "{{\"issuer\":{},\"lane\":{},\"gate_eligible\":{},",
            js(&self.issuer),
            js(self.lane.as_str()),
            self.gate_eligible()
        );
        let _ = write!(
            o,
            "\"retrieved_utc\":{},\"through\":{},\"sources\":[",
            js(&self.retrieved_utc),
            self.through
                .as_deref()
                .map_or_else(|| "null".to_string(), js)
        );
        for (n, s) in self.sources.iter().enumerate() {
            let c = &s.census;
            let _ = write!(
                o,
                "{}{{\"file\":{},\"kind\":{},\"quarter\":{},\"url\":{},\"url_basis\":{},\"bytes\":{},\
                 \"sha256\":{},\"visible_sheets\":{},\"hidden_sheets\":{},\"numeric_cells\":{},\
                 \"placeholder_cells\":{},\"formula_error_cells\":{},\"text_cells\":{},\"numeric_digest\":{}}}",
                if n == 0 { "" } else { "," },
                js(&s.file_name),
                js(s.kind.as_str()),
                js(&s.quarter),
                js(&s.url),
                js(&s.url_basis),
                s.bytes,
                js(&hex(&s.workbook.sha256)),
                c.visible_sheets,
                c.hidden_sheets,
                c.numeric_cells,
                c.placeholder_cells,
                c.formula_error_cells,
                c.text_cells,
                js(&hex(&c.numeric_digest))
            );
        }
        let total =
            |f: fn(&CellCensus) -> usize| self.sources.iter().map(|s| f(&s.census)).sum::<usize>();
        let _ = write!(
            o,
            "],\"extraction\":{{\"workbooks\":{},\"visible_sheets\":{},\"numeric_cells\":{},\
             \"placeholder_cells\":{},\"formula_error_cells\":{},\"text_cells\":{}}},",
            self.sources.len(),
            total(|c| c.visible_sheets),
            total(|c| c.numeric_cells),
            total(|c| c.placeholder_cells),
            total(|c| c.formula_error_cells),
            total(|c| c.text_cells)
        );
        o.push_str("\"mine_stats\":[");
        for (n, (file, read)) in self.mine_stats.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"quarter\":{},\"blocks\":{},\"cells\":{},\"number\":{},\"numeric_zero\":{},\
                 \"nil\":{},\"na\":{},\"missing\":{},\"digest\":{},\"hidden_sheets\":[{}],\"refusals\":[",
                if n == 0 { "" } else { "," },
                js(file),
                js(&read.quarter),
                read.blocks,
                read.values.len(),
                read.count("number"),
                read.numeric_zeros(),
                read.count("nil"),
                read.count("na"),
                read.count("missing"),
                js(&read.digest().to_hex()),
                read.hidden_sheets
                    .iter()
                    .map(|h| js(h))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for (k, r) in read.refusals.iter().enumerate() {
                let _ = write!(
                    o,
                    "{}{{\"locator\":{},\"reason\":{}}}",
                    if k == 0 { "" } else { "," },
                    js(&r.locator),
                    js(&r.reason)
                );
            }
            o.push_str("]}");
        }
        o.push_str("],");
        let _ = write!(
            o,
            "\"public_claims\":{{\"schema\":\"fmine.public_claims.v1\",\"digest\":{},\"cells\":[",
            js(&key::claims_digest(self.public_claims.cells.values()).to_hex())
        );
        for (n, cell) in self.public_claims.cells.values().enumerate() {
            if n != 0 {
                o.push(',');
            }
            o.push_str(&cell.to_json());
        }
        o.push_str("],\"refusals\":[");
        for (n, refusal) in self.public_claims.refusals.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"at\":{},\"reason\":{}}}",
                if n == 0 { "" } else { "," },
                js(&refusal.locator),
                js(&refusal.reason)
            );
        }
        let _ = write!(
            o,
            "],\"unregistered_all_zero\":[{}]}},",
            self.public_claims
                .unregistered_all_zero
                .iter()
                .map(|s| js(s))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut consistency = self.issuer_consistency.to_json();
        consistency.pop();
        let _ = write!(
            o,
            "\"issuer_consistency\":{consistency},\"mine_stats_vintages\":{},\"aisc_vintages\":{},\
             \"disclosures\":{}}},\"mine_stats_claims\":{},",
            self.mine_stats_vintages.to_json(),
            self.aisc_vintages.to_json(),
            self.disclosures.to_json(),
            self.mine_stats_claims.summary_json()
        );
        let _ = write!(o, "\"waterfall\":{},", self.waterfall.to_json());
        self.mine_ledger_json(&mut o);

        let counts = self.state_counts();
        let alt = self.alternate_counts();
        let _ = write!(
            o,
            "\"identity_ledger\":{{\"construction\":\"component_sum\",\"aisc_tables\":{},\"instances\":{},\
             \"by_family\":{{",
            self.aisc_tables,
            self.instances.len()
        );
        for (n, family) in IdentityFamily::ALL.iter().enumerate() {
            let _ = write!(
                o,
                "{}{}:{{\"basis\":{}",
                if n == 0 { "" } else { "," },
                js(family.as_str()),
                js(family.basis())
            );
            for code in STATE_CODES {
                let _ = write!(
                    o,
                    ",{}:{}",
                    js(code),
                    counts.get(&(*family, code)).copied().unwrap_or(0)
                );
            }
            o.push('}');
        }
        o.push_str("},\"totals\":{");
        for (n, code) in STATE_CODES.iter().enumerate() {
            let _ = write!(
                o,
                "{}{}:{}",
                if n == 0 { "" } else { "," },
                js(code),
                self.count_state(code)
            );
        }
        let _ = write!(
            o,
            "}},\"alternate_construction\":{{\"scored\":false,\"footing_consistent\":{},\
             \"rate_consistent_of_declared_consistent\":{}}},\"relaxation_only\":{},\"exceptions\":[",
            alt.footing_consistent,
            alt.rate_consistent,
            self.relaxation_only()
        );
        let exceptions: Vec<String> = self
            .instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
            .map(|i| self.exception_json(i, None))
            .collect();
        o.push_str(&exceptions.join(","));
        o.push_str("],");
        for (key, code) in [("refused", "v"), ("not_published", "np")] {
            let mut by_reason: BTreeMap<String, (&str, Vec<String>)> = BTreeMap::new();
            for i in self.instances.iter().filter(|i| i.state.code() == code) {
                let entry = by_reason
                    .entry(i.state.detail())
                    .or_insert_with(|| (i.state.reason_code().unwrap_or_default(), Vec::new()));
                entry.1.push(format!(
                    "{}:{}!{}:{}",
                    i.file,
                    i.sheet,
                    i.cell,
                    i.family.as_str()
                ));
            }
            let _ = write!(o, "{}:[", js(key));
            for (n, (reason, (reason_code, cells))) in by_reason.iter().enumerate() {
                let cells: Vec<String> = cells.iter().map(|c| js(c)).collect();
                let _ = write!(
                    o,
                    "{}{{\"reason\":{},\"reason_code\":{},\"count\":{},\"instances\":[{}]}}",
                    if n == 0 { "" } else { "," },
                    js(reason),
                    js(reason_code),
                    cells.len(),
                    cells.join(",")
                );
            }
            o.push_str("],");
        }
        o.push_str("\"unregistered_headers\":[");
        for (n, h) in self.unregistered_headers.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"sheet\":{},\"cell\":{},\"header\":{}}}",
                if n == 0 { "" } else { "," },
                js(&h.file),
                js(&h.sheet),
                js(&h.cell),
                js(&h.header)
            );
        }
        o.push_str("],\"table_refusals\":[");
        for (n, t) in self.table_refusals.iter().enumerate() {
            let _ = write!(
                o,
                "{}{{\"file\":{},\"sheet\":{},\"reason\":{}}}",
                if n == 0 { "" } else { "," },
                js(&t.file),
                js(&t.sheet),
                js(&t.reason)
            );
        }
        o.push_str("]},\"instances\":[");
        let records: Vec<String> = self
            .instances
            .iter()
            .map(|i| self.instance_json(i))
            .collect();
        o.push_str(&records.join(","));
        o.push_str("],");
        self.one_cell_json(&mut o);
        let hidden = &self.hidden_scope;
        let _ = write!(
            o,
            "\"hidden_scope\":{{\"scope\":{},\"counted\":false,\"aisc_tables\":{},\"instances\":{},\"by_state\":{{",
            js(HIDDEN_SCOPE),
            hidden.aisc_tables,
            hidden.instances.len()
        );
        for (n, code) in STATE_CODES.iter().enumerate() {
            let count = hidden
                .instances
                .iter()
                .filter(|i| i.state.code() == *code)
                .count();
            let _ = write!(o, "{}{}:{count}", if n == 0 { "" } else { "," }, js(code));
        }
        let hidden_exceptions: Vec<String> = hidden
            .instances
            .iter()
            .filter(|i| matches!(i.state.code(), "ii" | "iii" | "iv"))
            .map(|i| self.exception_json(i, Some(HIDDEN_SCOPE)))
            .collect();
        let _ = write!(
            o,
            "}},\"exceptions\":[{}],\"unregistered_headers\":{},\"table_refusals\":{}}},",
            hidden_exceptions.join(","),
            hidden.unregistered_headers.len(),
            hidden.table_refusals.len()
        );
        self.construction_discovery_json(&mut o);
        self.unit_discovery_json(&mut o);
        o.push_str(&transcribe::transcription_json(
            self.transcriptions.as_ref(),
        ));
        let _ = write!(
            o,
            "\"recovery\":{},",
            self.recovery
                .as_ref()
                .map_or_else(|| "null".to_string(), recovery::RecoveryReport::to_json)
        );
        o.push_str("\"quarantine\":[");
        let mut first = true;
        for s in &self.sources {
            for q in &s.workbook.quarantined {
                let item = match q {
                    QuarantineItem::HiddenSheet { sheet_name, state } => format!(
                        "{{\"file\":{},\"kind\":\"hidden_sheet\",\"sheet\":{},\"state\":{}}}",
                        js(&s.file_name),
                        js(sheet_name),
                        js(state)
                    ),
                    QuarantineItem::FormulaErrorCell {
                        sheet_name,
                        cell_ref,
                        error_token,
                    } => format!(
                        "{{\"file\":{},\"kind\":\"formula_error\",\"sheet\":{},\"cell\":{},\"token\":{}}}",
                        js(&s.file_name),
                        js(sheet_name),
                        js(cell_ref),
                        js(error_token)
                    ),
                };
                if !first {
                    o.push(',');
                }
                o.push_str(&item);
                first = false;
            }
        }
        let _ = write!(
            o,
            "],\"state_root\":{},\"anchor_id\":{}}}",
            js(&self.ledger_anchor.state_root.to_hex()),
            js(&self.ledger_anchor.anchor_id().to_string())
        );
        o
    }

    /// The commands worth running after this report, as `(command, why)`: the CLI puts them in the
    /// envelope's `next_actions`.
    #[must_use]
    pub fn next_actions(&self) -> Vec<(&'static str, &'static str)> {
        let other_lane = match self.lane {
            Lane::Public => (
                "fmine assure public --synthetic",
                "known-answer run on synthetic workbooks that exercises every ledger state",
            ),
            Lane::Synthetic => (
                "fmine assure public --issuer barrick",
                "the gate-eligible run over the workbooks in registries/sources/barrick.toml",
            ),
        };
        vec![
            other_lane,
            ("fmine robot-docs", "verbs, envelope fields and exit codes"),
        ]
    }
}

const STATE_CODES: [&str; 6] = ["i", "ii", "iii", "iv", "v", "np"];

/// What an `assure public` report does not claim; the CLI prints it in the envelope's `no_claim` field.
pub const NO_CLAIM: &str = "Reproduces and checks the issuer's published AISC reconciliation tables as printed. \
It makes no reserve, resource, geotechnical, regulatory or investment claim and implies no issuer affiliation.";

/// One family and scope of the `construction_discovery` section, as a JSON object (also the real-file test's log
/// line). Counts and probabilities only, never a cell value. A log tail of −∞ (probability 0) is `null`.
#[must_use]
pub fn discovery_json(scope: &str, d: &FootingDiscovery) -> String {
    let r = &d.report;
    let ln = |x: f64| {
        if x.is_finite() {
            format!("{x:.3}")
        } else {
            "null".to_string()
        }
    };
    let expected = r.expected_rounded_once.to_f64();
    let rate = if r.n == 0 {
        "null".to_string()
    } else {
        format!("{:.4}", expected / r.n as f64)
    };
    format!(
        "{{\"family\":{},\"scope\":{},\"totals\":{},\"foot_exact\":{},\"distinct\":{},\"distinct_foot_exact\":{},\
         \"rounded_once_accepts\":{},\"indeterminate\":{},\"expected_rounded_once\":{expected:.3},\
         \"expected_rate\":{rate},\"ln_upper_tail\":{},\"ln_lower_tail\":{},\"ln_component_sum_error_tail\":{},\
         \"ln_rounded_once_error_tail\":{},\"discovered\":{},\"reason_code\":{}}}",
        js(d.family.as_str()),
        js(scope),
        d.totals,
        d.foot_exact,
        r.n,
        r.foot_exact,
        r.rounded_once_accepts,
        r.indeterminate,
        ln(r.ln_upper_tail),
        ln(r.ln_lower_tail),
        ln(r.ln_component_sum_error_tail),
        ln(r.ln_rounded_once_error_tail),
        js(r.discovered.as_str()),
        r.discovered
            .reason_code()
            .map_or_else(|| "null".to_string(), js)
    )
}

/// JSON string literal.
pub(crate) fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Tries a registered reconstruction for a per-ounce miss. `None` when none is registered for the instance;
/// otherwise (ii) when every label check passes and the component closes the interval, or (iv) naming the
/// check that failed.
fn find_reconstruction<'a>(
    recs: &'a [RegisteredReconstruction],
    file: &str,
    sheet: &str,
    subject: &str,
    family: IdentityFamily,
) -> Option<&'a RegisteredReconstruction> {
    recs.iter().find(|r| {
        r.table_file == file && r.sheet == sheet && r.subject == subject && r.family == family
    })
}

/// The operand at `file:sheet!cell` of the loaded sources; blank when that file or sheet is not loaded.
fn operand_in(
    sources: &[LoadedSource],
    file: &str,
    sheet: &str,
    cell: &str,
    role: OperandRole,
) -> Operand {
    let ws = sources
        .iter()
        .find(|s| s.file_name == file)
        .and_then(|s| s.workbook.sheets.iter().find(|w| w.name == sheet));
    match (ws, parse_a1_ref(cell)) {
        (Some(ws), Some((r, c))) => operand_at(file, ws, r, c, role),
        _ => Operand {
            role,
            file: file.to_string(),
            sheet: sheet.to_string(),
            cell: cell.to_string(),
            value: OperandValue::Blank,
        },
    }
}

/// The cells a registered reconstruction reads (bd-sjrh.1): its stated quote cell, its label-check cells and
/// its component. A cell on a sheet or file that is not loaded is recorded as blank.
fn reconstruction_operands(
    sources: &[LoadedSource],
    rec: &RegisteredReconstruction,
) -> Vec<Operand> {
    let mut out = Vec::new();
    let mut add = |file: &str, sheet: &str, cell: &str, role: OperandRole| {
        out.push(operand_in(sources, file, sheet, cell, role));
    };
    if let ExplanationBasis::Stated { doc, locator, .. } = &rec.basis
        && let Some((sheet, cell)) = locator.split_once('!')
    {
        add(doc, sheet, cell, OperandRole::LabelCheck);
    }
    for (cell, _) in &rec.label_checks {
        add(
            &rec.component_file,
            &rec.component_sheet,
            cell,
            OperandRole::LabelCheck,
        );
    }
    add(
        &rec.component_file,
        &rec.component_sheet,
        &rec.component_cell,
        OperandRole::Component,
    );
    out
}

fn try_reconstruction(
    sources: &[LoadedSource],
    recs: &[RegisteredReconstruction],
    file: &str,
    sheet: &str,
    subject: &str,
    family: IdentityFamily,
    miss: RateMiss,
) -> Option<IdentityState> {
    let rec = find_reconstruction(recs, file, sheet, subject, family)?;
    let fail = |check: ReconstructionCheck| {
        Some(IdentityState::Undecidable {
            why: Undecided::ReconstructionNotApplied {
                evidence_id: rec.evidence_id.clone(),
                check,
            },
            missing_fact: DEFAULT_MISSING_FACT.to_string(),
            closing_document: None,
        })
    };
    if let ExplanationBasis::Stated {
        doc,
        locator,
        quote_prefix,
    } = &rec.basis
    {
        // A stated rule counts only while the issuer's text still says it (BD-331.S2).
        let quote = locator.split_once('!').and_then(|(sheet, cell)| {
            let ws = sources
                .iter()
                .find(|s| s.file_name == *doc)?
                .workbook
                .sheets
                .iter()
                .find(|s| s.name == sheet)?;
            parse_a1_ref(cell).and_then(|(r, c)| cell_text(ws, r, c))
        });
        if !quote
            .as_deref()
            .is_some_and(|t| t.starts_with(quote_prefix))
        {
            return fail(ReconstructionCheck::QuoteMismatch {
                at: format!("{doc}:{locator}"),
                found: quote.unwrap_or_default(),
                expected_prefix: quote_prefix.clone(),
            });
        }
    }
    let Some(src) = sources.iter().find(|s| s.file_name == rec.component_file) else {
        return fail(ReconstructionCheck::ComponentFileNotLoaded {
            file: rec.component_file.clone(),
        });
    };
    let Some(cs) = src
        .workbook
        .sheets
        .iter()
        .find(|s| s.name == rec.component_sheet)
    else {
        return fail(ReconstructionCheck::NoSheet {
            sheet: rec.component_sheet.clone(),
        });
    };
    for (cell, prefix) in &rec.label_checks {
        let text = parse_a1_ref(cell).and_then(|(r, c)| cell_text(cs, r, c));
        if !text.as_deref().is_some_and(|t| t.starts_with(prefix)) {
            return fail(ReconstructionCheck::LabelMismatch {
                at: format!("{}!{cell}", rec.component_sheet),
                found: text.unwrap_or_default(),
                expected_prefix: prefix.clone(),
            });
        }
    }
    let value = parse_a1_ref(&rec.component_cell)
        .and_then(|(r, c)| cs.cell(r, c))
        .and_then(|c| c.value.as_number());
    let c = match value {
        Some(v) if v.fract() == 0.0 && v.abs() < 1e15 => v as i64,
        _ => {
            return fail(ReconstructionCheck::NotWhole {
                at: format!("{}!{}", rec.component_sheet, rec.component_cell),
            });
        }
    };
    let component = CitedCell {
        file: rec.component_file.clone(),
        sheet: rec.component_sheet.clone(),
        cell: rec.component_cell.clone(),
        value: c,
    };
    match reconstruction_holds(miss, c) {
        Ok(true) => Some(IdentityState::Explained {
            evidence_id: rec.evidence_id.clone(),
            rule: rec.rule.clone(),
            component,
            basis: rec.basis.clone(),
        }),
        Ok(false) => fail(ReconstructionCheck::DoesNotClose { component }),
        Err(_) => Some(IdentityState::Refused {
            reason: crate::aisc::RefusedReason::KernelIndeterminate,
        }),
    }
}

/// What the run knows about each subject's ownership share for the release `quarter` (BD-331.S3): regions with
/// their registered members for the release (BD-331.S4), and each registered share confirmed or not against the
/// same-release Mine Stats quarter-to-date label.
fn share_bases(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    quarter: &str,
) -> BTreeMap<String, ShareBasis> {
    let mut out: BTreeMap<String, ShareBasis> = profile
        .regions
        .iter()
        .map(|r| {
            let members = profile.members_of(r, quarter);
            (r.clone(), ShareBasis::Region { members })
        })
        .collect();
    if profile.shares.is_empty() {
        return out;
    }
    let stats = sources
        .iter()
        .find(|s| s.kind == SourceKind::MineStats && s.quarter == quarter);
    let qtd = match stats {
        None => Err(format!("no Mine Stats workbook loaded for {quarter}")),
        Some(s) => find_qtd_sheet(&s.workbook.sheets, quarter)
            .map(|sheet| (s.file_name.as_str(), sheet))
            .map_err(|why| format!("{}: {why}", s.file_name)),
    };
    for share in &profile.shares {
        let basis = match &qtd {
            Err(why) => ShareBasis::Unconfirmed { why: why.clone() },
            Ok((file, sheet)) => {
                match confirm_share_label(sheet, &share.label, share.num, share.den) {
                    Ok(cell) => ShareBasis::Confirmed {
                        num: share.num,
                        den: share.den,
                        evidence: share.evidence.clone(),
                        label_cell: format!("{file}:{}!{cell}", sheet.name),
                    },
                    Err(why) => ShareBasis::Unconfirmed {
                        why: format!("{file}: {why}"),
                    },
                }
            }
        };
        out.insert(share.subject.clone(), basis);
    }
    out
}

/// Reuses the Mine Stats reads and ledger of an earlier run with equal Mine Stats inputs (bd-r7lp). The live
/// proofs rerun [`assess`] once per perturbed cell, the ledger's exact blame dominates a run, and the ledger
/// depends only on the profile and the Mine Stats workbooks. The key is the whole profile and each Mine Stats
/// source's file name, quarter and parsed workbook, compared for equality, so a change to any of them
/// recomputes. A run through a memo gives the report [`assess`] gives. Holds one entry, its first computed run's:
/// the proofs run the unperturbed sources first, and a perturbed Mine Stats cell (a printed share label, say)
/// recomputes without displacing it. Shareable across threads: runs that miss together each compute, and the
/// first to finish sets the entry.
#[derive(Debug, Default)]
pub struct MineStatsMemo {
    entry: OnceLock<MineStatsEntry>,
    hits: AtomicU64,
    misses: AtomicU64,
}

#[derive(Debug)]
struct MineStatsEntry {
    profile: IssuerProfile,
    inputs: Vec<(String, String, Workbook)>,
    reads: Vec<(String, MineStatsRead)>,
    ledger: MineLedger,
}

impl MineStatsEntry {
    fn holds(&self, profile: &IssuerProfile, sources: &[LoadedSource]) -> bool {
        let mut current = sources.iter().filter(|s| s.kind == SourceKind::MineStats);
        self.profile == *profile
            && self.inputs.iter().all(|(file, quarter, workbook)| {
                current.next().is_some_and(|s| {
                    s.file_name == *file && s.quarter == *quarter && s.workbook == *workbook
                })
            })
            && current.next().is_none()
    }
}

impl MineStatsMemo {
    /// Runs that reused the stored entry.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Runs that computed the Mine Stats section.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}

/// Reads every Mine Stats source and builds the ledger over the reads, or takes both from `memo` when its
/// entry holds for these inputs.
fn mine_stats_section(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    memo: Option<&MineStatsMemo>,
) -> (Vec<(String, MineStatsRead)>, MineLedger) {
    if let Some(m) = memo
        && let Some(e) = m.entry.get()
        && e.holds(profile, sources)
    {
        m.hits.fetch_add(1, Ordering::Relaxed);
        return (e.reads.clone(), e.ledger.clone());
    }
    let vocabulary: LabelVocabulary = profile.mine_stats_labels.iter().cloned().collect();
    let vocabulary = (!vocabulary.is_empty()).then_some(vocabulary);
    let mine_stats: Vec<(String, MineStatsRead)> = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats)
        .map(|s| {
            // The registry already checks the quarter format; a refusal here is kept, never dropped.
            let read = read_mine_stats(&s.workbook, &s.quarter, vocabulary.as_ref())
                .unwrap_or_else(|refusal| MineStatsRead {
                    quarter: s.quarter.clone(),
                    blocks: 0,
                    values: Vec::new(),
                    refusals: vec![CellRefusal {
                        locator: s.file_name.clone(),
                        reason: refusal.to_string(),
                    }],
                    hidden_sheets: Vec::new(),
                    repeated: Vec::new(),
                });
            (s.file_name.clone(), read)
        })
        .collect();
    let files: Vec<LedgerFile<'_>> = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats)
        .zip(&mine_stats)
        .map(|(s, (_, read))| LedgerFile {
            quarter: &s.quarter,
            read,
            workbook: &s.workbook,
        })
        .collect();
    let ledger = mine_ledger(
        &files,
        &LedgerRules {
            regions: &profile.regions,
            region_members: &profile.region_members,
            statuses: &profile.subject_statuses,
            corrections: &profile.printed_share_corrections,
            aliases: &profile.mine_stats_label_aliases,
            labels: &profile.shares,
        },
    );
    if let Some(m) = memo {
        m.misses.fetch_add(1, Ordering::Relaxed);
        if m.entry.get().is_none() {
            // A run that loses the race to set the entry keeps its own result; the entry stays the first set.
            let _ = m.entry.set(MineStatsEntry {
                profile: profile.clone(),
                inputs: sources
                    .iter()
                    .filter(|s| s.kind == SourceKind::MineStats)
                    .map(|s| (s.file_name.clone(), s.quarter.clone(), s.workbook.clone()))
                    .collect(),
                reads: mine_stats.clone(),
                ledger: ledger.clone(),
            });
        }
    }
    (mine_stats, ledger)
}

/// Builds the identity ledger over every AISC table in `sources` and commits the run to a state root.
#[must_use]
pub fn assess(
    profile: &IssuerProfile,
    lane: Lane,
    sources: Vec<LoadedSource>,
) -> PublicAssuranceReport {
    assess_inner(profile, lane, sources, None)
}

/// [`assess`], taking the Mine Stats section from `memo` when its inputs are unchanged since the memo's last
/// computed run. For callers that rerun the pipeline on perturbed copies of one set of sources.
#[must_use]
pub fn assess_memo(
    profile: &IssuerProfile,
    lane: Lane,
    sources: Vec<LoadedSource>,
    memo: &MineStatsMemo,
) -> PublicAssuranceReport {
    assess_inner(profile, lane, sources, Some(memo))
}

fn assess_inner(
    profile: &IssuerProfile,
    lane: Lane,
    sources: Vec<LoadedSource>,
    memo: Option<&MineStatsMemo>,
) -> PublicAssuranceReport {
    let issuer = profile.issuer.as_str();
    let subjects = profile.subjects.as_slice();
    let reconstructions = profile.reconstructions.as_slice();
    let mut visible = HiddenScope::default();
    let mut hidden = HiddenScope::default();
    let mut one_cell = Vec::new();
    let registered: BTreeMap<String, (i64, i64)> = profile
        .shares
        .iter()
        .map(|s| (s.subject.clone(), (s.num, s.den)))
        .collect();
    for src in sources
        .iter()
        .filter(|s| s.kind == SourceKind::AiscReconciliation)
    {
        let shares = share_bases(profile, &sources, &src.quarter);
        // A registered reconciliation holds a table on its own quarter's sheet ("Q2 2026" for 2026Q2). If that
        // sheet is missing or its cost-of-sales label stops matching, the run refuses the table by name; it
        // never drops it silently (BD-338.S3's label sample found the silent drop).
        let own_sheet = src
            .quarter
            .split_once('Q')
            .map(|(year, q)| format!("Q{q} {year}"));
        if let Some(own) = &own_sheet
            && !src.workbook.sheets.iter().any(|s| s.name == *own)
        {
            visible.table_refusals.push(TableRefusal {
                file: src.file_name.clone(),
                sheet: own.clone(),
                reason: "the release's own quarter sheet is missing".to_string(),
            });
        }
        let scopes = [
            (&src.workbook.sheets, &mut visible, true),
            (&src.workbook.hidden_sheets, &mut hidden, false),
        ];
        for (sheets, scope, shown) in scopes {
            for sheet in sheets {
                match locate_table(sheet, subjects) {
                    Ok(None) if shown && own_sheet.as_ref() == Some(&sheet.name) => {
                        scope.table_refusals.push(TableRefusal {
                            file: src.file_name.clone(),
                            sheet: sheet.name.clone(),
                            reason: "no cost-of-sales row on the release's own quarter sheet"
                                .to_string(),
                        });
                    }
                    Ok(None) => {}
                    Err(reason) => scope.table_refusals.push(TableRefusal {
                        file: src.file_name.clone(),
                        sheet: sheet.name.clone(),
                        reason,
                    }),
                    Ok(Some(table)) => {
                        scope.aisc_tables += 1;
                        let before = scope.instances.len();
                        // The reconstructions a miss consulted, whose cells become operands (bd-sjrh.1).
                        let mut consulted: Vec<(String, IdentityFamily, Vec<Operand>)> = Vec::new();
                        let mut explain =
                            |subject: &str, family: IdentityFamily, miss: RateMiss| {
                                let rec = find_reconstruction(
                                    reconstructions,
                                    &src.file_name,
                                    &sheet.name,
                                    subject,
                                    family,
                                );
                                if let Some(rec) = rec {
                                    consulted.push((
                                        subject.to_string(),
                                        family,
                                        reconstruction_operands(&sources, rec),
                                    ));
                                }
                                try_reconstruction(
                                    &sources,
                                    reconstructions,
                                    &src.file_name,
                                    &sheet.name,
                                    subject,
                                    family,
                                    miss,
                                )
                            };
                        ledger_for_table(
                            &src.file_name,
                            sheet,
                            &table,
                            subjects,
                            &shares,
                            &mut explain,
                            &mut scope.instances,
                            &mut scope.unregistered_headers,
                        );
                        for (subject, family, ops) in consulted {
                            for i in scope.instances[before..]
                                .iter_mut()
                                .filter(|i| i.subject == subject && i.family == family)
                            {
                                i.operands.extend(ops.iter().cloned());
                            }
                        }
                        // A cost-of-sales rate that read the footnote also read its confirmed share label.
                        for i in scope.instances[before..].iter_mut().filter(|i| {
                            i.family == IdentityFamily::CosPerOunce
                                && i.operands.iter().any(|o| o.role == OperandRole::LabelCheck)
                        }) {
                            let label = match shares.get(&i.subject) {
                                Some(ShareBasis::Confirmed { label_cell, .. }) => {
                                    label_cell.split_once(':').and_then(|(f, rest)| {
                                        rest.rsplit_once('!').map(|(s, c)| (f, s, c))
                                    })
                                }
                                _ => None,
                            };
                            if let Some((f, s, c)) = label {
                                let op = operand_in(&sources, f, s, c, OperandRole::LabelCheck);
                                i.operands.push(op);
                            }
                        }
                        if shown {
                            one_cell_checks(
                                &src.file_name,
                                sheet,
                                &table,
                                subjects,
                                &shares,
                                &registered,
                                &mut one_cell,
                            );
                        }
                    }
                }
            }
        }
    }
    for i in visible.instances.iter_mut().chain(&mut hidden.instances) {
        name_missing_fact(i, &profile.missing_facts);
    }
    let HiddenScope {
        aisc_tables,
        instances,
        unregistered_headers: unregistered,
        table_refusals,
    } = visible;

    let (mine_stats, mine_ledger) = mine_stats_section(profile, &sources, memo);
    let mut public_claims = aisc::key_aisc_operands(profile, &sources, &instances);
    let aisc_claims: Vec<_> = public_claims.cells.values().cloned().collect();
    let mine_stats_claims = key::mine_stats::build_claims(profile, &sources, &mine_stats);
    for cell in &mine_stats_claims.claims {
        public_claims
            .cells
            .insert(cell.locator.clone(), cell.clone());
    }
    public_claims.refusals.extend(
        mine_stats_claims
            .refusals
            .iter()
            .map(|r| aisc::KeyingRefusal {
                locator: r.locator.clone(),
                reason: r.reason.clone(),
            }),
    );
    public_claims
        .refusals
        .sort_by(|a, b| (&a.locator, &a.reason).cmp(&(&b.locator, &b.reason)));
    let disclosures = key::disclosure::VerifiedDisclosureRegistry::verify(
        profile,
        &sources,
        public_claims.cells.values(),
    );
    let mine_stats_vintages = key::consistency::compare_vintages_with_disclosures(
        &mine_stats_claims.claims,
        key::consistency::VintageScope::MineStats,
        Some(&disclosures),
    );
    let aisc_vintages = key::consistency::compare_vintages_with_disclosures(
        &aisc_claims,
        key::consistency::VintageScope::Aisc,
        Some(&disclosures),
    );
    let issuer_consistency = consistency::aisc_consistency(&public_claims, &instances);
    let waterfall = waterfall::pair_sites(
        profile,
        &sources,
        &mine_stats,
        &public_claims,
        &instances,
        &mine_ledger,
    );

    // The state root binds every input digest, every census and every instance's state, so changing any
    // input cell changes the root. Version 3 adds the hidden-scope section (BD-331.S2). Version 4 binds each
    // state as reason codes and fields, never its prose, and each instance's operands (bd-sjrh.1). Version 5
    // binds each file's displayed values, which the ledger now scores (bd-jg43.3): their digest and the shown,
    // token and refused counts. Version 6 adds the Mine Stats ledger section (BD-331.S4). Version 7 adds its
    // site-sheet family and binds each instance's nil-token certificate. Version 8 adds the public
    // waterfall digest, including pair classifications, source citations, intervals and exact rate gaps.
    let mut enc = Encoder::new_frame(*b"M0AS", 8);
    enc.put_str(issuer);
    enc.put_str(lane.as_str());
    enc.put_u64(sources.len() as u64);
    for s in &sources {
        enc.put_str(&s.file_name);
        enc.put_fixed(&s.workbook.sha256);
        enc.put_fixed(&s.census.numeric_digest);
        let displayed = s.workbook.displayed_census();
        enc.put_fixed(&displayed.digest);
        for n in [
            s.census.visible_sheets,
            s.census.hidden_sheets,
            s.census.numeric_cells,
            s.census.placeholder_cells,
            s.census.formula_error_cells,
            s.census.text_cells,
            displayed.shown,
            displayed.token,
            displayed.refused,
        ] {
            enc.put_u64(n as u64);
        }
    }
    for (instances, unregistered, table_refusals) in [
        (&instances, &unregistered, &table_refusals),
        (
            &hidden.instances,
            &hidden.unregistered_headers,
            &hidden.table_refusals,
        ),
    ] {
        enc.put_u64(instances.len() as u64);
        for i in instances {
            for field in [
                i.file.as_str(),
                &i.sheet,
                &i.cell,
                &i.subject,
                i.family.as_str(),
                i.state.code(),
            ] {
                enc.put_str(field);
            }
            encode_state(&mut enc, &i.state);
            enc.put_u64(i.operands.len() as u64);
            for o in &i.operands {
                enc.put_str(o.role.as_str());
                enc.put_str(&citation::locator(o));
                match &o.value {
                    OperandValue::Int(x) => {
                        enc.put_u8(0);
                        enc.put_i64(*x);
                    }
                    OperandValue::Token(t) => {
                        enc.put_u8(1);
                        enc.put_str(t);
                    }
                    OperandValue::Blank => enc.put_u8(2),
                    OperandValue::Unrounded(t) => {
                        enc.put_u8(3);
                        enc.put_str(t);
                    }
                }
            }
        }
        enc.put_u64(unregistered.len() as u64);
        for h in unregistered {
            enc.put_str(&format!("{}:{}!{}={}", h.file, h.sheet, h.cell, h.header));
        }
        enc.put_u64(table_refusals.len() as u64);
        for t in table_refusals {
            enc.put_str(&format!("{}:{}:{}", t.file, t.sheet, t.reason));
        }
    }
    encode_mine_ledger(&mut enc, &mine_ledger);
    enc.put_fixed(&waterfall.digest().0);
    let state_root = blake3_hash(&enc.finish());
    let ledger_anchor = ledger_anchor(issuer, lane, sources.len(), state_root);
    PublicAssuranceReport {
        issuer: issuer.to_string(),
        lane,
        retrieved_utc: profile.retrieved_utc.clone(),
        through: profile.through.clone(),
        sources,
        aisc_tables,
        table_refusals,
        instances,
        unregistered_headers: unregistered,
        mine_stats,
        public_claims,
        issuer_consistency,
        mine_stats_claims,
        mine_stats_vintages,
        aisc_vintages,
        disclosures,
        waterfall,
        mine_ledger,
        hidden_scope: hidden,
        one_cell,
        ledger_anchor,
        transcriptions: None,
        recovery: None,
    }
}

/// An optional text in the root frame: a tag, then the text, so an absent value and an empty one differ.
fn opt(enc: &mut Encoder, s: Option<&str>) {
    match s {
        None => enc.put_u8(0),
        Some(s) => {
            enc.put_u8(1);
            enc.put_str(s);
        }
    }
}

/// Binds the Mine Stats ledger into the root frame (v7): every instance's identity, operands and state as codes
/// and locators (a refusal's reason, an (iv) item's needed fact), the share used and the share printed, the
/// nil-token certificate, then every infeasible component with its correction sets and implied ranges.
fn encode_mine_ledger(enc: &mut Encoder, ledger: &MineLedger) {
    enc.put_u64(ledger.instances.len() as u64);
    for i in &ledger.instances {
        for field in [
            i.family.as_str(),
            &i.subject,
            &i.field,
            i.group.as_str(),
            &i.period.code(),
            &i.anchor,
            i.state.code(),
        ] {
            enc.put_str(field);
        }
        enc.put_u64(i.cells.len() as u64);
        for c in &i.cells {
            opt(enc, c.as_deref());
        }
        match &i.state {
            mine_ledger::State::Refused { reason, at } => {
                enc.put_str(reason.code());
                if let mine_ledger::Reason::Withheld { evidence } = reason {
                    enc.put_str(evidence);
                }
                opt(enc, at.as_deref());
            }
            mine_ledger::State::Unresolved(need) => {
                enc.put_str(need.code());
            }
            _ => enc.put_u8(0),
        }
        opt(enc, i.share.map(mine_ledger::rat_text).as_deref());
        opt(enc, i.printed_share.as_deref());
        enc.put_u8(u8::from(i.nil_token));
        opt(enc, i.component.as_deref());
    }
    enc.put_u64(ledger.components as u64);
    enc.put_u64(ledger.infeasible.len() as u64);
    for c in &ledger.infeasible {
        enc.put_str(&c.id);
        enc.put_u64(c.cells.len() as u64);
        for cell in &c.cells {
            enc.put_str(cell);
        }
        enc.put_u8(u8::from(c.joint_only));
        enc.put_u8(u8::from(c.minimum));
        enc.put_u64(c.corrections.len() as u64);
        for set in &c.corrections {
            enc.put_u64(set.cells.len() as u64);
            for cell in &set.cells {
                enc.put_str(cell);
            }
            enc.put_u64(set.implied.len() as u64);
            for (cell, range) in &set.implied {
                enc.put_str(cell);
                enc.put_str(&mine_ledger::range_text(range));
            }
        }
    }
    enc.put_u64(ledger.indeterminate.len() as u64);
    for (id, reason) in &ledger.indeterminate {
        enc.put_str(&format!("{id}:{reason}"));
    }
}

/// The anchor a run commits to: everything but the state root is fixed by the issuer, the lane and the number of
/// sources read.
fn ledger_anchor(issuer: &str, lane: Lane, sources: usize, state_root: Digest32) -> LedgerAnchor {
    let cell_id = match lane {
        Lane::Public => format!("CELL-PUBLIC({issuer})"),
        Lane::Synthetic => "CELL-SYNTHETIC".to_string(),
    };
    LedgerAnchor {
        format_version: FMT_ANCHOR_V1,
        cell_id,
        ledger_epoch: 1,
        commit_sequence: sources as u64,
        adapter_registry_epoch: 1,
        schema_epoch: 1,
        policy_epoch: 1,
        privacy_epoch: 1,
        residency_epoch: 1,
        state_root,
    }
}

/// Binds a state into the root frame (v4) as codes, numbers and locators, with a tag before every optional
/// part, so rewording any prose leaves the root alone.
fn encode_state(enc: &mut Encoder, state: &IdentityState) {
    let miss = |enc: &mut Encoder, m: &RateMiss| {
        for x in [m.r, m.n, m.k, m.q] {
            enc.put_i64(x);
        }
    };
    let share = |enc: &mut Encoder, s: &StatedShare| {
        enc.put_i64(s.num);
        enc.put_i64(s.den);
        for t in [&s.evidence, &s.footnote, &s.label_cell] {
            enc.put_str(t);
        }
    };
    let cell = |enc: &mut Encoder, c: &CitedCell| {
        for t in [&c.file, &c.sheet, &c.cell] {
            enc.put_str(t);
        }
        enc.put_i64(c.value);
    };
    let weighting = |enc: &mut Encoder, w: &MemberWeighting| {
        enc.put_u64(w.used.len() as u64);
        for m in &w.used {
            for t in [&m.subject, &m.rate_cell, &m.ounces_cell] {
                enc.put_str(t);
            }
            enc.put_i64(m.rate);
            enc.put_i64(m.ounces);
        }
        for names in [&w.excluded, &w.absent] {
            enc.put_u64(names.len() as u64);
            for n in names {
                enc.put_str(n);
            }
        }
    };
    match state {
        IdentityState::Consistent {
            alternate_consistent,
            basis,
        } => {
            enc.put_u8(u8::from(*alternate_consistent));
            match basis {
                None => enc.put_u8(0),
                Some(ConsistentBasis::Share(b)) => {
                    enc.put_u8(1);
                    share(enc, b);
                }
                Some(ConsistentBasis::Members(w)) => {
                    enc.put_u8(2);
                    weighting(enc, w);
                }
                Some(ConsistentBasis::MembersExact(w)) => {
                    enc.put_u8(3);
                    weighting(enc, w);
                }
            }
        }
        IdentityState::Explained {
            evidence_id,
            rule,
            component,
            basis,
        } => {
            enc.put_str(evidence_id);
            enc.put_str(rule);
            cell(enc, component);
            match basis {
                ExplanationBasis::Reconstructed => enc.put_u8(0),
                ExplanationBasis::Stated {
                    doc,
                    locator,
                    quote_prefix,
                } => {
                    enc.put_u8(1);
                    for t in [doc, locator, quote_prefix] {
                        enc.put_str(t);
                    }
                }
            }
        }
        IdentityState::IssuerDiscrepancy(c) => {
            for x in [c.printed, c.component_sum, c.residual] {
                enc.put_i64(x);
            }
            enc.put_str(&c.total_cell);
            enc.put_u64(c.line_cells.len() as u64);
            for l in &c.line_cells {
                enc.put_str(l);
            }
            opt(enc, c.sign_flip_witness.as_deref());
            enc.put_u8(u8::from(c.alternate_consistent));
        }
        IdentityState::Undecidable {
            why,
            missing_fact,
            closing_document,
        } => {
            enc.put_str(why.code());
            match why {
                Undecided::RateOutside(m) => miss(enc, m),
                Undecided::ShareOutside {
                    miss: m,
                    share: s,
                    s_interval,
                } => {
                    miss(enc, m);
                    share(enc, s);
                    enc.put_str(s_interval);
                }
                Undecided::ReconstructionNotApplied { evidence_id, check } => {
                    enc.put_str(evidence_id);
                    enc.put_str(check.code());
                    match check {
                        ReconstructionCheck::QuoteMismatch {
                            at,
                            found,
                            expected_prefix,
                        }
                        | ReconstructionCheck::LabelMismatch {
                            at,
                            found,
                            expected_prefix,
                        } => {
                            for t in [at, found, expected_prefix] {
                                enc.put_str(t);
                            }
                        }
                        ReconstructionCheck::ComponentFileNotLoaded { file: t }
                        | ReconstructionCheck::NoSheet { sheet: t }
                        | ReconstructionCheck::NotWhole { at: t } => enc.put_str(t),
                        ReconstructionCheck::DoesNotClose { component } => cell(enc, component),
                    }
                }
                Undecided::RegionOutsideMembers {
                    weighting: w,
                    certificate,
                } => {
                    weighting(enc, w);
                    enc.put_str(certificate);
                }
            }
            enc.put_str(missing_fact);
            opt(enc, closing_document.as_deref());
        }
        IdentityState::Refused { reason } => enc.put_str(reason.code()),
        IdentityState::RefusedAt { reason, at } => {
            enc.put_str(reason.code());
            enc.put_str(at);
        }
        IdentityState::NotPublished { reason } => enc.put_str(reason.code()),
    }
}

fn name_missing_fact(instance: &mut IdentityInstance, registered: &[RegisteredMissingFact]) {
    let IdentityState::Undecidable {
        missing_fact,
        closing_document,
        ..
    } = &mut instance.state
    else {
        return;
    };
    if let Some(row) = registered
        .iter()
        .find(|m| m.subject == instance.subject && m.family == instance.family)
    {
        *missing_fact = format!("{} ({})", row.text, row.evidence);
        closing_document.clone_from(&row.closing_document);
    }
}

/// `fmine assure public --issuer <issuer>`: the gate-eligible run over the registered public workbooks.
///
/// The issuer name `synthetic` is reserved for the invented workbooks: a registry under that name (for
/// example one written by [`synthetic::write_issuer_workspace`]) runs in the synthetic lane and is never
/// gate-eligible, though it goes through the same registry and digest checks.
pub fn run_public_assurance(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<PublicAssuranceReport, Refusal> {
    let sources = load_registered_sources(profile, root)?;
    // Documents carry no release quarter, so a `--through` run, which reproduces an earlier registry, reads no
    // transcriptions.
    let transcriptions = if profile.through.is_some() {
        None
    } else {
        transcribe::load_transcription_report(profile, root)?
    };
    let lane = if profile.issuer == synthetic::SYNTHETIC_ISSUER {
        Lane::Synthetic
    } else {
        Lane::Public
    };
    let mut report = assess(profile, lane, sources);
    report.recovery = recovery::assess(
        profile,
        transcriptions.as_ref(),
        &report.mine_stats_claims.claims,
    )?;
    report.transcriptions = transcriptions;
    Ok(report)
}

/// The Barrick M0 run (`GATE-BULL-01`): registry and corpus both under `root`.
pub fn run_barrick_m0_assurance(root: &Path) -> Result<PublicAssuranceReport, Refusal> {
    run_public_assurance(&sources::load_issuer_profile(root, "barrick")?, root)
}

/// `fmine assure public --synthetic`: the known-answer run over invented workbooks, with optional edits to
/// the synthetic AISC table (perturbation tests).
pub fn run_synthetic_assurance(
    edits: &[(String, fmine_io::xlsx::MinimalCell)],
) -> Result<PublicAssuranceReport, Refusal> {
    let sources = load_synthetic(synthetic::synthetic_workbooks(edits))?;
    Ok(assess(
        &synthetic::synthetic_profile(),
        Lane::Synthetic,
        sources,
    ))
}

/// Parses in-memory synthetic workbooks `(file, kind, quarter, bytes)` into loaded sources.
fn load_synthetic(
    files: Vec<(&'static str, SourceKind, &'static str, Vec<u8>)>,
) -> Result<Vec<LoadedSource>, Refusal> {
    let mut sources = Vec::new();
    for (file, kind, quarter, bytes) in files {
        let workbook = Workbook::from_bytes(file, &bytes)
            .map_err(|e| input_refusal(format!("synthetic {file}: {e}")))?;
        let census = workbook.cell_census();
        sources.push(LoadedSource {
            file_name: file.to_string(),
            kind,
            quarter: quarter.to_string(),
            url: String::new(),
            url_basis: "synthetic".to_string(),
            bytes: bytes.len() as u64,
            workbook,
            census,
        });
    }
    Ok(sources)
}

include!("../../xtask/testdata/public_lib_unit_tests.rs");
