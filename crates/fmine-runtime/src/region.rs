//! The region tree and capability profiles of plan §26.6 (BD-077), as types.
//!
//! A capability set is `CapSet<SPAWN, TIME, RANDOM, IO, REMOTE>`, in the plan's order. A context [`Cx<C>`] carries
//! one as its type and only narrows: [`Cx::open`] gives a child region's context, and [`Cx::narrow`] a context for a
//! call that needs less, each only when the new set is within the old one, which the compiler checks. The uses of
//! TIME and IO this crate has, [`Cx::clock`] and [`Cx::run`], exist only on a context holding that capability.
//!
//! Each region of the tree is a type whose set is the plan's row, and each region's set is checked against its
//! parent's when this crate compiles. Interior regions, for which the plan gives no set, hold the union of their
//! children's: the least that lets every child narrow from it. So RANDOM, which no region's row grants, stays with
//! the process root, and engines get seeds as inputs (INV-010).
//!
//! This is the compile-time half. asupersync is not a dependency yet, so no region here runs, supervises or
//! cancels anything. [`REGIONS`] records each region's supervisor, instances and role for the AppSpec and budget
//! beads (BD-078, BD-079) and the runtime capability-mask lab test (BD-097).
//!
//! ```
//! use fmine_runtime::region::{
//!     AuthorityRegion, Cx, LedgerWriterRegion, NodeRegion, ObjectStoreRegion, Region,
//! };
//!
//! let root = Cx::ambient_root();
//! let writer = root.open::<NodeRegion>().open::<AuthorityRegion>().open::<LedgerWriterRegion>();
//! assert!(writer.caps().time && writer.caps().io && !writer.caps().remote);
//! // The writer's context may give up TIME for the object store's, but not the reverse (see `Cx::narrow`).
//! let store = writer.narrow::<<ObjectStoreRegion as Region>::Caps>();
//! assert!(!store.caps().time);
//! ```

use std::marker::PhantomData;
use std::process::Command;

use crate::HostWallClock;
use crate::process::{PipedOutput, ProcessError, output_with_input};

/// A capability set as a type: `CapSet<false, true, false, true, false>` is the plan's `CapSet<0,1,0,1,0>`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapSet<
    const SPAWN: bool,
    const TIME: bool,
    const RANDOM: bool,
    const IO: bool,
    const REMOTE: bool,
>;

/// A capability set as values, for tables and runtime checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mask {
    pub spawn: bool,
    pub time: bool,
    pub random: bool,
    pub io: bool,
    pub remote: bool,
}

impl Mask {
    pub const NONE: Self = Self::of([false; 5]);
    pub const ALL: Self = Self::of([true; 5]);

    /// The set from its five bits in the plan's order.
    #[must_use]
    pub const fn of(bits: [bool; 5]) -> Self {
        Self {
            spawn: bits[0],
            time: bits[1],
            random: bits[2],
            io: bits[3],
            remote: bits[4],
        }
    }

    /// The five bits in the plan's order.
    #[must_use]
    pub const fn bits(self) -> [bool; 5] {
        [self.spawn, self.time, self.random, self.io, self.remote]
    }

