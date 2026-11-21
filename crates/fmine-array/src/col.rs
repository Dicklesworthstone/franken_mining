//! `FMT-COL-1` 64-byte aligned columnar interval chunks and zero-allocation multi-hole
//! sweep-line compositing (`BD-310`, plan §29.4).
//!
//! Layout (`FMNE` + `COL1` + version `1` + CRC32C):
//! - Run-length `HoleId`: canonical hole dictionary + strictly ascending `(hole_idx, end_row)` runs;
//! - Delta-zigzag LEB128 millimetre depths (`from_mm`, `to_mm`, monotonic non-overlapping per hole);
//! - Bit-packed `u16` `(measurand_id: 12 bits | qualifiers: 4 bits)` + `u64` validity bitmap;
//! - 64-byte aligned (`#[repr(C, align(64))]`, zero `unsafe`) `f64` buffers for value, dry density,
//!   and error variance;
//! - Per-chunk `ChunkZoneMap` (depth range in mm and 3D bounding box).

use std::collections::BTreeMap;

use fmine_core::codec::{CodecError, Decoder, Encoder};

use crate::{AlignedF64Block, ArrayError};

/// Format tag for `FMT-COL-1` binary chunks (`FMNE` + `COL1`).
pub const TAG_COL_CHUNK: [u8; 4] = *b"COL1";

/// Current schema version for `FMT-COL-1`.
pub const FMT_COL_V1: u16 = 1;

/// Maximum rows permitted in a single `FMT-COL-1` binary frame decode (bounds untrusted inputs).
pub const MAX_CHUNK_ROWS: usize = 20_000_000;

/// Maximum dictionary entries in a single `FMT-COL-1` chunk.
pub const MAX_HOLE_DICT: usize = 1_000_000;

/// Maximum UTF-8 byte length for a single `HoleId` dictionary entry.
pub const MAX_HOLE_ID_BYTES: usize = 128;

/// Maximum 12-bit `MeasurandId` value (`0..=4095`).
pub const MAX_MEASURAND_ID: u16 = 0x0FFF;

/// Bit-packed assay qualifier flags (`§29.4`: `<DL`, `>UL`, `CommittedFlagged`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct QualifierBits(u8);

impl QualifierBits {
    /// No qualifiers set.
    pub const NONE: Self = Self(0);
    /// Assay reported below detection limit (`<DL`).
    pub const BELOW_DL: Self = Self(0b001);
    /// Assay reported above upper limit (`>UL`).
    pub const ABOVE_UL: Self = Self(0b010);
    /// Committed with QA/QC flag (`CommittedFlagged`).
    pub const COMMITTED_FLAGGED: Self = Self(0b100);

    /// Construct from raw low 3 bits, refusing any reserved bit outside `0..=0b111`.
    pub const fn from_bits(bits: u8) -> Result<Self, ArrayError> {
        if bits & !0b111 != 0 {
            return Err(ArrayError::InvalidQualifierBits(bits));
        }
        Ok(Self(bits))
    }

    /// Underlying bitmask (`0..=7`).
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Combine two qualifier masks.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Test whether all bits in `flag` are set.
    #[must_use]
    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }
}

/// Pack a 12-bit `measurand_id` and 3-bit [`QualifierBits`] into one `u16`.
pub fn pack_measurand_qualifier(
    measurand_id: u16,
    qualifiers: QualifierBits,
) -> Result<u16, ArrayError> {
    if measurand_id > MAX_MEASURAND_ID {
        return Err(ArrayError::InvalidMeasurandId(measurand_id));
    }
    Ok(measurand_id | (u16::from(qualifiers.bits()) << 12))
}

/// Unpack a `(measurand_id, QualifierBits)` pair from a packed `u16`.
pub fn unpack_measurand_qualifier(packed: u16) -> Result<(u16, QualifierBits), ArrayError> {
    let measurand_id = packed & MAX_MEASURAND_ID;
    let raw_qual = (packed >> 12) as u8;
    let q = QualifierBits::from_bits(raw_qual)?;
    Ok((measurand_id, q))
}

/// Run-length entry assigning rows `[prev_end .. end_row)` to `hole_dict[hole_idx]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HoleRun {
    /// Index into [`ColumnarIntervalChunk::hole_dict`].
    pub hole_idx: u32,
    /// Exclusive ending row index of this run (strictly increasing across runs).
    pub end_row: u32,
}

