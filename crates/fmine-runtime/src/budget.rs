//! Region budgets and what each region does when its budget runs out (plan §26.7, BD-078).
//!
//! A [`Budget`] is `{deadline, poll_quota, cost_quota, priority}`, and two compose by [`Budget::meet`]: each
//! limit is the tighter of the two and the priority the higher. A child's budget is the meet of its parent's and
//! its own row, so it never outlasts, out-polls or outspends its parent.
//!
//! [`budget_table`] gives the §26.7 rows. Values the plan leaves to the run (an estimation run's deadline and size, a
//! peer link's window, whether `fa` is a debug build) are explicit [`Params`], and zero or overflowing values are
//! refused. A [`Meter`] spends a budget against times the caller passes in, so lab tests run on virtual time and
//! nothing here reads a clock. It refuses a poll or a charge past a limit, records nothing for it, and stays
//! exhausted, reporting the row's [`OnExhaust`].

use crate::region::RegionKind;

const MS: u64 = 1_000_000;
const S: u64 = 1_000 * MS;
const MIB: u64 = 1 << 20;

/// What a region may spend. `None` is no limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Nanoseconds allowed from the start.
    pub deadline_ns: Option<u64>,
    pub poll_quota: Option<u64>,
    /// In the row's [`CostUnit`].
    pub cost_quota: Option<u64>,
    /// Higher runs first.
    pub priority: u8,
}

const fn tighter(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if x < y { x } else { y }),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

impl Budget {
    /// No limit and the lowest priority: the identity of [`Budget::meet`].
    pub const UNLIMITED: Self = Self {
        deadline_ns: None,
        poll_quota: None,
        cost_quota: None,
        priority: 0,
    };

    /// The tighter of each limit, and the higher priority.
    #[must_use]
    pub const fn meet(self, other: Self) -> Self {
        Self {
            deadline_ns: tighter(self.deadline_ns, other.deadline_ns),
            poll_quota: tighter(self.poll_quota, other.poll_quota),
            cost_quota: tighter(self.cost_quota, other.cost_quota),
            priority: if self.priority > other.priority {
                self.priority
            } else {
                other.priority
            },
        }
    }
}

/// What a cost quota counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostUnit {
    Bytes,
    /// Blocks times realizations.
    BlockRealizations,
}

/// What a region does when a limit is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnExhaust {
    /// Refuse the batch (exit 6); the caller retries with the same batch id.
    RefuseBatch,
    /// Seal the segment early and downsample, with a continuity receipt; never drop silently.
    SealEarly,
    /// Cancel; partial results are never published (root-last).
    Cancel,
    /// Drain, persist the chunk receipt and resume in the next window.
    Drain,
    /// The post may have happened (exit 9): reconcile by idempotency key and never re-post blindly.
    Reconcile,
}

impl OnExhaust {
    /// The robot exit code it surfaces as (§30.1), where it ends a verb; `None` where the region carries on.
    #[must_use]
    pub const fn exit_code(self) -> Option<u8> {
        match self {
            Self::RefuseBatch => Some(6),
            Self::Reconcile => Some(9),
            Self::SealEarly | Self::Cancel | Self::Drain => None,
        }
    }
}

/// One row of the §26.7 budget table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetRow {
    pub region: RegionKind,
    pub budget: Budget,
    pub cost_unit: Option<CostUnit>,
    pub on_exhaust: OnExhaust,
}

impl BudgetRow {
    /// A meter from `now_ns` for this row's budget met with its parent's, so it never outlasts, out-polls or
    /// outspends `parent`. Pass [`Budget::UNLIMITED`] for a region with no parent budget.
    #[must_use]
    pub const fn meter(&self, parent: Budget, now_ns: u64) -> Meter {
        Meter::start(self.budget.meet(parent), self.on_exhaust, now_ns)
    }
}

/// Which `fa` build the finance bridge posts to: 120 s for a debug build, the 10 s target otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaBuild {
    Debug,
    Release,
}

/// The values §26.7 leaves to the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    /// An estimation run's deadline, such as 4 h.
    pub estimation_deadline_ns: u64,
    pub estimation_blocks: u64,
    pub estimation_realizations: u64,
    /// A peer link's window, in time and in bytes.
    pub peer_window_ns: u64,
    pub peer_window_bytes: u64,
    pub fa_build: FaBuild,
}

/// A parameter or a charge that cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    /// The named parameter is zero, which would exhaust the budget before it starts.
    Zero(&'static str),
    /// Blocks times realizations, or a running cost, overflows `u64`.
    Overflow,
    /// A time before the meter's start: time went backwards.
    BeforeStart,
}