    /// Whether `parent` holds every capability `self` holds.
    #[must_use]
    pub const fn within(self, parent: Self) -> bool {
        let (a, b) = (self.bits(), parent.bits());
        let mut i = 0;
        while i < 5 {
            if a[i] && !b[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    /// The capabilities either set holds.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        let (a, b) = (self.bits(), other.bits());
        Self::of([
            a[0] || b[0],
            a[1] || b[1],
            a[2] || b[2],
            a[3] || b[3],
            a[4] || b[4],
        ])
    }
}

/// The value of a [`CapSet`] type.
pub trait Caps {
    const MASK: Mask;
}

impl<const S: bool, const T: bool, const R: bool, const I: bool, const M: bool> Caps
    for CapSet<S, T, R, I, M>
{
    const MASK: Mask = Mask::of([S, T, R, I, M]);
}

/// The per-capability rule behind [`Within`]. Public only because [`Within`]'s impl names it; nothing outside this
/// crate can add a case, since both the type and the trait are this crate's.
#[doc(hidden)]
pub mod rule {
    /// One capability, held by the parent (`P`) and asked for by the child (`C`).
    pub struct Bit<const P: bool, const C: bool>;

    /// The child may hold a capability only if the parent does.
    pub trait Allowed {}

    impl Allowed for Bit<true, true> {}
    impl Allowed for Bit<true, false> {}
    impl Allowed for Bit<false, false> {}
}

use rule::{Allowed, Bit};

/// `Self` is within `P`: `P` holds every capability `Self` holds. There is no impl that adds one.
pub trait Within<P> {}

impl<
    const S: bool,
    const T: bool,
    const R: bool,
    const I: bool,
    const M: bool,
    const PS: bool,
    const PT: bool,
    const PR: bool,
    const PI: bool,
    const PM: bool,
> Within<CapSet<PS, PT, PR, PI, PM>> for CapSet<S, T, R, I, M>
where
    Bit<PS, S>: Allowed,
    Bit<PT, T>: Allowed,
    Bit<PR, R>: Allowed,
    Bit<PI, I>: Allowed,
    Bit<PM, M>: Allowed,
{
}

/// The capabilities as types, for [`Has`].
#[derive(Debug)]
pub struct Spawn;
#[derive(Debug)]
pub struct Time;
#[derive(Debug)]
pub struct Random;
#[derive(Debug)]
pub struct Io;
#[derive(Debug)]
pub struct Remote;

/// `Self` holds capability `K`.
pub trait Has<K> {}

impl<const T: bool, const R: bool, const I: bool, const M: bool> Has<Spawn>
    for CapSet<true, T, R, I, M>
{
}
impl<const S: bool, const R: bool, const I: bool, const M: bool> Has<Time>
    for CapSet<S, true, R, I, M>
{
}
impl<const S: bool, const T: bool, const I: bool, const M: bool> Has<Random>
    for CapSet<S, T, true, I, M>
{
}
impl<const S: bool, const T: bool, const R: bool, const M: bool> Has<Io>
    for CapSet<S, T, R, true, M>
{
}
impl<const S: bool, const T: bool, const R: bool, const I: bool> Has<Remote>
    for CapSet<S, T, R, I, true>
{
}

/// The node roles of `fmined` (§26.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Site,
    Regional,
    Enterprise,
    Relay,
}

impl Role {
    pub const ALL: [Self; 4] = [Self::Site, Self::Regional, Self::Enterprise, Self::Relay];
}

/// How a region restarts its children when one fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supervisor {
    /// The failed child and every child started after it.
    RestForOne,
    /// The failed child alone.
    OneForOne,
    /// Every child.
    OneForAll,
}

/// How many of a region a node runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instances {
    One,
    /// One per value of the named key, such as `cell` for `LedgerWriterRegion{cell}*`.
    PerKey(&'static str),
    /// Any number, with no key (`EstimationRunRegion*`).
    Many,
}

/// A region: its place in [`RegionKind`] and its capability set as a type.
pub trait Region {
    const KIND: RegionKind;
    type Caps: Caps;
}

/// One row of the region tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionSpec {
    pub kind: RegionKind,
    /// `None` for the process root.
    pub parent: Option<RegionKind>,
    pub caps: Mask,
    /// How it restarts its children, as the plan names it; `None` where it names none (the root and the leaves).
    pub supervisor: Option<Supervisor>,
    pub instances: Instances,
    /// The one role that runs it, or `None` for every role.
    pub only: Option<Role>,
}

const fn bit(digit: u8) -> bool {
    digit != 0
}

