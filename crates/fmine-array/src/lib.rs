//! Dense 2D/3D grids (`BD-036`), 64-byte SIMD-aligned columnar interval buffers (`FMT-COL-1`),
//! and zero-allocation sweep-line interval compositing (`BD-310`).

use std::fmt;

/// Errors raised by `Grid2D`, `Grid3D`, or `ColumnarIntervalTable`.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayError {
    /// Grid index `(i, j, k)` out of bounds `(nx, ny, nz)`.
    OutOfBounds {
        index: (usize, usize, usize),
        shape: (usize, usize, usize),
    },
    /// Invalid downhole interval (`to_m <= from_m` or `density <= 0`).
    InvalidInterval { row: usize, from_m: f64, to_m: f64 },
    /// A row starting above the previous row's end: out of depth order or overlapping it (`bd-b6r7`).
    RowOutOfOrder {
        row: usize,
        from_m: f64,
        prev_to_m: f64,
    },
    /// A composite length that is not finite or not positive.
    InvalidTargetLength { target_length_m: f64 },
    /// Caller-provided output slice is too small for the generated composites.
    OutputBufferTooSmall { required: usize, capacity: usize },
}

impl fmt::Display for ArrayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { index, shape } => {
                write!(f, "grid index {index:?} out of bounds for shape {shape:?}")
            }
            Self::InvalidInterval { row, from_m, to_m } => {
                write!(f, "invalid interval at row {row}: [{from_m}, {to_m}]")
            }
            Self::RowOutOfOrder {
                row,
                from_m,
                prev_to_m,
            } => write!(
                f,
                "row {row} starts at {from_m} m, above the previous row's end at {prev_to_m} m"
            ),
            Self::InvalidTargetLength { target_length_m } => write!(
                f,
                "composite length {target_length_m} m is not finite and positive"
            ),
            Self::OutputBufferTooSmall { required, capacity } => {
                write!(
                    f,
                    "output composite buffer too small (required {required}, capacity {capacity})"
                )
            }
        }
    }
}

impl std::error::Error for ArrayError {}

/// Dense 2D row-major array (`BD-036`).
#[derive(Debug, Clone, PartialEq)]
pub struct Grid2D<T> {
    nx: usize,
    ny: usize,
    data: Vec<T>,
}

impl<T: Clone> Grid2D<T> {
    /// Creates a `Grid2D` of shape `(nx, ny)` filled with `fill`.
    #[must_use]
    pub fn filled(nx: usize, ny: usize, fill: T) -> Self {
        Self {
            nx,
            ny,
            data: vec![fill; nx.saturating_mul(ny)],
        }
    }

    /// Shape `(nx, ny)`.
    #[must_use]
    pub const fn shape(&self) -> (usize, usize) {
        (self.nx, self.ny)
    }

    /// Bijective linear index `j * nx + i`.
    pub fn linear_index(&self, i: usize, j: usize) -> Result<usize, ArrayError> {
        if i >= self.nx || j >= self.ny {
            return Err(ArrayError::OutOfBounds {
                index: (i, j, 0),
                shape: (self.nx, self.ny, 1),
            });
        }
        Ok(j * self.nx + i)
    }

    /// Inverse mapping from `linear_idx` to `(i, j)`.
    pub fn coords_of(&self, linear_idx: usize) -> Result<(usize, usize), ArrayError> {
        if linear_idx >= self.data.len() || self.nx == 0 {
            return Err(ArrayError::OutOfBounds {
                index: (linear_idx, 0, 0),
                shape: (self.nx, self.ny, 1),
            });
        }
        Ok((linear_idx % self.nx, linear_idx / self.nx))
    }

    /// Returns a reference to cell `(i, j)`.
    pub fn get(&self, i: usize, j: usize) -> Result<&T, ArrayError> {
        let idx = self.linear_index(i, j)?;
        Ok(&self.data[idx])
    }

