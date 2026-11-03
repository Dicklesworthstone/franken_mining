//! Determinism: BitwisePortable
//! A Merkle Mountain Range over batch digests, with RFC 6962 inclusion and consistency proofs (BD-083, §26.9 A2).
//!
//! The tree is RFC 6962's (RFC 9162 §2.1). A leaf hashes as SHA-256(0x00 ‖ entry) ([`leaf_hash`]) and a node as
//! SHA-256(0x01 ‖ left ‖ right) ([`mmr_parent`]). The peaks are the log's complete subtrees, largest first, one for
//! each set bit of its size. Bagging them right to left gives RFC 6962's tree hash, so a root here is a Certificate
//! Transparency root and the proofs are RFC 6962's `PATH` and `PROOF`. A consistency proof from size `m` to size
//! `n` has at most ⌈log₂ n⌉ + 1 hashes.

use core::fmt;

use fmine_core::hash::{Digest32, mmr_parent, sha256};

/// Why an MMR operation or a proof check refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmrError {
    /// The log already holds `u64::MAX` entries.
    Full,
    /// A size past the log's length.
    BeyondLog { size: u64, len: u64 },
    /// A leaf index that is not below the tree size.
    IndexOutOfRange { index: u64, size: u64 },
    /// A consistency proof from the empty tree, which proves nothing.
    EmptyFirst,
    /// A first size past the second.
    FirstAfterSecond { first: u64, second: u64 },
    /// More hashes than the sizes take.
    ProofTooLong,
    /// Fewer hashes than the sizes take.
    ProofTooShort,
    /// An inclusion proof that rebuilds a different root.
    RootMismatch,
    /// A consistency proof that rebuilds a different first root.
    FirstRootMismatch,
    /// A consistency proof that rebuilds a different second root, or equal sizes with different roots.
    SecondRootMismatch,
}

impl fmt::Display for MmrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "the log holds u64::MAX entries"),
            Self::BeyondLog { size, len } => {
                write!(f, "size {size} is past the log's {len} entries")
            }
            Self::IndexOutOfRange { index, size } => {
                write!(f, "leaf {index} is not in a tree of {size}")
            }
            Self::EmptyFirst => write!(f, "a consistency proof from the empty tree proves nothing"),
            Self::FirstAfterSecond { first, second } => {
                write!(f, "first size {first} is past second size {second}")
            }
            Self::ProofTooLong => write!(f, "the proof has more hashes than its sizes take"),
            Self::ProofTooShort => write!(f, "the proof has fewer hashes than its sizes take"),
            Self::RootMismatch => write!(f, "the proof rebuilds a different root"),
            Self::FirstRootMismatch => write!(f, "the proof rebuilds a different first root"),
            Self::SecondRootMismatch => write!(f, "the proof rebuilds a different second root"),
        }
    }
}

impl std::error::Error for MmrError {}

/// RFC 6962's leaf hash, SHA-256(0x00 ‖ entry). The ledger's entries are batch digests.
#[must_use]
pub fn leaf_hash(entry: &[u8]) -> Digest32 {
    let mut buf = Vec::with_capacity(entry.len() + 1);
    buf.push(0x00);
    buf.extend_from_slice(entry);
    sha256(&buf)
}

/// The empty tree's root, SHA-256 of no bytes (RFC 6962 §2.1).
#[must_use]
pub fn empty_root() -> Digest32 {
    sha256(&[])
}

/// The root of a peak list, largest peak first, bagged right to left. No peaks is the empty root.
#[must_use]
pub fn root_from_peaks(peaks: &[Digest32]) -> Digest32 {
    let mut rest = peaks.iter().rev();
    match rest.next() {
        None => empty_root(),
        Some(last) => rest.fold(*last, |acc, peak| mmr_parent(peak, &acc)),
    }
}

/// An append-only log of entries and every complete subtree over them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mmr {
    /// `levels[h][i]` is the hash of the complete subtree over leaves `i·2^h` to `(i+1)·2^h - 1`.
    levels: Vec<Vec<Digest32>>,
    len: u64,
}

