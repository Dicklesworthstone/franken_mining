//! The fsqlite drillhole projection (`BD-121`, plan §26.1 and §27.4): every drill record's current version in
//! relational tables, each logging interval's 3-D box in an R-tree, and its logging text in FTS5.
//!
//! - **Tables.** `drill_record` holds the current version of each `drill/<kind>/<id>` key with its version count.
//!   `drill_hole` holds each collar's CRS and whether its trace could be desurveyed. `drill_interval` holds each
//!   logging interval: its hole, depths, text and box. `projection_mark` holds each cell's high-water mark.
//! - **Both or neither.** [`Projector::apply`] writes a batch's effect and its mark in one transaction and rolls it
//!   back on any error. Only `drill/` keys are projected; a cell's other keys belong to other projections.
//! - **Boxes.** An interval's box covers its desurveyed trace ([`boxes::interval_box`]). A collar or survey change
//!   recomputes the box of every interval on its hole. An interval on a hole that has no trace (no collar, no
//!   surveyed station, or a desurvey refusal) has no box, so [`DrillStore::bbox`] never returns it;
//!   [`DrillStore::unlocated_holes`] names those holes with the reason.
//! - **R-tree.** `drill_interval_box` is fsqlite's `rtree`. It stores `f32` rounded outward, about 1 m at a
//!   northing of 8.6 × 10⁶ m, so [`DrillStore::bbox`] checks each candidate against the exact `f64` box in
//!   `drill_interval`. An answer is exactly the intervals whose box meets the query box. fsqlite 0.5.1 copies the
//!   whole R-tree on the first write of each statement, so each batch writes it in at most a few statements of up
//!   to [`RTREE_ROWS_PER_STATEMENT`] rows. A rebuild can defer the R-tree and load it once at the end
//!   ([`DrillStore::defer_spatial_index`], [`DrillStore::rebuild_spatial_index`]). Until then
//!   [`DrillStore::bbox`] refuses ([`DrillStoreError::SpatialIndexStale`]).
//! - **Text.** `drill_interval_text` (FTS5) indexes each interval's lithology, mineralization, weathering and
//!   alteration. [`DrillStore::text`] takes an FTS5 query.
//!
//! The store is read and written through `fsqlite::blocking::Connection` only (INT-FSQL-001 criterion 10).

pub mod boxes;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::path::Path;

use fmine_core::hash::Digest32;
use fmine_drill::desurvey::{Trace, desurvey};
use fmine_drill::records::{DrillRecord, Interval, RecordKind, RecordVersion, decode_version};
use fmine_drill::store::{KEY_PREFIX, parse_key, record_key};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::batch::{DeltaBatch, DeltaKind};
use fsqlite::blocking::{Connection, Error as SqlError, ToSql, params};

use crate::mark::HighWaterMark;
use crate::projection::Projector;
use boxes::{Box3, interval_box};

/// The schema this module writes, recorded in `projection_meta`. A store with another is refused.
pub const SCHEMA: &str = "fmine.projection.drill.v1";
/// The most rows one R-tree statement writes or deletes.
pub const RTREE_ROWS_PER_STATEMENT: usize = 4096;
/// The most rows one FTS5 insert writes (two parameters each).
const TEXT_ROWS_PER_STATEMENT: usize = 512;

const META_DDL: &str =
    "CREATE TABLE IF NOT EXISTS projection_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)";
const DDL: &str = "
CREATE TABLE IF NOT EXISTS projection_mark(cell_id TEXT PRIMARY KEY, applied INTEGER NOT NULL, head BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS drill_record(cell_id TEXT NOT NULL, key TEXT NOT NULL, kind TEXT NOT NULL,
  record_id TEXT NOT NULL, hole_id TEXT, versions INTEGER NOT NULL, seq INTEGER NOT NULL, payload BLOB NOT NULL,
  PRIMARY KEY(cell_id, key));
CREATE INDEX IF NOT EXISTS drill_record_hole ON drill_record(cell_id, hole_id, kind);
CREATE TABLE IF NOT EXISTS drill_hole(cell_id TEXT NOT NULL, hole_id TEXT NOT NULL, crs TEXT NOT NULL,
  located INTEGER NOT NULL, reason TEXT, PRIMARY KEY(cell_id, hole_id));
CREATE TABLE IF NOT EXISTS drill_interval(rid INTEGER PRIMARY KEY, cell_id TEXT NOT NULL, interval_id TEXT NOT NULL,
  hole_id TEXT NOT NULL, from_mm INTEGER NOT NULL, to_mm INTEGER NOT NULL, body TEXT NOT NULL, crs TEXT,
  min_e REAL, max_e REAL, min_n REAL, max_n REAL, min_z REAL, max_z REAL, UNIQUE(cell_id, interval_id));
CREATE INDEX IF NOT EXISTS drill_interval_hole ON drill_interval(cell_id, hole_id);
CREATE VIRTUAL TABLE IF NOT EXISTS drill_interval_box USING rtree(id, min_e, max_e, min_n, max_n, min_z, max_z);
CREATE VIRTUAL TABLE IF NOT EXISTS drill_interval_text USING fts5(body);
";

/// A drill projection store that failed or refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrillStoreError {
    /// The engine failed a statement.
    Sql {
        /// What the statement was for.
        what: &'static str,
        /// The engine's message.
        error: String,
    },
    /// A `drill/` key whose payload is not a version of the record the key names.
    Record {
        /// The key.
        key: String,
        /// Why.
        error: String,
    },
    /// The store holds another schema's tables.
    Schema {
        /// The schema it records.
        found: String,
    },
    /// The R-tree does not hold every box yet: [`DrillStore::rebuild_spatial_index`] first.
    SpatialIndexStale,
    /// A depth, count or sequence beyond what SQLite's 64-bit integers hold.
    Range {
        /// What it was.
        what: &'static str,
        /// Its value.
        value: u64,
    },
}

