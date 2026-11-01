//! Provenance: frankensim crates/fs-sparse/src/lib.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence LicenseRef-MIT-OpenAI-Anthropic-Rider; Changes: Coo::new, Coo::push, Csr::from_parts and Csr::spmv refuse with SparseError instead of panicking, and from_parts also refuses a row pointer past nnz; Csr::row and Csr::get return None outside the matrix; spmv runs its scalar loop directly; tests take a test_ prefix; Left out: BSR, SELL-C-sigma, CsrCompact, spmm, identity, to_dense, try_from_parts_with_checkpoint, the x86 FMA capsule, the accelerator pilot, ops (transpose, symmetrize, SpGEMM), nested dissection, the FrankenNetworkx and FrankenNumpy interop, VERSION
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).
//!
//! Sparse matrices (BD-145, plan §27.1 wave 0): COO staging with deterministic assembly, canonical CSR and its
//! SpMV. [`direct`] holds the AMD ordering and the supernodal LDLᵀ; [`precond`] holds PCG and ILU(0).
//!
//! Assembly canonicalizes through a stable sort keyed by (row, col, insertion order): the resulting matrix is a
//! pure function of the triplet multiset-with-sequence, independent of how its producers interleaved their
//! pushes. SpMV accumulates each row in ascending-column order with fused `mul_add`.

use std::fmt;

pub mod direct;
pub mod precond;

/// Why a sparse-matrix call refused. Nothing is clamped, truncated or reordered to make the input fit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparseError {
    /// `nrows + 1` row pointers cannot be counted in `usize`.
    TooManyRows { nrows: usize },
    /// A COO entry outside the declared dimensions.
    OutOfRange {
        row: usize,
        col: usize,
        nrows: usize,
        ncols: usize,
    },
    /// Raw CSR parts that break the canonical invariant; `row` names the row when one row is at fault.
    NonCanonical {
        reason: &'static str,
        row: Option<usize>,
    },
    /// A vector whose length is not the one the matrix (or the other vector) needs.
    LengthMismatch {
        what: &'static str,
        expected: usize,
        got: usize,
    },
}

impl fmt::Display for SparseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SparseError::TooManyRows { nrows } => {
                write!(
                    f,
                    "{nrows} rows need more row pointers than usize can count"
                )
            }
            SparseError::OutOfRange {
                row,
                col,
                nrows,
                ncols,
            } => write!(
                f,
                "entry ({row},{col}) is outside the {nrows}x{ncols} matrix"
            ),
            SparseError::NonCanonical {
                reason,
                row: Some(r),
            } => write!(f, "non-canonical CSR at row {r}: {reason}"),
            SparseError::NonCanonical { reason, row: None } => {
                write!(f, "non-canonical CSR: {reason}")
            }
            SparseError::LengthMismatch {
                what,
                expected,
                got,
            } => write!(f, "{what}: length {got}, expected {expected}"),
        }
    }
}

impl std::error::Error for SparseError {}

// ---------------------------------------------------------------------------
// COO staging
// ---------------------------------------------------------------------------

/// Triplet staging buffer for assembly. Duplicate (row, col) entries are
/// allowed and ACCUMULATE (the FEM element-assembly contract).
#[derive(Debug, Clone, Default)]
pub struct Coo {
    nrows: usize,
    ncols: usize,
    rows: Vec<usize>,
    cols: Vec<usize>,
    vals: Vec<f64>,
}

impl Coo {
    /// Empty staging buffer with fixed dimensions. Refuses `nrows == usize::MAX`, whose `nrows + 1` row pointers
    /// cannot be counted.
    pub fn new(nrows: usize, ncols: usize) -> Result<Coo, SparseError> {
        if nrows.checked_add(1).is_none() {
            return Err(SparseError::TooManyRows { nrows });
        }
        Ok(Coo {
            nrows,
            ncols,
            rows: Vec::new(),
            cols: Vec::new(),
            vals: Vec::new(),
        })
    }

    /// Stage one contribution. An index outside the matrix refuses and stages nothing.
    pub fn push(&mut self, row: usize, col: usize, val: f64) -> Result<(), SparseError> {
        if row >= self.nrows || col >= self.ncols {
            return Err(SparseError::OutOfRange {
                row,
                col,
                nrows: self.nrows,
                ncols: self.ncols,
            });
        }
        self.rows.push(row);
        self.cols.push(col);
        self.vals.push(val);
        Ok(())
    }

    /// Number of staged triplets (before deduplication).
    #[must_use]
    pub fn staged(&self) -> usize {
        self.vals.len()
    }

