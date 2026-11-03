//! One `AppSpec` per role, compiled to a supervisor that decides restarts and escalations (plan §26.7, BD-079).
//!
//! The supervisor decides; it does not execute. The caller reports each start and failure with a time on its own
//! clock, and the supervisor says what restarts, what stops and whether the node may still write. It reads no
//! clock and starts nothing, so lab tests drive it on virtual time.
//!
//! A child's own failures are counted over a sliding window of [`WINDOW_NS`]. The first [`RESTARTS`] in a window
//! restart it with its group, under its parent's [`Supervisor`] strategy. The sync region's one-for-all is per
//! peer (§26.6), so a peer's session restarts with that peer's transfers and no other peer's. The next failure
//! escalates:
//! - a peer session stops with its transfers, and the peer is marked stale. The rest of the node runs on;
//! - a ledger writer stops with every other ledger writer, and the node turns read-only. Writes refuse with
//!   exit 4, and no writer starts again in this process, because its segment may be torn. Leaving read-only mode
//!   takes a new process, after `fmine doctor` verifies the hash chain;
//! - any other child fails its parent in turn. A failure that reaches the process root, which has no strategy,
//!   shuts the node down.
//!
//! A restarted child is a fresh instance under a fresh supervisor, so a restart clears the failure history of every
//! descendant of the children it restarts.

use std::collections::{BTreeMap, BTreeSet};

use crate::region::{Instances, RegionKind, RegionSpec, Role, Supervisor, profile};

/// Restarts allowed per child within [`WINDOW_NS`]. The next failure in the window escalates.
pub const RESTARTS: usize = 5;
/// 60 s.
pub const WINDOW_NS: u64 = 60_000_000_000;

/// One running instance of a region.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Child {
    pub region: RegionKind,
    /// A keyed region's key (`cell`, `source`, `fmt`, `peer` or `root`), or a many-instance region's id.
    pub key: Option<String>,
    /// The peer a peer session or a transfer belongs to.
    pub peer: Option<String>,
}

impl Child {
    /// The single instance of a region.
    #[must_use]
    pub const fn one(region: RegionKind) -> Self {
        Self {
            region,
            key: None,
            peer: None,
        }
    }

    /// An instance of a keyed or many-instance region. A peer session's key is its peer.
    #[must_use]
    pub fn keyed(region: RegionKind, key: &str) -> Self {
        Self {
            region,
            key: Some(key.to_owned()),
            peer: (region == RegionKind::PeerSession).then(|| key.to_owned()),
        }
    }

    /// The transfer of `root` over `peer`'s session.
    #[must_use]
    pub fn transfer(root: &str, peer: &str) -> Self {
        Self {
            region: RegionKind::Transfer,
            key: Some(root.to_owned()),
            peer: Some(peer.to_owned()),
        }
    }
}

/// The regions a node of one role runs, root first and each parent before its children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSpec {
    pub role: Role,
    pub regions: Vec<RegionSpec>,
}

/// Why an `AppSpec` does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecError {
    Empty,
    /// The first region has a parent.
    RootHasParent(RegionKind),
    /// A region after the first has no parent.
    SecondRoot(RegionKind),
    Duplicate(RegionKind),
    /// The region's parent is not listed before it.
    MissingParent(RegionKind),
    /// The region's parent has more than one instance, so a child could not name it.
    KeyedParent(RegionKind),
    /// The named region has children, is not the root, and has no restart strategy.
    NoStrategy(RegionKind),
    /// The region runs only on nodes of another role.
    WrongRole(RegionKind),
}

impl AppSpec {
    /// The plan's tree for `role`.
    #[must_use]
    pub fn for_role(role: Role) -> Self {
        Self {
            role,
            regions: profile(role),
        }
    }

    /// Checks the spec and gives a supervisor with nothing running.
    ///
    /// # Errors
    /// The first [`SpecError`] in region order.
    pub fn compile(&self) -> Result<CompiledSupervisor, SpecError> {
        let root = self.regions.first().ok_or(SpecError::Empty)?;
        if root.parent.is_some() {
            return Err(SpecError::RootHasParent(root.kind));
        }
        for (i, region) in self.regions.iter().enumerate() {
            let before = &self.regions[..i];
            if before.iter().any(|r| r.kind == region.kind) {
                return Err(SpecError::Duplicate(region.kind));
            }
            if region.only.is_some_and(|role| role != self.role) {
                return Err(SpecError::WrongRole(region.kind));
            }
            if i == 0 {
                continue;
            }
            let parent = region.parent.ok_or(SpecError::SecondRoot(region.kind))?;
            let parent = before
                .iter()
                .find(|r| r.kind == parent)
                .ok_or(SpecError::MissingParent(region.kind))?;
            if parent.instances != Instances::One {
                return Err(SpecError::KeyedParent(region.kind));
            }
            if parent.parent.is_some() && parent.supervisor.is_none() {
                return Err(SpecError::NoStrategy(parent.kind));
            }
        }
        Ok(CompiledSupervisor {
            spec: self.clone(),
            running: Vec::new(),
            history: BTreeMap::new(),
            stale: BTreeSet::new(),
            read_only: None,
            stopped: false,
            last_ns: 0,
        })
    }
}

