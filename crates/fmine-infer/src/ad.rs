//! Exact reverse-mode automatic differentiation (`Tape` / `Var`) for unconstrained log-density
//! evaluation (`BD-232`, `FD-FJ-00`).
//! Determinism: BitwisePortable

use fmine_core::det;
use fmine_evidence::{Refusal, RefusalCode};

/// Dimensionless unconstrained real scalar at the public inference boundary (`L-6`).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Real(pub f64);

impl Real {
    /// Validate that this dimensionless scalar is finite and normalize `-0.0` to `+0.0`.
    pub fn validate(self) -> Result<Self, Refusal> {
        if !self.0.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("non-finite scalar input: {}", self.0),
            ));
        }
        Ok(Self(if self.0 == 0.0 { 0.0 } else { self.0 }))
    }

    /// Underlying `f64` value.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Real {
    type Error = Refusal;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self(value).validate()
    }
}

/// Handle to a scalar node recorded on a reverse-mode [`Tape`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Var {
    index: usize,
}

#[derive(Debug, Clone, Copy)]
struct Node {
    value: f64,
    parents: [usize; 2],
    partials: [f64; 2],
    arity: u8,
}

/// Reverse-mode automatic differentiation Wengert tape using `BitwisePortable` [`fmine_core::det`]
/// elementary functions (`exp`, `ln`, `tanh`, `sqrt`).
#[derive(Debug, Clone, Default)]
pub struct Tape {
    nodes: Vec<Node>,
}