impl DrillStoreError {
    /// The refusal this error stands for: a stale index is `FMINE-REF-502` (the index is older than the state it
    /// would answer for); the rest are `FMINE-REF-506` (the store cannot be read or written).
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::SpatialIndexStale => Refusal::new(RefusalCode::StaleAnchor502, self.to_string()),
            _ => Refusal::new(RefusalCode::InputMissingOrUnreadable506, self.to_string()),
        }
    }
}

impl fmt::Display for DrillStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql { what, error } => write!(f, "drill projection: {what}: {error}"),
            Self::Record { key, error } => write!(f, "drill projection: '{key}': {error}"),
            Self::Schema { found } => write!(
                f,
                "drill projection: the store holds schema '{found}', not '{SCHEMA}'; rebuild it from the ledger"
            ),
            Self::SpatialIndexStale => write!(
                f,
                "drill projection: the spatial index was deferred and does not hold every box; rebuild it first"
            ),
            Self::Range { what, value } => {
                write!(
                    f,
                    "drill projection: {what} {value} is past SQLite's integer range"
                )
            }
        }
    }
}

impl std::error::Error for DrillStoreError {}

fn sql(what: &'static str) -> impl FnOnce(SqlError) -> DrillStoreError {
    move |e| DrillStoreError::Sql {
        what,
        error: e.to_string(),
    }
}

fn int(what: &'static str, value: u64) -> Result<i64, DrillStoreError> {
    i64::try_from(value).map_err(|_| DrillStoreError::Range { what, value })
}

fn nat(what: &'static str, value: i64) -> Result<u64, DrillStoreError> {
    u64::try_from(value).map_err(|_| DrillStoreError::Sql {
        what,
        error: format!("negative value {value}"),
    })
}

fn optional<T>(r: Result<T, SqlError>) -> Result<Option<T>, SqlError> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(SqlError::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

fn decode(key: &str, payload: &[u8]) -> Result<RecordVersion, DrillStoreError> {
    decode_version(payload).map_err(|e| DrillStoreError::Record {
        key: key.to_string(),
        error: e.to_string(),
    })
}

/// An interval's logging text as FTS5 indexes it.
fn body(iv: &Interval) -> String {
    let mut out = [
        iv.lithology.as_str(),
        iv.mineralization.as_str(),
        iv.weathering.as_str(),
    ]
    .join(" ");
    if let Some(a) = &iv.alteration {
        let _ = write!(out, " {} {}", a.kind, a.intensity);
    }
    out
}

/// The hole a record belongs to, for the records that have one.
fn hole_of(record: &DrillRecord) -> Option<&str> {
    match record {
        DrillRecord::Collar(c) => Some(&c.hole_id),
        DrillRecord::Survey(s) => Some(&s.hole_id),
        DrillRecord::Interval(i) => Some(&i.hole_id),
        _ => None,
    }
}

/// An interval an answer names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IntervalRef {
    /// The cell.
    pub cell_id: String,
    /// The interval.
    pub interval_id: String,
    /// Its hole.
    pub hole_id: String,
}

/// An interval as the store holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct IntervalRow {
    /// Which interval.
    pub at: IntervalRef,
    /// From depth along the hole, mm.
    pub from_mm: u64,
    /// To depth along the hole, mm.
    pub to_mm: u64,
    /// The text FTS5 indexes: lithology, mineralization, weathering, then alteration kind and intensity.
    pub body: String,
    /// The hole's CRS and the interval's box, or `None` when the hole has no trace.
    pub located: Option<(String, Box3)>,
}

/// A record's current version as the store holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredRecord {
    /// Versions applied: 1 for a record never superseded.
    pub versions: u64,
    /// The cell sequence of the batch that wrote the current version.
    pub seq: u64,
    /// The current version.
    pub version: RecordVersion,
}

/// A collared hole the store could not desurvey.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnlocatedHole {
    /// The cell.
    pub cell_id: String,
    /// The hole.
    pub hole_id: String,
    /// Why it has no trace.
    pub reason: String,
}

