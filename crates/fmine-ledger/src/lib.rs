//! FrankenMining S1 ledger (Bedrock, plan §26).
//!
//! Holds the durable L1 authority primitives:
//! - [`mmr`] (`BD-083`): RFC 6962 Merkle Mountain Range over batch digests with inclusion and consistency proofs;
//! - [`batch`] (`BD-081`): `FMT-LEDGER-1` canonical delta batch codec (`FMNE` + `LDGR`, CRC32C, SHA-256 chain);
//! - [`segment`] (`BD-082`): `FMT-SEG-1` append-only segment files with per-record CRC32C and MMR peak footer;
//! - [`cell`] (`BD-084`): single-writer cell leases, monotonic fencing epochs (`PO-BED-1`, `F6`), and gluing check (`F12`);
//! - [`verify`] (`BD-088`): hash-chain verification and digest-bound `F7` torn-tail repair plan/apply (`PO-ADIT-3`).

pub mod as_of_replay;
pub mod batch;
pub mod cell;
pub mod l0;
pub mod mmr;
pub mod segment;
pub mod verify;