impl Mmr {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of entries.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Whether the log has no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append an entry and return its index.
    pub fn push(&mut self, entry: &[u8]) -> Result<u64, MmrError> {
        let index = self.len;
        let len = index.checked_add(1).ok_or(MmrError::Full)?;
        let mut carry = Some(leaf_hash(entry));
        let mut height = 0;
        while let Some(node) = carry.take() {
            match self.levels.get_mut(height) {
                // A new top level holds one node, so nothing carries past it.
                None => self.levels.push(vec![node]),
                Some(level) => {
                    level.push(node);
                    if level.len().is_multiple_of(2)
                        && let [.., left, right] = level.as_slice()
                    {
                        carry = Some(mmr_parent(left, right));
                    }
                }
            }
            height += 1;
        }
        self.len = len;
        Ok(index)
    }

    /// The root of the whole log.
    #[must_use]
    pub fn root(&self) -> Digest32 {
        // A level with an odd count ends in a peak: the size's bit at that height is set.
        let peaks: Vec<Digest32> = self
            .levels
            .iter()
            .rev()
            .filter(|level| !level.len().is_multiple_of(2))
            .filter_map(|level| level.last().copied())
            .collect();
        root_from_peaks(&peaks)
    }

    /// The root of the log's first `size` entries.
    pub fn root_at(&self, size: u64) -> Result<Digest32, MmrError> {
        Ok(root_from_peaks(&self.peaks_at(size)?))
    }

    /// The peaks of the log's first `size` entries, largest first: the list a segment footer keeps.
    pub fn peaks_at(&self, size: u64) -> Result<Vec<Digest32>, MmrError> {
        self.within(size)?;
        (0..u64::BITS)
            .rev()
            .filter(|h| (size >> h) & 1 == 1)
            .map(|h| self.node(h, (size >> h) - 1, size))
            .collect()
    }

    /// RFC 6962's `PATH(index, D[size])`: the hashes that rebuild the root of the first `size` entries from entry
    /// `index`'s leaf hash, nearest first.
    pub fn inclusion_proof(&self, index: u64, size: u64) -> Result<Vec<Digest32>, MmrError> {
        self.within(size)?;
        if index >= size {
            return Err(MmrError::IndexOutOfRange { index, size });
        }
        let mut proof = Vec::new();
        self.path(index, 0, size, &mut proof)?;
        Ok(proof)
    }

    /// RFC 6962's `PROOF(first, D[second])`: the hashes that show the first `first` entries are a prefix of the
    /// first `second`. Equal sizes take no hashes.
    pub fn consistency_proof(&self, first: u64, second: u64) -> Result<Vec<Digest32>, MmrError> {
        self.within(second)?;
        if first == 0 {
            return Err(MmrError::EmptyFirst);
        }
        if first > second {
            return Err(MmrError::FirstAfterSecond { first, second });
        }
        let mut proof = Vec::new();
        if first < second {
            self.subproof(first, 0, second, true, &mut proof)?;
        }
        Ok(proof)
    }

    fn within(&self, size: u64) -> Result<(), MmrError> {
        if size > self.len {
            return Err(MmrError::BeyondLog {
                size,
                len: self.len,
            });
        }
        Ok(())
    }

    /// The stored subtree of height `height` at position `index` in its level, which a tree of `size` holds.
    fn node(&self, height: u32, index: u64, size: u64) -> Result<Digest32, MmrError> {
        let level = usize::try_from(height)
            .ok()
            .and_then(|h| self.levels.get(h));
        let index = usize::try_from(index).ok();
        level
            .zip(index)
            .and_then(|(level, i)| level.get(i))
            .copied()
            .ok_or(MmrError::BeyondLog {
                size,
                len: self.len,
            })
    }

    /// The hash of entries `lo` to `hi - 1`. RFC 6962's recursion only visits ranges whose start is a multiple of
    /// the least power of two at or above their width, so a power-of-two range is a stored subtree.
    fn range_hash(&self, lo: u64, hi: u64) -> Result<Digest32, MmrError> {
        let width = hi.checked_sub(lo).ok_or(MmrError::BeyondLog {
            size: hi,
            len: self.len,
        })?;
        if width.is_power_of_two() && lo.is_multiple_of(width) {
            return self.node(width.trailing_zeros(), lo >> width.trailing_zeros(), hi);
        }
        let k = split(width).ok_or(MmrError::BeyondLog {
            size: hi,
            len: self.len,
        })?;
        Ok(mmr_parent(
            &self.range_hash(lo, lo + k)?,
            &self.range_hash(lo + k, hi)?,
        ))
    }

    fn path(&self, m: u64, lo: u64, hi: u64, proof: &mut Vec<Digest32>) -> Result<(), MmrError> {
        let Some(k) = split(hi - lo) else {
            return Ok(());
        };
        if m < lo + k {
            self.path(m, lo, lo + k, proof)?;
            proof.push(self.range_hash(lo + k, hi)?);
        } else {
            self.path(m, lo + k, hi, proof)?;
            proof.push(self.range_hash(lo, lo + k)?);
        }
        Ok(())
    }

