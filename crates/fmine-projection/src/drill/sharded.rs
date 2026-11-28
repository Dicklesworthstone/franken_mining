//! The drill projection split over shard stores, each written by its own thread (`BD-173`, GATE-CORE-01).
//!
//! One fsqlite store is written through one connection, and a connection cannot leave the thread that opened it.
//! Separate stores in one process scale with the threads that write them: the GATE-CORE-01 ingest of 4.8 × 10⁵
//! intervals ran at 14,048 intervals/s into one store and 77,972 into eight. A [`ShardedDrillStore`] keeps `n`
//! [`DrillStore`]s, each on its own thread, and answers as one store does. The threads are scoped to a call
//! ([`ShardedDrillStore::with_shards_at`]) that runs the caller's work with the store and joins them before it
//! returns, so none is left running (INT-AS-001 criterion 9).
//!
//! - **Routing is by key alone.** Collars and surveys go to every shard, so each shard locates the intervals it holds;
//!   every other drill key goes to one shard, chosen by an FNV-1a hash of the key. A record never moves between
//!   shards, so one store keeps each key's version count, and a shard's share of a batch depends only on the batch.
//! - **A batch commits on every shard or on none.** Each shard writes its share and the mark in a transaction it
//!   leaves open, and each commits only once all have written; a shard that refuses rolls every one back. A stop
//!   between the commits leaves some shards one batch ahead. Until the rest hold it too, every answer is refused
//!   ([`DrillStoreError::ShardsApart`], `FMINE-REF-502`), so no answer mixes two states. The store's mark is the
//!   lowest of its shards', so catch-up offers that batch again, and a shard that holds it already skips it.
//! - **Answers merge.** A record is read from its key's shard. Interval listings and bbox and text answers are the
//!   shards' together, sorted. A hole's records gather its intervals, their samples and those samples' results from
//!   every shard. An index is current only when it is current on every shard. The text index limit
//!   ([`ShardedDrillStore::limit_text_index`]) applies to each shard, whose FTS5 index is its own.
//! - **A shard is opened only as itself.** Shard `i` of `n` records `"i/n"`, and opening it as another shard, or as a
//!   whole store, is refused ([`DrillStoreError::Shard`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use fmine_drill::records::{DrillRecord, RecordKind};
use fmine_drill::store::{KEY_PREFIX, parse_key};
use fmine_ledger::batch::DeltaBatch;

use super::boxes::Box3;
use super::{
    DrillStore, DrillStoreError, HoleRecords, IndexGarbage, IndexState, IndexStates, IntervalRef,
    IntervalRow, StoredRecord, UnlocatedHole, hole_records_through,
};
use crate::mark::HighWaterMark;
use crate::projection::Projector;

/// Work for a shard's thread.
type Job = Box<dyn FnOnce(&mut DrillStore) + Send>;

fn stopped() -> DrillStoreError {
    DrillStoreError::Sql {
        what: "reach a shard",
        error: "the shard's thread has stopped".to_string(),
    }
}

/// One shard: the channel to the thread that owns its store.
struct Shard {
    jobs: Sender<Job>,
}

impl Shard {
    /// Run `f` on the shard's store; its answer comes on the returned channel.
    fn send<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut DrillStore) -> R + Send + 'static,
    ) -> Result<Receiver<R>, DrillStoreError> {
        let (answer, answered) = mpsc::sync_channel(1);
        self.jobs
            .send(Box::new(move |store: &mut DrillStore| {
                let _ = answer.send(f(store));
            }))
            .map_err(|_| stopped())?;
        Ok(answered)
    }
}

fn wait<R>(answered: Receiver<R>) -> Result<R, DrillStoreError> {
    answered.recv().map_err(|_| stopped())
}

/// FNV-1a, 64-bit.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The shards an op of `key` goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    /// A collar or survey: every shard.
    Every,
    /// One shard.
    One(usize),
    /// Not a drill key: none.
    None,
}