    /// Canonical deterministic assembly into CSR.
    ///
    /// Contract: the result is a pure function of the SET of
    /// (row, col, insertion_seq, val) tuples — triplets may arrive in ANY
    /// stream order (tiles, threads, replays) and the matrix is bitwise
    /// identical, because duplicates are accumulated in insertion-sequence
    /// order after a stable sort by (row, col). Insertion sequence is LOGICAL
    /// identity (e.g. element id), never thread arrival.
    #[must_use]
    pub fn assemble(&self) -> Csr {
        let nnz_staged = self.vals.len();
        // Sort triplet INDICES by (row, col); the sort is stable, so equal
        // keys keep insertion order — the canonical accumulation order.
        let mut order: Vec<usize> = (0..nnz_staged).collect();
        order.sort_by_key(|&i| (self.rows[i], self.cols[i]));

        let mut row_ptr = vec![0usize; self.nrows + 1];
        let mut col_idx = Vec::new();
        let mut vals = Vec::new();
        let mut i = 0;
        while i < nnz_staged {
            let (r, c) = (self.rows[order[i]], self.cols[order[i]]);
            let mut acc = self.vals[order[i]];
            i += 1;
            while i < nnz_staged && self.rows[order[i]] == r && self.cols[order[i]] == c {
                acc += self.vals[order[i]];
                i += 1;
            }
            row_ptr[r + 1] += 1;
            col_idx.push(c);
            vals.push(acc);
        }
        for r in 0..self.nrows {
            row_ptr[r + 1] += row_ptr[r];
        }
        Csr {
            nrows: self.nrows,
            ncols: self.ncols,
            row_ptr,
            col_idx,
            vals,
        }
    }
}

// ---------------------------------------------------------------------------
// CSR — the canonical format
// ---------------------------------------------------------------------------

/// Compressed sparse row. CANONICAL INVARIANT: within each row, column
/// indices are strictly ascending (no duplicates). Every constructor
/// establishes this; every algorithm may rely on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Csr {
    nrows: usize,
    ncols: usize,
    row_ptr: Vec<usize>,
    col_idx: Vec<usize>,
    vals: Vec<f64>,
}

