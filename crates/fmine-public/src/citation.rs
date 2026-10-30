//! Cited numbers (bd-sjrh.1, BD-329.S1, part 3): each instance record carries the cells its state was computed
//! from and the numbers derived from them.
//!
//! - **Extracted:** `{"v","at","sha256"}`, where `at` is `file:sheet!cell` and `sha256` is that file's digest. A
//!   cell that is not a whole number gives `token`, `blank` or `unrounded` in place of `v`.
//! - **Derived:** `{"v","rule","rules_version","in"}`. `rule` is one of [`RULES`] at [`RULES_VERSION`], and `in`
//!   lists the locators of the cells it was computed from.
//!
//! The payload is not in the state root; nothing here moves it.

use crate::aisc::{Calc, IdentityInstance, Operand, OperandRole, OperandValue, implied_rate_text};
use crate::js;

/// The version of the derivation rules below. A change to any rule's arithmetic bumps it.
pub const RULES_VERSION: &str = "m0-ledger/1";

/// Sum of a footing's summand lines (a placeholder counts zero).
pub const COMPONENT_SUM: &str = "aisc.footing.component_sum";
/// A footing's printed total minus its component sum.
pub const RESIDUAL: &str = "aisc.footing.residual";
/// A bound of 1000·N/Q over N ± k/2 and Q ± 1/2, at three places, rounded outward.
pub const RATE_INTERVAL: &str = "aisc.rate_interval";
/// How many of the listed cells hold a number.
pub const COUNT: &str = "ledger.count";

/// The registered derivation rules.
pub const RULES: [&str; 4] = [COMPONENT_SUM, RESIDUAL, RATE_INTERVAL, COUNT];

/// One cited number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cited {
    /// A value read from a cell.
    Extracted {
        v: OperandValue,
        at: String,
        sha256: String,
    },
    /// A value computed by a registered rule. `v` is JSON number text.
    Derived {
        v: String,
        rule: &'static str,
        inputs: Vec<String>,
    },
}

impl Cited {
    #[must_use]
    pub fn to_json(&self) -> String {
        match self {
            Self::Extracted { v, at, sha256 } => {
                format!(
                    "{{{},\"at\":{},\"sha256\":{}}}",
                    value_json(v),
                    js(at),
                    js(sha256)
                )
            }
            Self::Derived { v, rule, inputs } => {
                let inputs: Vec<String> = inputs.iter().map(|c| js(c)).collect();
                format!(
                    "{{\"v\":{v},\"rule\":{},\"rules_version\":{},\"in\":[{}]}}",
                    js(rule),
                    js(RULES_VERSION),
                    inputs.join(",")
                )
            }
        }
    }
}

/// `file:sheet!cell`.
#[must_use]
pub fn locator(o: &Operand) -> String {
    format!("{}:{}!{}", o.file, o.sheet, o.cell)
}

/// An operand as an extracted citation. `sha_of` gives a loaded file's sha256.
#[must_use]
pub fn extracted(o: &Operand, sha_of: &dyn Fn(&str) -> String) -> Cited {
    Cited::Extracted {
        v: o.value.clone(),
        at: locator(o),
        sha256: sha_of(&o.file),
    }
}

/// The numbers derived for an instance, by name: `component_sum` and `residual` for a footing; `k`, `range_lo`
/// and `range_hi` for a rate. None when the state was decided before any arithmetic.
#[must_use]
pub fn derived(i: &IdentityInstance) -> Vec<(&'static str, Cited)> {
    let cells = |roles: &[OperandRole]| -> Vec<String> {
        i.operands
            .iter()
            .filter(|o| roles.contains(&o.role))
            .map(locator)
            .collect()
    };
    let d = |v: String, rule: &'static str, inputs: Vec<String>| Cited::Derived { v, rule, inputs };
    match i.calc {
        Calc::Footing {
            printed,
            component_sum,
            ..
        } => vec![
            (
                "component_sum",
                d(
                    component_sum.to_string(),
                    COMPONENT_SUM,
                    cells(&[OperandRole::Line]),
                ),
            ),
            (
                "residual",
                d(
                    (printed - component_sum).to_string(),
                    RESIDUAL,
                    cells(&[OperandRole::Total, OperandRole::Line]),
                ),
            ),
        ],
        Calc::Rate(m) => {
            // k counts the numerator's lines: block 1 (and block 2 for AISC), or the cost-of-sales row itself.
            let counted = cells(&[OperandRole::Counted]);
            let k_in = if counted.is_empty() {
                cells(&[OperandRole::Numerator])
            } else {
                counted
            };
            let mut range_in = cells(&[OperandRole::Numerator, OperandRole::Ounces]);
            range_in.extend(
                k_in.iter()
                    .filter(|c| !range_in.contains(c))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            let range = implied_rate_text(m.n, m.k, m.q);
            let (lo, hi) = range
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split_once(", ")
                .unwrap_or_default();
            vec![
                ("k", d(m.k.to_string(), COUNT, k_in)),
                (
                    "range_lo",
                    d(lo.to_string(), RATE_INTERVAL, range_in.clone()),
                ),
                ("range_hi", d(hi.to_string(), RATE_INTERVAL, range_in)),
            ]
        }
        Calc::None => Vec::new(),
    }
}

/// The `cited` object of one instance: `{"operands":[{"role",…extracted}],"derived":[{"name",…derived}]}`.
#[must_use]
pub fn cited_json(i: &IdentityInstance, sha_of: &dyn Fn(&str) -> String) -> String {
    let operands: Vec<String> = i
        .operands
        .iter()
        .map(|o| with_key("role", o.role.as_str(), &extracted(o, sha_of).to_json()))
        .collect();
    let derived: Vec<String> = derived(i)
        .iter()
        .map(|(name, c)| with_key("name", name, &c.to_json()))
        .collect();
    format!(
        "{{\"operands\":[{}],\"derived\":[{}]}}",
        operands.join(","),
        derived.join(",")
    )
}

/// `{"key":"value",…}` from an object's JSON `{…}`.
fn with_key(key: &str, value: &str, object: &str) -> String {
    format!(
        "{{{}:{},{}",
        js(key),
        js(value),
        object.trim_start_matches('{')
    )
}

fn value_json(v: &OperandValue) -> String {
    match v {
        OperandValue::Int(x) => format!("\"v\":{x}"),
        OperandValue::Token(t) => format!("\"token\":{}", js(t)),
        OperandValue::Blank => "\"blank\":true".to_string(),
        OperandValue::Unrounded(t) => format!("\"unrounded\":{}", js(t)),
    }
}