/// Per-chunk zone map carrying depth bounds (mm) and 3D spatial bounding box (`§29.4`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkZoneMap {
    /// Minimum `from_mm` across all rows in this chunk (`0` if empty).
    pub min_depth_mm: i32,
    /// Maximum `to_mm` across all rows in this chunk (`0` if empty).
    pub max_depth_mm: i32,
    /// Minimum 3D `[x, y, z]` coordinate of intervals/collars in this chunk.
    pub bbox_min: [f64; 3],
    /// Maximum 3D `[x, y, z]` coordinate of intervals/collars in this chunk.
    pub bbox_max: [f64; 3],
}

impl Default for ChunkZoneMap {
    fn default() -> Self {
        Self {
            min_depth_mm: 0,
            max_depth_mm: 0,
            bbox_min: [0.0; 3],
            bbox_max: [0.0; 3],
        }
    }
}

impl ChunkZoneMap {
    /// Whether this chunk's depth span overlaps `[query_from_mm, query_to_mm)`.
    #[must_use]
    pub const fn overlaps_depth_mm(&self, query_from_mm: i32, query_to_mm: i32) -> bool {
        self.min_depth_mm < query_to_mm && query_from_mm < self.max_depth_mm
    }

    /// Whether this chunk's 3D bounding box overlaps `[q_min, q_max]`.
    #[must_use]
    pub fn overlaps_bbox(&self, q_min: [f64; 3], q_max: [f64; 3]) -> bool {
        self.bbox_min[0] <= q_max[0]
            && q_min[0] <= self.bbox_max[0]
            && self.bbox_min[1] <= q_max[1]
            && q_min[1] <= self.bbox_max[1]
            && self.bbox_min[2] <= q_max[2]
            && q_min[2] <= self.bbox_max[2]
    }
}

/// Multi-hole output composite interval emitted by [`ColumnarIntervalChunk::composite_chunk_into`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HoleComposite {
    /// Dictionary index of the hole (`hole_dict[hole_idx]`).
    pub hole_idx: u32,
    /// Composite bin start depth in millimetres.
    pub from_mm: i32,
    /// Composite bin end depth in millimetres.
    pub to_mm: i32,
    /// Valid sampled length in metres inside `[from_mm, to_mm)`.
    pub length_m: f64,
    /// Mass-weighted mean dry density (t/m³), or `0.0` if the bin fell in a gap / invalid mask.
    pub mean_density_t_m3: f64,
    /// Mass-weighted composite value/grade, or `0.0` if the bin fell in a gap / invalid mask.
    pub mass_weighted_value: f64,
}

/// `FMT-COL-1` 64-byte-aligned columnar interval chunk (`BD-310`, plan §29.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnarIntervalChunk {
    len: usize,
    hole_dict: Vec<String>,
    hole_lookup: BTreeMap<String, u32>,
    hole_runs: Vec<HoleRun>,
    from_mm: Vec<i32>,
    to_mm: Vec<i32>,
    measurand_qual: Vec<u16>,
    validity_bitmap: Vec<u64>,
    value_blocks: Vec<AlignedF64Block>,
    density_blocks: Vec<AlignedF64Block>,
    variance_blocks: Vec<AlignedF64Block>,
    zone_map: ChunkZoneMap,
    has_bbox: bool,
}

/// Input row for appending into [`ColumnarIntervalChunk`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntervalRow<'a> {
    /// Drillhole identifier (run-length dictionary encoded).
    pub hole_id: &'a str,
    /// Interval start depth in millimetres (`to_mm > from_mm`).
    pub from_mm: i32,
    /// Interval end depth in millimetres (`to_mm > from_mm`).
    pub to_mm: i32,
    /// 12-bit measurand identifier (`0..=4095`).
    pub measurand_id: u16,
    /// Bit-packed `<DL` / `>UL` / `CommittedFlagged` qualifiers.
    pub qualifiers: QualifierBits,
    /// Validity bitmap flag (`false` marks missing/masked value without dropping the interval).
    pub valid: bool,
    /// Assay / measurement value (must be finite when `valid == true`).
    pub value: f64,
    /// Dry bulk density in t/m³ (must be finite and strictly positive).
    pub dry_density_t_m3: f64,
    /// Measurement error variance (must be finite and nonnegative).
    pub error_variance: f64,
    /// Optional 3D midpoint/collar coordinates to expand the chunk's spatial zone map.
    pub xyz: Option<[f64; 3]>,
}

