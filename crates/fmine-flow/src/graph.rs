//! Determinism: BitwisePortable
//! Directed mine-to-market material-flow graph (`BD-146`, plan §20.1, `G-TOUCH-3`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::FlowError;

/// The eight node classes of a mine-to-market material-flow graph (plan §20.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeClass {
    /// Resource-model and GC-model projections over as-mined solids.
    InSituVolume,
    /// Blast polygons shifted for movement and dig blocks.
    BlastedAndDug,
    /// Truck / haulage loads by destination.
    Transport,
    /// ROM pads, grade-binned stockpiles, long-term low-grade, and leach pads (decouple time).
    StockpileOrPad,
    /// Crusher, mill, flotation, CIL, refinery, and internal plant circuits.
    PlantUnit,
    /// Shipments, refinery outturn, and smelter settlement.
    ProductMarket,
    /// Cross-complex ore or third-party toll/purchased feed (back-allocation only; E157).
    CrossComplexThirdParty,
    /// Waste dumps (PAG/NAF) and tailings storage facilities (TSF).
    WasteAndTailings,
}

/// Stockpile mixing model declared on inventory-holding pads/stockpiles (plan §20.3 R2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StockpileMixingModel {
    /// Homogeneous stirred-tank approximation.
    WellMixed,
    /// First-in, first-out plug flow.
    Fifo,
    /// Stratified lift / finger-reclaim model.
    Layered,
}

/// One node in a [`FlowGraph`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowNode {
    pub id: String,
    pub name: String,
    pub class: NodeClass,
    /// True if this node carries opening/closing inventory between periods (`Δ inventory != 0`).
    pub holds_inventory: bool,
    /// Required for `StockpileOrPad`; disallowed when `holds_inventory == false`.
    pub mixing_model: Option<StockpileMixingModel>,
}

impl FlowNode {
    /// Construct a validated node declaration.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        class: NodeClass,
        holds_inventory: bool,
        mixing_model: Option<StockpileMixingModel>,
    ) -> Result<Self, FlowError> {
        let id = id.into();
        let name = name.into();
        if id.is_empty() || id.trim() != id {
            return Err(FlowError::InvalidGraph {
                detail: format!("node id {id:?} must be non-empty and trimmed"),
            });
        }
        if name.is_empty() || name.trim() != name {
            return Err(FlowError::InvalidGraph {
                detail: format!("node '{id}' name must be non-empty and trimmed"),
            });
        }
        if class == NodeClass::StockpileOrPad && (!holds_inventory || mixing_model.is_none()) {
            return Err(FlowError::InvalidGraph {
                detail: format!(
                    "stockpile/pad node '{id}' must set holds_inventory=true and a StockpileMixingModel"
                ),
            });
        }
        if !holds_inventory && mixing_model.is_some() {
            return Err(FlowError::InvalidGraph {
                detail: format!("non-inventory node '{id}' cannot declare a StockpileMixingModel"),
            });
        }
        Ok(Self {
            id,
            name,
            class,
            holds_inventory,
            mixing_model,
        })
    }
}

/// One directed transfer stream between two distinct nodes in a [`FlowGraph`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowStream {
    pub id: String,
    pub from: String,
    pub to: String,
    /// Whether water/moisture is tracked explicitly on this stream.
    pub carries_water: bool,
    /// Ordered set of extensive elemental metal channels carried on this stream (e.g. `Au`, `Cu`).
    pub elements: BTreeSet<String>,
}

impl FlowStream {
    /// Construct a validated directed stream declaration.
    pub fn new(
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        carries_water: bool,
        elements: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, FlowError> {
        let id = id.into();
        let from = from.into();
        let to = to.into();
        if id.is_empty() || id.trim() != id {
            return Err(FlowError::InvalidGraph {
                detail: format!("stream id {id:?} must be non-empty and trimmed"),
            });
        }
        if from.is_empty() || to.is_empty() {
            return Err(FlowError::InvalidGraph {
                detail: format!("stream '{id}' endpoints must be non-empty"),
            });
        }
        if from == to {
            return Err(FlowError::InvalidGraph {
                detail: format!("stream '{id}' cannot be a self-loop on node '{from}'"),
            });
        }
        let mut elems = BTreeSet::new();
        for el in elements {
            let s = el.into();
            if s.is_empty() || s.trim() != s {
                return Err(FlowError::InvalidGraph {
                    detail: format!("stream '{id}' has empty or untrimmed element name {s:?}"),
                });
            }
            elems.insert(s);
        }
        Ok(Self {
            id,
            from,
            to,
            carries_water,
            elements: elems,
        })
    }
}

/// Validated directed material-flow graph (`BD-146`, plan §20.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowGraph {
    nodes: BTreeMap<String, FlowNode>,
    streams: BTreeMap<String, FlowStream>,
}