/// What a failure leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// `failed` restarts with its group and every running descendant, listed in start order. After an
    /// escalation, `failed` is the ancestor whose parent's strategy acted.
    Restart {
        failed: Child,
        restarted: Vec<Child>,
    },
    /// The peer's session escalated. It stops with its transfers, and the peer is stale.
    PeerStale { peer: String, stopped: Vec<Child> },
    /// A ledger writer escalated. Every ledger writer stops, and the node refuses writes.
    ReadOnly { cell: String, stopped: Vec<Child> },
    /// The failure reached the process root. Nothing runs.
    Shutdown,
}

/// Why the supervisor refused a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The role's spec has no such region.
    NotInSpec(RegionKind),
    /// The key or the peer does not fit the region.
    Shape(Child),
    AlreadyRunning(Child),
    /// The named parent, or a transfer's peer session, is not running.
    ParentNotRunning(Child),
    NotRunning(Child),
    /// The node is read-only after the writer for `cell` escalated: a write, or a ledger writer's start.
    ReadOnly {
        cell: String,
    },
    /// A time before the last call's.
    TimeWentBack,
    /// The node shut down.
    Stopped,
}

impl Refused {
    /// The robot exit code (§30.1): 4 for a write refused in read-only degraded mode. The others are the caller's
    /// errors and map to its own codes.
    #[must_use]
    pub const fn exit_code(&self) -> Option<u8> {
        match self {
            Self::ReadOnly { .. } => Some(4),
            _ => None,
        }
    }
}

/// A role's supervisor: what runs, each child's recent failures, the stale peers and whether the node is read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSupervisor {
    spec: AppSpec,
    /// In start order.
    running: Vec<Child>,
    /// Each child's own failures that restarted it, within the window.
    history: BTreeMap<Child, Vec<u64>>,
    stale: BTreeSet<String>,
    read_only: Option<String>,
    stopped: bool,
    last_ns: u64,
}

impl CompiledSupervisor {
    #[must_use]
    pub fn role(&self) -> Role {
        self.spec.role
    }

    /// The running children, in start order.
    #[must_use]
    pub fn running(&self) -> &[Child] {
        &self.running
    }

    #[must_use]
    pub fn is_stale(&self, peer: &str) -> bool {
        self.stale.contains(peer)
    }

    /// Whether the node may write.
    ///
    /// # Errors
    /// [`Refused::ReadOnly`] (exit 4) after a ledger writer escalated, and [`Refused::Stopped`] after a shutdown.
    pub fn writes(&self) -> Result<(), Refused> {
        if self.stopped {
            return Err(Refused::Stopped);
        }
        match &self.read_only {
            Some(cell) => Err(Refused::ReadOnly { cell: cell.clone() }),
            None => Ok(()),
        }
    }

    fn spec_of(&self, region: RegionKind) -> Result<&RegionSpec, Refused> {
        self.spec
            .regions
            .iter()
            .find(|r| r.kind == region)
            .ok_or(Refused::NotInSpec(region))
    }

    /// Whether `region` lies strictly below `ancestor`.
    fn descends(&self, region: RegionKind, ancestor: RegionKind) -> bool {
        let mut next = self.spec_of(region).ok().and_then(|r| r.parent);
        while let Some(parent) = next {
            if parent == ancestor {
                return true;
            }
            next = self.spec_of(parent).ok().and_then(|r| r.parent);
        }
        false
    }

    fn tick(&mut self, now_ns: u64) -> Result<(), Refused> {
        if self.stopped {
            return Err(Refused::Stopped);
        }
        if now_ns < self.last_ns {
            return Err(Refused::TimeWentBack);
        }
        self.last_ns = now_ns;
        Ok(())
    }

