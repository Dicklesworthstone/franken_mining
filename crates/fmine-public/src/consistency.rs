//! DESIGN (bd-504e.1): identity counts at the first registered vintage, and changes between adjacent vintages.
//! Comparatives share the total's semantic key, never its spreadsheet coordinates. These are findings about
//! registered public assertions, not a claim that the earliest registered file was the first publication.

use std::collections::{BTreeMap, BTreeSet};

use crate::aisc::{AiscClaimIndex, IdentityFamily, IdentityInstance, OperandRole, OperandValue};
use crate::key::{KeyedCell, SeriesKey};

pub mod mine_stats;

pub const FIRST_REGISTERED_CAVEAT: &str = "Earliest registered vintage, not necessarily the issuer's first publication. Hidden sheets are excluded.";
pub type FamilyCounts<F = IdentityFamily> = BTreeMap<(F, &'static str), usize>;

/// The existing AISC and Mine Stats ledgers keep distinct families and output schemas.
pub trait ConsistencyFamily: Copy + Ord + 'static {
    const ALL: &'static [Self];
    const SCHEMA: &'static str;
    fn as_str(self) -> &'static str;
}

impl ConsistencyFamily for IdentityFamily {
    const ALL: &'static [Self] = &IdentityFamily::ALL;
    const SCHEMA: &'static str = "fmine.issuer_consistency.aisc.v1";
    fn as_str(self) -> &'static str {
        self.as_str()
    }
}

/// The state at one cited total or rate, with its registered release and source digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appearance {
    pub release: String,
    pub at: String,
    pub sha256: String,
    pub state: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    Persisted,
    Corrected,
    Broken,
    Reclassified,
    SingleVintage,
}

impl TransitionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Persisted => "persisted",
            Self::Corrected => "corrected",
            Self::Broken => "broken",
            Self::Reclassified => "reclassified",
            Self::SingleVintage => "single_vintage",
        }
    }
}

