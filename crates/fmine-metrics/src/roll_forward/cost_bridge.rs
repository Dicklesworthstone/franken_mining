//! DESIGN: source-bound, symmetric cost and sold-volume effects between two declared PDF cost rates.
//! Determinism: Exact (rational points and conservative rational enclosures).
//!
//! Cost is independently printed USD millions; sales are explicitly Mlb for copper or koz for gold.
//! The printed rate is used only for the separate reconciliation. No cost is reconstructed from it.

use std::collections::{BTreeMap, BTreeSet};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::table_identities::CostRate;
use fmine_units::UnitsError;

use super::{
    SourceCell, TableIdentity, TableIdentityRegistry, TableReport, TermRole, canonical_readings,
    convention, definition_json, interval, js, rat, sha256, source_cell_json, xf,
};
use xf::{Interval, Rat};

pub const NO_CLAIM: &str = "Descriptive accounting bridge of independently printed cost and sold-volume assertions under an explicitly matched subject, ownership and definition. Arithmetic closure does not identify operational causes or verify accounting policy. Source bytes and reader agreement remain caller obligations. Selected filing vintages are retained; no first-publication or forecasting claim.";
const RULE: &str = "pdf-cost-volume/1";
const ONE: Rat = Rat::from_integer(1);
const TWO: Rat = Rat::from_integer(2);
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derived {
    pub point: Rat,
    pub interval: Interval,
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bar {
    pub id: &'static str,
    pub value: Derived,
    pub sign_not_determined: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub identity: String,
    pub period: String,
    pub recomputed_rate: Derived,
    pub printed_rate: Derived,
    pub printed_rate_gap: Derived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostBridge {
    pub issuer: String,
    pub subject: String,
    pub ownership: String,
    pub definition: String,
    pub unit: CostRate,
    pub definitions: [TableIdentity; 2],
    pub inputs: Vec<SourceCell>,
    pub endpoints: [Endpoint; 2],
    pub bars: [Bar; 2],
    pub recomputed_change: Derived,
    pub printed_change: Derived,
    pub printed_rate_gap_change: Derived,
    pub table_check_digest: String,
}

fn bad(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("PDF cost bridge: {}", detail.into()))
}

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact arithmetic overflow",
        )
    })
}

fn incompatible(detail: impl Into<String>) -> Refusal {
    bad(RefusalCode::BasisMismatch105, detail)
}

fn identity<'a>(report: &'a TableReport, id: &str) -> Result<&'a TableIdentity> {
    let found: Vec<_> = report.definitions.iter().filter(|d| d.id == id).collect();
    match found.as_slice() {
        [d] => Ok(d),
        _ => Err(incompatible(format!(
            "exactly one declared identity must match {id}"
        ))),
    }
}

fn quantity(d: &TableIdentity, role: TermRole) -> Result<&str> {
    let found: Vec<_> = d.terms.iter().filter(|t| t.role == role).collect();
    match found.as_slice() {
        [t] => Ok(&t.quantity),
        _ => Err(incompatible(
            "each endpoint needs one numerator and one sold denominator",
        )),
    }
}

fn quantities(d: &TableIdentity) -> impl Iterator<Item = &str> {
    std::iter::once(d.total.as_str()).chain(d.terms.iter().map(|t| t.quantity.as_str()))
}

// A shared arbitrary word such as "attributable" cannot establish an unchanged ownership share.
fn basis(value: &str) -> Result<(String, String, String)> {
    let mut fields = BTreeMap::new();
    for part in value.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            return Err(incompatible(
                "basis requires subject, ownership, definition and denominator=sold",
            ));
        };
        if !matches!(key, "subject" | "ownership" | "definition" | "denominator")
            || value.is_empty()
            || value.trim() != value
            || value.chars().any(char::is_control)
            || fields.insert(key, value).is_some()
        {
            return Err(incompatible(
                "basis has an unknown, empty or duplicate field",
            ));
        }
    }
    let get = |key| {
        fields
            .get(key)
            .copied()
            .ok_or_else(|| incompatible(format!("basis lacks {key}")))
    };
    let subject = get("subject")?;
    let ownership = get("ownership")?;
    let definition = get("definition")?;
    if get("denominator")? != "sold"
        || !definition.split_once('@').is_some_and(|(name, version)| {
            !name.is_empty() && !version.is_empty() && !definition.chars().any(char::is_whitespace)
        })
    {
        return Err(incompatible(
            "basis needs a named versioned definition and denominator=sold",
        ));
    }
    if ownership != "100%" {
        let share = ownership
            .strip_prefix("attributable:")
            .and_then(|v| v.split_once('/'))
            .and_then(|(n, d)| Some((n.parse::<i128>().ok()?, d.parse::<i128>().ok()?)))
            .filter(|(n, d)| *n > 0 && *d > 0 && n <= d);
        if share.is_none() {
            return Err(incompatible(
                "ownership must be 100% or attributable:n/d with 0 < n/d <= 1",
            ));
        }
    }
    Ok((subject.into(), ownership.into(), definition.into()))
}

