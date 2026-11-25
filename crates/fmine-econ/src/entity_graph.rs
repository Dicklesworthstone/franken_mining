//! Determinism: BitwisePortable
//! Helm K1 `CapitalEntityGraph`: Barrick Parent vs. `NA-NewCo` IPO carve-out dual denominator,
//! NGM 61.5%/38.5% & Pueblo Viejo 60%/40% capital call/dilution, Fourmile vend-in vs. tolling
//! option, Kibali repatriation gates, Twiga 50/50 economic benefits sharing, Reko Diq DSCR
//! ring-fence & free-carry recovery, plus Loulo-Gounkoto H1 2026 & Tongon divestiture fixtures
//! (`BD-314`, plan §23.2–§23.4).
//!
//! All physical, monetary, and ratio quantities use exact `fmine_units::Money` (`10^-4` major
//! units) and `fmine_units::ExactQty` (`i128` reduced rationals) with zero bare `f64` parameters
//! (L-6). No affiliation is implied with any issuer named in public-filing portfolio fixtures.

use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_units::{Currency, ExactQty, Money};

use crate::price_deck::{Commodity, PriceDeck};
use crate::regime::{CountryRegimeRow, RegimeFiscalEvaluation, evaluate_regime_fiscal_terms};
use crate::{EconError, money_mul_exact};

/// Stable identifier for an entity node in a [`CapitalEntityGraph`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityNodeId(pub String);

impl EntityNodeId {
    /// Construct a validated non-empty node identifier.
    pub fn new(id: impl Into<String>) -> Result<Self, EconError> {
        let raw = id.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed != raw {
            return Err(EconError::InvalidDomain {
                field: "entity_node_id",
                detail: format!("entity node id must be non-empty and trimmed: {raw:?}"),
            });
        }
        Ok(Self(raw))
    }

    /// Borrow the raw string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Entity classification in the ownership and IPO-perimeter graph (plan §23.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// Ultimate parent issuer (e.g., Barrick Parent).
    Parent,
    /// Minority-IPO carve-out entity (e.g., `NA-NewCo` anchored by NGM, PV, Fourmile; E658).
    CarveOut,
    /// Nevada Gold Mines JV (61.5% Barrick / 38.5% Newmont; E457).
    NgmJv,
    /// Pueblo Viejo JV (60% Barrick / 40% Newmont; E672).
    PvJv,
    /// Kibali JV (45% Barrick / 45% AngloGold Ashanti / 10% SOKIMO; E242–E243).
    KibaliJv,
    /// Twiga JV (North Mara & Bulyanhulu: 84% Barrick / 16% GoT free-carried + 50/50 benefits; E448).
    TwigaJv,
    /// Reko Diq JV (50% Barrick / 25% federal SOEs / 15% Balochistan funded + 10% free-carried; E302–E303).
    RekoDiqJv,
    /// Loulo-Gounkoto JV (80% Barrick / 20% Mali; E410–E414, E670).
    LouloGounkotoJv,
    /// Wholly owned operating or development asset (e.g., Fourmile pre-vend-in; E139, E658).
    WhollyOwnedAsset,
    /// Divested portfolio asset whose forward operating series terminates on `divested_on` (E485–E488).
    DivestedAsset,
}

impl EntityKind {
    fn tag(self) -> u8 {
        match self {
            Self::Parent => 1,
            Self::CarveOut => 2,
            Self::NgmJv => 3,
            Self::PvJv => 4,
            Self::KibaliJv => 5,
            Self::TwigaJv => 6,
            Self::RekoDiqJv => 7,
            Self::LouloGounkotoJv => 8,
            Self::WhollyOwnedAsset => 9,
            Self::DivestedAsset => 10,
        }
    }
}

/// One legal or economic entity node in a [`CapitalEntityGraph`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityNode {
    pub id: EntityNodeId,
    pub kind: EntityKind,
    pub name: String,
    /// Share count denominator for listed/carve-out entities (`Parent` / `CarveOut`).
    pub share_count: Option<u64>,
    pub currency: Currency,
    /// Optional row id in `registries/country_regimes.toml`.
    pub country_regime_id: Option<String>,
    /// Closing date (`YYYY-MM-DD`) for [`EntityKind::DivestedAsset`].
    pub divested_on: Option<String>,
    pub source_e_ids: Vec<String>,
}

/// Directed cash-flow / ownership waterfall edge from child asset/JV to parent holder (§23.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaterfallEdgeKind {
    /// Direct equity ownership share in `(0, 1]`.
    DirectEquity { equity_fraction: ExactQty },
    /// JV ownership edge subject to partner capital calls and optional non-consent dilution (NGM/PV).
    JvCapitalCall {
        initial_equity_fraction: ExactQty,
        default_dilution_enabled: bool,
    },
    /// Equity-accounted JV where cash exits only through dividend/loan repatriation gates (Kibali).
    EquityAccountedRepatriation {
        equity_fraction: ExactQty,
        repatriable_fraction: ExactQty,
    },
    /// State-sharing JV with annual economic-benefits true-up (Twiga 84/16 + 50/50; E448).
    EconomicBenefitsSharing {
        equity_fraction: ExactQty,
        state_target_share: ExactQty,
    },
    /// Project-financed JV with DSCR lock and sponsor recovery of free-carried funding (Reko Diq).
    ProjectFinanceDscrLock {
        equity_fraction: ExactQty,
        free_carried_fraction: ExactQty,
        min_dscr: ExactQty,
    },
    /// Closed divestiture exit terminating operating series with capped closing proceeds (Tongon/Hemlo).
    DivestitureExit {
        closed_date: String,
        max_proceeds: Money,
    },
}

impl WaterfallEdgeKind {
    /// Nominal parent equity fraction carried by this edge (`0` for a completed divestiture exit).
    #[must_use]
    pub fn equity_fraction(&self) -> ExactQty {
        match self {
            Self::DirectEquity { equity_fraction }
            | Self::EquityAccountedRepatriation {
                equity_fraction, ..
            }
            | Self::EconomicBenefitsSharing {
                equity_fraction, ..
            }
            | Self::ProjectFinanceDscrLock {
                equity_fraction, ..
            } => *equity_fraction,
            Self::JvCapitalCall {
                initial_equity_fraction,
                ..
            } => *initial_equity_fraction,
            Self::DivestitureExit { .. } => ExactQty::from_integer(0),
        }
    }

    fn tag(&self) -> u8 {
        match self {
            Self::DirectEquity { .. } => 1,
            Self::JvCapitalCall { .. } => 2,
            Self::EquityAccountedRepatriation { .. } => 3,
            Self::EconomicBenefitsSharing { .. } => 4,
            Self::ProjectFinanceDscrLock { .. } => 5,
            Self::DivestitureExit { .. } => 6,
        }
    }
}

/// One directed waterfall edge (`from_child -> to_parent`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaterfallEdge {
    pub from_child: EntityNodeId,
    pub to_parent: EntityNodeId,
    pub kind: WaterfallEdgeKind,
    pub source_e_ids: Vec<String>,
}

/// Directed acyclic ownership, IPO-perimeter, JV, and state-sharing graph (plan §23.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalEntityGraph {
    pub graph_id: String,
    pub as_of_date: String,
    pub nodes: Vec<EntityNode>,
    pub edges: Vec<WaterfallEdge>,
}

impl CapitalEntityGraph {
    /// Validate graph topology, acyclicity, share bounds, currency uniformity, and divestiture hygiene.
    pub fn validate(&self) -> Result<(), EconError> {
        if self.graph_id.trim().is_empty() {
            return Err(EconError::InvalidDomain {
                field: "graph_id",
                detail: "CapitalEntityGraph graph_id must be non-empty".to_string(),
            });
        }
        if !is_iso_date(&self.as_of_date) {
            return Err(EconError::InvalidDomain {
                field: "as_of_date",
                detail: format!("expected YYYY-MM-DD as_of_date, got {:?}", self.as_of_date),
            });
        }
        if self.nodes.is_empty() {
            return Err(EconError::InvalidDomain {
                field: "nodes",
                detail: "CapitalEntityGraph must contain at least one entity node".to_string(),
            });
        }

        let mut node_map: BTreeMap<&str, &EntityNode> = BTreeMap::new();
        for node in &self.nodes {
            if node.id.as_str().trim().is_empty() || node.id.as_str() != node.id.as_str().trim() {
                return Err(EconError::InvalidDomain {
                    field: "entity_node_id",
                    detail: format!("invalid entity node id {:?}", node.id.as_str()),
                });
            }
            if node.name.trim().is_empty() {
                return Err(EconError::InvalidDomain {
                    field: "entity_name",
                    detail: format!("node '{}' must have a non-empty name", node.id.as_str()),
                });
            }
            if node_map.insert(node.id.as_str(), node).is_some() {
                return Err(EconError::InvalidDomain {
                    field: "entity_node_id",
                    detail: format!("duplicate entity node id '{}'", node.id.as_str()),
                });
            }
            if let Some(shares) = node.share_count
                && shares == 0
            {
                return Err(EconError::InvalidDomain {
                    field: "share_count",
                    detail: format!("node '{}' share_count must be > 0", node.id.as_str()),
                });
            }
            if let Some(ref reg_id) = node.country_regime_id
                && (reg_id.trim().is_empty() || reg_id != reg_id.trim())
            {
                return Err(EconError::InvalidDomain {
                    field: "country_regime_id",
                    detail: format!(
                        "node '{}' country_regime_id must be non-empty and trimmed: {reg_id:?}",
                        node.id.as_str()
                    ),
                });
            }
            for eid in &node.source_e_ids {
                if eid.trim().is_empty() || eid != eid.trim() {
                    return Err(EconError::InvalidDomain {
                        field: "source_e_ids",
                        detail: format!(
                            "node '{}' has empty or untrimmed source_e_id {eid:?}",
                            node.id.as_str()
                        ),
                    });
                }
            }
            if node.kind == EntityKind::DivestedAsset {
                match node.divested_on.as_deref() {
                    Some(d) if is_iso_date(d) => {}
                    other => {
                        return Err(EconError::InvalidDomain {
                            field: "divested_on",
                            detail: format!(
                                "DivestedAsset '{}' requires YYYY-MM-DD divested_on, got {other:?}",
                                node.id.as_str()
                            ),
                        });
                    }
                }
            } else if node.divested_on.is_some()
                && matches!(node.kind, EntityKind::Parent | EntityKind::CarveOut)
            {
                return Err(EconError::InvalidDomain {
                    field: "divested_on",
                    detail: format!(
                        "{:?} node '{}' cannot carry divested_on",
                        node.kind,
                        node.id.as_str()
                    ),
                });
            } else if let Some(ref d) = node.divested_on
                && !is_iso_date(d)
            {
                return Err(EconError::InvalidDomain {
                    field: "divested_on",
                    detail: format!("node '{}' invalid divested_on {d:?}", node.id.as_str()),
                });
            }
        }

        let zero = ExactQty::from_integer(0);
        let one = ExactQty::from_integer(1);
        let mut outgoing_sum: BTreeMap<&str, ExactQty> = BTreeMap::new();
        let mut indegree: BTreeMap<&str, usize> = node_map.keys().map(|&k| (k, 0)).collect();
        let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut seen_pairs: BTreeSet<(&str, &str)> = BTreeSet::new();

        for edge in &self.edges {
            let child =
                node_map
                    .get(edge.from_child.as_str())
                    .ok_or_else(|| EconError::InvalidDomain {
                        field: "edge.from_child",
                        detail: format!("unknown child node '{}'", edge.from_child.as_str()),
                    })?;
            let parent =
                node_map
                    .get(edge.to_parent.as_str())
                    .ok_or_else(|| EconError::InvalidDomain {
                        field: "edge.to_parent",
                        detail: format!("unknown parent node '{}'", edge.to_parent.as_str()),
                    })?;
            if edge.from_child == edge.to_parent {
                return Err(EconError::InvalidDomain {
                    field: "edge",
                    detail: format!("self-loop on node '{}'", edge.from_child.as_str()),
                });
            }
            if !seen_pairs.insert((edge.from_child.as_str(), edge.to_parent.as_str())) {
                return Err(EconError::InvalidDomain {
                    field: "edge",
                    detail: format!(
                        "duplicate waterfall edge '{}' -> '{}'",
                        edge.from_child.as_str(),
                        edge.to_parent.as_str()
                    ),
                });
            }
            if child.currency != parent.currency {
                return Err(EconError::CurrencyMismatch {
                    left: child.currency,
                    right: parent.currency,
                });
            }
            for eid in &edge.source_e_ids {
                if eid.trim().is_empty() || eid != eid.trim() {
                    return Err(EconError::InvalidDomain {
                        field: "source_e_ids",
                        detail: format!(
                            "edge '{}' -> '{}' has empty or untrimmed source_e_id {eid:?}",
                            edge.from_child.as_str(),
                            edge.to_parent.as_str()
                        ),
                    });
                }
            }

            if child.kind == EntityKind::DivestedAsset
                && !matches!(edge.kind, WaterfallEdgeKind::DivestitureExit { .. })
            {
                return Err(EconError::InvalidDomain {
                    field: "edge.kind",
                    detail: format!(
                        "DivestedAsset '{}' cannot carry non-divestiture ongoing waterfall edge",
                        child.id.as_str()
                    ),
                });
            }

            match &edge.kind {
                WaterfallEdgeKind::DirectEquity { equity_fraction } => {
                    validate_fraction_positive("equity_fraction", *equity_fraction)?;
                }
                WaterfallEdgeKind::JvCapitalCall {
                    initial_equity_fraction,
                    ..
                } => {
                    validate_fraction_positive(
                        "initial_equity_fraction",
                        *initial_equity_fraction,
                    )?;
                }
                WaterfallEdgeKind::EquityAccountedRepatriation {
                    equity_fraction,
                    repatriable_fraction,
                } => {
                    validate_fraction_positive("equity_fraction", *equity_fraction)?;
                    validate_fraction_unit("repatriable_fraction", *repatriable_fraction)?;
                }
                WaterfallEdgeKind::EconomicBenefitsSharing {
                    equity_fraction,
                    state_target_share,
                } => {
                    validate_fraction_positive("equity_fraction", *equity_fraction)?;
                    validate_fraction_unit("state_target_share", *state_target_share)?;
                }
                WaterfallEdgeKind::ProjectFinanceDscrLock {
                    equity_fraction,
                    free_carried_fraction,
                    min_dscr,
                } => {
                    validate_fraction_positive("equity_fraction", *equity_fraction)?;
                    validate_fraction_unit("free_carried_fraction", *free_carried_fraction)?;
                    if equity_fraction
                        .checked_add(*free_carried_fraction)?
                        .checked_cmp(one)?
                        == Ordering::Greater
                    {
                        return Err(EconError::InvalidDomain {
                            field: "free_carried_fraction",
                            detail: "equity_fraction + free_carried_fraction cannot exceed 1"
                                .to_string(),
                        });
                    }
                    if min_dscr.signum() <= 0 {
                        return Err(EconError::InvalidDomain {
                            field: "min_dscr",
                            detail: "min_dscr must be strictly positive".to_string(),
                        });
                    }
                }
                WaterfallEdgeKind::DivestitureExit {
                    closed_date,
                    max_proceeds,
                } => {
                    if !is_iso_date(closed_date) {
                        return Err(EconError::InvalidDomain {
                            field: "closed_date",
                            detail: format!("invalid closed_date {closed_date:?}"),
                        });
                    }
                    if max_proceeds.ccy != child.currency {
                        return Err(EconError::CurrencyMismatch {
                            left: max_proceeds.ccy,
                            right: child.currency,
                        });
                    }
                    if max_proceeds.minor < 0 {
                        return Err(EconError::InvalidDomain {
                            field: "max_proceeds",
                            detail: "divestiture max_proceeds must be non-negative".to_string(),
                        });
                    }
                    if child.kind != EntityKind::DivestedAsset {
                        return Err(EconError::InvalidDomain {
                            field: "edge.kind",
                            detail: format!(
                                "DivestitureExit edge requires DivestedAsset child, found {:?} on '{}'",
                                child.kind,
                                child.id.as_str()
                            ),
                        });
                    }
                    if child.divested_on.as_deref() != Some(closed_date.as_str()) {
                        return Err(EconError::InvalidDomain {
                            field: "closed_date",
                            detail: format!(
                                "DivestitureExit closed_date '{}' != child divested_on {:?}",
                                closed_date, child.divested_on
                            ),
                        });
                    }
                }
            }

            let eq = edge.kind.equity_fraction();
            if eq.checked_cmp(zero)? != Ordering::Equal {
                let prev = outgoing_sum
                    .get(edge.from_child.as_str())
                    .copied()
                    .unwrap_or(zero);
                let next = prev.checked_add(eq)?;
                if next.checked_cmp(one)? == Ordering::Greater {
                    return Err(EconError::InvalidDomain {
                        field: "equity_fraction",
                        detail: format!(
                            "outgoing equity from '{}' exceeds 100% (prevents perimeter double-counting)",
                            edge.from_child.as_str()
                        ),
                    });
                }
                outgoing_sum.insert(edge.from_child.as_str(), next);
            }

            adj.entry(edge.from_child.as_str())
                .or_default()
                .push(edge.to_parent.as_str());
            if let Some(deg) = indegree.get_mut(edge.to_parent.as_str()) {
                *deg += 1;
            }
        }

        // Kahn's topological sort for exact acyclicity proof.
        let mut ready: BTreeSet<&str> = indegree
            .iter()
            .filter_map(|(&k, &d)| if d == 0 { Some(k) } else { None })
            .collect();
        let mut visited = 0usize;
        while let Some(u) = ready.pop_first() {
            visited += 1;
            if let Some(neighbors) = adj.get(u) {
                for &v in neighbors {
                    if let Some(deg) = indegree.get_mut(v) {
                        *deg -= 1;
                        if *deg == 0 {
                            ready.insert(v);
                        }
                    }
                }
            }
        }
        if visited != node_map.len() {
            return Err(EconError::InvalidDomain {
                field: "edges",
                detail: "CapitalEntityGraph contains a cycle".to_string(),
            });
        }

        Ok(())
    }