    /// Records `child` as started at `now_ns`. Starting a stale peer's session clears its stale mark.
    ///
    /// # Errors
    /// A [`Refused`] for a region outside the spec, a key or peer that does not fit, a child already running, a
    /// parent or peer session not running, a ledger writer on a read-only node, a time before the last call's,
    /// or a stopped node.
    pub fn start(&mut self, child: Child, now_ns: u64) -> Result<(), Refused> {
        self.tick(now_ns)?;
        let spec = self.spec_of(child.region)?;
        let keyed = !matches!(spec.instances, Instances::One);
        let fits = child.key.is_some() == keyed
            && match child.region {
                RegionKind::PeerSession => child.peer.is_some() && child.peer == child.key,
                RegionKind::Transfer => child.peer.is_some(),
                _ => child.peer.is_none(),
            };
        if !fits {
            return Err(Refused::Shape(child));
        }
        if self.running.contains(&child) {
            return Err(Refused::AlreadyRunning(child));
        }
        let mut needs: Vec<Child> = spec.parent.map(Child::one).into_iter().collect();
        if let (RegionKind::Transfer, Some(peer)) = (child.region, &child.peer) {
            needs.push(Child::keyed(RegionKind::PeerSession, peer));
        }
        if let Some(missing) = needs.into_iter().find(|n| !self.running.contains(n)) {
            return Err(Refused::ParentNotRunning(missing));
        }
        if let (RegionKind::LedgerWriter, Some(cell)) = (child.region, &self.read_only) {
            return Err(Refused::ReadOnly { cell: cell.clone() });
        }
        if let (RegionKind::PeerSession, Some(peer)) = (child.region, &child.peer) {
            self.stale.remove(peer);
        }
        self.running.push(child);
        Ok(())
    }

    /// Reports that `child` failed at `now_ns`, and returns what follows.
    ///
    /// # Errors
    /// [`Refused::NotRunning`], [`Refused::TimeWentBack`] or [`Refused::Stopped`].
    pub fn fail(&mut self, child: &Child, now_ns: u64) -> Result<Outcome, Refused> {
        self.tick(now_ns)?;
        if !self.running.contains(child) {
            return Err(Refused::NotRunning(child.clone()));
        }
        let mut failed = child.clone();
        loop {
            let parent = self.spec_of(failed.region)?.parent;
            let strategy = match parent {
                Some(p) => self.spec_of(p)?.supervisor.map(|s| (p, s)),
                None => None,
            };
            let Some((parent, strategy)) = strategy else {
                self.stopped = true;
                self.running.clear();
                self.history.clear();
                return Ok(Outcome::Shutdown);
            };
            let window = self.history.entry(failed.clone()).or_default();
            window.retain(|&t| now_ns.checked_sub(t).is_some_and(|d| d < WINDOW_NS));
            if window.len() < RESTARTS {
                window.push(now_ns);
                return Ok(self.restart(failed, parent, strategy));
            }
            match (failed.region, &failed.key) {
                (RegionKind::PeerSession, Some(peer)) => {
                    let peer = peer.clone();
                    let stopped = self.stop(|c| c.peer.as_ref() == Some(&peer));
                    self.stale.insert(peer.clone());
                    return Ok(Outcome::PeerStale { peer, stopped });
                }
                (RegionKind::LedgerWriter, Some(cell)) => {
                    let cell = cell.clone();
                    let stopped = self.stop(|c| c.region == RegionKind::LedgerWriter);
                    self.read_only = Some(cell.clone());
                    return Ok(Outcome::ReadOnly { cell, stopped });
                }
                _ => failed = Child::one(parent),
            }
        }
    }

    /// Stops the running children that match, and forgets their failures.
    fn stop(&mut self, matches: impl Fn(&Child) -> bool) -> Vec<Child> {
        let (stopped, kept) = self.running.drain(..).partition(|c| matches(c));
        self.running = kept;
        self.history.retain(|c, _| !matches(c));
        stopped
    }

    fn restart(&mut self, failed: Child, parent: RegionKind, strategy: Supervisor) -> Outcome {
        let siblings: Vec<&Child> = self
            .running
            .iter()
            .filter(|c| {
                self.spec_of(c.region)
                    .is_ok_and(|r| r.parent == Some(parent))
            })
            .collect();
        let group: Vec<Child> = match strategy {
            Supervisor::OneForOne => vec![failed.clone()],
            Supervisor::OneForAll => siblings
                .into_iter()
                .filter(|c| c.peer == failed.peer)
                .cloned()
                .collect(),
            Supervisor::RestForOne => siblings
                .into_iter()
                .skip_while(|&c| *c != failed)
                .cloned()
                .collect(),
        };
        let below = |c: &Child| group.iter().any(|g| self.descends(c.region, g.region));
        let restarted = self
            .running
            .iter()
            .filter(|&c| group.contains(c) || below(c))
            .cloned()
            .collect();
        let cleared: Vec<Child> = self.history.keys().filter(|&c| below(c)).cloned().collect();
        for c in cleared {
            self.history.remove(&c);
        }
        Outcome::Restart { failed, restarted }
    }
}

#[cfg(test)]
mod tests;