fn period(code: &str) -> Result<(u32, u32)> {
    if code.len() != 6 || !code.is_ascii() || !code.as_bytes()[..4].iter().all(u8::is_ascii_digit) {
        return Err(incompatible(
            "periods must explicitly name a calendar FY, half-year or quarter",
        ));
    }
    let year = code[..4]
        .parse::<u32>()
        .ok()
        .filter(|y| *y > 0)
        .ok_or_else(|| incompatible("invalid calendar year"))?;
    let (first, last) = match &code[4..] {
        "FY" => (1, 12),
        "H1" => (1, 6),
        "H2" => (7, 12),
        "Q1" => (1, 3),
        "Q2" => (4, 6),
        "Q3" => (7, 9),
        "Q4" => (10, 12),
        _ => return Err(incompatible("unsupported calendar period")),
    };
    Ok((year * 12 + first, year * 12 + last))
}

/// Rebind and solve the selected identities and every connected declared component. Cached feasible
/// labels cannot hide a contradictory cost subtotal or changes to a retained source cell.
fn replay(
    report: &TableReport,
    before: &TableIdentity,
    after: &TableIdentity,
) -> Result<TableReport> {
    let mut ids = BTreeSet::from([before.id.clone(), after.id.clone()]);
    loop {
        let keys: BTreeSet<_> = report
            .definitions
            .iter()
            .filter(|d| ids.contains(&d.id))
            .flat_map(|d| quantities(d).map(move |q| (d.doc.as_str(), q)))
            .collect();
        let next: Vec<_> = report
            .definitions
            .iter()
            .filter(|d| quantities(d).any(|q| keys.contains(&(d.doc.as_str(), q))))
            .map(|d| d.id.clone())
            .collect();
        let old_len = ids.len();
        ids.extend(next);
        if ids.len() == old_len {
            break;
        }
    }
    if report
        .refused
        .iter()
        .any(|r| r.identity == "registry" || ids.contains(&r.identity))
    {
        return Err(bad(
            RefusalCode::SourceCellUnusable208,
            "a selected or connected identity was refused",
        ));
    }
    for id in &ids {
        let checks: Vec<_> = report
            .components
            .iter()
            .filter(|c| c.component.identities.contains(id))
            .collect();
        if !matches!(checks.as_slice(), [c] if matches!(c.component.outcome, xf::Outcome::Feasible { .. }))
        {
            return Err(bad(
                RefusalCode::NonIdentifiableAttribution104,
                format!("identity {id} needs a checked feasible connected component"),
            ));
        }
    }
    let definitions: Vec<_> = report
        .definitions
        .iter()
        .filter(|d| ids.contains(&d.id))
        .cloned()
        .collect();
    let keys: BTreeSet<_> = definitions
        .iter()
        .flat_map(|d| quantities(d).map(move |q| (d.doc.as_str(), q)))
        .collect();
    let mut documents = BTreeMap::new();
    let mut readings = Vec::new();
    for s in report
        .cells
        .iter()
        .filter(|s| keys.contains(&(s.source.locator.doc.as_str(), s.source.quantity.as_str())))
    {
        if documents
            .insert(s.document.file.clone(), s.document.clone())
            .is_some_and(|old| old != s.document)
        {
            return Err(incompatible(
                "one document has conflicting retained registration metadata",
            ));
        }
        readings.push(s.source.clone());
    }
    canonical_readings(&mut readings);
    let registry = TableIdentityRegistry {
        issuer: report.issuer.clone(),
        identities: definitions,
        reserve_rows: Vec::new(),
        geo_checks: Vec::new(),
        cutoffs: Vec::new(),
    };
    let checked = super::assess(
        &registry,
        &readings,
        &documents.into_values().collect::<Vec<_>>(),
    );
    if !checked.refused.is_empty()
        || checked
            .components
            .iter()
            .any(|c| !matches!(c.component.outcome, xf::Outcome::Feasible { .. }))
    {
        return Err(bad(
            RefusalCode::NonIdentifiableAttribution104,
            "source rebinding or exact connected identity replay failed",
        ));
    }
    for cell in &checked.cells {
        let original: Vec<_> = report
            .cells
            .iter()
            .filter(|s| s.cell.locator == cell.cell.locator)
            .collect();
        if !matches!(original.as_slice(), [s] if *s == cell) {
            return Err(bad(
                RefusalCode::FrozenRecordMutation507,
                "retained numeric cell differs from its source-bound reading",
            ));
        }
    }
    Ok(checked)
}