    /// Look up a node by identifier.
    #[must_use]
    pub fn node(&self, id: &EntityNodeId) -> Option<&EntityNode> {
        self.nodes.iter().find(|n| &n.id == id)
    }

    /// Compute the exact effective equity fraction held by `holder` in `asset` by summing the
    /// exact product of edge equity fractions across all directed paths `asset -> ... -> holder`.
    pub fn effective_equity_fraction(
        &self,
        asset: &EntityNodeId,
        holder: &EntityNodeId,
    ) -> Result<ExactQty, EconError> {
        self.validate()?;
        if self.node(asset).is_none() {
            return Err(EconError::InvalidDomain {
                field: "asset",
                detail: format!("unknown asset node '{}'", asset.as_str()),
            });
        }
        if self.node(holder).is_none() {
            return Err(EconError::InvalidDomain {
                field: "holder",
                detail: format!("unknown holder node '{}'", holder.as_str()),
            });
        }
        if asset == holder {
            return Ok(ExactQty::from_integer(1));
        }
        let mut out_edges: BTreeMap<&str, Vec<(&str, ExactQty)>> = BTreeMap::new();
        for e in &self.edges {
            out_edges
                .entry(e.from_child.as_str())
                .or_default()
                .push((e.to_parent.as_str(), e.kind.equity_fraction()));
        }
        // Graph is validated acyclic, so memoized exact path-product DP terminates cleanly.
        let mut memo: BTreeMap<&str, ExactQty> = BTreeMap::new();
        eval_path_equity(asset.as_str(), holder.as_str(), &out_edges, &mut memo)
    }

    /// Compute a canonical order-independent SHA-256 digest of the validated entity graph.
    pub fn digest(&self) -> Result<Digest32, EconError> {
        self.validate()?;
        let mut nodes_sorted: Vec<&EntityNode> = self.nodes.iter().collect();
        nodes_sorted.sort_by(|a, b| a.id.cmp(&b.id));
        let mut edges_sorted: Vec<&WaterfallEdge> = self.edges.iter().collect();
        edges_sorted.sort_by(|a, b| {
            a.from_child
                .cmp(&b.from_child)
                .then_with(|| a.to_parent.cmp(&b.to_parent))
        });

        let mut enc = Encoder::new_frame(*b"CEG1", 1);
        enc.put_str(&self.graph_id);
        enc.put_str(&self.as_of_date);
        enc.put_u64(nodes_sorted.len() as u64);
        for n in nodes_sorted {
            enc.put_str(n.id.as_str());
            enc.put_u8(n.kind.tag());
            enc.put_str(&n.name);
            enc.put_u8(u8::from(n.share_count.is_some()));
            if let Some(s) = n.share_count {
                enc.put_u64(s);
            }
            enc.put_str(currency_code(n.currency));
            enc.put_u8(u8::from(n.country_regime_id.is_some()));
            if let Some(ref reg_id) = n.country_regime_id {
                enc.put_str(reg_id);
            }
            enc.put_u8(u8::from(n.divested_on.is_some()));
            if let Some(ref div_on) = n.divested_on {
                enc.put_str(div_on);
            }
            let mut eids = n.source_e_ids.clone();
            eids.sort();
            eids.dedup();
            enc.put_u64(eids.len() as u64);
            for eid in &eids {
                enc.put_str(eid);
            }
        }
        enc.put_u64(edges_sorted.len() as u64);
        for e in edges_sorted {
            enc.put_str(e.from_child.as_str());
            enc.put_str(e.to_parent.as_str());
            enc.put_u8(e.kind.tag());
            match &e.kind {
                WaterfallEdgeKind::DirectEquity { equity_fraction } => {
                    put_qty(&mut enc, *equity_fraction);
                }
                WaterfallEdgeKind::JvCapitalCall {
                    initial_equity_fraction,
                    default_dilution_enabled,
                } => {
                    put_qty(&mut enc, *initial_equity_fraction);
                    enc.put_u8(u8::from(*default_dilution_enabled));
                }
                WaterfallEdgeKind::EquityAccountedRepatriation {
                    equity_fraction,
                    repatriable_fraction,
                } => {
                    put_qty(&mut enc, *equity_fraction);
                    put_qty(&mut enc, *repatriable_fraction);
                }
                WaterfallEdgeKind::EconomicBenefitsSharing {
                    equity_fraction,
                    state_target_share,
                } => {
                    put_qty(&mut enc, *equity_fraction);
                    put_qty(&mut enc, *state_target_share);
                }
                WaterfallEdgeKind::ProjectFinanceDscrLock {
                    equity_fraction,
                    free_carried_fraction,
                    min_dscr,
                } => {
                    put_qty(&mut enc, *equity_fraction);
                    put_qty(&mut enc, *free_carried_fraction);
                    put_qty(&mut enc, *min_dscr);
                }
                WaterfallEdgeKind::DivestitureExit {
                    closed_date,
                    max_proceeds,
                } => {
                    enc.put_str(closed_date);
                    enc.put_str(currency_code(max_proceeds.ccy));
                    enc.put_i128(max_proceeds.minor);
                }
            }
            let mut eids = e.source_e_ids.clone();
            eids.sort();
            eids.dedup();
            enc.put_u64(eids.len() as u64);
            for eid in &eids {
                enc.put_str(eid);
            }
        }
        Ok(sha256(&enc.finish()))
    }
}

fn eval_path_equity<'a>(
    cur: &'a str,
    target: &'a str,
    out_edges: &BTreeMap<&'a str, Vec<(&'a str, ExactQty)>>,
    memo: &mut BTreeMap<&'a str, ExactQty>,
) -> Result<ExactQty, EconError> {
    if cur == target {
        return Ok(ExactQty::from_integer(1));
    }
    if let Some(&cached) = memo.get(cur) {
        return Ok(cached);
    }
    let mut sum = ExactQty::from_integer(0);
    if let Some(edges) = out_edges.get(cur) {
        for &(next, frac) in edges {
            if frac.signum() == 0 {
                continue;
            }
            let sub = eval_path_equity(next, target, out_edges, memo)?;
            if sub.signum() != 0 {
                let prod = frac.checked_mul(sub)?;
                sum = sum.checked_add(prod)?;
            }
        }
    }
    memo.insert(cur, sum);
    Ok(sum)
}

// ---------------------------------------------------------------------------
// 1. Dual-entity denominator evaluation: Barrick Parent vs. NA-NewCo (§23.2)
// ---------------------------------------------------------------------------

/// Input for one listed/carve-out valuation denominator (`Parent` or `NA-NewCo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityDenominatorInput {
    pub entity_id: EntityNodeId,
    pub baseline_nav: Money,
    pub baseline_shares: u64,
    /// Attributable incremental NAV added by the candidate on this denominator.
    pub delta_nav: Money,
    /// Net share change (negative for buybacks, positive for issuance/IPO dilution).
    pub delta_shares: i64,
    /// Post-candidate attributable total AISC cost numerator.
    pub post_aisc_total_cost: Money,
    /// Post-candidate attributable payable gold-equivalent ounces sold (`> 0`).
    pub post_payable_ounces_sold: ExactQty,
    /// Post-candidate attributable annual free cash flow.
    pub post_annual_fcf: Money,
    /// Post-candidate equity value / market capitalization (`> 0`) for FCF yield.
    pub post_equity_value: Money,
}

/// Exact per-entity denominator result (`ΔNAV/share`, AISC, FCF yield).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityDenominatorResult {
    pub entity_id: EntityNodeId,
    pub currency: Currency,
    pub ending_shares: u64,
    pub ending_nav: Money,
    /// Exact baseline NAV per actual share in major currency units / share.
    pub baseline_nav_per_share: ExactQty,
    /// Exact ending NAV per actual share in major currency units / share.
    pub ending_nav_per_share: ExactQty,
    /// Exact `ending_nav_per_share - baseline_nav_per_share` in major currency units / share.
    pub delta_nav_per_share: ExactQty,
    /// Exact post-candidate AISC in major currency units / oz.
    pub post_aisc_per_oz: ExactQty,
    /// Exact post-candidate annual FCF yield (`post_annual_fcf / post_equity_value`).
    pub post_fcf_yield: ExactQty,
}

/// Simultaneous evaluation on both Parent and CarveOut (`NA-NewCo`) denominators (§23.2 item 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualEntityEvaluation {
    pub candidate_id: String,
    pub parent: EntityDenominatorResult,
    pub carve_out: EntityDenominatorResult,
    /// `true` when ΔNAV/share is strictly accretive (`> 0`) for one denominator and strictly
    /// dilutive (`< 0`) for the other.
    pub divergent_accretion: bool,
}

/// Evaluate one capital candidate simultaneously on both Parent and `NA-NewCo` denominators.
pub fn evaluate_dual_entity_delta_nav(
    candidate_id: impl Into<String>,
    parent_input: &EntityDenominatorInput,
    carve_out_input: &EntityDenominatorInput,
) -> Result<DualEntityEvaluation, EconError> {
    let cid = candidate_id.into();
    if cid.trim().is_empty() {
        return Err(EconError::InvalidDomain {
            field: "candidate_id",
            detail: "candidate_id must be non-empty".to_string(),
        });
    }
    if parent_input.entity_id == carve_out_input.entity_id {
        return Err(EconError::InvalidDomain {
            field: "entity_id",
            detail: "Parent and CarveOut denominators must have distinct entity IDs".to_string(),
        });
    }
    if parent_input.baseline_nav.ccy != carve_out_input.baseline_nav.ccy {
        return Err(EconError::CurrencyMismatch {
            left: parent_input.baseline_nav.ccy,
            right: carve_out_input.baseline_nav.ccy,
        });
    }

    let parent = eval_single_denominator(parent_input)?;
    let carve_out = eval_single_denominator(carve_out_input)?;
    let p_sign = parent.delta_nav_per_share.signum();
    let c_sign = carve_out.delta_nav_per_share.signum();
    let divergent_accretion = (p_sign > 0 && c_sign < 0) || (p_sign < 0 && c_sign > 0);

    Ok(DualEntityEvaluation {
        candidate_id: cid,
        parent,
        carve_out,
        divergent_accretion,
    })
}

fn eval_single_denominator(
    input: &EntityDenominatorInput,
) -> Result<EntityDenominatorResult, EconError> {
    let ccy = input.baseline_nav.ccy;
    check_ccy(ccy, input.delta_nav.ccy)?;
    check_ccy(ccy, input.post_aisc_total_cost.ccy)?;
    check_ccy(ccy, input.post_annual_fcf.ccy)?;
    check_ccy(ccy, input.post_equity_value.ccy)?;

    if input.baseline_shares == 0 {
        return Err(EconError::InvalidDomain {
            field: "baseline_shares",
            detail: "baseline_shares must be strictly positive".to_string(),
        });
    }
    let ending_shares_i128 = i128::from(input.baseline_shares)
        .checked_add(i128::from(input.delta_shares))
        .ok_or(EconError::Overflow)?;
    if ending_shares_i128 <= 0 || ending_shares_i128 > i128::from(u64::MAX) {
        return Err(EconError::InvalidDomain {
            field: "ending_shares",
            detail: format!("ending shares {ending_shares_i128} must lie in 1..=u64::MAX"),
        });
    }
    let ending_shares = ending_shares_i128 as u64;
    if input.post_payable_ounces_sold.signum() <= 0 {
        return Err(EconError::InvalidDomain {
            field: "post_payable_ounces_sold",
            detail: "post_payable_ounces_sold must be strictly positive".to_string(),
        });
    }
    if input.post_aisc_total_cost.minor < 0 {
        return Err(EconError::InvalidDomain {
            field: "post_aisc_total_cost",
            detail: "post_aisc_total_cost must be non-negative".to_string(),
        });
    }
    if input.post_equity_value.minor <= 0 {
        return Err(EconError::InvalidDomain {
            field: "post_equity_value",
            detail: "post_equity_value must be strictly positive for FCF yield".to_string(),
        });
    }

    let ending_nav = input.baseline_nav.checked_add(input.delta_nav)?;
    let baseline_nav_per_share = money_as_major_qty(input.baseline_nav)?
        .checked_div(ExactQty::from_integer(i128::from(input.baseline_shares)))?;
    let ending_nav_per_share = money_as_major_qty(ending_nav)?
        .checked_div(ExactQty::from_integer(i128::from(ending_shares)))?;
    let delta_nav_per_share = ending_nav_per_share.checked_sub(baseline_nav_per_share)?;
    let post_aisc_per_oz = money_as_major_qty(input.post_aisc_total_cost)?
        .checked_div(input.post_payable_ounces_sold)?;
    let post_fcf_yield = ExactQty::new(input.post_annual_fcf.minor, input.post_equity_value.minor)?;

    Ok(EntityDenominatorResult {
        entity_id: input.entity_id.clone(),
        currency: ccy,
        ending_shares,
        ending_nav,
        baseline_nav_per_share,
        ending_nav_per_share,
        delta_nav_per_share,
        post_aisc_per_oz,
        post_fcf_yield,
    })
}

