//! Build-time / dev-only symbolic golden formula derivations from pinned `frankensympy@9d785d0`
//! (`BD-139`, plan §27.3 & §39, `OPEN-SUITE-3`, `GATE-ORACLE-01`).
//! Determinism: BitwisePortable
//!
//! This crate lives strictly in layer `T` (Testpit) and is excluded from every production closure
//! by `xtask check-oracle` (`GATE-ORACLE-01`). Symbolic formulas, exact partial derivatives,
//! LaTeX strings, Rust codegen expressions, and exact rational test evaluations are captured
//! out-of-band by `frankensympy@9d785d0` (`fsym-core`, `fsym-calculus`, `fsym-simplify`,
//! `fsym-solvers`, `fsym-printing`) on `nightly-2026-08-31` into
//! `fixtures/fmine-derive/golden_formulas.txt` and cross-checked here against fail-closed
//! `BitwisePortable` evaluators and independent central finite differences.

use std::collections::BTreeSet;

use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};

/// Exact pinned `frankensympy` Git revision recorded in `franken-stack.lock` (`FD-FSYM-00`).
pub const FSYM_PIN_REV: &str = "9d785d0ef485e02ffce670bf77b4bb280b157c3f";

/// Exact SHA-256 digest of `fixtures/fmine-derive/golden_formulas.txt` captured from
/// `frankensympy@9d785d0` on `nightly-2026-08-31` (`GATE-ORACLE-01`).
pub const GOLDEN_FIXTURE_SHA256: &str =
    "56f60d5f268350305d5806bd03a3653860c201ffdfd6a3ff8d036d57672732a4";

/// One checked-in symbolic golden formula derived by `frankensympy@9d785d0`.
/// The companion `rust_code` column in `fixtures/fmine-derive/golden_formulas.txt` is verified
/// cryptographically through [`GOLDEN_FIXTURE_SHA256`] and exposed on [`GoldenFormulaRow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldenFormulaSpec {
    pub id: &'static str,
    pub domain: &'static str,
    pub vars: &'static str,
    pub fsym_canonical: &'static str,
    pub latex: &'static str,
    pub exact_eval: &'static str,
}

/// One parsed row from `fixtures/fmine-derive/golden_formulas.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct GoldenFormulaRow {
    pub id: String,
    pub domain: String,
    pub vars: String,
    pub fsym_canonical: String,
    pub latex: String,
    pub rust_code: String,
    pub exact_eval: String,
    pub f64_eval: f64,
}

