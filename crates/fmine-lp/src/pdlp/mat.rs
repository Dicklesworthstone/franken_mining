//! The solver's own CSR copy (`u32` columns) and the fixed-chunk parallel loops it runs on.
//! Determinism: DeterministicPerIsa (each row sums in ascending-column order; reductions sum fixed `CHUNK`-sized
//! pieces in index order, so the thread count never changes a bit).

use std::ops::Range;

use fmine_evidence::Refusal;
use fmine_numerics::sparse::Csr;

use super::refuse;

/// Reductions add per-chunk partial sums in chunk order. Chunk boundaries are absolute indices, so the result is
/// the same at any thread count.
pub(crate) const CHUNK: usize = 1 << 14;

/// Below this many stored entries every loop runs on the calling thread.
const PARALLEL_MIN_NNZ: usize = 1 << 17;

/// Compressed sparse rows with `u32` columns: half the index traffic of the `usize` CSR it is copied from.
#[derive(Debug, Clone)]
pub(crate) struct Mat {
    pub(crate) ncols: usize,
    pub(crate) ptr: Vec<usize>,
    pub(crate) idx: Vec<u32>,
    pub(crate) val: Vec<f64>,
}

impl Mat {
    /// Copies `a`, refusing a non-finite entry or more than `u32::MAX` rows or columns.
    pub(crate) fn from_csr(a: &Csr) -> Result<Mat, Refusal> {
        let (nrows, ncols) = (a.nrows(), a.ncols());
        if nrows >= u32::MAX as usize || ncols >= u32::MAX as usize {
            return Err(refuse(format!(
                "a {nrows} x {ncols} matrix: rows and columns must each be fewer than u32::MAX"
            )));
        }
        let mut ptr = Vec::with_capacity(nrows + 1);
        let mut idx = Vec::with_capacity(a.nnz());
        let mut val = Vec::with_capacity(a.nnz());
        ptr.push(0);
        for r in 0..nrows {
            let (cols, vals) = a
                .row(r)
                .ok_or_else(|| refuse(format!("row {r} of the matrix is unreadable")))?;
            for (&c, &v) in cols.iter().zip(vals) {
                if !v.is_finite() {
                    return Err(refuse(format!("matrix entry ({r}, {c}) is {v}")));
                }
                idx.push(c as u32);
                val.push(v);
            }
            ptr.push(idx.len());
        }
        Ok(Mat {
            ncols,
            ptr,
            idx,
            val,
        })
    }

    pub(crate) fn nrows(&self) -> usize {
        self.ptr.len().saturating_sub(1)
    }

    pub(crate) fn nnz(&self) -> usize {
        self.val.len()
    }

    /// Row `r < nrows`.
    pub(crate) fn row(&self, r: usize) -> (&[u32], &[f64]) {
        let span = self.ptr[r]..self.ptr[r + 1];
        (&self.idx[span.clone()], &self.val[span])
    }

    /// The transpose, by a counting sort: each of its rows lists the original rows in ascending order.
    pub(crate) fn transpose(&self) -> Mat {
        let nrows = self.nrows();
        let mut ptr = vec![0usize; self.ncols + 1];
        for &j in &self.idx {
            ptr[j as usize + 1] += 1;
        }
        for j in 0..self.ncols {
            ptr[j + 1] += ptr[j];
        }
        let mut next = ptr.clone();
        let mut idx = vec![0u32; self.nnz()];
        let mut val = vec![0.0f64; self.nnz()];
        for r in 0..nrows {
            let (cols, vals) = self.row(r);
            for (&j, &v) in cols.iter().zip(vals) {
                let p = next[j as usize];
                idx[p] = r as u32;
                val[p] = v;
                next[j as usize] += 1;
            }
        }
        Mat {
            ncols: nrows,
            ptr,
            idx,
            val,
        }
    }
}

/// How many workers to use for a matrix with `nnz` entries: one below [`PARALLEL_MIN_NNZ`], otherwise `threads`
/// (0 meaning the machine's parallelism, capped at 16).
pub(crate) fn workers(threads: usize, nnz: usize) -> usize {
    if nnz < PARALLEL_MIN_NNZ {
        return 1;
    }
    let want = if threads == 0 {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(16))
    } else {
        threads
    };
    want.max(1)
}

/// `0..len` cut into at most `workers` contiguous pieces whose interior boundaries are multiples of [`CHUNK`].
pub(crate) fn even_pieces(len: usize, workers: usize) -> Vec<Range<usize>> {
    let chunks = len.div_ceil(CHUNK).max(1);
    let per = chunks.div_ceil(workers.max(1));
    let mut out = Vec::new();
    let mut start = 0;
    while start < len {
        let end = (start + per * CHUNK).min(len);
        out.push(start..end);
        start = end;
    }
    if out.is_empty() {
        out.push(0..0);
    }
    out
}

/// The rows of `m` cut into at most `workers` contiguous pieces of about equal stored entries.
pub(crate) fn nnz_pieces(m: &Mat, workers: usize) -> Vec<Range<usize>> {
    let nrows = m.nrows();
    let workers = workers.max(1);
    let total = m.nnz().max(1);
    let mut out = Vec::new();
    let mut start = 0;
    for k in 1..=workers {
        let target = total * k / workers;
        let mut end = start;
        while end < nrows && (m.ptr[end + 1] <= target || k == workers) {
            end += 1;
        }
        if end > start {
            out.push(start..end);
        }
        start = end;
    }
    if out.is_empty() {
        out.push(0..nrows);
    }
    out
}

/// Mutable outputs a parallel loop splits along with its index range.
pub(crate) trait Parts: Send + Sized {
    fn split_front(self, n: usize) -> (Self, Self);
}

