//! Determinism: BitwisePortable
//! Exact extensive conservation checker and commit-time ledger law (`BD-146`, `BD-147`, plan
//! §20.2 & §26.3 `CELL-FLOW`, `INV-005`, `PO-BED-6`).
//!
//! All extensive balances (`dry_tonnes`, `water_tonnes`, per-element metal) are checked strictly
//! in exact `fmine_units::ExactQty` rationals — zero bare `f64` parameters (L-6) and zero plugs.
//! Intensive grades (`metal ÷ dry_tonnes`) are derived strictly after extensive balance checks.
//! [`check_batch_conservation`] and [`conservation_commit_law`] wire this exact conservation law
//! directly into `fmine-ledger`'s commit-time law hooks on both L0 (`L0Ledger::commit_with_conservation`)
//! and L1 (`SemanticSegmentWriter::append_with_law` / `commit_prepared_with_law`).

use std::collections::BTreeMap;

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::sha256;
use fmine_ledger::batch::{DeltaBatch, DeltaKind, DeltaOp, Witness, WitnessKind};
use fmine_ledger::l0::L0Ledger;
use fmine_units::ExactQty;

use crate::FlowError;
use crate::graph::FlowGraph;

/// Canonical wire frame tag for `CELL-FLOW` period balance records (`FMNE` + `FLWB` + CRC32C).
pub const TAG_FLOW_BALANCE: [u8; 4] = *b"FLWB";
/// Canonical ledger key prefix for `CELL-FLOW` period balance records checked by `PO-BED-6`.
pub const FLOW_BALANCE_KEY_PREFIX: &str = "flow/period/";
const FMT_FLWB_V1: u16 = 1;
const TAG_FLOW_WITNESS: [u8; 4] = *b"FLWW";
const MAX_FLOW_ENTRIES: usize = 65_536;
const MAX_FLOW_ELEMENTS: usize = 1_024;
const MAX_ID_BYTES: usize = 256;

/// Extensive physical quantities for one stream transfer or inventory snapshot (`ExactQty`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensiveQuantities {
    /// Dry solid mass (e.g. dry metric tonnes).
    pub dry_mass: ExactQty,
    /// Water mass (optional, `0` when dry stream).
    pub water_mass: ExactQty,
    /// Contained elemental metal masses keyed by canonical element symbol (e.g. `"Au"`, `"Cu"`).
    pub elements: BTreeMap<String, ExactQty>,
}

impl ExtensiveQuantities {
    /// Zero extensive quantities.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            dry_mass: ExactQty::from_integer(0),
            water_mass: ExactQty::from_integer(0),
            elements: BTreeMap::new(),
        }
    }

    /// Construct extensive quantities, refusing negative physical masses.
    pub fn new(
        dry_mass: ExactQty,
        water_mass: ExactQty,
        elements: BTreeMap<String, ExactQty>,
    ) -> Result<Self, FlowError> {
        if dry_mass.signum() < 0 {
            return Err(FlowError::InvalidQuantity {
                detail: "dry_mass must be >= 0".to_string(),
            });
        }
        if water_mass.signum() < 0 {
            return Err(FlowError::InvalidQuantity {
                detail: "water_mass must be >= 0".to_string(),
            });
        }
        for (el, qty) in &elements {
            if qty.signum() < 0 {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("element '{el}' mass must be >= 0"),
                });
            }
        }
        Ok(Self {
            dry_mass,
            water_mass,
            elements,
        })
    }

    fn checked_add(&self, other: &Self) -> Result<Self, FlowError> {
        let dry_mass = self.dry_mass.checked_add(other.dry_mass)?;
        let water_mass = self.water_mass.checked_add(other.water_mass)?;
        let mut elements = self.elements.clone();
        for (el, &q) in &other.elements {
            let cur = elements
                .get(el)
                .copied()
                .unwrap_or_else(|| ExactQty::from_integer(0));
            elements.insert(el.clone(), cur.checked_add(q)?);
        }
        Ok(Self {
            dry_mass,
            water_mass,
            elements,
        })
    }

    fn checked_sub(&self, other: &Self) -> Result<Self, FlowError> {
        let dry_mass = self.dry_mass.checked_sub(other.dry_mass)?;
        let water_mass = self.water_mass.checked_sub(other.water_mass)?;
        let mut elements = self.elements.clone();
        for (el, &q) in &other.elements {
            let cur = elements
                .get(el)
                .copied()
                .unwrap_or_else(|| ExactQty::from_integer(0));
            elements.insert(el.clone(), cur.checked_sub(q)?);
        }
        Ok(Self {
            dry_mass,
            water_mass,
            elements,
        })
    }
}

/// Measured or reported flow along a single registered stream for one period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamFlow {
    pub stream_id: String,
    pub quantities: ExtensiveQuantities,
}

/// Opening and closing inventory for a single inventory-holding node over one period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryDelta {
    pub node_id: String,
    pub opening: ExtensiveQuantities,
    pub closing: ExtensiveQuantities,
}

/// Exact non-negative conservation tolerances per channel (`PO-BED-6`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceTolerance {
    pub dry_mass: ExactQty,
    pub water_mass: ExactQty,
    pub default_element: ExactQty,
    pub per_element: BTreeMap<String, ExactQty>,
}

impl BalanceTolerance {
    /// Strict zero-residual exact balance tolerance.
    #[must_use]
    pub fn exact_zero() -> Self {
        let z = ExactQty::from_integer(0);
        Self {
            dry_mass: z,
            water_mass: z,
            default_element: z,
            per_element: BTreeMap::new(),
        }
    }

    fn element_tol(&self, element: &str) -> ExactQty {
        self.per_element
            .get(element)
            .copied()
            .unwrap_or(self.default_element)
    }
}

/// Exact residual summary (`Σ in − Σ out − Δ inventory`) for one interior conservation node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeResidual {
    pub node_id: String,
    pub dry_mass_residual: ExactQty,
    pub water_mass_residual: ExactQty,
    pub element_residuals: BTreeMap<String, ExactQty>,
}