/// Complete 26-formula symbolic catalog captured from `frankensympy@9d785d0` (`BD-139`).
pub const GOLDEN_CATALOG: &[GoldenFormulaSpec] = &[
    GoldenFormulaSpec {
        id: "lane_mine_cutoff",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h",
        fsym_canonical: "c_h*(R*(P - 1*r))**(-1)",
        latex: r"c\_h \frac{1}{\left(R \left(P - 1 r\right)\right)}",
        exact_eval: "1/5",
    },
    GoldenFormulaSpec {
        id: "lane_mine_cutoff_d_ch",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h",
        fsym_canonical: "(R*(P - 1*r))**(-1)",
        latex: r"\frac{1}{\left(R \left(P - 1 r\right)\right)}",
        exact_eval: "2/171",
    },
    GoldenFormulaSpec {
        id: "lane_mine_cutoff_d_P",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h",
        fsym_canonical: "-1*R*c_h*(R*(P - 1*r))**(-2)",
        latex: r"-1 R c\_h \frac{1}{\left(R \left(P - 1 r\right)\right)^{2}}",
        exact_eval: "-1/475",
    },
    GoldenFormulaSpec {
        id: "lane_mine_cutoff_d_R",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h",
        fsym_canonical: "-1*c_h*(R*(P - 1*r))**(-2)*(P - 1*r)",
        latex: r"-1 c\_h \frac{1}{\left(R \left(P - 1 r\right)\right)^{2}} \left(P - 1 r\right)",
        exact_eval: "-2/9",
    },
    GoldenFormulaSpec {
        id: "lane_mill_cutoff",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h,f,F,H",
        fsym_canonical: "-1*(R*(P - 1*r))**(-1)*(-1*c_h - 1*H**(-1)*(F + f))",
        latex: r"-1 \frac{1}{\left(R \left(P - 1 r\right)\right)} \left(-1 c\_h - 1 \frac{1}{H} \left(F + f\right)\right)",
        exact_eval: "4/15",
    },
    GoldenFormulaSpec {
        id: "lane_mill_cutoff_d_F",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h,f,F,H",
        fsym_canonical: "(H*R*(P - 1*r))**(-1)",
        latex: r"\frac{1}{\left(H R \left(P - 1 r\right)\right)}",
        exact_eval: "1/855",
    },
    GoldenFormulaSpec {
        id: "lane_mill_cutoff_d_H",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h,f,F,H",
        fsym_canonical: "-1*(R*H**2*(P - 1*r))**(-1)*(F + f)",
        latex: r"-1 \frac{1}{\left(R H^{2} \left(P - 1 r\right)\right)} \left(F + f\right)",
        exact_eval: "-1/150",
    },
    GoldenFormulaSpec {
        id: "lane_refinery_cutoff",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h,f,F,K",
        fsym_canonical: "c_h*(R*(P - 1*r - 1*K**(-1)*(F + f)))**(-1)",
        latex: r"c\_h \frac{1}{\left(R \left(P - 1 r - 1 \frac{1}{K} \left(F + f\right)\right)\right)}",
        exact_eval: "5/23",
    },
    GoldenFormulaSpec {
        id: "lane_refinery_cutoff_d_K",
        domain: "fm-cutoff",
        vars: "P,r,R,c_h,f,F,K",
        fsym_canonical: "-1*R*c_h*(K**2*(R*(P - 1*r - 1*K**(-1)*(F + f)))**2)**(-1)*(F + f)",
        latex: r"-1 R c\_h \frac{1}{\left(K^{2} \left(R \left(P - 1 r - 1 \frac{1}{K} \left(F + f\right)\right)\right)^{2}\right)} \left(F + f\right)",
        exact_eval: "-4/1587",
    },
    GoldenFormulaSpec {
        id: "two_product_recovery",
        domain: "fm-recon",
        vars: "f,c,t",
        fsym_canonical: "c*(f*(c - 1*t))**(-1)*(f - 1*t)",
        latex: r"c \frac{1}{\left(f \left(c - 1 t\right)\right)} \left(f - 1 t\right)",
        exact_eval: "10/11",
    },
    GoldenFormulaSpec {
        id: "two_product_recovery_d_f",
        domain: "fm-recon",
        vars: "f,c,t",
        fsym_canonical: "(c*(f*(c - 1*t))**(-1) - 1*c*(f*(c - 1*t))**(-2)*(c - 1*t)*(f - 1*t))",
        latex: r"c \frac{1}{\left(f \left(c - 1 t\right)\right)} - 1 c \frac{1}{\left(f \left(c - 1 t\right)\right)^{2}} \left(c - 1 t\right) \left(f - 1 t\right)",
        exact_eval: "5/99",
    },
    GoldenFormulaSpec {
        id: "two_product_recovery_d_c",
        domain: "fm-recon",
        vars: "f,c,t",
        fsym_canonical: "((f*(c - 1*t))**(-1)*(f - 1*t) - 1*c*f*(f*(c - 1*t))**(-2)*(f - 1*t))",
        latex: r"\frac{1}{\left(f \left(c - 1 t\right)\right)} \left(f - 1 t\right) - 1 c f \frac{1}{\left(f \left(c - 1 t\right)\right)^{2}} \left(f - 1 t\right)",
        exact_eval: "-1/2178",
    },
    GoldenFormulaSpec {
        id: "two_product_recovery_d_t",
        domain: "fm-recon",
        vars: "f,c,t",
        fsym_canonical: "(c*f*(f*(c - 1*t))**(-2)*(f - 1*t) - 1*c*(f*(c - 1*t))**(-1))",
        latex: r"c f \frac{1}{\left(f \left(c - 1 t\right)\right)^{2}} \left(f - 1 t\right) - 1 c \frac{1}{\left(f \left(c - 1 t\right)\right)}",
        exact_eval: "-500/1089",
    },
    GoldenFormulaSpec {
        id: "nsr_ad_valorem",
        domain: "fm-fiscal",
        vars: "g,R,P,tau,c_t,c_m",
        fsym_canonical: "(R*g*(1 - 1*tau)*(P - 1*c_t) - 1*c_m)",
        latex: r"R g \left(1 - 1 tau\right) \left(P - 1 c\_t\right) - 1 c\_m",
        exact_eval: "7497/50",
    },
    GoldenFormulaSpec {
        id: "nsr_ad_valorem_d_g",
        domain: "fm-fiscal",
        vars: "g,R,P,tau,c_t,c_m",
        fsym_canonical: "R*(1 - 1*tau)*(P - 1*c_t)",
        latex: r"R \left(1 - 1 tau\right) \left(P - 1 c\_t\right)",
        exact_eval: "3249/50",
    },
    GoldenFormulaSpec {
        id: "nsr_ad_valorem_d_tau",
        domain: "fm-fiscal",
        vars: "g,R,P,tau,c_t,c_m",
        fsym_canonical: "-1*R*g*(P - 1*c_t)",
        latex: r"-1 R g \left(P - 1 c\_t\right)",
        exact_eval: "-1026/5",
    },
    GoldenFormulaSpec {
        id: "nsr_breakeven_grade",
        domain: "fm-fiscal",
        vars: "R,P,tau,c_t,c_m",
        fsym_canonical: "c_m*(R*(1 - 1*tau)*(P - 1*c_t))**(-1)",
        latex: r"c\_m \frac{1}{\left(R \left(1 - 1 tau\right) \left(P - 1 c\_t\right)\right)}",
        exact_eval: "250/361",
    },
    GoldenFormulaSpec {
        id: "wgc_aisc_unit_cost",
        domain: "fm-metrics",
        vars: "C,T,g,R",
        fsym_canonical: "C*(R*T*g)**(-1)",
        latex: r"C \frac{1}{\left(R T g\right)}",
        exact_eval: "50",
    },
    GoldenFormulaSpec {
        id: "wgc_aisc_d_C",
        domain: "fm-metrics",
        vars: "C,T,g,R",
        fsym_canonical: "(R*T*g)**(-1)",
        latex: r"\frac{1}{\left(R T g\right)}",
        exact_eval: "1/16",
    },
    GoldenFormulaSpec {
        id: "wgc_aisc_d_T",
        domain: "fm-metrics",
        vars: "C,T,g,R",
        fsym_canonical: "-1*C*R*g*(R*T*g)**(-2)",
        latex: r"-1 C R g \frac{1}{\left(R T g\right)^{2}}",
        exact_eval: "-5",
    },
    GoldenFormulaSpec {
        id: "wgc_aisc_d_g",
        domain: "fm-metrics",
        vars: "C,T,g,R",
        fsym_canonical: "-1*C*R*T*(R*T*g)**(-2)",
        latex: r"-1 C R T \frac{1}{\left(R T g\right)^{2}}",
        exact_eval: "-25",
    },
    GoldenFormulaSpec {
        id: "wgc_aisc_d_R",
        domain: "fm-metrics",
        vars: "C,T,g,R",
        fsym_canonical: "-1*C*T*g*(R*T*g)**(-2)",
        latex: r"-1 C T g \frac{1}{\left(R T g\right)^{2}}",
        exact_eval: "-125/2",
    },
    GoldenFormulaSpec {
        id: "bilinear_metal_residual",
        domain: "fm-recon",
        vars: "T,g,M",
        fsym_canonical: "(T*g - 1*M)",
        latex: r"T g - 1 M",
        exact_eval: "25",
    },
    GoldenFormulaSpec {
        id: "bilinear_metal_d_T",
        domain: "fm-recon",
        vars: "T,g,M",
        fsym_canonical: "g",
        latex: "g",
        exact_eval: "5/2",
    },
    GoldenFormulaSpec {
        id: "bilinear_metal_d_g",
        domain: "fm-recon",
        vars: "T,g,M",
        fsym_canonical: "T",
        latex: "T",
        exact_eval: "250",
    },
    GoldenFormulaSpec {
        id: "bilinear_metal_d_M",
        domain: "fm-recon",
        vars: "T,g,M",
        fsym_canonical: "-1",
        latex: "-1",
        exact_eval: "-1",
    },
];

/// Parse an exact signed integer or reduced fraction `"num"` / `"num/den"` (`den > 0`)
/// into its `f64` value, refusing malformed, zero-denominator, or non-finite forms.
pub fn parse_exact_rational(raw: &str) -> Result<f64, Refusal> {
    if raw.is_empty() || raw.trim() != raw {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!("malformed exact_eval rational {raw:?}"),
        ));
    }
    let val = if let Some((num_s, den_s)) = raw.split_once('/') {
        let num: i128 = num_s.parse().map_err(|_| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("invalid exact_eval numerator in {raw:?}"),
            )
        })?;
        let den: i128 = den_s.parse().map_err(|_| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("invalid exact_eval denominator in {raw:?}"),
            )
        })?;
        if den <= 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("exact_eval denominator must be positive in {raw:?}"),
            ));
        }
        (num as f64) / (den as f64)
    } else {
        let num: i128 = raw.parse().map_err(|_| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("invalid exact_eval integer {raw:?}"),
            )
        })?;
        num as f64
    };
    check_finite_output("exact_eval", val)
}

