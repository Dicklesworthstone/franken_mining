//! Bounded compilation of empirical formulas into a reusable scalar program.
//! Coordinates are numerical values in the card's explicitly declared units;
//! coefficients therefore belong to that convention, not to arbitrary SI inputs.
//! Determinism: DeterministicPerIsa

use super::{Result, fail};
use fmine_core::det;
use fmine_evidence::RefusalCode;
use std::collections::BTreeMap;

const MAX_OPS: usize = 512;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Constant(f64),
    Input(usize),
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Power(i32),
    Exp,
    Ln,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Formula(Vec<Op>);

impl Formula {
    pub(super) fn compile(text: &str, names: &[String]) -> Result<Self> {
        if text.is_empty() || text.len() > 4096 || !text.is_ascii() {
            return Err(syntax("formula must contain 1..4096 ASCII bytes"));
        }
        let mut parser = Parser {
            text: text.as_bytes(),
            at: 0,
            names: names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.as_str(), i))
                .collect(),
            program: Vec::new(),
        };
        parser.expression(0)?;
        parser.space();
        if parser.at != parser.text.len() {
            return Err(syntax("unexpected token or unbalanced parentheses"));
        }
        Ok(Self(parser.program))
    }

    /// Fixed-size evaluation workspace: no parsing, name lookup or allocation.
    pub(super) fn evaluate(&self, inputs: &[f64]) -> Result<f64> {
        let mut stack = [0.0; MAX_OPS];
        let mut top = 0;
        for op in &self.0 {
            let value = match *op {
                Op::Constant(v) => v,
                Op::Input(i) => *inputs
                    .get(i)
                    .ok_or_else(|| syntax("input layout changed"))?,
                _ => {
                    if top == 0 {
                        return Err(syntax("invalid compiled stack"));
                    }
                    top -= 1;
                    let right = stack[top];
                    match *op {
                        Op::Negate => -right,
                        Op::Power(p) => {
                            let value = det::powi(right, p);
                            if right != 0.0 && value == 0.0 {
                                return Err(arithmetic("power underflows"));
                            }
                            value
                        }
                        Op::Exp => {
                            let value = det::exp(right);
                            if value == 0.0 {
                                return Err(arithmetic("exponential underflows"));
                            }
                            value
                        }
                        Op::Ln => {
                            if right <= 0.0 {
                                return Err(domain("logarithm requires a positive coordinate"));
                            }
                            det::ln(right)
                        }
                        _ => {
                            if top == 0 {
                                return Err(syntax("invalid binary stack"));
                            }
                            top -= 1;
                            let left = stack[top];
                            match *op {
                                Op::Add => left + right,
                                Op::Subtract => left - right,
                                Op::Multiply => {
                                    let value = left * right;
                                    if left != 0.0 && right != 0.0 && value == 0.0 {
                                        return Err(arithmetic("product underflows"));
                                    }
                                    value
                                }
                                Op::Divide => {
                                    if right == 0.0 {
                                        return Err(domain("division by zero"));
                                    }
                                    let value = left / right;
                                    if left != 0.0 && value == 0.0 {
                                        return Err(arithmetic("quotient underflows"));
                                    }
                                    value
                                }
                                _ => return Err(syntax("invalid compiled operation")),
                            }
                        }
                    }
                }
            };
            if !value.is_finite() || top >= MAX_OPS {
                return Err(arithmetic(
                    "formula result or workspace is outside its finite domain",
                ));
            }
            stack[top] = value;
            top += 1;
        }
        if top != 1 {
            return Err(syntax("formula does not produce exactly one value"));
        }
        Ok(stack[0])
    }
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
    names: BTreeMap<&'a str, usize>,
    program: Vec<Op>,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        self.space();
        if self.text.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn push(&mut self, op: Op) -> Result<()> {
        if self.program.len() == MAX_OPS {
            return Err(syntax("formula exceeds 512 operations"));
        }
        self.program.push(op);
        Ok(())
    }
    fn expression(&mut self, depth: usize) -> Result<()> {
        self.term(depth)?;
        loop {
            let op = if self.take(b'+') {
                Op::Add
            } else if self.take(b'-') {
                Op::Subtract
            } else {
                return Ok(());
            };
            self.term(depth)?;
            self.push(op)?;
        }
    }
    fn term(&mut self, depth: usize) -> Result<()> {
        self.unary(depth)?;
        loop {
            let op = if self.take(b'*') {
                Op::Multiply
            } else if self.take(b'/') {
                Op::Divide
            } else {
                return Ok(());
            };
            self.unary(depth)?;
            self.push(op)?;
        }
    }
    fn unary(&mut self, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(syntax("formula nesting exceeds 32 levels"));
        }
        if self.take(b'-') {
            self.unary(depth + 1)?;
            return self.push(Op::Negate);
        }
        if self.take(b'+') {
            return self.unary(depth + 1);
        }
        self.primary(depth)?;
        if self.take(b'^') {
            self.space();
            let start = self.at;
            if matches!(self.text.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            while self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
            let text = std::str::from_utf8(&self.text[start..self.at])
                .map_err(|_| syntax("invalid exponent"))?;
            let exponent = text
                .parse::<i32>()
                .map_err(|_| syntax("integer exponent required"))?;
            if !(-16..=16).contains(&exponent) {
                return Err(syntax("integer exponent must lie in [-16,16]"));
            }
            self.push(Op::Power(exponent))?;
        }
        Ok(())
    }
    fn primary(&mut self, depth: usize) -> Result<()> {
        if self.take(b'(') {
            self.expression(depth + 1)?;
            return if self.take(b')') {
                Ok(())
            } else {
                Err(syntax("missing closing parenthesis"))
            };
        }
        self.space();
        let start = self.at;
        let first = self
            .text
            .get(self.at)
            .copied()
            .ok_or_else(|| syntax("missing operand"))?;
        if first.is_ascii_alphabetic() || first == b'_' {
            self.at += 1;
            while self
                .text
                .get(self.at)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            {
                self.at += 1;
            }
            let name = std::str::from_utf8(&self.text[start..self.at])
                .map_err(|_| syntax("invalid name"))?;
            if let Some(&i) = self.names.get(name) {
                return self.push(Op::Input(i));
            }
            let op = match name {
                "exp" | "EXP" => Op::Exp,
                "ln" | "LN" => Op::Ln,
                _ => return Err(syntax("undeclared coordinate or unsupported function")),
            };
            if !self.take(b'(') {
                return Err(syntax("function requires parentheses"));
            }
            self.expression(depth + 1)?;
            if !self.take(b')') {
                return Err(syntax("unbalanced function parentheses"));
            }
            return self.push(op);
        }
        if !first.is_ascii_digit() && first != b'.' {
            return Err(syntax("unsupported operand"));
        }
        while self
            .text
            .get(self.at)
            .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
        {
            self.at += 1;
        }
        if matches!(self.text.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.text.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            while self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
        }
        let text = std::str::from_utf8(&self.text[start..self.at])
            .map_err(|_| syntax("invalid literal"))?;
        let value = text
            .parse::<f64>()
            .map_err(|_| syntax("invalid decimal literal"))?;
        let nonzero_mantissa = text
            .split(['e', 'E'])
            .next()
            .is_some_and(|m| m.bytes().any(|b| (b'1'..=b'9').contains(&b)));
        if !value.is_finite() || (value == 0.0 && nonzero_mantissa) {
            return Err(syntax("literal is outside finite representable range"));
        }
        self.push(Op::Constant(value))
    }
}

fn syntax(what: &str) -> fmine_evidence::Refusal {
    fail(
        RefusalCode::NoClaimInDomain201,
        format!("formula parse: {what}"),
    )
}
fn arithmetic(what: &str) -> fmine_evidence::Refusal {
    fail(RefusalCode::NumericalCertificateFailed207, what)
}
fn domain(what: &str) -> fmine_evidence::Refusal {
    fail(RefusalCode::OutsidePhysicalDomain109, what)
}