    /// RFC 6962's `SUBPROOF(m - lo, D[lo:hi], whole)`, for `lo < m <= hi`.
    fn subproof(
        &self,
        m: u64,
        lo: u64,
        hi: u64,
        whole: bool,
        proof: &mut Vec<Digest32>,
    ) -> Result<(), MmrError> {
        if m == hi {
            if !whole {
                proof.push(self.range_hash(lo, hi)?);
            }
            return Ok(());
        }
        let k = split(hi - lo).ok_or(MmrError::BeyondLog {
            size: hi,
            len: self.len,
        })?;
        if m <= lo + k {
            self.subproof(m, lo, lo + k, whole, proof)?;
            proof.push(self.range_hash(lo + k, hi)?);
        } else {
            self.subproof(m, lo + k, hi, false, proof)?;
            proof.push(self.range_hash(lo, lo + k)?);
        }
        Ok(())
    }
}

/// The largest power of two below `width`, for a width of 2 or more.
fn split(width: u64) -> Option<u64> {
    (width >= 2).then(|| 1 << (u64::BITS - 1 - (width - 1).leading_zeros()))
}

/// Check that `proof` rebuilds `root` from leaf `index`'s hash in a tree of `size` (RFC 9162 §2.1.3.2).
pub fn verify_inclusion(
    index: u64,
    size: u64,
    leaf: &Digest32,
    root: &Digest32,
    proof: &[Digest32],
) -> Result<(), MmrError> {
    if index >= size {
        return Err(MmrError::IndexOutOfRange { index, size });
    }
    let (mut f, mut s, mut r) = (index, size - 1, *leaf);
    for p in proof {
        if s == 0 {
            return Err(MmrError::ProofTooLong);
        }
        if f & 1 == 1 || f == s {
            r = mmr_parent(p, &r);
            while f & 1 == 0 && f != 0 {
                f >>= 1;
                s >>= 1;
            }
        } else {
            r = mmr_parent(&r, p);
        }
        f >>= 1;
        s >>= 1;
    }
    if s != 0 {
        return Err(MmrError::ProofTooShort);
    }
    if r != *root {
        return Err(MmrError::RootMismatch);
    }
    Ok(())
}

/// Check that `proof` shows the tree of `first` entries with `first_root` is a prefix of the tree of `second` with
/// `second_root` (RFC 9162 §2.1.4.2). Equal sizes take an empty proof and equal roots.
pub fn verify_consistency(
    first: u64,
    second: u64,
    first_root: &Digest32,
    second_root: &Digest32,
    proof: &[Digest32],
) -> Result<(), MmrError> {
    if first == 0 {
        return Err(MmrError::EmptyFirst);
    }
    if first > second {
        return Err(MmrError::FirstAfterSecond { first, second });
    }
    if first == second {
        if !proof.is_empty() {
            return Err(MmrError::ProofTooLong);
        }
        if first_root != second_root {
            return Err(MmrError::SecondRootMismatch);
        }
        return Ok(());
    }
    if proof.is_empty() {
        return Err(MmrError::ProofTooShort);
    }
    // A power-of-two first tree is one subtree of the second, and the verifier already has its hash.
    let (seed, path) = if first.is_power_of_two() {
        (first_root, proof)
    } else {
        proof.split_first().ok_or(MmrError::ProofTooShort)?
    };
    let (mut f, mut s) = (first - 1, second - 1);
    while f & 1 == 1 {
        f >>= 1;
        s >>= 1;
    }
    let (mut fr, mut sr) = (*seed, *seed);
    for c in path {
        if s == 0 {
            return Err(MmrError::ProofTooLong);
        }
        if f & 1 == 1 || f == s {
            fr = mmr_parent(c, &fr);
            sr = mmr_parent(c, &sr);
            while f & 1 == 0 && f != 0 {
                f >>= 1;
                s >>= 1;
            }
        } else {
            sr = mmr_parent(&sr, c);
        }
        f >>= 1;
        s >>= 1;
    }
    if s != 0 {
        return Err(MmrError::ProofTooShort);
    }
    if fr != *first_root {
        return Err(MmrError::FirstRootMismatch);
    }
    if sr != *second_root {
        return Err(MmrError::SecondRootMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