/// Verified period conservation report across all interior nodes of a [`FlowGraph`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceReport {
    pub node_residuals: Vec<NodeResidual>,
    /// Whole-system net extensive input minus output minus total inventory change.
    pub system_dry_mass_residual: ExactQty,
    pub system_element_residuals: BTreeMap<String, ExactQty>,
}

fn abs_exact(q: ExactQty) -> Result<ExactQty, FlowError> {
    if q.signum() < 0 {
        Ok(q.checked_neg()?)
    } else {
        Ok(q)
    }
}

fn check_channel(
    node_id: &str,
    channel: &str,
    residual: ExactQty,
    tolerance: ExactQty,
) -> Result<(), FlowError> {
    if tolerance.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: format!("negative tolerance on channel '{channel}'"),
        });
    }
    let mag = abs_exact(residual)?;
    if mag.checked_cmp(tolerance)? == core::cmp::Ordering::Greater {
        return Err(FlowError::ConservationViolation {
            node_id: node_id.to_string(),
            channel: channel.to_string(),
            residual: format!("{}/{}", mag.num(), mag.den()),
            tolerance: format!("{}/{}", tolerance.num(), tolerance.den()),
        });
    }
    Ok(())
}

fn validate_non_negative(q: &ExtensiveQuantities, label: &str) -> Result<(), FlowError> {
    if q.dry_mass.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: format!("{label} has negative dry_mass"),
        });
    }
    if q.water_mass.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: format!("{label} has negative water_mass"),
        });
    }
    for (el, qty) in &q.elements {
        if el.is_empty() {
            return Err(FlowError::InvalidQuantity {
                detail: format!("{label} has empty element symbol"),
            });
        }
        if qty.signum() < 0 {
            return Err(FlowError::InvalidQuantity {
                detail: format!("{label} has negative mass for element '{el}'"),
            });
        }
    }
    Ok(())
}

fn validate_tolerance(tolerance: &BalanceTolerance) -> Result<(), FlowError> {
    if tolerance.dry_mass.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "negative tolerance on channel 'dry_mass'".to_string(),
        });
    }
    if tolerance.water_mass.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "negative tolerance on channel 'water_mass'".to_string(),
        });
    }
    if tolerance.default_element.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "negative default_element tolerance".to_string(),
        });
    }
    for (el, tol) in &tolerance.per_element {
        if tol.signum() < 0 {
            return Err(FlowError::InvalidQuantity {
                detail: format!("negative tolerance on element '{el}'"),
            });
        }
    }
    Ok(())
}

/// Verify exact extensive conservation (`Σ in − Σ out − Δ inventory = 0` within `tolerance`)
/// at every interior node of `graph` (`INV-005`, `PO-BED-6`). Refuses violations with
/// `FMINE-REF-101` — never plugs or adjusts observations.
pub fn verify_period_balance(
    graph: &FlowGraph,
    flows: &[StreamFlow],
    inventories: &[InventoryDelta],
    tolerance: &BalanceTolerance,
) -> Result<BalanceReport, FlowError> {
    validate_tolerance(tolerance)?;
    let mut flow_map: BTreeMap<&str, &ExtensiveQuantities> = BTreeMap::new();
    for f in flows {
        let Some(stream) = graph.stream(&f.stream_id) else {
            return Err(FlowError::UnknownStream {
                stream_id: f.stream_id.clone(),
            });
        };
        validate_non_negative(&f.quantities, &format!("stream '{}'", f.stream_id))?;
        if !stream.carries_water && f.quantities.water_mass.signum() > 0 {
            return Err(FlowError::InvalidQuantity {
                detail: format!(
                    "stream '{}' does not carry water (carries_water = false) but received positive water_mass",
                    f.stream_id
                ),
            });
        }
        for el in f.quantities.elements.keys() {
            if !stream.elements.contains(el) {
                return Err(FlowError::InvalidQuantity {
                    detail: format!(
                        "stream '{}' received undeclared element '{el}'",
                        f.stream_id
                    ),
                });
            }
        }
        if flow_map
            .insert(f.stream_id.as_str(), &f.quantities)
            .is_some()
        {
            return Err(FlowError::DuplicateId {
                kind: "stream_flow",
                id: f.stream_id.clone(),
            });
        }
    }

    let mut inv_map: BTreeMap<&str, &InventoryDelta> = BTreeMap::new();
    for inv in inventories {
        let Some(node) = graph.node(&inv.node_id) else {
            return Err(FlowError::UnknownNode {
                node_id: inv.node_id.clone(),
            });
        };
        if !node.holds_inventory {
            return Err(FlowError::InvalidQuantity {
                detail: format!(
                    "node '{}' does not hold inventory; cannot supply InventoryDelta",
                    inv.node_id
                ),
            });
        }
        validate_non_negative(
            &inv.opening,
            &format!("opening inventory '{}'", inv.node_id),
        )?;
        validate_non_negative(
            &inv.closing,
            &format!("closing inventory '{}'", inv.node_id),
        )?;
        if inv_map.insert(inv.node_id.as_str(), inv).is_some() {
            return Err(FlowError::DuplicateId {
                kind: "inventory_delta",
                id: inv.node_id.clone(),
            });
        }
    }

    let mut node_residuals = Vec::new();
    let mut system_dry = ExactQty::from_integer(0);
    let mut system_elems: BTreeMap<String, ExactQty> = BTreeMap::new();

    for node in graph.interior_nodes() {
        let mut total_in = ExtensiveQuantities::zero();
        for s in graph.incoming_streams(&node.id) {
            let Some(q) = flow_map.get(s.id.as_str()) else {
                return Err(FlowError::InvalidQuantity {
                    detail: format!(
                        "missing StreamFlow for incoming stream '{}' into interior node '{}'",
                        s.id, node.id
                    ),
                });
            };
            total_in = total_in.checked_add(q)?;
        }
        let mut total_out = ExtensiveQuantities::zero();
        for s in graph.outgoing_streams(&node.id) {
            let Some(q) = flow_map.get(s.id.as_str()) else {
                return Err(FlowError::InvalidQuantity {
                    detail: format!(
                        "missing StreamFlow for outgoing stream '{}' from interior node '{}'",
                        s.id, node.id
                    ),
                });
            };
            total_out = total_out.checked_add(q)?;
        }
        let delta_inv = if let Some(inv) = inv_map.get(node.id.as_str()) {
            inv.closing.checked_sub(&inv.opening)?
        } else if node.class == crate::graph::NodeClass::StockpileOrPad {
            return Err(FlowError::InvalidQuantity {
                detail: format!(
                    "missing InventoryDelta for stockpile node '{}' (cannot assume zero Δ inventory)",
                    node.id
                ),
            });
        } else {
            ExtensiveQuantities::zero()
        };

        let net_flow = total_in.checked_sub(&total_out)?;
        let residual = net_flow.checked_sub(&delta_inv)?;

        check_channel(&node.id, "dry_mass", residual.dry_mass, tolerance.dry_mass)?;
        check_channel(
            &node.id,
            "water_mass",
            residual.water_mass,
            tolerance.water_mass,
        )?;
        for (el, &res) in &residual.elements {
            check_channel(&node.id, el, res, tolerance.element_tol(el))?;
            let cur = system_elems
                .get(el)
                .copied()
                .unwrap_or_else(|| ExactQty::from_integer(0));
            system_elems.insert(el.clone(), cur.checked_add(res)?);
        }
        system_dry = system_dry.checked_add(residual.dry_mass)?;

        node_residuals.push(NodeResidual {
            node_id: node.id.clone(),
            dry_mass_residual: residual.dry_mass,
            water_mass_residual: residual.water_mass,
            element_residuals: residual.elements,
        });
    }

    Ok(BalanceReport {
        node_residuals,
        system_dry_mass_residual: system_dry,
        system_element_residuals: system_elems,
    })
}