macro_rules! regions {
    ($($ty:ident = $kind:ident < $parent:ident: [$s:literal, $t:literal, $r:literal, $i:literal, $m:literal],
       $sup:expr, $inst:expr, $only:expr;)*) => {
        /// The regions of §26.6, one variant per row.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum RegionKind { $($kind),* }

        $(
            #[doc = concat!("The `", stringify!($ty), "` of §26.6.")]
            #[derive(Debug, Clone, Copy)]
            pub struct $ty;

            impl Region for $ty {
                const KIND: RegionKind = RegionKind::$kind;
                type Caps = CapSet<{ bit($s) }, { bit($t) }, { bit($r) }, { bit($i) }, { bit($m) }>;
            }

            // The edge to the parent, checked when the crate compiles: the region's set is within its parent's.
            const _: () = {
                const fn edge<C: Within<P>, P>() {}
                edge::<<$ty as Region>::Caps, <$parent as Region>::Caps>();
            };
        )*

        /// Every region, parents before children, in the plan's order.
        pub const REGIONS: &[RegionSpec] = &[$(RegionSpec {
            kind: RegionKind::$kind,
            parent: if matches!(RegionKind::$kind, RegionKind::Process) {
                None
            } else {
                Some(<$parent as Region>::KIND)
            },
            caps: Mask::of([bit($s), bit($t), bit($r), bit($i), bit($m)]),
            supervisor: $sup,
            instances: $inst,
            only: $only,
        }),*];
    };
}

use Instances::{Many, One, PerKey};
use Supervisor::{OneForAll, OneForOne, RestForOne};

// Type = kind < parent: [SPAWN, TIME, RANDOM, IO, REMOTE], supervisor, instances, role. Leaf rows are the plan's;
// an interior row is the union of its children's.
regions! {
    ProcessRegion = Process < ProcessRegion: [1, 1, 1, 1, 1], None, One, None;
    NodeRegion = Node < ProcessRegion: [1, 1, 0, 1, 1], Some(RestForOne), One, None;
    AuthorityRegion = Authority < NodeRegion: [0, 1, 0, 1, 0], Some(RestForOne), One, None;
    LedgerWriterRegion = LedgerWriter < AuthorityRegion: [0, 1, 0, 1, 0], None, PerKey("cell"), None;
    ObjectStoreRegion = ObjectStore < AuthorityRegion: [0, 0, 0, 1, 0], None, One, None;
    IngestRegion = Ingest < NodeRegion: [0, 1, 0, 1, 0], Some(OneForOne), One, Some(Role::Site);
    TelemetryRegion = Telemetry < IngestRegion: [0, 1, 0, 1, 0], None, PerKey("source"), Some(Role::Site);
    ImportQuarantineRegion = ImportQuarantine < IngestRegion: [0, 0, 0, 1, 0], None, PerKey("fmt"),
        Some(Role::Site);
    ComputeRegion = Compute < NodeRegion: [1, 0, 0, 0, 0], Some(OneForOne), One, None;
    LawRegion = Law < ComputeRegion: [0, 0, 0, 0, 0], None, One, None;
    EstimationRunRegion = EstimationRun < ComputeRegion: [1, 0, 0, 0, 0], None, Many, None;
    ProjectionRegion = Projection < NodeRegion: [1, 0, 0, 1, 0], None, One, None;
    SyncRegion = Sync < NodeRegion: [1, 1, 0, 1, 1], Some(OneForAll), One, None;
    PeerSessionRegion = PeerSession < SyncRegion: [1, 1, 0, 1, 1], None, PerKey("peer"), None;
    TransferRegion = Transfer < SyncRegion: [0, 1, 0, 1, 1], None, PerKey("root"), None;
    FinanceBridgeRegion = FinanceBridge < NodeRegion: [0, 1, 0, 1, 0], None, One, Some(Role::Enterprise);
    EffectRegion = Effect < NodeRegion: [0, 1, 0, 0, 0], None, One, None;
    OperationsRegion = Operations < NodeRegion: [1, 1, 0, 1, 0], None, One, None;
}

impl RegionKind {
    /// This region's row.
    #[must_use]
    pub fn spec(self) -> &'static RegionSpec {
        // `REGIONS` holds one row per variant, in variant order.
        &REGIONS[self as usize]
    }
}

/// The regions a node of `role` runs, parents before children.
#[must_use]
pub fn profile(role: Role) -> Vec<RegionSpec> {
    REGIONS
        .iter()
        .filter(|r| r.only.is_none_or(|only| only == role))
        .copied()
        .collect()
}

