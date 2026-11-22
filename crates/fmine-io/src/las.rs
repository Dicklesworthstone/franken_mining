//! Determinism: BitwisePortable
//! Pure-Rust bounded ASPRS LAS 1.4 binary point-cloud reader (`BD-196`, plan §32.1 & §39 WP-11).
//!
//! Supports:
//! - Full 375-byte ASPRS LAS 1.4 Public Header Block (`LASF` signature, version `1.4`, legacy and
//!   64-bit extended point counts, scale/offset decoding, declared bounding box verification);
//! - Variable Length Records (VLRs) and Extended VLRs (EVLRs), including OGC WKT CRS (`record_id
//!   2112`) EPSG parsing into `fmine_crs::CrsId` (returns `None` when unregistered; never guesses);
//! - Point Data Record Formats `0..=10` (coordinates, intensity, return counts, classification,
//!   scan angle, user data, point source ID, optional GPS time, RGB, NIR);
//! - Strict `PO-PORT-1` byte (`MAX_LAS_BYTES`) and point (`MAX_LAS_POINTS`) caps, zero `unsafe`,
//!   and explicit refusal of compressed LAZ format bits (`format >= 128`).

use std::fmt;

use fmine_core::hash::{Digest32, sha256};
use fmine_crs::CrsId;
use fmine_evidence::{Refusal, RefusalCode};

/// ASPRS LAS 1.4 mandatory public header size in bytes.
pub const LAS14_HEADER_SIZE: usize = 375;
/// ASPRS VLR fixed header size in bytes.
pub const LAS_VLR_HEADER_SIZE: usize = 54;
/// ASPRS EVLR fixed header size in bytes.
pub const LAS_EVLR_HEADER_SIZE: usize = 60;
/// Maximum admitted LAS byte payload (64 MiB, `PO-PORT-1`).
pub const MAX_LAS_BYTES: usize = 64 * 1024 * 1024;
/// Maximum admitted point count per LAS import (2,000,000 points).
pub const MAX_LAS_POINTS: usize = 2_000_000;
/// Maximum admitted VLR + EVLR record count.
pub const MAX_LAS_VLRS: usize = 4_096;

/// Typed errors raised by the bounded LAS 1.4 binary reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LasError {
    ByteBudgetExceeded {
        actual: usize,
        limit: usize,
    },
    PointBudgetExceeded {
        actual: u64,
        limit: usize,
    },
    InvalidMagic,
    UnsupportedVersion {
        major: u8,
        minor: u8,
    },
    CompressedLazUnsupported {
        raw_format: u8,
    },
    UnsupportedPointFormat {
        format: u8,
    },
    MalformedHeader {
        detail: String,
    },
    MalformedVlr {
        detail: String,
    },
    TruncatedPointData {
        expected_bytes: u64,
        actual_bytes: usize,
    },
    BoundingBoxMismatch {
        detail: String,
    },
}

impl LasError {
    /// Map LAS reader failures to registered `FMINE-REF-*` codes.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::BoundingBoxMismatch { .. } => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            _ => Refusal::new(RefusalCode::SourceCellUnusable208, self.to_string()),
        }
    }
}

impl fmt::Display for LasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteBudgetExceeded { actual, limit } => {
                write!(f, "LAS byte size {actual} exceeds limit {limit}")
            }
            Self::PointBudgetExceeded { actual, limit } => {
                write!(f, "LAS point count {actual} exceeds limit {limit}")
            }
            Self::InvalidMagic => write!(f, "invalid LAS file signature (expected 'LASF')"),
            Self::UnsupportedVersion { major, minor } => {
                write!(f, "unsupported LAS version {major}.{minor} (expected 1.4)")
            }
            Self::CompressedLazUnsupported { raw_format } => {
                write!(
                    f,
                    "compressed LAZ point format {raw_format} is unsupported; decompress before ingest"
                )
            }
            Self::UnsupportedPointFormat { format } => {
                write!(f, "unsupported LAS 1.4 point data record format {format}")
            }
            Self::MalformedHeader { detail } => write!(f, "malformed LAS 1.4 header: {detail}"),
            Self::MalformedVlr { detail } => write!(f, "malformed LAS 1.4 VLR/EVLR: {detail}"),
            Self::TruncatedPointData {
                expected_bytes,
                actual_bytes,
            } => write!(
                f,
                "truncated LAS point data: need {expected_bytes} bytes, file has {actual_bytes}"
            ),
            Self::BoundingBoxMismatch { detail } => {
                write!(f, "LAS header bounding box mismatch: {detail}")
            }
        }
    }
}

impl std::error::Error for LasError {}