fn closed(lo: Rat, hi: Rat) -> Interval {
    Interval {
        lo,
        hi,
        lo_strict: false,
        hi_strict: false,
    }
}

fn point(value: Rat) -> Interval {
    closed(value, value)
}

fn add(a: Interval, b: Interval) -> Result<Interval> {
    Ok(closed(
        exact(a.lo.checked_add(b.lo))?,
        exact(a.hi.checked_add(b.hi))?,
    ))
}

fn sub(a: Interval, b: Interval) -> Result<Interval> {
    Ok(closed(
        exact(a.lo.checked_sub(b.hi))?,
        exact(a.hi.checked_sub(b.lo))?,
    ))
}

fn mul(a: Interval, b: Interval) -> Result<Interval> {
    let mut lo = exact(a.lo.checked_mul(b.lo))?;
    let mut hi = lo;
    for v in [
        a.lo.checked_mul(b.hi),
        a.hi.checked_mul(b.lo),
        a.hi.checked_mul(b.hi),
    ] {
        let v = exact(v)?;
        if exact(v.checked_cmp(lo))?.is_lt() {
            lo = v;
        }
        if exact(v.checked_cmp(hi))?.is_gt() {
            hi = v;
        }
    }
    Ok(closed(lo, hi))
}

fn inverse(a: Interval) -> Result<Interval> {
    if a.lo.signum() <= 0 {
        return Err(bad(
            RefusalCode::NonIdentifiableAttribution104,
            "the entire sold-volume rounding interval must be strictly positive",
        ));
    }
    Ok(closed(
        exact(ONE.checked_div(a.hi))?,
        exact(ONE.checked_div(a.lo))?,
    ))
}

fn derived(value: Rat, interval: Interval, inputs: impl IntoIterator<Item = String>) -> Derived {
    Derived {
        point: value,
        interval,
        inputs: inputs
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

fn difference(a: &Derived, b: &Derived) -> Result<Derived> {
    Ok(derived(
        exact(a.point.checked_sub(b.point))?,
        sub(a.interval, b.interval)?,
        a.inputs.iter().chain(&b.inputs).cloned(),
    ))
}

fn input(report: &TableReport, d: &TableIdentity, quantity: &str) -> Result<Derived> {
    let candidates: Vec<_> = report
        .cells
        .iter()
        .filter(|s| s.source.locator.doc == d.doc && s.source.quantity == quantity)
        .collect();
    let [cell] = candidates.as_slice() else {
        return Err(bad(
            RefusalCode::SourceCellUnusable208,
            "one source cell per declared operand is required",
        ));
    };
    let half = exact(cell.cell.step.checked_div(TWO))?;
    let bounds = convention(&d.convention)
        .interval(cell.cell.printed, half)
        .map_err(|_| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "source rounding interval failed",
            )
        })?;
    Ok(derived(
        cell.cell.printed,
        closed(bounds.lo, bounds.hi),
        [cell.cell.locator.clone()],
    ))
}

fn endpoint(
    report: &TableReport,
    d: &TableIdentity,
    unit: CostRate,
) -> Result<(Endpoint, Derived, Derived)> {
    let cost = input(report, d, quantity(d, TermRole::Numerator)?)?;
    let sold = input(report, d, quantity(d, TermRole::Denominator)?)?;
    let printed_rate = input(report, d, &d.total)?;
    let scale = Rat::from_integer(i128::from(unit.factor));
    let rate = exact(exact(cost.point.checked_div(sold.point))?.checked_mul(scale))?;
    let recomputed_rate = derived(
        rate,
        mul(mul(cost.interval, inverse(sold.interval)?)?, point(scale))?,
        cost.inputs.iter().chain(&sold.inputs).cloned(),
    );
    let printed_rate_gap = difference(&printed_rate, &recomputed_rate)?;
    Ok((
        Endpoint {
            identity: d.id.clone(),
            period: d.total_period.clone(),
            recomputed_rate,
            printed_rate,
            printed_rate_gap,
        },
        cost,
        sold,
    ))
}