/// The §26.7 budget table for `p`, in the plan's order: the ledger writer's commit, the telemetry segment seal, an
/// estimation run, a peer session's window and a finance-bridge post.
///
/// # Errors
/// [`BudgetError::Zero`] for a zero parameter, and [`BudgetError::Overflow`] when blocks times realizations
/// overflows.
pub fn budget_table(p: &Params) -> Result<[BudgetRow; 5], BudgetError> {
    for (name, v) in [
        ("estimation_deadline_ns", p.estimation_deadline_ns),
        ("estimation_blocks", p.estimation_blocks),
        ("estimation_realizations", p.estimation_realizations),
        ("peer_window_ns", p.peer_window_ns),
        ("peer_window_bytes", p.peer_window_bytes),
    ] {
        if v == 0 {
            return Err(BudgetError::Zero(name));
        }
    }
    let estimation_cost = p
        .estimation_blocks
        .checked_mul(p.estimation_realizations)
        .ok_or(BudgetError::Overflow)?;
    let row =
        |region, deadline_ns, cost: Option<(u64, CostUnit)>, priority, on_exhaust| BudgetRow {
            region,
            budget: Budget {
                deadline_ns: Some(deadline_ns),
                poll_quota: None,
                cost_quota: cost.map(|c| c.0),
                priority,
            },
            cost_unit: cost.map(|c| c.1),
            on_exhaust,
        };
    let fa_deadline = match p.fa_build {
        FaBuild::Debug => 120 * S,
        FaBuild::Release => 10 * S,
    };
    Ok([
        row(
            RegionKind::LedgerWriter,
            250 * MS,
            None,
            200,
            OnExhaust::RefuseBatch,
        ),
        row(
            RegionKind::Telemetry,
            S,
            Some((64 * MIB, CostUnit::Bytes)),
            180,
            OnExhaust::SealEarly,
        ),
        row(
            RegionKind::EstimationRun,
            p.estimation_deadline_ns,
            Some((estimation_cost, CostUnit::BlockRealizations)),
            80,
            OnExhaust::Cancel,
        ),
        row(
            RegionKind::PeerSession,
            p.peer_window_ns,
            Some((p.peer_window_bytes, CostUnit::Bytes)),
            120,
            OnExhaust::Drain,
        ),
        row(
            RegionKind::FinanceBridge,
            fa_deadline,
            None,
            150,
            OnExhaust::Reconcile,
        ),
    ])
}

/// The limit a meter reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    Deadline,
    Polls,
    Cost,
}

/// A spent budget: which limit, and what the region does now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exhausted {
    pub limit: Limit,
    pub on_exhaust: OnExhaust,
}

/// Why a poll or a charge did not go through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Exhausted(Exhausted),
    Invalid(BudgetError),
}

/// A budget being spent. The caller passes every time in, in nanoseconds on its own clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meter {
    budget: Budget,
    on_exhaust: OnExhaust,
    started_ns: u64,
    polls: u64,
    cost: u64,
    exhausted: Option<Exhausted>,
}

impl Meter {
    /// A meter for `budget` from `now_ns`, doing `on_exhaust` when it runs out.
    #[must_use]
    pub const fn start(budget: Budget, on_exhaust: OnExhaust, now_ns: u64) -> Self {
        Self {
            budget,
            on_exhaust,
            started_ns: now_ns,
            polls: 0,
            cost: 0,
            exhausted: None,
        }
    }

    /// The polls made and the cost charged so far; neither counts a refused one.
    #[must_use]
    pub const fn spent(&self) -> (u64, u64) {
        (self.polls, self.cost)
    }

    fn stop(&mut self, limit: Limit) -> Stop {
        let e = Exhausted {
            limit,
            on_exhaust: self.on_exhaust,
        };
        self.exhausted = Some(e);
        Stop::Exhausted(e)
    }

    /// Checks the deadline at `now_ns`: reached once the elapsed time equals it.
    fn check_time(&mut self, now_ns: u64) -> Result<(), Stop> {
        if let Some(e) = self.exhausted {
            return Err(Stop::Exhausted(e));
        }
        let elapsed = now_ns
            .checked_sub(self.started_ns)
            .ok_or(Stop::Invalid(BudgetError::BeforeStart))?;
        if self.budget.deadline_ns.is_some_and(|d| elapsed >= d) {
            return Err(self.stop(Limit::Deadline));
        }
        Ok(())
    }

    /// One poll at `now_ns`.
    ///
    /// # Errors
    /// [`Stop::Exhausted`] at or past the deadline, past the poll quota, or once exhausted;
    /// [`Stop::Invalid`] for a time before the start.
    pub fn poll(&mut self, now_ns: u64) -> Result<(), Stop> {
        self.check_time(now_ns)?;
        if self.budget.poll_quota.is_some_and(|q| self.polls >= q) {
            return Err(self.stop(Limit::Polls));
        }
        self.polls += 1;
        Ok(())
    }

    /// Charges `cost` at `now_ns`. A charge that would pass the quota is refused whole and not recorded.
    ///
    /// # Errors
    /// [`Stop::Exhausted`] at or past the deadline, past the cost quota, or once exhausted;
    /// [`Stop::Invalid`] for a time before the start or a running cost past `u64`.
    pub fn charge(&mut self, cost: u64, now_ns: u64) -> Result<(), Stop> {
        self.check_time(now_ns)?;
        let total = self
            .cost
            .checked_add(cost)
            .ok_or(Stop::Invalid(BudgetError::Overflow))?;
        if self.budget.cost_quota.is_some_and(|q| total > q) {
            return Err(self.stop(Limit::Cost));
        }
        self.cost = total;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