    /// Sets cell `(i, j)`.
    pub fn set(&mut self, i: usize, j: usize, value: T) -> Result<(), ArrayError> {
        let idx = self.linear_index(i, j)?;
        self.data[idx] = value;
        Ok(())
    }
}

/// Dense 3D row-major array (`BD-036`).
#[derive(Debug, Clone, PartialEq)]
pub struct Grid3D<T> {
    nx: usize,
    ny: usize,
    nz: usize,
    data: Vec<T>,
}

impl<T: Clone> Grid3D<T> {
    /// Creates a `Grid3D` of shape `(nx, ny, nz)` filled with `fill`.
    #[must_use]
    pub fn filled(nx: usize, ny: usize, nz: usize, fill: T) -> Self {
        Self {
            nx,
            ny,
            nz,
            data: vec![fill; nx.saturating_mul(ny).saturating_mul(nz)],
        }
    }

    /// Shape `(nx, ny, nz)`.
    #[must_use]
    pub const fn shape(&self) -> (usize, usize, usize) {
        (self.nx, self.ny, self.nz)
    }

    /// Bijective linear index `(k * ny + j) * nx + i`.
    pub fn linear_index(&self, i: usize, j: usize, k: usize) -> Result<usize, ArrayError> {
        if i >= self.nx || j >= self.ny || k >= self.nz {
            return Err(ArrayError::OutOfBounds {
                index: (i, j, k),
                shape: (self.nx, self.ny, self.nz),
            });
        }
        Ok((k * self.ny + j) * self.nx + i)
    }

    /// Inverse mapping from `linear_idx` to `(i, j, k)`.
    pub fn coords_of(&self, linear_idx: usize) -> Result<(usize, usize, usize), ArrayError> {
        if linear_idx >= self.data.len() || self.nx == 0 || self.ny == 0 {
            return Err(ArrayError::OutOfBounds {
                index: (linear_idx, 0, 0),
                shape: (self.nx, self.ny, self.nz),
            });
        }
        let i = linear_idx % self.nx;
        let rem = linear_idx / self.nx;
        let j = rem % self.ny;
        let k = rem / self.ny;
        Ok((i, j, k))
    }

    /// Returns a reference to cell `(i, j, k)`.
    pub fn get(&self, i: usize, j: usize, k: usize) -> Result<&T, ArrayError> {
        let idx = self.linear_index(i, j, k)?;
        Ok(&self.data[idx])
    }

    /// Sets cell `(i, j, k)`.
    pub fn set(&mut self, i: usize, j: usize, k: usize, value: T) -> Result<(), ArrayError> {
        let idx = self.linear_index(i, j, k)?;
        self.data[idx] = value;
        Ok(())
    }

    /// Underlying contiguous slice.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

// --- FMT-COL-1 64-Byte Aligned Columnar Interval Buffer & Zero-Alloc Compositor (BD-310) ---

/// 64-byte cache-line / AVX-512 aligned block of 8 `f64` values (`FMT-COL-1`).
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlignedF64Block(pub [f64; 8]);

/// Columnar downhole assay interval table backed by `AlignedF64Block` chunks (`BD-310`).
#[derive(Debug, Clone, Default)]
pub struct ColumnarIntervalTable {
    len: usize,
    from_blocks: Vec<AlignedF64Block>,
    to_blocks: Vec<AlignedF64Block>,
    density_blocks: Vec<AlignedF64Block>,
    grade_blocks: Vec<AlignedF64Block>,
}

/// Output downhole composite interval conserving length $\times$ density $\times$ grade.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CompositeInterval {
    pub from_m: f64,
    pub to_m: f64,
    pub length_m: f64,
    pub mean_density_t_m3: f64,
    pub mass_weighted_grade_g_t: f64,
}