fn route(key: &str, n: usize) -> Route {
    if !key.starts_with(KEY_PREFIX) {
        return Route::None;
    }
    match parse_key(key) {
        Some((RecordKind::Collar | RecordKind::Survey, _)) => Route::Every,
        Some(_) => Route::One(shard_of(key, n)),
        // Shard 0 refuses a key it cannot read, and the batch with it.
        None => Route::One(0),
    }
}

/// The shard of `n` that a drill key other than a collar's or a survey's goes to.
pub(crate) fn shard_of(key: &str, n: usize) -> usize {
    let n64 = u64::try_from(n).unwrap_or(u64::MAX);
    fnv1a(key.as_bytes())
        .checked_rem(n64)
        .and_then(|i| usize::try_from(i).ok())
        .unwrap_or(0)
}

/// Each shard's share of `batch`: the same batch with only the ops routed to it.
fn split(batch: &DeltaBatch, n: usize) -> Vec<DeltaBatch> {
    let mut parts: Vec<DeltaBatch> = (0..n)
        .map(|_| DeltaBatch {
            format_version: batch.format_version,
            cell_id: batch.cell_id.clone(),
            epoch: batch.epoch,
            seq: batch.seq,
            prev_hash: batch.prev_hash,
            anchor: batch.anchor.clone(),
            deltas: Vec::new(),
            witnesses: Vec::new(),
            cross_cell_refs: Vec::new(),
        })
        .collect();
    for op in &batch.deltas {
        match route(&op.key, n) {
            Route::Every => {
                for part in &mut parts {
                    part.deltas.push(op.clone());
                }
            }
            Route::One(i) => parts[i].deltas.push(op.clone()),
            Route::None => {}
        }
    }
    parts
}

/// The state of an index over all shards: the worst of theirs (failed, then withheld, then stale).
fn combine<'a>(states: impl Iterator<Item = &'a IndexState>) -> IndexState {
    let rank = |s: &IndexState| match s {
        IndexState::Current => 0,
        IndexState::Stale => 1,
        IndexState::Withheld(_) => 2,
        IndexState::Failed(_) => 3,
    };
    let mut out = IndexState::Current;
    for state in states {
        if rank(state) > rank(&out) {
            out = state.clone();
        }
    }
    out
}

/// The drill projection over `n` shard stores, alive inside [`ShardedDrillStore::with_shards_at`] or
/// [`ShardedDrillStore::with_shards_in_memory`], whose call its shards' threads do not outlive. See the module
/// documentation.
pub struct ShardedDrillStore<'scope> {
    shards: Vec<Shard>,
    /// Each shard's marks as it last committed or reported them.
    marks: Vec<BTreeMap<String, HighWaterMark>>,
    /// A test's stop between commits: only this many shards commit the next batch.
    #[cfg(test)]
    commit_only: Option<usize>,
    /// Ties the store to the scope its shards' threads run in, so it cannot leave it.
    scope: PhantomData<&'scope mut &'scope ()>,
}

impl fmt::Debug for ShardedDrillStore<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShardedDrillStore")
            .field("shards", &self.shards.len())
            .field("marks", &self.marks)
            .finish_non_exhaustive()
    }
}