impl ColumnarIntervalChunk {
    /// Create an empty `FMT-COL-1` chunk.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an empty `FMT-COL-1` chunk with pre-reserved capacity for `row_capacity` rows and
    /// `hole_capacity` holes.
    #[must_use]
    pub fn with_capacity(row_capacity: usize, hole_capacity: usize) -> Self {
        let blocks = row_capacity.div_ceil(8);
        let words = row_capacity.div_ceil(64);
        Self {
            len: 0,
            hole_dict: Vec::with_capacity(hole_capacity),
            hole_lookup: BTreeMap::new(),
            hole_runs: Vec::with_capacity(hole_capacity),
            from_mm: Vec::with_capacity(row_capacity),
            to_mm: Vec::with_capacity(row_capacity),
            measurand_qual: Vec::with_capacity(row_capacity),
            validity_bitmap: Vec::with_capacity(words),
            value_blocks: Vec::with_capacity(blocks),
            density_blocks: Vec::with_capacity(blocks),
            variance_blocks: Vec::with_capacity(blocks),
            zone_map: ChunkZoneMap::default(),
            has_bbox: false,
        }
    }

    /// Number of stored intervals.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the chunk contains zero intervals.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Dictionary of unique hole identifiers.
    #[must_use]
    pub fn hole_dict(&self) -> &[String] {
        &self.hole_dict
    }

    /// Run-length hole assignments.
    #[must_use]
    pub fn hole_runs(&self) -> &[HoleRun] {
        &self.hole_runs
    }

    /// Millimetre `from` depth slice.
    #[must_use]
    pub fn from_mm_slice(&self) -> &[i32] {
        &self.from_mm
    }

    /// Millimetre `to` depth slice.
    #[must_use]
    pub fn to_mm_slice(&self) -> &[i32] {
        &self.to_mm
    }

    /// Bit-packed `(measurand_id | qualifiers << 12)` slice.
    #[must_use]
    pub fn packed_measurand_qualifiers(&self) -> &[u16] {
        &self.measurand_qual
    }

    /// 64-bit validity bitmap words.
    #[must_use]
    pub fn validity_bitmap(&self) -> &[u64] {
        &self.validity_bitmap
    }

    /// 64-byte aligned value blocks.
    #[must_use]
    pub fn value_blocks(&self) -> &[AlignedF64Block] {
        &self.value_blocks
    }

    /// 64-byte aligned dry density blocks.
    #[must_use]
    pub fn density_blocks(&self) -> &[AlignedF64Block] {
        &self.density_blocks
    }

    /// 64-byte aligned error variance blocks.
    #[must_use]
    pub fn variance_blocks(&self) -> &[AlignedF64Block] {
        &self.variance_blocks
    }

    /// Chunk zone map (depth range in mm and 3D bounding box).
    #[must_use]
    pub const fn zone_map(&self) -> ChunkZoneMap {
        self.zone_map
    }