impl ColumnarIntervalTable {
    /// Creates an empty `ColumnarIntervalTable`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a validated assay interval `(from_m, to_m, density_t_m3, grade_g_t)`. Rows go in depth order: a row
    /// starting above the previous row's end is refused ([`ArrayError::RowOutOfOrder`]), and a gap is allowed.
    /// A refused row leaves the table unchanged.
    pub fn push(
        &mut self,
        from_m: f64,
        to_m: f64,
        density_t_m3: f64,
        grade_g_t: f64,
    ) -> Result<(), ArrayError> {
        if !from_m.is_finite()
            || !to_m.is_finite()
            || to_m <= from_m
            || !density_t_m3.is_finite()
            || density_t_m3 <= 0.0
            || !grade_g_t.is_finite()
        {
            return Err(ArrayError::InvalidInterval {
                row: self.len,
                from_m,
                to_m,
            });
        }
        if let Some(prev) = self.len.checked_sub(1) {
            let prev_to_m = self.to_blocks[prev / 8].0[prev % 8];
            if from_m < prev_to_m {
                return Err(ArrayError::RowOutOfOrder {
                    row: self.len,
                    from_m,
                    prev_to_m,
                });
            }
        }
        let block_idx = self.len / 8;
        let lane = self.len % 8;
        if lane == 0 {
            self.from_blocks.push(AlignedF64Block([0.0; 8]));
            self.to_blocks.push(AlignedF64Block([0.0; 8]));
            self.density_blocks.push(AlignedF64Block([0.0; 8]));
            self.grade_blocks.push(AlignedF64Block([0.0; 8]));
        }
        self.from_blocks[block_idx].0[lane] = from_m;
        self.to_blocks[block_idx].0[lane] = to_m;
        self.density_blocks[block_idx].0[lane] = density_t_m3;
        self.grade_blocks[block_idx].0[lane] = grade_g_t;
        self.len += 1;
        Ok(())
    }