impl Parts for &mut [f64] {
    fn split_front(self, n: usize) -> (Self, Self) {
        self.split_at_mut(n)
    }
}

impl Parts for () {
    fn split_front(self, _n: usize) -> (Self, Self) {
        ((), ())
    }
}

impl<A: Parts, B: Parts> Parts for (A, B) {
    fn split_front(self, n: usize) -> (Self, Self) {
        let (a0, a1) = self.0.split_front(n);
        let (b0, b1) = self.1.split_front(n);
        ((a0, b0), (a1, b1))
    }
}

impl<A: Parts, B: Parts, C: Parts> Parts for (A, B, C) {
    fn split_front(self, n: usize) -> (Self, Self) {
        let (a0, a1) = self.0.split_front(n);
        let (b0, b1) = self.1.split_front(n);
        let (c0, c1) = self.2.split_front(n);
        ((a0, b0, c0), (a1, b1, c1))
    }
}

/// Runs `f(range, outputs)` once per piece, the outputs cut to match, each piece on its own thread when there is
/// more than one. Results come back in piece order. Every output must be as long as the pieces cover.
pub(crate) fn run<P, R, F>(pieces: &[Range<usize>], parts: P, f: F) -> Result<Vec<R>, Refusal>
where
    P: Parts,
    R: Send,
    F: Fn(Range<usize>, P) -> R + Sync,
{
    if let [only] = pieces {
        return Ok(vec![f(only.clone(), parts)]);
    }
    std::thread::scope(|s| {
        let mut rest = parts;
        let mut handles = Vec::with_capacity(pieces.len());
        for piece in pieces {
            let (head, tail) = rest.split_front(piece.len());
            rest = tail;
            let (f, piece) = (&f, piece.clone());
            handles.push(s.spawn(move || f(piece, head)));
        }
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .map_err(|_| refuse("a PDLP worker thread panicked"))
            })
            .collect()
    })
}

/// `y = M x` over the given row pieces of `m`.
pub(crate) fn spmv(
    m: &Mat,
    pieces: &[Range<usize>],
    x: &[f64],
    y: &mut [f64],
) -> Result<(), Refusal> {
    run(pieces, y, |rows, out: &mut [f64]| {
        for (r, o) in rows.zip(out.iter_mut()) {
            let (cols, vals) = m.row(r);
            let mut acc = 0.0f64;
            for (&c, &v) in cols.iter().zip(vals) {
                acc = v.mul_add(x[c as usize], acc);
            }
            *o = acc;
        }
    })?;
    Ok(())
}

/// Sums per-chunk partials in chunk order.
pub(crate) fn fold<const K: usize>(parts: Vec<Vec<[f64; K]>>) -> [f64; K] {
    let mut total = [0.0f64; K];
    for chunk in parts.into_iter().flatten() {
        for (t, v) in total.iter_mut().zip(chunk) {
            *t += v;
        }
    }
    total
}

/// The [`CHUNK`]-aligned sub-ranges of `range`, which starts on a chunk boundary.
pub(crate) fn chunks(range: Range<usize>) -> impl Iterator<Item = Range<usize>> {
    let end = range.end;
    range.step_by(CHUNK).map(move |s| s..(s + CHUNK).min(end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_numerics::sparse::Coo;

    fn sample(nrows: usize, ncols: usize, seed: u64) -> Csr {
        let mut coo = Coo::new(nrows, ncols).unwrap();
        let mut s = seed;
        for r in 0..nrows {
            for _ in 0..3 {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let c = (s >> 33) as usize % ncols;
                coo.push(r, c, ((s >> 11) % 1000) as f64 / 100.0 - 5.0)
                    .unwrap();
            }
        }
        coo.assemble()
    }

    #[test]
    fn spmv_matches_the_numerics_csr_bit_for_bit_at_any_piece_count() {
        let a = sample(5_000, 3_000, 7);
        let m = Mat::from_csr(&a).unwrap();
        let x: Vec<f64> = (0..3_000).map(|j| (j as f64).sin()).collect();
        let mut want = vec![0.0; 5_000];
        a.spmv(&x, &mut want).unwrap();
        for w in [1, 2, 3, 7] {
            let mut got = vec![0.0; 5_000];
            spmv(&m, &nnz_pieces(&m, w), &x, &mut got).unwrap();
            assert_eq!(
                got.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                want.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn transpose_lists_every_entry_once_in_row_order() {
        let a = sample(400, 250, 11);
        let m = Mat::from_csr(&a).unwrap();
        let t = m.transpose();
        assert_eq!(t.nrows(), 250);
        assert_eq!(t.nnz(), m.nnz());
        for j in 0..250 {
            let (rows, vals) = t.row(j);
            assert!(rows.windows(2).all(|w| w[0] < w[1]));
            for (&r, &v) in rows.iter().zip(vals) {
                assert_eq!(a.get(r as usize, j), Some(v));
            }
        }
        let back = t.transpose();
        assert_eq!((back.ptr, back.idx), (m.ptr, m.idx));
    }

    #[test]
    fn pieces_cover_the_range_on_chunk_boundaries() {
        for (len, w) in [(0, 4), (1, 4), (CHUNK, 3), (5 * CHUNK + 7, 3), (100, 1)] {
            let p = even_pieces(len, w);
            assert_eq!(p.first().map(|r| r.start), Some(0));
            assert_eq!(p.last().map(|r| r.end), Some(len));
            assert!(
                p.windows(2)
                    .all(|q| q[0].end == q[1].start && q[0].end % CHUNK == 0)
            );
            assert!(p.len() <= w.max(1));
        }
    }
}