impl Tape {
    /// Create an empty tape with preallocated capacity.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(cap),
        }
    }

    /// Current primal value of a variable node (`None` if `v` does not belong to this tape).
    #[must_use]
    pub fn try_value(&self, v: Var) -> Option<f64> {
        self.nodes.get(v.index).map(|n| n.value)
    }

    /// Current primal value of a variable node (panics only if `v` is from another tape).
    #[must_use]
    pub fn value(&self, v: Var) -> f64 {
        self.nodes[v.index].value
    }

    fn get_val(&self, v: Var) -> Result<f64, Refusal> {
        self.try_value(v).ok_or_else(|| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("AD variable index {} out of bounds for tape", v.index),
            )
        })
    }

    /// Record an independent input variable.
    pub fn input(&mut self, value: Real) -> Result<Var, Refusal> {
        let v = check_finite("ad_input", value.0)?;
        Ok(self.push(Node {
            value: v,
            parents: [0, 0],
            partials: [0.0, 0.0],
            arity: 0,
        }))
    }

    /// Record a slice of independent input variables.
    pub fn inputs(&mut self, values: &[f64]) -> Result<Vec<Var>, Refusal> {
        let mut out = Vec::with_capacity(values.len());
        for &raw in values {
            let r = Real(raw).validate()?;
            out.push(self.input(r)?);
        }
        Ok(out)
    }

    /// Record a constant node.
    pub fn constant(&mut self, value: Real) -> Result<Var, Refusal> {
        self.input(value)
    }

    /// Addition `a + b`.
    pub fn add(&mut self, a: Var, b: Var) -> Result<Var, Refusal> {
        let val = check_finite("ad_add", self.get_val(a)? + self.get_val(b)?)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, b.index],
            partials: [1.0, 1.0],
            arity: 2,
        }))
    }

    /// Subtraction `a - b`.
    pub fn sub(&mut self, a: Var, b: Var) -> Result<Var, Refusal> {
        let val = check_finite("ad_sub", self.get_val(a)? - self.get_val(b)?)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, b.index],
            partials: [1.0, -1.0],
            arity: 2,
        }))
    }

    /// Multiplication `a * b`.
    pub fn mul(&mut self, a: Var, b: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        let vb = self.get_val(b)?;
        let val = check_finite("ad_mul", va * vb)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, b.index],
            partials: [vb, va],
            arity: 2,
        }))
    }

    /// Division `a / b`, refusing zero or non-finite quotients.
    pub fn div(&mut self, a: Var, b: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        let vb = self.get_val(b)?;
        if vb == 0.0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "AD division by zero",
            ));
        }
        let val = check_finite("ad_div", va / vb)?;
        let da = check_finite("ad_div_da", 1.0 / vb)?;
        let db = check_finite("ad_div_db", -val / vb)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, b.index],
            partials: [da, db],
            arity: 2,
        }))
    }

    /// Unary negation `-a`.
    pub fn neg(&mut self, a: Var) -> Result<Var, Refusal> {
        let val = check_finite("ad_neg", -self.get_val(a)?)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [-1.0, 0.0],
            arity: 1,
        }))
    }

    /// Square `a * a`.
    pub fn square(&mut self, a: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        let val = check_finite("ad_square", va * va)?;
        let da = check_finite("ad_square_da", 2.0 * va)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Exponential `exp(a)` via `BitwisePortable` [`det::exp`].
    pub fn exp_op(&mut self, a: Var) -> Result<Var, Refusal> {
        let val = check_finite("ad_exp", det::exp(self.get_val(a)?))?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [val, 0.0],
            arity: 1,
        }))
    }

    /// Natural logarithm `ln(a)` via `BitwisePortable` [`det::ln`], refusing `a <= 0`.
    pub fn ln_op(&mut self, a: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        if va <= 0.0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("AD ln domain violation: {va} <= 0"),
            ));
        }
        let val = check_finite("ad_ln", det::ln(va))?;
        let da = check_finite("ad_ln_da", 1.0 / va)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Square root `sqrt(a)` for `a > 0`.
    pub fn sqrt(&mut self, a: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        if va <= 0.0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("AD sqrt domain violation: {va} <= 0"),
            ));
        }
        let val = check_finite("ad_sqrt", det::sqrt(va))?;
        let da = check_finite("ad_sqrt_da", 0.5 / val)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Logistic sigmoid `1 / (1 + exp(-a))`, overflow-safe for all finite `a`.
    pub fn sigmoid(&mut self, a: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        let val = sigmoid_f64(va);
        let da = check_finite("ad_sigmoid_da", val * (1.0 - val))?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Overflow-safe log-sigmoid `ln(sigmoid(a)) = -softplus(-a)` with derivative `sigmoid(-a)`.
    pub fn log_sigmoid(&mut self, a: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        let val = check_finite("ad_log_sigmoid", -softplus_f64(-va))?;
        let da = sigmoid_f64(-va);
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Overflow-safe `ln(1 - sigmoid(a)) = -softplus(a)` with derivative `-sigmoid(a)`.
    pub fn log_one_minus_sigmoid(&mut self, a: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        let val = check_finite("ad_log1m_sigmoid", -softplus_f64(va))?;
        let da = -sigmoid_f64(va);
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Overflow-safe softplus `ln(1 + exp(a))` with derivative `sigmoid(a)`.
    pub fn softplus(&mut self, a: Var) -> Result<Var, Refusal> {
        let va = self.get_val(a)?;
        let val = check_finite("ad_softplus", softplus_f64(va))?;
        let da = sigmoid_f64(va);
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Hyperbolic tangent `tanh(a)` via `BitwisePortable` [`det::tanh`].
    pub fn tanh_op(&mut self, a: Var) -> Result<Var, Refusal> {
        let val = check_finite("ad_tanh", det::tanh(self.get_val(a)?))?;
        let da = check_finite("ad_tanh_da", 1.0 - val * val)?;
        Ok(self.push(Node {
            value: val,
            parents: [a.index, 0],
            partials: [da, 0.0],
            arity: 1,
        }))
    }

    /// Reverse-mode backpropagation from scalar `output` to the requested `wrt` input variables.
    pub fn value_and_grad(&self, output: Var, wrt: &[Var]) -> Result<(f64, Vec<f64>), Refusal> {
        let n = self.nodes.len();
        if output.index >= n {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "AD output variable index out of bounds",
            ));
        }
        let val = check_finite("ad_output", self.nodes[output.index].value)?;
        let mut adjoints = vec![0.0_f64; n];
        adjoints[output.index] = 1.0;

        for idx in (0..=output.index).rev() {
            let adj = adjoints[idx];
            if adj == 0.0 {
                continue;
            }
            let node = self.nodes[idx];
            for k in 0..usize::from(node.arity) {
                let p = node.parents[k];
                let contrib = adj * node.partials[k];
                let updated = adjoints[p] + contrib;
                if !updated.is_finite() {
                    return Err(Refusal::new(
                        RefusalCode::NumericalCertificateFailed207,
                        "non-finite adjoint accumulated during reverse-mode AD",
                    ));
                }
                adjoints[p] = updated;
            }
        }

        let mut grads = Vec::with_capacity(wrt.len());
        for &v in wrt {
            if v.index >= n {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "AD wrt variable index out of bounds",
                ));
            }
            let g = check_finite("ad_grad", adjoints[v.index])?;
            grads.push(g);
        }
        Ok((val, grads))
    }

    fn push(&mut self, node: Node) -> Var {
        let index = self.nodes.len();
        self.nodes.push(node);
        Var { index }
    }
}