/// Derive an exact intensive grade (`metal ÷ dry_mass`) strictly after extensive reconciliation.
/// Refuses non-positive dry mass (`NOT_DEFINED`, plan §20.2).
pub fn derive_grade(metal: ExactQty, dry_mass: ExactQty) -> Result<ExactQty, FlowError> {
    if dry_mass.signum() <= 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "cannot derive grade for non-positive dry_mass (NOT_DEFINED)".to_string(),
        });
    }
    if metal.signum() < 0 {
        return Err(FlowError::InvalidQuantity {
            detail: "cannot derive grade for negative metal mass".to_string(),
        });
    }
    Ok(metal.checked_div(dry_mass)?)
}

/// Canonical `CELL-FLOW` period material-balance record committed under `flow/period/<period_id>`
/// and enforced at commit time (`BD-147`, `INV-005`, `PO-BED-6`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodBalanceRecord {
    /// Non-empty trimmed period identifier (e.g. `"2026-01"`).
    pub period_id: String,
    /// Optional immediately preceding period identifier whose committed closing inventory must
    /// equal this period's opening inventory at every inventory-holding node (no hidden step-change
    /// or plug between periods).
    pub previous_period_id: Option<String>,
    /// Measured or reconciled extensive flows for streams in this period.
    pub flows: Vec<StreamFlow>,
    /// Opening and closing inventories for inventory-holding nodes in this period.
    pub inventories: Vec<InventoryDelta>,
}

fn encode_qty(enc: &mut Encoder, q: ExactQty) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

fn decode_qty(dec: &mut Decoder<'_>) -> Result<ExactQty, FlowError> {
    let num = dec.get_i128().map_err(|e| FlowError::InvalidQuantity {
        detail: format!("codec error reading ExactQty numerator: {e}"),
    })?;
    let den = dec.get_i128().map_err(|e| FlowError::InvalidQuantity {
        detail: format!("codec error reading ExactQty denominator: {e}"),
    })?;
    if den <= 0 {
        return Err(FlowError::InvalidQuantity {
            detail: format!("ExactQty denominator {den} must be > 0"),
        });
    }
    let q = ExactQty::new(num, den)?;
    if q.num() != num || q.den() != den {
        return Err(FlowError::InvalidQuantity {
            detail: format!(
                "non-canonical unreduced ExactQty {num}/{den} (canonical {}/{})",
                q.num(),
                q.den()
            ),
        });
    }
    Ok(q)
}

fn encode_extensive(enc: &mut Encoder, q: &ExtensiveQuantities) -> Result<(), FlowError> {
    validate_non_negative(q, "extensive quantities")?;
    if q.elements.len() > MAX_FLOW_ELEMENTS {
        return Err(FlowError::InvalidQuantity {
            detail: format!(
                "element count {} exceeds {MAX_FLOW_ELEMENTS}",
                q.elements.len()
            ),
        });
    }
    encode_qty(enc, q.dry_mass);
    encode_qty(enc, q.water_mass);
    enc.put_u32(q.elements.len() as u32);
    for (el, &qty) in &q.elements {
        enc.put_str(el);
        encode_qty(enc, qty);
    }
    Ok(())
}

fn decode_extensive(dec: &mut Decoder<'_>) -> Result<ExtensiveQuantities, FlowError> {
    let dry_mass = decode_qty(dec)?;
    let water_mass = decode_qty(dec)?;
    let elem_count = dec.get_u32().map_err(|e| FlowError::InvalidQuantity {
        detail: format!("codec error reading element count: {e}"),
    })? as usize;
    if elem_count > MAX_FLOW_ELEMENTS {
        return Err(FlowError::InvalidQuantity {
            detail: format!("element count {elem_count} exceeds {MAX_FLOW_ELEMENTS}"),
        });
    }
    let mut elements = BTreeMap::new();
    let mut prev_el: Option<String> = None;
    for _ in 0..elem_count {
        let el = dec
            .get_str(MAX_ID_BYTES)
            .map_err(|e| FlowError::InvalidQuantity {
                detail: format!("codec error reading element symbol: {e}"),
            })?
            .to_string();
        if el.is_empty() || el.trim() != el {
            return Err(FlowError::InvalidQuantity {
                detail: format!("invalid element symbol {el:?}"),
            });
        }
        if let Some(prev) = &prev_el
            && prev.as_str() >= el.as_str()
        {
            return Err(FlowError::InvalidQuantity {
                detail: format!("element '{el}' is duplicate or out of canonical order"),
            });
        }
        let qty = decode_qty(dec)?;
        prev_el = Some(el.clone());
        elements.insert(el, qty);
    }
    ExtensiveQuantities::new(dry_mass, water_mass, elements)
}

