//! FrankenMining S1 ledger (Bedrock, plan §26). Today it holds the Merkle Mountain Range over batch digests and its
//! RFC 6962 proofs (BD-083). Delta batches, cells, leases, witnesses and segment files come with their own beads.

pub mod mmr;