// ---------------------------------------------------------------------------
// 2. NGM (61.5%/38.5%) & Pueblo Viejo (60%/40%) capital call & dilution (§23.2)
// ---------------------------------------------------------------------------

/// Input for a JV growth-capital call with pro-rata funding or contractual dilution (§23.2 item 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JvCapitalCallInput {
    pub jv_id: EntityNodeId,
    /// Operator's pre-call ownership share (e.g., `615/1000` for NGM, `60/100` for Pueblo Viejo).
    pub prior_operator_share: ExactQty,
    /// Pre-call agreed JV capital/equity base `V > 0`.
    pub prior_total_capital_account: Money,
    /// Contractual total capital required for the growth project (`>= 0`).
    pub total_call_required: Money,
    /// Operator's funded cash contribution `P >= 0`.
    pub operator_contribution: Money,
    /// JV partner's funded cash contribution `X >= 0`.
    pub partner_contribution: Money,
    /// Contractual non-consent dilution multiplier (`>= 1`).
    /// `1` gives straight pre-money dilution `(s*V + P) / (V + P + X)`.
    /// Values `> 1` apply an explicit dilution penalty to any shortfall covered by the funding party.
    pub non_consent_penalty_multiplier: ExactQty,
}

/// Exact conserved ownership and funding result of a JV capital call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JvCapitalCallOutcome {
    pub jv_id: EntityNodeId,
    pub total_funded: Money,
    pub fully_funded: bool,
    pub funding_shortfall: Money,
    pub prior_operator_share: ExactQty,
    pub prior_partner_share: ExactQty,
    pub new_operator_share: ExactQty,
    pub new_partner_share: ExactQty,
    pub operator_share_delta: ExactQty,
    pub dilution_triggered: bool,
}

/// Evaluate a JV capital call (NGM 61.5%/38.5%, Pueblo Viejo 60%/40%), conserving
/// `new_operator_share + new_partner_share == 1` exactly.
pub fn evaluate_jv_capital_call(
    input: &JvCapitalCallInput,
) -> Result<JvCapitalCallOutcome, EconError> {
    let ccy = input.prior_total_capital_account.ccy;
    check_ccy(ccy, input.total_call_required.ccy)?;
    check_ccy(ccy, input.operator_contribution.ccy)?;
    check_ccy(ccy, input.partner_contribution.ccy)?;

    validate_fraction_unit("prior_operator_share", input.prior_operator_share)?;
    if input.prior_total_capital_account.minor <= 0 {
        return Err(EconError::InvalidDomain {
            field: "prior_total_capital_account",
            detail: "prior_total_capital_account must be strictly positive".to_string(),
        });
    }
    if input.total_call_required.minor < 0
        || input.operator_contribution.minor < 0
        || input.partner_contribution.minor < 0
    {
        return Err(EconError::InvalidDomain {
            field: "capital_call",
            detail: "required call and contributions must be non-negative".to_string(),
        });
    }
    let one = ExactQty::from_integer(1);
    let zero = ExactQty::from_integer(0);
    if input.non_consent_penalty_multiplier.checked_cmp(one)? == Ordering::Less {
        return Err(EconError::InvalidDomain {
            field: "non_consent_penalty_multiplier",
            detail: "non_consent_penalty_multiplier must be >= 1".to_string(),
        });
    }

    let prior_partner_share = one.checked_sub(input.prior_operator_share)?;
    let total_funded = input
        .operator_contribution
        .checked_add(input.partner_contribution)?;
    let (fully_funded, funding_shortfall) = if total_funded.minor >= input.total_call_required.minor
    {
        (true, Money::from_minor(ccy, 0))
    } else {
        (false, input.total_call_required.checked_sub(total_funded)?)
    };

    let v_qty = ExactQty::from_integer(input.prior_total_capital_account.minor);
    let p_qty = ExactQty::from_integer(input.operator_contribution.minor);
    let x_qty = ExactQty::from_integer(input.partner_contribution.minor);
    let total_after = v_qty.checked_add(p_qty)?.checked_add(x_qty)?;

    let mut op_account = input
        .prior_operator_share
        .checked_mul(v_qty)?
        .checked_add(p_qty)?;

    // Apply explicit contractual sole-risk dilution penalty ONLY to the shortfall actually covered
    // above a party's pro-rata obligation on total_call_required (or on total_funded when required == 0).
    // If a party only funds its own pro-rata share while the other defaults without cover, normal
    // straight pre-money dilution applies rather than an unearned cover penalty.
    let extra_mult = input.non_consent_penalty_multiplier.checked_sub(one)?;
    if extra_mult.signum() > 0 && total_funded.minor > 0 {
        let basis_minor = if input.total_call_required.minor > 0 {
            input.total_call_required.minor
        } else {
            total_funded.minor
        };
        let basis_qty = ExactQty::from_integer(basis_minor);
        let op_prorata = input.prior_operator_share.checked_mul(basis_qty)?;
        let partner_prorata = prior_partner_share.checked_mul(basis_qty)?;

        let op_over = p_qty.checked_sub(op_prorata)?;
        let partner_over = x_qty.checked_sub(partner_prorata)?;

        let penalty_shift = if op_over.signum() > 0 && partner_over.signum() < 0 {
            let partner_shortfall = partner_over.checked_neg()?;
            let covered = if op_over.checked_cmp(partner_shortfall)? == Ordering::Less {
                op_over
            } else {
                partner_shortfall
            };
            covered.checked_mul(extra_mult)?
        } else if partner_over.signum() > 0 && op_over.signum() < 0 {
            let op_shortfall = op_over.checked_neg()?;
            let covered = if partner_over.checked_cmp(op_shortfall)? == Ordering::Less {
                partner_over
            } else {
                op_shortfall
            };
            covered.checked_mul(extra_mult)?.checked_neg()?
        } else {
            zero
        };

        if penalty_shift.signum() != 0 {
            op_account = op_account.checked_add(penalty_shift)?;
            if op_account.signum() < 0 {
                op_account = zero;
            } else if op_account.checked_cmp(total_after)? == Ordering::Greater {
                op_account = total_after;
            }
        }
    }

    let new_operator_share = op_account.checked_div(total_after)?;
    let new_partner_share = one.checked_sub(new_operator_share)?;
    let operator_share_delta = new_operator_share.checked_sub(input.prior_operator_share)?;
    let dilution_triggered = operator_share_delta.signum() != 0;

    Ok(JvCapitalCallOutcome {
        jv_id: input.jv_id.clone(),
        total_funded,
        fully_funded,
        funding_shortfall,
        prior_operator_share: input.prior_operator_share,
        prior_partner_share,
        new_operator_share,
        new_partner_share,
        operator_share_delta,
        dilution_triggered,
    })
}

// ---------------------------------------------------------------------------
// 3. Fourmile vend-in vs. inter-entity tolling option (§23.2 item 1, §15.6)
// ---------------------------------------------------------------------------

/// Preferred strategic path for Fourmile on a given valuation denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FourmilePreferredPath {
    VendInToNgm,
    InterEntityTolling,
    Indifferent,
}

/// Inputs comparing Fourmile vend-in to NGM vs. inter-entity tolling (FACT E139, E141, E658).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FourmileOptionInput {
    pub parent_id: EntityNodeId,
    pub parent_shares: u64,
    pub newco_id: EntityNodeId,
    pub newco_shares: u64,
    /// Barrick Parent's retained equity fraction in `NA-NewCo` post-minority IPO (in `(0, 1]`).
    pub parent_retained_newco_fraction: ExactQty,
    /// NGM equity fraction held by `NA-NewCo` (FACT E457: `615/1000`).
    pub ngm_barrick_fraction: ExactQty,
    /// Standalone 100% project NAV of Fourmile before NGM synergies or tolling leakage.
    pub fourmile_standalone_project_nav: Money,
    /// Fair-market-value vend-in payment from Newmont for its `1 - ngm_barrick_fraction` share.
    /// Walled under `INV-007`: never exposed in Newmont-facing JV projections.
    pub vend_in_fmv_payment_from_newmont: Money,
    /// 100%-basis NPV of shared NGM infrastructure synergies realized under vend-in.
    pub vend_in_synergy_nav_uplift_100pct: Money,
    /// Present value of gross inter-entity tolling fee paid by Fourmile to NGM processing plants.
    pub tolling_fee_pv_paid_to_ngm: Money,
    /// Incremental standalone capital/transport cost PV incurred under tolling vs. full vend-in.
    pub tolling_standalone_penalty_pv: Money,
}

/// Dual-denominator comparison of Fourmile vend-in vs. inter-entity tolling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FourmileOptionComparison {
    /// Always `true`: vend-in FMV pricing crosses a JV information wall (`INV-007`, E139/E658).
    pub walled_transfer_price: bool,
    pub vend_in_newco_delta_nav: Money,
    pub vend_in_parent_delta_nav: Money,
    pub vend_in_newco_delta_nav_per_share: ExactQty,
    pub vend_in_parent_delta_nav_per_share: ExactQty,
    pub tolling_newco_net_leakage: Money,
    pub tolling_newco_delta_nav: Money,
    pub tolling_parent_delta_nav: Money,
    pub tolling_newco_delta_nav_per_share: ExactQty,
    pub tolling_parent_delta_nav_per_share: ExactQty,
    /// `vend_in_newco_delta_nav - tolling_newco_delta_nav`.
    pub vend_in_advantage_newco: Money,
    /// `vend_in_parent_delta_nav - tolling_parent_delta_nav`.
    pub vend_in_advantage_parent: Money,
    /// Exact Newmont FMV payment at which Vend-In equals Tolling on `NA-NewCo` (and Parent).
    pub breakeven_newmont_fmv_payment: Money,
    pub preferred_for_newco: FourmilePreferredPath,
    pub preferred_for_parent: FourmilePreferredPath,
}

/// Compare Fourmile vend-in to NGM vs. inter-entity tolling on both Parent and `NA-NewCo`.
pub fn compare_fourmile_vend_in_vs_tolling(
    input: &FourmileOptionInput,
) -> Result<FourmileOptionComparison, EconError> {
    let ccy = input.fourmile_standalone_project_nav.ccy;
    check_ccy(ccy, input.vend_in_fmv_payment_from_newmont.ccy)?;
    check_ccy(ccy, input.vend_in_synergy_nav_uplift_100pct.ccy)?;
    check_ccy(ccy, input.tolling_fee_pv_paid_to_ngm.ccy)?;
    check_ccy(ccy, input.tolling_standalone_penalty_pv.ccy)?;

    if input.parent_id == input.newco_id {
        return Err(EconError::InvalidDomain {
            field: "newco_id",
            detail: "parent_id and newco_id must be distinct".to_string(),
        });
    }
    if input.parent_shares == 0 || input.newco_shares == 0 {
        return Err(EconError::InvalidDomain {
            field: "shares",
            detail: "parent_shares and newco_shares must be strictly positive".to_string(),
        });
    }
    validate_fraction_positive(
        "parent_retained_newco_fraction",
        input.parent_retained_newco_fraction,
    )?;
    validate_fraction_positive("ngm_barrick_fraction", input.ngm_barrick_fraction)?;
    if input.vend_in_fmv_payment_from_newmont.minor < 0
        || input.tolling_fee_pv_paid_to_ngm.minor < 0
        || input.tolling_standalone_penalty_pv.minor < 0
    {
        return Err(EconError::InvalidDomain {
            field: "fourmile_option",
            detail: "FMV payment, tolling fee PV, and standalone penalty must be non-negative"
                .to_string(),
        });
    }

    let one = ExactQty::from_integer(1);
    let newmont_share = one.checked_sub(input.ngm_barrick_fraction)?;

    // Vend-in: NA-NewCo retains ngm_barrick_fraction of (standalone + synergy) + Newmont FMV payment.
    let combined_vend_in_100pct = input
        .fourmile_standalone_project_nav
        .checked_add(input.vend_in_synergy_nav_uplift_100pct)?;
    let vend_in_without_payment =
        money_mul_exact(combined_vend_in_100pct, input.ngm_barrick_fraction)?;
    let vend_in_newco_delta_nav =
        vend_in_without_payment.checked_add(input.vend_in_fmv_payment_from_newmont)?;
    let vend_in_parent_delta_nav = money_mul_exact(
        vend_in_newco_delta_nav,
        input.parent_retained_newco_fraction,
    )?;

    // Tolling: NA-NewCo keeps 100% of Fourmile, pays gross toll to NGM (losing Newmont's 38.5%
    // share of the toll as net leakage) plus any standalone infrastructure penalty.
    let tolling_newco_net_leakage =
        money_mul_exact(input.tolling_fee_pv_paid_to_ngm, newmont_share)?;
    let tolling_newco_delta_nav = input
        .fourmile_standalone_project_nav
        .checked_sub(tolling_newco_net_leakage)?
        .checked_sub(input.tolling_standalone_penalty_pv)?;
    let tolling_parent_delta_nav = money_mul_exact(
        tolling_newco_delta_nav,
        input.parent_retained_newco_fraction,
    )?;

    let vend_in_advantage_newco = vend_in_newco_delta_nav.checked_sub(tolling_newco_delta_nav)?;
    let vend_in_advantage_parent =
        vend_in_parent_delta_nav.checked_sub(tolling_parent_delta_nav)?;

    // Exact breakeven Newmont payment: tolling_newco_delta_nav - ngm_share * (standalone + synergy).
    // May be negative when shared-infrastructure synergies alone make vend-in dominate tolling even at $0 payment.
    let breakeven_newmont_fmv_payment =
        tolling_newco_delta_nav.checked_sub(vend_in_without_payment)?;

    let parent_sh = ExactQty::from_integer(i128::from(input.parent_shares));
    let newco_sh = ExactQty::from_integer(i128::from(input.newco_shares));
    let vend_in_newco_delta_nav_per_share =
        money_as_major_qty(vend_in_newco_delta_nav)?.checked_div(newco_sh)?;
    let vend_in_parent_delta_nav_per_share =
        money_as_major_qty(vend_in_parent_delta_nav)?.checked_div(parent_sh)?;
    let tolling_newco_delta_nav_per_share =
        money_as_major_qty(tolling_newco_delta_nav)?.checked_div(newco_sh)?;
    let tolling_parent_delta_nav_per_share =
        money_as_major_qty(tolling_parent_delta_nav)?.checked_div(parent_sh)?;

    Ok(FourmileOptionComparison {
        walled_transfer_price: true,
        vend_in_newco_delta_nav,
        vend_in_parent_delta_nav,
        vend_in_newco_delta_nav_per_share,
        vend_in_parent_delta_nav_per_share,
        tolling_newco_net_leakage,
        tolling_newco_delta_nav,
        tolling_parent_delta_nav,
        tolling_newco_delta_nav_per_share,
        tolling_parent_delta_nav_per_share,
        vend_in_advantage_newco,
        vend_in_advantage_parent,
        breakeven_newmont_fmv_payment,
        preferred_for_newco: classify_fourmile_path(vend_in_advantage_newco),
        preferred_for_parent: classify_fourmile_path(vend_in_advantage_parent),
    })
}

fn classify_fourmile_path(advantage: Money) -> FourmilePreferredPath {
    match advantage.minor.cmp(&0) {
        Ordering::Greater => FourmilePreferredPath::VendInToNgm,
        Ordering::Less => FourmilePreferredPath::InterEntityTolling,
        Ordering::Equal => FourmilePreferredPath::Indifferent,
    }
}