fn extensive_equal(a: &ExtensiveQuantities, b: &ExtensiveQuantities) -> bool {
    if a.dry_mass != b.dry_mass || a.water_mass != b.water_mass {
        return false;
    }
    let zero = ExactQty::from_integer(0);
    for el in a.elements.keys().chain(b.elements.keys()) {
        let av = a.elements.get(el).copied().unwrap_or(zero);
        let bv = b.elements.get(el).copied().unwrap_or(zero);
        if av != bv {
            return false;
        }
    }
    true
}

impl PeriodBalanceRecord {
    /// Canonical `CELL-FLOW` ledger key for this period (`flow/period/<period_id>`).
    #[must_use]
    pub fn delta_key(&self) -> String {
        format!("{FLOW_BALANCE_KEY_PREFIX}{}", self.period_id)
    }

    /// Encode into canonical `FMNE` + `FLWB` v1 + CRC32C wire bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, FlowError> {
        if self.period_id.is_empty()
            || self.period_id.trim() != self.period_id
            || self.period_id.len() > MAX_ID_BYTES
        {
            return Err(FlowError::InvalidQuantity {
                detail: format!("invalid period_id {:?}", self.period_id),
            });
        }
        if let Some(prev) = &self.previous_period_id
            && (prev.is_empty()
                || prev.trim() != prev
                || prev.len() > MAX_ID_BYTES
                || prev == &self.period_id)
        {
            return Err(FlowError::InvalidQuantity {
                detail: format!("invalid previous_period_id {prev:?}"),
            });
        }
        if self.flows.len() > MAX_FLOW_ENTRIES || self.inventories.len() > MAX_FLOW_ENTRIES {
            return Err(FlowError::InvalidQuantity {
                detail: format!("flow or inventory count exceeds {MAX_FLOW_ENTRIES}"),
            });
        }

        let mut canonical_flows: BTreeMap<&str, &ExtensiveQuantities> = BTreeMap::new();
        for f in &self.flows {
            if f.stream_id.is_empty()
                || f.stream_id.trim() != f.stream_id
                || f.stream_id.len() > MAX_ID_BYTES
            {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("invalid stream_id {:?}", f.stream_id),
                });
            }
            if canonical_flows
                .insert(f.stream_id.as_str(), &f.quantities)
                .is_some()
            {
                return Err(FlowError::DuplicateId {
                    kind: "stream_flow",
                    id: f.stream_id.clone(),
                });
            }
        }
        let mut canonical_invs: BTreeMap<&str, &InventoryDelta> = BTreeMap::new();
        for inv in &self.inventories {
            if inv.node_id.is_empty()
                || inv.node_id.trim() != inv.node_id
                || inv.node_id.len() > MAX_ID_BYTES
            {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("invalid inventory node_id {:?}", inv.node_id),
                });
            }
            if canonical_invs.insert(inv.node_id.as_str(), inv).is_some() {
                return Err(FlowError::DuplicateId {
                    kind: "inventory_delta",
                    id: inv.node_id.clone(),
                });
            }
        }

        let mut enc = Encoder::new_frame(TAG_FLOW_BALANCE, FMT_FLWB_V1);
        enc.put_str(&self.period_id);
        match &self.previous_period_id {
            None => enc.put_u8(0),
            Some(prev) => {
                enc.put_u8(1);
                enc.put_str(prev);
            }
        }
        enc.put_u32(canonical_flows.len() as u32);
        for (stream_id, quantities) in canonical_flows {
            enc.put_str(stream_id);
            encode_extensive(&mut enc, quantities)?;
        }
        enc.put_u32(canonical_invs.len() as u32);
        for (node_id, inv) in canonical_invs {
            enc.put_str(node_id);
            encode_extensive(&mut enc, &inv.opening)?;
            encode_extensive(&mut enc, &inv.closing)?;
        }
        Ok(enc.finish())
    }

    /// Decode canonical `FMNE` + `FLWB` v1 + CRC32C wire bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FlowError> {
        let mut dec = Decoder::open_frame(bytes, TAG_FLOW_BALANCE, FMT_FLWB_V1, FMT_FLWB_V1)
            .map_err(|e| FlowError::InvalidQuantity {
                detail: format!("FLWB frame decode failed: {e}"),
            })?;
        let period_id = dec
            .get_str(MAX_ID_BYTES)
            .map_err(|e| FlowError::InvalidQuantity {
                detail: format!("FLWB period_id decode failed: {e}"),
            })?
            .to_string();
        if period_id.is_empty() || period_id.trim() != period_id {
            return Err(FlowError::InvalidQuantity {
                detail: format!("invalid period_id {period_id:?}"),
            });
        }
        let previous_period_id = match dec.get_u8().map_err(|e| FlowError::InvalidQuantity {
            detail: format!("FLWB previous_period flag decode failed: {e}"),
        })? {
            0 => None,
            1 => {
                let prev = dec
                    .get_str(MAX_ID_BYTES)
                    .map_err(|e| FlowError::InvalidQuantity {
                        detail: format!("FLWB previous_period_id decode failed: {e}"),
                    })?
                    .to_string();
                if prev.is_empty() || prev.trim() != prev || prev == period_id {
                    return Err(FlowError::InvalidQuantity {
                        detail: format!("invalid previous_period_id {prev:?}"),
                    });
                }
                Some(prev)
            }
            tag => {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("invalid previous_period flag {tag}"),
                });
            }
        };

        let flow_count = dec.get_u32().map_err(|e| FlowError::InvalidQuantity {
            detail: format!("FLWB flow count decode failed: {e}"),
        })? as usize;
        if flow_count > MAX_FLOW_ENTRIES {
            return Err(FlowError::InvalidQuantity {
                detail: format!("flow count {flow_count} exceeds {MAX_FLOW_ENTRIES}"),
            });
        }
        let mut flows = Vec::with_capacity(flow_count);
        let mut prev_stream: Option<String> = None;
        for _ in 0..flow_count {
            let stream_id = dec
                .get_str(MAX_ID_BYTES)
                .map_err(|e| FlowError::InvalidQuantity {
                    detail: format!("FLWB stream_id decode failed: {e}"),
                })?
                .to_string();
            if stream_id.is_empty() || stream_id.trim() != stream_id {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("invalid stream_id {stream_id:?}"),
                });
            }
            if let Some(prev) = &prev_stream
                && prev.as_str() >= stream_id.as_str()
            {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("stream_id '{stream_id}' is duplicate or unsorted"),
                });
            }
            let quantities = decode_extensive(&mut dec)?;
            prev_stream = Some(stream_id.clone());
            flows.push(StreamFlow {
                stream_id,
                quantities,
            });
        }

        let inv_count = dec.get_u32().map_err(|e| FlowError::InvalidQuantity {
            detail: format!("FLWB inventory count decode failed: {e}"),
        })? as usize;
        if inv_count > MAX_FLOW_ENTRIES {
            return Err(FlowError::InvalidQuantity {
                detail: format!("inventory count {inv_count} exceeds {MAX_FLOW_ENTRIES}"),
            });
        }
        let mut inventories = Vec::with_capacity(inv_count);
        let mut prev_node: Option<String> = None;
        for _ in 0..inv_count {
            let node_id = dec
                .get_str(MAX_ID_BYTES)
                .map_err(|e| FlowError::InvalidQuantity {
                    detail: format!("FLWB inventory node_id decode failed: {e}"),
                })?
                .to_string();
            if node_id.is_empty() || node_id.trim() != node_id {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("invalid inventory node_id {node_id:?}"),
                });
            }
            if let Some(prev) = &prev_node
                && prev.as_str() >= node_id.as_str()
            {
                return Err(FlowError::InvalidQuantity {
                    detail: format!("inventory node_id '{node_id}' is duplicate or unsorted"),
                });
            }
            let opening = decode_extensive(&mut dec)?;
            let closing = decode_extensive(&mut dec)?;
            prev_node = Some(node_id.clone());
            inventories.push(InventoryDelta {
                node_id,
                opening,
                closing,
            });
        }
        dec.finish().map_err(|e| FlowError::InvalidQuantity {
            detail: format!("FLWB trailing bytes: {e}"),
        })?;
        Ok(Self {
            period_id,
            previous_period_id,
            flows,
            inventories,
        })
    }

    /// Wrap this period balance record as an append-only `Put` or `Supersede` [`DeltaOp`].
    pub fn into_delta_op(&self, kind: DeltaKind) -> Result<DeltaOp, FlowError> {
        if !matches!(kind, DeltaKind::Put | DeltaKind::Supersede) {
            return Err(FlowError::InvalidQuantity {
                detail: format!("flow period delta kind must be Put or Supersede, got {kind:?}"),
            });
        }
        Ok(DeltaOp::new(self.delta_key(), kind, self.to_bytes()?))
    }
}