/// Overflow-safe `BitwisePortable` logistic sigmoid `1 / (1 + exp(-x))`.
#[must_use]
pub(crate) fn sigmoid_f64(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + det::exp(-x))
    } else {
        let e = det::exp(x);
        e / (1.0 + e)
    }
}

/// Overflow-safe `BitwisePortable` softplus `ln(1 + exp(x))`.
#[must_use]
pub(crate) fn softplus_f64(x: f64) -> f64 {
    if x > 35.0 {
        x
    } else if x < -35.0 {
        det::exp(x)
    } else if x >= 0.0 {
        x + det::ln(1.0 + det::exp(-x))
    } else {
        det::ln(1.0 + det::exp(x))
    }
}

/// Overflow-safe `BitwisePortable` `ln(exp(a) + exp(b))`.
#[must_use]
pub(crate) fn log_add_exp(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    if a >= b {
        a + det::ln(1.0 + det::exp(b - a))
    } else {
        b + det::ln(1.0 + det::exp(a - b))
    }
}

fn check_finite(label: &str, v: f64) -> Result<f64, Refusal> {
    if !v.is_finite() {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("non-finite value in {label}: {v}"),
        ));
    }
    Ok(if v == 0.0 { 0.0 } else { v })
}

/// Unconstrained differentiable target log-density for NUTS / HMC (`BD-232`).
///
/// Implementors evaluate $\log p(z)$ (up to an additive normalizing constant) and its exact
/// gradient $\nabla_z \log p(z)$ on $\mathbb{R}^d$, either via [`Tape`] reverse-mode AD or
/// closed-form analytic derivatives.
pub trait LogDensity {
    /// Dimension $d \ge 1$ of the unconstrained parameter vector $z \in \mathbb{R}^d$.
    fn dim(&self) -> usize;

    /// Human-readable parameter names (length `dim()`).
    fn param_names(&self) -> Vec<String>;

    /// Evaluate `(log_density(z), grad_z log_density(z))`.
    fn log_density_and_grad(&self, z: &[f64]) -> Result<(f64, Vec<f64>), Refusal>;
}

/// Convenience adapter that wraps any reverse-mode [`Tape`] log-density closure as a [`LogDensity`].
pub struct TapeLogDensity<F> {
    dim: usize,
    names: Vec<String>,
    build: F,
}

impl<F> TapeLogDensity<F>
where
    F: Fn(&mut Tape, &[Var]) -> Result<Var, Refusal>,
{
    /// Construct a tape-differentiated [`LogDensity`], refusing empty dimensions or mismatched names.
    pub fn new(names: Vec<String>, build: F) -> Result<Self, Refusal> {
        if names.is_empty() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "LogDensity requires at least one parameter",
            ));
        }
        let dim = names.len();
        Ok(Self { dim, names, build })
    }
}

impl<F> LogDensity for TapeLogDensity<F>
where
    F: Fn(&mut Tape, &[Var]) -> Result<Var, Refusal>,
{
    fn dim(&self) -> usize {
        self.dim
    }

    fn param_names(&self) -> Vec<String> {
        self.names.clone()
    }

    fn log_density_and_grad(&self, z: &[f64]) -> Result<(f64, Vec<f64>), Refusal> {
        if z.len() != self.dim {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "LogDensity dimension mismatch: expected {}, got {}",
                    self.dim,
                    z.len()
                ),
            ));
        }
        let mut tape = Tape::with_capacity(self.dim * 8 + 16);
        let vars = tape.inputs(z)?;
        let out = (self.build)(&mut tape, &vars)?;
        tape.value_and_grad(out, &vars)
    }
}
