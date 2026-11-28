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
//! - **Indexes.** `drill_interval_box` is fsqlite's `rtree` and `drill_interval_text` its FTS5 over each
//!   interval's lithology, mineralization, weathering and alteration. Both are append-only: a box or text that
//!   changes gets a new index row under a new id (`bid`, `tid`), and `drill_interval` records which id is current.
//!   [`DrillStore::bbox`] and [`DrillStore::text`] look each index hit up by that id and drop the ones that are not
//!   current, so a superseded row never answers. The R-tree stores `f32` rounded outward (about 1 m at a northing
//!   of 8.6 × 10⁶ m), so [`DrillStore::bbox`] also checks each hit against the exact `f64` box: an answer is
//!   exactly the intervals whose box meets the query box. A box or text that a batch leaves as it was keeps its
//!   row.
//! - **Why append-only.** In fsqlite 0.5.1 a delete from either index costs a pass over the whole index (an FTS5
//!   delete by rowid, about 0.6 s per row at 10⁵ rows), and each statement that writes the R-tree copies it. So the
//!   store never deletes index rows in a batch, and writes each index in a few statements of up to
//!   [`RTREE_ROWS_PER_STATEMENT`] rows. [`DrillStore::index_garbage`] counts the superseded rows;
//!   [`DrillStore::rebuild_indexes`] drops both indexes and reloads them from `drill_interval`.
//! - **Deferred indexes.** A rebuild from the ledger can skip the indexes batch by batch
//!   ([`DrillStore::defer_indexes`]) and load them once ([`DrillStore::rebuild_indexes`]). Until then, and if a
//!   rebuild stopped half way, an index that is not current refuses its queries ([`DrillStoreError::IndexStale`]).
//!   Each index has its own state ([`DrillStore::index_states`]), so one that cannot be loaded leaves the other
//!   serving.
//! - **FTS5's limit.** fsqlite 0.5.1's FTS5 fails some segment writes ("pending hash produced no encodable segment
//!   leaves") when the text holds few distinct words, each repeated many times. Measured: 400 rows of one
//!   4,095-byte field repeating a word fail where 100 load; rows of the same four words fail between 10⁵ and
//!   2.5 × 10⁵ rows. Rows that share two words and add one of 97 others load at 5 × 10⁵, so where a real
//!   campaign's logging meets the limit is not known. A text write the engine fails marks the text index failed
//!   with its message, and the batch is applied again without it; a text load that fails in
//!   [`DrillStore::rebuild_indexes`] does the same. Ingest and [`DrillStore::bbox`] go on, and [`DrillStore::text`]
//!   refuses until a rebuild loads the text index.
//! - **The text index's limit.** Undoing a failed FTS5 statement removes its documents one by one: a
//!   single-statement load of 10⁷ intervals was still undoing itself 19 minutes into the index load, when the run
//!   was stopped. And a `MATCH` costs time in proportion to the index (0.67 s for 100 hits at 10⁶ rows). So the
//!   store keeps a text index for at most [`TEXT_INDEX_MAX_ROWS`] intervals (10⁵, where it was measured to load
//!   and to answer a 100-hit query in 59 ms), loads it in statements of 10,000 rows, and past the limit withholds
//!   it ([`IndexState::Withheld`]): `text` refuses and names INT-FSEARCH-001, which serves text at campaign scale.
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
use fsqlite::blocking::{Connection, Error as SqlError, Params, Row, ToSql, params};

use crate::mark::HighWaterMark;
use crate::projection::Projector;
use boxes::{Box3, interval_box};

/// The schema this module writes, recorded in `projection_meta`. A store with another is refused.
pub const SCHEMA: &str = "fmine.projection.drill.v2";
/// The most rows one R-tree statement writes.
pub const RTREE_ROWS_PER_STATEMENT: usize = 4096;
/// The most rows one FTS5 insert writes (two parameters each).
const TEXT_ROWS_PER_STATEMENT: usize = 512;
/// The most new interval rows one insert writes (sixteen parameters each; fsqlite takes up to 32,766).
const INTERVAL_ROWS_PER_STATEMENT: usize = 256;
/// The most new record rows one insert writes (eight parameters each).
const RECORD_ROWS_PER_STATEMENT: usize = 512;
/// The most keys one lookup of a batch's stored records or intervals binds.
const KEYS_PER_LOOKUP: usize = 512;
/// The most intervals the store keeps a text index for, by default (module documentation).
pub const TEXT_INDEX_MAX_ROWS: u64 = 100_000;
/// The most rows one statement of a text-index load writes: a failed statement is undone document by document.
const TEXT_LOAD_ROWS: i64 = if cfg!(test) { 64 } else { 10_000 };
/// What a text write is for; a failure of it marks the text index failed.
const ADD_TEXT: &str = "add text to the text index";
/// The stored state of an index the engine failed to write, before its message.
const FAILED: &str = "failed: ";
/// The stored state of a text index past its limit, before the reason.
const WITHHELD: &str = "withheld: ";

const META_DDL: &str =
    "CREATE TABLE IF NOT EXISTS projection_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)";