/// Build the canonical `WitnessKind::Conservation` witness for a verified [`BalanceReport`]
/// (`BD-147`, `PO-BED-6`).
pub fn conservation_witness(period_id: &str, report: &BalanceReport) -> Result<Witness, FlowError> {
    if period_id.is_empty() || period_id.trim() != period_id || period_id.len() > MAX_ID_BYTES {
        return Err(FlowError::InvalidQuantity {
            detail: format!("invalid period_id {period_id:?}"),
        });
    }
    let mut enc = Encoder::new_frame(TAG_FLOW_WITNESS, FMT_FLWB_V1);
    enc.put_str(period_id);
    encode_qty(&mut enc, report.system_dry_mass_residual);
    enc.put_u32(report.system_element_residuals.len() as u32);
    for (el, &res) in &report.system_element_residuals {
        enc.put_str(el);
        encode_qty(&mut enc, res);
    }
    enc.put_u32(report.node_residuals.len() as u32);
    for nr in &report.node_residuals {
        enc.put_str(&nr.node_id);
        encode_qty(&mut enc, nr.dry_mass_residual);
        encode_qty(&mut enc, nr.water_mass_residual);
        enc.put_u32(nr.element_residuals.len() as u32);
        for (el, &res) in &nr.element_residuals {
            enc.put_str(el);
            encode_qty(&mut enc, res);
        }
    }
    let digest = sha256(&enc.finish());
    Ok(Witness::new(
        WitnessKind::Conservation,
        format!("{FLOW_BALANCE_KEY_PREFIX}{period_id}"),
        digest,
    ))
}

