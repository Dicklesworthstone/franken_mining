//! FrankenMining `S0` core substrate (`BD-016`..`BD-021`).
//! Determinism: BitwisePortable

pub mod anchor;
pub mod codec;
pub mod det;
pub mod hash;
pub mod id;
pub mod rand;

pub use anchor::{FMT_ANCHOR_V1, LedgerAnchor, SourceAnchor};
pub use codec::{CodecError, Decoder, Encoder, MAGIC_FMNE, crc32c};
pub use hash::{Blake3, Digest32, blake3_hash, sha256};
pub use id::{IdParseError, ObjectId, PrefixKind};
pub use rand::{PhiloxStream, RngKey, philox4x32_10, sobol_owen_2d};