impl FlowGraph {
    /// Build a validated `FlowGraph` from nodes and directed streams.
    pub fn build(
        nodes: impl IntoIterator<Item = FlowNode>,
        streams: impl IntoIterator<Item = FlowStream>,
    ) -> Result<Self, FlowError> {
        let mut g = Self::default();
        for node in nodes {
            if g.nodes.contains_key(&node.id) {
                return Err(FlowError::DuplicateId {
                    kind: "node",
                    id: node.id,
                });
            }
            g.nodes.insert(node.id.clone(), node);
        }
        for stream in streams {
            if g.streams.contains_key(&stream.id) {
                return Err(FlowError::DuplicateId {
                    kind: "stream",
                    id: stream.id,
                });
            }
            if !g.nodes.contains_key(&stream.from) {
                return Err(FlowError::UnknownNode {
                    node_id: stream.from,
                });
            }
            if !g.nodes.contains_key(&stream.to) {
                return Err(FlowError::UnknownNode { node_id: stream.to });
            }
            g.streams.insert(stream.id.clone(), stream);
        }
        if g.nodes.is_empty() {
            return Err(FlowError::InvalidGraph {
                detail: "flow graph must contain at least one node".to_string(),
            });
        }
        Ok(g)
    }

    /// All nodes in deterministic ID order.
    #[must_use]
    pub fn nodes(&self) -> &BTreeMap<String, FlowNode> {
        &self.nodes
    }

    /// All streams in deterministic ID order.
    #[must_use]
    pub fn streams(&self) -> &BTreeMap<String, FlowStream> {
        &self.streams
    }

    /// Look up one node by ID.
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&FlowNode> {
        self.nodes.get(id)
    }

    /// Look up one stream by ID.
    #[must_use]
    pub fn stream(&self, id: &str) -> Option<&FlowStream> {
        self.streams.get(id)
    }

    /// Incoming stream IDs for `node_id` in deterministic order.
    #[must_use]
    pub fn incoming_streams(&self, node_id: &str) -> Vec<&FlowStream> {
        self.streams.values().filter(|s| s.to == node_id).collect()
    }

    /// Outgoing stream IDs for `node_id` in deterministic order.
    #[must_use]
    pub fn outgoing_streams(&self, node_id: &str) -> Vec<&FlowStream> {
        self.streams
            .values()
            .filter(|s| s.from == node_id)
            .collect()
    }

    /// Nodes that have both incoming and outgoing streams (interior conservation nodes).
    #[must_use]
    pub fn interior_nodes(&self) -> Vec<&FlowNode> {
        self.nodes
            .values()
            .filter(|n| {
                !self.incoming_streams(&n.id).is_empty() && !self.outgoing_streams(&n.id).is_empty()
            })
            .collect()
    }

    /// All `StockpileOrPad` nodes lying on any directed path from `source_node` to `target_node`.
    pub fn intervening_stockpiles(
        &self,
        source_node: &str,
        target_node: &str,
    ) -> Result<Vec<String>, FlowError> {
        if !self.nodes.contains_key(source_node) {
            return Err(FlowError::UnknownNode {
                node_id: source_node.to_string(),
            });
        }
        if !self.nodes.contains_key(target_node) {
            return Err(FlowError::UnknownNode {
                node_id: target_node.to_string(),
            });
        }
        // Nodes reachable forward from source_node.
        let mut forward = BTreeSet::new();
        let mut q = VecDeque::new();
        q.push_back(source_node.to_string());
        forward.insert(source_node.to_string());
        while let Some(u) = q.pop_front() {
            for s in self.outgoing_streams(&u) {
                if forward.insert(s.to.clone()) {
                    q.push_back(s.to.clone());
                }
            }
        }
        // Nodes that can reach target_node (backward BFS).
        let mut backward = BTreeSet::new();
        q.push_back(target_node.to_string());
        backward.insert(target_node.to_string());
        while let Some(v) = q.pop_front() {
            for s in self.incoming_streams(&v) {
                if backward.insert(s.from.clone()) {
                    q.push_back(s.from.clone());
                }
            }
        }
        let mut stockpiles = Vec::new();
        for id in forward.intersection(&backward) {
            if id != source_node
                && id != target_node
                && let Some(node) = self.nodes.get(id)
                && (node.class == NodeClass::StockpileOrPad || node.holds_inventory)
            {
                stockpiles.push(id.clone());
            }
        }
        Ok(stockpiles)
    }