/// Commit-time conservation law (`BD-147`, `INV-005`, `PO-BED-6`) for `L0Ledger::commit_with_conservation`,
/// `SemanticSegmentWriter::append_with_law`, and `SemanticSegmentWriter::commit_prepared_with_law`.
///
/// Inspects every delta under [`FLOW_BALANCE_KEY_PREFIX`] in `batch`, decodes its canonical
/// [`PeriodBalanceRecord`], verifies exact extensive conservation across every interior node of
/// `graph` within `tolerance`, checks cross-period inventory continuity (`current.opening ==
/// previous.closing`) against prior committed state in `ledger` (or earlier in the same batch),
/// and verifies any attached `WitnessKind::Conservation` digest. Any failure returns `Err(...)`,
/// causing `fmine-ledger` to refuse the commit with `L0Error::ConservationViolation` (`FMINE-REF-101`,
/// exit 3) and leave both L0 and L1 state untouched.
pub fn check_batch_conservation(
    graph: &FlowGraph,
    tolerance: &BalanceTolerance,
    ledger: &L0Ledger,
    batch: &DeltaBatch,
) -> Result<(), String> {
    validate_tolerance(tolerance).map_err(|e| format!("{} ({})", e.to_refusal().code, e))?;
    let mut in_batch_records: BTreeMap<String, PeriodBalanceRecord> = BTreeMap::new();

    for op in &batch.deltas {
        if !op.key.starts_with(FLOW_BALANCE_KEY_PREFIX) {
            continue;
        }
        if !matches!(op.kind, DeltaKind::Put | DeltaKind::Supersede) {
            return Err(format!(
                "FMINE-REF-101: flow period key '{}' requires Put or Supersede, got {:?}",
                op.key, op.kind
            ));
        }
        let rec = PeriodBalanceRecord::from_bytes(&op.payload).map_err(|e| {
            format!(
                "FMINE-REF-101: invalid flow period payload on '{}': {e}",
                op.key
            )
        })?;
        let expected_key = rec.delta_key();
        if op.key != expected_key {
            return Err(format!(
                "FMINE-REF-101: delta key '{}' does not match payload period key '{}'",
                op.key, expected_key
            ));
        }
        let report = verify_period_balance(graph, &rec.flows, &rec.inventories, tolerance)
            .map_err(|e| format!("{} ({})", e.to_refusal().code, e))?;

        if let Some(prev_id) = &rec.previous_period_id {
            let prev_rec = if let Some(in_batch) = in_batch_records.get(prev_id) {
                in_batch.clone()
            } else {
                let prev_key = format!("{FLOW_BALANCE_KEY_PREFIX}{prev_id}");
                let history = ledger.key_history(&batch.cell_id, &prev_key).map_err(|e| {
                    format!(
                        "FMINE-REF-101: previous period '{prev_id}' not found in cell '{}': {e}",
                        batch.cell_id
                    )
                })?;
                let latest = history.last().ok_or_else(|| {
                    format!(
                        "FMINE-REF-101: previous period '{prev_id}' has no committed version in cell '{}'",
                        batch.cell_id
                    )
                })?;
                PeriodBalanceRecord::from_bytes(&latest.payload).map_err(|e| {
                    format!(
                        "FMINE-REF-101: committed previous period '{prev_id}' payload corrupt: {e}"
                    )
                })?
            };

            let prev_closing: BTreeMap<&str, &ExtensiveQuantities> = prev_rec
                .inventories
                .iter()
                .map(|inv| (inv.node_id.as_str(), &inv.closing))
                .collect();
            let mut seen_nodes: BTreeMap<&str, ()> = BTreeMap::new();
            for cur_inv in &rec.inventories {
                seen_nodes.insert(cur_inv.node_id.as_str(), ());
                let Some(&prior) = prev_closing.get(cur_inv.node_id.as_str()) else {
                    return Err(format!(
                        "FMINE-REF-101: node '{}' opening inventory in '{}' has no matching closing inventory in previous period '{prev_id}'",
                        cur_inv.node_id, rec.period_id
                    ));
                };
                if !extensive_equal(&cur_inv.opening, prior) {
                    return Err(format!(
                        "FMINE-REF-101: cross-period inventory continuity violated at node '{}' between '{prev_id}' closing and '{}' opening (no hidden plug)",
                        cur_inv.node_id, rec.period_id
                    ));
                }
            }
            let zero_qty = ExtensiveQuantities::zero();
            for (prior_node, prior_close) in &prev_closing {
                if !seen_nodes.contains_key(prior_node) && !extensive_equal(prior_close, &zero_qty)
                {
                    return Err(format!(
                        "FMINE-REF-101: node '{prior_node}' carried non-zero closing inventory in '{prev_id}' but was omitted from '{}' inventories",
                        rec.period_id
                    ));
                }
            }
        }

        let expected_witness = conservation_witness(&rec.period_id, &report)
            .map_err(|e| format!("FMINE-REF-101: witness derivation failed: {e}"))?;
        for w in &batch.witnesses {
            if w.kind == WitnessKind::Conservation
                && w.subject_id == op.key
                && w.digest != expected_witness.digest
            {
                return Err(format!(
                    "FMINE-REF-101: conservation witness digest mismatch on '{}' (expected {}, got {})",
                    op.key, expected_witness.digest, w.digest
                ));
            }
        }

        in_batch_records.insert(rec.period_id.clone(), rec);
    }

    Ok(())
}