    /// Check whether row `index` is marked valid in the validity bitmap.
    #[must_use]
    pub fn is_valid(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }
        (self.validity_bitmap[index / 64] & (1u64 << (index % 64))) != 0
    }

    /// Append one validated interval row. Within the same hole, intervals must be in non-overlapping
    /// depth order (`from_mm >= prev_to_mm`); gaps are permitted. A refused row leaves the chunk untouched.
    pub fn push(&mut self, row: IntervalRow<'_>) -> Result<(), ArrayError> {
        if row.hole_id.is_empty() || row.hole_id.len() > MAX_HOLE_ID_BYTES {
            return Err(ArrayError::InvalidHoleId {
                row: self.len,
                detail: format!(
                    "length {} outside 1..={MAX_HOLE_ID_BYTES}",
                    row.hole_id.len()
                ),
            });
        }
        if row.to_mm <= row.from_mm
            || !row.dry_density_t_m3.is_finite()
            || row.dry_density_t_m3 <= 0.0
            || !row.error_variance.is_finite()
            || row.error_variance < 0.0
            || (row.valid && !row.value.is_finite())
        {
            return Err(ArrayError::InvalidInterval {
                row: self.len,
                from_m: f64::from(row.from_mm) * 0.001,
                to_m: f64::from(row.to_mm) * 0.001,
            });
        }
        if let Some(xyz) = row.xyz
            && (!xyz[0].is_finite() || !xyz[1].is_finite() || !xyz[2].is_finite())
        {
            return Err(ArrayError::InvalidInterval {
                row: self.len,
                from_m: f64::from(row.from_mm) * 0.001,
                to_m: f64::from(row.to_mm) * 0.001,
            });
        }
        let packed_mq = pack_measurand_qualifier(row.measurand_id, row.qualifiers)?;
        if self.len >= MAX_CHUNK_ROWS {
            return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                len: self.len + 1,
                max: MAX_CHUNK_ROWS,
            }));
        }

        // Check depth monotonicity if continuing the active hole run.
        let existing_hole = self.hole_lookup.get(row.hole_id).copied();
        let continuing_same_hole = matches!(
            (self.hole_runs.last(), existing_hole),
            (Some(last_run), Some(idx)) if last_run.hole_idx == idx
        );
        if continuing_same_hole {
            let prev_to = self.to_mm[self.len - 1];
            if row.from_mm < prev_to {
                return Err(ArrayError::RowOutOfOrder {
                    row: self.len,
                    from_m: f64::from(row.from_mm) * 0.001,
                    prev_to_m: f64::from(prev_to) * 0.001,
                });
            }
        }

        // Commit only after all validation passed.
        let hole_idx = match existing_hole {
            Some(idx) => idx,
            None => {
                if self.hole_dict.len() >= MAX_HOLE_DICT {
                    return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                        len: self.hole_dict.len() + 1,
                        max: MAX_HOLE_DICT,
                    }));
                }
                let idx = self.hole_dict.len() as u32;
                let owned = row.hole_id.to_string();
                self.hole_dict.push(owned.clone());
                self.hole_lookup.insert(owned, idx);
                idx
            }
        };

        let next_end = (self.len + 1) as u32;
        if continuing_same_hole {
            if let Some(last) = self.hole_runs.last_mut() {
                last.end_row = next_end;
            }
        } else {
            self.hole_runs.push(HoleRun {
                hole_idx,
                end_row: next_end,
            });
        }

        let block_idx = self.len / 8;
        let lane = self.len % 8;
        if lane == 0 {
            self.value_blocks.push(AlignedF64Block([0.0; 8]));
            self.density_blocks.push(AlignedF64Block([0.0; 8]));
            self.variance_blocks.push(AlignedF64Block([0.0; 8]));
        }
        let word_idx = self.len / 64;
        let bit_idx = self.len % 64;
        if bit_idx == 0 {
            self.validity_bitmap.push(0u64);
        }
        if row.valid {
            self.validity_bitmap[word_idx] |= 1u64 << bit_idx;
            self.value_blocks[block_idx].0[lane] = row.value;
        } else {
            self.value_blocks[block_idx].0[lane] = 0.0;
        }
        self.density_blocks[block_idx].0[lane] = row.dry_density_t_m3;
        self.variance_blocks[block_idx].0[lane] = row.error_variance;
        self.from_mm.push(row.from_mm);
        self.to_mm.push(row.to_mm);
        self.measurand_qual.push(packed_mq);

        if self.len == 0 {
            self.zone_map.min_depth_mm = row.from_mm;
            self.zone_map.max_depth_mm = row.to_mm;
        } else {
            if row.from_mm < self.zone_map.min_depth_mm {
                self.zone_map.min_depth_mm = row.from_mm;
            }
            if row.to_mm > self.zone_map.max_depth_mm {
                self.zone_map.max_depth_mm = row.to_mm;
            }
        }
        if let Some(xyz) = row.xyz {
            if !self.has_bbox {
                self.zone_map.bbox_min = xyz;
                self.zone_map.bbox_max = xyz;
                self.has_bbox = true;
            } else {
                for axis in 0..3 {
                    if xyz[axis] < self.zone_map.bbox_min[axis] {
                        self.zone_map.bbox_min[axis] = xyz[axis];
                    }
                    if xyz[axis] > self.zone_map.bbox_max[axis] {
                        self.zone_map.bbox_max[axis] = xyz[axis];
                    }
                }
            }
        }

        self.len += 1;
        Ok(())
    }

    /// Encode this chunk into a canonical `FMNE` + `COL1` + CRC32C binary frame (`BD-310`).
    ///
    /// Depths are delta-zigzag LEB128 encoded per hole run; `HoleId`s are dictionary + run-length
    /// encoded; measurands and qualifiers are bit-packed `u16`s with a `u64` validity bitmap.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(TAG_COL_CHUNK, FMT_COL_V1);
        enc.put_u32(self.len as u32);

        // 1) HoleId dictionary.
        enc.put_u32(self.hole_dict.len() as u32);
        for hole in &self.hole_dict {
            enc.put_str(hole);
        }

        // 2) Run-length HoleRuns.
        enc.put_u32(self.hole_runs.len() as u32);
        for run in &self.hole_runs {
            enc.put_u32(run.hole_idx);
            enc.put_u32(run.end_row);
        }

        // 3) Delta-zigzag LEB128 depths per hole run.
        let mut delta_buf = Vec::with_capacity(self.len * 3);
        let mut run_start = 0usize;
        for run in &self.hole_runs {
            let run_end = run.end_row as usize;
            let mut prev_to = 0i64;
            for row in run_start..run_end {
                let f = i64::from(self.from_mm[row]);
                let t = i64::from(self.to_mm[row]);
                put_signed_varint(&mut delta_buf, f - prev_to);
                put_unsigned_varint(&mut delta_buf, (t - f) as u64);
                prev_to = t;
            }
            run_start = run_end;
        }
        enc.put_bytes(&delta_buf);

        // 4) Bit-packed measurand_id + qualifiers, and validity bitmap.
        for &mq in &self.measurand_qual {
            enc.put_u16(mq);
        }
        enc.put_u32(self.validity_bitmap.len() as u32);
        for &word in &self.validity_bitmap {
            enc.put_u64(word);
        }

        // 5) Contiguous f64 values, dry densities, and error variances (rehydrated into 64-B blocks).
        for row in 0..self.len {
            let b = row / 8;
            let l = row % 8;
            enc.put_u64(self.value_blocks[b].0[l].to_bits());
            enc.put_u64(self.density_blocks[b].0[l].to_bits());
            enc.put_u64(self.variance_blocks[b].0[l].to_bits());
        }

        // 6) Per-chunk zone map.
        enc.put_u8(u8::from(self.has_bbox));
        enc.put_u32(self.zone_map.min_depth_mm as u32);
        enc.put_u32(self.zone_map.max_depth_mm as u32);
        for v in self.zone_map.bbox_min {
            enc.put_u64(v.to_bits());
        }
        for v in self.zone_map.bbox_max {
            enc.put_u64(v.to_bits());
        }

        enc.finish()
    }

    /// Decode a canonical `FMNE` + `COL1` + CRC32C binary frame into a 64-byte aligned chunk.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ArrayError> {
        let mut dec = Decoder::open_frame(bytes, TAG_COL_CHUNK, FMT_COL_V1, FMT_COL_V1)?;
        let len = dec.get_u32()? as usize;
        if len > MAX_CHUNK_ROWS {
            return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                len,
                max: MAX_CHUNK_ROWS,
            }));
        }

        let dict_len = dec.get_u32()? as usize;
        if dict_len > MAX_HOLE_DICT {
            return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                len: dict_len,
                max: MAX_HOLE_DICT,
            }));
        }
        let mut hole_dict = Vec::with_capacity(dict_len);
        let mut hole_lookup = BTreeMap::new();
        for idx in 0..dict_len {
            let name = dec.get_str(MAX_HOLE_ID_BYTES)?.to_string();
            if name.is_empty() || hole_lookup.insert(name.clone(), idx as u32).is_some() {
                return Err(ArrayError::InvalidHoleId {
                    row: idx,
                    detail: "empty or duplicate HoleId dictionary entry".to_string(),
                });
            }
            hole_dict.push(name);
        }

        let runs_len = dec.get_u32()? as usize;
        if runs_len > len {
            return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                len: runs_len,
                max: len,
            }));
        }
        let mut hole_runs = Vec::with_capacity(runs_len);
        let mut prev_end = 0u32;
        for _ in 0..runs_len {
            let hole_idx = dec.get_u32()?;
            let end_row = dec.get_u32()?;
            if (hole_idx as usize) >= dict_len || end_row <= prev_end || (end_row as usize) > len {
                return Err(ArrayError::Codec(CodecError::NonCanonicalOrder));
            }
            hole_runs.push(HoleRun { hole_idx, end_row });
            prev_end = end_row;
        }
        if (prev_end as usize) != len {
            return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                len: prev_end as usize,
                max: len,
            }));
        }

        // Decode delta-zigzag LEB128 depths.
        let delta_bytes = dec.get_bytes(len.saturating_mul(12).max(16))?;
        let mut dpos = 0usize;
        let mut from_mm = Vec::with_capacity(len);
        let mut to_mm = Vec::with_capacity(len);
        let mut run_start = 0usize;
        for run in &hole_runs {
            let run_end = run.end_row as usize;
            let mut prev_to = 0i64;
            for row in run_start..run_end {
                let df = get_signed_varint(delta_bytes, &mut dpos)?;
                let span = get_unsigned_varint(delta_bytes, &mut dpos)?;
                if span == 0 {
                    return Err(ArrayError::InvalidInterval {
                        row,
                        from_m: 0.0,
                        to_m: 0.0,
                    });
                }
                let f64_i = prev_to.checked_add(df).ok_or(ArrayError::Codec(
                    CodecError::LengthOutOfBounds {
                        len: row,
                        max: i32::MAX as usize,
                    },
                ))?;
                let t64_i = f64_i
                    .checked_add(i64::try_from(span).map_err(|_| {
                        ArrayError::Codec(CodecError::LengthOutOfBounds {
                            len: row,
                            max: i32::MAX as usize,
                        })
                    })?)
                    .ok_or(ArrayError::Codec(CodecError::LengthOutOfBounds {
                        len: row,
                        max: i32::MAX as usize,
                    }))?;
                let f = i32::try_from(f64_i).map_err(|_| {
                    ArrayError::Codec(CodecError::LengthOutOfBounds {
                        len: row,
                        max: i32::MAX as usize,
                    })
                })?;
                let t = i32::try_from(t64_i).map_err(|_| {
                    ArrayError::Codec(CodecError::LengthOutOfBounds {
                        len: row,
                        max: i32::MAX as usize,
                    })
                })?;
                if row > run_start && f < prev_to as i32 {
                    return Err(ArrayError::RowOutOfOrder {
                        row,
                        from_m: f64::from(f) * 0.001,
                        prev_to_m: (prev_to as f64) * 0.001,
                    });
                }
                from_mm.push(f);
                to_mm.push(t);
                prev_to = i64::from(t);
            }
            run_start = run_end;
        }
        if dpos != delta_bytes.len() {
            return Err(ArrayError::Codec(CodecError::TrailingBytes(
                delta_bytes.len() - dpos,
            )));
        }

        let mut measurand_qual = Vec::with_capacity(len);
        for _ in 0..len {
            let mq = dec.get_u16()?;
            let _ = unpack_measurand_qualifier(mq)?;
            measurand_qual.push(mq);
        }

        let expected_words = len.div_ceil(64);
        let words_len = dec.get_u32()? as usize;
        if words_len != expected_words {
            return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                len: words_len,
                max: expected_words,
            }));
        }
        let mut validity_bitmap = Vec::with_capacity(words_len);
        for _ in 0..words_len {
            validity_bitmap.push(dec.get_u64()?);
        }

        let blocks = len.div_ceil(8);
        let mut value_blocks = vec![AlignedF64Block([0.0; 8]); blocks];
        let mut density_blocks = vec![AlignedF64Block([0.0; 8]); blocks];
        let mut variance_blocks = vec![AlignedF64Block([0.0; 8]); blocks];
        for row in 0..len {
            let v = f64::from_bits(dec.get_u64()?);
            let d = f64::from_bits(dec.get_u64()?);
            let var = f64::from_bits(dec.get_u64()?);
            let is_valid = (validity_bitmap[row / 64] & (1u64 << (row % 64))) != 0;
            if !d.is_finite()
                || d <= 0.0
                || !var.is_finite()
                || var < 0.0
                || (is_valid && !v.is_finite())
            {
                return Err(ArrayError::InvalidInterval {
                    row,
                    from_m: f64::from(from_mm[row]) * 0.001,
                    to_m: f64::from(to_mm[row]) * 0.001,
                });
            }
            let b = row / 8;
            let l = row % 8;
            value_blocks[b].0[l] = v;
            density_blocks[b].0[l] = d;
            variance_blocks[b].0[l] = var;
        }

        let has_bbox = dec.get_u8()? != 0;
        let min_depth_mm = dec.get_u32()? as i32;
        let max_depth_mm = dec.get_u32()? as i32;
        let bbox_min = [
            f64::from_bits(dec.get_u64()?),
            f64::from_bits(dec.get_u64()?),
            f64::from_bits(dec.get_u64()?),
        ];
        let bbox_max = [
            f64::from_bits(dec.get_u64()?),
            f64::from_bits(dec.get_u64()?),
            f64::from_bits(dec.get_u64()?),
        ];
        dec.finish()?;

        Ok(Self {
            len,
            hole_dict,
            hole_lookup,
            hole_runs,
            from_mm,
            to_mm,
            measurand_qual,
            validity_bitmap,
            value_blocks,
            density_blocks,
            variance_blocks,
            zone_map: ChunkZoneMap {
                min_depth_mm,
                max_depth_mm,
                bbox_min,
                bbox_max,
            },
            has_bbox,
        })
    }

    /// Zero-allocation, single-core O(N) sweep-line multi-hole compositing into `out` (`BD-310`).
    ///
    /// Splits each hole's intervals on regular `target_length_mm` boundaries starting at that hole's
    /// first `from_mm`, conserving mass (`length * density`) and contained metal
    /// (`length * density * value`) across valid rows (rows with cleared validity bits are skipped).
    /// Performs **zero heap allocations** during execution.
    pub fn composite_chunk_into(
        &self,
        target_length_mm: i32,
        out: &mut [HoleComposite],
    ) -> Result<usize, ArrayError> {
        if target_length_mm <= 0 {
            return Err(ArrayError::InvalidTargetLength {
                target_length_m: f64::from(target_length_mm) * 0.001,
            });
        }
        if self.len == 0 {
            return Ok(0);
        }

        let target = i64::from(target_length_mm);
        let mut total_written = 0usize;
        let mut run_start = 0usize;

        for run in &self.hole_runs {
            let run_end = run.end_row as usize;
            if run_start >= run_end {
                continue;
            }
            let hole_start = i64::from(self.from_mm[run_start]);
            let hole_end = i64::from(self.to_mm[run_end - 1]);
            let span = hole_end - hole_start;
            let num_bins = usize::try_from((span + target - 1) / target).unwrap_or(usize::MAX);
            let needed = total_written.saturating_add(num_bins);
            if out.len() < needed {
                return Err(ArrayError::OutputBufferTooSmall {
                    required: needed,
                    capacity: out.len(),
                });
            }

            let hole_slice = &mut out[total_written..total_written + num_bins];
            let mut bin_from = hole_start;
            for slot in hole_slice.iter_mut() {
                let bin_to = (bin_from + target).min(hole_end);
                *slot = HoleComposite {
                    hole_idx: run.hole_idx,
                    from_mm: bin_from as i32,
                    to_mm: bin_to as i32,
                    length_m: 0.0,
                    mean_density_t_m3: 0.0,
                    mass_weighted_value: 0.0,
                };
                bin_from = bin_to;
            }

            // Monotone O(1)-amortized sweep-line cursor across bins for this hole.
            let mut bin_cursor = 0usize;
            for row in run_start..run_end {
                let word = self.validity_bitmap[row >> 6];
                if (word & (1u64 << (row & 63))) == 0 {
                    continue;
                }
                let r_from = i64::from(self.from_mm[row]);
                let r_to = i64::from(self.to_mm[row]);
                let blk = row >> 3;
                let lane = row & 7;
                let rho = self.density_blocks[blk].0[lane];
                let val = self.value_blocks[blk].0[lane];

                while bin_cursor < num_bins && i64::from(hole_slice[bin_cursor].to_mm) <= r_from {
                    bin_cursor += 1;
                }
                let mut b = bin_cursor;
                while b < num_bins {
                    let slot = &mut hole_slice[b];
                    let s_from = i64::from(slot.from_mm);
                    if s_from >= r_to {
                        break;
                    }
                    let s_to = i64::from(slot.to_mm);
                    let ov_start = if r_from > s_from { r_from } else { s_from };
                    let ov_end = if r_to < s_to { r_to } else { s_to };
                    let dl_mm = ov_end - ov_start;
                    if dl_mm > 0 {
                        let dl_m = (dl_mm as f64) * 0.001;
                        let dm = dl_m * rho;
                        slot.length_m += dl_m;
                        slot.mean_density_t_m3 += dm;
                        slot.mass_weighted_value += dm * val;
                    }
                    if s_to <= r_to {
                        b += 1;
                    } else {
                        break;
                    }
                }
            }

            for slot in hole_slice.iter_mut() {
                let total_mass = slot.mean_density_t_m3;
                let total_metal = slot.mass_weighted_value;
                if slot.length_m > 0.0 && total_mass > 0.0 {
                    slot.mean_density_t_m3 = total_mass / slot.length_m;
                    slot.mass_weighted_value = total_metal / total_mass;
                }
            }

            total_written += num_bins;
            run_start = run_end;
        }

        Ok(total_written)
    }
}