    /// Guard `G-TOUCH-3` (plan §20.1): refuses direct period-to-period ratio comparison
    /// ("mined this month ÷ milled this month") whenever any stockpile or inventory buffer decouples
    /// `source_node` from `target_node`.
    pub fn check_direct_ratio_admissible(
        &self,
        source_node: &str,
        target_node: &str,
    ) -> Result<(), FlowError> {
        let stockpiles = self.intervening_stockpiles(source_node, target_node)?;
        if !stockpiles.is_empty() {
            return Err(FlowError::StockpileDecouplingViolation {
                source_node: source_node.to_string(),
                target_node: target_node.to_string(),
                intervening_stockpiles: stockpiles,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_graph_invariants_and_g_touch_3_decoupling() {
        let nodes = vec![
            FlowNode::new(
                "pit",
                "Open Pit Ex-Pit",
                NodeClass::BlastedAndDug,
                false,
                None,
            )
            .expect("pit"),
            FlowNode::new(
                "rom",
                "ROM Stockpile",
                NodeClass::StockpileOrPad,
                true,
                Some(StockpileMixingModel::Fifo),
            )
            .expect("rom"),
            FlowNode::new("mill", "SAG/Ball Mill", NodeClass::PlantUnit, true, None).expect("mill"),
            FlowNode::new(
                "dore",
                "Refinery Dore",
                NodeClass::ProductMarket,
                false,
                None,
            )
            .expect("dore"),
            FlowNode::new(
                "tsf",
                "Tailings TSF",
                NodeClass::WasteAndTailings,
                false,
                None,
            )
            .expect("tsf"),
        ];
        let streams = vec![
            FlowStream::new("pit_to_rom", "pit", "rom", true, ["Au"]).expect("s1"),
            FlowStream::new("rom_to_mill", "rom", "mill", true, ["Au"]).expect("s2"),
            FlowStream::new("mill_to_dore", "mill", "dore", false, ["Au"]).expect("s3"),
            FlowStream::new("mill_to_tsf", "mill", "tsf", true, ["Au"]).expect("s4"),
        ];
        let g = FlowGraph::build(nodes, streams).expect("build graph");
        assert_eq!(g.nodes().len(), 5);
        assert_eq!(g.streams().len(), 4);
        assert_eq!(g.interior_nodes().len(), 2); // rom and mill

        // G-TOUCH-3: pit -> mill passes through ROM stockpile -> direct ratio refused!
        let err = g.check_direct_ratio_admissible("pit", "mill").unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-102");
        assert!(matches!(
            err,
            FlowError::StockpileDecouplingViolation {
                ref intervening_stockpiles,
                ..
            } if intervening_stockpiles == &vec!["rom".to_string()]
        ));

        // Direct adjacent stream with no intervening inventory buffer (mill -> dore) is admissible.
        g.check_direct_ratio_admissible("mill", "dore")
            .expect("adjacent mill->dore");

        // Invalid graph checks: self-loop, missing mixing model on stockpile, dangling endpoint.
        assert!(FlowStream::new("bad", "pit", "pit", false, ["Au"]).is_err());
        assert!(
            FlowNode::new("bad_rom", "Bad ROM", NodeClass::StockpileOrPad, true, None).is_err()
        );
        assert!(
            FlowGraph::build(
                vec![FlowNode::new("a", "A", NodeClass::PlantUnit, false, None).unwrap()],
                vec![FlowStream::new("s", "a", "missing", false, ["Au"]).unwrap()]
            )
            .is_err()
        );
    }
}
