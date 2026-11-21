//! FrankenMining S1 content-addressed object store (`fmine-object`, Bedrock, plan §26.5,
//! `BD-090`, `BD-091`, `PO-BED-3`).
//!
//! - [`merkle`]: fanout-16 SHA-256 Merkle tree and chunk inclusion proofs over CAS chunk digests;
//! - [`cas`]: `FMT-CAS-1` canonical chunk/manifest codec (`FMNE` + `CASM` + CRC32C, 4 MiB default chunk);
//! - [`store`]: root-last atomic CAS publication (`stage_object` → verify all chunks → `publish_staged`,
//!   `PO-BED-3`) with resumable staging receipts (`OB-SEAL`);
//! - [`gc`]: fail-closed reachability garbage collection from live ledger roots (`BD-091`).

pub mod cas;
pub mod gc;
pub mod merkle;
pub mod store;