/// Minimum ASPRS LAS 1.4 point record length for Point Data Record Formats `0..=10`.
#[must_use]
pub const fn min_point_record_len(format: u8) -> Option<u16> {
    match format {
        0 => Some(20),
        1 => Some(28),
        2 => Some(26),
        3 => Some(34),
        4 => Some(57),
        5 => Some(63),
        6 => Some(30),
        7 => Some(36),
        8 => Some(38),
        9 => Some(59),
        10 => Some(67),
        _ => None,
    }
}

/// Decoded ASPRS LAS 1.4 Public Header Block (375 bytes).
#[derive(Debug, Clone, PartialEq)]
pub struct LasHeader {
    pub file_source_id: u16,
    pub global_encoding: u16,
    pub version_major: u8,
    pub version_minor: u8,
    pub system_identifier: String,
    pub generating_software: String,
    pub creation_day_of_year: u16,
    pub creation_year: u16,
    pub header_size: u16,
    pub offset_to_point_data: u32,
    pub num_vlrs: u32,
    pub point_format: u8,
    pub point_record_length: u16,
    pub point_count: u64,
    pub scale: [f64; 3],
    pub offset: [f64; 3],
    pub min_xyz: [f64; 3],
    pub max_xyz: [f64; 3],
    pub start_of_first_evlr: u64,
    pub num_evlrs: u32,
}

impl LasHeader {
    /// Returns `true` when Global Encoding bit 4 (`0x0010`) declares OGC WKT CRS usage.
    #[must_use]
    pub const fn uses_wkt_crs(&self) -> bool {
        (self.global_encoding & 0x0010) != 0
    }
}

/// One Variable Length Record (VLR) or Extended VLR (EVLR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LasVlr {
    pub user_id: String,
    pub record_id: u16,
    pub description: String,
    pub payload: Vec<u8>,
    pub is_extended: bool,
}

/// One decoded LAS 1.4 survey point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LasPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub intensity: u16,
    pub return_number: u8,
    pub number_of_returns: u8,
    pub classification: u8,
    pub scan_angle_deg: f64,
    pub user_data: u8,
    pub point_source_id: u16,
    pub gps_time: Option<f64>,
    pub rgb: Option<[u16; 3]>,
    pub nir: Option<u16>,
}

/// Decoded bounded LAS 1.4 point cloud with SHA-256 provenance and explicit/detected CRS.
#[derive(Debug, Clone, PartialEq)]
pub struct LasPointCloud {
    pub source_sha256: Digest32,
    pub header: LasHeader,
    pub vlrs: Vec<LasVlr>,
    pub wkt_crs: Option<String>,
    pub detected_crs: Option<CrsId>,
    pub points: Vec<LasPoint>,
}

fn read_u16_le(bytes: &[u8], off: usize) -> Result<u16, LasError> {
    let sl = bytes
        .get(off..off + 2)
        .ok_or_else(|| LasError::MalformedHeader {
            detail: format!("u16 out of bounds at {off}"),
        })?;
    Ok(u16::from_le_bytes([sl[0], sl[1]]))
}

fn read_u32_le(bytes: &[u8], off: usize) -> Result<u32, LasError> {
    let sl = bytes
        .get(off..off + 4)
        .ok_or_else(|| LasError::MalformedHeader {
            detail: format!("u32 out of bounds at {off}"),
        })?;
    Ok(u32::from_le_bytes([sl[0], sl[1], sl[2], sl[3]]))
}

fn read_i32_le(bytes: &[u8], off: usize) -> Result<i32, LasError> {
    let sl = bytes
        .get(off..off + 4)
        .ok_or_else(|| LasError::MalformedHeader {
            detail: format!("i32 out of bounds at {off}"),
        })?;
    Ok(i32::from_le_bytes([sl[0], sl[1], sl[2], sl[3]]))
}

fn read_i16_le(bytes: &[u8], off: usize) -> Result<i16, LasError> {
    let sl = bytes
        .get(off..off + 2)
        .ok_or_else(|| LasError::MalformedHeader {
            detail: format!("i16 out of bounds at {off}"),
        })?;
    Ok(i16::from_le_bytes([sl[0], sl[1]]))
}

fn read_u64_le(bytes: &[u8], off: usize) -> Result<u64, LasError> {
    let sl = bytes
        .get(off..off + 8)
        .ok_or_else(|| LasError::MalformedHeader {
            detail: format!("u64 out of bounds at {off}"),
        })?;
    Ok(u64::from_le_bytes([
        sl[0], sl[1], sl[2], sl[3], sl[4], sl[5], sl[6], sl[7],
    ]))
}

fn read_f64_le(bytes: &[u8], off: usize) -> Result<f64, LasError> {
    let bits = read_u64_le(bytes, off)?;
    let val = f64::from_bits(bits);
    if !val.is_finite() {
        return Err(LasError::MalformedHeader {
            detail: format!("non-finite f64 at byte offset {off}"),
        });
    }
    Ok(val)
}