const DDL: &str = "
CREATE TABLE IF NOT EXISTS projection_mark(cell_id TEXT PRIMARY KEY, applied INTEGER NOT NULL, head BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS projection_counter(key TEXT PRIMARY KEY, value INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS drill_record(cell_id TEXT NOT NULL, key TEXT NOT NULL, kind TEXT NOT NULL,
  record_id TEXT NOT NULL, hole_id TEXT, versions INTEGER NOT NULL, seq INTEGER NOT NULL, payload BLOB NOT NULL,
  PRIMARY KEY(cell_id, key));
CREATE INDEX IF NOT EXISTS drill_record_hole ON drill_record(cell_id, hole_id, kind);
CREATE TABLE IF NOT EXISTS drill_hole(cell_id TEXT NOT NULL, hole_id TEXT NOT NULL, crs TEXT NOT NULL,
  located INTEGER NOT NULL, reason TEXT, PRIMARY KEY(cell_id, hole_id));
CREATE TABLE IF NOT EXISTS drill_interval(rid INTEGER PRIMARY KEY, cell_id TEXT NOT NULL, interval_id TEXT NOT NULL,
  hole_id TEXT NOT NULL, from_mm INTEGER NOT NULL, to_mm INTEGER NOT NULL, body TEXT NOT NULL, tid INTEGER NOT NULL,
  bid INTEGER, crs TEXT, min_e REAL, max_e REAL, min_n REAL, max_n REAL, min_z REAL, max_z REAL,
  UNIQUE(cell_id, interval_id));
CREATE INDEX IF NOT EXISTS drill_interval_hole ON drill_interval(cell_id, hole_id);
CREATE UNIQUE INDEX IF NOT EXISTS drill_interval_bid ON drill_interval(bid);
CREATE UNIQUE INDEX IF NOT EXISTS drill_interval_tid ON drill_interval(tid);
";
const CREATE_BOX_INDEX: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS drill_interval_box USING \
                                rtree(id, min_e, max_e, min_n, max_n, min_z, max_z)";
const CREATE_TEXT_INDEX: &str =
    "CREATE VIRTUAL TABLE IF NOT EXISTS drill_interval_text USING fts5(body)";
/// The columns [`read_prior`] reads, in its order.
const PRIOR_COLUMNS: &str = "bid, crs, min_e, max_e, min_n, max_n, min_z, max_z";

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
    /// An index does not hold every interval: it was deferred, its rebuild stopped, or (the text index) the engine
    /// failed to write it or the store holds more intervals than it is kept for. [`DrillStore::rebuild_indexes`]
    /// reloads it.
    IndexStale {
        /// Which index.
        index: Index,
        /// Why, for a text index that failed or is withheld.
        reason: Option<String>,
    },
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
            Self::IndexStale { .. } => Refusal::new(RefusalCode::StaleAnchor502, self.to_string()),
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
            Self::IndexStale { index, reason } => {
                write!(
                    f,
                    "drill projection: the {} index does not hold every interval (",
                    index.name()
                )?;
                match reason {
                    Some(why) => f.write_str(why)?,
                    None => f.write_str("deferred, or its rebuild stopped; rebuild it first")?,
                }
                f.write_str(")")
            }
            Self::Range { what, value } => write!(
                f,
                "drill projection: {what} {value} is past SQLite's integer range"
            ),
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

/// A CRS and box from their nullable columns: `None` unless every one is set and the box is well formed.
fn located(crs: Option<String>, v: [Option<f64>; 6]) -> Option<(String, Box3)> {
    match (crs, v) {
        (
            Some(crs),
            [
                Some(min_e),
                Some(max_e),
                Some(min_n),
                Some(max_n),
                Some(min_z),
                Some(max_z),
            ],
        ) => Box3::new([min_e, min_n, min_z], [max_e, max_n, max_z]).map(|b| (crs, b)),
        _ => None,
    }
}

/// [`PRIOR_COLUMNS`] from `r`, starting at column `at`.
fn read_prior(r: &Row<'_>, at: usize) -> Result<Prior, SqlError> {
    let mut v = [None; 6];
    for (k, slot) in v.iter_mut().enumerate() {
        *slot = r.get::<_, Option<f64>>(at + 2 + k)?;
    }
    Ok(Prior {
        bid: r.get(at)?,
        located: located(r.get(at + 1)?, v),
    })
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

/// Superseded index rows that no answer uses, kept until [`DrillStore::rebuild_indexes`], which empties both
/// counts. Exact while an index is current; a text index that failed or is withheld takes no writes and gains none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndexGarbage {
    /// R-tree rows of boxes since replaced or removed.
    pub stale_boxes: u64,
    /// FTS5 rows of texts since replaced.
    pub stale_texts: u64,
}

/// One of the two indexes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Index {
    /// The R-tree of interval boxes.
    Boxes,
    /// The FTS5 index of logging text.
    Texts,
}

impl Index {
    /// Its row in `projection_meta`.
    const fn key(self) -> &'static str {
        match self {
            Self::Boxes => "boxes",
            Self::Texts => "texts",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Boxes => "spatial",
            Self::Texts => "text",
        }
    }
}

/// The state of an index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexState {
    /// It holds a row for every box or text in `drill_interval`.
    Current,
    /// It was deferred, or its rebuild stopped.
    Stale,
    /// The engine failed to write it, with the engine's message.
    Failed(String),
    /// Not kept: the store holds more intervals than the text index is kept for ([`TEXT_INDEX_MAX_ROWS`]).
    Withheld(String),
}

/// The state of each index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexStates {
    /// The R-tree.
    pub boxes: IndexState,
    /// The text index.
    pub texts: IndexState,
}

/// An interval's box as the store held it before this batch.
#[derive(Clone)]
struct Prior {
    /// Its R-tree id; `None` when it had no box.
    bid: Option<i64>,
    located: Option<(String, Box3)>,
}

/// A stored interval as this batch found it: its row and text ids, its text and its box.
struct StoredInterval {
    rid: i64,
    tid: i64,
    body: String,
    prior: Prior,
}

/// What the store held, before this batch, of the records and intervals the batch writes, read a few statements at a
/// time. [`DrillStore::upsert_record`] and [`DrillStore::upsert_interval`] keep it current as they supersede, so a
/// second version in the same batch finds what a read of its row would.
#[derive(Default)]
struct Stored {
    /// Versions and hole of each stored record, by key.
    records: BTreeMap<String, (i64, Option<String>)>,
    /// Each stored interval, by interval id.
    intervals: BTreeMap<String, StoredInterval>,
}