/// Start a thread per shard in a scope, open shard `i` of `n` at `path(i)` (in memory for `None`) on it, and run
/// `body` with the store on the calling thread. Dropping the store closes the shards' channels, which ends their
/// threads, and the scope joins them.
fn with_shards<R>(
    n: usize,
    path: impl Fn(usize) -> Option<PathBuf>,
    body: impl for<'s> FnOnce(ShardedDrillStore<'s>) -> R,
) -> Result<R, DrillStoreError> {
    if n == 0 {
        return Err(DrillStoreError::Sql {
            what: "open a sharded store",
            error: "it needs at least one shard".to_string(),
        });
    }
    std::thread::scope(|scope| {
        let mut shards = Vec::with_capacity(n);
        for i in 0..n {
            let (jobs, inbox) = mpsc::channel::<Job>();
            let (ready, opened) = mpsc::sync_channel(1);
            let (label, at) = (format!("{i}/{n}"), path(i));
            scope.spawn(move || {
                let mut store = match DrillStore::open_shard(at.as_deref(), &label) {
                    Ok(store) => {
                        let _ = ready.send(Ok(()));
                        store
                    }
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        return;
                    }
                };
                for job in inbox {
                    job(&mut store);
                }
            });
            // A shard that fails to open ends the call; the shards already open close as `shards` drops.
            opened.recv().map_err(|_| stopped())??;
            shards.push(Shard { jobs });
        }
        let mut store = ShardedDrillStore {
            shards,
            marks: Vec::new(),
            #[cfg(test)]
            commit_only: None,
            scope: PhantomData,
        };
        store.marks = store.on_all(|s| s.stored_marks())?;
        Ok(body(store))
    })
}

