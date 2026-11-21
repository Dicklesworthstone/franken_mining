//! Determinism: BitwisePortable
//! Exact extensive conservation checker (`BD-146`, plan §20.2, `INV-005`, `PO-BED-6`).
//!
//! All extensive balances (`dry_tonnes`, `water_tonnes`, per-element metal) are checked strictly
//! in exact `fmine_units::ExactQty` rationals — zero bare `f64` parameters (L-6) and zero plugs.
//! Intensive grades (`metal ÷ dry_tonnes`) are derived strictly after extensive balance checks.

use std::collections::BTreeMap;

use fmine_units::ExactQty;

use crate::FlowError;
use crate::graph::FlowGraph;

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

/// Verify exact extensive conservation (`Σ in − Σ out − Δ inventory = 0` within `tolerance`)
/// at every interior node of `graph` (`INV-005`, `PO-BED-6`). Refuses violations with
/// `FMINE-REF-101` — never plugs or adjusts observations.
pub fn verify_period_balance(
    graph: &FlowGraph,
    flows: &[StreamFlow],
    inventories: &[InventoryDelta],
    tolerance: &BalanceTolerance,
) -> Result<BalanceReport, FlowError> {
    let mut flow_map: BTreeMap<&str, &ExtensiveQuantities> = BTreeMap::new();
    for f in flows {
        if graph.stream(&f.stream_id).is_none() {
            return Err(FlowError::UnknownStream {
                stream_id: f.stream_id.clone(),
            });
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
            if let Some(q) = flow_map.get(s.id.as_str()) {
                total_in = total_in.checked_add(q)?;
            }
        }
        let mut total_out = ExtensiveQuantities::zero();
        for s in graph.outgoing_streams(&node.id) {
            if let Some(q) = flow_map.get(s.id.as_str()) {
                total_out = total_out.checked_add(q)?;
            }
        }
        let delta_inv = if let Some(inv) = inv_map.get(node.id.as_str()) {
            inv.closing.checked_sub(&inv.opening)?
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
}