/// `?first, ?first+1, …`, `n` placeholders.
fn placeholders(first: usize, n: usize) -> String {
    let mut text = String::with_capacity(n * 6);
    for k in 0..n {
        let sep = if k == 0 { "" } else { ", " };
        let _ = write!(text, "{sep}?{}", first + k);
    }
    text
}

/// An interval whose box must be (re)computed in this batch.
struct Span {
    hole_id: String,
    from_mm: u64,
    to_mm: u64,
    /// `None` for an interval new in this batch.
    prior: Option<Prior>,
}

/// A drill record new in this batch, inserted with the batch's other new records before any is read back.
struct NewRecord<'a> {
    kind: &'static str,
    record_id: &'a str,
    hole_id: Option<String>,
    versions: i64,
    seq: i64,
    payload: &'a [u8],
}

/// An interval new in this batch. Its row is written once, with its box, after the batch's traces are known.
struct NewInterval {
    rid: i64,
    tid: i64,
    hole_id: String,
    from_mm: i64,
    to_mm: i64,
    body: String,
}

/// A new interval's row as written: its box, if its hole has a trace, and its R-tree id.
struct NewRow<'a> {
    interval_id: &'a str,
    new: &'a NewInterval,
    bid: Option<i64>,
    crs: Option<String>,
    /// `min_e, max_e, min_n, max_n, min_z, max_z`.
    bounds: [Option<f64>; 6],
}

/// What writing an interval's row gives back.
struct Upserted {
    rid: i64,
    /// Its box before this batch; `None` for an interval new in this batch.
    prior: Option<Prior>,
    /// Its text with a new text id, when the text is new or changed.
    text: Option<(i64, String)>,
}

/// Row and index ids, and garbage counts. Restored when a transaction rolls back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counters {
    next_rid: i64,
    next_bid: i64,
    next_tid: i64,
    stale_boxes: i64,
    stale_texts: i64,
}

/// The fsqlite drillhole projection store. See the module documentation.
pub struct DrillStore {
    conn: Connection,
    defer: bool,
    text_limit: u64,
    counters: Counters,
    #[cfg(test)]
    crash_at: std::cell::Cell<Option<u64>>,
    #[cfg(test)]
    steps: std::cell::Cell<u64>,
}