/// Parse `fixtures/fmine-derive/golden_formulas.txt`, refusing malformed, empty, contradictory,
/// or duplicate rows.
pub fn parse_golden_fixture(text: &str) -> Result<Vec<GoldenFormulaRow>, Refusal> {
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 9 || cols[0] != "FORMULA" {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "golden_formulas.txt line {}: expected 9 tab-separated FORMULA columns, got {}",
                    idx + 1,
                    cols.len()
                ),
            ));
        }
        if cols[1..9].iter().any(|c| c.trim().is_empty()) {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("golden_formulas.txt line {}: empty field", idx + 1),
            ));
        }
        let exact_f64 = parse_exact_rational(cols[7])?;
        let f64_eval: f64 = cols[8].parse().map_err(|_| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "golden_formulas.txt line {}: invalid f64_eval {:?}",
                    idx + 1,
                    cols[8]
                ),
            )
        })?;
        if !f64_eval.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("golden_formulas.txt line {}: non-finite f64_eval", idx + 1),
            ));
        }
        let tol = 1e-12 * exact_f64.abs().max(1.0);
        if (exact_f64 - f64_eval).abs() > tol {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "golden_formulas.txt line {}: exact_eval {:?} ({exact_f64}) disagrees with f64_eval {f64_eval}",
                    idx + 1,
                    cols[7]
                ),
            ));
        }
        if !seen.insert(cols[1].to_string()) {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "golden_formulas.txt line {}: duplicate formula id {:?}",
                    idx + 1,
                    cols[1]
                ),
            ));
        }
        rows.push(GoldenFormulaRow {
            id: cols[1].to_string(),
            domain: cols[2].to_string(),
            vars: cols[3].to_string(),
            fsym_canonical: cols[4].to_string(),
            latex: cols[5].to_string(),
            rust_code: cols[6].to_string(),
            exact_eval: cols[7].to_string(),
            f64_eval: canonical_zero(f64_eval),
        });
    }
    if rows.is_empty() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "golden_formulas.txt contains no FORMULA rows",
        ));
    }
    Ok(rows)
}

/// Verify that a parsed golden fixture matches both [`GOLDEN_FIXTURE_SHA256`] and
/// [`GOLDEN_CATALOG`] byte-for-byte on every formula.
pub fn verify_fixture_against_catalog(
    fixture_text: &str,
) -> Result<Vec<GoldenFormulaRow>, Refusal> {
    let actual_digest = sha256(fixture_text.as_bytes()).to_hex();
    if actual_digest != GOLDEN_FIXTURE_SHA256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "golden fixture sha256 mismatch: expected {GOLDEN_FIXTURE_SHA256}, got {actual_digest}"
            ),
        ));
    }
    let rows = parse_golden_fixture(fixture_text)?;
    if rows.len() != GOLDEN_CATALOG.len() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "golden fixture row count mismatch: expected {}, got {}",
                GOLDEN_CATALOG.len(),
                rows.len()
            ),
        ));
    }
    for (row, spec) in rows.iter().zip(GOLDEN_CATALOG.iter()) {
        if row.id != spec.id
            || row.domain != spec.domain
            || row.vars != spec.vars
            || row.fsym_canonical != spec.fsym_canonical
            || row.latex != spec.latex
            || row.exact_eval != spec.exact_eval
        {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "golden fixture drift at formula {:?} (expected {:?})",
                    row.id, spec.id
                ),
            ));
        }
    }
    Ok(rows)
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn require_finite(name: &str, value: f64) -> Result<f64, Refusal> {
    if !value.is_finite() {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("non-finite input {name}={value}"),
        ));
    }
    Ok(canonical_zero(value))
}

fn require_positive(name: &str, value: f64) -> Result<f64, Refusal> {
    let v = require_finite(name, value)?;
    if v <= 0.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("input {name}={v} must be strictly positive"),
        ));
    }
    Ok(v)
}

fn require_non_negative(name: &str, value: f64) -> Result<f64, Refusal> {
    let v = require_finite(name, value)?;
    if v < 0.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("input {name}={v} must be non-negative"),
        ));
    }
    Ok(v)
}

fn check_finite_output(name: &str, value: f64) -> Result<f64, Refusal> {
    if !value.is_finite() {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("non-finite golden evaluation for {name}"),
        ));
    }
    Ok(canonical_zero(value))
}

/// Inputs for Lane's three limiting cut-off grades (`fm-cutoff`, `BD-252`, plan §15.6 / §27.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneInputs {
    /// Metal selling price `P` (USD per metal unit).
    pub price: f64,
    /// Selling/refining cost `r` (USD per metal unit, `P > r`).
    pub refining_cost: f64,
    /// Metallurgical recovery fraction `R` in `(0, 1]`.
    pub recovery: f64,
    /// Processing / milling variable cost `c_h` (USD per tonne milled, `>= 0`).
    pub milling_cost: f64,
    /// Fixed period cost `f` (USD per period, `>= 0`).
    pub fixed_cost: f64,
    /// Opportunity cost of capital `F = d * V` (USD per period, `>= 0`).
    pub opportunity_cost: f64,
    /// Mill throughput capacity `H` (tonnes per period, `> 0`).
    pub mill_capacity: f64,
    /// Refinery / market metal capacity `K` (metal units per period, `> 0`).
    pub refinery_capacity: f64,
}

/// Lane's three limiting cut-offs (`g_m`, `g_h`, `g_r`) and their `fsym`-derived partial derivatives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneCutoffsWithJacobian {
    pub g_mine: f64,
    pub dg_mine_d_ch: f64,
    pub dg_mine_d_price: f64,
    pub dg_mine_d_recovery: f64,
    pub g_mill: f64,
    pub dg_mill_d_opportunity: f64,
    pub dg_mill_d_mill_capacity: f64,
    pub g_refinery: f64,
    pub dg_refinery_d_refinery_capacity: f64,
}

/// Evaluate Lane's three limiting cut-offs and exact `fsym@9d785d0` partial derivatives.
pub fn eval_lane_cutoffs(inp: LaneInputs) -> Result<LaneCutoffsWithJacobian, Refusal> {
    let p = require_finite("price", inp.price)?;
    let r = require_non_negative("refining_cost", inp.refining_cost)?;
    let rec = require_positive("recovery", inp.recovery)?;
    if rec > 1.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("recovery={rec} must lie in (0, 1]"),
        ));
    }
    let ch = require_non_negative("milling_cost", inp.milling_cost)?;
    let f_fix = require_non_negative("fixed_cost", inp.fixed_cost)?;
    let cap_f = require_non_negative("opportunity_cost", inp.opportunity_cost)?;
    let h = require_positive("mill_capacity", inp.mill_capacity)?;
    let k = require_positive("refinery_capacity", inp.refinery_capacity)?;

    let net_price = p - r;
    if !(net_price.is_finite() && net_price > 0.0) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("net metal margin P - r = {net_price} must be strictly positive"),
        ));
    }
    let denom_mh = rec * net_price;
    if !(denom_mh.is_finite() && denom_mh > 0.0) {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "non-positive or non-finite Lane denominator R*(P - r)",
        ));
    }
    let overhead = check_finite_output("lane_overhead", f_fix + cap_f)?;
    let ref_overhead = check_finite_output("lane_refinery_overhead", overhead / k)?;
    let mill_overhead = check_finite_output("lane_mill_overhead", overhead / h)?;
    let net_ref_price = net_price - ref_overhead;
    if !(net_ref_price.is_finite() && net_ref_price > 0.0) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "refinery net margin P - r - (f + F)/K = {net_ref_price} must be strictly positive"
            ),
        ));
    }
    let denom_r = rec * net_ref_price;
    if !(denom_r.is_finite() && denom_r > 0.0) {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "non-positive or non-finite Lane denominator R*(P - r - (f + F)/K)",
        ));
    }

    // Algebraically equivalent simplified forms of fsym@9d785d0 derivatives that avoid
    // premature denominator-squaring overflow/underflow at extreme scales.
    let g_mine = check_finite_output("lane_mine_cutoff", ch / denom_mh)?;
    let dg_mine_d_ch = check_finite_output("lane_mine_cutoff_d_ch", 1.0 / denom_mh)?;
    let dg_mine_d_price = check_finite_output("lane_mine_cutoff_d_P", -g_mine / net_price)?;
    let dg_mine_d_recovery = check_finite_output("lane_mine_cutoff_d_R", -g_mine / rec)?;

    let g_mill = check_finite_output("lane_mill_cutoff", (ch + mill_overhead) / denom_mh)?;
    let dg_mill_d_opportunity = check_finite_output("lane_mill_cutoff_d_F", dg_mine_d_ch / h)?;
    let dg_mill_d_mill_capacity =
        check_finite_output("lane_mill_cutoff_d_H", -((mill_overhead / denom_mh) / h))?;

    let g_refinery = check_finite_output("lane_refinery_cutoff", ch / denom_r)?;
    let dg_refinery_d_refinery_capacity = check_finite_output(
        "lane_refinery_cutoff_d_K",
        -((g_refinery / k) * (ref_overhead / net_ref_price)),
    )?;

    Ok(LaneCutoffsWithJacobian {
        g_mine,
        dg_mine_d_ch,
        dg_mine_d_price,
        dg_mine_d_recovery,
        g_mill,
        dg_mill_d_opportunity,
        dg_mill_d_mill_capacity,
        g_refinery,
        dg_refinery_d_refinery_capacity,
    })
}