impl ShardedDrillStore<'_> {
    /// Open the `n` shards in `dir` (`shard-<i>.db`), creating the directory and any shard that is new, and run
    /// `body` with the store. Each shard's store is opened and written by a thread of its own; the threads end
    /// when `body` returns and are joined before this does (no detached worker, INT-AS-001 criterion 9). The store
    /// cannot leave `body`.
    ///
    /// # Errors
    /// [`DrillStoreError::Shard`] for a shard built as another shard or as a whole store;
    /// [`DrillStoreError::Schema`] for a shard of another schema; [`DrillStoreError::Sql`] if the engine fails or
    /// `n` is 0. `body` does not run then.
    pub fn with_shards_at<R>(
        dir: impl AsRef<Path>,
        n: usize,
        body: impl for<'s> FnOnce(ShardedDrillStore<'s>) -> R,
    ) -> Result<R, DrillStoreError> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir).map_err(|e| DrillStoreError::Sql {
            what: "create the shard directory",
            error: format!("{}: {e}", dir.display()),
        })?;
        with_shards(n, |i| Some(dir.join(format!("shard-{i}.db"))), body)
    }

    /// As [`ShardedDrillStore::with_shards_at`], with `n` shards in memory, gone when `body` returns.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails or `n` is 0. `body` does not run then.
    pub fn with_shards_in_memory<R>(
        n: usize,
        body: impl for<'s> FnOnce(ShardedDrillStore<'s>) -> R,
    ) -> Result<R, DrillStoreError> {
        with_shards(n, |_| None, body)
    }

    /// How many shards the store has.
    #[must_use]
    pub const fn shards(&self) -> usize {
        self.shards.len()
    }

    /// Refuse to answer while a stop between commits leaves the shards at different batches of a cell.
    fn in_step(&self) -> Result<(), DrillStoreError> {
        let cells: BTreeSet<&String> = self.marks.iter().flat_map(BTreeMap::keys).collect();
        for cell in cells {
            let applied =
                |m: &BTreeMap<String, HighWaterMark>| m.get(cell).map_or(0, |h| h.applied);
            let behind = self.marks.iter().map(applied).min().unwrap_or(0);
            let ahead = self.marks.iter().map(applied).max().unwrap_or(0);
            if behind != ahead {
                return Err(DrillStoreError::ShardsApart {
                    cell_id: cell.clone(),
                    behind,
                    ahead,
                });
            }
        }
        Ok(())
    }

    /// Run `f` on every shard at once and gather the answers, failing with the first shard's error.
    fn on_all<R: Send + 'static>(
        &self,
        f: impl Fn(&mut DrillStore) -> Result<R, DrillStoreError> + Clone + Send + 'static,
    ) -> Result<Vec<R>, DrillStoreError> {
        let pending = self
            .shards
            .iter()
            .map(|s| s.send(f.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        pending.into_iter().map(|p| wait(p)?).collect()
    }

    /// Run `f` on shard `i`.
    fn on_one<R: Send + 'static>(
        &self,
        i: usize,
        f: impl FnOnce(&mut DrillStore) -> Result<R, DrillStoreError> + Send + 'static,
    ) -> Result<R, DrillStoreError> {
        wait(self.shards[i].send(f)?)?
    }

    /// As [`DrillStore::defer_indexes`], on every shard.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if a shard's thread has stopped.
    pub fn defer_indexes(&mut self, defer: bool) -> Result<(), DrillStoreError> {
        self.on_all(move |s| {
            s.defer_indexes(defer);
            Ok(())
        })
        .map(|_| ())
    }

    /// As [`DrillStore::limit_text_index`], for each shard's text index.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if a shard's thread has stopped.
    pub fn limit_text_index(&mut self, rows: u64) -> Result<(), DrillStoreError> {
        self.on_all(move |s| {
            s.limit_text_index(rows);
            Ok(())
        })
        .map(|_| ())
    }

    /// The state of each index over all shards.
    ///
    /// # Errors
    /// As [`DrillStore::index_states`].
    pub fn index_states(&self) -> Result<IndexStates, DrillStoreError> {
        let states = self.on_all(|s| s.index_states())?;
        Ok(IndexStates {
            boxes: combine(states.iter().map(|s| &s.boxes)),
            texts: combine(states.iter().map(|s| &s.texts)),
        })
    }

    /// Whether both indexes are current on every shard.
    ///
    /// # Errors
    /// As [`DrillStore::index_states`].
    pub fn indexes_are_current(&self) -> Result<bool, DrillStoreError> {
        let s = self.index_states()?;
        Ok(s.boxes == IndexState::Current && s.texts == IndexState::Current)
    }

    /// The superseded index rows all shards hold.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if a shard's thread has stopped.
    pub fn index_garbage(&self) -> Result<IndexGarbage, DrillStoreError> {
        let all = self.on_all(|s| Ok(s.index_garbage()))?;
        Ok(IndexGarbage {
            stale_boxes: all.iter().map(|g| g.stale_boxes).sum(),
            stale_texts: all.iter().map(|g| g.stale_texts).sum(),
        })
    }

    /// As [`DrillStore::rebuild_indexes`], on every shard at once, so the load's memory is every shard's together;
    /// returns the states over all shards.
    ///
    /// # Errors
    /// As [`DrillStore::rebuild_indexes`], from the first shard that fails.
    pub fn rebuild_indexes(&mut self) -> Result<IndexStates, DrillStoreError> {
        self.on_all(|s| s.rebuild_indexes())?;
        self.index_states()
    }

    /// As [`DrillStore::bbox`], over all shards.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::bbox`], from the
    /// first shard that refuses.
    pub fn bbox(&self, crs: &str, query: &Box3) -> Result<Vec<IntervalRef>, DrillStoreError> {
        self.in_step()?;
        let (crs, query) = (crs.to_string(), *query);
        let mut out = self.on_all(move |s| s.bbox(&crs, &query))?.concat();
        out.sort();
        Ok(out)
    }

    /// As [`DrillStore::text`], over all shards.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::text`], from the
    /// first shard that refuses.
    pub fn text(&self, query: &str) -> Result<Vec<IntervalRef>, DrillStoreError> {
        self.in_step()?;
        let query = query.to_string();
        let mut out = self.on_all(move |s| s.text(&query))?.concat();
        out.sort();
        Ok(out)
    }

    /// As [`DrillStore::record`], from the key's shard.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::record`].
    pub fn record(
        &self,
        cell_id: &str,
        key: &str,
    ) -> Result<Option<StoredRecord>, DrillStoreError> {
        self.in_step()?;
        let i = match route(key, self.shards.len()) {
            Route::Every => 0,
            Route::One(i) => i,
            Route::None => return Ok(None),
        };
        let (cell, key) = (cell_id.to_string(), key.to_string());
        self.on_one(i, move |s| s.record(&cell, &key))
    }

    /// As [`DrillStore::record_keys`], over all shards.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::record_keys`].
    pub fn record_keys(&self) -> Result<Vec<(String, String)>, DrillStoreError> {
        self.in_step()?;
        let keys: BTreeSet<(String, String)> = self
            .on_all(|s| s.record_keys())?
            .into_iter()
            .flatten()
            .collect();
        Ok(keys.into_iter().collect())
    }

    /// As [`DrillStore::intervals`], over all shards.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::intervals`].
    pub fn intervals(&self) -> Result<Vec<IntervalRow>, DrillStoreError> {
        self.in_step()?;
        let mut out = self.on_all(|s| s.intervals())?.concat();
        out.sort_by(|a, b| a.at.cmp(&b.at));
        Ok(out)
    }

    /// As [`DrillStore::unlocated_holes`]: every shard holds every collar and survey, so shard 0 answers.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::unlocated_holes`].
    pub fn unlocated_holes(&self) -> Result<Vec<UnlocatedHole>, DrillStoreError> {
        self.in_step()?;
        self.on_one(0, |s| s.unlocated_holes())
    }

    /// As [`DrillStore::holes`]: shard 0 answers.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::holes`].
    pub fn holes(&self) -> Result<Vec<(String, String)>, DrillStoreError> {
        self.in_step()?;
        self.on_one(0, |s| s.holes())
    }

    /// As [`DrillStore::hole_records`], gathered from every shard.
    ///
    /// # Errors
    /// [`DrillStoreError::ShardsApart`] after a stop between commits; otherwise as [`DrillStore::hole_records`].
    pub fn hole_records(
        &self,
        cell_id: &str,
        hole_id: &str,
    ) -> Result<HoleRecords, DrillStoreError> {
        self.in_step()?;
        hole_records_through(hole_id, |parents, kinds| {
            let cell = cell_id.to_string();
            let parents: Vec<String> = parents.iter().map(|p| (*p).to_string()).collect();
            let kinds = kinds.to_vec();
            let found: Vec<Vec<(String, DrillRecord)>> = self.on_all(move |s| {
                let parents: Vec<&str> = parents.iter().map(String::as_str).collect();
                s.children(&cell, &parents, &kinds)
            })?;
            Ok(found.concat())
        })
    }

    /// Reread every shard's marks.
    fn reread_marks(&mut self) -> Result<(), DrillStoreError> {
        self.marks = self.on_all(|s| s.stored_marks())?;
        Ok(())
    }

    /// A test's stop between commits: only the first `shards` shards to write the next batch commit it.
    #[cfg(test)]
    pub(crate) const fn commit_only(&mut self, shards: Option<usize>) {
        self.commit_only = shards;
    }

    /// How many intervals shard `i` holds.
    #[cfg(test)]
    pub(crate) fn shard_intervals(&self, i: usize) -> Result<usize, DrillStoreError> {
        self.on_one(i, |s| s.intervals().map(|rows| rows.len()))
    }

    /// Shard `i`'s own marks.
    #[cfg(test)]
    pub(crate) fn shard_marks(
        &self,
        i: usize,
    ) -> Result<BTreeMap<String, HighWaterMark>, DrillStoreError> {
        self.on_one(i, |s| s.stored_marks())
    }
}