// ---------------------------------------------------------------------------
// 4. Kibali JV repatriation gate + CountryRegime integration (§23.2 item 3)
// ---------------------------------------------------------------------------

/// Input for Kibali equity-accounted JV waterfall (45/45/10 + DRC repatriation gate; FACT E242–E243).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KibaliWaterfallInput {
    pub payable_gold_oz_100pct: ExactQty,
    pub operating_cost_100pct: Money,
    pub capex_100pct: Money,
    /// Fraction of 100%-basis distributable cash cleared through DRC dividend/loan repatriation gates.
    pub repatriable_fraction: ExactQty,
    /// Barrick equity share (`45/100`; supports post-2029 Art. 80 renewal dilution).
    pub barrick_equity_fraction: ExactQty,
    /// AngloGold Ashanti equity share (`45/100`).
    pub anglogold_equity_fraction: ExactQty,
    /// SOKIMO state equity share (`10/100`, stepping +5% per Art. 80 renewal).
    pub sokimo_equity_fraction: ExactQty,
}

/// Exact conserved Kibali waterfall output separating repatriated dividends from trapped JV cash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KibaliWaterfallResult {
    pub fiscal: RegimeFiscalEvaluation,
    pub distributable_cash_100pct: Money,
    pub funding_deficit_100pct: Money,
    pub repatriated_total_100pct: Money,
    pub trapped_cash_100pct: Money,
    pub barrick_repatriated: Money,
    pub anglogold_repatriated: Money,
    pub sokimo_repatriated: Money,
    pub barrick_trapped_share: Money,
    pub anglogold_trapped_share: Money,
    pub sokimo_trapped_share: Money,
}

/// Evaluate Kibali's fiscal terms (`drc_kibali_2026`: 5.7% royalty, 30% CIT, 50% super-profits
/// tax > `$2,000/oz`) and repatriation gate, conserving every minor unit.
pub fn evaluate_kibali_waterfall(
    regime: &CountryRegimeRow,
    deck: &PriceDeck,
    input: &KibaliWaterfallInput,
    allow_historical: bool,
) -> Result<KibaliWaterfallResult, EconError> {
    let ccy = input.operating_cost_100pct.ccy;
    check_ccy(ccy, input.capex_100pct.ccy)?;
    if input.capex_100pct.minor < 0 {
        return Err(EconError::InvalidDomain {
            field: "capex_100pct",
            detail: "capex_100pct must be non-negative".to_string(),
        });
    }
    if regime.state == crate::regime::RegimeState::Suspension
        && input.payable_gold_oz_100pct.signum() > 0
    {
        return Err(EconError::InvalidDomain {
            field: "regime.state",
            detail: format!(
                "cannot evaluate active Kibali production on suspended regime '{}'",
                regime.id
            ),
        });
    }
    validate_fraction_unit("repatriable_fraction", input.repatriable_fraction)?;
    let shares = [
        input.barrick_equity_fraction,
        input.anglogold_equity_fraction,
        input.sokimo_equity_fraction,
    ];
    validate_simplex("kibali_equity_shares", &shares)?;

    let fiscal = evaluate_regime_fiscal_terms(
        regime,
        deck,
        Commodity::Au,
        input.payable_gold_oz_100pct,
        input.operating_cost_100pct,
        allow_historical,
    )?;

    let net_cash = fiscal
        .breakdown
        .after_tax_cashflow
        .checked_sub(input.capex_100pct)?;
    let zero_m = Money::from_minor(ccy, 0);
    let (distributable_cash_100pct, funding_deficit_100pct) = if net_cash.minor >= 0 {
        (net_cash, zero_m)
    } else {
        (zero_m, zero_m.checked_sub(net_cash)?)
    };

    let repatriated_total_100pct =
        money_mul_exact(distributable_cash_100pct, input.repatriable_fraction)?;
    let trapped_cash_100pct = distributable_cash_100pct.checked_sub(repatriated_total_100pct)?;

    let rep_slices = allocate_money_exact(repatriated_total_100pct, &shares)?;
    let trap_slices = allocate_money_exact(trapped_cash_100pct, &shares)?;

    Ok(KibaliWaterfallResult {
        fiscal,
        distributable_cash_100pct,
        funding_deficit_100pct,
        repatriated_total_100pct,
        trapped_cash_100pct,
        barrick_repatriated: rep_slices[0],
        anglogold_repatriated: rep_slices[1],
        sokimo_repatriated: rep_slices[2],
        barrick_trapped_share: trap_slices[0],
        anglogold_trapped_share: trap_slices[1],
        sokimo_trapped_share: trap_slices[2],
    })
}

// ---------------------------------------------------------------------------
// 5. Twiga JV 84%/16% + 50/50 economic benefits annual true-up (§23.2 item 4)
// ---------------------------------------------------------------------------

/// Input for Twiga (North Mara & Bulyanhulu) 50/50 economic benefits true-up (FACT E448).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwigaWaterfallInput {
    pub gross_revenue_100pct: Money,
    pub operating_and_capital_costs_100pct: Money,
    pub royalties_and_fees_paid_to_got: Money,
    pub corporate_income_tax_paid_to_got: Money,
    pub other_state_taxes_and_levies: Money,
    /// Barrick equity share (`84/100`).
    pub barrick_equity_fraction: ExactQty,
    /// Government of Tanzania free-carried equity share (`16/100`).
    pub got_free_carried_fraction: ExactQty,
    /// Contractual total economic benefits share for GoT (`50/100`; FACT E448).
    pub got_target_economic_benefit_share: ExactQty,
}

/// Exact conserved Twiga economic-benefits waterfall result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwigaWaterfallResult {
    pub total_economic_benefits: Money,
    pub state_direct_take: Money,
    pub pre_true_up_distributable: Money,
    /// Explicit JV cash deficit when `state_direct_take > total_economic_benefits` (never silently dropped).
    pub funding_deficit: Money,
    pub got_equity_dividend: Money,
    pub barrick_pre_true_up_dividend: Money,
    pub got_pre_true_up_total: Money,
    pub got_target_amount: Money,
    pub annual_true_up_to_got: Money,
    /// Non-zero only if even 100% of Barrick's distributable dividend cannot cover the 50% gap.
    pub true_up_shortfall: Money,
    pub got_final_total: Money,
    pub barrick_net_cash: Money,
}

/// Evaluate the Twiga 84/16 + 50/50 economic benefits sharing annual true-up (`E448`),
/// conserving `barrick_net_cash + got_final_total - funding_deficit == total_economic_benefits`.
pub fn evaluate_twiga_waterfall(
    input: &TwigaWaterfallInput,
) -> Result<TwigaWaterfallResult, EconError> {
    let ccy = input.gross_revenue_100pct.ccy;
    check_ccy(ccy, input.operating_and_capital_costs_100pct.ccy)?;
    check_ccy(ccy, input.royalties_and_fees_paid_to_got.ccy)?;
    check_ccy(ccy, input.corporate_income_tax_paid_to_got.ccy)?;
    check_ccy(ccy, input.other_state_taxes_and_levies.ccy)?;

    if input.gross_revenue_100pct.minor < 0
        || input.operating_and_capital_costs_100pct.minor < 0
        || input.royalties_and_fees_paid_to_got.minor < 0
        || input.corporate_income_tax_paid_to_got.minor < 0
        || input.other_state_taxes_and_levies.minor < 0
    {
        return Err(EconError::InvalidDomain {
            field: "twiga_amounts",
            detail: "Twiga revenue, costs, royalties, and tax inputs must be non-negative"
                .to_string(),
        });
    }
    let eq_shares = [
        input.barrick_equity_fraction,
        input.got_free_carried_fraction,
    ];
    validate_simplex("twiga_equity_shares", &eq_shares)?;
    validate_fraction_unit(
        "got_target_economic_benefit_share",
        input.got_target_economic_benefit_share,
    )?;

    let zero_m = Money::from_minor(ccy, 0);
    let total_economic_benefits = input
        .gross_revenue_100pct
        .checked_sub(input.operating_and_capital_costs_100pct)?;
    let state_direct_take = input
        .royalties_and_fees_paid_to_got
        .checked_add(input.corporate_income_tax_paid_to_got)?
        .checked_add(input.other_state_taxes_and_levies)?;

    if total_economic_benefits.minor <= 0 {
        let funding_deficit = state_direct_take.checked_sub(total_economic_benefits)?;
        return Ok(TwigaWaterfallResult {
            total_economic_benefits,
            state_direct_take,
            pre_true_up_distributable: zero_m,
            funding_deficit,
            got_equity_dividend: zero_m,
            barrick_pre_true_up_dividend: zero_m,
            got_pre_true_up_total: state_direct_take,
            got_target_amount: zero_m,
            annual_true_up_to_got: zero_m,
            true_up_shortfall: zero_m,
            got_final_total: state_direct_take,
            barrick_net_cash: zero_m,
        });
    }

    let got_target_amount = money_mul_exact(
        total_economic_benefits,
        input.got_target_economic_benefit_share,
    )?;
    let (pre_true_up_distributable, funding_deficit) =
        if total_economic_benefits.minor >= state_direct_take.minor {
            (
                total_economic_benefits.checked_sub(state_direct_take)?,
                zero_m,
            )
        } else {
            (
                zero_m,
                state_direct_take.checked_sub(total_economic_benefits)?,
            )
        };

    let slices = allocate_money_exact(pre_true_up_distributable, &eq_shares)?;
    let barrick_pre_true_up_dividend = slices[0];
    let got_equity_dividend = slices[1];
    let got_pre_true_up_total = state_direct_take.checked_add(got_equity_dividend)?;

    let (annual_true_up_to_got, true_up_shortfall) =
        if got_pre_true_up_total.minor < got_target_amount.minor {
            let needed = got_target_amount.checked_sub(got_pre_true_up_total)?;
            if needed.minor <= barrick_pre_true_up_dividend.minor {
                (needed, zero_m)
            } else {
                (
                    barrick_pre_true_up_dividend,
                    needed.checked_sub(barrick_pre_true_up_dividend)?,
                )
            }
        } else {
            (zero_m, zero_m)
        };

    let got_final_total = got_pre_true_up_total.checked_add(annual_true_up_to_got)?;
    let barrick_net_cash = barrick_pre_true_up_dividend.checked_sub(annual_true_up_to_got)?;

    Ok(TwigaWaterfallResult {
        total_economic_benefits,
        state_direct_take,
        pre_true_up_distributable,
        funding_deficit,
        got_equity_dividend,
        barrick_pre_true_up_dividend,
        got_pre_true_up_total,
        got_target_amount,
        annual_true_up_to_got,
        true_up_shortfall,
        got_final_total,
        barrick_net_cash,
    })
}

// ---------------------------------------------------------------------------
// 6. Reko Diq JV DSCR ring-fence & free-carried recovery (§23.2 item 5)
// ---------------------------------------------------------------------------

/// Input for Reko Diq project-finance DSCR lock and free-carried advance recovery (FACT E302–E303).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RekoDiqWaterfallInput {
    /// Cash Flow Available for Debt Service on 100% project basis.
    pub cfads_100pct: Money,
    /// Mandatory scheduled interest + principal service due this period (`>= 0`).
    pub scheduled_debt_service: Money,
    /// Minimum DSCR covenant floor gating equity distributions (`> 0`, e.g., `125/100`).
    pub min_dscr_covenant: ExactQty,
    /// Fraction of post-debt-service cash swept to debt/DSRA when DSCR lock fires (`[0, 1]`).
    pub cash_sweep_fraction_on_lock: ExactQty,
    /// Barrick's unrecovered advance funding Balochistan's 10% free-carried share (`>= 0`).
    pub opening_carry_recovery_balance: Money,
    /// Maximum fraction of Balochistan's 10% free-carried dividend diverted to repay Barrick (`[0, 1]`).
    pub carry_recovery_cap_fraction: ExactQty,
    /// Barrick equity share (`50/100`).
    pub barrick_fraction: ExactQty,
    /// Pakistan federal SOEs equity share (`25/100`).
    pub federal_soe_fraction: ExactQty,
    /// Balochistan funded equity share (`15/100`).
    pub balochistan_funded_fraction: ExactQty,
    /// Balochistan free-carried equity share (`10/100`).
    pub balochistan_free_carried_fraction: ExactQty,
}

/// Exact conserved Reko Diq DSCR ring-fence and carry-recovery output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RekoDiqWaterfallResult {
    /// Exact rational `cfads / scheduled_debt_service`, or `None` when no service is due.
    pub dscr: Option<ExactQty>,
    pub dscr_lock_active: bool,
    pub debt_service_paid: Money,
    pub debt_service_shortfall: Money,
    /// Non-zero when `cfads_100pct < 0`, preserving exact operating cash shortfall.
    pub operating_deficit: Money,
    pub swept_to_debt_or_dsra: Money,
    pub retained_in_lockbox: Money,
    pub barrick_base_distribution: Money,
    pub carry_repaid_to_barrick: Money,
    pub barrick_total_distribution: Money,
    pub federal_soe_distribution: Money,
    pub balochistan_funded_distribution: Money,
    pub balochistan_free_net_distribution: Money,
    pub closing_carry_recovery_balance: Money,
}