/// Explicit selection only. Equal semantic quantity ids, family and source-bound basis are required;
/// sequential calendar supports must have the same kind, without comparing their elapsed day counts.
pub fn assess(report: &TableReport, before_id: &str, after_id: &str) -> Result<CostBridge> {
    let before = identity(report, before_id)?;
    let after = identity(report, after_id)?;
    let unit = before
        .kind
        .cost_rate()
        .ok_or_else(|| incompatible("a sold-metal cost-rate identity is required"))?;
    if before_id == after_id
        || before.kind != after.kind
        || before.family != after.family
        || before.total != after.total
        || before.basis != after.basis
        || before.unit != after.unit
        || quantity(before, TermRole::Numerator)? != quantity(after, TermRole::Numerator)?
        || quantity(before, TermRole::Denominator)? != quantity(after, TermRole::Denominator)?
    {
        return Err(incompatible(
            "endpoints must share metal, metric family, semantic operands, unit and explicit basis",
        ));
    }
    let (subject, ownership, definition) = basis(&before.basis)?;
    let ((first0, last0), (first1, last1)) =
        (period(&before.total_period)?, period(&after.total_period)?);
    if last0 >= first1 || last0 - first0 != last1 - first1 {
        return Err(incompatible(
            "periods must advance without overlap on equal calendar support kinds",
        ));
    }
    let checked = replay(report, before, after)?;
    let (e0, c0, q0) = endpoint(&checked, before, unit)?;
    let (e1, c1, q1) = endpoint(&checked, after, unit)?;
    let scale = Rat::from_integer(i128::from(unit.factor));
    let half = exact(ONE.checked_div(TWO))?;
    let (w0, w1) = (
        exact(ONE.checked_div(q0.point))?,
        exact(ONE.checked_div(q1.point))?,
    );
    let cost_point = exact(
        exact(c1.point.checked_sub(c0.point))?
            .checked_mul(exact(exact(w0.checked_add(w1))?.checked_mul(half))?),
    )?;
    let volume_point = exact(
        exact(exact(c0.point.checked_add(c1.point))?.checked_mul(half))?
            .checked_mul(exact(w1.checked_sub(w0))?),
    )?;
    let (w0_box, w1_box) = (inverse(q0.interval)?, inverse(q1.interval)?);
    let cost_box = mul(
        sub(c1.interval, c0.interval)?,
        mul(add(w0_box, w1_box)?, point(half))?,
    )?;
    let volume_box = mul(
        mul(add(c0.interval, c1.interval)?, point(half))?,
        sub(w1_box, w0_box)?,
    )?;
    let inputs: Vec<_> = c0
        .inputs
        .iter()
        .chain(&c1.inputs)
        .chain(&q0.inputs)
        .chain(&q1.inputs)
        .cloned()
        .collect();
    let bar = |id, value: Rat, interval: Interval| -> Result<Bar> {
        let interval = mul(interval, point(scale))?;
        Ok(Bar {
            id,
            value: derived(exact(value.checked_mul(scale))?, interval, inputs.clone()),
            sign_not_determined: interval.lo.signum() <= 0 && interval.hi.signum() >= 0,
        })
    };
    let bars = [
        bar("cost", cost_point, cost_box)?,
        bar("sales_volume", volume_point, volume_box)?,
    ];
    let recomputed_change = difference(&e1.recomputed_rate, &e0.recomputed_rate)?;
    if exact(bars[0].value.point.checked_add(bars[1].value.point))? != recomputed_change.point {
        return Err(bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact symmetric effects do not close",
        ));
    }
    let printed_change = difference(&e1.printed_rate, &e0.printed_rate)?;
    let printed_rate_gap_change = difference(&e1.printed_rate_gap, &e0.printed_rate_gap)?;
    Ok(CostBridge {
        issuer: report.issuer.clone(),
        subject,
        ownership,
        definition,
        unit,
        definitions: [before.clone(), after.clone()],
        inputs: checked.cells,
        endpoints: [e0, e1],
        bars,
        recomputed_change,
        printed_change,
        printed_rate_gap_change,
        table_check_digest: report.digest(),
    })
}

impl Derived {
    fn json(&self) -> String {
        format!(
            "{{\"point\":{},\"interval\":{},\"in\":{}}}",
            rat(self.point),
            interval(&self.interval),
            super::strings(&self.inputs)
        )
    }
}