impl Csr {
    /// Build from raw parts, VALIDATING the canonical invariant — silently
    /// accepting a non-canonical matrix would void every determinism claim
    /// downstream. Refuses with [`SparseError::NonCanonical`].
    pub fn from_parts(
        nrows: usize,
        ncols: usize,
        row_ptr: Vec<usize>,
        col_idx: Vec<usize>,
        vals: Vec<f64>,
    ) -> Result<Csr, SparseError> {
        let refuse = |reason: &'static str, row: Option<usize>| {
            Err(SparseError::NonCanonical { reason, row })
        };
        if nrows.checked_add(1) != Some(row_ptr.len()) {
            return refuse("row_ptr must have nrows+1 entries", None);
        }
        if row_ptr.first() != Some(&0) {
            return refuse("row_ptr must start at 0", None);
        }
        if row_ptr.last() != Some(&col_idx.len()) {
            return refuse("row_ptr end must equal nnz", None);
        }
        if col_idx.len() != vals.len() {
            return refuse("col_idx/vals length mismatch", None);
        }
        for r in 0..nrows {
            let (start, end) = (row_ptr[r], row_ptr[r + 1]);
            if start > end {
                return refuse("row_ptr must be monotone", Some(r));
            }
            if end > col_idx.len() {
                return refuse("row_ptr must not pass nnz", Some(r));
            }
            let cols = &col_idx[start..end];
            if cols.windows(2).any(|w| w[0] >= w[1]) {
                return refuse("columns must be strictly ascending", Some(r));
            }
            if let Some(&last) = cols.last()
                && last >= ncols
            {
                return refuse("column out of range", Some(r));
            }
        }
        Ok(Csr {
            nrows,
            ncols,
            row_ptr,
            col_idx,
            vals,
        })
    }

    /// Rows.
    #[must_use]
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// Columns.
    #[must_use]
    pub fn ncols(&self) -> usize {
        self.ncols
    }

    /// Stored nonzeros.
    #[must_use]
    pub fn nnz(&self) -> usize {
        self.vals.len()
    }

    /// One row's (columns, values) — also the graph neighbor view (CSR IS
    /// the adjacency structure). `None` past the last row.
    #[must_use]
    pub fn row(&self, r: usize) -> Option<(&[usize], &[f64])> {
        let start = *self.row_ptr.get(r)?;
        let end = *self.row_ptr.get(r.checked_add(1)?)?;
        Some((self.col_idx.get(start..end)?, self.vals.get(start..end)?))
    }

    /// Row `r < nrows`, for the loops of this module tree that run over `0..nrows`.
    pub(crate) fn row_at(&self, r: usize) -> (&[usize], &[f64]) {
        let span = self.row_ptr[r]..self.row_ptr[r + 1];
        (&self.col_idx[span.clone()], &self.vals[span])
    }

    /// Value access (O(log nnz_row)): `Some(0.0)` if not stored, `None` outside the matrix.
    #[must_use]
    pub fn get(&self, r: usize, c: usize) -> Option<f64> {
        if c >= self.ncols {
            return None;
        }
        let (cols, vals) = self.row(r)?;
        Some(match cols.binary_search(&c) {
            Ok(k) => vals[k],
            Err(_) => 0.0,
        })
    }

    /// y = A·x. Deterministic by construction: each y[r] accumulates in
    /// ascending-column order with fused mul_add. Refuses wrong lengths with
    /// [`SparseError::LengthMismatch`] and writes nothing.
    pub fn spmv(&self, x: &[f64], y: &mut [f64]) -> Result<(), SparseError> {
        if x.len() != self.ncols {
            return Err(SparseError::LengthMismatch {
                what: "spmv x (ncols)",
                expected: self.ncols,
                got: x.len(),
            });
        }
        if y.len() != self.nrows {
            return Err(SparseError::LengthMismatch {
                what: "spmv y (nrows)",
                expected: self.nrows,
                got: y.len(),
            });
        }
        for (r, out) in y.iter_mut().enumerate() {
            let (cols, vals) = self.row_at(r);
            let mut acc = 0.0f64;
            for (&c, &v) in cols.iter().zip(vals) {
                acc = v.mul_add(x[c], acc);
            }
            *out = acc;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn lcg(seed: &mut u64) -> f64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*seed >> 11) as f64) / (1u64 << 53) as f64 - 0.5
    }

    /// A matrix from (row, col, value) triplets, through `Coo`.
    pub(crate) fn csr(nrows: usize, ncols: usize, entries: &[(usize, usize, f64)]) -> Csr {
        let mut coo = Coo::new(nrows, ncols).expect("dimensions");
        for &(r, c, v) in entries {
            coo.push(r, c, v).expect("entry inside the matrix");
        }
        coo.assemble()
    }

    /// Row-major dense expansion (the oracle side of the SpMV tests).
    pub(crate) fn dense(a: &Csr) -> Vec<f64> {
        let mut d = vec![0.0; a.nrows() * a.ncols()];
        for r in 0..a.nrows() {
            let (cols, vals) = a.row_at(r);
            for (&c, &v) in cols.iter().zip(vals) {
                d[r * a.ncols() + c] = v;
            }
        }
        d
    }

    /// Random sparse matrix via COO with duplicate-accumulation exercised.
    pub(crate) fn random_coo(nrows: usize, ncols: usize, per_row: usize, seed: u64) -> Coo {
        let mut s = seed;
        let mut coo = Coo::new(nrows, ncols).expect("dimensions");
        for r in 0..nrows {
            for _ in 0..per_row {
                let c = ((lcg(&mut s) + 0.5) * ncols as f64) as usize % ncols;
                coo.push(r, c, lcg(&mut s))
                    .expect("entry inside the matrix");
            }
        }
        coo
    }

    /// 5-point Laplacian on an n×n grid — the FEM-patterned fixture.
    pub(crate) fn laplacian_2d(n: usize) -> Csr {
        let dim = n * n;
        let mut e = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let u = i * n + j;
                e.push((u, u, 4.0));
                if i > 0 {
                    e.push((u, u - n, -1.0));
                }
                if i + 1 < n {
                    e.push((u, u + n, -1.0));
                }
                if j > 0 {
                    e.push((u, u - 1, -1.0));
                }
                if j + 1 < n {
                    e.push((u, u + 1, -1.0));
                }
            }
        }
        csr(dim, dim, &e)
    }

    #[test]
    fn test_assembly_accumulates_duplicates_and_sorts() {
        let a = csr(2, 3, &[(1, 2, 1.0), (0, 1, 2.0), (1, 2, 0.5), (0, 0, -1.0)]);
        assert_eq!(a.nnz(), 3);
        assert_eq!(a.get(0, 0).map(f64::to_bits), Some((-1.0f64).to_bits()));
        assert_eq!(a.get(0, 1).map(f64::to_bits), Some(2.0f64.to_bits()));
        assert_eq!(a.get(1, 2).map(f64::to_bits), Some(1.5f64.to_bits()));
        assert_eq!(
            a.get(1, 0),
            Some(0.0),
            "an unstored entry inside the matrix is zero"
        );
        let (cols, _) = a.row_at(0);
        assert_eq!(cols, &[0, 1], "columns must come out sorted");
    }

    #[test]
    fn test_assembly_is_stream_order_invariant_bitwise() {
        // Shuffle the triplet stream as tiles would, assemble, and demand BITWISE equality — possible because
        // accumulation order is (row, col, insertion_seq). Whole rows (tiles) move; each element keeps its own
        // sequence.
        let base = random_coo(40, 40, 12, 0x5EED);
        let a = base.assemble();
        let n = base.staged();
        for stride in [7usize, 13, 29] {
            assert!(
                !n.is_multiple_of(stride),
                "stride must not divide n for full cycle"
            );
            let mut shuffled = Coo::new(40, 40).expect("dimensions");
            let mut idx = 0;
            let mut row_order: Vec<usize> = (0..40).collect();
            for _ in 0..stride {
                idx = (idx + stride) % 40;
                row_order.swap(idx, (idx * stride + 3) % 40);
            }
            for &r in &row_order {
                for i in 0..n {
                    if base.rows[i] == r {
                        shuffled
                            .push(base.rows[i], base.cols[i], base.vals[i])
                            .expect("entry inside the matrix");
                    }
                }
            }
            let b = shuffled.assemble();
            assert_eq!(a.nnz(), b.nnz());
            assert!(
                a.vals
                    .iter()
                    .zip(&b.vals)
                    .all(|(x, y)| x.to_bits() == y.to_bits()),
                "assembly depended on tile interleaving (stride {stride})"
            );
        }
        println!(
            "{{\"suite\":\"fmine-numerics-sparse\",\"case\":\"assembly-order-invariance\",\"verdict\":\"pass\",\"detail\":\"3 tile interleavings bitwise identical\"}}"
        );
    }

    #[test]
    fn test_spmv_matches_dense_reference() {
        let mut seed = 3u64;
        for (nr, nc) in [(1usize, 1usize), (17, 9), (64, 64), (100, 40)] {
            let a = random_coo(nr, nc, 6, 0xF00 + nr as u64).assemble();
            let x: Vec<f64> = (0..nc).map(|_| lcg(&mut seed)).collect();
            let mut y = vec![0.0; nr];
            a.spmv(&x, &mut y).expect("lengths match");
            let d = dense(&a);
            for r in 0..nr {
                let want: f64 = (0..nc).map(|c| d[r * nc + c] * x[c]).sum();
                let scale = want.abs().max(1.0);
                assert!(
                    (y[r] - want).abs() / scale < 1e-13,
                    "spmv row {r} of {nr}x{nc}: {} vs dense {want}",
                    y[r]
                );
            }
        }
    }

    #[test]
    fn test_spmv_linearity_property() {
        let a = laplacian_2d(9);
        let n = a.nrows();
        let mut seed = 5u64;
        let x1: Vec<f64> = (0..n).map(|_| lcg(&mut seed)).collect();
        let x2: Vec<f64> = (0..n).map(|_| lcg(&mut seed)).collect();
        let alpha = 1.75; // power of two × small — keeps the check tight
        let combo: Vec<f64> = x1.iter().zip(&x2).map(|(a, b)| alpha * a + b).collect();
        let (mut y1, mut y2, mut yc) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        a.spmv(&x1, &mut y1).expect("lengths match");
        a.spmv(&x2, &mut y2).expect("lengths match");
        a.spmv(&combo, &mut yc).expect("lengths match");
        for i in 0..n {
            let want = alpha * y1[i] + y2[i];
            assert!((yc[i] - want).abs() < 1e-12, "linearity at {i}");
        }
    }

    #[test]
    fn test_adversarial_patterns() {
        // Empty rows, dense row, single column — the shapes that break naive pointer arithmetic.
        let a = csr(
            5,
            4,
            &[
                (2, 0, 1.0),
                (2, 1, 2.0),
                (2, 2, 3.0),
                (2, 3, 4.0),
                (4, 0, -3.0),
            ],
        );
        assert_eq!(a.nnz(), 5);
        let mut y = vec![0.0; 5];
        a.spmv(&[1.0, 1.0, 1.0, 1.0], &mut y)
            .expect("lengths match");
        assert_eq!(y, vec![0.0, 0.0, 10.0, 0.0, -3.0]);
        let b = csr(3, 1, &[(0, 0, 2.0), (2, 0, -1.0)]);
        let mut yb = vec![0.0; 3];
        b.spmv(&[4.0], &mut yb).expect("lengths match");
        assert_eq!(yb, vec![8.0, 0.0, -4.0]);
        let e = csr(3, 3, &[]);
        assert_eq!(e.nnz(), 0);
        let mut ye = vec![1.0; 3];
        e.spmv(&[1.0, 1.0, 1.0], &mut ye).expect("lengths match");
        assert_eq!(ye, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_from_parts_rejects_non_canonical() {
        for (rp, ci, reason, row) in [
            (
                vec![0usize, 2],
                vec![1usize, 1],
                "columns must be strictly ascending",
                Some(0),
            ),
            (
                vec![0, 2],
                vec![1, 0],
                "columns must be strictly ascending",
                Some(0),
            ),
            (vec![0, 1], vec![5], "column out of range", Some(0)),
            (
                vec![0, 1, 1],
                vec![0],
                "row_ptr must have nrows+1 entries",
                None,
            ),
            (vec![1, 1], vec![0], "row_ptr must start at 0", None),
            (vec![0, 2], vec![0], "row_ptr end must equal nnz", None),
        ] {
            let vals = vec![1.0; ci.len()];
            assert_eq!(
                Csr::from_parts(1, 2, rp, ci, vals),
                Err(SparseError::NonCanonical { reason, row })
            );
        }
        // Two rows: a pointer past nnz, then a decrease (upstream sliced past the end and panicked here).
        assert_eq!(
            Csr::from_parts(2, 2, vec![0, 5, 2], vec![0, 1], vec![1.0, 2.0]),
            Err(SparseError::NonCanonical {
                reason: "row_ptr must not pass nnz",
                row: Some(0)
            })
        );
        assert_eq!(
            Csr::from_parts(3, 2, vec![0, 2, 1, 2], vec![0, 1], vec![1.0, 2.0]),
            Err(SparseError::NonCanonical {
                reason: "row_ptr must be monotone",
                row: Some(1)
            })
        );
        assert_eq!(
            Csr::from_parts(1, 2, vec![0, 1], vec![0], vec![]),
            Err(SparseError::NonCanonical {
                reason: "col_idx/vals length mismatch",
                row: None
            })
        );
        let ok = Csr::from_parts(2, 3, vec![0, 2, 3], vec![0, 2, 1], vec![1.0, 2.0, 3.0]);
        assert_eq!(ok, Ok(csr(2, 3, &[(0, 0, 1.0), (0, 2, 2.0), (1, 1, 3.0)])));
    }

    #[test]
    fn test_push_outside_the_matrix_refuses_and_stages_nothing() {
        let mut coo = Coo::new(2, 3).expect("dimensions");
        assert_eq!(
            coo.push(2, 0, 1.0),
            Err(SparseError::OutOfRange {
                row: 2,
                col: 0,
                nrows: 2,
                ncols: 3
            })
        );
        assert!(coo.push(0, 3, 1.0).is_err());
        assert_eq!(coo.staged(), 0);
        assert_eq!(
            Coo::new(usize::MAX, 1).map(|c| c.staged()),
            Err(SparseError::TooManyRows { nrows: usize::MAX })
        );
    }

    #[test]
    fn test_spmv_refuses_wrong_lengths_and_writes_nothing() {
        let a = csr(2, 3, &[(0, 0, 1.0), (1, 2, 2.0)]);
        let mut y = vec![7.0; 2];
        assert_eq!(
            a.spmv(&[1.0, 1.0], &mut y),
            Err(SparseError::LengthMismatch {
                what: "spmv x (ncols)",
                expected: 3,
                got: 2
            })
        );
        let mut y3 = vec![7.0; 3];
        assert!(a.spmv(&[1.0, 1.0, 1.0], &mut y3).is_err());
        assert_eq!(y, vec![7.0, 7.0]);
        assert_eq!(y3, vec![7.0, 7.0, 7.0]);
    }

    #[test]
    fn test_row_and_get_outside_the_matrix_are_none() {
        let a = csr(2, 3, &[(0, 0, 1.0), (1, 2, 2.0)]);
        assert_eq!(a.row(1), Some((&[2usize][..], &[2.0][..])));
        assert_eq!(a.row(2), None);
        assert_eq!(a.row(usize::MAX), None);
        assert_eq!(a.get(1, 2), Some(2.0));
        assert_eq!(a.get(2, 0), None);
        assert_eq!(a.get(0, 3), None);
    }
}