impl Projector for ShardedDrillStore<'_> {
    type Error = DrillStoreError;

    fn apply(&mut self, batch: &DeltaBatch, mark: &HighWaterMark) -> Result<(), DrillStoreError> {
        let n = self.shards.len();
        let cell = batch.cell_id.as_str();
        // Which shards write the batch: a shard a stop left one batch ahead holds it already.
        let mut writes = Vec::new();
        for (i, part) in split(batch, n).into_iter().enumerate() {
            let held = self.marks[i].get(cell).copied().unwrap_or_default();
            if held.applied >= mark.applied {
                if held.applied == mark.applied && held.head != mark.head {
                    return Err(DrillStoreError::Sql {
                        what: "apply to a shard",
                        error: format!(
                            "shard {i}/{n} holds {} batches of cell '{cell}', the last of them another batch",
                            held.applied
                        ),
                    });
                }
                continue;
            }
            if held.applied + 1 != mark.applied {
                return Err(DrillStoreError::Sql {
                    what: "apply to a shard",
                    error: format!(
                        "shard {i}/{n} holds {} batches of cell '{cell}', and this batch makes {}",
                        held.applied, mark.applied
                    ),
                });
            }
            writes.push((i, part));
        }
        // Every shard writes its share in a transaction it leaves open.
        let mut failed = None;
        let mut pending = Vec::new();
        for (i, part) in writes {
            let mark = *mark;
            match self.shards[i].send(move |s| s.prepare(&part, &mark)) {
                Ok(p) => pending.push((i, p)),
                Err(e) => {
                    failed.get_or_insert(e);
                }
            }
        }
        let mut prepared = Vec::new();
        for (i, p) in pending {
            match wait(p).and_then(|r| r) {
                Ok(()) => prepared.push(i),
                Err(e) => {
                    failed.get_or_insert(e);
                }
            }
        }
        // Then all commit, or all roll back.
        let commit = failed.is_none();
        #[cfg(test)]
        let commit_only = self.commit_only.take();
        #[cfg(not(test))]
        let commit_only: Option<usize> = None;
        let mut finishing = Vec::new();
        for (k, i) in prepared.into_iter().enumerate() {
            let commit = commit && commit_only.is_none_or(|only| k < only);
            finishing.push((i, commit, self.shards[i].send(move |s| s.finish(commit))));
        }
        for (i, committed, p) in finishing {
            match p.and_then(wait).and_then(|r| r) {
                Ok(()) if committed => {
                    self.marks[i].insert(cell.to_string(), *mark);
                }
                Ok(()) => {}
                Err(e) => {
                    failed.get_or_insert(e);
                }
            }
        }
        if commit && let Some(only) = commit_only {
            failed.get_or_insert(DrillStoreError::Sql {
                what: "commit on every shard",
                error: format!("stopped after {only} commits"),
            });
        }
        match failed {
            None => Ok(()),
            Some(e) => {
                let _ = self.reread_marks();
                Err(e)
            }
        }
    }

    /// Each cell's lowest mark over the shards: the batches every shard holds. A cell a shard has not applied is at
    /// genesis there, and a cell at genesis is left out.
    fn stored_marks(&self) -> Result<BTreeMap<String, HighWaterMark>, DrillStoreError> {
        let all = self.on_all(|s| s.stored_marks())?;
        let cells: BTreeSet<&String> = all.iter().flat_map(BTreeMap::keys).collect();
        let mut out = BTreeMap::new();
        for cell in cells {
            let lowest = all
                .iter()
                .map(|m| m.get(cell).copied().unwrap_or_default())
                .min_by_key(|m| m.applied)
                .unwrap_or_default();
            if lowest.applied > 0 {
                out.insert(cell.clone(), lowest);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_sends_collars_and_surveys_everywhere_and_the_rest_to_one_shard() {
        assert_eq!(route("drill/collar/H1", 4), Route::Every);
        assert_eq!(route("drill/survey/H1-S0", 4), Route::Every);
        assert_eq!(route("note/1", 4), Route::None);
        assert_eq!(route("drill/widget/1", 4), Route::One(0));
        // FNV-1a's published test value: "a" hashes to 0xaf63dc4c8601ec8c.
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        let mut seen = [0_usize; 4];
        for k in 0..400 {
            let Route::One(i) = route(&format!("drill/interval/I{k:03}"), 4) else {
                panic!("an interval goes to one shard");
            };
            assert_eq!(route(&format!("drill/interval/I{k:03}"), 4), Route::One(i));
            seen[i] += 1;
        }
        assert!(seen.iter().all(|&c| c > 60), "{seen:?}");
    }
}
