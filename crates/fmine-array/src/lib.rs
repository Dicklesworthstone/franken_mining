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

    /// Appends a validated assay interval `(from_m, to_m, density_t_m3, grade_g_t)`.
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
    /// composite grades, returning the number of written `CompositeInterval`s.
    pub fn composite_into(
        &self,
        target_length_m: f64,
        out: &mut [CompositeInterval],
    ) -> Result<usize, ArrayError> {
        if self.len == 0 || target_length_m <= 0.0 {
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
}