fn put_unsigned_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

fn put_signed_varint(out: &mut Vec<u8>, v: i64) {
    let zigzag = ((v << 1) ^ (v >> 63)) as u64;
    put_unsigned_varint(out, zigzag);
}

fn get_unsigned_varint(bytes: &[u8], pos: &mut usize) -> Result<u64, ArrayError> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        let Some(&byte) = bytes.get(*pos) else {
            return Err(ArrayError::Codec(CodecError::UnexpectedEof {
                needed: 1,
                remaining: 0,
            }));
        };
        *pos += 1;
        if shift >= 64 {
            return Err(ArrayError::Codec(CodecError::LengthOutOfBounds {
                len: *pos,
                max: 10,
            }));
        }
        result |= u64::from(byte & 0x7F) << shift;
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
    }
}

fn get_signed_varint(bytes: &[u8], pos: &mut usize) -> Result<i64, ArrayError> {
    let zz = get_unsigned_varint(bytes, pos)?;
    Ok(((zz >> 1) as i64) ^ -((zz & 1) as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_col_1_roundtrip_bit_packing_zone_map_and_one_bit_flip_refusal() {
        let mut chunk = ColumnarIntervalChunk::with_capacity(4, 2);
        chunk
            .push(IntervalRow {
                hole_id: "DDH-001",
                from_mm: 0,
                to_mm: 1500,
                measurand_id: 7,
                qualifiers: QualifierBits::NONE,
                valid: true,
                value: 1.25,
                dry_density_t_m3: 2.65,
                error_variance: 0.04,
                xyz: Some([100.0, 200.0, -50.0]),
            })
            .expect("row 0");
        chunk
            .push(IntervalRow {
                hole_id: "DDH-001",
                from_mm: 2000,
                to_mm: 3500,
                measurand_id: 7,
                qualifiers: QualifierBits::BELOW_DL.union(QualifierBits::COMMITTED_FLAGGED),
                valid: false,
                value: f64::NAN,
                dry_density_t_m3: 2.70,
                error_variance: 0.01,
                xyz: Some([100.5, 200.5, -52.0]),
            })
            .expect("row 1");
        chunk
            .push(IntervalRow {
                hole_id: "DDH-002",
                from_mm: 1000,
                to_mm: 4000,
                measurand_id: 4095,
                qualifiers: QualifierBits::ABOVE_UL,
                valid: true,
                value: 12.5,
                dry_density_t_m3: 2.80,
                error_variance: 0.25,
                xyz: Some([150.0, 250.0, -10.0]),
            })
            .expect("row 2");

        assert_eq!(chunk.len(), 3);
        assert_eq!(chunk.hole_dict(), &["DDH-001", "DDH-002"]);
        assert_eq!(
            chunk.hole_runs(),
            &[
                HoleRun {
                    hole_idx: 0,
                    end_row: 2
                },
                HoleRun {
                    hole_idx: 1,
                    end_row: 3
                }
            ]
        );
        assert!(chunk.is_valid(0));
        assert!(!chunk.is_valid(1));
        assert!(chunk.is_valid(2));
        assert_eq!(
            unpack_measurand_qualifier(chunk.packed_measurand_qualifiers()[1]).expect("unpack"),
            (
                7,
                QualifierBits::BELOW_DL.union(QualifierBits::COMMITTED_FLAGGED)
            )
        );
        assert_eq!(chunk.zone_map().min_depth_mm, 0);
        assert_eq!(chunk.zone_map().max_depth_mm, 4000);
        assert!(chunk.zone_map().overlaps_depth_mm(3500, 5000));
        assert!(!chunk.zone_map().overlaps_depth_mm(4000, 5000));
        assert!(
            chunk
                .zone_map()
                .overlaps_bbox([120.0, 210.0, -40.0], [160.0, 260.0, 0.0])
        );

        let bytes = chunk.to_bytes();
        let decoded = ColumnarIntervalChunk::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, chunk);
        assert_eq!(decoded.to_bytes(), bytes);

        // Single-bit flip anywhere refuses.
        for i in 0..bytes.len() {
            let mut mutated = bytes.clone();
            mutated[i] ^= 0x01;
            assert!(
                ColumnarIntervalChunk::from_bytes(&mutated).is_err(),
                "bit flip at {i} must refuse"
            );
        }
    }

    #[test]
    fn test_fmt_col_1_multi_hole_compositing_conserves_mass_metal_and_skips_invalid() {
        let mut chunk = ColumnarIntervalChunk::new();
        let rows = [
            ("H1", 0, 1500, true, 1.20, 2.60),
            ("H1", 1500, 3200, true, 3.50, 2.80),
            ("H1", 3200, 4000, false, 999.0, 2.70), // invalid: must be ignored
            ("H1", 4000, 6000, true, 0.80, 2.70),
            ("H2", 1000, 2500, true, 2.10, 2.50),
            ("H2", 3500, 5000, true, 4.00, 2.90),
        ];
        let mut raw_mass = 0.0;
        let mut raw_metal = 0.0;
        for (h, f, t, valid, val, rho) in rows {
            if valid {
                let len_m = f64::from(t - f) * 0.001;
                raw_mass += len_m * rho;
                raw_metal += len_m * rho * val;
            }
            chunk
                .push(IntervalRow {
                    hole_id: h,
                    from_mm: f,
                    to_mm: t,
                    measurand_id: 1,
                    qualifiers: QualifierBits::NONE,
                    valid,
                    value: val,
                    dry_density_t_m3: rho,
                    error_variance: 0.0,
                    xyz: None,
                })
                .expect("push");
        }
        let mut out = [HoleComposite::default(); 8];
        let written = chunk
            .composite_chunk_into(2000, &mut out)
            .expect("composite");
        assert_eq!(written, 5); // H1: 3 bins (0-2,2-4,4-6), H2: 2 bins (1-3,3-5)
        let comp_mass: f64 = out[..written]
            .iter()
            .map(|c| c.length_m * c.mean_density_t_m3)
            .sum();
        let comp_metal: f64 = out[..written]
            .iter()
            .map(|c| c.length_m * c.mean_density_t_m3 * c.mass_weighted_value)
            .sum();
        assert!(
            (comp_mass - raw_mass).abs() < 1e-11,
            "{comp_mass} vs {raw_mass}"
        );
        assert!(
            (comp_metal - raw_metal).abs() < 1e-11,
            "{comp_metal} vs {raw_metal}"
        );
    }
}