impl fmt::Debug for DrillStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DrillStore")
            .field("path", &self.conn.path())
            .field("defer", &self.defer)
            .field("counters", &self.counters)
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
        // A commit does not wait for the disk: the mark commits with the rows it covers, so a power loss that drops
        // the last batches leaves a consistent store at an earlier mark, and catch-up applies them again.
        conn.execute_batch("PRAGMA synchronous = NORMAL")
            .map_err(sql("set synchronous"))?;
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
                let created = conn
                    .execute_batch(DDL)
                    .map_err(sql("create the tables"))
                    .and_then(|()| {
                        conn.execute(
                            "INSERT INTO projection_meta(key, value) VALUES ('schema', ?1), ('boxes', 'current'), \
                             ('texts', 'current')",
                            params![SCHEMA],
                        )
                        .map_err(sql("record the schema"))
                    })
                    .and_then(|_| {
                        conn.execute(
                            "INSERT INTO projection_counter(key, value) VALUES ('next_bid', 1), ('next_tid', 1), \
                             ('stale_boxes', 0), ('stale_texts', 0)",
                            params![],
                        )
                        .map_err(sql("start the counters"))
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
        // Recreate an index a stopped rebuild dropped; the store then records it as stale.
        for ddl in [CREATE_BOX_INDEX, CREATE_TEXT_INDEX] {
            conn.execute_batch(ddl)
                .map_err(sql("create an index table"))?;
        }
        let mut counters = Counters {
            next_rid: conn
                .query_row(
                    "SELECT COALESCE(MAX(rid), 0) + 1 FROM drill_interval",
                    params![],
                    |r| r.get::<_, i64>(0),
                )
                .map_err(sql("read the next interval row"))?,
            ..Counters::default()
        };
        let rows = conn
            .prepare("SELECT key, value FROM projection_counter")
            .and_then(|mut st| {
                st.query_map(params![], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sql("read the counters"))?;
        for (key, value) in rows {
            match key.as_str() {
                "next_bid" => counters.next_bid = value,
                "next_tid" => counters.next_tid = value,
                "stale_boxes" => counters.stale_boxes = value,
                "stale_texts" => counters.stale_texts = value,
                _ => {}
            }
        }
        if counters.next_bid < 1 || counters.next_tid < 1 {
            return Err(DrillStoreError::Sql {
                what: "read the counters",
                error: format!("next ids {counters:?} are not positive"),
            });
        }
        Ok(Self {
            conn,
            defer: false,
            text_limit: TEXT_INDEX_MAX_ROWS,
            counters,
            #[cfg(test)]
            crash_at: std::cell::Cell::new(None),
            #[cfg(test)]
            steps: std::cell::Cell::new(0),
        })
    }

    /// Skip the R-tree and text index batch by batch (`true`) or keep them current (`false`, the default). While
    /// deferred, each apply leaves a current index stale, and [`DrillStore::bbox`] and [`DrillStore::text`] refuse
    /// until [`DrillStore::rebuild_indexes`]. For a rebuild from the ledger: one load of each index instead of a
    /// write per batch.
    pub const fn defer_indexes(&mut self, defer: bool) {
        self.defer = defer;
    }

    /// Keep a text index for at most `rows` intervals ([`TEXT_INDEX_MAX_ROWS`] by default). A store that holds
    /// more withholds it: a current text index becomes [`IndexState::Withheld`] at the batch that passes the limit,
    /// and [`DrillStore::rebuild_indexes`] loads none. A raised limit takes effect at the next rebuild.
    pub const fn limit_text_index(&mut self, rows: u64) {
        self.text_limit = rows;
    }

    /// The intervals the store holds: their row ids run from 1, and no interval row is ever deleted.
    const fn intervals_held(&self) -> i64 {
        self.counters.next_rid - 1
    }

    /// The stored state of a text index past its limit, or `None` within it.
    fn text_withheld(&self) -> Option<String> {
        let held = self.intervals_held();
        u64::try_from(held).is_ok_and(|h| h > self.text_limit).then(|| {
            format!(
                "{WITHHELD}the store holds {held} intervals, past the {} its text index is kept for in \
                 fsqlite 0.5.1; text search over a full campaign is INT-FSEARCH-001's",
                self.text_limit
            )
        })
    }

    /// The state of each index.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails or a stored state is not one this module writes.
    pub fn index_states(&self) -> Result<IndexStates, DrillStoreError> {
        Ok(IndexStates {
            boxes: self.index_state(Index::Boxes)?,
            texts: self.index_state(Index::Texts)?,
        })
    }

    /// Whether both indexes hold a row for every box and text in `drill_interval`.
    ///
    /// # Errors
    /// As [`DrillStore::index_states`].
    pub fn indexes_are_current(&self) -> Result<bool, DrillStoreError> {
        let s = self.index_states()?;
        Ok(s.boxes == IndexState::Current && s.texts == IndexState::Current)
    }

    fn index_state(&self, index: Index) -> Result<IndexState, DrillStoreError> {
        let value = self
            .conn
            .prepare_cached("SELECT value FROM projection_meta WHERE key = ?1")
            .and_then(|mut st| st.query_row(params![index.key()], |r| r.get::<_, String>(0)))
            .map_err(sql("read an index state"))?;
        match value.as_str() {
            "current" => Ok(IndexState::Current),
            "stale" => Ok(IndexState::Stale),
            other => other
                .strip_prefix(FAILED)
                .map(|why| IndexState::Failed(why.to_string()))
                .or_else(|| {
                    other
                        .strip_prefix(WITHHELD)
                        .map(|why| IndexState::Withheld(why.to_string()))
                })
                .ok_or_else(|| DrillStoreError::Sql {
                    what: "read an index state",
                    error: format!("'{other}' is not a state this module writes"),
                }),
        }
    }

    /// Refuse unless `index` is current.
    fn require(&self, index: Index) -> Result<(), DrillStoreError> {
        match self.index_state(index)? {
            IndexState::Current => Ok(()),
            IndexState::Stale => Err(DrillStoreError::IndexStale {
                index,
                reason: None,
            }),
            IndexState::Failed(why) => Err(DrillStoreError::IndexStale {
                index,
                reason: Some(format!("the engine failed to write it: {why}")),
            }),
            IndexState::Withheld(why) => Err(DrillStoreError::IndexStale {
                index,
                reason: Some(why),
            }),
        }
    }

    /// The superseded index rows the store holds.
    #[must_use]
    pub fn index_garbage(&self) -> IndexGarbage {
        IndexGarbage {
            stale_boxes: self.counters.stale_boxes.unsigned_abs(),
            stale_texts: self.counters.stale_texts.unsigned_abs(),
        }
    }

    /// Drop the R-tree and text index and reload them from `drill_interval`, without superseded rows, marking each
    /// current once loaded; returns their states. fsqlite 0.5.1 cannot recreate a virtual table in the transaction
    /// that dropped it, so this is three transactions. The first marks both stale and drops them; the second
    /// recreates both and loads the R-tree; the third loads the text index, in statements of a bounded number of
    /// rows. A stop between them leaves an index not yet loaded stale, which its queries refuse and this repairs. A
    /// text load the engine fails (see the module documentation) leaves the text index failed with the engine's
    /// message, and the R-tree current; a store past its text limit ([`DrillStore::limit_text_index`]) loads no
    /// text index and leaves it withheld.
    ///
    /// # Errors
    /// [`DrillStoreError::Sql`] if the engine fails anything but the text load.
    pub fn rebuild_indexes(&mut self) -> Result<IndexStates, DrillStoreError> {
        self.in_transaction(|s| {
            s.set_index(Index::Boxes, "stale")?;
            s.set_index(Index::Texts, "stale")?;
            s.exec(
                "drop the spatial index",
                "DROP TABLE IF EXISTS drill_interval_box",
                params![],
            )?;
            s.exec(
                "drop the text index",
                "DROP TABLE IF EXISTS drill_interval_text",
                params![],
            )
            .map(|_| ())
        })?;
        self.in_transaction(|s| {
            s.exec("create the spatial index", CREATE_BOX_INDEX, params![])?;
            s.exec("create the text index", CREATE_TEXT_INDEX, params![])?;
            s.exec(
                "load the spatial index",
                "INSERT INTO drill_interval_box SELECT bid, min_e, max_e, min_n, max_n, min_z, max_z \
                 FROM drill_interval WHERE bid IS NOT NULL",
                params![],
            )?;
            s.counters.stale_boxes = 0;
            s.counters.stale_texts = 0;
            s.write_counters()?;
            s.set_index(Index::Boxes, "current")
        })?;
        let loaded = self.in_transaction(|s| {
            if let Some(why) = s.text_withheld() {
                return s.set_index(Index::Texts, &why);
            }
            let (mut from, last) = (0, s.intervals_held());
            while from < last {
                let to = last.min(from + TEXT_LOAD_ROWS);
                s.exec(
                    "load the text index",
                    "INSERT INTO drill_interval_text(rowid, body) SELECT tid, body FROM drill_interval \
                     WHERE rid > ?1 AND rid <= ?2",
                    params![from, to],
                )?;
                from = to;
            }
            s.set_index(Index::Texts, "current")
        });
        if let Err(e) = loaded {
            self.text_failed(&e)?;
        }
        self.index_states()
    }

    /// Record that the engine failed to write the text index, with its message.
    fn text_failed(&mut self, e: &DrillStoreError) -> Result<(), DrillStoreError> {
        let state = match e {
            DrillStoreError::Sql { what, error } => format!("{FAILED}{what}: {error}"),
            other => format!("{FAILED}{other}"),
        };
        self.in_transaction(|s| s.set_index(Index::Texts, &state))
    }

    /// The intervals whose box meets `query` in `crs`, sorted by cell and interval id.
    ///
    /// # Errors
    /// [`DrillStoreError::IndexStale`] while the R-tree is not current; [`DrillStoreError::Sql`] if the engine
    /// fails.
    pub fn bbox(&self, crs: &str, query: &Box3) -> Result<Vec<IntervalRef>, DrillStoreError> {
        self.require(Index::Boxes)?;
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
            .prepare_cached(&format!(
                "SELECT cell_id, interval_id, hole_id, {PRIOR_COLUMNS} FROM drill_interval WHERE bid = ?1"
            ))
            .map_err(sql("prepare the box refinement"))?;
        let mut out = Vec::new();
        for bid in candidates {
            let row = optional(look.query_row(params![bid], |r| {
                Ok((
                    IntervalRef {
                        cell_id: r.get(0)?,
                        interval_id: r.get(1)?,
                        hole_id: r.get(2)?,
                    },
                    read_prior(r, 3)?,
                ))
            }))
            .map_err(sql("refine a box candidate"))?;
            // No row: a superseded box.
            if let Some((at, prior)) = row
                && prior
                    .located
                    .is_some_and(|(c, b)| c == crs && b.meets(query))
            {
                out.push(at);
            }
        }
        out.sort();
        Ok(out)
    }

    /// The intervals whose logging text matches the FTS5 query `query`, sorted by cell and interval id.
    ///
    /// # Errors
    /// [`DrillStoreError::IndexStale`] while the text index is not current; [`DrillStoreError::Sql`] for a query
    /// FTS5 cannot parse, or if the engine fails.
    pub fn text(&self, query: &str) -> Result<Vec<IntervalRef>, DrillStoreError> {
        self.require(Index::Texts)?;
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT rowid FROM drill_interval_text WHERE drill_interval_text MATCH ?1",
            )
            .map_err(sql("prepare the text query"))?;
        let tids = st
            .query_map(params![query], |r| r.get::<_, i64>(0))
            .map_err(sql("query the text index"))?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(sql("read the text index"))?;
        let mut look = self
            .conn
            .prepare_cached(
                "SELECT cell_id, interval_id, hole_id FROM drill_interval WHERE tid = ?1",
            )
            .map_err(sql("prepare the text lookup"))?;
        let mut out = Vec::with_capacity(tids.len());
        for tid in tids {
            // No row: a superseded text.
            out.extend(
                optional(look.query_row(params![tid], |r| {
                    Ok(IntervalRef {
                        cell_id: r.get(0)?,
                        interval_id: r.get(1)?,
                        hole_id: r.get(2)?,
                    })
                }))
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
            .prepare_cached(&format!(
                "SELECT cell_id, interval_id, hole_id, from_mm, to_mm, body, {PRIOR_COLUMNS} FROM drill_interval"
            ))
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
                    read_prior(r, 6)?,
                ))
            })
            .map_err(sql("list the intervals"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql("read the interval listing"))?;
        let mut out = Vec::with_capacity(rows.len());
        for (at, from, to, body, prior) in rows {
            out.push(IntervalRow {
                at,
                from_mm: nat("from_mm", from)?,
                to_mm: nat("to_mm", to)?,
                body,
                located: prior.located,
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

    /// A test's crash point: after this many writes of what comes next, the process aborts.
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
        args: impl Params,
    ) -> Result<usize, DrillStoreError> {
        let n = self.conn.execute(sql_text, args).map_err(sql(what))?;
        self.step();
        Ok(n)
    }

    fn exec_cached(
        &self,
        what: &'static str,
        sql_text: &str,
        args: impl Params,
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
        let counters = self.counters;
        let done =
            body(self).and_then(|()| self.conn.execute_batch("COMMIT").map_err(sql("commit")));
        match done {
            Ok(()) => {
                self.step();
                Ok(())
            }
            Err(e) => {
                self.counters = counters;
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

    fn set_index(&self, index: Index, state: &str) -> Result<(), DrillStoreError> {
        self.exec_cached(
            "record an index state",
            "UPDATE projection_meta SET value = ?1 WHERE key = ?2",
            params![state, index.key()],
        )
        .map(|_| ())
    }

    fn write_counters(&self) -> Result<(), DrillStoreError> {
        let c = self.counters;
        self.exec_cached(
            "record the counters",
            "INSERT INTO projection_counter(key, value) VALUES ('next_bid', ?1), ('next_tid', ?2), \
             ('stale_boxes', ?3), ('stale_texts', ?4) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![c.next_bid, c.next_tid, c.stale_boxes, c.stale_texts],
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

    /// Read what the store holds of the records and intervals `batch` writes.
    fn read_stored(&self, batch: &DeltaBatch) -> Result<Stored, DrillStoreError> {
        let cell = batch.cell_id.as_str();
        let mut keys = Vec::new();
        let mut interval_ids = Vec::new();
        for op in &batch.deltas {
            if let Some((kind, id)) = parse_key(&op.key) {
                keys.push(op.key.as_str());
                if kind == RecordKind::Interval {
                    interval_ids.push(id);
                }
            }
        }
        let mut stored = Stored::default();
        for chunk in keys.chunks(KEYS_PER_LOOKUP) {
            let text = format!(
                "SELECT key, versions, hole_id FROM drill_record WHERE cell_id = ?1 AND key IN ({})",
                placeholders(2, chunk.len())
            );
            let mut args: Vec<&dyn ToSql> = Vec::with_capacity(chunk.len() + 1);
            args.push(&cell);
            args.extend(chunk.iter().map(|k| k as &dyn ToSql));
            let rows = self
                .conn
                .prepare_cached(&text)
                .and_then(|mut st| {
                    st.query_map(args.as_slice(), |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, Option<String>>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()
                })
                .map_err(sql("read a batch's stored records"))?;
            for (key, versions, hole) in rows {
                stored.records.insert(key, (versions, hole));
            }
        }
        for chunk in interval_ids.chunks(KEYS_PER_LOOKUP) {
            let text = format!(
                "SELECT interval_id, rid, tid, body, {PRIOR_COLUMNS} FROM drill_interval \
                 WHERE cell_id = ?1 AND interval_id IN ({})",
                placeholders(2, chunk.len())
            );
            let mut args: Vec<&dyn ToSql> = Vec::with_capacity(chunk.len() + 1);
            args.push(&cell);
            args.extend(chunk.iter().map(|k| k as &dyn ToSql));
            let rows = self
                .conn
                .prepare_cached(&text)
                .and_then(|mut st| {
                    st.query_map(args.as_slice(), |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            StoredInterval {
                                rid: r.get(1)?,
                                tid: r.get(2)?,
                                body: r.get(3)?,
                                prior: read_prior(r, 4)?,
                            },
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()
                })
                .map_err(sql("read a batch's stored intervals"))?;
            stored.intervals.extend(rows);
        }
        Ok(stored)
    }

    /// Write `key`'s new current version: supersede it in place if the store holds it (`stored` says), change it in
    /// `fresh` if this batch added it, and otherwise add it to `fresh`, whose rows [`DrillStore::write_new_records`]
    /// inserts. Returns the hole its previous version was on, if it had one.
    #[allow(clippy::too_many_arguments)]
    fn upsert_record<'a>(
        &self,
        cell: &str,
        key: &'a str,
        kind: RecordKind,
        id: &'a str,
        hole: Option<&str>,
        seq: i64,
        payload: &'a [u8],
        stored: &mut Stored,
        fresh: &mut BTreeMap<&'a str, NewRecord<'a>>,
    ) -> Result<Option<String>, DrillStoreError> {
        if let Some(new) = fresh.get_mut(key) {
            let prior_hole = std::mem::replace(&mut new.hole_id, hole.map(str::to_string));
            new.versions += 1;
            (new.seq, new.payload) = (seq, payload);
            return Ok(prior_hole);
        }
        if let Some((versions, held_hole)) = stored.records.get_mut(key) {
            *versions += 1;
            let prior_hole = std::mem::replace(held_hole, hole.map(str::to_string));
            self.exec_cached(
                "supersede a record",
                "UPDATE drill_record SET hole_id = ?1, versions = ?2, seq = ?3, payload = ?4 \
                 WHERE cell_id = ?5 AND key = ?6",
                params![hole, *versions, seq, payload, cell, key],
            )?;
            return Ok(prior_hole);
        }
        fresh.insert(
            key,
            NewRecord {
                kind: kind.code(),
                record_id: id,
                hole_id: hole.map(str::to_string),
                versions: 1,
                seq,
                payload,
            },
        );
        Ok(None)
    }

    /// Insert the records new in this batch, a few statements at a time.
    fn write_new_records(
        &self,
        cell: &str,
        fresh: &BTreeMap<&str, NewRecord<'_>>,
    ) -> Result<(), DrillStoreError> {
        const COLUMNS: usize = 8;
        let rows: Vec<(&&str, &NewRecord<'_>)> = fresh.iter().collect();
        for chunk in rows.chunks(RECORD_ROWS_PER_STATEMENT) {
            let mut text = String::from(
                "INSERT INTO drill_record(cell_id, key, kind, record_id, hole_id, versions, seq, payload) VALUES ",
            );
            let mut args: Vec<&dyn ToSql> = Vec::with_capacity(chunk.len() * COLUMNS);
            for (i, (key, new)) in chunk.iter().enumerate() {
                text.push_str(if i == 0 { "(" } else { ", (" });
                for k in 0..COLUMNS {
                    let sep = if k == 0 { "" } else { ", " };
                    let _ = write!(text, "{sep}?{}", i * COLUMNS + k + 1);
                }
                text.push(')');
                args.extend([
                    &cell as &dyn ToSql,
                    *key,
                    &new.kind,
                    &new.record_id,
                    &new.hole_id,
                    &new.versions,
                    &new.seq,
                    &new.payload,
                ]);
            }
            self.exec_cached("insert new records", &text, args.as_slice())?;
        }
        Ok(())
    }

    /// Write `iv`'s row: supersede it in place if the store holds it (`stored` says), change it in `fresh` if this
    /// batch added it, and otherwise add it to `fresh`, whose rows [`DrillStore::locate`] writes with their boxes.
    fn upsert_interval(
        &mut self,
        iv: &Interval,
        stored: &mut Stored,
        fresh: &mut BTreeMap<String, NewInterval>,
    ) -> Result<Upserted, DrillStoreError> {
        let (from, to, text) = (
            int("from_mm", iv.from_mm)?,
            int("to_mm", iv.to_mm)?,
            body(iv),
        );
        if let Some(new) = fresh.get_mut(&iv.interval_id) {
            // Added earlier in this batch, so it had no box before it.
            let new_text = if new.body == text {
                None
            } else {
                new.tid = self.counters.next_tid;
                self.counters.next_tid += 1;
                self.counters.stale_texts += 1;
                Some((new.tid, text.clone()))
            };
            new.hole_id.clone_from(&iv.hole_id);
            (new.from_mm, new.to_mm, new.body) = (from, to, text);
            return Ok(Upserted {
                rid: new.rid,
                prior: Some(Prior {
                    bid: None,
                    located: None,
                }),
                text: new_text,
            });
        }
        if let Some(held) = stored.intervals.get_mut(&iv.interval_id) {
            let new_text = if held.body == text {
                None
            } else {
                held.tid = self.counters.next_tid;
                self.counters.next_tid += 1;
                self.counters.stale_texts += 1;
                Some((held.tid, text.clone()))
            };
            self.exec_cached(
                "supersede an interval",
                "UPDATE drill_interval SET hole_id = ?1, from_mm = ?2, to_mm = ?3, body = ?4, tid = ?5 \
                 WHERE rid = ?6",
                params![iv.hole_id, from, to, text, held.tid, held.rid],
            )?;
            held.body = text;
            return Ok(Upserted {
                rid: held.rid,
                prior: Some(held.prior.clone()),
                text: new_text,
            });
        }
        let (rid, tid) = (self.counters.next_rid, self.counters.next_tid);
        self.counters.next_rid += 1;
        self.counters.next_tid += 1;
        fresh.insert(
            iv.interval_id.clone(),
            NewInterval {
                rid,
                tid,
                hole_id: iv.hole_id.clone(),
                from_mm: from,
                to_mm: to,
                body: text.clone(),
            },
        );
        Ok(Upserted {
            rid,
            prior: None,
            text: Some((tid, text)),
        })
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
        let stale_texts = self.counters.stale_texts;
        let mut dirty_holes = BTreeSet::new();
        let mut spans: BTreeMap<i64, Span> = BTreeMap::new();
        let mut fresh: BTreeMap<String, NewInterval> = BTreeMap::new();
        let mut fresh_records: BTreeMap<&str, NewRecord<'_>> = BTreeMap::new();
        let mut stored = self.read_stored(batch)?;
        let mut texts: Vec<(i64, String)> = Vec::new();
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
            let prior_hole = self.upsert_record(
                cell,
                &op.key,
                kind,
                id,
                hole_of(record),
                seq,
                &op.payload,
                &mut stored,
                &mut fresh_records,
            )?;
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
                    let Upserted { rid, prior, text } =
                        self.upsert_interval(iv, &mut stored, &mut fresh)?;
                    spans.insert(
                        rid,
                        Span {
                            hole_id: iv.hole_id.clone(),
                            from_mm: iv.from_mm,
                            to_mm: iv.to_mm,
                            prior,
                        },
                    );
                    texts.extend(text);
                }
                _ => {}
            }
        }
        // Before anything reads a record back: the traces below read collars and surveys.
        self.write_new_records(cell, &fresh_records)?;
        for hole in &dirty_holes {
            for (rid, from_mm, to_mm, prior) in self.hole_intervals(cell, hole)? {
                spans.entry(rid).or_insert(Span {
                    hole_id: hole.clone(),
                    from_mm,
                    to_mm,
                    prior: Some(prior),
                });
            }
        }
        let boxes = self.locate(cell, &dirty_holes, &spans, &fresh)?;
        let states = self.index_states()?;
        if matches!(
            states.texts,
            IndexState::Failed(_) | IndexState::Withheld(_)
        ) {
            // A text index that no longer takes writes gains no superseded rows.
            self.counters.stale_texts = stale_texts;
        }
        if states.boxes == IndexState::Current {
            if self.defer {
                self.set_index(Index::Boxes, "stale")?;
            } else {
                self.write_rtree(&boxes)?;
            }
        }
        if states.texts == IndexState::Current {
            if let Some(why) = self.text_withheld() {
                self.set_index(Index::Texts, &why)?;
            } else if self.defer {
                self.set_index(Index::Texts, "stale")?;
            } else {
                self.write_text(&texts)?;
            }
        }
        self.write_counters()?;
        self.exec_cached(
            "record the mark",
            "INSERT INTO projection_mark(cell_id, applied, head) VALUES (?1, ?2, ?3) \
             ON CONFLICT(cell_id) DO UPDATE SET applied = excluded.applied, head = excluded.head",
            params![cell, int("mark", mark.applied)?, mark.head.0.to_vec()],
        )?;
        Ok(())
    }

    /// Row id, from, to and box of every interval on `hole`.
    fn hole_intervals(
        &self,
        cell: &str,
        hole: &str,
    ) -> Result<Vec<(i64, u64, u64, Prior)>, DrillStoreError> {
        let rows = self
            .conn
            .prepare_cached(&format!(
                "SELECT rid, from_mm, to_mm, {PRIOR_COLUMNS} FROM drill_interval WHERE cell_id = ?1 AND hole_id = ?2"
            ))
            .and_then(|mut st| {
                st.query_map(params![cell, hole], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        read_prior(r, 3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sql("read a hole's intervals"))?;
        rows.into_iter()
            .map(|(rid, from, to, prior)| {
                Ok((rid, nat("from_mm", from)?, nat("to_mm", to)?, prior))
            })
            .collect()
    }

    /// Compute the boxes of `spans`, write the trace status of their holes, the rows of the intervals in `fresh` and
    /// the boxes that changed; returns the new boxes with their new R-tree ids.
    fn locate(
        &mut self,
        cell: &str,
        dirty_holes: &BTreeSet<String>,
        spans: &BTreeMap<i64, Span>,
        fresh: &BTreeMap<String, NewInterval>,
    ) -> Result<Vec<(i64, Box3)>, DrillStoreError> {
        let fresh_by_rid: BTreeMap<i64, (&str, &NewInterval)> = fresh
            .iter()
            .map(|(id, new)| (new.rid, (id.as_str(), new)))
            .collect();
        let mut rows = Vec::with_capacity(fresh.len());
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
            let now = traces
                .get(span.hole_id.as_str())
                .and_then(|t| t.as_ref().ok())
                .map(|t| {
                    (
                        t.crs().to_string(),
                        interval_box(t, span.from_mm, span.to_mm),
                    )
                });
            let new = fresh_by_rid.get(rid);
            let before = span.prior.as_ref().and_then(|p| p.located.clone());
            // A stored interval whose box is unchanged: nothing to write. (Only a new interval has no prior.)
            if new.is_none() && before == now {
                continue;
            }
            if span.prior.as_ref().is_some_and(|p| p.bid.is_some()) {
                self.counters.stale_boxes += 1;
            }
            let bid = now.as_ref().map(|_| {
                let bid = self.counters.next_bid;
                self.counters.next_bid += 1;
                bid
            });
            let (crs, lo, hi) = match &now {
                Some((crs, b)) => (Some(crs.as_str()), Some(b.min()), Some(b.max())),
                None => (None, None, None),
            };
            let at = |c: Option<[f64; 3]>, k: usize| c.map(|v| v[k]);
            if let Some(&(interval_id, new)) = new {
                rows.push(NewRow {
                    interval_id,
                    new,
                    bid,
                    crs: crs.map(str::to_string),
                    bounds: [
                        at(lo, 0),
                        at(hi, 0),
                        at(lo, 1),
                        at(hi, 1),
                        at(lo, 2),
                        at(hi, 2),
                    ],
                });
                if let (Some(bid), Some((_, b))) = (bid, now) {
                    boxes.push((bid, b));
                }
                continue;
            }
            self.exec_cached(
                "write an interval's box",
                "UPDATE drill_interval SET bid = ?1, crs = ?2, min_e = ?3, max_e = ?4, min_n = ?5, max_n = ?6, \
                 min_z = ?7, max_z = ?8 WHERE rid = ?9",
                params![
                    bid,
                    crs,
                    at(lo, 0),
                    at(hi, 0),
                    at(lo, 1),
                    at(hi, 1),
                    at(lo, 2),
                    at(hi, 2),
                    rid
                ],
            )?;
            if let (Some(bid), Some((_, b))) = (bid, now) {
                boxes.push((bid, b));
            }
        }
        self.write_new_rows(cell, &rows)?;
        Ok(boxes)
    }

    /// Insert the rows of the intervals new in this batch, with their boxes, a few statements at a time.
    fn write_new_rows(&self, cell: &str, rows: &[NewRow<'_>]) -> Result<(), DrillStoreError> {
        const COLUMNS: usize = 16;
        for chunk in rows.chunks(INTERVAL_ROWS_PER_STATEMENT) {
            let mut text = String::from(
                "INSERT INTO drill_interval(rid, cell_id, interval_id, hole_id, from_mm, to_mm, body, tid, bid, crs, \
                 min_e, max_e, min_n, max_n, min_z, max_z) VALUES ",
            );
            let mut args: Vec<&dyn ToSql> = Vec::with_capacity(chunk.len() * COLUMNS);
            for (i, row) in chunk.iter().enumerate() {
                text.push_str(if i == 0 { "(" } else { ", (" });
                for k in 0..COLUMNS {
                    let sep = if k == 0 { "" } else { ", " };
                    let _ = write!(text, "{sep}?{}", i * COLUMNS + k + 1);
                }
                text.push(')');
                let n = row.new;
                args.extend([
                    &n.rid as &dyn ToSql,
                    &cell,
                    &row.interval_id,
                    &n.hole_id,
                    &n.from_mm,
                    &n.to_mm,
                    &n.body,
                    &n.tid,
                    &row.bid,
                    &row.crs,
                ]);
                args.extend(row.bounds.iter().map(|v| v as &dyn ToSql));
            }
            self.exec_cached("insert new intervals", &text, args.as_slice())?;
        }
        Ok(())
    }

    /// Add `boxes` to the R-tree, a few statements at a time.
    fn write_rtree(&self, boxes: &[(i64, Box3)]) -> Result<(), DrillStoreError> {
        for chunk in boxes.chunks(RTREE_ROWS_PER_STATEMENT) {
            let mut text = String::from("INSERT INTO drill_interval_box VALUES ");
            for (i, (bid, b)) in chunk.iter().enumerate() {
                let (lo, hi) = (b.min(), b.max());
                let sep = if i == 0 { "" } else { ", " };
                // Display of a finite f64 is its shortest round-trip decimal, never an exponent.
                let _ = write!(
                    text,
                    "{sep}({bid}, {}, {}, {}, {}, {}, {})",
                    lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
                );
            }
            self.exec("add boxes to the spatial index", &text, params![])?;
        }
        Ok(())
    }

    /// Add `texts` to the text index, a few hundred rows a statement.
    fn write_text(&self, texts: &[(i64, String)]) -> Result<(), DrillStoreError> {
        for chunk in texts.chunks(TEXT_ROWS_PER_STATEMENT) {
            let mut text = String::from("INSERT INTO drill_interval_text(rowid, body) VALUES ");
            let mut args: Vec<&dyn ToSql> = Vec::with_capacity(chunk.len() * 2);
            for (i, (tid, body)) in chunk.iter().enumerate() {
                // fsqlite 0.5.1 takes numbered placeholders only; a bare `?` is an internal error.
                let sep = if i == 0 { "" } else { ", " };
                let _ = write!(text, "{sep}(?{}, ?{})", 2 * i + 1, 2 * i + 2);
                args.push(tid);
                args.push(body);
            }
            self.exec(ADD_TEXT, &text, args.as_slice())?;
        }
        Ok(())
    }
}

impl Projector for DrillStore {
    type Error = DrillStoreError;

    fn apply(&mut self, batch: &DeltaBatch, mark: &HighWaterMark) -> Result<(), DrillStoreError> {
        match self.in_transaction(|s| s.apply_batch(batch, mark)) {
            // The engine cannot write this text (module documentation): record that, then apply the batch without
            // the text index.
            Err(e @ DrillStoreError::Sql { what: ADD_TEXT, .. }) => {
                self.text_failed(&e)?;
                self.in_transaction(|s| s.apply_batch(batch, mark))
            }
            done => done,
        }
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