/// Evaluate Reko Diq's DSCR distribution lock, cash sweep, and Barrick's recovery of free-carried
/// funding advances, conserving every minor unit of CFADS.
pub fn evaluate_reko_diq_waterfall(
    input: &RekoDiqWaterfallInput,
) -> Result<RekoDiqWaterfallResult, EconError> {
    let ccy = input.cfads_100pct.ccy;
    check_ccy(ccy, input.scheduled_debt_service.ccy)?;
    check_ccy(ccy, input.opening_carry_recovery_balance.ccy)?;

    if input.scheduled_debt_service.minor < 0 || input.opening_carry_recovery_balance.minor < 0 {
        return Err(EconError::InvalidDomain {
            field: "reko_diq_balances",
            detail:
                "scheduled_debt_service and opening_carry_recovery_balance must be non-negative"
                    .to_string(),
        });
    }
    if input.min_dscr_covenant.signum() <= 0 {
        return Err(EconError::InvalidDomain {
            field: "min_dscr_covenant",
            detail: "min_dscr_covenant must be strictly positive".to_string(),
        });
    }
    validate_fraction_unit(
        "cash_sweep_fraction_on_lock",
        input.cash_sweep_fraction_on_lock,
    )?;
    validate_fraction_unit(
        "carry_recovery_cap_fraction",
        input.carry_recovery_cap_fraction,
    )?;
    let shares = [
        input.barrick_fraction,
        input.federal_soe_fraction,
        input.balochistan_funded_fraction,
        input.balochistan_free_carried_fraction,
    ];
    validate_simplex("reko_diq_equity_shares", &shares)?;

    let zero_m = Money::from_minor(ccy, 0);
    let (dscr, dscr_lock_active) = if input.scheduled_debt_service.minor > 0 {
        let ratio = ExactQty::new(input.cfads_100pct.minor, input.scheduled_debt_service.minor)?;
        let locked = ratio.checked_cmp(input.min_dscr_covenant)? == Ordering::Less;
        (Some(ratio), locked)
    } else {
        (None, false)
    };

    if input.cfads_100pct.minor < input.scheduled_debt_service.minor {
        let (paid, operating_deficit) = if input.cfads_100pct.minor >= 0 {
            (input.cfads_100pct, zero_m)
        } else {
            (zero_m, zero_m.checked_sub(input.cfads_100pct)?)
        };
        let shortfall = input.scheduled_debt_service.checked_sub(paid)?;
        return Ok(RekoDiqWaterfallResult {
            dscr,
            dscr_lock_active,
            debt_service_paid: paid,
            debt_service_shortfall: shortfall,
            operating_deficit,
            swept_to_debt_or_dsra: zero_m,
            retained_in_lockbox: zero_m,
            barrick_base_distribution: zero_m,
            carry_repaid_to_barrick: zero_m,
            barrick_total_distribution: zero_m,
            federal_soe_distribution: zero_m,
            balochistan_funded_distribution: zero_m,
            balochistan_free_net_distribution: zero_m,
            closing_carry_recovery_balance: input.opening_carry_recovery_balance,
        });
    }

    let post_debt_cash = input
        .cfads_100pct
        .checked_sub(input.scheduled_debt_service)?;
    if dscr_lock_active {
        let swept = money_mul_exact(post_debt_cash, input.cash_sweep_fraction_on_lock)?;
        let retained = post_debt_cash.checked_sub(swept)?;
        return Ok(RekoDiqWaterfallResult {
            dscr,
            dscr_lock_active: true,
            debt_service_paid: input.scheduled_debt_service,
            debt_service_shortfall: zero_m,
            operating_deficit: zero_m,
            swept_to_debt_or_dsra: swept,
            retained_in_lockbox: retained,
            barrick_base_distribution: zero_m,
            carry_repaid_to_barrick: zero_m,
            barrick_total_distribution: zero_m,
            federal_soe_distribution: zero_m,
            balochistan_funded_distribution: zero_m,
            balochistan_free_net_distribution: zero_m,
            closing_carry_recovery_balance: input.opening_carry_recovery_balance,
        });
    }

    let slices = allocate_money_exact(post_debt_cash, &shares)?;
    let barrick_base = slices[0];
    let federal_soe = slices[1];
    let balochistan_funded = slices[2];
    let balochistan_free_gross = slices[3];

    let max_carry_repay =
        money_mul_exact(balochistan_free_gross, input.carry_recovery_cap_fraction)?;
    let carry_repaid = if input.opening_carry_recovery_balance.minor <= max_carry_repay.minor {
        input.opening_carry_recovery_balance
    } else {
        max_carry_repay
    };
    let barrick_total = barrick_base.checked_add(carry_repaid)?;
    let balochistan_free_net = balochistan_free_gross.checked_sub(carry_repaid)?;
    let closing_carry = input
        .opening_carry_recovery_balance
        .checked_sub(carry_repaid)?;

    Ok(RekoDiqWaterfallResult {
        dscr,
        dscr_lock_active: false,
        debt_service_paid: input.scheduled_debt_service,
        debt_service_shortfall: zero_m,
        operating_deficit: zero_m,
        swept_to_debt_or_dsra: zero_m,
        retained_in_lockbox: zero_m,
        barrick_base_distribution: barrick_base,
        carry_repaid_to_barrick: carry_repaid,
        barrick_total_distribution: barrick_total,
        federal_soe_distribution: federal_soe,
        balochistan_funded_distribution: balochistan_funded,
        balochistan_free_net_distribution: balochistan_free_net,
        closing_carry_recovery_balance: closing_carry,
    })
}

// ---------------------------------------------------------------------------
// 7. Loulo-Gounkoto H1 2026 reset & Tongon divestiture fixtures (§23.4)
// ---------------------------------------------------------------------------

/// Input for Loulo-Gounkoto post-settlement 2023 Code fiscal reset (FACT E401–E414, E483, E670).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LouloGounkotoResetInput {
    /// Attributable gold production on Barrick's equity basis (e.g., H1 2026: `152_000` oz @ 80%).
    pub attributable_gold_oz: ExactQty,
    /// Barrick's equity fraction (`80/100`; Mali holds `20/100`).
    pub barrick_equity_fraction: ExactQty,
    /// 100%-basis operating cost before royalties, CIT, and one-time settlement charges.
    pub operating_cost_100pct: Money,
    /// 100%-basis retroactive 2024–2025 royalty/tax/penalty charge (FACT E412–E413: `$200M` Apr 2026).
    pub retroactive_tax_and_penalty_100pct: Money,
    /// 100%-basis settlement payment (FACT E407: `$253M` paid 28 Nov 2025).
    pub settlement_payment_100pct: Money,
}

/// Conserved evaluation of Loulo-Gounkoto under the post-settlement 2023 Mining Code reset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LouloGounkotoResetResult {
    pub gross_gold_oz_100pct: ExactQty,
    pub fiscal: RegimeFiscalEvaluation,
    pub after_tax_before_onetime_100pct: Money,
    pub one_time_charges_100pct: Money,
    pub net_post_reset_cash_100pct: Money,
    pub barrick_attributable_net_cash: Money,
    pub mali_state_equity_net_cash: Money,
}

/// Evaluate Loulo-Gounkoto under a registered [`CountryRegimeRow`].
///
/// Refuses `mali_loulo_2025_not_published` with `EconError::NotPublished` (`FMINE-REF-201`) and
/// refuses superseded 1991-Code rows in live mode (`FMINE-REF-502`), while evaluating the H1 2026
/// post-settlement 2023 Code reset with exact 80/20 conservation.
pub fn evaluate_loulo_gounkoto_reset(
    regime: &CountryRegimeRow,
    deck: &PriceDeck,
    input: &LouloGounkotoResetInput,
    allow_historical: bool,
) -> Result<LouloGounkotoResetResult, EconError> {
    let ccy = input.operating_cost_100pct.ccy;
    check_ccy(ccy, input.retroactive_tax_and_penalty_100pct.ccy)?;
    check_ccy(ccy, input.settlement_payment_100pct.ccy)?;
    validate_fraction_positive("barrick_equity_fraction", input.barrick_equity_fraction)?;
    if input
        .barrick_equity_fraction
        .checked_add(regime.state_equity_fraction)?
        .checked_cmp(ExactQty::from_integer(1))?
        == Ordering::Greater
    {
        return Err(EconError::InvalidDomain {
            field: "barrick_equity_fraction",
            detail: format!(
                "barrick_equity_fraction + regime '{}' state_equity_fraction exceeds 100%",
                regime.id
            ),
        });
    }
    if input.attributable_gold_oz.signum() < 0
        || input.retroactive_tax_and_penalty_100pct.minor < 0
        || input.settlement_payment_100pct.minor < 0
    {
        return Err(EconError::InvalidDomain {
            field: "loulo_reset",
            detail: "attributable_gold_oz and one-time charges must be non-negative".to_string(),
        });
    }
    if regime.state == crate::regime::RegimeState::Suspension
        && input.attributable_gold_oz.signum() > 0
    {
        return Err(EconError::InvalidDomain {
            field: "regime.state",
            detail: format!(
                "cannot evaluate active production on suspended regime '{}'",
                regime.id
            ),
        });
    }

    let gross_gold_oz_100pct = input
        .attributable_gold_oz
        .checked_div(input.barrick_equity_fraction)?;
    let fiscal = evaluate_regime_fiscal_terms(
        regime,
        deck,
        Commodity::Au,
        gross_gold_oz_100pct,
        input.operating_cost_100pct,
        allow_historical,
    )?;

    let one_time_charges_100pct = input
        .retroactive_tax_and_penalty_100pct
        .checked_add(input.settlement_payment_100pct)?;
    let after_tax_before_onetime_100pct = fiscal.breakdown.after_tax_cashflow;
    let net_post_reset_cash_100pct =
        after_tax_before_onetime_100pct.checked_sub(one_time_charges_100pct)?;

    let barrick_attributable_net_cash =
        money_mul_exact(net_post_reset_cash_100pct, input.barrick_equity_fraction)?;
    let mali_state_equity_net_cash =
        net_post_reset_cash_100pct.checked_sub(barrick_attributable_net_cash)?;

    Ok(LouloGounkotoResetResult {
        gross_gold_oz_100pct,
        fiscal,
        after_tax_before_onetime_100pct,
        one_time_charges_100pct,
        net_post_reset_cash_100pct,
        barrick_attributable_net_cash,
        mali_state_equity_net_cash,
    })
}

/// Evaluation of a single reporting period `[period_start_date, period_end_date]` for a divested
/// asset (e.g., Tongon closed 2025-12-01; Hemlo closed 2025-11-26; FACT E485–E488, E553–E554).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DivestiturePeriodOutcome {
    pub asset_id: EntityNodeId,
    pub period_start_date: String,
    pub period_end_date: String,
    pub closed_date: String,
    pub post_divestiture_terminated: bool,
    pub recognized_operating_cashflow: Money,
    pub recognized_exit_proceeds: Money,
    pub suppressed_stale_operating_cashflow: Money,
}

/// Enforce divestiture termination over `[period_start_date, period_end_date]` so forward
/// operating series never silently carry forward past a closed exit date (`Tongon` closed
/// `2025-12-01`, up to `$305M` proceeds; E485–E488).
///
/// - When `period_start_date > closed_date` (strictly post-divestiture, e.g., Q1 2026): closing
///   proceeds must be zero; if `strict_refuse_post_exit_nonzero` is `true` and
///   `projected_operating_cashflow != 0`, refuses with `FMINE-REF-109`; otherwise zeros operating
///   cash flow and records the suppressed stale amount.
/// - When `period_start_date <= closed_date <= period_end_date` (closing period, e.g., Q4 2025
///   `2025-10-01..2025-12-31` or exact day `2025-12-01..2025-12-01`): admits `closing_proceeds`
///   (`0 <= proceeds <= max_proceeds`) alongside final pre-close stub operating cash flow.
/// - When `period_end_date < closed_date` (strictly pre-divestiture): passes operating cash flow
///   and requires zero `closing_proceeds`.
pub fn evaluate_divested_asset_period(
    node: &EntityNode,
    edge: &WaterfallEdge,
    period_start_date: &str,
    period_end_date: &str,
    projected_operating_cashflow: Money,
    closing_proceeds: Money,
    strict_refuse_post_exit_nonzero: bool,
) -> Result<DivestiturePeriodOutcome, EconError> {
    if node.kind != EntityKind::DivestedAsset {
        return Err(EconError::InvalidDomain {
            field: "node.kind",
            detail: format!(
                "expected DivestedAsset for '{}', found {:?}",
                node.id.as_str(),
                node.kind
            ),
        });
    }
    if edge.from_child != node.id {
        return Err(EconError::InvalidDomain {
            field: "edge.from_child",
            detail: format!(
                "edge child '{}' does not match node '{}'",
                edge.from_child.as_str(),
                node.id.as_str()
            ),
        });
    }
    let WaterfallEdgeKind::DivestitureExit {
        closed_date,
        max_proceeds,
    } = &edge.kind
    else {
        return Err(EconError::InvalidDomain {
            field: "edge.kind",
            detail: "divested asset evaluation requires a DivestitureExit edge".to_string(),
        });
    };
    if !is_iso_date(period_start_date) {
        return Err(EconError::InvalidDomain {
            field: "period_start_date",
            detail: format!("invalid YYYY-MM-DD period_start_date {period_start_date:?}"),
        });
    }
    if !is_iso_date(period_end_date) {
        return Err(EconError::InvalidDomain {
            field: "period_end_date",
            detail: format!("invalid YYYY-MM-DD period_end_date {period_end_date:?}"),
        });
    }
    if period_start_date > period_end_date {
        return Err(EconError::InvalidDomain {
            field: "period_start_date",
            detail: format!(
                "period_start_date '{period_start_date}' > period_end_date '{period_end_date}'"
            ),
        });
    }
    if !is_iso_date(closed_date) {
        return Err(EconError::InvalidDomain {
            field: "closed_date",
            detail: format!("invalid YYYY-MM-DD closed_date {closed_date:?}"),
        });
    }
    if node.divested_on.as_deref() != Some(closed_date.as_str()) {
        return Err(EconError::InvalidDomain {
            field: "divested_on",
            detail: format!(
                "node divested_on {:?} != edge closed_date '{}'",
                node.divested_on, closed_date
            ),
        });
    }
    check_ccy(node.currency, max_proceeds.ccy)?;
    check_ccy(node.currency, projected_operating_cashflow.ccy)?;
    check_ccy(node.currency, closing_proceeds.ccy)?;

    if closing_proceeds.minor < 0 {
        return Err(EconError::InvalidDomain {
            field: "closing_proceeds",
            detail: "closing_proceeds must be non-negative".to_string(),
        });
    }
    if closing_proceeds.minor > max_proceeds.minor {
        return Err(EconError::InvalidDomain {
            field: "closing_proceeds",
            detail: format!(
                "closing_proceeds ({}) exceeds contracted max_proceeds ({})",
                closing_proceeds.minor, max_proceeds.minor
            ),
        });
    }

    let zero_m = Money::from_minor(node.currency, 0);
    if period_start_date > closed_date.as_str() {
        if closing_proceeds.minor != 0 {
            return Err(EconError::InvalidDomain {
                field: "closing_proceeds",
                detail: "cannot recognize closing proceeds strictly after closed_date".to_string(),
            });
        }
        if strict_refuse_post_exit_nonzero && projected_operating_cashflow.minor != 0 {
            return Err(EconError::InvalidDomain {
                field: "projected_operating_cashflow",
                detail: format!(
                    "non-zero operating cash flow on '{}' after divestiture date {}",
                    node.id.as_str(),
                    closed_date
                ),
            });
        }
        Ok(DivestiturePeriodOutcome {
            asset_id: node.id.clone(),
            period_start_date: period_start_date.to_string(),
            period_end_date: period_end_date.to_string(),
            closed_date: closed_date.clone(),
            post_divestiture_terminated: true,
            recognized_operating_cashflow: zero_m,
            recognized_exit_proceeds: zero_m,
            suppressed_stale_operating_cashflow: projected_operating_cashflow,
        })
    } else if period_end_date < closed_date.as_str() {
        if closing_proceeds.minor != 0 {
            return Err(EconError::InvalidDomain {
                field: "closing_proceeds",
                detail: "cannot recognize closing proceeds before closed_date".to_string(),
            });
        }
        Ok(DivestiturePeriodOutcome {
            asset_id: node.id.clone(),
            period_start_date: period_start_date.to_string(),
            period_end_date: period_end_date.to_string(),
            closed_date: closed_date.clone(),
            post_divestiture_terminated: false,
            recognized_operating_cashflow: projected_operating_cashflow,
            recognized_exit_proceeds: zero_m,
            suppressed_stale_operating_cashflow: zero_m,
        })
    } else {
        Ok(DivestiturePeriodOutcome {
            asset_id: node.id.clone(),
            period_start_date: period_start_date.to_string(),
            period_end_date: period_end_date.to_string(),
            closed_date: closed_date.clone(),
            post_divestiture_terminated: false,
            recognized_operating_cashflow: projected_operating_cashflow,
            recognized_exit_proceeds: closing_proceeds,
            suppressed_stale_operating_cashflow: zero_m,
        })
    }
}