/// What code running in a region may do, as the type `C`. It narrows and never widens.
#[derive(Debug)]
pub struct Cx<C> {
    kind: RegionKind,
    caps: PhantomData<C>,
}

impl Cx<<ProcessRegion as Region>::Caps> {
    /// The process root, which holds every capability: the one source of ambient authority. Only the process
    /// entry point may call it. `xtask check-caps` bans the name outside this crate, so other code only receives a
    /// context and narrows it.
    #[must_use]
    pub const fn ambient_root() -> Self {
        Self {
            kind: RegionKind::Process,
            caps: PhantomData,
        }
    }
}

impl<C: Caps> Cx<C> {
    /// The region this context belongs to.
    #[must_use]
    pub const fn region(&self) -> RegionKind {
        self.kind
    }

    /// The capabilities, as values.
    #[must_use]
    pub const fn caps(&self) -> Mask {
        C::MASK
    }

    /// The context of region `R`, which compiles only when `R`'s set is within this one's.
    ///
    /// ```compile_fail,E0277
    /// use fmine_runtime::region::{ComputeRegion, Cx, LawRegion, NodeRegion, ProjectionRegion};
    /// // A law region holds nothing, so it cannot open the projection region, which holds IO.
    /// let law = Cx::ambient_root().open::<NodeRegion>().open::<ComputeRegion>().open::<LawRegion>();
    /// let _ = law.open::<ProjectionRegion>();
    /// ```
    #[must_use]
    pub fn open<R: Region>(&self) -> Cx<R::Caps>
    where
        R::Caps: Within<C>,
    {
        Cx {
            kind: R::KIND,
            caps: PhantomData,
        }
    }

    /// This region's context with fewer capabilities, as for a call into an engine (layers K and E get
    /// `CapSet<0,0,0,0,0>`, or SPAWN alone for estimation tiles). It compiles only when `D` is within `C`.
    ///
    /// ```compile_fail,E0277
    /// use fmine_runtime::region::{AuthorityRegion, CapSet, Cx, NodeRegion, ObjectStoreRegion};
    /// // The object store has no TIME, so its context cannot be widened to the ledger writer's.
    /// let node = Cx::ambient_root().open::<NodeRegion>();
    /// let store = node.open::<AuthorityRegion>().open::<ObjectStoreRegion>();
    /// let _ = store.narrow::<CapSet<false, true, false, true, false>>();
    /// ```
    #[must_use]
    pub fn narrow<D: Caps + Within<C>>(&self) -> Cx<D> {
        Cx {
            kind: self.kind,
            caps: PhantomData,
        }
    }

    /// The host wall clock, on a context holding TIME.
    ///
    /// ```compile_fail,E0277
    /// use fmine_runtime::region::{ComputeRegion, Cx, EstimationRunRegion, NodeRegion};
    /// // An estimation run gets its times as inputs; it has no clock.
    /// let compute = Cx::ambient_root().open::<NodeRegion>().open::<ComputeRegion>();
    /// let _ = compute.open::<EstimationRunRegion>().clock();
    /// ```
    #[must_use]
    pub fn clock(&self) -> HostWallClock
    where
        C: Has<Time>,
    {
        HostWallClock
    }

    /// Runs `command` through [`output_with_input`], on a context holding IO.
    ///
    /// ```compile_fail,E0277
    /// use fmine_runtime::region::{Cx, EffectRegion, NodeRegion};
    /// // The effect region prepares documents and never actuates: it has no IO.
    /// let effect = Cx::ambient_root().open::<NodeRegion>().open::<EffectRegion>();
    /// let _ = effect.run(&mut std::process::Command::new("true"), b"", 16);
    /// ```
    ///
    /// # Errors
    /// As [`output_with_input`].
    pub fn run(
        &self,
        command: &mut Command,
        input: &[u8],
        limit: usize,
    ) -> Result<PipedOutput, ProcessError>
    where
        C: Has<Io>,
    {
        output_with_input(command, input, limit)
    }
}

#[cfg(test)]
mod tests;