impl CostBridge {
    fn body(&self) -> String {
        let endpoints = self.endpoints.iter().map(|e| format!(
            "{{\"identity\":{},\"period\":{},\"recomputed_rate\":{},\"printed_rate\":{},\"printed_rate_gap\":{}}}",
            js(&e.identity), js(&e.period), e.recomputed_rate.json(), e.printed_rate.json(), e.printed_rate_gap.json()
        )).collect::<Vec<_>>().join(",");
        let bars = self.bars.iter().map(|b| format!(
            "{{\"id\":{},\"point\":{},\"interval\":{},\"sign_not_determined\":{},\"in\":{}}}",
            js(b.id), rat(b.value.point), interval(&b.value.interval), b.sign_not_determined,
            super::strings(&b.value.inputs)
        )).collect::<Vec<_>>().join(",");
        let definitions = self
            .definitions
            .iter()
            .map(definition_json)
            .collect::<Vec<_>>()
            .join(",");
        let inputs = self
            .inputs
            .iter()
            .map(source_cell_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.pdf-cost-bridge.v1\",\"rules_version\":{},\"issuer\":{},\"subject\":{},\"ownership\":{},\"definition\":{},\"unit\":{},\"numerator_unit\":{},\"denominator_unit\":{},\"factor\":{},\"descriptive_only\":true,\"identifying\":false,\"rounding\":\"closed conservative rational enclosures of declared source rounding boxes; shared dependencies not narrowed; not confidence intervals\",\"table_check_digest\":{},\"definitions\":[{definitions}],\"inputs\":[{inputs}],\"endpoints\":[{endpoints}],\"bars\":[{bars}],\"recomputed_change\":{},\"printed_change\":{},\"printed_rate_gap_change\":{},\"no_claim\":{}}}",
            js(RULE),
            js(&self.issuer),
            js(&self.subject),
            js(&self.ownership),
            js(&self.definition),
            js(self.unit.unit),
            js(self.unit.numerator_unit),
            js(self.unit.denominator_unit),
            self.unit.factor,
            js(&self.table_check_digest),
            self.recomputed_change.json(),
            self.printed_change.json(),
            self.printed_rate_gap_change.json(),
            js(NO_CLAIM)
        )
    }

    #[must_use]
    pub fn digest(&self) -> String {
        sha256(self.body().as_bytes()).to_hex()
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let body = self.body();
        format!("{{\"digest\":{},{}", js(&self.digest()), &body[1..])
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let number = |v: Rat| {
            if v.den() == 1 {
                v.num().to_string()
            } else {
                format!("{}/{}", v.num(), v.den())
            }
        };
        let bounds = |v: Interval| format!("[{}, {}]", number(v.lo), number(v.hi));
        let escaped = |s: &str| s.replace('|', "\\|").replace(['\n', '\r'], " ");
        let mut out = format!(
            "\n## Cost and sold-volume bridge\n\n{}: {} → {}, in {}. Definition {}, ownership {}.\n\n{NO_CLAIM}\n\n| Effect | Exact midpoint | Rounding enclosure | Sign |\n|---|---:|---|---|\n",
            escaped(&self.subject),
            self.endpoints[0].period,
            self.endpoints[1].period,
            self.unit.unit,
            escaped(&self.definition),
            escaped(&self.ownership)
        );
        for bar in &self.bars {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                bar.id,
                number(bar.value.point),
                bounds(bar.value.interval),
                if bar.sign_not_determined {
                    "sign not determined"
                } else {
                    "determined"
                }
            ));
        }
        out.push_str(&format!("\nRecomputed change: {}. Printed change: {}. Separate printed-rate gap change: {}.\n\n| Endpoint | Recomputed rate | Printed rate | Separate gap |\n|---|---:|---:|---:|\n",
            number(self.recomputed_change.point), number(self.printed_change.point), number(self.printed_rate_gap_change.point)));
        for e in &self.endpoints {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                e.period,
                number(e.recomputed_rate.point),
                number(e.printed_rate.point),
                number(e.printed_rate_gap.point)
            ));
        }
        out.push_str("\nAll enclosures retain source rounding; they are not confidence intervals.\n\n| Source operand | Printed token | SHA-256 |\n|---|---|---|\n");
        for s in &self.inputs {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                escaped(&s.cell.locator),
                escaped(&s.source.token),
                s.document.sha256
            ));
        }
        out.push_str(&format!(
            "\nBridge digest: {}. Parent table-check digest: {}.\n",
            self.digest(),
            self.table_check_digest
        ));
        out
    }
}