/// Inputs for the AMIRA P754 two-product metallurgical recovery identity (`fm-recon`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoProductInputs {
    pub feed_grade: f64,
    pub concentrate_grade: f64,
    pub tailings_grade: f64,
}

/// Two-product metallurgical recovery `R = c(f - t) / (f(c - t))` and its exact Jacobian.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoProductWithJacobian {
    pub recovery: f64,
    pub dr_df: f64,
    pub dr_dc: f64,
    pub dr_dt: f64,
}

/// Evaluate two-product metallurgical recovery and exact `fsym@9d785d0` partial derivatives.
pub fn eval_two_product_recovery(inp: TwoProductInputs) -> Result<TwoProductWithJacobian, Refusal> {
    let f = require_positive("feed_grade", inp.feed_grade)?;
    let c = require_positive("concentrate_grade", inp.concentrate_grade)?;
    let t = require_non_negative("tailings_grade", inp.tailings_grade)?;
    let c_minus_t = c - t;
    if !(c_minus_t.is_finite() && c_minus_t != 0.0) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "two-product recovery denominator (c - t) must be nonzero",
        ));
    }
    let f_minus_t = f - t;
    let c_ratio = check_finite_output("two_product_c_ratio", c / c_minus_t)?;
    // Algebraically simplified single-fraction forms of fsym@9d785d0 derivatives:
    //   dR/df = c*t / (f^2*(c - t))
    //   dR/dc = -t*(f - t) / (f*(c - t)^2)
    //   dR/dt = c*(f - c) / (f*(c - t)^2)
    // Eliminates catastrophic cancellation when t << f and avoids denom^2 overflow.
    let recovery = check_finite_output("two_product_recovery", c_ratio * (f_minus_t / f))?;
    let dr_df = check_finite_output("two_product_recovery_d_f", (c_ratio * (t / f)) / f)?;
    let dr_dc = check_finite_output(
        "two_product_recovery_d_c",
        -((t / c_minus_t) * (f_minus_t / f)) / c_minus_t,
    )?;
    let dr_dt = check_finite_output(
        "two_product_recovery_d_t",
        (c_ratio * ((f - c) / c_minus_t)) / f,
    )?;
    Ok(TwoProductWithJacobian {
        recovery,
        dr_df,
        dr_dc,
        dr_dt,
    })
}

/// Inputs for ad-valorem Net Smelter Return (`fm-fiscal`, plan §27.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NsrInputs {
    pub grade: f64,
    pub recovery: f64,
    pub price: f64,
    pub royalty_rate: f64,
    pub treatment_cost: f64,
    pub mining_and_processing_cost: f64,
}

/// Ad-valorem NSR, partial derivatives, and exact break-even grade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NsrWithSensitivities {
    pub nsr: f64,
    pub dnsr_dg: f64,
    pub dnsr_dtau: f64,
    pub breakeven_grade: f64,
}

/// Evaluate ad-valorem NSR, sensitivities, and break-even grade from `fsym@9d785d0`.
pub fn eval_nsr(inp: NsrInputs) -> Result<NsrWithSensitivities, Refusal> {
    let g = require_non_negative("grade", inp.grade)?;
    let r = require_positive("recovery", inp.recovery)?;
    if r > 1.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("recovery={r} must lie in (0, 1]"),
        ));
    }
    let p = require_finite("price", inp.price)?;
    let tau = require_non_negative("royalty_rate", inp.royalty_rate)?;
    if tau >= 1.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("royalty_rate={tau} must lie in [0, 1)"),
        ));
    }
    let ct = require_non_negative("treatment_cost", inp.treatment_cost)?;
    let cm = require_non_negative("mining_and_processing_cost", inp.mining_and_processing_cost)?;
    let net_p = p - ct;
    if !(net_p.is_finite() && net_p > 0.0) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("net price P - c_t = {net_p} must be strictly positive"),
        ));
    }
    let unit_rev = r * (1.0 - tau) * net_p;
    if !(unit_rev.is_finite() && unit_rev > 0.0) {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "non-positive or non-finite NSR unit revenue R*(1 - tau)*(P - c_t)",
        ));
    }
    let nsr = check_finite_output("nsr_ad_valorem", g * unit_rev - cm)?;
    let dnsr_dg = check_finite_output("nsr_ad_valorem_d_g", unit_rev)?;
    let dnsr_dtau = check_finite_output("nsr_ad_valorem_d_tau", -(r * g * net_p))?;
    let breakeven_grade = check_finite_output("nsr_breakeven_grade", cm / unit_rev)?;
    Ok(NsrWithSensitivities {
        nsr,
        dnsr_dg,
        dnsr_dtau,
        breakeven_grade,
    })
}

/// Inputs for WGC AISC unit cost (`fm-metrics`, `GB-26`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WgcAiscInputs {
    pub total_cost: f64,
    pub tonnes: f64,
    pub grade: f64,
    pub recovery: f64,
}

/// WGC AISC unit cost `C / (T * g * R)` and its exact `fsym@9d785d0` Jacobian.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WgcAiscWithJacobian {
    pub aisc: f64,
    pub daisc_dc: f64,
    pub daisc_dt: f64,
    pub daisc_dg: f64,
    pub daisc_dr: f64,
}

/// Evaluate WGC AISC unit cost and exact `fsym@9d785d0` partial derivatives.
pub fn eval_wgc_aisc(inp: WgcAiscInputs) -> Result<WgcAiscWithJacobian, Refusal> {
    let c = require_non_negative("total_cost", inp.total_cost)?;
    let t = require_positive("tonnes", inp.tonnes)?;
    let g = require_positive("grade", inp.grade)?;
    let r = require_positive("recovery", inp.recovery)?;
    if r > 1.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("recovery={r} must lie in (0, 1]"),
        ));
    }
    let metal = t * g * r;
    if !(metal.is_finite() && metal > 0.0) {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "non-positive or non-finite recovered metal T*g*R",
        ));
    }
    // Algebraically equivalent simplified forms of fsym@9d785d0 derivatives (-AISC / x) that
    // prevent metal^2 overflow and preserve exact -1 elasticities across all scales.
    let aisc = check_finite_output("wgc_aisc_unit_cost", c / metal)?;
    let daisc_dc = check_finite_output("wgc_aisc_d_C", 1.0 / metal)?;
    let daisc_dt = check_finite_output("wgc_aisc_d_T", -aisc / t)?;
    let daisc_dg = check_finite_output("wgc_aisc_d_g", -aisc / g)?;
    let daisc_dr = check_finite_output("wgc_aisc_d_R", -aisc / r)?;
    Ok(WgcAiscWithJacobian {
        aisc,
        daisc_dc,
        daisc_dt,
        daisc_dg,
        daisc_dr,
    })
}

