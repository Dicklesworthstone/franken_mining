//! Bedrock projections (`BD-120`, plan §26.1): fsqlite, fgdb and frankensearch views are derived from the ledger,
//! rebuildable, high-water-marked and never consulted as truth.
//!
//! [`mark`] holds the per-cell [`mark::HighWaterMark`], the [`mark::Anchor`] of all of them, stamped answers and a
//! projection's [`mark::Status`] against the log. [`projection`] holds the [`projection::Projector`] trait that a
//! store implements and the [`projection::Projection`] that feeds it batches in order under those marks.

pub mod mark;
pub mod projection;