    /// Number of stored intervals.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if no intervals are stored.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Zero-allocation sweep-line interval compositing into caller-supplied `out` slice (`BD-310`).
    ///
    /// Splits raw intervals at regular `target_length_m` boundaries and computes density-weighted
    /// composite grades, returning the number of written `CompositeInterval`s. A `target_length_m` that is not
    /// finite and positive is refused ([`ArrayError::InvalidTargetLength`]); an empty table writes nothing.
    pub fn composite_into(
        &self,
        target_length_m: f64,
        out: &mut [CompositeInterval],
    ) -> Result<usize, ArrayError> {
        if !target_length_m.is_finite() || target_length_m <= 0.0 {
            return Err(ArrayError::InvalidTargetLength { target_length_m });
        }
        if self.len == 0 {
            return Ok(0);
        }
        let collar_start = self.from_blocks[0].0[0];
        let mut max_to = collar_start;
        for row in 0..self.len {
            let t = self.to_blocks[row / 8].0[row % 8];
            if t > max_to {
                max_to = t;
            }
        }
        let total_span = max_to - collar_start;
        let num_bins = ((total_span / target_length_m).ceil() as usize).max(1);
        if out.len() < num_bins {
            return Err(ArrayError::OutputBufferTooSmall {
                required: num_bins,
                capacity: out.len(),
            });
        }

        for (b, slot) in out.iter_mut().take(num_bins).enumerate() {
            let b_from = collar_start + (b as f64) * target_length_m;
            let b_to = (b_from + target_length_m).min(max_to);
            *slot = CompositeInterval {
                from_m: b_from,
                to_m: b_to,
                length_m: 0.0,
                mean_density_t_m3: 0.0,
                mass_weighted_grade_g_t: 0.0,
            };
        }

        // Sweep-line accumulation into bins without heap allocation.
        for row in 0..self.len {
            let blk = row / 8;
            let lane = row % 8;
            let r_from = self.from_blocks[blk].0[lane];
            let r_to = self.to_blocks[blk].0[lane];
            let rho = self.density_blocks[blk].0[lane];
            let grade = self.grade_blocks[blk].0[lane];

            let first_bin = (((r_from - collar_start) / target_length_m).floor() as isize)
                .clamp(0, (num_bins - 1) as isize) as usize;
            let last_bin = (((r_to - collar_start) / target_length_m).floor() as isize)
                .clamp(0, (num_bins - 1) as isize) as usize;

            for slot in out.iter_mut().take(last_bin + 1).skip(first_bin) {
                let overlap_start = r_from.max(slot.from_m);
                let overlap_end = r_to.min(slot.to_m);
                let dl = overlap_end - overlap_start;
                if dl > 0.0 {
                    let dm = dl * rho;
                    slot.length_m += dl;
                    // Temporarily accumulate mass in `mean_density_t_m3` and metal in `mass_weighted_grade_g_t`.
                    slot.mean_density_t_m3 += dm;
                    slot.mass_weighted_grade_g_t += dm * grade;
                }
            }
        }

        for slot in out.iter_mut().take(num_bins) {
            let total_mass = slot.mean_density_t_m3;
            let total_metal = slot.mass_weighted_grade_g_t;
            if slot.length_m > 0.0 && total_mass > 0.0 {
                slot.mean_density_t_m3 = total_mass / slot.length_m;
                slot.mass_weighted_grade_g_t = total_metal / total_mass;
            }
        }

        Ok(num_bins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid2d_and_grid3d_bijective_indexing() {
        let g2 = Grid2D::filled(7, 5, 0_u32);
        for j in 0..5 {
            for i in 0..7 {
                let idx = g2.linear_index(i, j).unwrap();
                assert_eq!(g2.coords_of(idx).unwrap(), (i, j));
            }
        }

        let g3 = Grid3D::filled(4, 5, 6, 1.0_f64);
        for k in 0..6 {
            for j in 0..5 {
                for i in 0..4 {
                    let idx = g3.linear_index(i, j, k).unwrap();
                    assert_eq!(g3.coords_of(idx).unwrap(), (i, j, k));
                }
            }
        }
    }

    #[test]
    fn test_fmt_col_1_alignment_and_zero_alloc_compositing() {
        assert_eq!(std::mem::align_of::<AlignedF64Block>(), 64);

        let mut table = ColumnarIntervalTable::new();
        // Raw intervals with varying lengths and densities across 0..6m.
        table.push(0.0, 1.5, 2.60, 1.20).unwrap();
        table.push(1.5, 3.2, 2.80, 3.50).unwrap();
        table.push(3.2, 6.0, 2.70, 0.80).unwrap();

        let raw_metal = 1.5 * 2.60 * 1.20 + 1.7 * 2.80 * 3.50 + 2.8 * 2.70 * 0.80;
        let raw_mass = 1.5 * 2.60 + 1.7 * 2.80 + 2.8 * 2.70;

        let mut composites = [CompositeInterval::default(); 4];
        let n = table.composite_into(2.0, &mut composites).unwrap();
        assert_eq!(n, 3);

        let comp_mass: f64 = composites[..n]
            .iter()
            .map(|c| c.length_m * c.mean_density_t_m3)
            .sum();
        let comp_metal: f64 = composites[..n]
            .iter()
            .map(|c| c.length_m * c.mean_density_t_m3 * c.mass_weighted_grade_g_t)
            .sum();

        assert!((comp_mass - raw_mass).abs() < 1e-12);
        assert!((comp_metal - raw_metal).abs() < 1e-12);
    }

    fn composites(table: &ColumnarIntervalTable) -> Vec<CompositeInterval> {
        let mut out = [CompositeInterval::default(); 16];
        let n = table.composite_into(1.0, &mut out).unwrap();
        out[..n].to_vec()
    }

    /// bd-b6r7: a row starting above the previous row's end (out of depth order, or overlapping it) is refused and
    /// leaves the table as it was. A row starting at the previous end, or after a gap, is kept.
    #[test]
    fn test_push_refuses_out_of_order_and_overlapping_rows() {
        let mut table = ColumnarIntervalTable::new();
        table.push(3.0, 6.0, 2.7, 1.0).unwrap();
        let before = composites(&table);
        assert_eq!(
            table.push(0.0, 3.0, 2.7, 1.0),
            Err(ArrayError::RowOutOfOrder {
                row: 1,
                from_m: 0.0,
                prev_to_m: 6.0
            })
        );
        assert_eq!(
            table.push(5.0, 7.0, 2.7, 1.0),
            Err(ArrayError::RowOutOfOrder {
                row: 1,
                from_m: 5.0,
                prev_to_m: 6.0
            })
        );
        assert_eq!(table.len(), 1);
        assert_eq!(composites(&table), before);
        table.push(6.0, 8.0, 2.7, 1.0).unwrap();
        table.push(9.0, 10.0, 2.7, 1.0).unwrap();
        assert_eq!(table.len(), 3);

        // Across a block boundary: the ninth row would open a new block, and its refusal opens none.
        let mut full = ColumnarIntervalTable::new();
        for i in 0..8_u32 {
            let from = f64::from(i);
            full.push(from, from + 1.0, 2.7, 1.0).unwrap();
        }
        assert!(matches!(
            full.push(7.5, 9.0, 2.7, 1.0),
            Err(ArrayError::RowOutOfOrder { row: 8, .. })
        ));
        assert_eq!((full.len(), full.from_blocks.len()), (8, 1));
    }

    /// bd-b6r7: a composite length that is zero, negative or not finite is refused, for an empty table too, and
    /// nothing is written.
    #[test]
    fn test_composite_refuses_bad_target_length() {
        let mut table = ColumnarIntervalTable::new();
        table.push(0.0, 2.0, 2.7, 1.0).unwrap();
        let empty = ColumnarIntervalTable::new();
        let mut out = [CompositeInterval::default(); 4];
        for t in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for tab in [&table, &empty] {
                let err = tab.composite_into(t, &mut out).unwrap_err();
                assert!(
                    matches!(err, ArrayError::InvalidTargetLength { target_length_m }
                        if target_length_m.to_bits() == t.to_bits()),
                    "{t}: {err}"
                );
            }
        }
        assert_eq!(out, [CompositeInterval::default(); 4]);
        assert_eq!(empty.composite_into(1.0, &mut out), Ok(0));
        assert_eq!(table.composite_into(1.0, &mut out), Ok(2));
    }

    /// bd-b6r7: rows with a gap between them conserve mass and metal, and a bin wholly in the gap is empty.
    #[test]
    fn test_composite_conserves_mass_across_a_gap() {
        let rows = [
            (0.0, 1.5, 2.60, 1.20),
            (4.0, 5.0, 2.80, 3.50),
            (5.0, 7.3, 2.70, 0.80),
        ];
        let mut table = ColumnarIntervalTable::new();
        for (from, to, rho, grade) in rows {
            table.push(from, to, rho, grade).unwrap();
        }
        let raw_mass: f64 = rows.iter().map(|r| (r.1 - r.0) * r.2).sum();
        let raw_metal: f64 = rows.iter().map(|r| (r.1 - r.0) * r.2 * r.3).sum();

        let mut out = [CompositeInterval::default(); 8];
        let n = table.composite_into(2.0, &mut out).unwrap();
        assert_eq!(n, 4);
        assert_eq!((out[1].from_m, out[1].to_m), (2.0, 4.0));
        assert_eq!(
            (
                out[1].length_m,
                out[1].mean_density_t_m3,
                out[1].mass_weighted_grade_g_t
            ),
            (0.0, 0.0, 0.0)
        );
        assert!((out[0].length_m - 1.5).abs() < 1e-12);

        let mass: f64 = out[..n]
            .iter()
            .map(|c| c.length_m * c.mean_density_t_m3)
            .sum();
        let metal: f64 = out[..n]
            .iter()
            .map(|c| c.length_m * c.mean_density_t_m3 * c.mass_weighted_grade_g_t)
            .sum();
        assert!((mass - raw_mass).abs() < 1e-12, "{mass} vs {raw_mass}");
        assert!((metal - raw_metal).abs() < 1e-12, "{metal} vs {raw_metal}");
    }
}