/// Reusable commit-time conservation law closure for `L0Ledger::commit_with_conservation`,
/// `SemanticSegmentWriter::append_with_law`, and `SemanticSegmentWriter::commit_prepared_with_law`
/// (`BD-147`, `PO-BED-6`).
pub fn conservation_commit_law<'a>(
    graph: &'a FlowGraph,
    tolerance: &'a BalanceTolerance,
) -> impl Fn(&L0Ledger, &DeltaBatch) -> Result<(), String> + 'a {
    move |ledger, batch| check_batch_conservation(graph, tolerance, ledger, batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{FlowNode, FlowStream, NodeClass, StockpileMixingModel};

    fn qty(dry: i128, water: i128, au: i128) -> ExtensiveQuantities {
        let mut elems = BTreeMap::new();
        elems.insert("Au".to_string(), ExactQty::from_integer(au));
        ExtensiveQuantities::new(
            ExactQty::from_integer(dry),
            ExactQty::from_integer(water),
            elems,
        )
        .expect("qty")
    }

    #[test]
    fn test_exact_conservation_closure_and_refusal_no_plug() {
        let g = FlowGraph::build(
            vec![
                FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None).unwrap(),
                FlowNode::new(
                    "rom",
                    "ROM Pad",
                    NodeClass::StockpileOrPad,
                    true,
                    Some(StockpileMixingModel::WellMixed),
                )
                .unwrap(),
                FlowNode::new("mill", "Mill", NodeClass::PlantUnit, true, None).unwrap(),
                FlowNode::new("dore", "Dore", NodeClass::ProductMarket, false, None).unwrap(),
                FlowNode::new("tsf", "TSF", NodeClass::WasteAndTailings, false, None).unwrap(),
            ],
            vec![
                FlowStream::new("s1", "pit", "rom", true, ["Au"]).unwrap(),
                FlowStream::new("s2", "rom", "mill", true, ["Au"]).unwrap(),
                FlowStream::new("s3", "mill", "dore", false, ["Au"]).unwrap(),
                FlowStream::new("s4", "mill", "tsf", true, ["Au"]).unwrap(),
            ],
        )
        .expect("graph");

        // Exact closed balance:
        // ROM: in 1000 t / 2000 g Au, out 800 t / 1600 g Au, opening 500/1000 -> closing 700/1400 (Δ=+200/+400).
        // Mill: in 800 t / 1600 g Au, out dore 0 t / 1440 g Au + tsf 800 t / 160 g Au, Δ=0.
        let flows = vec![
            StreamFlow {
                stream_id: "s1".to_string(),
                quantities: qty(1000, 50, 2000),
            },
            StreamFlow {
                stream_id: "s2".to_string(),
                quantities: qty(800, 40, 1600),
            },
            StreamFlow {
                stream_id: "s3".to_string(),
                quantities: qty(0, 0, 1440),
            },
            StreamFlow {
                stream_id: "s4".to_string(),
                quantities: qty(800, 40, 160),
            },
        ];
        let invs = vec![InventoryDelta {
            node_id: "rom".to_string(),
            opening: qty(500, 25, 1000),
            closing: qty(700, 35, 1400),
        }];

        let report = verify_period_balance(&g, &flows, &invs, &BalanceTolerance::exact_zero())
            .expect("exact closed balance passes");
        assert_eq!(report.system_dry_mass_residual, ExactQty::from_integer(0));
        assert_eq!(
            report.system_element_residuals["Au"],
            ExactQty::from_integer(0)
        );
        assert_eq!(
            derive_grade(ExactQty::from_integer(1600), ExactQty::from_integer(800)).unwrap(),
            ExactQty::from_integer(2)
        );
        assert!(derive_grade(ExactQty::from_integer(10), ExactQty::from_integer(0)).is_err());

        // Perturb dore Au by +1 g -> refused with FMINE-REF-101 (never plugged!).
        let mut bad_flows = flows;
        bad_flows[2].quantities = qty(0, 0, 1441);
        let err = verify_period_balance(&g, &bad_flows, &invs, &BalanceTolerance::exact_zero())
            .unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-101");
    }

    #[test]
    fn test_stockpile_split_merge_metamorphic_conservation() {
        // Metamorphic law (§33.4, PO-BED-6): splitting one stockpile into two parallel bins
        // (high-grade + low-grade) that sum to the same inflows, outflows, and inventory deltas
        // yields identical whole-system extensive conservation residuals.
        let single = FlowGraph::build(
            vec![
                FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None).unwrap(),
                FlowNode::new(
                    "rom",
                    "Merged ROM",
                    NodeClass::StockpileOrPad,
                    true,
                    Some(StockpileMixingModel::Layered),
                )
                .unwrap(),
                FlowNode::new("mill", "Mill", NodeClass::PlantUnit, false, None).unwrap(),
            ],
            vec![
                FlowStream::new("in", "pit", "rom", false, ["Au"]).unwrap(),
                FlowStream::new("out", "rom", "mill", false, ["Au"]).unwrap(),
            ],
        )
        .unwrap();
        let split = FlowGraph::build(
            vec![
                FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None).unwrap(),
                FlowNode::new(
                    "rom_hg",
                    "HG Bin",
                    NodeClass::StockpileOrPad,
                    true,
                    Some(StockpileMixingModel::Layered),
                )
                .unwrap(),
                FlowNode::new(
                    "rom_lg",
                    "LG Bin",
                    NodeClass::StockpileOrPad,
                    true,
                    Some(StockpileMixingModel::Layered),
                )
                .unwrap(),
                FlowNode::new("mill", "Mill", NodeClass::PlantUnit, false, None).unwrap(),
            ],
            vec![
                FlowStream::new("in_hg", "pit", "rom_hg", false, ["Au"]).unwrap(),
                FlowStream::new("in_lg", "pit", "rom_lg", false, ["Au"]).unwrap(),
                FlowStream::new("out_hg", "rom_hg", "mill", false, ["Au"]).unwrap(),
                FlowStream::new("out_lg", "rom_lg", "mill", false, ["Au"]).unwrap(),
            ],
        )
        .unwrap();

        let tol = BalanceTolerance {
            dry_mass: ExactQty::from_integer(10),
            water_mass: ExactQty::from_integer(0),
            default_element: ExactQty::from_integer(20),
            per_element: BTreeMap::new(),
        };

        let rep_single = verify_period_balance(
            &single,
            &[
                StreamFlow {
                    stream_id: "in".to_string(),
                    quantities: qty(1000, 0, 2500),
                },
                StreamFlow {
                    stream_id: "out".to_string(),
                    quantities: qty(700, 0, 1800),
                },
            ],
            &[InventoryDelta {
                node_id: "rom".to_string(),
                opening: qty(400, 0, 900),
                closing: qty(695, 0, 1590), // Δ = +295 t, +690 g -> residual = +5 t, +10 g
            }],
            &tol,
        )
        .expect("merged balance");

        let rep_split = verify_period_balance(
            &split,
            &[
                StreamFlow {
                    stream_id: "in_hg".to_string(),
                    quantities: qty(400, 0, 1600),
                },
                StreamFlow {
                    stream_id: "in_lg".to_string(),
                    quantities: qty(600, 0, 900),
                },
                StreamFlow {
                    stream_id: "out_hg".to_string(),
                    quantities: qty(300, 0, 1200),
                },
                StreamFlow {
                    stream_id: "out_lg".to_string(),
                    quantities: qty(400, 0, 600),
                },
            ],
            &[
                InventoryDelta {
                    node_id: "rom_hg".to_string(),
                    opening: qty(100, 0, 400),
                    closing: qty(198, 0, 796), // Δ = +98 t, +396 g -> res = +2 t, +4 g
                },
                InventoryDelta {
                    node_id: "rom_lg".to_string(),
                    opening: qty(300, 0, 500),
                    closing: qty(497, 0, 794), // Δ = +197 t, +294 g -> res = +3 t, +6 g
                },
            ],
            &tol,
        )
        .expect("split balance");

        assert_eq!(
            rep_single.system_dry_mass_residual,
            rep_split.system_dry_mass_residual
        );
        assert_eq!(
            rep_single.system_element_residuals,
            rep_split.system_element_residuals
        );
    }

    #[test]
    fn test_balance_rejects_missing_streams_missing_stockpile_delta_negative_and_undeclared() {
        let g = FlowGraph::build(
            vec![
                FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None).unwrap(),
                FlowNode::new(
                    "rom",
                    "ROM",
                    NodeClass::StockpileOrPad,
                    true,
                    Some(StockpileMixingModel::WellMixed),
                )
                .unwrap(),
                FlowNode::new("mill", "Mill", NodeClass::PlantUnit, false, None).unwrap(),
            ],
            vec![
                FlowStream::new("s_in", "pit", "rom", false, ["Au"]).unwrap(),
                FlowStream::new("s_out", "rom", "mill", false, ["Au"]).unwrap(),
            ],
        )
        .unwrap();
        let inv = vec![InventoryDelta {
            node_id: "rom".to_string(),
            opening: qty(100, 0, 200),
            closing: qty(100, 0, 200),
        }];
        let flows = vec![
            StreamFlow {
                stream_id: "s_in".to_string(),
                quantities: qty(500, 0, 1000),
            },
            StreamFlow {
                stream_id: "s_out".to_string(),
                quantities: qty(500, 0, 1000),
            },
        ];

        // 1. Missing outgoing stream flow must refuse (never silently treated as zero).
        assert!(
            verify_period_balance(&g, &flows[..1], &inv, &BalanceTolerance::exact_zero()).is_err()
        );

        // 2. Missing InventoryDelta on a StockpileOrPad node must refuse (never silently Δ = 0).
        assert!(verify_period_balance(&g, &flows, &[], &BalanceTolerance::exact_zero()).is_err());

        // 3. Positive water_mass on carries_water = false stream must refuse.
        let mut wet_flows = flows.clone();
        wet_flows[0].quantities = qty(500, 10, 1000);
        assert!(
            verify_period_balance(&g, &wet_flows, &inv, &BalanceTolerance::exact_zero()).is_err()
        );

        // 4. Undeclared element ("Cu" on ["Au"] stream) must refuse.
        let mut extra_el = flows.clone();
        extra_el[0]
            .quantities
            .elements
            .insert("Cu".to_string(), ExactQty::from_integer(5));
        assert!(
            verify_period_balance(&g, &extra_el, &inv, &BalanceTolerance::exact_zero()).is_err()
        );

        // 5. Directly-constructed negative dry_mass must refuse.
        let mut neg_flows = flows;
        neg_flows[0].quantities.dry_mass = ExactQty::from_integer(-1);
        assert!(
            verify_period_balance(&g, &neg_flows, &inv, &BalanceTolerance::exact_zero()).is_err()
        );
    }

    #[test]
    fn test_period_balance_codec_canonical_and_negative_tolerance_refusal() {
        // 1. Canonical FLWB wire roundtrip is order-independent on input Vecs.
        let rec_a = PeriodBalanceRecord {
            period_id: "2026-01".to_string(),
            previous_period_id: None,
            flows: vec![
                StreamFlow {
                    stream_id: "s2".into(),
                    quantities: qty(800, 40, 1600),
                },
                StreamFlow {
                    stream_id: "s1".into(),
                    quantities: qty(1000, 50, 2000),
                },
            ],
            inventories: vec![InventoryDelta {
                node_id: "rom".into(),
                opening: qty(500, 25, 1000),
                closing: qty(700, 35, 1400),
            }],
        };
        let rec_b = PeriodBalanceRecord {
            flows: vec![rec_a.flows[1].clone(), rec_a.flows[0].clone()],
            ..rec_a.clone()
        };
        let bytes_a = rec_a.to_bytes().expect("encode a");
        let bytes_b = rec_b.to_bytes().expect("encode b");
        assert_eq!(bytes_a, bytes_b, "FLWB encoding must be canonical");
        let decoded = PeriodBalanceRecord::from_bytes(&bytes_a).expect("decode");
        assert_eq!(decoded.flows[0].stream_id, "s1");
        assert_eq!(decoded.flows[1].stream_id, "s2");

        // 2. Non-canonical unreduced ExactQty (2/4) in wire bytes is refused.
        let mut enc = Encoder::new_frame(TAG_FLOW_BALANCE, FMT_FLWB_V1);
        enc.put_str("2026-01");
        enc.put_u8(0);
        enc.put_u32(1);
        enc.put_str("s1");
        enc.put_i128(2);
        enc.put_i128(4); // unreduced!
        enc.put_i128(0);
        enc.put_i128(1);
        enc.put_u32(0);
        enc.put_u32(0);
        assert!(PeriodBalanceRecord::from_bytes(&enc.finish()).is_err());

        // 3. Self-referential previous_period_id is refused.
        let self_prev = PeriodBalanceRecord {
            previous_period_id: Some("2026-01".to_string()),
            ..rec_a
        };
        assert!(self_prev.to_bytes().is_err());

        // 4. Negative tolerance is refused even on a graph with zero interior nodes.
        let two_node = FlowGraph::build(
            vec![
                FlowNode::new("pit", "Pit", NodeClass::BlastedAndDug, false, None).unwrap(),
                FlowNode::new("mill", "Mill", NodeClass::PlantUnit, false, None).unwrap(),
            ],
            vec![FlowStream::new("s1", "pit", "mill", false, ["Au"]).unwrap()],
        )
        .unwrap();
        let mut neg_tol = BalanceTolerance::exact_zero();
        neg_tol.dry_mass = ExactQty::from_integer(-1);
        assert!(verify_period_balance(&two_node, &[], &[], &neg_tol).is_err());
    }
}