/// An interval whose box must be (re)computed in this batch.
struct Span {
    hole_id: String,
    from_mm: u64,
    to_mm: u64,
    /// Whether it had a row before this batch, so possibly an R-tree row to replace.
    existed: bool,
}

/// The fsqlite drillhole projection store. See the module documentation.
pub struct DrillStore {
    conn: Connection,
    defer_index: bool,
    next_rid: i64,
    #[cfg(test)]
    crash_at: std::cell::Cell<Option<u64>>,
    #[cfg(test)]
    steps: std::cell::Cell<u64>,
}

impl fmt::Debug for DrillStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DrillStore")
            .field("path", &self.conn.path())
            .field("defer_index", &self.defer_index)
            .field("next_rid", &self.next_rid)
            .finish_non_exhaustive()
    }
}

impl DrillStore {
    /// Open the store at `path`, creating its tables if it is new.
    ///
    /// # Errors
    /// [`DrillStoreError::Schema`] for a store of another schema; [`DrillStoreError::Sql`] if the engine fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DrillStoreError> {
        Self::init(Connection::open(path).map_err(sql("open the store"))?)
    }

    /// A store in memory, gone when dropped.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails.
    pub fn open_in_memory() -> Result<Self, DrillStoreError> {
        Self::init(Connection::open_in_memory().map_err(sql("open a store in memory"))?)
    }

    fn init(conn: Connection) -> Result<Self, DrillStoreError> {
        conn.execute_batch(META_DDL)
            .map_err(sql("create projection_meta"))?;
        let found = optional(conn.query_row(
            "SELECT value FROM projection_meta WHERE key = 'schema'",
            params![],
            |r| r.get::<_, String>(0),
        ))
        .map_err(sql("read the schema"))?;
        match found {
            Some(found) if found != SCHEMA => return Err(DrillStoreError::Schema { found }),
            Some(_) => {}
            None => {
                conn.execute_batch("BEGIN").map_err(sql("begin"))?;
                let created = conn.execute_batch(DDL).map_err(sql("create the tables")).and_then(|()| {
                    conn.execute(
                        "INSERT INTO projection_meta(key, value) VALUES ('schema', ?1), ('spatial_index', 'current')",
                        params![SCHEMA],
                    )
                    .map_err(sql("record the schema"))
                });
                match created {
                    Ok(_) => conn
                        .execute_batch("COMMIT")
                        .map_err(sql("commit the tables"))?,
                    Err(e) => {
                        let _ = conn.execute_batch("ROLLBACK");
                        return Err(e);
                    }
                }
            }
        }
        let next_rid = conn
            .query_row(
                "SELECT COALESCE(MAX(rid), 0) + 1 FROM drill_interval",
                params![],
                |r| r.get::<_, i64>(0),
            )
            .map_err(sql("read the next interval row"))?;
        Ok(Self {
            conn,
            defer_index: false,
            next_rid,
            #[cfg(test)]
            crash_at: std::cell::Cell::new(None),
            #[cfg(test)]
            steps: std::cell::Cell::new(0),
        })
    }

    /// Defer R-tree writes (`true`) or keep the R-tree current batch by batch (`false`, the default). While
    /// deferred, each apply leaves the index stale, and [`DrillStore::bbox`] refuses until
    /// [`DrillStore::rebuild_spatial_index`]. For a rebuild from the ledger: one R-tree load instead of a write
    /// per batch, each of which copies the whole tree in fsqlite 0.5.1.
    pub const fn defer_spatial_index(&mut self, defer: bool) {
        self.defer_index = defer;
    }

    /// Whether the R-tree holds exactly the boxes in `drill_interval`.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails.
    pub fn spatial_index_is_current(&self) -> Result<bool, DrillStoreError> {
        let state = self
            .conn
            .query_row(
                "SELECT value FROM projection_meta WHERE key = 'spatial_index'",
                params![],
                |r| r.get::<_, String>(0),
            )
            .map_err(sql("read the spatial index state"))?;
        Ok(state == "current")
    }

    /// Reload the R-tree from the boxes in `drill_interval`, in one transaction, and mark it current.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails; the index is then as it was.
    pub fn rebuild_spatial_index(&mut self) -> Result<(), DrillStoreError> {
        self.in_transaction(|s| {
            s.exec("clear the spatial index", "DELETE FROM drill_interval_box", params![])?;
            s.exec(
                "load the spatial index",
                "INSERT INTO drill_interval_box SELECT rid, min_e, max_e, min_n, max_n, min_z, max_z \
                 FROM drill_interval WHERE min_e IS NOT NULL",
                params![],
            )?;
            s.set_spatial_index("current")
        })
    }

    /// The intervals whose box meets `query` in `crs`, sorted by cell and interval id.
    ///
    /// # Errors
    /// [`DrillStoreError::SpatialIndexStale`] while the index is deferred; [`DrillStoreError::Sql`] if the engine
    /// fails.
    pub fn bbox(&self, crs: &str, query: &Box3) -> Result<Vec<IntervalRef>, DrillStoreError> {
        if !self.spatial_index_is_current()? {
            return Err(DrillStoreError::SpatialIndexStale);
        }
        let (lo, hi) = (query.min(), query.max());
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT id FROM drill_interval_box WHERE max_e >= ?1 AND min_e <= ?2 AND max_n >= ?3 \
                 AND min_n <= ?4 AND max_z >= ?5 AND min_z <= ?6",
            )
            .map_err(sql("prepare the box query"))?;
        let candidates = st
            .query_map(params![lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(sql("query the spatial index"))?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(sql("read the spatial index"))?;
        let mut look = self
            .conn
            .prepare_cached(
                "SELECT cell_id, interval_id, hole_id, crs, min_e, max_e, min_n, max_n, min_z, max_z \
                 FROM drill_interval WHERE rid = ?1",
            )
            .map_err(sql("prepare the box refinement"))?;
        let mut out = Vec::new();
        for rid in candidates {
            let row = look
                .query_row(params![rid], |r| {
                    Ok((
                        IntervalRef {
                            cell_id: r.get(0)?,
                            interval_id: r.get(1)?,
                            hole_id: r.get(2)?,
                        },
                        r.get::<_, Option<String>>(3)?,
                        [r.get::<_, Option<f64>>(4)?, r.get(6)?, r.get(8)?],
                        [r.get::<_, Option<f64>>(5)?, r.get(7)?, r.get(9)?],
                    ))
                })
                .map_err(sql("refine a box candidate"))?;
            let (at, row_crs, min, max) = row;
            let exact = match (min, max) {
                ([Some(a), Some(b), Some(c)], [Some(x), Some(y), Some(z)]) => {
                    Box3::new([a, b, c], [x, y, z])
                }
                _ => None,
            };
            if row_crs.as_deref() == Some(crs) && exact.is_some_and(|b| b.meets(query)) {
                out.push(at);
            }
        }
        out.sort();
        Ok(out)
    }

    /// The intervals whose logging text matches the FTS5 query `query`, sorted by cell and interval id.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] for a query FTS5 cannot parse, or if the engine fails.
    pub fn text(&self, query: &str) -> Result<Vec<IntervalRef>, DrillStoreError> {
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT rowid FROM drill_interval_text WHERE drill_interval_text MATCH ?1",
            )
            .map_err(sql("prepare the text query"))?;
        let rids = st
            .query_map(params![query], |r| r.get::<_, i64>(0))
            .map_err(sql("query the text index"))?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(sql("read the text index"))?;
        let mut look = self
            .conn
            .prepare_cached(
                "SELECT cell_id, interval_id, hole_id FROM drill_interval WHERE rid = ?1",
            )
            .map_err(sql("prepare the text lookup"))?;
        let mut out = Vec::with_capacity(rids.len());
        for rid in rids {
            out.push(
                look.query_row(params![rid], |r| {
                    Ok(IntervalRef {
                        cell_id: r.get(0)?,
                        interval_id: r.get(1)?,
                        hole_id: r.get(2)?,
                    })
                })
                .map_err(sql("look up a text match"))?,
            );
        }
        out.sort();
        Ok(out)
    }

    /// The current version of `key` in `cell_id`, if it is recorded.
    ///
    /// # Errors
    /// [`DrillStoreError::Record`] for a payload that does not decode; [`DrillStoreError::Sql`] if the engine fails.
    pub fn record(
        &self,
        cell_id: &str,
        key: &str,
    ) -> Result<Option<StoredRecord>, DrillStoreError> {
        let row = optional(self.conn.query_row(
            "SELECT versions, seq, payload FROM drill_record WHERE cell_id = ?1 AND key = ?2",
            params![cell_id, key],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            },
        ))
        .map_err(sql("read a record"))?;
        row.map(|(versions, seq, payload)| {
            Ok(StoredRecord {
                versions: nat("versions", versions)?,
                seq: nat("seq", seq)?,
                version: decode(key, &payload)?,
            })
        })
        .transpose()
    }

    /// Every `(cell, key)` the store holds a record for, sorted.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails.
    pub fn record_keys(&self) -> Result<Vec<(String, String)>, DrillStoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT cell_id, key FROM drill_record")
            .map_err(sql("prepare the record listing"))?;
        let mut keys = st
            .query_map(params![], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(sql("list the records"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql("read the record listing"))?;
        keys.sort();
        Ok(keys)
    }

    /// Every interval the store holds, sorted by cell and interval id.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails.
    pub fn intervals(&self) -> Result<Vec<IntervalRow>, DrillStoreError> {
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT cell_id, interval_id, hole_id, from_mm, to_mm, body, crs, min_e, max_e, min_n, max_n, \
                 min_z, max_z FROM drill_interval",
            )
            .map_err(sql("prepare the interval listing"))?;
        let rows = st
            .query_map(params![], |r| {
                Ok((
                    IntervalRef {
                        cell_id: r.get(0)?,
                        interval_id: r.get(1)?,
                        hole_id: r.get(2)?,
                    },
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    [r.get::<_, Option<f64>>(7)?, r.get(9)?, r.get(11)?],
                    [r.get::<_, Option<f64>>(8)?, r.get(10)?, r.get(12)?],
                ))
            })
            .map_err(sql("list the intervals"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql("read the interval listing"))?;
        let mut out = Vec::with_capacity(rows.len());
        for (at, from, to, body, crs, min, max) in rows {
            let located = match (crs, min, max) {
                (Some(crs), [Some(a), Some(b), Some(c)], [Some(x), Some(y), Some(z)]) => {
                    Box3::new([a, b, c], [x, y, z]).map(|b| (crs, b))
                }
                _ => None,
            };
            out.push(IntervalRow {
                at,
                from_mm: nat("from_mm", from)?,
                to_mm: nat("to_mm", to)?,
                body,
                located,
            });
        }
        out.sort_by(|a, b| a.at.cmp(&b.at));
        Ok(out)
    }

    /// The collared holes that have no trace, with the reason, sorted.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails.
    pub fn unlocated_holes(&self) -> Result<Vec<UnlocatedHole>, DrillStoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT cell_id, hole_id, reason FROM drill_hole WHERE located = 0")
            .map_err(sql("prepare the unlocated listing"))?;
        let mut out = st
            .query_map(params![], |r| {
                Ok(UnlocatedHole {
                    cell_id: r.get(0)?,
                    hole_id: r.get(1)?,
                    reason: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            })
            .map_err(sql("list the unlocated holes"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql("read the unlocated listing"))?;
        out.sort();
        Ok(out)
    }

    /// A test's crash point: after this many writes of the next apply, the process aborts.
    #[cfg(test)]
    pub(crate) fn crash_after(&self, writes: Option<u64>) {
        self.steps.set(0);
        self.crash_at.set(writes);
    }

    /// Writes counted since [`DrillStore::crash_after`].
    #[cfg(test)]
    pub(crate) fn writes(&self) -> u64 {
        self.steps.get()
    }

    /// One write done: where a test crash can happen.
    fn step(&self) {
        #[cfg(test)]
        {
            let n = self.steps.get() + 1;
            self.steps.set(n);
            if self.crash_at.get() == Some(n) {
                std::process::abort();
            }
        }
    }

    fn exec(
        &self,
        what: &'static str,
        sql_text: &str,
        args: impl fsqlite::blocking::Params,
    ) -> Result<usize, DrillStoreError> {
        let n = self.conn.execute(sql_text, args).map_err(sql(what))?;
        self.step();
        Ok(n)
    }

    fn exec_cached(
        &self,
        what: &'static str,
        sql_text: &str,
        args: impl fsqlite::blocking::Params,
    ) -> Result<usize, DrillStoreError> {
        let n = self
            .conn
            .prepare_cached(sql_text)
            .and_then(|mut st| st.execute(args))
            .map_err(sql(what))?;
        self.step();
        Ok(n)
    }

    /// Run `body` in one transaction: commit on success, roll back on any error.
    fn in_transaction(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<(), DrillStoreError>,
    ) -> Result<(), DrillStoreError> {
        self.conn.execute_batch("BEGIN").map_err(sql("begin"))?;
        self.step();
        let next_rid = self.next_rid;
        let done =
            body(self).and_then(|()| self.conn.execute_batch("COMMIT").map_err(sql("commit")));
        match done {
            Ok(()) => {
                self.step();
                Ok(())
            }
            Err(e) => {
                self.next_rid = next_rid;
                match self.conn.execute_batch("ROLLBACK") {
                    Ok(()) => Err(e),
                    Err(r) => Err(DrillStoreError::Sql {
                        what: "roll back",
                        error: format!("{r}, after: {e}"),
                    }),
                }
            }
        }
    }

    fn set_spatial_index(&self, state: &str) -> Result<(), DrillStoreError> {
        self.exec_cached(
            "record the spatial index state",
            "UPDATE projection_meta SET value = ?1 WHERE key = 'spatial_index'",
            params![state],
        )
        .map(|_| ())
    }

    fn version(&self, cell: &str, key: &str) -> Result<Option<RecordVersion>, DrillStoreError> {
        let payload = optional(
            self.conn
                .prepare_cached("SELECT payload FROM drill_record WHERE cell_id = ?1 AND key = ?2")
                .and_then(|mut st| st.query_row(params![cell, key], |r| r.get::<_, Vec<u8>>(0))),
        )
        .map_err(sql("read a record"))?;
        payload.map(|p| decode(key, &p)).transpose()
    }

    /// Write `key`'s new current version; returns the hole its previous version was on, if it had one.
    #[allow(clippy::too_many_arguments)]
    fn upsert_record(
        &self,
        cell: &str,
        key: &str,
        kind: RecordKind,
        id: &str,
        hole: Option<&str>,
        seq: i64,
        payload: &[u8],
    ) -> Result<Option<String>, DrillStoreError> {
        let prior = optional(
            self.conn
                .prepare_cached(
                    "SELECT versions, hole_id FROM drill_record WHERE cell_id = ?1 AND key = ?2",
                )
                .and_then(|mut st| {
                    st.query_row(params![cell, key], |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                    })
                }),
        )
        .map_err(sql("read a record's versions"))?;
        match &prior {
            None => self.exec_cached(
                "insert a record",
                "INSERT INTO drill_record(cell_id, key, kind, record_id, hole_id, versions, seq, payload) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)",
                params![cell, key, kind.code(), id, hole, seq, payload],
            )?,
            Some((versions, _)) => self.exec_cached(
                "supersede a record",
                "UPDATE drill_record SET hole_id = ?1, versions = ?2, seq = ?3, payload = ?4 \
                 WHERE cell_id = ?5 AND key = ?6",
                params![hole, versions + 1, seq, payload, cell, key],
            )?,
        };
        Ok(prior.and_then(|(_, h)| h))
    }

    /// Write `iv`'s row (not its box); returns its row id and whether it had one before.
    fn upsert_interval(
        &mut self,
        cell: &str,
        iv: &Interval,
    ) -> Result<(i64, bool), DrillStoreError> {
        let prior = optional(
            self.conn
                .prepare_cached(
                    "SELECT rid FROM drill_interval WHERE cell_id = ?1 AND interval_id = ?2",
                )
                .and_then(|mut st| {
                    st.query_row(params![cell, iv.interval_id], |r| r.get::<_, i64>(0))
                }),
        )
        .map_err(sql("read an interval's row"))?;
        let (from, to, text) = (
            int("from_mm", iv.from_mm)?,
            int("to_mm", iv.to_mm)?,
            body(iv),
        );
        if let Some(rid) = prior {
            self.exec_cached(
                "supersede an interval",
                "UPDATE drill_interval SET hole_id = ?1, from_mm = ?2, to_mm = ?3, body = ?4 WHERE rid = ?5",
                params![iv.hole_id, from, to, text, rid],
            )?;
            return Ok((rid, true));
        }
        let rid = self.next_rid;
        self.exec_cached(
            "insert an interval",
            "INSERT INTO drill_interval(rid, cell_id, interval_id, hole_id, from_mm, to_mm, body) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![rid, cell, iv.interval_id, iv.hole_id, from, to, text],
        )?;
        self.next_rid += 1;
        Ok((rid, false))
    }

    /// The trace of `hole` from its current collar and surveys, or why it has none.
    fn trace(&self, cell: &str, hole: &str) -> Result<Result<Trace, String>, DrillStoreError> {
        let key = record_key(RecordKind::Collar, hole);
        let Some(version) = self.version(cell, &key)? else {
            return Ok(Err("no collar is recorded".to_string()));
        };
        let DrillRecord::Collar(collar) = version.record else {
            return Err(DrillStoreError::Record {
                key,
                error: "a collar key holds another record".to_string(),
            });
        };
        let rows = self
            .conn
            .prepare_cached(
                "SELECT key, payload FROM drill_record WHERE cell_id = ?1 AND hole_id = ?2 AND kind = ?3",
            )
            .and_then(|mut st| {
                st.query_map(params![cell, hole, RecordKind::Survey.code()], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sql("read a hole's surveys"))?;
        let mut surveys = Vec::with_capacity(rows.len());
        for (key, payload) in rows {
            match decode(&key, &payload)?.record {
                DrillRecord::Survey(s) => surveys.push(s),
                _ => {
                    return Err(DrillStoreError::Record {
                        key,
                        error: "a survey key holds another record".to_string(),
                    });
                }
            }
        }
        surveys.sort_by(|a, b| a.survey_id.cmp(&b.survey_id));
        Ok(desurvey(&collar, &surveys).map_err(|e| e.to_string()))
    }

    fn apply_batch(
        &mut self,
        batch: &DeltaBatch,
        mark: &HighWaterMark,
    ) -> Result<(), DrillStoreError> {
        let cell = batch.cell_id.as_str();
        let seq = int("batch seq", batch.seq)?;
        let mut dirty_holes = BTreeSet::new();
        let mut spans: BTreeMap<i64, Span> = BTreeMap::new();
        let mut texts: BTreeMap<i64, (String, bool)> = BTreeMap::new();
        for op in &batch.deltas {
            if !op.key.starts_with(KEY_PREFIX) {
                continue;
            }
            let Some((kind, id)) = parse_key(&op.key) else {
                return Err(DrillStoreError::Record {
                    key: op.key.clone(),
                    error: "not a drill record key this projection knows".to_string(),
                });
            };
            if !matches!(op.kind, DeltaKind::Put | DeltaKind::Supersede) {
                return Err(DrillStoreError::Record {
                    key: op.key.clone(),
                    error: format!(
                        "{:?} on a drill record, which is only put and superseded",
                        op.kind
                    ),
                });
            }
            let version = decode(&op.key, &op.payload)?;
            let record = &version.record;
            if record.kind() != kind || record.id() != id {
                return Err(DrillStoreError::Record {
                    key: op.key.clone(),
                    error: format!("the payload is {} '{}'", record.kind().code(), record.id()),
                });
            }
            let prior_hole =
                self.upsert_record(cell, &op.key, kind, id, hole_of(record), seq, &op.payload)?;
            match record {
                DrillRecord::Collar(c) => {
                    let crs = c
                        .actual
                        .as_ref()
                        .or(c.planned.as_ref())
                        .map(|p| p.crs.as_str());
                    self.exec_cached(
                        "write a hole",
                        "INSERT INTO drill_hole(cell_id, hole_id, crs, located, reason) VALUES (?1, ?2, ?3, 0, ?4) \
                         ON CONFLICT(cell_id, hole_id) DO UPDATE SET crs = excluded.crs",
                        params![cell, c.hole_id, crs.unwrap_or_default(), "not yet desurveyed"],
                    )?;
                    dirty_holes.insert(c.hole_id.clone());
                }
                DrillRecord::Survey(s) => {
                    dirty_holes.insert(s.hole_id.clone());
                    dirty_holes.extend(prior_hole);
                }
                DrillRecord::Interval(iv) => {
                    let (rid, existed) = self.upsert_interval(cell, iv)?;
                    spans.insert(
                        rid,
                        Span {
                            hole_id: iv.hole_id.clone(),
                            from_mm: iv.from_mm,
                            to_mm: iv.to_mm,
                            existed,
                        },
                    );
                    texts.insert(rid, (body(iv), existed));
                }
                _ => {}
            }
        }
        for hole in &dirty_holes {
            for (rid, from, to) in self.hole_intervals(cell, hole)? {
                spans.entry(rid).or_insert(Span {
                    hole_id: hole.clone(),
                    from_mm: from,
                    to_mm: to,
                    existed: true,
                });
            }
        }
        let boxes = self.locate(cell, &dirty_holes, &spans)?;
        if self.defer_index || !self.spatial_index_is_current()? {
            self.set_spatial_index("stale")?;
        } else {
            let replaced: Vec<i64> = spans
                .iter()
                .filter(|(_, s)| s.existed)
                .map(|(rid, _)| *rid)
                .collect();
            self.write_rtree(&replaced, &boxes)?;
        }
        self.write_text(&texts)?;
        self.exec_cached(
            "record the mark",
            "INSERT INTO projection_mark(cell_id, applied, head) VALUES (?1, ?2, ?3) \
             ON CONFLICT(cell_id) DO UPDATE SET applied = excluded.applied, head = excluded.head",
            params![cell, int("mark", mark.applied)?, mark.head.0.to_vec()],
        )?;
        Ok(())
    }

    /// Row id, from and to of every interval on `hole`.
    fn hole_intervals(
        &self,
        cell: &str,
        hole: &str,
    ) -> Result<Vec<(i64, u64, u64)>, DrillStoreError> {
        let rows = self
            .conn
            .prepare_cached("SELECT rid, from_mm, to_mm FROM drill_interval WHERE cell_id = ?1 AND hole_id = ?2")
            .and_then(|mut st| {
                st.query_map(params![cell, hole], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sql("read a hole's intervals"))?;
        rows.into_iter()
            .map(|(rid, from, to)| Ok((rid, nat("from_mm", from)?, nat("to_mm", to)?)))
            .collect()
    }

    /// Compute and write the boxes of `spans` and the trace status of their holes; returns the located boxes.
    fn locate(
        &self,
        cell: &str,
        dirty_holes: &BTreeSet<String>,
        spans: &BTreeMap<i64, Span>,
    ) -> Result<Vec<(i64, Box3)>, DrillStoreError> {
        let mut holes: BTreeSet<&str> = dirty_holes.iter().map(String::as_str).collect();
        holes.extend(spans.values().map(|s| s.hole_id.as_str()));
        let mut traces = BTreeMap::new();
        for hole in holes {
            let trace = self.trace(cell, hole)?;
            let (located, reason) = match &trace {
                Ok(_) => (1_i64, None),
                Err(why) => (0, Some(why.as_str())),
            };
            self.exec_cached(
                "record a hole's trace status",
                "UPDATE drill_hole SET located = ?1, reason = ?2 WHERE cell_id = ?3 AND hole_id = ?4",
                params![located, reason, cell, hole],
            )?;
            traces.insert(hole, trace);
        }
        let mut boxes = Vec::new();
        for (rid, span) in spans {
            let located = traces
                .get(span.hole_id.as_str())
                .and_then(|t| t.as_ref().ok())
                .map(|t| (t.crs(), interval_box(t, span.from_mm, span.to_mm)));
            let (crs, lo, hi) = match &located {
                Some((crs, b)) => (Some(*crs), Some(b.min()), Some(b.max())),
                None => (None, None, None),
            };
            let at = |c: Option<[f64; 3]>, k: usize| c.map(|v| v[k]);
            self.exec_cached(
                "write an interval's box",
                "UPDATE drill_interval SET crs = ?1, min_e = ?2, max_e = ?3, min_n = ?4, max_n = ?5, \
                 min_z = ?6, max_z = ?7 WHERE rid = ?8",
                params![crs, at(lo, 0), at(hi, 0), at(lo, 1), at(hi, 1), at(lo, 2), at(hi, 2), rid],
            )?;
            if let Some((_, b)) = located {
                boxes.push((*rid, b));
            }
        }
        Ok(boxes)
    }

    /// Replace the R-tree rows of `replaced` with `boxes`, a few statements at a time.
    fn write_rtree(&self, replaced: &[i64], boxes: &[(i64, Box3)]) -> Result<(), DrillStoreError> {
        for chunk in replaced.chunks(RTREE_ROWS_PER_STATEMENT) {
            let ids: Vec<String> = chunk.iter().map(i64::to_string).collect();
            self.exec(
                "remove boxes from the spatial index",
                &format!(
                    "DELETE FROM drill_interval_box WHERE id IN ({})",
                    ids.join(", ")
                ),
                params![],
            )?;
        }
        for chunk in boxes.chunks(RTREE_ROWS_PER_STATEMENT) {
            let mut text = String::from("INSERT INTO drill_interval_box VALUES ");
            for (i, (rid, b)) in chunk.iter().enumerate() {
                let (lo, hi) = (b.min(), b.max());
                let sep = if i == 0 { "" } else { ", " };
                // Display of a finite f64 is its shortest round-trip decimal, never an exponent.
                let _ = write!(
                    text,
                    "{sep}({rid}, {}, {}, {}, {}, {}, {})",
                    lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
                );
            }
            self.exec("add boxes to the spatial index", &text, params![])?;
        }
        Ok(())
    }

    /// Replace the FTS5 rows of `texts`: delete the ones that existed, then insert all.
    fn write_text(&self, texts: &BTreeMap<i64, (String, bool)>) -> Result<(), DrillStoreError> {
        let existed: Vec<String> = texts
            .iter()
            .filter(|(_, (_, e))| *e)
            .map(|(rid, _)| rid.to_string())
            .collect();
        for chunk in existed.chunks(RTREE_ROWS_PER_STATEMENT) {
            self.exec(
                "remove text from the text index",
                &format!(
                    "DELETE FROM drill_interval_text WHERE rowid IN ({})",
                    chunk.join(", ")
                ),
                params![],
            )?;
        }
        let rows: Vec<(i64, &str)> = texts
            .iter()
            .map(|(rid, (t, _))| (*rid, t.as_str()))
            .collect();
        for chunk in rows.chunks(TEXT_ROWS_PER_STATEMENT) {
            let mut text = String::from("INSERT INTO drill_interval_text(rowid, body) VALUES ");
            let mut args: Vec<&dyn ToSql> = Vec::with_capacity(chunk.len() * 2);
            for (i, (rid, body)) in chunk.iter().enumerate() {
                // fsqlite 0.5.1 takes numbered placeholders only; a bare `?` is an internal error.
                let sep = if i == 0 { "" } else { ", " };
                let _ = write!(text, "{sep}(?{}, ?{})", 2 * i + 1, 2 * i + 2);
                args.push(rid);
                args.push(body);
            }
            self.exec("add text to the text index", &text, args.as_slice())?;
        }
        Ok(())
    }
}

impl Projector for DrillStore {
    type Error = DrillStoreError;

    fn apply(&mut self, batch: &DeltaBatch, mark: &HighWaterMark) -> Result<(), DrillStoreError> {
        self.in_transaction(|s| s.apply_batch(batch, mark))
    }

    fn stored_marks(&self) -> Result<BTreeMap<String, HighWaterMark>, DrillStoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT cell_id, applied, head FROM projection_mark")
            .map_err(sql("prepare the mark listing"))?;
        let rows = st
            .query_map(params![], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(sql("list the marks"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql("read the marks"))?;
        let mut marks = BTreeMap::new();
        for (cell, applied, head) in rows {
            let head: [u8; 32] = head.try_into().map_err(|h: Vec<u8>| DrillStoreError::Sql {
                what: "read a mark",
                error: format!("cell '{cell}' has a {}-byte head, not 32", h.len()),
            })?;
            marks.insert(
                cell,
                HighWaterMark {
                    applied: nat("applied", applied)?,
                    head: Digest32(head),
                },
            );
        }
        Ok(marks)
    }
}

#[cfg(test)]
mod tests;