/// Inputs for the bilinear contained-metal residual `r(T, g, M) = T*g - M` (`fm-recon`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BilinearMetalInputs {
    pub dry_tonnes: f64,
    pub grade: f64,
    pub contained_metal: f64,
}

/// Bilinear contained-metal residual and its exact `fsym@9d785d0` Jacobian.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BilinearMetalWithJacobian {
    pub residual: f64,
    pub dr_dt: f64,
    pub dr_dg: f64,
    pub dr_dm: f64,
}

/// Evaluate bilinear contained-metal residual and exact `fsym@9d785d0` partial derivatives.
pub fn eval_bilinear_metal(inp: BilinearMetalInputs) -> Result<BilinearMetalWithJacobian, Refusal> {
    let t = require_non_negative("dry_tonnes", inp.dry_tonnes)?;
    let g = require_non_negative("grade", inp.grade)?;
    let m = require_non_negative("contained_metal", inp.contained_metal)?;
    let residual = check_finite_output("bilinear_metal_residual", t * g - m)?;
    Ok(BilinearMetalWithJacobian {
        residual,
        dr_dt: g,
        dr_dg: t,
        dr_dm: -1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const GOLDEN_FIXTURE_TEXT: &str =
        include_str!("../../../fixtures/fmine-derive/golden_formulas.txt");

    #[test]
    fn test_fixture_matches_catalog_and_detects_tampering() {
        let rows = verify_fixture_against_catalog(GOLDEN_FIXTURE_TEXT)
            .expect("pinned fsym fixture must match GOLDEN_CATALOG");
        assert_eq!(rows.len(), 26);

        // Every exact rational string in the fixture must agree with its printed f64 evaluation.
        for row in &rows {
            let rat_f64 = parse_exact_rational(&row.exact_eval).expect("valid exact rational");
            assert!(
                (rat_f64 - row.f64_eval).abs() <= 1e-12 * rat_f64.abs().max(1.0),
                "{}: exact_eval {} ({rat_f64}) != f64_eval {}",
                row.id,
                row.exact_eval,
                row.f64_eval
            );
        }

        // Planted negative: tampering one exact rational value must refuse with FMINE-REF-505.
        let tampered = GOLDEN_FIXTURE_TEXT.replacen("\t1/5\t", "\t1/6\t", 1);
        assert_ne!(tampered, GOLDEN_FIXTURE_TEXT);
        let err = verify_fixture_against_catalog(&tampered)
            .expect_err("tampered golden fixture must be refused");
        assert_eq!(err.code, RefusalCode::DigestMismatch505.as_str());
    }

    #[test]
    fn test_exact_rational_fixture_points_match_evaluators() {
        let rows = verify_fixture_against_catalog(GOLDEN_FIXTURE_TEXT).expect("fixture");
        let by_id: BTreeMap<&str, f64> = rows
            .iter()
            .map(|r| {
                (
                    r.id.as_str(),
                    parse_exact_rational(&r.exact_eval).expect("valid exact rational"),
                )
            })
            .collect();

        let lane = eval_lane_cutoffs(LaneInputs {
            price: 100.0,
            refining_cost: 5.0,
            recovery: 0.9,
            milling_cost: 17.1,
            fixed_cost: 30.0,
            opportunity_cost: 27.0,
            mill_capacity: 10.0,
            refinery_capacity: 7.5,
        })
        .expect("lane");

        let two_p = eval_two_product_recovery(TwoProductInputs {
            feed_grade: 2.0,
            concentrate_grade: 20.0,
            tailings_grade: 0.2,
        })
        .expect("two_product");

        let nsr = eval_nsr(NsrInputs {
            grade: 3.0,
            recovery: 0.9,
            price: 80.0,
            royalty_rate: 0.05,
            treatment_cost: 4.0,
            mining_and_processing_cost: 45.0,
        })
        .expect("nsr");

        let aisc = eval_wgc_aisc(WgcAiscInputs {
            total_cost: 800.0,
            tonnes: 10.0,
            grade: 2.0,
            recovery: 0.8,
        })
        .expect("aisc");

        let bil = eval_bilinear_metal(BilinearMetalInputs {
            dry_tonnes: 250.0,
            grade: 2.5,
            contained_metal: 600.0,
        })
        .expect("bilinear");

        let checks: [(&str, f64); 26] = [
            ("lane_mine_cutoff", lane.g_mine),
            ("lane_mine_cutoff_d_ch", lane.dg_mine_d_ch),
            ("lane_mine_cutoff_d_P", lane.dg_mine_d_price),
            ("lane_mine_cutoff_d_R", lane.dg_mine_d_recovery),
            ("lane_mill_cutoff", lane.g_mill),
            ("lane_mill_cutoff_d_F", lane.dg_mill_d_opportunity),
            ("lane_mill_cutoff_d_H", lane.dg_mill_d_mill_capacity),
            ("lane_refinery_cutoff", lane.g_refinery),
            (
                "lane_refinery_cutoff_d_K",
                lane.dg_refinery_d_refinery_capacity,
            ),
            ("two_product_recovery", two_p.recovery),
            ("two_product_recovery_d_f", two_p.dr_df),
            ("two_product_recovery_d_c", two_p.dr_dc),
            ("two_product_recovery_d_t", two_p.dr_dt),
            ("nsr_ad_valorem", nsr.nsr),
            ("nsr_ad_valorem_d_g", nsr.dnsr_dg),
            ("nsr_ad_valorem_d_tau", nsr.dnsr_dtau),
            ("nsr_breakeven_grade", nsr.breakeven_grade),
            ("wgc_aisc_unit_cost", aisc.aisc),
            ("wgc_aisc_d_C", aisc.daisc_dc),
            ("wgc_aisc_d_T", aisc.daisc_dt),
            ("wgc_aisc_d_g", aisc.daisc_dg),
            ("wgc_aisc_d_R", aisc.daisc_dr),
            ("bilinear_metal_residual", bil.residual),
            ("bilinear_metal_d_T", bil.dr_dt),
            ("bilinear_metal_d_g", bil.dr_dg),
            ("bilinear_metal_d_M", bil.dr_dm),
        ];

        for (id, got) in checks {
            let expected = by_id[id];
            let tol = 1e-13 * expected.abs().max(1.0);
            assert!(
                (got - expected).abs() <= tol,
                "{id}: evaluator got {got:.16}, fsym exact rational {expected:.16}"
            );
        }
    }

    #[test]
    fn test_symbolic_jacobians_match_central_finite_differences() {
        let h = 1e-6;
        let assert_fd = |label: &str, analytic: f64, f_plus: f64, f_minus: f64| {
            let fd = (f_plus - f_minus) / (2.0 * h);
            let tol = 1e-7 * analytic.abs().max(1.0);
            assert!(
                (analytic - fd).abs() <= tol,
                "{label}: analytic {analytic:.12} vs central FD {fd:.12}"
            );
        };

        // 1. Lane cut-off Jacobians at a distinct non-fixture operating point.
        let l0 = LaneInputs {
            price: 2_350.0,
            refining_cost: 45.0,
            recovery: 0.87,
            milling_cost: 28.5,
            fixed_cost: 120.0,
            opportunity_cost: 85.0,
            mill_capacity: 15.0,
            refinery_capacity: 0.40,
        };
        let lr = eval_lane_cutoffs(l0).expect("lane");
        assert_fd(
            "dg_mine/dc_h",
            lr.dg_mine_d_ch,
            eval_lane_cutoffs(LaneInputs {
                milling_cost: l0.milling_cost + h,
                ..l0
            })
            .expect("p")
            .g_mine,
            eval_lane_cutoffs(LaneInputs {
                milling_cost: l0.milling_cost - h,
                ..l0
            })
            .expect("m")
            .g_mine,
        );
        assert_fd(
            "dg_mine/dP",
            lr.dg_mine_d_price,
            eval_lane_cutoffs(LaneInputs {
                price: l0.price + h,
                ..l0
            })
            .expect("p")
            .g_mine,
            eval_lane_cutoffs(LaneInputs {
                price: l0.price - h,
                ..l0
            })
            .expect("m")
            .g_mine,
        );
        assert_fd(
            "dg_mine/dR",
            lr.dg_mine_d_recovery,
            eval_lane_cutoffs(LaneInputs {
                recovery: l0.recovery + h,
                ..l0
            })
            .expect("p")
            .g_mine,
            eval_lane_cutoffs(LaneInputs {
                recovery: l0.recovery - h,
                ..l0
            })
            .expect("m")
            .g_mine,
        );
        assert_fd(
            "dg_mill/dF",
            lr.dg_mill_d_opportunity,
            eval_lane_cutoffs(LaneInputs {
                opportunity_cost: l0.opportunity_cost + h,
                ..l0
            })
            .expect("p")
            .g_mill,
            eval_lane_cutoffs(LaneInputs {
                opportunity_cost: l0.opportunity_cost - h,
                ..l0
            })
            .expect("m")
            .g_mill,
        );
        assert_fd(
            "dg_mill/dH",
            lr.dg_mill_d_mill_capacity,
            eval_lane_cutoffs(LaneInputs {
                mill_capacity: l0.mill_capacity + h,
                ..l0
            })
            .expect("p")
            .g_mill,
            eval_lane_cutoffs(LaneInputs {
                mill_capacity: l0.mill_capacity - h,
                ..l0
            })
            .expect("m")
            .g_mill,
        );
        assert_fd(
            "dg_ref/dK",
            lr.dg_refinery_d_refinery_capacity,
            eval_lane_cutoffs(LaneInputs {
                refinery_capacity: l0.refinery_capacity + h,
                ..l0
            })
            .expect("p")
            .g_refinery,
            eval_lane_cutoffs(LaneInputs {
                refinery_capacity: l0.refinery_capacity - h,
                ..l0
            })
            .expect("m")
            .g_refinery,
        );

        // 2. Two-product recovery Jacobian.
        let tp0 = TwoProductInputs {
            feed_grade: 1.45,
            concentrate_grade: 27.5,
            tailings_grade: 0.14,
        };
        let tpr = eval_two_product_recovery(tp0).expect("tp");
        assert_fd(
            "dR/df",
            tpr.dr_df,
            eval_two_product_recovery(TwoProductInputs {
                feed_grade: tp0.feed_grade + h,
                ..tp0
            })
            .expect("p")
            .recovery,
            eval_two_product_recovery(TwoProductInputs {
                feed_grade: tp0.feed_grade - h,
                ..tp0
            })
            .expect("m")
            .recovery,
        );
        assert_fd(
            "dR/dc",
            tpr.dr_dc,
            eval_two_product_recovery(TwoProductInputs {
                concentrate_grade: tp0.concentrate_grade + h,
                ..tp0
            })
            .expect("p")
            .recovery,
            eval_two_product_recovery(TwoProductInputs {
                concentrate_grade: tp0.concentrate_grade - h,
                ..tp0
            })
            .expect("m")
            .recovery,
        );
        assert_fd(
            "dR/dt",
            tpr.dr_dt,
            eval_two_product_recovery(TwoProductInputs {
                tailings_grade: tp0.tailings_grade + h,
                ..tp0
            })
            .expect("p")
            .recovery,
            eval_two_product_recovery(TwoProductInputs {
                tailings_grade: tp0.tailings_grade - h,
                ..tp0
            })
            .expect("m")
            .recovery,
        );

        // 3. NSR sensitivities.
        let n0 = NsrInputs {
            grade: 2.35,
            recovery: 0.89,
            price: 92.5,
            royalty_rate: 0.06,
            treatment_cost: 6.2,
            mining_and_processing_cost: 52.0,
        };
        let nr = eval_nsr(n0).expect("nsr");
        assert_fd(
            "dNSR/dg",
            nr.dnsr_dg,
            eval_nsr(NsrInputs {
                grade: n0.grade + h,
                ..n0
            })
            .expect("p")
            .nsr,
            eval_nsr(NsrInputs {
                grade: n0.grade - h,
                ..n0
            })
            .expect("m")
            .nsr,
        );
        assert_fd(
            "dNSR/dtau",
            nr.dnsr_dtau,
            eval_nsr(NsrInputs {
                royalty_rate: n0.royalty_rate + h,
                ..n0
            })
            .expect("p")
            .nsr,
            eval_nsr(NsrInputs {
                royalty_rate: n0.royalty_rate - h,
                ..n0
            })
            .expect("m")
            .nsr,
        );

        // 4. WGC AISC Jacobian.
        let a0 = WgcAiscInputs {
            total_cost: 1_250.0,
            tonnes: 14.5,
            grade: 1.75,
            recovery: 0.88,
        };
        let ar = eval_wgc_aisc(a0).expect("aisc");
        assert_fd(
            "dAISC/dC",
            ar.daisc_dc,
            eval_wgc_aisc(WgcAiscInputs {
                total_cost: a0.total_cost + h,
                ..a0
            })
            .expect("p")
            .aisc,
            eval_wgc_aisc(WgcAiscInputs {
                total_cost: a0.total_cost - h,
                ..a0
            })
            .expect("m")
            .aisc,
        );
        assert_fd(
            "dAISC/dT",
            ar.daisc_dt,
            eval_wgc_aisc(WgcAiscInputs {
                tonnes: a0.tonnes + h,
                ..a0
            })
            .expect("p")
            .aisc,
            eval_wgc_aisc(WgcAiscInputs {
                tonnes: a0.tonnes - h,
                ..a0
            })
            .expect("m")
            .aisc,
        );
        assert_fd(
            "dAISC/dg",
            ar.daisc_dg,
            eval_wgc_aisc(WgcAiscInputs {
                grade: a0.grade + h,
                ..a0
            })
            .expect("p")
            .aisc,
            eval_wgc_aisc(WgcAiscInputs {
                grade: a0.grade - h,
                ..a0
            })
            .expect("m")
            .aisc,
        );
        assert_fd(
            "dAISC/dR",
            ar.daisc_dr,
            eval_wgc_aisc(WgcAiscInputs {
                recovery: a0.recovery + h,
                ..a0
            })
            .expect("p")
            .aisc,
            eval_wgc_aisc(WgcAiscInputs {
                recovery: a0.recovery - h,
                ..a0
            })
            .expect("m")
            .aisc,
        );
    }

    #[test]
    fn test_algebraic_domain_invariants() {
        // Lane ordering: with positive fixed + opportunity costs, g_mine < g_mill and g_mine < g_refinery.
        let lane = eval_lane_cutoffs(LaneInputs {
            price: 100.0,
            refining_cost: 5.0,
            recovery: 0.9,
            milling_cost: 17.1,
            fixed_cost: 30.0,
            opportunity_cost: 27.0,
            mill_capacity: 10.0,
            refinery_capacity: 7.5,
        })
        .expect("lane");
        assert!(lane.g_mine < lane.g_mill && lane.g_mine < lane.g_refinery);

        // Two-product signs for t < f < c: dR/df > 0, dR/dc < 0, dR/dt < 0.
        let tp = eval_two_product_recovery(TwoProductInputs {
            feed_grade: 2.0,
            concentrate_grade: 20.0,
            tailings_grade: 0.2,
        })
        .expect("tp");
        assert!(tp.dr_df > 0.0 && tp.dr_dc < 0.0 && tp.dr_dt < 0.0);

        // NSR break-even grade evaluates NSR to 0 exactly.
        let n_in = NsrInputs {
            grade: 3.0,
            recovery: 0.9,
            price: 80.0,
            royalty_rate: 0.05,
            treatment_cost: 4.0,
            mining_and_processing_cost: 45.0,
        };
        let n_out = eval_nsr(n_in).expect("nsr");
        let at_be = eval_nsr(NsrInputs {
            grade: n_out.breakeven_grade,
            ..n_in
        })
        .expect("nsr at breakeven");
        assert!(at_be.nsr.abs() < 1e-13, "NSR(g_be) = {}", at_be.nsr);

        // WGC AISC elasticities (x / AISC) * dAISC/dx = (+1, -1, -1, -1).
        let a_in = WgcAiscInputs {
            total_cost: 800.0,
            tonnes: 10.0,
            grade: 2.0,
            recovery: 0.8,
        };
        let a_out = eval_wgc_aisc(a_in).expect("aisc");
        assert!((a_in.total_cost * a_out.daisc_dc / a_out.aisc - 1.0).abs() < 1e-14);
        assert!((a_in.tonnes * a_out.daisc_dt / a_out.aisc + 1.0).abs() < 1e-14);
        assert!((a_in.grade * a_out.daisc_dg / a_out.aisc + 1.0).abs() < 1e-14);
        assert!((a_in.recovery * a_out.daisc_dr / a_out.aisc + 1.0).abs() < 1e-14);
    }

    #[test]
    fn test_pole_and_domain_refusals_fail_closed() {
        let base_lane = LaneInputs {
            price: 100.0,
            refining_cost: 5.0,
            recovery: 0.9,
            milling_cost: 17.1,
            fixed_cost: 30.0,
            opportunity_cost: 27.0,
            mill_capacity: 10.0,
            refinery_capacity: 7.5,
        };
        assert_eq!(
            eval_lane_cutoffs(LaneInputs {
                price: 5.0,
                ..base_lane
            })
            .unwrap_err()
            .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            eval_lane_cutoffs(LaneInputs {
                refinery_capacity: 0.5, // (30+27)/0.5 = 114 > 95
                ..base_lane
            })
            .unwrap_err()
            .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            eval_two_product_recovery(TwoProductInputs {
                feed_grade: 2.0,
                concentrate_grade: 2.0,
                tailings_grade: 2.0, // pole c == t
            })
            .unwrap_err()
            .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            eval_nsr(NsrInputs {
                grade: 2.0,
                recovery: 0.9,
                price: 80.0,
                royalty_rate: 1.0, // 100% royalty pole
                treatment_cost: 4.0,
                mining_and_processing_cost: 45.0,
            })
            .unwrap_err()
            .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            eval_wgc_aisc(WgcAiscInputs {
                total_cost: 800.0,
                tonnes: 0.0,
                grade: 2.0,
                recovery: 0.8,
            })
            .unwrap_err()
            .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
    }

    #[test]
    fn test_golden_evaluators_are_bitwise_deterministic() {
        let inp = LaneInputs {
            price: 2_350.0,
            refining_cost: 45.0,
            recovery: 0.87,
            milling_cost: 28.5,
            fixed_cost: 120.0,
            opportunity_cost: 85.0,
            mill_capacity: 15.0,
            refinery_capacity: 0.40,
        };
        let a = eval_lane_cutoffs(inp).expect("run 1");
        let b = eval_lane_cutoffs(inp).expect("run 2");
        assert_eq!(a.g_mine.to_bits(), b.g_mine.to_bits());
        assert_eq!(a.g_mill.to_bits(), b.g_mill.to_bits());
        assert_eq!(a.g_refinery.to_bits(), b.g_refinery.to_bits());
        assert_eq!(
            a.dg_refinery_d_refinery_capacity.to_bits(),
            b.dg_refinery_d_refinery_capacity.to_bits()
        );
    }

    #[test]
    fn test_two_product_no_catastrophic_cancellation_near_zero_tailings() {
        // Near-zero tailings t = 1e-12 would lose ~12 digits under unsimplified subtraction.
        let f = 2.0_f64;
        let c = 20.0_f64;
        let t = 1e-12_f64;
        let out = eval_two_product_recovery(TwoProductInputs {
            feed_grade: f,
            concentrate_grade: c,
            tailings_grade: t,
        })
        .expect("near-zero tailings");

        let exact_dr_df = 2.5e-13_f64 / (1.0 - t / c);
        let exact_dr_dc = -2.5e-15_f64 * (1.0 - t / f) / ((1.0 - t / c) * (1.0 - t / c));
        let exact_dr_dt = -0.45_f64 / ((1.0 - t / c) * (1.0 - t / c));
        assert!(
            ((out.dr_df - exact_dr_df) / exact_dr_df).abs() < 1e-14,
            "dr_df lost precision: got {:.16e}, expected {:.16e}",
            out.dr_df,
            exact_dr_df
        );
        assert!(
            ((out.dr_dc - exact_dr_dc) / exact_dr_dc).abs() < 1e-14,
            "dr_dc lost precision: got {:.16e}, expected {:.16e}",
            out.dr_dc,
            exact_dr_dc
        );
        assert!(
            ((out.dr_dt - exact_dr_dt) / exact_dr_dt).abs() < 1e-14,
            "dr_dt lost precision: got {:.16e}, expected {:.16e}",
            out.dr_dt,
            exact_dr_dt
        );

        // Exact t = 0: recovery == 1, dr_df == +0.0, dr_dc == +0.0, dr_dt == -0.45.
        let zero_t = eval_two_product_recovery(TwoProductInputs {
            feed_grade: f,
            concentrate_grade: c,
            tailings_grade: 0.0,
        })
        .expect("zero tailings");
        assert_eq!(zero_t.recovery, 1.0);
        assert_eq!(zero_t.dr_df.to_bits(), 0.0_f64.to_bits());
        assert_eq!(zero_t.dr_dc.to_bits(), 0.0_f64.to_bits());
        assert!((zero_t.dr_dt + 0.45).abs() < 1e-15);
    }

    #[test]
    fn test_large_scale_no_premature_squaring_overflow_or_silent_underflow() {
        // WGC AISC at T = 1e160: metal^2 would overflow to +inf and silently underflow derivatives
        // to 0.0 unless simplified to -aisc / x.
        let aisc = eval_wgc_aisc(WgcAiscInputs {
            total_cost: 1e150,
            tonnes: 1e160,
            grade: 1.0,
            recovery: 1.0,
        })
        .expect("large-scale aisc");
        assert!((aisc.aisc - 1e-10).abs() <= 1e-24);
        assert!(
            aisc.daisc_dt != 0.0 && ((aisc.daisc_dt + 1e-170) / 1e-170).abs() < 1e-14,
            "daisc_dt silently underflowed or drifted: {:.16e}",
            aisc.daisc_dt
        );
        assert!(((aisc.daisc_dg + 1e-10) / 1e-10).abs() < 1e-14);
        assert!(((aisc.daisc_dr + 1e-10) / 1e-10).abs() < 1e-14);

        // Lane cut-offs at P = 1e200, H = K = 1e160: naive denom^2 / h^2 / k^2*denom_r^2 would
        // overflow to +inf and silently underflow derivatives to 0.0.
        let lane = eval_lane_cutoffs(LaneInputs {
            price: 1e200,
            refining_cost: 0.0,
            recovery: 1.0,
            milling_cost: 1e200,
            fixed_cost: 1e199,
            opportunity_cost: 0.0,
            mill_capacity: 1e160,
            refinery_capacity: 1e160,
        })
        .expect("large-scale lane");
        assert!((lane.g_mine - 1.0).abs() < 1e-15);
        assert!(
            lane.dg_mine_d_price != 0.0 && ((lane.dg_mine_d_price + 1e-200) / 1e-200).abs() < 1e-14
        );
        assert!((lane.dg_mine_d_recovery + 1.0).abs() < 1e-14);
        assert!(lane.dg_mill_d_mill_capacity < 0.0 && lane.dg_mill_d_mill_capacity.is_finite());
        assert!(
            lane.dg_refinery_d_refinery_capacity < 0.0
                && lane.dg_refinery_d_refinery_capacity.is_finite()
        );

        // Overhead sum overflow classifies as NumericalCertificateFailed207 (not domain error).
        let ov = eval_lane_cutoffs(LaneInputs {
            price: 100.0,
            refining_cost: 5.0,
            recovery: 0.9,
            milling_cost: 10.0,
            fixed_cost: 1e308,
            opportunity_cost: 1e308,
            mill_capacity: 10.0,
            refinery_capacity: 10.0,
        })
        .expect_err("overhead overflow must refuse");
        assert_eq!(ov.code, RefusalCode::NumericalCertificateFailed207.as_str());
    }

    #[test]
    fn test_parse_golden_fixture_refuses_malformed_and_contradictory_rows() {
        assert_eq!(
            parse_golden_fixture("# comments only\n\n")
                .unwrap_err()
                .code,
            RefusalCode::DigestMismatch505.as_str()
        );
        assert_eq!(
            parse_exact_rational("1/0").unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            parse_exact_rational("1/-5").unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            parse_exact_rational("1.5").unwrap_err().code,
            RefusalCode::DigestMismatch505.as_str()
        );
        // Contradictory exact_eval vs f64_eval must refuse even on standalone parse.
        let bad_match = "FORMULA\tid1\tfm-recon\tg\tg\tg\tg\t1/5\t0.5\n";
        assert_eq!(
            parse_golden_fixture(bad_match).unwrap_err().code,
            RefusalCode::DigestMismatch505.as_str()
        );
        // Duplicate ID refuses.
        let dup = "FORMULA\tid1\td\tv\tc\tl\tr\t1/2\t0.5\nFORMULA\tid1\td\tv\tc\tl\tr\t1/2\t0.5\n";
        assert_eq!(
            parse_golden_fixture(dup).unwrap_err().code,
            RefusalCode::DigestMismatch505.as_str()
        );
        // Whitespace-only column refuses.
        let blank_col = "FORMULA\tid1\t   \tv\tc\tl\tr\t1/2\t0.5\n";
        assert_eq!(
            parse_golden_fixture(blank_col).unwrap_err().code,
            RefusalCode::DigestMismatch505.as_str()
        );
        // Non-finite f64_eval refuses.
        let nan_row = "FORMULA\tid1\td\tv\tc\tl\tr\t1/2\tNaN\n";
        assert_eq!(
            parse_golden_fixture(nan_row).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
    }

    #[test]
    fn test_canonical_zero_eliminates_negative_zero_bits() {
        let pos_zero = 0.0_f64.to_bits();
        let lane = eval_lane_cutoffs(LaneInputs {
            price: 100.0,
            refining_cost: 5.0,
            recovery: 0.9,
            milling_cost: 0.0,
            fixed_cost: 0.0,
            opportunity_cost: 0.0,
            mill_capacity: 10.0,
            refinery_capacity: 10.0,
        })
        .expect("zero cost lane");
        assert_eq!(lane.g_mine.to_bits(), pos_zero);
        assert_eq!(lane.dg_mine_d_price.to_bits(), pos_zero);
        assert_eq!(lane.dg_mine_d_recovery.to_bits(), pos_zero);
        assert_eq!(lane.dg_mill_d_mill_capacity.to_bits(), pos_zero);
        assert_eq!(lane.dg_refinery_d_refinery_capacity.to_bits(), pos_zero);

        let aisc = eval_wgc_aisc(WgcAiscInputs {
            total_cost: 0.0,
            tonnes: 10.0,
            grade: 2.0,
            recovery: 0.8,
        })
        .expect("zero cost aisc");
        assert_eq!(aisc.aisc.to_bits(), pos_zero);
        assert_eq!(aisc.daisc_dt.to_bits(), pos_zero);
        assert_eq!(aisc.daisc_dg.to_bits(), pos_zero);
        assert_eq!(aisc.daisc_dr.to_bits(), pos_zero);

        let nsr = eval_nsr(NsrInputs {
            grade: 0.0,
            recovery: 0.9,
            price: 80.0,
            royalty_rate: 0.05,
            treatment_cost: 4.0,
            mining_and_processing_cost: 0.0,
        })
        .expect("zero grade nsr");
        assert_eq!(nsr.nsr.to_bits(), pos_zero);
        assert_eq!(nsr.dnsr_dtau.to_bits(), pos_zero);
        assert_eq!(nsr.breakeven_grade.to_bits(), pos_zero);

        let bil = eval_bilinear_metal(BilinearMetalInputs {
            dry_tonnes: -0.0,
            grade: -0.0,
            contained_metal: -0.0,
        })
        .expect("neg zero bilinear");
        assert_eq!(bil.residual.to_bits(), pos_zero);
        assert_eq!(bil.dr_dt.to_bits(), pos_zero);
        assert_eq!(bil.dr_dg.to_bits(), pos_zero);
    }
}