/// One adjacent-vintage transition. Indices refer to `IdentityHistory::sequence` so both citations survive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub from: usize,
    pub to: usize,
    pub kind: TransitionKind,
    /// `None` when an operand could not be keyed; that comparison never proves persistence or correction.
    pub operands_changed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityHistory<F = IdentityFamily> {
    pub series: SeriesKey,
    pub family: F,
    pub sequence: Vec<Appearance>,
    pub transitions: Vec<Transition>,
    /// Multiple different assertions in one registered release are kept, not arbitrarily selected.
    pub ambiguous_vintages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnkeyedIdentity<F = IdentityFamily> {
    pub at: String,
    pub family: F,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consistency<F = IdentityFamily> {
    pub first_registered: FamilyCounts<F>,
    pub all_files: FamilyCounts<F>,
    pub histories: Vec<IdentityHistory<F>>,
    /// Included in `all_files`, excluded from the first-registered denominator, and named individually.
    pub unkeyed: Vec<UnkeyedIdentity<F>>,
}

pub type AiscConsistency = Consistency<IdentityFamily>;

impl<F> Default for Consistency<F> {
    fn default() -> Self {
        Self {
            first_registered: BTreeMap::new(),
            all_files: BTreeMap::new(),
            histories: Vec::new(),
            unkeyed: Vec::new(),
        }
    }
}

impl<F: ConsistencyFamily> Consistency<F> {
    /// Stable diagnostic output. Counts are per identity family, with `np` explicitly inside the denominator.
    /// The binomial intervals describe those counts; they are neither a forecast-calibration claim nor a gate.
    #[must_use]
    pub fn to_json(&self) -> String {
        use crate::js;
        let histories = self.histories.iter().map(|h| {
            let sequence = h.sequence.iter().map(appearance_json).collect::<Vec<_>>().join(",");
            let first = h.sequence.first();
            let total = first.map_or_else(|| "null".to_string(), |a| format!(
                "{{\"at\":{},\"sha256\":{}}}", js(&a.at), js(&a.sha256)));
            let first_registered = first.filter(|a| h.ambiguous_vintages.iter().all(|v| v > &a.release))
                .map_or_else(|| "null".to_string(), appearance_json);
            let steps = h.transitions.iter().map(|t| format!(
                "{{\"from\":{},\"to\":{},\"kind\":{},\"operands_changed\":{}}}",
                t.from, t.to, js(t.kind.as_str()),
                t.operands_changed.map_or_else(|| "null".to_string(), |b| b.to_string())
            )).collect::<Vec<_>>().join(",");
            let ambiguous = h.ambiguous_vintages.iter().map(|v| js(v)).collect::<Vec<_>>().join(",");
            format!(
                "{{\"family\":{},\"subject\":{},\"period\":{},\"total\":{},\
                 \"first_registered\":{},\"sequence\":[{}],\"steps\":[{}],\"ambiguous_vintages\":[{}]}}",
                js(h.family.as_str()), js(&h.series.subject), js(&h.series.valid_period.code()),
                total, first_registered, sequence, steps, ambiguous,
            )
        }).collect::<Vec<_>>().join(",");
        let unkeyed = self
            .unkeyed
            .iter()
            .map(|u| {
                format!(
                    "{{\"at\":{},\"family\":{},\"reason\":{}}}",
                    js(&u.at),
                    js(u.family.as_str()),
                    js(&u.reason)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"caveat\":{},\
             \"within_vintage\":{{\"denominator\":\"family total including not-published (np); first_registered excludes unkeyed totals and ambiguous earliest releases\",\
             \"interval_method\":\"equal-tailed 95% Clopper-Pearson, descriptive binomial interval; null for an empty family or more than 100000 trials\",\
             \"first_registered\":{},\"all_files\":{}}},\"transitions\":[{}],\"unkeyed\":[{}]}}",
            js(F::SCHEMA),
            js(FIRST_REGISTERED_CAVEAT),
            family_counts_json(&self.first_registered),
            family_counts_json(&self.all_files),
            histories,
            unkeyed,
        )
    }
}

fn appearance_json(a: &Appearance) -> String {
    use crate::js;
    format!(
        "{{\"release\":{},\"at\":{},\"sha256\":{},\"state\":{}}}",
        js(&a.release),
        js(&a.at),
        js(&a.sha256),
        js(a.state)
    )
}

fn family_counts_json<F: ConsistencyFamily>(counts: &FamilyCounts<F>) -> String {
    let families = F::ALL
        .iter()
        .copied()
        .map(|family| {
            let total = counts
                .iter()
                .filter(|((f, _), _)| *f == family)
                .map(|(_, n)| n)
                .sum::<usize>();
            let states = ["i", "ii", "iii", "iv", "v", "np"]
                .into_iter()
                .map(|state| {
                    let n = counts.get(&(family, state)).copied().unwrap_or(0);
                    let rate = if total == 0 {
                        "null".to_string()
                    } else {
                        (n as f64 / total as f64).to_string()
                    };
                    let interval = clopper_pearson_95(n, total)
                        .map_or_else(|| "null".to_string(), |(lo, hi)| format!("[{lo},{hi}]"));
                    format!("\"{state}\":{{\"n\":{n},\"rate\":{rate},\"cp95\":{interval}}}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "\"{}\":{{\"n\":{total},\"states\":{{{states}}}}}",
                family.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{families}}}")
}

/// The caller supplies visible instances only. A missing key never drops an all-file instance or invents
/// a period. Ambiguous earliest vintages are retained in the history and excluded from first-registered counts.
#[must_use]
pub fn aisc_consistency(index: &AiscClaimIndex, instances: &[IdentityInstance]) -> AiscConsistency {
    let mut report = AiscConsistency::default();
    let mut groups: BTreeMap<_, Vec<(&IdentityInstance, &KeyedCell)>> = BTreeMap::new();
    for instance in instances {
        *report
            .all_files
            .entry((instance.family, instance.state.code()))
            .or_default() += 1;
        let at = format!("{}:{}!{}", instance.file, instance.sheet, instance.cell);
        if let Some(total) = index.cells.get(&at) {
            groups
                .entry((total.key.series_key(), instance.family))
                .or_default()
                .push((instance, total));
        } else {
            let reason = index.refusals.iter().find(|r| r.locator == at).map_or_else(
                || "identity total has no semantic key".to_string(),
                |r| r.reason.clone(),
            );
            report.unkeyed.push(UnkeyedIdentity {
                at,
                family: instance.family,
                reason,
            });
        }
    }
    report
        .unkeyed
        .sort_by(|a, b| (&a.at, a.family).cmp(&(&b.at, b.family)));
    finish_histories(
        groups,
        &mut report,
        |i| i.state.code(),
        |a, b| {
            a.state.code() == b.state.code()
                && a.state.label() == b.state.label()
                && a.state.reason_code() == b.state.reason_code()
        },
        |i| operand_signature(index, i),
        &BTreeSet::new(),
    );
    report
}

/// Shared grouping policy: an ambiguous release can never prove persistence or a correction.
fn finish_histories<T: PartialEq + std::fmt::Debug, F: ConsistencyFamily, S: Eq>(
    groups: BTreeMap<(SeriesKey, F), Vec<(&T, &KeyedCell)>>,
    report: &mut Consistency<F>,
    code: impl Fn(&T) -> &'static str,
    same_state: impl Fn(&T, &T) -> bool,
    signature: impl Fn(&T) -> Option<S>,
    source_ambiguity: &BTreeSet<(SeriesKey, String)>,
) {
    for ((series, family), mut instances) in groups {
        instances.sort_by(|(a, ka), (b, kb)| {
            (&ka.key.reporting_vintage, &ka.locator, code(a))
                .cmp(&(&kb.key.reporting_vintage, &kb.locator, code(b)))
                .then_with(|| format!("{a:?}").cmp(&format!("{b:?}")))
        });
        instances.dedup_by(|(a, ka), (b, kb)| ka.locator == kb.locator && a == b);
        let mut counts = BTreeMap::new();
        for (_, total) in &instances {
            *counts
                .entry(total.key.reporting_vintage.release.clone())
                .or_insert(0_usize) += 1;
        }
        let ambiguous_vintages: Vec<_> = counts
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(v, _)| v)
            .chain(
                source_ambiguity
                    .iter()
                    .filter(|(key, _)| key == &series)
                    .map(|(_, release)| release.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let Some((first, total)) = instances.first() else {
            continue;
        };
        if ambiguous_vintages
            .iter()
            .all(|v| v > &total.key.reporting_vintage.release)
        {
            *report
                .first_registered
                .entry((family, code(first)))
                .or_default() += 1;
        }
        let sequence = instances
            .iter()
            .map(|(i, total)| Appearance {
                release: total.key.reporting_vintage.release.clone(),
                at: total.locator.clone(),
                sha256: total.key.reporting_vintage.file_sha256.clone(),
                state: code(i),
            })
            .collect();
        let signatures: Vec<_> = instances.iter().map(|(i, _)| signature(i)).collect();
        let mut transitions = Vec::new();
        for (from, pair) in instances.windows(2).enumerate() {
            let (a, ka) = pair[0];
            let (b, kb) = pair[1];
            let changed = signatures[from]
                .as_ref()
                .zip(signatures[from + 1].as_ref())
                .map(|(a, b)| a != b);
            let a_release = &ka.key.reporting_vintage.release;
            let b_release = &kb.key.reporting_vintage.release;
            let ambiguous = a_release == b_release
                || ambiguous_vintages
                    .iter()
                    .any(|release| a_release <= release && release <= b_release);
            let kind = if ambiguous {
                TransitionKind::Reclassified
            } else if same_state(a, b) && changed == Some(false) {
                TransitionKind::Persisted
            } else if code(a) == "iii" && matches!(code(b), "i" | "ii") && changed == Some(true) {
                TransitionKind::Corrected
            } else if code(a) == "i" && code(b) == "iii" {
                TransitionKind::Broken
            } else {
                TransitionKind::Reclassified
            };
            transitions.push(Transition {
                from,
                to: from + 1,
                kind,
                operands_changed: changed,
            });
        }
        if instances.len() == 1 {
            transitions.push(Transition {
                from: 0,
                to: 0,
                kind: TransitionKind::SingleVintage,
                operands_changed: None,
            });
        }
        report.histories.push(IdentityHistory {
            series,
            family,
            sequence,
            transitions,
            ambiguous_vintages,
        });
    }
}

// DESIGN: role and semantic key preserve which input changed, including repeated labels and regional members.
// Comparing only a bag of numbers would miss a swapped pair of operands; comparing coordinates would turn a
// moved column into a restatement. LabelCheck operands carry context, not the displayed arithmetic inputs.
fn operand_signature(
    index: &AiscClaimIndex,
    instance: &IdentityInstance,
) -> Option<Vec<(&'static str, SeriesKey, String)>> {
    let mut out = Vec::new();
    for operand in instance
        .operands
        .iter()
        .filter(|o| o.role != OperandRole::LabelCheck)
    {
        let cell = index.cells.get(&format!(
            "{}:{}!{}",
            operand.file, operand.sheet, operand.cell
        ))?;
        let value = match &operand.value {
            OperandValue::Int(n) => format!("number:{n}"),
            OperandValue::Token(t) => format!("token:{t}"),
            OperandValue::Blank => "blank".to_string(),
            OperandValue::Unrounded(t) => format!("unrounded:{t}"),
        };
        out.push((operand.role.as_str(), cell.key.series_key(), value));
    }
    out.sort();
    Some(out)
}

/// Equal-tailed 95% Clopper–Pearson interval for `successes` of `trials` (descriptive, no pass threshold).
/// Returns no interval for an empty/invalid sample or beyond the explicit 100,000-trial computation budget.
/// DESIGN: invert binomial tails by fixed bisection, using the existing portable transcendental functions.
#[must_use]
pub fn clopper_pearson_95(successes: usize, trials: usize) -> Option<(f64, f64)> {
    if trials == 0 || successes > trials || trials > 100_000 {
        return None;
    }
    let invert = |k, tail| {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..60 {
            let mid = (lo + hi) / 2.0;
            if binomial_cdf(k, trials, mid) > tail {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    };
    Some((
        if successes == 0 {
            0.0
        } else {
            invert(successes - 1, 0.975)
        },
        if successes == trials {
            1.0
        } else {
            invert(successes, 0.025)
        },
    ))
}

fn binomial_cdf(k: usize, n: usize, p: f64) -> f64 {
    use fmine_core::det::{exp, ln};
    if p == 0.0 || k == n {
        return 1.0;
    }
    if p == 1.0 {
        return 0.0;
    }
    // Sum the tail whose boundary lies outside the mode, so the recurrence only decreases and cannot overflow.
    let lower = (k as f64) < (n as f64 + 1.0) * p;
    let j = if lower { k } else { k + 1 };
    let log_choose: f64 = (1..=j.min(n - j))
        .map(|i| ln((n - i + 1) as f64) - ln(i as f64))
        .sum();
    let mut term = exp(log_choose + j as f64 * ln(p) + (n - j) as f64 * ln(1.0 - p));
    let mut sum = term;
    if lower {
        for i in (1..=j).rev() {
            term *= i as f64 / (n - i + 1) as f64 * (1.0 - p) / p;
            sum += term;
        }
        sum
    } else {
        for i in j..n {
            term *= (n - i) as f64 / (i + 1) as f64 * p / (1.0 - p);
            sum += term;
        }
        1.0 - sum
    }
}