/// Build the documented 2026 Barrick public-lane portfolio [`CapitalEntityGraph`] fixture
/// (`FACT` ownership bases + `DESIGN` illustrative `NA-NewCo` minority carve-out; no affiliation).
pub fn barrick_2026_portfolio_graph(
    parent_retained_newco_fraction: ExactQty,
    kibali_repatriable_fraction: ExactQty,
    reko_diq_min_dscr: ExactQty,
) -> Result<CapitalEntityGraph, EconError> {
    let parent = EntityNodeId::new("barrick_parent")?;
    let newco = EntityNodeId::new("na_newco")?;
    let ngm = EntityNodeId::new("ngm_jv")?;
    let pv = EntityNodeId::new("pv_jv")?;
    let fourmile = EntityNodeId::new("fourmile")?;
    let kibali = EntityNodeId::new("kibali_jv")?;
    let twiga = EntityNodeId::new("twiga_jv")?;
    let reko_diq = EntityNodeId::new("reko_diq_jv")?;
    let loulo = EntityNodeId::new("loulo_gounkoto_jv")?;
    let tongon = EntityNodeId::new("tongon_divested")?;

    let graph = CapitalEntityGraph {
        graph_id: "barrick_public_portfolio_2026_v1".to_string(),
        as_of_date: "2026-08-10".to_string(),
        nodes: vec![
            EntityNode {
                id: parent.clone(),
                kind: EntityKind::Parent,
                name: "Barrick Parent (Public Fixture)".to_string(),
                share_count: Some(1_750_000_000),
                currency: Currency::USD,
                country_regime_id: None,
                divested_on: None,
                source_e_ids: vec!["E658".to_string()],
            },
            EntityNode {
                id: newco.clone(),
                kind: EntityKind::CarveOut,
                name: "NA-NewCo IPO Carve-Out Perimeter (DESIGN)".to_string(),
                share_count: Some(500_000_000),
                currency: Currency::USD,
                country_regime_id: None,
                divested_on: None,
                source_e_ids: vec!["E629".to_string(), "E658".to_string()],
            },
            EntityNode {
                id: ngm.clone(),
                kind: EntityKind::NgmJv,
                name: "Nevada Gold Mines JV (61.5% / 38.5%)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: None,
                divested_on: None,
                source_e_ids: vec!["E457".to_string()],
            },
            EntityNode {
                id: pv.clone(),
                kind: EntityKind::PvJv,
                name: "Pueblo Viejo JV (60% / 40%)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: None,
                divested_on: None,
                source_e_ids: vec!["E672".to_string()],
            },
            EntityNode {
                id: fourmile.clone(),
                kind: EntityKind::WhollyOwnedAsset,
                name: "Fourmile Discovery (100% pre-vend-in)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: None,
                divested_on: None,
                source_e_ids: vec!["E139".to_string(), "E141".to_string(), "E658".to_string()],
            },
            EntityNode {
                id: kibali.clone(),
                kind: EntityKind::KibaliJv,
                name: "Kibali JV (45% / 45% / 10% SOKIMO)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: Some("drc_kibali_2026".to_string()),
                divested_on: None,
                source_e_ids: vec!["E242".to_string(), "E243".to_string()],
            },
            EntityNode {
                id: twiga.clone(),
                kind: EntityKind::TwigaJv,
                name: "Twiga JV (North Mara & Bulyanhulu 84% / 16% + 50/50)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: Some("tanzania_twiga_2020".to_string()),
                divested_on: None,
                source_e_ids: vec!["E448".to_string()],
            },
            EntityNode {
                id: reko_diq.clone(),
                kind: EntityKind::RekoDiqJv,
                name: "Reko Diq JV (50% / 25% / 15% + 10% free-carried)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: None,
                divested_on: None,
                source_e_ids: vec!["E302".to_string(), "E303".to_string()],
            },
            EntityNode {
                id: loulo.clone(),
                kind: EntityKind::LouloGounkotoJv,
                name: "Loulo-Gounkoto JV (80% / 20% Mali, 2023 Code reset)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: Some("mali_loulo_2026".to_string()),
                divested_on: None,
                source_e_ids: vec![
                    "E410".to_string(),
                    "E414".to_string(),
                    "E483".to_string(),
                    "E638".to_string(),
                    "E670".to_string(),
                ],
            },
            EntityNode {
                id: tongon.clone(),
                kind: EntityKind::DivestedAsset,
                name: "Tongon (Divested to Atlantic Group, closed 2025-12-01)".to_string(),
                share_count: None,
                currency: Currency::USD,
                country_regime_id: None,
                divested_on: Some("2025-12-01".to_string()),
                source_e_ids: vec![
                    "E485".to_string(),
                    "E488".to_string(),
                    "E553".to_string(),
                    "E554".to_string(),
                ],
            },
        ],
        edges: vec![
            WaterfallEdge {
                from_child: newco.clone(),
                to_parent: parent.clone(),
                kind: WaterfallEdgeKind::DirectEquity {
                    equity_fraction: parent_retained_newco_fraction,
                },
                source_e_ids: vec!["E658".to_string()],
            },
            WaterfallEdge {
                from_child: ngm,
                to_parent: newco.clone(),
                kind: WaterfallEdgeKind::JvCapitalCall {
                    initial_equity_fraction: ExactQty::new(615, 1_000)?,
                    default_dilution_enabled: true,
                },
                source_e_ids: vec!["E457".to_string(), "E658".to_string()],
            },
            WaterfallEdge {
                from_child: pv,
                to_parent: newco.clone(),
                kind: WaterfallEdgeKind::JvCapitalCall {
                    initial_equity_fraction: ExactQty::new(60, 100)?,
                    default_dilution_enabled: true,
                },
                source_e_ids: vec!["E658".to_string(), "E672".to_string()],
            },
            WaterfallEdge {
                from_child: fourmile,
                to_parent: newco,
                kind: WaterfallEdgeKind::DirectEquity {
                    equity_fraction: ExactQty::from_integer(1),
                },
                source_e_ids: vec!["E139".to_string(), "E658".to_string()],
            },
            WaterfallEdge {
                from_child: kibali,
                to_parent: parent.clone(),
                kind: WaterfallEdgeKind::EquityAccountedRepatriation {
                    equity_fraction: ExactQty::new(45, 100)?,
                    repatriable_fraction: kibali_repatriable_fraction,
                },
                source_e_ids: vec!["E242".to_string(), "E243".to_string()],
            },
            WaterfallEdge {
                from_child: twiga,
                to_parent: parent.clone(),
                kind: WaterfallEdgeKind::EconomicBenefitsSharing {
                    equity_fraction: ExactQty::new(84, 100)?,
                    state_target_share: ExactQty::new(50, 100)?,
                },
                source_e_ids: vec!["E448".to_string()],
            },
            WaterfallEdge {
                from_child: reko_diq,
                to_parent: parent.clone(),
                kind: WaterfallEdgeKind::ProjectFinanceDscrLock {
                    equity_fraction: ExactQty::new(50, 100)?,
                    free_carried_fraction: ExactQty::new(10, 100)?,
                    min_dscr: reko_diq_min_dscr,
                },
                source_e_ids: vec!["E302".to_string(), "E303".to_string()],
            },
            WaterfallEdge {
                from_child: loulo,
                to_parent: parent.clone(),
                kind: WaterfallEdgeKind::DirectEquity {
                    equity_fraction: ExactQty::new(80, 100)?,
                },
                source_e_ids: vec!["E410".to_string(), "E670".to_string()],
            },
            WaterfallEdge {
                from_child: tongon,
                to_parent: parent,
                kind: WaterfallEdgeKind::DivestitureExit {
                    closed_date: "2025-12-01".to_string(),
                    max_proceeds: Money::from_major(Currency::USD, 305_000_000),
                },
                source_e_ids: vec!["E485".to_string(), "E488".to_string()],
            },
        ],
    };
    graph.validate()?;
    Ok(graph)
}

// ---------------------------------------------------------------------------
// Internal exact helpers (zero f64, deterministic largest-remainder split)
// ---------------------------------------------------------------------------

fn check_ccy(left: Currency, right: Currency) -> Result<(), EconError> {
    if left != right {
        return Err(EconError::CurrencyMismatch { left, right });
    }
    Ok(())
}

fn currency_code(ccy: Currency) -> &'static str {
    match ccy {
        Currency::USD => "USD",
        Currency::CAD => "CAD",
        Currency::AUD => "AUD",
        Currency::ZAR => "ZAR",
        Currency::PKR => "PKR",
        Currency::CLP => "CLP",
        Currency::PEN => "PEN",
        Currency::ARS => "ARS",
        Currency::TZS => "TZS",
        Currency::XAF => "XAF",
        Currency::CDF => "CDF",
        Currency::ZMW => "ZMW",
        Currency::PGK => "PGK",
        Currency::DOP => "DOP",
        Currency::SAR => "SAR",
        Currency::EUR => "EUR",
        Currency::JPY => "JPY",
    }
}

fn put_qty(enc: &mut Encoder, qty: ExactQty) {
    enc.put_i128(qty.num());
    enc.put_i128(qty.den());
}

fn money_as_major_qty(money: Money) -> Result<ExactQty, EconError> {
    Ok(ExactQty::new(money.minor, Money::SCALE)?)
}

fn validate_fraction_unit(field: &'static str, qty: ExactQty) -> Result<(), EconError> {
    let one = ExactQty::from_integer(1);
    if qty.signum() < 0 || qty.checked_cmp(one)? == Ordering::Greater {
        return Err(EconError::InvalidDomain {
            field,
            detail: "fraction must lie in [0, 1]".to_string(),
        });
    }
    Ok(())
}

fn validate_fraction_positive(field: &'static str, qty: ExactQty) -> Result<(), EconError> {
    let one = ExactQty::from_integer(1);
    if qty.signum() <= 0 || qty.checked_cmp(one)? == Ordering::Greater {
        return Err(EconError::InvalidDomain {
            field,
            detail: "equity fraction must lie in (0, 1]".to_string(),
        });
    }
    Ok(())
}

