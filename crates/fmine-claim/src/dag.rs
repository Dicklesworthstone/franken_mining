//! DESIGN (BD-344, plan §6.3): immutable temporal ancestry metadata, checked at an availability cut
//! and a vector of ledger sequence marks. This checks supplied evidence; it does not authenticate clocks,
//! prove parent completeness, or replace the separate intercepted-read replay.
//!
//! Traversals are iterative. Hash maps are lookup tables only; ordered node/parent vectors determine every
//! traversal and diagnostic. Ancestry work is linear apart from canonical sorting and returned path lengths.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

use fmine_core::{Encoder, blake3_hash, hash::Digest32, id::ObjectId};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_time::{Anchor, RecordTime, TimeEvidence};

use crate::Claim;

pub mod replay;

pub const RULES_VERSION: &str = "temporal-cone/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeKind {
    Observation = 0,
    AttestedInterpretation = 1,
    FittedParameters = 2,
    CorrelationCard = 3,
    ExclusionAct = 4,
    PriceDeck = 5,
    RegistryRow = 6,
    ReferencePrior = 7,
    Claim = 8,
    Forecast = 9,
    Decision = 10,
    Recipe = 11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// A source has no declared inputs. Missing availability remains unknown and refuses on use.
    Source { available_at: Option<i64> },
    /// Computed availability is max(input availability, performed_at). A later declaration can delay
    /// release; an earlier declaration is an error, never an override of the computed time.
    Derived {
        performed_at: Option<i64>,
        declared_available_at: Option<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalNode {
    pub id: ObjectId,
    pub kind: NodeKind,
    pub parents: Vec<ObjectId>,
    pub availability: Availability,
    pub recorded_at: Option<RecordTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeCut {
    pub decision_ns: i64,
    pub anchor: Anchor,
}

/// A successful check is constructible only by the immutable graph. The closure includes its root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCone {
    root: ObjectId,
    cut: ConeCut,
    nodes: Vec<ObjectId>,
    availability: BTreeMap<ObjectId, i64>,
    anchor_marks: BTreeMap<String, u64>,
    digest: Digest32,
}

impl CheckedCone {
    #[must_use]
    pub const fn root(&self) -> ObjectId {
        self.root
    }
    #[must_use]
    pub fn cut(&self) -> &ConeCut {
        &self.cut
    }
    #[must_use]
    pub fn nodes(&self) -> &[ObjectId] {
        &self.nodes
    }
    #[must_use]
    pub fn available_at(&self, id: &ObjectId) -> Option<i64> {
        self.availability.get(id).copied()
    }
    #[must_use]
    pub fn anchor_marks(&self) -> &BTreeMap<String, u64> {
        &self.anchor_marks
    }
    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierViolation {
    pub node: ObjectId,
    pub kind: NodeKind,
    pub available_at: i64,
    pub recorded_at: RecordTime,
    pub after_cut: bool,
    pub after_anchor: bool,
    /// One shortest root-to-node path; equal-length alternatives break ties by ObjectId order.
    pub path: Vec<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConeViolation {
    root: ObjectId,
    cut: ConeCut,
    nodes: Vec<ObjectId>,
    frontier: Vec<FrontierViolation>,
    digest: Digest32,
}

impl ConeViolation {
    #[must_use]
    pub const fn root(&self) -> ObjectId {
        self.root
    }
    #[must_use]
    pub fn cut(&self) -> &ConeCut {
        &self.cut
    }
    #[must_use]
    pub fn nodes(&self) -> &[ObjectId] {
        &self.nodes
    }
    #[must_use]
    pub fn frontier(&self) -> &[FrontierViolation] {
        &self.frontier
    }
    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeField {
    AvailableAt,
    PerformedAt,
    RecordedAt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConeError {
    DuplicateNode {
        node: ObjectId,
    },
    MissingNode {
        root: ObjectId,
        node: ObjectId,
        path: Vec<ObjectId>,
    },
    MissingTime {
        root: ObjectId,
        node: ObjectId,
        field: TimeField,
        path: Vec<ObjectId>,
    },
    SourceHasParents {
        root: ObjectId,
        node: ObjectId,
        path: Vec<ObjectId>,
    },
    BackdatedAvailability {
        root: ObjectId,
        node: ObjectId,
        declared: i64,
        computed: i64,
        path: Vec<ObjectId>,
    },
    MissingAnchorCell {
        root: ObjectId,
        node: ObjectId,
        cell: String,
        seq: u64,
        path: Vec<ObjectId>,
    },
    InvalidRecordCell {
        root: ObjectId,
        node: ObjectId,
        path: Vec<ObjectId>,
    },
    Cycle {
        root: ObjectId,
        cycle: Vec<ObjectId>,
    },
    ClaimBinding {
        node: ObjectId,
        detail: &'static str,
    },
    Violation(Box<ConeViolation>),
}

impl ConeError {
    #[must_use]
    pub fn refusal(&self) -> Refusal {
        Refusal::new(RefusalCode::TemporalConeViolation110, self.to_string())
    }
}

fn show_path(path: &[ObjectId]) -> String {
    path.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" -> ")
}

impl fmt::Display for ConeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode { node } => write!(f, "duplicate temporal node {node}"),
            Self::MissingNode { root, node, path } => write!(
                f,
                "temporal root {root}: missing {node}; path {}",
                show_path(path)
            ),
            Self::MissingTime {
                root,
                node,
                field,
                path,
            } => write!(
                f,
                "temporal root {root}: {node} has no {field:?}; path {}",
                show_path(path)
            ),
            Self::SourceHasParents { root, node, path } => write!(
                f,
                "temporal root {root}: source {node} declares parents without a performed act; path {}",
                show_path(path)
            ),
            Self::BackdatedAvailability {
                root,
                node,
                declared,
                computed,
                path,
            } => write!(
                f,
                "temporal root {root}: {node} declares availability {declared} before computed {computed}; path {}",
                show_path(path)
            ),
            Self::MissingAnchorCell {
                root,
                node,
                cell,
                seq,
                path,
            } => write!(
                f,
                "temporal root {root}: {node} record {cell}:{seq} has no anchor mark; path {}",
                show_path(path)
            ),
            Self::InvalidRecordCell { root, node, path } => write!(
                f,
                "temporal root {root}: {node} has an empty record cell; path {}",
                show_path(path)
            ),
            Self::Cycle { root, cycle } => write!(
                f,
                "temporal root {root}: ancestor cycle {}",
                show_path(cycle)
            ),
            Self::ClaimBinding { node, detail } => write!(f, "temporal Claim {node}: {detail}"),
            Self::Violation(v) => {
                write!(f, "temporal root {} at {}: ", v.root, v.cut.decision_ns)?;
                for (i, item) in v.frontier.iter().enumerate() {
                    if i != 0 {
                        f.write_str("; ")?;
                    }
                    write!(
                        f,
                        "{} {:?} available {} record {}:{} after_cut={} after_anchor={} path {}",
                        item.node,
                        item.kind,
                        item.available_at,
                        item.recorded_at.cell(),
                        item.recorded_at.seq(),
                        item.after_cut,
                        item.after_anchor,
                        show_path(&item.path)
                    )?;
                }
                Ok(())
            }
        }
    }
}
impl std::error::Error for ConeError {}

/// Node metadata is copied, normalized and sealed on construction. Unrelated malformed or future nodes
/// are permitted in a store; checking a root inspects only that root's ancestry.
#[derive(Debug, Clone)]
pub struct TemporalGraph {
    nodes: Vec<TemporalNode>,
    index: HashMap<ObjectId, usize>,
}

impl TemporalGraph {
    pub fn new(nodes: impl IntoIterator<Item = TemporalNode>) -> Result<Self, ConeError> {
        let mut nodes: Vec<_> = nodes.into_iter().collect();
        nodes.sort_by_key(|n| n.id);
        for pair in nodes.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(ConeError::DuplicateNode { node: pair[0].id });
            }
        }
        for node in &mut nodes {
            node.parents.sort();
            node.parents.dedup();
        }
        let index = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
        Ok(Self { nodes, index })
    }

    #[must_use]
    pub fn node(&self, id: &ObjectId) -> Option<&TemporalNode> {
        self.index.get(id).and_then(|i| self.nodes.get(*i))
    }

    pub fn nodes(&self) -> impl Iterator<Item = &TemporalNode> {
        self.nodes.iter()
    }

    /// Bind a graph's Claim node to the actual immutable Claim's identity and immediate parent set.
    pub fn verify_claim<T>(&self, claim: &Claim<T>) -> Result<(), ConeError> {
        let id = claim.claim_id;
        claim.check_id().map_err(|_| ConeError::ClaimBinding {
            node: id,
            detail: "Claim id differs from its current canonical body",
        })?;
        let node = self.node(&id).ok_or_else(|| ConeError::MissingNode {
            root: id,
            node: id,
            path: vec![id],
        })?;
        let mut parents = claim.parents.clone();
        parents.sort();
        parents.dedup();
        if node.kind != NodeKind::Claim || node.parents != parents {
            return Err(ConeError::ClaimBinding {
                node: id,
                detail: "node kind or parents differ from the actual Claim",
            });
        }
        Ok(())
    }

    /// Check the complete root-inclusive cone. Unavailable nodes are reduced to the frontier whose
    /// own ancestors are admissible; breadth-first predecessors give shortest deterministic paths.
    pub fn check(&self, root: ObjectId, cut: &ConeCut) -> Result<CheckedCone, ConeError> {
        let mut previous = HashMap::from([(root, None)]);
        let mut queue = VecDeque::from([root]);
        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            let node = self.node(&id).ok_or_else(|| missing(root, id, &previous))?;
            order.push(id);
            for parent in &node.parents {
                if let std::collections::hash_map::Entry::Vacant(slot) = previous.entry(*parent) {
                    slot.insert(Some(id));
                    queue.push_back(*parent);
                }
            }
        }

        let mut work = HashMap::new();
        let mut children: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        for id in &order {
            let node = self.node(id).ok_or_else(|| missing(root, *id, &previous))?;
            if node.parents.is_empty() {
                queue.push_back(*id);
            }
            work.insert(
                *id,
                Work {
                    remaining: node.parents.len(),
                    max_input: None,
                    ancestor_violates: false,
                },
            );
            for parent in &node.parents {
                children.entry(*parent).or_default().push(*id);
            }
        }
        let mut availability = BTreeMap::new();
        let mut anchor_marks = BTreeMap::new();
        let mut frontier = Vec::new();
        while let Some(id) = queue.pop_front() {
            let node = self.node(&id).ok_or_else(|| missing(root, id, &previous))?;
            let state = work.get(&id).ok_or_else(|| missing(root, id, &previous))?;
            let no_time = |field| ConeError::MissingTime {
                root,
                node: id,
                field,
                path: path_to(id, &previous),
            };
            let at = match node.availability {
                Availability::Source { available_at } => {
                    if !node.parents.is_empty() {
                        return Err(ConeError::SourceHasParents {
                            root,
                            node: id,
                            path: path_to(id, &previous),
                        });
                    }
                    available_at.ok_or_else(|| no_time(TimeField::AvailableAt))?
                }
                Availability::Derived {
                    performed_at,
                    declared_available_at,
                } => {
                    let performed = performed_at.ok_or_else(|| no_time(TimeField::PerformedAt))?;
                    let computed = state
                        .max_input
                        .map_or(performed, |at: i64| at.max(performed));
                    if let Some(declared) = declared_available_at
                        && declared < computed
                    {
                        return Err(ConeError::BackdatedAvailability {
                            root,
                            node: id,
                            declared,
                            computed,
                            path: path_to(id, &previous),
                        });
                    }
                    declared_available_at.unwrap_or(computed)
                }
            };
            let record = node
                .recorded_at
                .as_ref()
                .ok_or_else(|| no_time(TimeField::RecordedAt))?;
            if record.cell().trim().is_empty() {
                return Err(ConeError::InvalidRecordCell {
                    root,
                    node: id,
                    path: path_to(id, &previous),
                });
            }
            let mark =
                cut.anchor
                    .mark(record.cell())
                    .ok_or_else(|| ConeError::MissingAnchorCell {
                        root,
                        node: id,
                        cell: record.cell().to_string(),
                        seq: record.seq(),
                        path: path_to(id, &previous),
                    })?;
            anchor_marks.insert(record.cell().to_string(), mark);
            let (after_cut, after_anchor) = (at > cut.decision_ns, record.seq() > mark);
            let violates = after_cut || after_anchor;
            let ancestor_violates = state.ancestor_violates;
            if violates && !ancestor_violates {
                frontier.push(FrontierViolation {
                    node: id,
                    kind: node.kind,
                    available_at: at,
                    recorded_at: record.clone(),
                    after_cut,
                    after_anchor,
                    path: path_to(id, &previous),
                });
            }
            availability.insert(id, at);
            for child in children.get(&id).into_iter().flatten() {
                let child_state = work
                    .get_mut(child)
                    .ok_or_else(|| missing(root, *child, &previous))?;
                child_state.remaining -= 1;
                child_state.max_input = Some(child_state.max_input.map_or(at, |old| old.max(at)));
                child_state.ancestor_violates |= violates || ancestor_violates;
                if child_state.remaining == 0 {
                    queue.push_back(*child);
                }
            }
        }
        if availability.len() != order.len() {
            return Err(ConeError::Cycle {
                root,
                cycle: self.cycle(&order, &work),
            });
        }
        let nodes: Vec<_> = availability.keys().copied().collect();
        let digest = self.cone_digest(root, cut, &availability, &anchor_marks)?;
        if !frontier.is_empty() {
            frontier.sort_by_key(|v| v.node);
            return Err(ConeError::Violation(Box::new(ConeViolation {
                root,
                cut: cut.clone(),
                nodes,
                frontier,
                digest,
            })));
        }
        Ok(CheckedCone {
            root,
            cut: cut.clone(),
            nodes,
            availability,
            anchor_marks,
            digest,
        })
    }

    fn cycle(&self, order: &[ObjectId], work: &HashMap<ObjectId, Work>) -> Vec<ObjectId> {
        let mut cursor = order
            .iter()
            .filter(|id| work.get(id).is_some_and(|w| w.remaining > 0))
            .min()
            .copied();
        let mut seen = HashMap::new();
        let mut path = Vec::new();
        while let Some(id) = cursor {
            if let Some(start) = seen.get(&id).copied() {
                let mut cycle = path[start..].to_vec();
                cycle.push(id);
                return cycle;
            }
            seen.insert(id, path.len());
            path.push(id);
            cursor = self
                .node(&id)
                .and_then(|n| {
                    n.parents
                        .iter()
                        .find(|p| work.get(p).is_some_and(|w| w.remaining > 0))
                })
                .copied();
        }
        path
    }

    fn cone_digest(
        &self,
        root: ObjectId,
        cut: &ConeCut,
        availability: &BTreeMap<ObjectId, i64>,
        marks: &BTreeMap<String, u64>,
    ) -> Result<Digest32, ConeError> {
        let mut e = Encoder::new_frame(*b"TCNE", 1);
        e.put_str(RULES_VERSION);
        e.put_str(&root.to_string());
        e.put_i64(cut.decision_ns);
        e.put_u64(marks.len() as u64);
        for (cell, mark) in marks {
            e.put_str(cell);
            e.put_u64(*mark);
        }
        e.put_u64(availability.len() as u64);
        for (id, effective) in availability {
            let node = self.node(id).ok_or_else(|| ConeError::MissingNode {
                root,
                node: *id,
                path: Vec::new(),
            })?;
            e.put_str(&id.to_string());
            e.put_u8(node.kind as u8);
            e.put_u64(node.parents.len() as u64);
            for parent in &node.parents {
                e.put_str(&parent.to_string());
            }
            match node.availability {
                Availability::Source { available_at } => {
                    e.put_u8(0);
                    put_time(&mut e, available_at);
                }
                Availability::Derived {
                    performed_at,
                    declared_available_at,
                } => {
                    e.put_u8(1);
                    put_time(&mut e, performed_at);
                    put_time(&mut e, declared_available_at);
                }
            }
            e.put_i64(*effective);
            let record = node
                .recorded_at
                .as_ref()
                .ok_or_else(|| ConeError::MissingTime {
                    root,
                    node: *id,
                    field: TimeField::RecordedAt,
                    path: Vec::new(),
                })?;
            e.put_str(record.cell());
            e.put_u64(record.seq());
            e.put_i64(record.wall_ns());
            match record.evidence() {
                TimeEvidence::HostSynced => e.put_u8(0),
                TimeEvidence::NtpBounded { max_skew_ms } => {
                    e.put_u8(1);
                    e.put_u32(max_skew_ms);
                }
                TimeEvidence::OfflineUncertain => e.put_u8(2),
            }
        }
        Ok(blake3_hash(&e.finish()))
    }
}

struct Work {
    remaining: usize,
    max_input: Option<i64>,
    ancestor_violates: bool,
}

fn path_to(node: ObjectId, previous: &HashMap<ObjectId, Option<ObjectId>>) -> Vec<ObjectId> {
    let mut path = vec![node];
    let mut cursor = node;
    while let Some(Some(parent)) = previous.get(&cursor) {
        path.push(*parent);
        cursor = *parent;
    }
    path.reverse();
    path
}

fn missing(
    root: ObjectId,
    node: ObjectId,
    previous: &HashMap<ObjectId, Option<ObjectId>>,
) -> ConeError {
    ConeError::MissingNode {
        root,
        node,
        path: path_to(node, previous),
    }
}

fn put_time(e: &mut Encoder, value: Option<i64>) {
    e.put_u8(u8::from(value.is_some()));
    if let Some(at) = value {
        e.put_i64(at);
    }
}

#[cfg(test)]
mod tests;