fn decode_c_ascii(slice: &[u8]) -> String {
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).trim().to_string()
}

/// Extract a known EPSG code (`EPSG:4326`, `EPSG:32601..=32660`, `EPSG:32701..=32760`) from an OGC
/// WKT CRS string if unambiguously declared (`AUTHORITY["EPSG","32641"]` or `ID["EPSG",32641]`).
#[must_use]
pub fn detect_crs_from_wkt(wkt: &str) -> Option<CrsId> {
    let bytes = wkt.as_bytes();
    let mut last_epsg: Option<u32> = None;
    let mut i = 0;
    while i + 6 < bytes.len() {
        if &bytes[i..i + 6] == b"\"EPSG\"" {
            // Scan forward past separator/quote to digits.
            let mut j = i + 6;
            while j < bytes.len() && matches!(bytes[j], b',' | b' ' | b'"') {
                j += 1;
            }
            let start = j;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > start
                && let Ok(code) = core::str::from_utf8(&bytes[start..j])
                    .unwrap_or("")
                    .parse::<u32>()
            {
                last_epsg = Some(code);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    match last_epsg? {
        4326 => Some(CrsId::Epsg4326),
        c @ 32601..=32660 => Some(CrsId::Wgs84Utm {
            zone: (c - 32600) as u8,
            north: true,
        }),
        c @ 32701..=32760 => Some(CrsId::Wgs84Utm {
            zone: (c - 32700) as u8,
            north: false,
        }),
        _ => None,
    }
}

/// Parse an ASPRS LAS 1.4 binary payload under strict byte/point/VLR caps (`BD-196`).
pub fn parse_las14(bytes: &[u8]) -> Result<LasPointCloud, LasError> {
    if bytes.len() > MAX_LAS_BYTES {
        return Err(LasError::ByteBudgetExceeded {
            actual: bytes.len(),
            limit: MAX_LAS_BYTES,
        });
    }
    if bytes.len() < LAS14_HEADER_SIZE {
        return Err(LasError::MalformedHeader {
            detail: format!(
                "file too short for LAS 1.4 header: {} < {LAS14_HEADER_SIZE} bytes",
                bytes.len()
            ),
        });
    }
    if &bytes[0..4] != b"LASF" {
        return Err(LasError::InvalidMagic);
    }

    let file_source_id = read_u16_le(bytes, 4)?;
    let global_encoding = read_u16_le(bytes, 6)?;
    let version_major = bytes[24];
    let version_minor = bytes[25];
    if version_major != 1 || version_minor != 4 {
        return Err(LasError::UnsupportedVersion {
            major: version_major,
            minor: version_minor,
        });
    }

    let system_identifier = decode_c_ascii(&bytes[26..58]);
    let generating_software = decode_c_ascii(&bytes[58..90]);
    let creation_day_of_year = read_u16_le(bytes, 90)?;
    let creation_year = read_u16_le(bytes, 92)?;
    let header_size = read_u16_le(bytes, 94)?;
    if usize::from(header_size) < LAS14_HEADER_SIZE || usize::from(header_size) > bytes.len() {
        return Err(LasError::MalformedHeader {
            detail: format!(
                "invalid LAS 1.4 header_size {header_size} (must be >= {LAS14_HEADER_SIZE} and <= file len)"
            ),
        });
    }
    let offset_to_point_data = read_u32_le(bytes, 96)?;
    if (offset_to_point_data as usize) < usize::from(header_size)
        || (offset_to_point_data as usize) > bytes.len()
    {
        return Err(LasError::MalformedHeader {
            detail: format!("invalid offset_to_point_data {offset_to_point_data}"),
        });
    }

    let num_vlrs = read_u32_le(bytes, 100)?;
    let raw_point_format = bytes[104];
    if (raw_point_format & 0x80) != 0 {
        return Err(LasError::CompressedLazUnsupported {
            raw_format: raw_point_format,
        });
    }
    let min_rec_len =
        min_point_record_len(raw_point_format).ok_or(LasError::UnsupportedPointFormat {
            format: raw_point_format,
        })?;
    let point_record_length = read_u16_le(bytes, 105)?;
    if point_record_length < min_rec_len {
        return Err(LasError::MalformedHeader {
            detail: format!(
                "point_record_length {point_record_length} < minimum {min_rec_len} for format {raw_point_format}"
            ),
        });
    }

    let legacy_point_count = u64::from(read_u32_le(bytes, 107)?);
    let scale = [
        read_f64_le(bytes, 131)?,
        read_f64_le(bytes, 139)?,
        read_f64_le(bytes, 147)?,
    ];
    for (axis, &s) in scale.iter().enumerate() {
        if s <= 0.0 {
            return Err(LasError::MalformedHeader {
                detail: format!("scale[{axis}] must be strictly positive, got {s}"),
            });
        }
    }
    let offset = [
        read_f64_le(bytes, 155)?,
        read_f64_le(bytes, 163)?,
        read_f64_le(bytes, 171)?,
    ];
    let max_x = read_f64_le(bytes, 179)?;
    let min_x = read_f64_le(bytes, 187)?;
    let max_y = read_f64_le(bytes, 195)?;
    let min_y = read_f64_le(bytes, 203)?;
    let max_z = read_f64_le(bytes, 211)?;
    let min_z = read_f64_le(bytes, 219)?;

    let start_of_first_evlr = read_u64_le(bytes, 235)?;
    let num_evlrs = read_u32_le(bytes, 243)?;
    let ext_point_count = read_u64_le(bytes, 247)?;
    // LAS 1.4 §Public Header Block: extended count is authoritative for formats >= 6 or when > 0.
    let point_count = if ext_point_count > 0 || raw_point_format >= 6 {
        ext_point_count
    } else {
        legacy_point_count
    };
    if point_count > MAX_LAS_POINTS as u64 {
        return Err(LasError::PointBudgetExceeded {
            actual: point_count,
            limit: MAX_LAS_POINTS,
        });
    }
    if point_count > 0 && (min_x > max_x || min_y > max_y || min_z > max_z) {
        return Err(LasError::MalformedHeader {
            detail: "header min_xyz exceeds max_xyz".to_string(),
        });
    }

    let total_vlr_count = u64::from(num_vlrs) + u64::from(num_evlrs);
    if total_vlr_count > MAX_LAS_VLRS as u64 {
        return Err(LasError::MalformedVlr {
            detail: format!("total VLR+EVLR count {total_vlr_count} exceeds cap {MAX_LAS_VLRS}"),
        });
    }

    // Parse VLRs between `header_size` and `offset_to_point_data`.
    let mut vlrs = Vec::with_capacity(total_vlr_count as usize);
    let mut cursor = usize::from(header_size);
    let point_data_start = offset_to_point_data as usize;
    for idx in 0..num_vlrs {
        if cursor + LAS_VLR_HEADER_SIZE > point_data_start {
            return Err(LasError::MalformedVlr {
                detail: format!("VLR {idx} header overruns offset_to_point_data"),
            });
        }
        let user_id = decode_c_ascii(&bytes[cursor + 2..cursor + 18]);
        let record_id = read_u16_le(bytes, cursor + 18)?;
        let rec_len = usize::from(read_u16_le(bytes, cursor + 20)?);
        let description = decode_c_ascii(&bytes[cursor + 22..cursor + 54]);
        cursor += LAS_VLR_HEADER_SIZE;
        let end = cursor
            .checked_add(rec_len)
            .ok_or_else(|| LasError::MalformedVlr {
                detail: "VLR payload offset overflow".to_string(),
            })?;
        if end > point_data_start {
            return Err(LasError::MalformedVlr {
                detail: format!("VLR {idx} payload overruns offset_to_point_data"),
            });
        }
        vlrs.push(LasVlr {
            user_id,
            record_id,
            description,
            payload: bytes[cursor..end].to_vec(),
            is_extended: false,
        });
        cursor = end;
    }

    // Verify point data range and parse EVLRs if present.
    let point_bytes_u64 = point_count
        .checked_mul(u64::from(point_record_length))
        .ok_or_else(|| LasError::MalformedHeader {
            detail: "point byte length overflow".to_string(),
        })?;
    let point_end_u64 = u64::from(offset_to_point_data)
        .checked_add(point_bytes_u64)
        .ok_or_else(|| LasError::MalformedHeader {
            detail: "point end offset overflow".to_string(),
        })?;
    if point_end_u64 > bytes.len() as u64 {
        return Err(LasError::TruncatedPointData {
            expected_bytes: point_end_u64,
            actual_bytes: bytes.len(),
        });
    }

    if num_evlrs > 0 {
        if start_of_first_evlr < point_end_u64 || start_of_first_evlr > bytes.len() as u64 {
            return Err(LasError::MalformedVlr {
                detail: format!("invalid start_of_first_evlr {start_of_first_evlr}"),
            });
        }
        let mut e_cursor = start_of_first_evlr as usize;
        for idx in 0..num_evlrs {
            if e_cursor + LAS_EVLR_HEADER_SIZE > bytes.len() {
                return Err(LasError::MalformedVlr {
                    detail: format!("EVLR {idx} header truncated"),
                });
            }
            let user_id = decode_c_ascii(&bytes[e_cursor + 2..e_cursor + 18]);
            let record_id = read_u16_le(bytes, e_cursor + 18)?;
            let rec_len_u64 = read_u64_le(bytes, e_cursor + 20)?;
            let rec_len = usize::try_from(rec_len_u64).map_err(|_| LasError::MalformedVlr {
                detail: format!("EVLR {idx} payload length overflow"),
            })?;
            let description = decode_c_ascii(&bytes[e_cursor + 28..e_cursor + 60]);
            e_cursor += LAS_EVLR_HEADER_SIZE;
            let end = e_cursor
                .checked_add(rec_len)
                .ok_or_else(|| LasError::MalformedVlr {
                    detail: "EVLR payload offset overflow".to_string(),
                })?;
            if end > bytes.len() {
                return Err(LasError::MalformedVlr {
                    detail: format!("EVLR {idx} payload truncated"),
                });
            }
            vlrs.push(LasVlr {
                user_id,
                record_id,
                description,
                payload: bytes[e_cursor..end].to_vec(),
                is_extended: true,
            });
            e_cursor = end;
        }
    }

    // Extract OGC WKT CRS (LASF_Projection record_id 2112) when present.
    let wkt_crs = vlrs
        .iter()
        .find(|v| v.user_id == "LASF_Projection" && v.record_id == 2112)
        .map(|v| decode_c_ascii(&v.payload));
    let detected_crs = wkt_crs.as_deref().and_then(detect_crs_from_wkt);

    // Decode all points and check header bounding box consistency.
    let rec_len = usize::from(point_record_length);
    let mut points = Vec::with_capacity(point_count as usize);
    let tol = [
        scale[0] * 2.0 + 1e-9,
        scale[1] * 2.0 + 1e-9,
        scale[2] * 2.0 + 1e-9,
    ];
    for p_idx in 0..(point_count as usize) {
        let base = point_data_start + p_idx * rec_len;
        let rec = &bytes[base..base + rec_len];
        let x = f64::from(read_i32_le(rec, 0)?) * scale[0] + offset[0];
        let y = f64::from(read_i32_le(rec, 4)?) * scale[1] + offset[1];
        let z = f64::from(read_i32_le(rec, 8)?) * scale[2] + offset[2];
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
            return Err(LasError::BoundingBoxMismatch {
                detail: format!("point {p_idx} produced non-finite coordinate"),
            });
        }
        if x < min_x - tol[0]
            || x > max_x + tol[0]
            || y < min_y - tol[1]
            || y > max_y + tol[1]
            || z < min_z - tol[2]
            || z > max_z + tol[2]
        {
            return Err(LasError::BoundingBoxMismatch {
                detail: format!(
                    "point {p_idx} ({x}, {y}, {z}) outside header bbox \
                     [{min_x}..{max_x}, {min_y}..{max_y}, {min_z}..{max_z}]"
                ),
            });
        }
        let intensity = read_u16_le(rec, 12)?;
        let (
            return_number,
            number_of_returns,
            classification,
            scan_angle_deg,
            user_data,
            point_source_id,
            gps_time,
            rgb,
            nir,
        ) = if raw_point_format <= 5 {
            let flags = rec[14];
            let ret_num = flags & 0x07;
            let num_ret = (flags >> 3) & 0x07;
            let class = rec[15] & 0x1F;
            let scan_deg = f64::from(rec[16] as i8);
            let udata = rec[17];
            let ps_id = read_u16_le(rec, 18)?;
            let gps = match raw_point_format {
                1 | 3 | 4 | 5 => Some(read_f64_le(rec, 20)?),
                _ => None,
            };
            let rgb = match raw_point_format {
                2 => Some([
                    read_u16_le(rec, 20)?,
                    read_u16_le(rec, 22)?,
                    read_u16_le(rec, 24)?,
                ]),
                3 | 5 => Some([
                    read_u16_le(rec, 28)?,
                    read_u16_le(rec, 30)?,
                    read_u16_le(rec, 32)?,
                ]),
                _ => None,
            };
            (
                ret_num, num_ret, class, scan_deg, udata, ps_id, gps, rgb, None,
            )
        } else {
            // LAS 1.4 extended formats 6..=10.
            let ret_byte = rec[14];
            let ret_num = ret_byte & 0x0F;
            let num_ret = (ret_byte >> 4) & 0x0F;
            let class = rec[16];
            let udata = rec[17];
            let scan_deg = f64::from(read_i16_le(rec, 18)?) * 0.006;
            let ps_id = read_u16_le(rec, 20)?;
            let gps = Some(read_f64_le(rec, 22)?);
            let rgb = match raw_point_format {
                7 | 8 | 10 => Some([
                    read_u16_le(rec, 30)?,
                    read_u16_le(rec, 32)?,
                    read_u16_le(rec, 34)?,
                ]),
                _ => None,
            };
            let nir = match raw_point_format {
                8 | 10 => Some(read_u16_le(rec, 36)?),
                _ => None,
            };
            (
                ret_num, num_ret, class, scan_deg, udata, ps_id, gps, rgb, nir,
            )
        };

        points.push(LasPoint {
            x,
            y,
            z,
            intensity,
            return_number,
            number_of_returns,
            classification,
            scan_angle_deg,
            user_data,
            point_source_id,
            gps_time,
            rgb,
            nir,
        });
    }

    Ok(LasPointCloud {
        source_sha256: sha256(bytes),
        header: LasHeader {
            file_source_id,
            global_encoding,
            version_major,
            version_minor,
            system_identifier,
            generating_software,
            creation_day_of_year,
            creation_year,
            header_size,
            offset_to_point_data,
            num_vlrs,
            point_format: raw_point_format,
            point_record_length,
            point_count,
            scale,
            offset,
            min_xyz: [min_x, min_y, min_z],
            max_xyz: [max_x, max_y, max_z],
            start_of_first_evlr,
            num_evlrs,
        },
        vlrs,
        wkt_crs,
        detected_crs,
        points,
    })
}

/// Deterministic minimal LAS 1.4 binary writer for Format 6 or 7 with optional OGC WKT CRS VLR
/// (supports synthetic spec-conformance tests without external dependencies).
pub fn write_minimal_las14(
    points: &[LasPoint],
    scale: [f64; 3],
    offset: [f64; 3],
    point_format: u8,
    wkt_crs: Option<&str>,
) -> Result<Vec<u8>, LasError> {
    if point_format != 6 && point_format != 7 {
        return Err(LasError::UnsupportedPointFormat {
            format: point_format,
        });
    }
    for &s in &scale {
        if !s.is_finite() || s <= 0.0 {
            return Err(LasError::MalformedHeader {
                detail: "write_minimal_las14 scale must be finite and > 0".to_string(),
            });
        }
    }
    let rec_len = min_point_record_len(point_format).ok_or(LasError::UnsupportedPointFormat {
        format: point_format,
    })?;
    let mut vlr_bytes = Vec::new();
    let mut num_vlrs = 0_u32;
    let mut global_encoding = 0_u16;
    if let Some(wkt) = wkt_crs {
        global_encoding |= 0x0010; // WKT bit
        let mut payload = wkt.as_bytes().to_vec();
        payload.push(0);
        let plen = u16::try_from(payload.len()).map_err(|_| LasError::MalformedVlr {
            detail: "WKT VLR exceeds u16 length".to_string(),
        })?;
        vlr_bytes.extend_from_slice(&0_u16.to_le_bytes()); // reserved
        let mut uid = [0u8; 16];
        uid[..15].copy_from_slice(b"LASF_Projection");
        vlr_bytes.extend_from_slice(&uid);
        vlr_bytes.extend_from_slice(&2112_u16.to_le_bytes());
        vlr_bytes.extend_from_slice(&plen.to_le_bytes());
        let mut desc = [0u8; 32];
        desc[..15].copy_from_slice(b"OGC WKT CRS VLR");
        vlr_bytes.extend_from_slice(&desc);
        vlr_bytes.extend_from_slice(&payload);
        num_vlrs = 1;
    }

    let mut min_xyz = [f64::INFINITY; 3];
    let mut max_xyz = [f64::NEG_INFINITY; 3];
    let mut pt_bytes = Vec::with_capacity(points.len() * usize::from(rec_len));
    for p in points {
        let qx = ((p.x - offset[0]) / scale[0]).round() as i32;
        let qy = ((p.y - offset[1]) / scale[1]).round() as i32;
        let qz = ((p.z - offset[2]) / scale[2]).round() as i32;
        let rx = f64::from(qx) * scale[0] + offset[0];
        let ry = f64::from(qy) * scale[1] + offset[1];
        let rz = f64::from(qz) * scale[2] + offset[2];
        min_xyz[0] = min_xyz[0].min(rx);
        min_xyz[1] = min_xyz[1].min(ry);
        min_xyz[2] = min_xyz[2].min(rz);
        max_xyz[0] = max_xyz[0].max(rx);
        max_xyz[1] = max_xyz[1].max(ry);
        max_xyz[2] = max_xyz[2].max(rz);

        pt_bytes.extend_from_slice(&qx.to_le_bytes());
        pt_bytes.extend_from_slice(&qy.to_le_bytes());
        pt_bytes.extend_from_slice(&qz.to_le_bytes());
        pt_bytes.extend_from_slice(&p.intensity.to_le_bytes());
        pt_bytes.push((p.return_number & 0x0F) | ((p.number_of_returns & 0x0F) << 4));
        pt_bytes.push(0); // classification flags + channel + scan dir
        pt_bytes.push(p.classification);
        pt_bytes.push(p.user_data);
        let scan_raw = (p.scan_angle_deg / 0.006).round() as i16;
        pt_bytes.extend_from_slice(&scan_raw.to_le_bytes());
        pt_bytes.extend_from_slice(&p.point_source_id.to_le_bytes());
        pt_bytes.extend_from_slice(&p.gps_time.unwrap_or(0.0).to_bits().to_le_bytes());
        if point_format == 7 {
            let rgb = p.rgb.unwrap_or([0, 0, 0]);
            pt_bytes.extend_from_slice(&rgb[0].to_le_bytes());
            pt_bytes.extend_from_slice(&rgb[1].to_le_bytes());
            pt_bytes.extend_from_slice(&rgb[2].to_le_bytes());
        }
    }
    if points.is_empty() {
        min_xyz = [0.0; 3];
        max_xyz = [0.0; 3];
    }

    let offset_to_point_data =
        u32::try_from(LAS14_HEADER_SIZE + vlr_bytes.len()).map_err(|_| {
            LasError::MalformedHeader {
                detail: "offset_to_point_data overflow".to_string(),
            }
        })?;
    let mut header = vec![0u8; LAS14_HEADER_SIZE];
    header[0..4].copy_from_slice(b"LASF");
    header[6..8].copy_from_slice(&global_encoding.to_le_bytes());
    header[24] = 1;
    header[25] = 4;
    header[26..38].copy_from_slice(b"FRANKEN_MINE");
    header[58..66].copy_from_slice(b"fmine-io");
    header[90..92].copy_from_slice(&1_u16.to_le_bytes());
    header[92..94].copy_from_slice(&2026_u16.to_le_bytes());
    header[94..96].copy_from_slice(&(LAS14_HEADER_SIZE as u16).to_le_bytes());
    header[96..100].copy_from_slice(&offset_to_point_data.to_le_bytes());
    header[100..104].copy_from_slice(&num_vlrs.to_le_bytes());
    header[104] = point_format;
    header[105..107].copy_from_slice(&rec_len.to_le_bytes());
    header[131..139].copy_from_slice(&scale[0].to_bits().to_le_bytes());
    header[139..147].copy_from_slice(&scale[1].to_bits().to_le_bytes());
    header[147..155].copy_from_slice(&scale[2].to_bits().to_le_bytes());
    header[155..163].copy_from_slice(&offset[0].to_bits().to_le_bytes());
    header[163..171].copy_from_slice(&offset[1].to_bits().to_le_bytes());
    header[171..179].copy_from_slice(&offset[2].to_bits().to_le_bytes());
    header[179..187].copy_from_slice(&max_xyz[0].to_bits().to_le_bytes());
    header[187..195].copy_from_slice(&min_xyz[0].to_bits().to_le_bytes());
    header[195..203].copy_from_slice(&max_xyz[1].to_bits().to_le_bytes());
    header[203..211].copy_from_slice(&min_xyz[1].to_bits().to_le_bytes());
    header[211..219].copy_from_slice(&max_xyz[2].to_bits().to_le_bytes());
    header[219..227].copy_from_slice(&min_xyz[2].to_bits().to_le_bytes());
    header[247..255].copy_from_slice(&(points.len() as u64).to_le_bytes());

    let mut out = Vec::with_capacity(LAS14_HEADER_SIZE + vlr_bytes.len() + pt_bytes.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&vlr_bytes);
    out.extend_from_slice(&pt_bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_las14_format7_roundtrip_wkt_crs_and_evlr() {
        let pts = vec![
            LasPoint {
                x: 512_345.25,
                y: 3_210_987.50,
                z: 1_142.125,
                intensity: 4200,
                return_number: 1,
                number_of_returns: 2,
                classification: 2, // ASPRS Ground
                scan_angle_deg: -12.0,
                user_data: 7,
                point_source_id: 11,
                gps_time: Some(123_456.75),
                rgb: Some([1000, 2000, 3000]),
                nir: None,
            },
            LasPoint {
                x: 512_360.75,
                y: 3_210_999.00,
                z: 1_155.875,
                intensity: 5100,
                return_number: 2,
                number_of_returns: 2,
                classification: 2,
                scan_angle_deg: 6.0,
                user_data: 8,
                point_source_id: 11,
                gps_time: Some(123_456.80),
                rgb: Some([4000, 5000, 6000]),
                nir: None,
            },
        ];
        let wkt = r#"PROJCS["WGS 84 / UTM zone 41N",AUTHORITY["EPSG","32641"]]"#;
        let mut bytes = write_minimal_las14(
            &pts,
            [0.001, 0.001, 0.001],
            [500_000.0, 3_200_000.0, 1_000.0],
            7,
            Some(wkt),
        )
        .unwrap();

        // Append one EVLR to exercise LAS 1.4 EVLR parsing as well!
        let evlr_start = bytes.len() as u64;
        bytes[235..243].copy_from_slice(&evlr_start.to_le_bytes());
        bytes[243..247].copy_from_slice(&1_u32.to_le_bytes());
        let evlr_payload = b"bench_survey_2026Q2";
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        let mut uid = [0u8; 16];
        uid[..10].copy_from_slice(b"FMINE_META");
        bytes.extend_from_slice(&uid);
        bytes.extend_from_slice(&99_u16.to_le_bytes());
        bytes.extend_from_slice(&(evlr_payload.len() as u64).to_le_bytes());
        let mut desc = [0u8; 32];
        desc[..11].copy_from_slice(b"Survey tag!");
        bytes.extend_from_slice(&desc);
        bytes.extend_from_slice(evlr_payload);

        let cloud = parse_las14(&bytes).unwrap();
        assert!(cloud.header.uses_wkt_crs());
        assert_eq!(cloud.header.point_format, 7);
        assert_eq!(cloud.header.point_count, 2);
        assert_eq!(cloud.vlrs.len(), 2);
        assert!(!cloud.vlrs[0].is_extended);
        assert!(cloud.vlrs[1].is_extended);
        assert_eq!(cloud.detected_crs, Some(CrsId::REKO_DIQ_UTM41N));
        assert_eq!(cloud.points.len(), 2);
        assert!((cloud.points[0].x - 512_345.25).abs() < 1e-6);
        assert!((cloud.points[0].z - 1_142.125).abs() < 1e-6);
        assert_eq!(cloud.points[0].classification, 2);
        assert_eq!(cloud.points[0].rgb, Some([1000, 2000, 3000]));
        assert_eq!(cloud.points[0].gps_time, Some(123_456.75));
    }

    #[test]
    fn test_las14_adversarial_refusals_magic_laz_version_and_bbox() {
        let pts = vec![LasPoint {
            x: 10.0,
            y: 20.0,
            z: 30.0,
            intensity: 100,
            return_number: 1,
            number_of_returns: 1,
            classification: 2,
            scan_angle_deg: 0.0,
            user_data: 0,
            point_source_id: 1,
            gps_time: Some(1.0),
            rgb: None,
            nir: None,
        }];
        let base = write_minimal_las14(&pts, [0.01, 0.01, 0.01], [0.0, 0.0, 0.0], 6, None).unwrap();
        let cloud = parse_las14(&base).unwrap();
        assert_eq!(cloud.detected_crs, None); // undeclared CRS stays None (never guessed!)

        // 1. Bad magic.
        let mut bad_magic = base.clone();
        bad_magic[0] = b'X';
        assert!(matches!(
            parse_las14(&bad_magic),
            Err(LasError::InvalidMagic)
        ));

        // 2. LAS 1.2 version refused.
        let mut v12 = base.clone();
        v12[25] = 2;
        assert!(matches!(
            parse_las14(&v12),
            Err(LasError::UnsupportedVersion { major: 1, minor: 2 })
        ));

        // 3. Compressed LAZ high bit refused.
        let mut laz = base.clone();
        laz[104] |= 0x80;
        assert!(matches!(
            parse_las14(&laz),
            Err(LasError::CompressedLazUnsupported { .. })
        ));

        // 4a. Inverted header bounding box (min_x = 10 > max_x = 5) refused with FMINE-REF-208.
        let mut inverted_bbox = base.clone();
        inverted_bbox[179..187].copy_from_slice(&5.0_f64.to_bits().to_le_bytes());
        assert!(matches!(
            parse_las14(&inverted_bbox),
            Err(LasError::MalformedHeader { .. })
        ));

        // 4b. Valid header bbox [0..5] violated by point x = 10 refused with FMINE-REF-109.
        let mut bad_bbox = base.clone();
        bad_bbox[179..187].copy_from_slice(&5.0_f64.to_bits().to_le_bytes()); // max_x = 5
        bad_bbox[187..195].copy_from_slice(&0.0_f64.to_bits().to_le_bytes()); // min_x = 0
        assert_eq!(
            parse_las14(&bad_bbox).unwrap_err().to_refusal().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // 5. Truncated point payload refused.
        assert!(matches!(
            parse_las14(&base[..base.len() - 5]),
            Err(LasError::TruncatedPointData { .. })
        ));
    }
}