fn validate_simplex(field: &'static str, weights: &[ExactQty]) -> Result<(), EconError> {
    let mut sum = ExactQty::from_integer(0);
    for &w in weights {
        validate_fraction_unit(field, w)?;
        sum = sum.checked_add(w)?;
    }
    if sum.checked_cmp(ExactQty::from_integer(1))? != Ordering::Equal {
        return Err(EconError::InvalidDomain {
            field,
            detail: format!(
                "equity shares must sum exactly to 1, got {}/{}",
                sum.num(),
                sum.den()
            ),
        });
    }
    Ok(())
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9];
    if !digits.iter().all(|&i| b[i].is_ascii_digit()) {
        return false;
    }
    let Ok(year) = s[0..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = s[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = s[8..10].parse::<u32>() else {
        return false;
    };
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    (1..=max_day).contains(&day)
}

/// Deterministic largest-remainder exact `Money` allocation conserving `sum(out) == total` with
/// zero residual across a validated simplex of `ExactQty` weights.
fn allocate_money_exact(total: Money, weights: &[ExactQty]) -> Result<Vec<Money>, EconError> {
    if total.minor < 0 {
        return Err(EconError::InvalidDomain {
            field: "allocate_money_exact",
            detail: "cannot allocate negative total with simplex allocator".to_string(),
        });
    }
    validate_simplex("weights", weights)?;

    let mut floors: Vec<i128> = Vec::with_capacity(weights.len());
    let mut rems: Vec<(usize, ExactQty)> = Vec::with_capacity(weights.len());
    let mut floor_sum: i128 = 0;

    for (idx, &w) in weights.iter().enumerate() {
        let exact = ExactQty::from_integer(total.minor).checked_mul(w)?;
        let q = exact.num() / exact.den();
        let r = exact.num() % exact.den();
        floors.push(q);
        floor_sum = floor_sum.checked_add(q).ok_or(EconError::Overflow)?;
        rems.push((idx, ExactQty::new(r, exact.den())?));
    }

    let leftover = total
        .minor
        .checked_sub(floor_sum)
        .ok_or(EconError::Overflow)?;
    if leftover < 0 || leftover as usize > weights.len() {
        return Err(EconError::Overflow);
    }

    // Sort by largest fractional remainder descending, ties broken by lowest index.
    let mut ordered_indices: Vec<usize> = (0..weights.len()).collect();
    for i in 0..ordered_indices.len() {
        for j in (i + 1)..ordered_indices.len() {
            let a = ordered_indices[i];
            let b = ordered_indices[j];
            let cmp = rems[b].1.checked_cmp(rems[a].1)?;
            if cmp == Ordering::Greater || (cmp == Ordering::Equal && b < a) {
                ordered_indices.swap(i, j);
            }
        }
    }

    for &idx in ordered_indices.iter().take(leftover as usize) {
        floors[idx] = floors[idx].checked_add(1).ok_or(EconError::Overflow)?;
    }

    Ok(floors
        .into_iter()
        .map(|m| Money::from_minor(total.ccy, m))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::price_deck::{PricePoint, PriceUnit};
    use crate::regime::parse_country_regime_registry;

    fn usd(major: i128) -> Money {
        Money::from_major(Currency::USD, major)
    }

    fn qty(n: i128, d: i128) -> ExactQty {
        ExactQty::new(n, d).expect("valid ExactQty")
    }

    fn gold_deck(id: &str, usd_per_oz: i128) -> PriceDeck {
        PriceDeck::build(
            id,
            1,
            "2026-08-10",
            Currency::USD,
            vec![
                PricePoint::new(Commodity::Au, PriceUnit::PerTroyOz, usd(usd_per_oz))
                    .expect("valid Au price"),
            ],
            None,
        )
        .expect("build gold deck")
    }

    fn load_workspace_regimes() -> Vec<CountryRegimeRow> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text = std::fs::read_to_string(root.join("registries/country_regimes.toml"))
            .expect("read registries/country_regimes.toml");
        parse_country_regime_registry(&text).expect("parse country_regimes.toml")
    }

    #[test]
    fn test_barrick_portfolio_graph_topology_effective_equity_and_digest_invariance() {
        let graph = barrick_2026_portfolio_graph(qty(85, 100), qty(70, 100), qty(125, 100))
            .expect("valid barrick 2026 graph");

        let parent = EntityNodeId::new("barrick_parent").unwrap();
        let newco = EntityNodeId::new("na_newco").unwrap();
        let ngm = EntityNodeId::new("ngm_jv").unwrap();
        let pv = EntityNodeId::new("pv_jv").unwrap();
        let kibali = EntityNodeId::new("kibali_jv").unwrap();
        let tongon = EntityNodeId::new("tongon_divested").unwrap();

        // Direct NA-NewCo holdings: NGM 61.5% (E457), PV 60% (E672).
        assert_eq!(
            graph.effective_equity_fraction(&ngm, &newco).unwrap(),
            qty(615, 1_000)
        );
        assert_eq!(
            graph.effective_equity_fraction(&pv, &newco).unwrap(),
            qty(60, 100)
        );
        // Through 85%-retained NA-NewCo into Barrick Parent:
        // NGM -> Parent = 0.615 * 0.85 = 522.75 / 1000 = 2091 / 4000.
        assert_eq!(
            graph.effective_equity_fraction(&ngm, &parent).unwrap(),
            qty(2_091, 4_000)
        );
        // PV -> Parent = 0.60 * 0.85 = 51 / 100.
        assert_eq!(
            graph.effective_equity_fraction(&pv, &parent).unwrap(),
            qty(51, 100)
        );
        // Kibali direct to Parent = 45%.
        assert_eq!(
            graph.effective_equity_fraction(&kibali, &parent).unwrap(),
            qty(45, 100)
        );
        // Divested Tongon carries exact 0% ongoing equity.
        assert_eq!(
            graph.effective_equity_fraction(&tongon, &parent).unwrap(),
            ExactQty::from_integer(0)
        );

        // Permuting node and edge order preserves the canonical SHA-256 digest.
        let mut permuted = graph.clone();
        permuted.nodes.reverse();
        permuted.edges.reverse();
        assert_eq!(graph.digest().unwrap(), permuted.digest().unwrap());

        // Cycle refusal (FMINE-REF-109).
        let mut cyclic = graph.clone();
        cyclic.edges.push(WaterfallEdge {
            from_child: parent.clone(),
            to_parent: newco.clone(),
            kind: WaterfallEdgeKind::DirectEquity {
                equity_fraction: qty(10, 100),
            },
            source_e_ids: vec!["SYNTH".to_string()],
        });
        let err = cyclic.validate().expect_err("cycle must refuse");
        assert_eq!(err.to_refusal().code, "FMINE-REF-109");

        // Double-counting an asset across both NA-NewCo and Parent (> 100% outgoing equity) refuses.
        let mut double_counted = graph;
        double_counted.edges.push(WaterfallEdge {
            from_child: ngm,
            to_parent: parent,
            kind: WaterfallEdgeKind::DirectEquity {
                equity_fraction: qty(615, 1_000),
            },
            source_e_ids: vec!["SYNTH".to_string()],
        });
        let err2 = double_counted
            .validate()
            .expect_err("double-counted equity > 100% must refuse");
        assert_eq!(err2.to_refusal().code, "FMINE-REF-109");
    }

    #[test]
    fn test_dual_entity_delta_nav_and_fourmile_vend_in_vs_tolling() {
        let parent_id = EntityNodeId::new("barrick_parent").unwrap();
        let newco_id = EntityNodeId::new("na_newco").unwrap();

        // Scenario: Equity-funded growth project that is accretive on Parent (no Parent share
        // issuance, +$200M ΔNAV on 1,000M shares = +$0.20/sh) but dilutive on NA-NewCo (+$200M
        // NAV on $10,000M base with +50M new shares on 500M base: 10200/550 - 10000/500 = -$1.4545/sh).
        let parent_in = EntityDenominatorInput {
            entity_id: parent_id.clone(),
            baseline_nav: usd(35_000_000_000),
            baseline_shares: 1_000_000_000,
            delta_nav: usd(200_000_000),
            delta_shares: 0,
            post_aisc_total_cost: usd(5_250_000_000),
            post_payable_ounces_sold: ExactQty::from_integer(3_500_000),
            post_annual_fcf: usd(2_800_000_000),
            post_equity_value: usd(40_000_000_000),
        };
        let newco_in = EntityDenominatorInput {
            entity_id: newco_id.clone(),
            baseline_nav: usd(10_000_000_000),
            baseline_shares: 500_000_000,
            delta_nav: usd(200_000_000),
            delta_shares: 50_000_000,
            post_aisc_total_cost: usd(2_800_000_000),
            post_payable_ounces_sold: ExactQty::from_integer(2_000_000),
            post_annual_fcf: usd(1_500_000_000),
            post_equity_value: usd(15_000_000_000),
        };

        let dual = evaluate_dual_entity_delta_nav("cand_newco_equity_raise", &parent_in, &newco_in)
            .expect("dual evaluation");
        assert_eq!(dual.parent.delta_nav_per_share, qty(1, 5)); // +$0.20/share exact
        assert_eq!(dual.parent.post_aisc_per_oz, ExactQty::from_integer(1_500));
        assert_eq!(dual.parent.post_fcf_yield, qty(7, 100)); // 7.0% exact
        // NA-NewCo: 10200/550 - 20 = (10200 - 11000)/550 = -800/550 = -16/11 $/share exact.
        assert_eq!(dual.carve_out.delta_nav_per_share, qty(-16, 11));
        assert_eq!(
            dual.carve_out.post_aisc_per_oz,
            ExactQty::from_integer(1_400)
        );
        assert_eq!(dual.carve_out.post_fcf_yield, qty(1, 10)); // 10.0% exact
        assert!(dual.divergent_accretion);

        // Fourmile vend-in vs. inter-entity tolling option (FACT E139, E141, E658; illustrative values):
        // Standalone Fourmile NAV = $4,000M; NGM synergy = $600M (100% basis);
        // NGM Barrick share = 61.5%; Parent retains 85% of NA-NewCo.
        // Tolling fee PV = $1,000M -> Newmont leakage = 38.5% * $1,000M = $385M; standalone penalty = $215M.
        // -> Tolling NA-NewCo ΔNAV = 4000 - 385 - 215 = $3,400M.
        // -> Vend-in without FMV payment = 0.615 * 4600 = $2,829M.
        // -> Exact breakeven Newmont FMV payment = 3400 - 2829 = $571M!
        let mut fm = FourmileOptionInput {
            parent_id,
            parent_shares: 1_750_000_000,
            newco_id,
            newco_shares: 500_000_000,
            parent_retained_newco_fraction: qty(85, 100),
            ngm_barrick_fraction: qty(615, 1_000),
            fourmile_standalone_project_nav: usd(4_000_000_000),
            vend_in_fmv_payment_from_newmont: usd(700_000_000),
            vend_in_synergy_nav_uplift_100pct: usd(600_000_000),
            tolling_fee_pv_paid_to_ngm: usd(1_000_000_000),
            tolling_standalone_penalty_pv: usd(215_000_000),
        };
        let cmp = compare_fourmile_vend_in_vs_tolling(&fm).expect("compare fourmile");
        assert!(cmp.walled_transfer_price);
        assert_eq!(cmp.tolling_newco_net_leakage, usd(385_000_000));
        assert_eq!(cmp.tolling_newco_delta_nav, usd(3_400_000_000));
        assert_eq!(cmp.vend_in_newco_delta_nav, usd(3_529_000_000));
        assert_eq!(cmp.vend_in_advantage_newco, usd(129_000_000));
        assert_eq!(cmp.breakeven_newmont_fmv_payment, usd(571_000_000));
        assert_eq!(cmp.preferred_for_newco, FourmilePreferredPath::VendInToNgm);
        assert_eq!(cmp.preferred_for_parent, FourmilePreferredPath::VendInToNgm);

        // At exact breakeven payment ($571M), both paths are indifferent on both denominators.
        fm.vend_in_fmv_payment_from_newmont = cmp.breakeven_newmont_fmv_payment;
        let eq_cmp = compare_fourmile_vend_in_vs_tolling(&fm).expect("breakeven compare");
        assert_eq!(eq_cmp.vend_in_advantage_newco, usd(0));
        assert_eq!(eq_cmp.vend_in_advantage_parent, usd(0));
        assert_eq!(
            eq_cmp.preferred_for_newco,
            FourmilePreferredPath::Indifferent
        );
        assert_eq!(
            eq_cmp.preferred_for_parent,
            FourmilePreferredPath::Indifferent
        );

        // Below breakeven ($400M FMV payment), tolling beats vend-in by $171M on NA-NewCo.
        fm.vend_in_fmv_payment_from_newmont = usd(400_000_000);
        let toll_cmp = compare_fourmile_vend_in_vs_tolling(&fm).expect("tolling wins");
        assert_eq!(toll_cmp.vend_in_advantage_newco, usd(-171_000_000));
        assert_eq!(
            toll_cmp.preferred_for_newco,
            FourmilePreferredPath::InterEntityTolling
        );
    }

    #[test]
    fn test_ngm_and_pv_capital_call_dilution_conservation() {
        let ngm_id = EntityNodeId::new("ngm_jv").unwrap();
        let one = ExactQty::from_integer(1);

        // 1. Pro-rata funded NGM capital call ($1,000M: Barrick $615M, Newmont $385M) -> zero dilution.
        let prorata = evaluate_jv_capital_call(&JvCapitalCallInput {
            jv_id: ngm_id.clone(),
            prior_operator_share: qty(615, 1_000),
            prior_total_capital_account: usd(10_000_000_000),
            total_call_required: usd(1_000_000_000),
            operator_contribution: usd(615_000_000),
            partner_contribution: usd(385_000_000),
            non_consent_penalty_multiplier: one,
        })
        .expect("pro-rata call");
        assert!(prorata.fully_funded);
        assert_eq!(prorata.funding_shortfall, usd(0));
        assert!(!prorata.dilution_triggered);
        assert_eq!(prorata.new_operator_share, qty(615, 1_000));
        assert_eq!(
            prorata
                .new_operator_share
                .checked_add(prorata.new_partner_share)
                .unwrap(),
            one
        );

        // 2. Non-consenting partner (Barrick funds full $1,000M on $10,000M base, straight dilution):
        // new_barrick = (6150 + 1000) / 11000 = 7150 / 11000 = 13/20 = 65.00% exact!
        let straight = evaluate_jv_capital_call(&JvCapitalCallInput {
            jv_id: ngm_id.clone(),
            prior_operator_share: qty(615, 1_000),
            prior_total_capital_account: usd(10_000_000_000),
            total_call_required: usd(1_000_000_000),
            operator_contribution: usd(1_000_000_000),
            partner_contribution: usd(0),
            non_consent_penalty_multiplier: one,
        })
        .expect("straight dilution");
        assert!(straight.fully_funded);
        assert!(straight.dilution_triggered);
        assert_eq!(straight.new_operator_share, qty(13, 20));
        assert_eq!(straight.new_partner_share, qty(7, 20));

        // 3. Contractual 2x penalty dilution: Barrick covers Newmont's $385M shortfall -> extra
        // $385M transferred from partner capital account: (7150 + 385) / 11000 = 7535 / 11000 = 137/200 = 68.5%!
        let penalty = evaluate_jv_capital_call(&JvCapitalCallInput {
            jv_id: ngm_id,
            prior_operator_share: qty(615, 1_000),
            prior_total_capital_account: usd(10_000_000_000),
            total_call_required: usd(1_000_000_000),
            operator_contribution: usd(1_000_000_000),
            partner_contribution: usd(0),
            non_consent_penalty_multiplier: ExactQty::from_integer(2),
        })
        .expect("2x penalty dilution");
        assert_eq!(penalty.new_operator_share, qty(137, 200));
        assert_eq!(penalty.new_partner_share, qty(63, 200));
        assert_eq!(
            penalty
                .new_operator_share
                .checked_add(penalty.new_partner_share)
                .unwrap(),
            one
        );
    }

    #[test]
    fn test_kibali_twiga_and_reko_diq_jv_waterfalls_exact_conservation() {
        let regimes = load_workspace_regimes();
        let kibali_row = regimes
            .iter()
            .find(|r| r.id == "drc_kibali_2026")
            .expect("drc_kibali_2026 row");

        // --- 1. Kibali JV (45/45/10, 5.7% royalty, 50% super-profits tax > $2,000/oz, 60% repatriated) ---
        let deck_2600 = gold_deck("deck_kibali_2600", 2_600);
        let kibali_res = evaluate_kibali_waterfall(
            kibali_row,
            &deck_2600,
            &KibaliWaterfallInput {
                payable_gold_oz_100pct: ExactQty::from_integer(700_000),
                operating_cost_100pct: usd(700_000_000),
                capex_100pct: usd(120_000_000),
                repatriable_fraction: qty(60, 100),
                barrick_equity_fraction: qty(45, 100),
                anglogold_equity_fraction: qty(45, 100),
                sokimo_equity_fraction: qty(10, 100),
            },
            false,
        )
        .expect("kibali waterfall");
        assert!(kibali_res.fiscal.super_profits_triggered);
        assert_eq!(
            kibali_res.fiscal.breakdown.effective_royalty_rate,
            qty(57, 1_000)
        );
        assert_eq!(
            kibali_res
                .repatriated_total_100pct
                .checked_add(kibali_res.trapped_cash_100pct)
                .unwrap(),
            kibali_res.distributable_cash_100pct
        );
        assert_eq!(
            kibali_res
                .barrick_repatriated
                .checked_add(kibali_res.anglogold_repatriated)
                .unwrap()
                .checked_add(kibali_res.sokimo_repatriated)
                .unwrap(),
            kibali_res.repatriated_total_100pct
        );
        assert!(kibali_res.barrick_trapped_share.minor > 0);

        // --- 2. Twiga JV (North Mara & Bulyanhulu 84/16 + 50/50 economic benefits annual true-up; E448) ---
        // Gross revenue = $1,000M; costs = $400M -> Total Economic Benefits = $600M -> 50% GoT target = $300M.
        // State direct take = $70M royalty + $120M CIT + $10M levies = $200M.
        // Pre-true-up distributable = $400M -> GoT 16% dividend = $64M, Barrick 84% = $336M.
        // GoT pre-true-up total = $264M < $300M -> annual true-up = $36M!
        // Final: GoT = $300M, Barrick = $300M (exact 50/50 conservation!).
        let twiga_res = evaluate_twiga_waterfall(&TwigaWaterfallInput {
            gross_revenue_100pct: usd(1_000_000_000),
            operating_and_capital_costs_100pct: usd(400_000_000),
            royalties_and_fees_paid_to_got: usd(70_000_000),
            corporate_income_tax_paid_to_got: usd(120_000_000),
            other_state_taxes_and_levies: usd(10_000_000),
            barrick_equity_fraction: qty(84, 100),
            got_free_carried_fraction: qty(16, 100),
            got_target_economic_benefit_share: qty(50, 100),
        })
        .expect("twiga waterfall");
        assert_eq!(twiga_res.total_economic_benefits, usd(600_000_000));
        assert_eq!(twiga_res.got_equity_dividend, usd(64_000_000));
        assert_eq!(twiga_res.annual_true_up_to_got, usd(36_000_000));
        assert_eq!(twiga_res.true_up_shortfall, usd(0));
        assert_eq!(twiga_res.got_final_total, usd(300_000_000));
        assert_eq!(twiga_res.barrick_net_cash, usd(300_000_000));
        assert_eq!(
            twiga_res
                .got_final_total
                .checked_add(twiga_res.barrick_net_cash)
                .unwrap(),
            twiga_res.total_economic_benefits
        );

        // --- 3. Reko Diq JV (50/25/15/10, DSCR lock @ 1.25x & free-carry advance recovery; E302–E303) ---
        // (a) DSCR = 120 / 100 = 1.20x < 1.25x -> lock fires, 100% of post-debt $20M swept, 0 dividends.
        let rd_locked = evaluate_reko_diq_waterfall(&RekoDiqWaterfallInput {
            cfads_100pct: usd(120_000_000),
            scheduled_debt_service: usd(100_000_000),
            min_dscr_covenant: qty(125, 100),
            cash_sweep_fraction_on_lock: ExactQty::from_integer(1),
            opening_carry_recovery_balance: usd(30_000_000),
            carry_recovery_cap_fraction: qty(80, 100),
            barrick_fraction: qty(50, 100),
            federal_soe_fraction: qty(25, 100),
            balochistan_funded_fraction: qty(15, 100),
            balochistan_free_carried_fraction: qty(10, 100),
        })
        .expect("reko diq locked");
        assert!(rd_locked.dscr_lock_active);
        assert_eq!(rd_locked.dscr, Some(qty(6, 5)));
        assert_eq!(rd_locked.swept_to_debt_or_dsra, usd(20_000_000));
        assert_eq!(rd_locked.barrick_total_distribution, usd(0));
        assert_eq!(rd_locked.closing_carry_recovery_balance, usd(30_000_000));

        // (b) DSCR = 300 / 100 = 3.00x >= 1.25x -> $200M distributable:
        // Base: Barrick $100M, Fed SOE $50M, Balochistan funded $30M, Balochistan free-carried $20M.
        // Carry cap = 80% of $20M = $16M -> repaid to Barrick ($116M total), Balochistan free net = $4M,
        // closing carry balance = $30M - $16M = $14M!
        let rd_open = evaluate_reko_diq_waterfall(&RekoDiqWaterfallInput {
            cfads_100pct: usd(300_000_000),
            scheduled_debt_service: usd(100_000_000),
            min_dscr_covenant: qty(125, 100),
            cash_sweep_fraction_on_lock: ExactQty::from_integer(1),
            opening_carry_recovery_balance: usd(30_000_000),
            carry_recovery_cap_fraction: qty(80, 100),
            barrick_fraction: qty(50, 100),
            federal_soe_fraction: qty(25, 100),
            balochistan_funded_fraction: qty(15, 100),
            balochistan_free_carried_fraction: qty(10, 100),
        })
        .expect("reko diq unlocked");
        assert!(!rd_open.dscr_lock_active);
        assert_eq!(rd_open.dscr, Some(ExactQty::from_integer(3)));
        assert_eq!(rd_open.barrick_base_distribution, usd(100_000_000));
        assert_eq!(rd_open.carry_repaid_to_barrick, usd(16_000_000));
        assert_eq!(rd_open.barrick_total_distribution, usd(116_000_000));
        assert_eq!(rd_open.federal_soe_distribution, usd(50_000_000));
        assert_eq!(rd_open.balochistan_funded_distribution, usd(30_000_000));
        assert_eq!(rd_open.balochistan_free_net_distribution, usd(4_000_000));
        assert_eq!(rd_open.closing_carry_recovery_balance, usd(14_000_000));
        // Exact CFADS conservation:
        let total_out = rd_open
            .debt_service_paid
            .checked_add(rd_open.swept_to_debt_or_dsra)
            .unwrap()
            .checked_add(rd_open.retained_in_lockbox)
            .unwrap()
            .checked_add(rd_open.barrick_total_distribution)
            .unwrap()
            .checked_add(rd_open.federal_soe_distribution)
            .unwrap()
            .checked_add(rd_open.balochistan_funded_distribution)
            .unwrap()
            .checked_add(rd_open.balochistan_free_net_distribution)
            .unwrap();
        assert_eq!(total_out, usd(300_000_000));
    }

    #[test]
    fn test_loulo_gounkoto_h1_2026_reset_and_tongon_divestiture_fixtures() {
        let regimes = load_workspace_regimes();
        let loulo_2026 = regimes
            .iter()
            .find(|r| r.id == "mali_loulo_2026")
            .expect("mali_loulo_2026");
        let loulo_2025_np = regimes
            .iter()
            .find(|r| r.id == "mali_loulo_2025_not_published")
            .expect("mali_loulo_2025_not_published");
        let loulo_2023_sup = regimes
            .iter()
            .find(|r| r.id == "mali_loulo_2023_superseded")
            .expect("mali_loulo_2023_superseded");

        let deck_4500 = gold_deck("deck_loulo_4500", 4_500);
        let h1_2026_in = LouloGounkotoResetInput {
            // FACT E483/E670: H1 2026 production = 152 koz Au on Barrick 80% basis (= 190 koz 100%).
            attributable_gold_oz: ExactQty::from_integer(152_000),
            barrick_equity_fraction: qty(80, 100),
            // Illustrative H1 2026 100%-basis operating costs + FACT E412–E413 $200M retroactive tax/penalty
            // + FACT E407 $253M Nov 2025 settlement payment.
            operating_cost_100pct: usd(250_000_000),
            retroactive_tax_and_penalty_100pct: usd(200_000_000),
            settlement_payment_100pct: usd(253_000_000),
        };

        // 1. Q1-Q3 2025 provisional administration (NOT_PUBLISHED) MUST refuse (FMINE-REF-201).
        let np_err = evaluate_loulo_gounkoto_reset(loulo_2025_np, &deck_4500, &h1_2026_in, false)
            .expect_err("NotPublished Q1-Q3 2025 must refuse");
        assert_eq!(np_err.to_refusal().code, "FMINE-REF-201");

        // 2. Superseded 1991-Code row MUST refuse in live mode (INV-013, FMINE-REF-502).
        let stale_err =
            evaluate_loulo_gounkoto_reset(loulo_2023_sup, &deck_4500, &h1_2026_in, false)
                .expect_err("superseded 1991-Code row must refuse");
        assert_eq!(stale_err.to_refusal().code, "FMINE-REF-502");

        // 3. H1 2026 post-settlement 2023 Code reset: 190,000 oz @ $4,500/oz -> 17.85% royalty (E414).
        let res = evaluate_loulo_gounkoto_reset(loulo_2026, &deck_4500, &h1_2026_in, false)
            .expect("H1 2026 Loulo reset");
        assert_eq!(res.gross_gold_oz_100pct, ExactQty::from_integer(190_000));
        assert_eq!(
            res.fiscal.breakdown.effective_royalty_rate,
            qty(1_785, 10_000)
        );
        assert_eq!(res.one_time_charges_100pct, usd(453_000_000));
        assert_eq!(
            res.barrick_attributable_net_cash
                .checked_add(res.mali_state_equity_net_cash)
                .unwrap(),
            res.net_post_reset_cash_100pct
        );

        // 4. Tongon divestiture fixture (closed 2025-12-01, up to $305M proceeds; E485–E488, E553–E554).
        let graph = barrick_2026_portfolio_graph(qty(85, 100), qty(70, 100), qty(125, 100))
            .expect("barrick graph");
        let tongon_id = EntityNodeId::new("tongon_divested").unwrap();
        let tongon_node = graph.node(&tongon_id).expect("tongon node");
        let tongon_edge = graph
            .edges
            .iter()
            .find(|e| e.from_child == tongon_id)
            .expect("tongon edge");

        // Closing quarter Q4 2025 (2025-10-01..2025-12-31 containing closed_date 2025-12-01)
        // recognizes stub operating cash flow + up to $305M exit proceeds cleanly.
        let at_close = evaluate_divested_asset_period(
            tongon_node,
            tongon_edge,
            "2025-10-01",
            "2025-12-31",
            usd(15_000_000),
            usd(305_000_000),
            true,
        )
        .expect("closing quarter Q4 2025");
        assert!(!at_close.post_divestiture_terminated);
        assert_eq!(at_close.recognized_exit_proceeds, usd(305_000_000));

        // Pre-close quarter Q3 2025 refuses closing proceeds before closed_date.
        let pre_close_err = evaluate_divested_asset_period(
            tongon_node,
            tongon_edge,
            "2025-07-01",
            "2025-09-30",
            usd(20_000_000),
            usd(1_000_000),
            true,
        )
        .expect_err("proceeds before close must refuse");
        assert_eq!(pre_close_err.to_refusal().code, "FMINE-REF-109");

        // Proceeds > contracted $305M ceiling refuse (FMINE-REF-109).
        let over_cap = evaluate_divested_asset_period(
            tongon_node,
            tongon_edge,
            "2025-12-01",
            "2025-12-01",
            usd(0),
            usd(306_000_000),
            true,
        )
        .expect_err("over-cap proceeds must refuse");
        assert_eq!(over_cap.to_refusal().code, "FMINE-REF-109");

        // Post-divestiture (Q1 2026: 2026-01-01..2026-03-31) zeroes stale projected operating cash flow...
        let q1_2026 = evaluate_divested_asset_period(
            tongon_node,
            tongon_edge,
            "2026-01-01",
            "2026-03-31",
            usd(42_000_000),
            usd(0),
            false,
        )
        .expect("post-divestiture zeroed");
        assert!(q1_2026.post_divestiture_terminated);
        assert_eq!(q1_2026.recognized_operating_cashflow, usd(0));
        assert_eq!(q1_2026.suppressed_stale_operating_cashflow, usd(42_000_000));

        // ...and strictly refuses non-zero forward operating cash flow in strict mode (FMINE-REF-109).
        let strict_err = evaluate_divested_asset_period(
            tongon_node,
            tongon_edge,
            "2026-01-01",
            "2026-03-31",
            usd(42_000_000),
            usd(0),
            true,
        )
        .expect_err("strict post-divestiture cashflow must refuse");
        assert_eq!(strict_err.to_refusal().code, "FMINE-REF-109");

        // 5. Edge-case & safety regressions: invalid calendar date, DivestedAsset ongoing equity,
        //    Suspension with active ounces, Twiga funding_deficit, Reko Diq negative CFADS, currency mismatch.
        assert!(!is_iso_date("2026-02-30"));
        assert!(is_iso_date("2024-02-29"));
        assert!(!is_iso_date("2025-02-29"));

        let mut bad_div = graph.clone();
        bad_div.edges.push(WaterfallEdge {
            from_child: tongon_id,
            to_parent: EntityNodeId::new("na_newco").unwrap(),
            kind: WaterfallEdgeKind::DirectEquity {
                equity_fraction: qty(10, 100),
            },
            source_e_ids: vec!["SYNTH".to_string()],
        });
        assert_eq!(
            bad_div.validate().unwrap_err().to_refusal().code,
            "FMINE-REF-109"
        );

        let mut suspended_loulo = loulo_2026.clone();
        suspended_loulo.state = crate::regime::RegimeState::Suspension;
        assert_eq!(
            evaluate_loulo_gounkoto_reset(&suspended_loulo, &deck_4500, &h1_2026_in, false)
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-109"
        );

        let twiga_def = evaluate_twiga_waterfall(&TwigaWaterfallInput {
            gross_revenue_100pct: usd(500_000_000),
            operating_and_capital_costs_100pct: usd(450_000_000),
            royalties_and_fees_paid_to_got: usd(70_000_000),
            corporate_income_tax_paid_to_got: usd(0),
            other_state_taxes_and_levies: usd(0),
            barrick_equity_fraction: qty(84, 100),
            got_free_carried_fraction: qty(16, 100),
            got_target_economic_benefit_share: qty(50, 100),
        })
        .expect("twiga deficit");
        assert_eq!(twiga_def.funding_deficit, usd(20_000_000));
        assert_eq!(
            twiga_def
                .got_final_total
                .checked_add(twiga_def.barrick_net_cash)
                .unwrap()
                .checked_sub(twiga_def.funding_deficit)
                .unwrap(),
            twiga_def.total_economic_benefits
        );

        let rd_neg = evaluate_reko_diq_waterfall(&RekoDiqWaterfallInput {
            cfads_100pct: usd(-25_000_000),
            scheduled_debt_service: usd(100_000_000),
            min_dscr_covenant: qty(125, 100),
            cash_sweep_fraction_on_lock: ExactQty::from_integer(1),
            opening_carry_recovery_balance: usd(30_000_000),
            carry_recovery_cap_fraction: qty(80, 100),
            barrick_fraction: qty(50, 100),
            federal_soe_fraction: qty(25, 100),
            balochistan_funded_fraction: qty(15, 100),
            balochistan_free_carried_fraction: qty(10, 100),
        })
        .expect("reko diq negative cfads");
        assert!(rd_neg.dscr_lock_active);
        assert_eq!(rd_neg.operating_deficit, usd(25_000_000));
        assert_eq!(rd_neg.debt_service_shortfall, usd(100_000_000));

        let cad_mismatch = evaluate_jv_capital_call(&JvCapitalCallInput {
            jv_id: EntityNodeId::new("ngm_jv").unwrap(),
            prior_operator_share: qty(615, 1_000),
            prior_total_capital_account: Money::from_major(Currency::CAD, 10_000_000_000),
            total_call_required: usd(1_000_000_000),
            operator_contribution: usd(615_000_000),
            partner_contribution: usd(385_000_000),
            non_consent_penalty_multiplier: ExactQty::from_integer(1),
        })
        .expect_err("currency mismatch must refuse");
        assert_eq!(cad_mismatch.to_refusal().code, "FMINE-REF-105");

        // 6. Fresh-eyes regressions (E-1..E-6):
        // (a) Duplicate source_e_ids deduplicate identically in digest; blank country_regime_id refuses.
        let mut dup_eid_graph = graph.clone();
        dup_eid_graph.nodes[0].source_e_ids.push("E658".to_string());
        assert_eq!(graph.digest().unwrap(), dup_eid_graph.digest().unwrap());
        let mut blank_reg_graph = graph.clone();
        blank_reg_graph.nodes[0].country_regime_id = Some("  ".to_string());
        assert_eq!(
            blank_reg_graph.validate().unwrap_err().to_refusal().code,
            "FMINE-REF-109"
        );

        // (b) JV call: when operator funds ONLY its pro-rata $615M while partner defaults $0,
        //     2x sole-risk cover penalty does NOT fire (straight pre-money dilution 6765/10615).
        let uncovered_default = evaluate_jv_capital_call(&JvCapitalCallInput {
            jv_id: EntityNodeId::new("ngm_jv").unwrap(),
            prior_operator_share: qty(615, 1_000),
            prior_total_capital_account: usd(10_000_000_000),
            total_call_required: usd(1_000_000_000),
            operator_contribution: usd(615_000_000),
            partner_contribution: usd(0),
            non_consent_penalty_multiplier: ExactQty::from_integer(2),
        })
        .expect("uncovered default uses straight dilution");
        assert!(!uncovered_default.fully_funded);
        assert_eq!(uncovered_default.funding_shortfall, usd(385_000_000));
        assert_eq!(uncovered_default.new_operator_share, qty(1_353, 2_123));

        // (c) Kibali 3-way trapped conservation + Suspension refusal.
        let kibali_row = regimes
            .iter()
            .find(|r| r.id == "drc_kibali_2026")
            .expect("drc_kibali_2026");
        let kibali_in = KibaliWaterfallInput {
            payable_gold_oz_100pct: ExactQty::from_integer(100_000),
            operating_cost_100pct: usd(100_000_000),
            capex_100pct: usd(10_000_000),
            repatriable_fraction: qty(60, 100),
            barrick_equity_fraction: qty(45, 100),
            anglogold_equity_fraction: qty(45, 100),
            sokimo_equity_fraction: qty(10, 100),
        };
        let kibali_ok = evaluate_kibali_waterfall(kibali_row, &deck_4500, &kibali_in, false)
            .expect("kibali ok");
        assert_eq!(
            kibali_ok
                .barrick_trapped_share
                .checked_add(kibali_ok.anglogold_trapped_share)
                .unwrap()
                .checked_add(kibali_ok.sokimo_trapped_share)
                .unwrap(),
            kibali_ok.trapped_cash_100pct
        );
        let mut kibali_susp = kibali_row.clone();
        kibali_susp.state = crate::regime::RegimeState::Suspension;
        assert_eq!(
            evaluate_kibali_waterfall(&kibali_susp, &deck_4500, &kibali_in, false)
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-109"
        );

        // (d) Reko Diq zero scheduled debt service + negative CFADS -> dscr_lock_active == false.
        let rd_zero_debt = evaluate_reko_diq_waterfall(&RekoDiqWaterfallInput {
            cfads_100pct: usd(-15_000_000),
            scheduled_debt_service: usd(0),
            min_dscr_covenant: qty(125, 100),
            cash_sweep_fraction_on_lock: ExactQty::from_integer(1),
            opening_carry_recovery_balance: usd(30_000_000),
            carry_recovery_cap_fraction: qty(80, 100),
            barrick_fraction: qty(50, 100),
            federal_soe_fraction: qty(25, 100),
            balochistan_funded_fraction: qty(15, 100),
            balochistan_free_carried_fraction: qty(10, 100),
        })
        .expect("reko diq zero debt service");
        assert!(!rd_zero_debt.dscr_lock_active);
        assert_eq!(rd_zero_debt.dscr, None);
        assert_eq!(rd_zero_debt.operating_deficit, usd(15_000_000));

        // (e) Loulo barrick_equity + state_equity > 100% refuses (FMINE-REF-109).
        let mut over_100_loulo = h1_2026_in;
        over_100_loulo.barrick_equity_fraction = qty(90, 100);
        assert_eq!(
            evaluate_loulo_gounkoto_reset(loulo_2026, &deck_4500, &over_100_loulo, false)
                .unwrap_err()
                .to_refusal()
                .code,
            "FMINE-REF-109"
        );
    }
}
