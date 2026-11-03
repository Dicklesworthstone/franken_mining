//! The Parquet subset that OMF 2 writes (`BD-194`, plan §32.2, E858), read without the `parquet` crate, which the
//! dependency policy bans (L-5).
//!
//! Read: the Thrift compact footer; a flat schema of required or optional leaves, or one level of groups under the
//! root, with at most one optional level on any path (definition levels 0 or 1, no repetition); physical types
//! BOOLEAN, INT32, INT64, FLOAT, DOUBLE and BYTE_ARRAY; logical types string, date, timestamp (microseconds, UTC) and
//! integer; data pages v1 and v2, PLAIN values, RLE/bit-packed definition levels; codecs UNCOMPRESSED and GZIP.
//!
//! Refused, never guessed: dictionary pages and every other encoding or codec, repeated fields, deeper nesting,
//! other logical types, encrypted files, column data in other files, page CRCs that do not match, and pages, values
//! or levels that do not account for exactly the bytes and rows the metadata declares.

use super::gzip::{crc32, gunzip};

/// The largest page accepted, compressed or not.
pub(crate) const MAX_PAGE_BYTES: usize = 1 << 28;

/// The most bytes one file's pages may inflate to in total.
pub(crate) const MAX_DECODED_BYTES: usize = 1 << 30;

/// The most rows one file may declare.
pub(crate) const MAX_ROWS: usize = 1 << 28;

const MAX_THRIFT_DEPTH: usize = 32;
const MAGIC: &[u8; 4] = b"PAR1";

/// A column's physical type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Physical {
    Boolean,
    Int32,
    Int64,
    Float,
    Double,
    ByteArray,
}

/// A column's logical type, within the subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Logical {
    None,
    String,
    Integer { bits: u8, signed: bool },
    Date,
    TimestampMicrosUtc,
}

/// A leaf column: its path from the root, its types and its largest definition level (0 or 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Leaf {
    pub path: Vec<String>,
    pub physical: Physical,
    pub logical: Logical,
    pub optional: bool,
}

/// The values of a column that are present, in row order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Values {
    Boolean(Vec<bool>),
    Int32(Vec<i32>),
    Int64(Vec<i64>),
    Float(Vec<f32>),
    Double(Vec<f64>),
    Bytes(Vec<Vec<u8>>),
}

impl Values {
    fn empty(physical: Physical) -> Self {
        match physical {
            Physical::Boolean => Self::Boolean(Vec::new()),
            Physical::Int32 => Self::Int32(Vec::new()),
            Physical::Int64 => Self::Int64(Vec::new()),
            Physical::Float => Self::Float(Vec::new()),
            Physical::Double => Self::Double(Vec::new()),
            Physical::ByteArray => Self::Bytes(Vec::new()),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Boolean(v) => v.len(),
            Self::Int32(v) => v.len(),
            Self::Int64(v) => v.len(),
            Self::Float(v) => v.len(),
            Self::Double(v) => v.len(),
            Self::Bytes(v) => v.len(),
        }
    }
}

/// One decoded column. `present` has one entry per row for an optional column, and is `None` for a required one,
/// whose every row is present.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Column {
    pub leaf: Leaf,
    pub present: Option<Vec<bool>>,
    pub values: Values,
}

/// A decoded file: its row count and its leaf columns in schema order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    pub rows: usize,
    pub columns: Vec<Column>,
}

// --- Thrift compact protocol ---

#[derive(Debug, Clone)]
enum T<'a> {
    Bool(bool),
    Int(i64),
    Double,
    Bin(&'a [u8]),
    List(Vec<T<'a>>),
    Struct(Vec<(i16, T<'a>)>),
    Map,
}

struct Thrift<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Thrift<'a> {
    fn byte(&mut self) -> Result<u8, String> {
        let b = *self.b.get(self.pos).ok_or("Thrift data is truncated")?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self
            .b
            .get(self.pos..self.pos.saturating_add(n))
            .ok_or("Thrift data is truncated")?;
        self.pos += n;
        Ok(s)
    }

    fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0_u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            let bits = u64::from(b & 0x7F);
            if shift == 63 && bits > 1 {
                return Err("Thrift varint overflows 64 bits".to_owned());
            }
            v |= bits << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("Thrift varint is longer than 10 bytes".to_owned())
    }

    fn zigzag(&mut self) -> Result<i64, String> {
        let v = self.varint()?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }

    fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }

    fn value(&mut self, kind: u8, depth: usize) -> Result<T<'a>, String> {
        if depth > MAX_THRIFT_DEPTH {
            return Err(format!("Thrift data nested deeper than {MAX_THRIFT_DEPTH}"));
        }
        match kind {
            // In a list, a boolean is a byte of its own.
            1 | 2 => match self.byte()? {
                1 => Ok(T::Bool(true)),
                0 | 2 => Ok(T::Bool(false)),
                b => Err(format!("Thrift boolean byte {b}")),
            },
            3 => Ok(T::Int(i64::from(self.byte()? as i8))),
            4..=6 => Ok(T::Int(self.zigzag()?)),
            // No field the subset reads is a double, so its value is skipped.
            7 => self.take(8).map(|_| T::Double),
            8 => {
                let n = usize::try_from(self.varint()?).map_err(|_| "Thrift binary too long")?;
                Ok(T::Bin(self.take(n)?))
            }
            9 | 10 => {
                let h = self.byte()?;
                let n = match h >> 4 {
                    15 => usize::try_from(self.varint()?).map_err(|_| "Thrift list too long")?,
                    n => usize::from(n),
                };
                // Each element takes at least a byte, so a longer list cannot be in the data.
                if n > self.remaining() {
                    return Err(format!(
                        "Thrift list of {n} elements is longer than its data"
                    ));
                }
                let elem = h & 0x0F;
                (0..n)
                    .map(|_| self.value(elem, depth + 1))
                    .collect::<Result<Vec<_>, _>>()
                    .map(T::List)
            }
            11 => {
                let n = usize::try_from(self.varint()?).map_err(|_| "Thrift map too long")?;
                if n > 0 {
                    if n > self.remaining() {
                        return Err(format!("Thrift map of {n} entries is longer than its data"));
                    }
                    let kv = self.byte()?;
                    for _ in 0..n {
                        self.value(kv >> 4, depth + 1)?;
                        self.value(kv & 0x0F, depth + 1)?;
                    }
                }
                Ok(T::Map)
            }
            12 => self.strukt(depth + 1).map(T::Struct),
            k => Err(format!("Thrift type {k} is not a compact protocol type")),
        }
    }

    fn strukt(&mut self, depth: usize) -> Result<Vec<(i16, T<'a>)>, String> {
        if depth > MAX_THRIFT_DEPTH {
            return Err(format!("Thrift data nested deeper than {MAX_THRIFT_DEPTH}"));
        }
        let mut fields: Vec<(i16, T<'a>)> = Vec::new();
        let mut last = 0_i16;
        loop {
            let h = self.byte()?;
            if h == 0 {
                return Ok(fields);
            }
            let kind = h & 0x0F;
            let id = match h >> 4 {
                0 => i16::try_from(self.zigzag()?).map_err(|_| "Thrift field id out of range")?,
                d => last
                    .checked_add(i16::from(d))
                    .ok_or("Thrift field id out of range")?,
            };
            let v = match kind {
                1 => T::Bool(true),
                2 => T::Bool(false),
                k => self.value(k, depth)?,
            };
            if fields.iter().any(|(f, _)| *f == id) {
                return Err(format!("Thrift field {id} appears twice"));
            }
            fields.push((id, v));
            last = id;
        }
    }
}

type Fields<'s, 'a> = &'s [(i16, T<'a>)];

fn get<'s, 'a>(s: Fields<'s, 'a>, id: i16) -> Option<&'s T<'a>> {
    s.iter().find(|(f, _)| *f == id).map(|(_, v)| v)
}

fn opt_int(s: Fields<'_, '_>, id: i16, what: &str) -> Result<Option<i64>, String> {
    match get(s, id) {
        None => Ok(None),
        Some(T::Int(v)) => Ok(Some(*v)),
        Some(_) => Err(format!("{what} is not an integer")),
    }
}

fn int(s: Fields<'_, '_>, id: i16, what: &str) -> Result<i64, String> {
    opt_int(s, id, what)?.ok_or_else(|| format!("{what} is missing"))
}

fn size(s: Fields<'_, '_>, id: i16, what: &str) -> Result<usize, String> {
    let v = int(s, id, what)?;
    usize::try_from(v).map_err(|_| format!("{what} is {v}"))
}

fn opt_bool(s: Fields<'_, '_>, id: i16, what: &str) -> Result<Option<bool>, String> {
    match get(s, id) {
        None => Ok(None),
        Some(T::Bool(v)) => Ok(Some(*v)),
        Some(_) => Err(format!("{what} is not a boolean")),
    }
}

fn opt_bin<'a>(s: Fields<'_, 'a>, id: i16, what: &str) -> Result<Option<&'a [u8]>, String> {
    match get(s, id) {
        None => Ok(None),
        Some(T::Bin(v)) => Ok(Some(v)),
        Some(_) => Err(format!("{what} is not binary")),
    }
}

fn list<'s, 'a>(s: Fields<'s, 'a>, id: i16, what: &str) -> Result<&'s [T<'a>], String> {
    match get(s, id) {
        Some(T::List(v)) => Ok(v),
        None => Err(format!("{what} is missing")),
        Some(_) => Err(format!("{what} is not a list")),
    }
}

fn opt_struct<'s, 'a>(
    s: Fields<'s, 'a>,
    id: i16,
    what: &str,
) -> Result<Option<Fields<'s, 'a>>, String> {
    match get(s, id) {
        None => Ok(None),
        Some(T::Struct(v)) => Ok(Some(v)),
        Some(_) => Err(format!("{what} is not a struct")),
    }
}

fn as_struct<'s, 'a>(t: &'s T<'a>, what: &str) -> Result<Fields<'s, 'a>, String> {
    match t {
        T::Struct(v) => Ok(v),
        _ => Err(format!("{what} is not a struct")),
    }
}

fn text(b: &[u8], what: &str) -> Result<String, String> {
    String::from_utf8(b.to_vec()).map_err(|_| format!("{what} is not UTF-8"))
}

// --- Schema ---

fn physical(code: i64) -> Result<Physical, String> {
    Ok(match code {
        0 => Physical::Boolean,
        1 => Physical::Int32,
        2 => Physical::Int64,
        4 => Physical::Float,
        5 => Physical::Double,
        6 => Physical::ByteArray,
        3 => return Err("physical type INT96 is outside the subset".to_owned()),
        7 => return Err("physical type FIXED_LEN_BYTE_ARRAY is outside the subset".to_owned()),
        c => return Err(format!("physical type {c} is unknown")),
    })
}

fn logical(e: Fields<'_, '_>, name: &str) -> Result<Logical, String> {
    if let Some(u) = opt_struct(e, 10, "logical type")? {
        let [(id, v)] = u else {
            return Err(format!(
                "column {name}: logical type is not a one-member union"
            ));
        };
        let inner = as_struct(v, "logical type")?;
        return Ok(match *id {
            1 => Logical::String,
            6 => Logical::Date,
            8 => {
                let utc = opt_bool(inner, 1, "timestamp UTC flag")?;
                let unit = opt_struct(inner, 2, "timestamp unit")?;
                match (utc, unit) {
                    (Some(true), Some([(2, _)])) => Logical::TimestampMicrosUtc,
                    _ => {
                        return Err(format!(
                            "column {name}: only timestamps in microseconds, adjusted to UTC, are in the subset"
                        ));
                    }
                }
            }
            10 => {
                let bits = int(inner, 1, "integer bit width")?;
                let signed =
                    opt_bool(inner, 2, "integer sign")?.ok_or("integer sign is missing")?;
                match bits {
                    8 | 16 | 32 | 64 => Logical::Integer {
                        bits: bits as u8,
                        signed,
                    },
                    b => return Err(format!("column {name}: integer bit width {b}")),
                }
            }
            id => {
                return Err(format!(
                    "column {name}: logical type {id} is outside the subset"
                ));
            }
        });
    }
    Ok(match opt_int(e, 6, "converted type")? {
        None => Logical::None,
        Some(0) => Logical::String,
        Some(6) => Logical::Date,
        Some(10) => Logical::TimestampMicrosUtc,
        Some(c @ 11..=18) => {
            let i = (c - 11) as u32;
            Logical::Integer {
                bits: 8 << (i % 4),
                signed: c >= 15,
            }
        }
        Some(c) => {
            return Err(format!(
                "column {name}: converted type {c} is outside the subset"
            ));
        }
    })
}

/// Repetition: `Ok(true)` for optional, `Ok(false)` for required.
fn optional(e: Fields<'_, '_>, name: &str) -> Result<bool, String> {
    match opt_int(e, 3, "repetition")? {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        Some(2) => Err(format!(
            "column {name} is repeated; the subset has no repetition"
        )),
        Some(r) => Err(format!("column {name}: repetition {r} is unknown")),
        None => Err(format!("column {name} has no repetition")),
    }
}

fn leaves(schema: &[T<'_>]) -> Result<Vec<Leaf>, String> {
    let root = as_struct(schema.first().ok_or("the schema is empty")?, "schema root")?;
    let mut rest = schema[1..].iter();
    let mut out = Vec::new();
    let children = size(root, 5, "root child count")?;
    for _ in 0..children {
        let e = as_struct(
            rest.next().ok_or("the schema is truncated")?,
            "schema element",
        )?;
        let name = text(
            opt_bin(e, 4, "column name")?.ok_or("a column has no name")?,
            "column name",
        )?;
        match opt_int(e, 5, "child count")? {
            Some(n) if n > 0 => {
                if get(e, 1).is_some() {
                    return Err(format!("group {name} has a physical type"));
                }
                let group_optional = optional(e, &name)?;
                for _ in 0..n {
                    let c = as_struct(
                        rest.next().ok_or("the schema is truncated")?,
                        "schema element",
                    )?;
                    let cname = text(
                        opt_bin(c, 4, "column name")?.ok_or("a column has no name")?,
                        "column name",
                    )?;
                    if matches!(opt_int(c, 5, "child count")?, Some(m) if m > 0) {
                        return Err(format!(
                            "group {name}.{cname}: groups nest one level at most in the subset"
                        ));
                    }
                    let leaf = leaf(c, vec![name.clone(), cname], group_optional)?;
                    out.push(leaf);
                }
            }
            _ => out.push(leaf(e, vec![name], false)?),
        }
    }
    if rest.next().is_some() {
        return Err("the schema has elements past the root's children".to_owned());
    }
    if out.is_empty() {
        return Err("the schema has no columns".to_owned());
    }
    Ok(out)
}

fn leaf(e: Fields<'_, '_>, path: Vec<String>, under_optional: bool) -> Result<Leaf, String> {
    let name = path.join(".");
    let physical = physical(int(e, 1, "physical type")?)?;
    let own = optional(e, &name)?;
    if own && under_optional {
        return Err(format!(
            "column {name}: definition levels above 1 are outside the subset"
        ));
    }
    let logical = logical(e, &name)?;
    let fits = match logical {
        Logical::None => true,
        Logical::String => physical == Physical::ByteArray,
        Logical::Date => physical == Physical::Int32,
        Logical::TimestampMicrosUtc => physical == Physical::Int64,
        Logical::Integer { bits: 64, .. } => physical == Physical::Int64,
        Logical::Integer { .. } => physical == Physical::Int32,
    };
    if !fits {
        return Err(format!(
            "column {name}: logical type {logical:?} on physical {physical:?}"
        ));
    }
    Ok(Leaf {
        path,
        physical,
        logical,
        optional: own || under_optional,
    })
}

// --- Pages ---

/// `n` definition levels of bit width 1 from the RLE/bit-packed hybrid encoding.
fn levels(b: &[u8], n: usize) -> Result<Vec<bool>, String> {
    let mut t = Thrift { b, pos: 0 };
    let mut out = Vec::with_capacity(n.min(b.len().saturating_mul(8)));
    while out.len() < n {
        let h = t.varint()?;
        let count = usize::try_from(h >> 1).map_err(|_| "level run too long")?;
        if h & 1 == 0 {
            let v = t.byte()?;
            if v > 1 {
                return Err(format!("definition level {v} above the column's 1"));
            }
            if count == 0 || count > n - out.len() {
                return Err(format!(
                    "a level run of {count} where {} remain",
                    n - out.len()
                ));
            }
            out.resize(out.len() + count, v == 1);
        } else {
            // Groups of eight levels, one byte each at bit width 1; the last group may be padded.
            let bytes = t.take(count)?;
            for byte in bytes {
                for bit in 0..8 {
                    if out.len() < n {
                        out.push((byte >> bit) & 1 == 1);
                    }
                }
            }
        }
    }
    if t.pos != b.len() {
        return Err(format!(
            "{} bytes after the definition levels",
            b.len() - t.pos
        ));
    }
    Ok(out)
}

fn plain(b: &[u8], count: usize, into: &mut Values) -> Result<(), String> {
    let width = |w: usize| {
        let need = count.checked_mul(w).ok_or("value count overflows")?;
        if need == b.len() {
            Ok(())
        } else {
            Err(format!(
                "{count} values need {need} bytes, the page holds {}",
                b.len()
            ))
        }
    };
    match into {
        Values::Boolean(v) => {
            if b.len() != count.div_ceil(8) {
                return Err(format!(
                    "{count} booleans need {} bytes, the page holds {}",
                    count.div_ceil(8),
                    b.len()
                ));
            }
            v.extend((0..count).map(|i| (b[i / 8] >> (i % 8)) & 1 == 1));
        }
        Values::Int32(v) => {
            width(4)?;
            v.extend(b.as_chunks::<4>().0.iter().map(|c| i32::from_le_bytes(*c)));
        }
        Values::Float(v) => {
            width(4)?;
            v.extend(b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)));
        }
        Values::Int64(v) => {
            width(8)?;
            v.extend(b.as_chunks::<8>().0.iter().map(|c| i64::from_le_bytes(*c)));
        }
        Values::Double(v) => {
            width(8)?;
            v.extend(b.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)));
        }
        Values::Bytes(v) => {
            let mut pos = 0_usize;
            for _ in 0..count {
                let len = b
                    .get(pos..pos.saturating_add(4))
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize)
                    .ok_or("a byte array's length is truncated")?;
                let end = (pos + 4)
                    .checked_add(len)
                    .ok_or("a byte array is truncated")?;
                let item = b.get(pos + 4..end).ok_or("a byte array is truncated")?;
                v.push(item.to_vec());
                pos = end;
            }
            if pos != b.len() {
                return Err(format!(
                    "{} bytes after the page's byte arrays",
                    b.len() - pos
                ));
            }
        }
    }
    Ok(())
}

struct Chunk<'a> {
    bytes: &'a [u8],
    gzip: bool,
    optional: bool,
    decoded: usize,
}

impl Chunk<'_> {
    fn inflate(&mut self, body: &[u8], size: usize) -> Result<Vec<u8>, String> {
        self.decoded = self.decoded.saturating_add(size);
        if self.decoded > MAX_DECODED_BYTES {
            return Err(format!("pages inflate past {MAX_DECODED_BYTES} bytes"));
        }
        if self.gzip {
            // `gunzip` inflates to exactly the trailer's size, at most `size`; the two must agree.
            let out = gunzip(body, size)?;
            if out.len() == size {
                Ok(out)
            } else {
                Err(format!(
                    "a page inflates to {} bytes, its header says {size}",
                    out.len()
                ))
            }
        } else if body.len() == size {
            Ok(body.to_vec())
        } else {
            Err(format!(
                "an uncompressed page of {} bytes whose header says {size}",
                body.len()
            ))
        }
    }

    /// One page at `pos`, holding at most `remaining` values: returns the position after it and its value count.
    fn page(
        &mut self,
        pos: usize,
        end: usize,
        remaining: usize,
        present: &mut Vec<bool>,
        values: &mut Values,
    ) -> Result<(usize, usize), String> {
        let mut t = Thrift {
            b: &self.bytes[..end],
            pos,
        };
        let h = t.strukt(0)?;
        let kind = int(&h, 1, "page type")?;
        let full = size(&h, 2, "uncompressed page size")?;
        let compressed = size(&h, 3, "compressed page size")?;
        if full > MAX_PAGE_BYTES || compressed > MAX_PAGE_BYTES {
            return Err(format!("a page larger than {MAX_PAGE_BYTES} bytes"));
        }
        let start = t.pos;
        let body = self
            .bytes
            .get(start..start + compressed)
            .filter(|_| start + compressed <= end)
            .ok_or("a page runs past its column chunk")?;
        // The i32 field's bits, sign-extended into the i64 it was read as.
        if let Some(crc) = opt_int(&h, 4, "page CRC")?
            && crc32(body) != crc as u32
        {
            return Err("a page's CRC does not match".to_owned());
        }
        let too_many = |n: usize| {
            if n > remaining {
                Err(format!(
                    "a page of {n} values where its chunk has {remaining} left"
                ))
            } else {
                Ok(())
            }
        };
        let (n, nulls, levels_bytes, value_bytes) = match kind {
            0 => {
                let d =
                    opt_struct(&h, 5, "data page header")?.ok_or("data page without its header")?;
                let n = size(d, 1, "page value count")?;
                too_many(n)?;
                if int(d, 2, "value encoding")? != 0 {
                    return Err("value encoding other than PLAIN".to_owned());
                }
                let data = self.inflate(body, full)?;
                if self.optional {
                    if int(d, 3, "definition level encoding")? != 3 {
                        return Err("definition levels not RLE-encoded".to_owned());
                    }
                    let len = data
                        .get(..4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as usize)
                        .ok_or("the definition levels' length is truncated")?;
                    let lv = data
                        .get(4..4 + len)
                        .ok_or("the definition levels are truncated")?
                        .to_vec();
                    (n, None, lv, data[4 + len..].to_vec())
                } else {
                    (n, None, Vec::new(), data)
                }
            }
            3 => {
                let d = opt_struct(&h, 8, "data page v2 header")?
                    .ok_or("data page v2 without its header")?;
                let n = size(d, 1, "page value count")?;
                too_many(n)?;
                let nulls = size(d, 2, "page null count")?;
                let rows = size(d, 3, "page row count")?;
                if int(d, 4, "value encoding")? != 0 {
                    return Err("value encoding other than PLAIN".to_owned());
                }
                let def = size(d, 5, "definition levels' length")?;
                if size(d, 6, "repetition levels' length")? != 0 {
                    return Err("repetition levels in a column without repetition".to_owned());
                }
                if rows != n {
                    return Err(format!("a page of {n} values in {rows} rows"));
                }
                if !self.optional && def != 0 {
                    return Err("definition levels in a required column".to_owned());
                }
                let lv = body
                    .get(..def)
                    .ok_or("the definition levels are truncated")?;
                let rest = &body[def..];
                let rest_full = full
                    .checked_sub(def)
                    .ok_or("a page smaller than its definition levels")?;
                let compressed_values = opt_bool(d, 7, "compression flag")?.unwrap_or(true);
                let data = if compressed_values {
                    self.inflate(rest, rest_full)?
                } else if rest.len() == rest_full {
                    rest.to_vec()
                } else {
                    return Err("an uncompressed page whose sizes differ".to_owned());
                };
                (n, Some(nulls), lv.to_vec(), data)
            }
            2 => return Err("dictionary pages are outside the subset".to_owned()),
            k => return Err(format!("page type {k} is outside the subset")),
        };
        let count = if self.optional {
            let p = levels(&levels_bytes, n)?;
            let c = p.iter().filter(|x| **x).count();
            if nulls.is_some_and(|z| z != n - c) {
                return Err(format!(
                    "a page declares {} nulls and its levels hold {}",
                    nulls.unwrap_or(0),
                    n - c
                ));
            }
            present.extend(p);
            c
        } else {
            if nulls.is_some_and(|z| z != 0) {
                return Err("nulls in a required column".to_owned());
            }
            n
        };
        plain(&value_bytes, count, values)?;
        Ok((start + compressed, n))
    }
}

/// Reads a whole Parquet file in the subset.
pub(crate) fn read(bytes: &[u8]) -> Result<Table, String> {
    if bytes.len() < 12 || &bytes[..4] != MAGIC || &bytes[bytes.len() - 4..] != MAGIC {
        return Err("not a Parquet file (PAR1 magic)".to_owned());
    }
    let n = bytes.len();
    let footer_len =
        u32::from_le_bytes([bytes[n - 8], bytes[n - 7], bytes[n - 6], bytes[n - 5]]) as usize;
    let footer_start = (n - 8)
        .checked_sub(footer_len)
        .filter(|s| *s >= 4)
        .ok_or("the Parquet footer is longer than the file")?;
    let mut t = Thrift {
        b: &bytes[footer_start..n - 8],
        pos: 0,
    };
    let meta = t.strukt(0)?;
    if t.pos != footer_len {
        return Err("bytes after the Parquet footer's metadata".to_owned());
    }
    if get(&meta, 8).is_some() {
        return Err("encrypted Parquet files are outside the subset".to_owned());
    }
    let leaves = leaves(list(&meta, 2, "schema")?)?;
    let rows = size(&meta, 3, "row count")?;
    if rows > MAX_ROWS {
        return Err(format!("{rows} rows, more than the {MAX_ROWS} allowed"));
    }
    let mut columns: Vec<Column> = leaves
        .iter()
        .map(|leaf| Column {
            leaf: leaf.clone(),
            present: leaf.optional.then(Vec::new),
            values: Values::empty(leaf.physical),
        })
        .collect();
    let mut decoded = 0_usize;
    let mut rows_seen = 0_usize;
    for rg in list(&meta, 4, "row groups")? {
        let rg = as_struct(rg, "row group")?;
        let rg_rows = size(rg, 3, "row group row count")?;
        rows_seen = rows_seen
            .checked_add(rg_rows)
            .ok_or("row count overflows")?;
        let chunks = list(rg, 1, "column chunks")?;
        if chunks.len() != columns.len() {
            return Err(format!(
                "a row group has {} column chunks for {} columns",
                chunks.len(),
                columns.len()
            ));
        }
        for (chunk, column) in chunks.iter().zip(columns.iter_mut()) {
            let chunk = as_struct(chunk, "column chunk")?;
            let name = column.leaf.path.join(".");
            if get(chunk, 1).is_some() {
                return Err(format!("column {name}: data in another file"));
            }
            if get(chunk, 8).is_some() || get(chunk, 9).is_some() {
                return Err(format!("column {name}: encrypted column data"));
            }
            let m = opt_struct(chunk, 3, "column metadata")?
                .ok_or_else(|| format!("column {name}: no column metadata"))?;
            if physical(int(m, 1, "physical type")?)? != column.leaf.physical {
                return Err(format!("column {name}: chunk type differs from the schema"));
            }
            let path = list(m, 3, "column path")?
                .iter()
                .map(|p| match p {
                    T::Bin(b) => text(b, "column path"),
                    _ => Err("column path is not binary".to_owned()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if path != column.leaf.path {
                return Err(format!(
                    "column {name}: chunk path {path:?} differs from the schema"
                ));
            }
            let gzip = match int(m, 4, "codec")? {
                0 => false,
                2 => true,
                c => return Err(format!("column {name}: codec {c} is outside the subset")),
            };
            if get(m, 11).is_some() {
                return Err(format!(
                    "column {name}: dictionary pages are outside the subset"
                ));
            }
            let levels_due = size(m, 5, "chunk value count")?;
            if levels_due != rg_rows {
                return Err(format!(
                    "column {name}: {levels_due} values in a row group of {rg_rows} rows"
                ));
            }
            let start = size(m, 9, "data page offset")?;
            let end = start
                .checked_add(size(m, 7, "chunk compressed size")?)
                .filter(|e| start >= 4 && *e <= footer_start)
                .ok_or_else(|| format!("column {name}: chunk lies outside the file's data"))?;
            let mut c = Chunk {
                bytes,
                gzip,
                optional: column.leaf.optional,
                decoded,
            };
            // Pages until the chunk's bytes are used; each takes at least its header's byte.
            let mut pos = start;
            let mut levels_read = 0_usize;
            let mut present = Vec::new();
            while pos < end {
                let (next, n) = c
                    .page(
                        pos,
                        end,
                        levels_due - levels_read,
                        &mut present,
                        &mut column.values,
                    )
                    .map_err(|e| format!("column {name}: {e}"))?;
                levels_read += n;
                pos = next;
            }
            decoded = c.decoded;
            if levels_read != levels_due {
                return Err(format!(
                    "column {name}: pages hold {levels_read} values; the chunk declares {levels_due}"
                ));
            }
            if let Some(p) = column.present.as_mut() {
                p.extend(present);
            }
        }
    }
    if rows_seen != rows {
        return Err(format!(
            "row groups hold {rows_seen} rows; the file declares {rows}"
        ));
    }
    for c in &columns {
        let present = c
            .present
            .as_ref()
            .map_or(rows, |p| p.iter().filter(|x| **x).count());
        if c.values.len() != present {
            return Err(format!(
                "column {}: {} values for {present} present rows",
                c.leaf.path.join("."),
                c.values.len()
            ));
        }
    }
    Ok(Table { rows, columns })
}

/// A Parquet writer for the subset, for tests only: it writes what `read` accepts, and the files the refusal tests
/// then damage. Values pages are stored raw, or as gzip members holding stored DEFLATE blocks.
#[cfg(test)]
pub(crate) mod testkit {
    use super::*;

    pub(crate) enum W {
        Bool(bool),
        I8(i8),
        I32(i32),
        I64(i64),
        Bin(Vec<u8>),
        List(u8, Vec<W>),
        Struct(Vec<(i16, W)>),
    }

    fn varint(out: &mut Vec<u8>, mut v: u64) {
        loop {
            let b = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                return;
            }
            out.push(b | 0x80);
        }
    }

    fn zigzag(out: &mut Vec<u8>, v: i64) {
        varint(out, ((v << 1) ^ (v >> 63)) as u64);
    }

    impl W {
        fn kind(&self) -> u8 {
            match self {
                W::Bool(true) => 1,
                W::Bool(false) => 2,
                W::I8(_) => 3,
                W::I32(_) => 5,
                W::I64(_) => 6,
                W::Bin(_) => 8,
                W::List(..) => 9,
                W::Struct(_) => 12,
            }
        }

        pub(crate) fn ser(&self, out: &mut Vec<u8>) {
            match self {
                W::Bool(b) => out.push(u8::from(*b)),
                W::I8(v) => out.push(*v as u8),
                W::I32(v) => zigzag(out, i64::from(*v)),
                W::I64(v) => zigzag(out, *v),
                W::Bin(b) => {
                    varint(out, b.len() as u64);
                    out.extend_from_slice(b);
                }
                W::List(elem, items) => {
                    if items.len() < 15 {
                        out.push(((items.len() as u8) << 4) | elem);
                    } else {
                        out.push(0xF0 | elem);
                        varint(out, items.len() as u64);
                    }
                    for i in items {
                        i.ser(out);
                    }
                }
                W::Struct(fields) => {
                    let mut last = 0_i16;
                    for (id, v) in fields {
                        let delta = id - last;
                        if (1..=15).contains(&delta) {
                            out.push(((delta as u8) << 4) | v.kind());
                        } else {
                            out.push(v.kind());
                            zigzag(out, i64::from(*id));
                        }
                        if !matches!(v, W::Bool(_)) {
                            v.ser(out);
                        }
                        last = *id;
                    }
                    out.push(0);
                }
            }
        }
    }

    /// A gzip member whose DEFLATE stream is stored blocks.
    pub(crate) fn gzip_stored(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff];
        let blocks: Vec<&[u8]> = if data.is_empty() {
            vec![&[]]
        } else {
            data.chunks(65_535).collect()
        };
        for (i, b) in blocks.iter().enumerate() {
            out.push(u8::from(i + 1 == blocks.len()));
            out.extend_from_slice(&(b.len() as u16).to_le_bytes());
            out.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
            out.extend_from_slice(b);
        }
        out.extend_from_slice(&crc32(data).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out
    }

    /// One column to write: `group` names an enclosing group and whether it is optional.
    pub(crate) struct Col {
        pub name: &'static str,
        pub group: Option<(&'static str, bool)>,
        pub physical: Physical,
        pub logical: Logical,
        pub optional: bool,
        pub present: Option<Vec<bool>>,
        pub values: Values,
    }

    #[derive(Clone, Copy)]
    pub(crate) struct Opts {
        pub v2: bool,
        pub gzip: bool,
        pub rows_per_page: usize,
        pub rows_per_group: usize,
        pub bitpacked_levels: bool,
        pub crc: bool,
    }

    impl Default for Opts {
        fn default() -> Self {
            Self {
                v2: true,
                gzip: true,
                rows_per_page: usize::MAX,
                rows_per_group: usize::MAX,
                bitpacked_levels: false,
                crc: false,
            }
        }
    }

    fn physical_code(p: Physical) -> i32 {
        match p {
            Physical::Boolean => 0,
            Physical::Int32 => 1,
            Physical::Int64 => 2,
            Physical::Float => 4,
            Physical::Double => 5,
            Physical::ByteArray => 6,
        }
    }

    fn logical_fields(l: Logical) -> Vec<(i16, W)> {
        let union =
            |id: i16, inner: Vec<(i16, W)>| vec![(10, W::Struct(vec![(id, W::Struct(inner))]))];
        match l {
            Logical::None => vec![],
            Logical::String => union(1, vec![]),
            Logical::Date => union(6, vec![]),
            Logical::TimestampMicrosUtc => union(
                8,
                vec![
                    (1, W::Bool(true)),
                    (2, W::Struct(vec![(2, W::Struct(vec![]))])),
                ],
            ),
            Logical::Integer { bits, signed } => {
                union(10, vec![(1, W::I8(bits as i8)), (2, W::Bool(signed))])
            }
        }
    }

    fn levels_rle(p: &[bool], bitpacked: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if bitpacked {
            let groups = p.len().div_ceil(8);
            varint(&mut out, ((groups as u64) << 1) | 1);
            for g in p.chunks(8) {
                out.push(
                    g.iter()
                        .enumerate()
                        .fold(0, |b, (i, x)| b | (u8::from(*x) << i)),
                );
            }
            return out;
        }
        let mut i = 0;
        while i < p.len() {
            let j = (i..p.len()).find(|&j| p[j] != p[i]).unwrap_or(p.len());
            varint(&mut out, ((j - i) as u64) << 1);
            out.push(u8::from(p[i]));
            i = j;
        }
        out
    }

    fn plain_bytes(v: &Values, range: std::ops::Range<usize>) -> Vec<u8> {
        let mut out = Vec::new();
        match v {
            Values::Boolean(x) => {
                let x = &x[range];
                for g in x.chunks(8) {
                    out.push(
                        g.iter()
                            .enumerate()
                            .fold(0, |b, (i, x)| b | (u8::from(*x) << i)),
                    );
                }
            }
            Values::Int32(x) => x[range]
                .iter()
                .for_each(|v| out.extend_from_slice(&v.to_le_bytes())),
            Values::Int64(x) => x[range]
                .iter()
                .for_each(|v| out.extend_from_slice(&v.to_le_bytes())),
            Values::Float(x) => x[range]
                .iter()
                .for_each(|v| out.extend_from_slice(&v.to_le_bytes())),
            Values::Double(x) => x[range]
                .iter()
                .for_each(|v| out.extend_from_slice(&v.to_le_bytes())),
            Values::Bytes(x) => x[range].iter().for_each(|v| {
                out.extend_from_slice(&(v.len() as u32).to_le_bytes());
                out.extend_from_slice(v);
            }),
        }
        out
    }

    /// A Parquet file of `rows` rows holding `cols`.
    pub(crate) fn write(cols: &[Col], rows: usize, o: Opts) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        let mut row_groups = Vec::new();
        // Each column's next value index, across pages and groups.
        let mut next_value = vec![0_usize; cols.len()];
        let mut g0 = 0;
        while g0 < rows || (rows == 0 && g0 == 0) {
            let g1 = rows.min(g0.saturating_add(o.rows_per_group));
            let mut chunks = Vec::new();
            for (ci, c) in cols.iter().enumerate() {
                let chunk_start = out.len();
                let mut p0 = g0;
                loop {
                    let p1 = g1.min(p0.saturating_add(o.rows_per_page));
                    let pres: Vec<bool> = match &c.present {
                        Some(p) => p[p0..p1].to_vec(),
                        None => vec![true; p1 - p0],
                    };
                    let k = pres.iter().filter(|x| **x).count();
                    let vals = plain_bytes(&c.values, next_value[ci]..next_value[ci] + k);
                    next_value[ci] += k;
                    let lv = if c.present.is_some() {
                        levels_rle(&pres, o.bitpacked_levels)
                    } else {
                        Vec::new()
                    };
                    let n = (p1 - p0) as i32;
                    let (header_fields, body) = if o.v2 {
                        let packed = if o.gzip {
                            gzip_stored(&vals)
                        } else {
                            vals.clone()
                        };
                        let mut body = lv.clone();
                        body.extend_from_slice(&packed);
                        (
                            vec![
                                (1, W::I32(3)),
                                (2, W::I32((lv.len() + vals.len()) as i32)),
                                (3, W::I32(body.len() as i32)),
                                (
                                    8,
                                    W::Struct(vec![
                                        (1, W::I32(n)),
                                        (2, W::I32((p1 - p0 - k) as i32)),
                                        (3, W::I32(n)),
                                        (4, W::I32(0)),
                                        (5, W::I32(lv.len() as i32)),
                                        (6, W::I32(0)),
                                        (7, W::Bool(o.gzip)),
                                    ]),
                                ),
                            ],
                            body,
                        )
                    } else {
                        let mut data = Vec::new();
                        if c.present.is_some() {
                            data.extend_from_slice(&(lv.len() as u32).to_le_bytes());
                            data.extend_from_slice(&lv);
                        }
                        data.extend_from_slice(&vals);
                        let body = if o.gzip {
                            gzip_stored(&data)
                        } else {
                            data.clone()
                        };
                        (
                            vec![
                                (1, W::I32(0)),
                                (2, W::I32(data.len() as i32)),
                                (3, W::I32(body.len() as i32)),
                                (
                                    5,
                                    W::Struct(vec![
                                        (1, W::I32(n)),
                                        (2, W::I32(0)),
                                        (3, W::I32(3)),
                                        (4, W::I32(3)),
                                    ]),
                                ),
                            ],
                            body,
                        )
                    };
                    let mut header_fields = header_fields;
                    if o.crc {
                        header_fields.insert(3, (4, W::I32(crc32(&body) as i32)));
                    }
                    W::Struct(header_fields).ser(&mut out);
                    out.extend_from_slice(&body);
                    p0 = p1;
                    if p0 >= g1 {
                        break;
                    }
                }
                let mut path = Vec::new();
                if let Some((g, _)) = c.group {
                    path.push(W::Bin(g.as_bytes().to_vec()));
                }
                path.push(W::Bin(c.name.as_bytes().to_vec()));
                chunks.push(W::Struct(vec![
                    (2, W::I64(chunk_start as i64)),
                    (
                        3,
                        W::Struct(vec![
                            (1, W::I32(physical_code(c.physical))),
                            (2, W::List(5, vec![W::I32(0), W::I32(3)])),
                            (3, W::List(8, path)),
                            (4, W::I32(if o.gzip { 2 } else { 0 })),
                            (5, W::I64((g1 - g0) as i64)),
                            (6, W::I64((out.len() - chunk_start) as i64)),
                            (7, W::I64((out.len() - chunk_start) as i64)),
                            (9, W::I64(chunk_start as i64)),
                        ]),
                    ),
                ]));
            }
            row_groups.push(W::Struct(vec![
                (1, W::List(12, chunks)),
                (2, W::I64(0)),
                (3, W::I64((g1 - g0) as i64)),
            ]));
            if g1 >= rows {
                break;
            }
            g0 = g1;
        }
        // The schema: root, then each column or group in order.
        let mut schema = Vec::new();
        let mut top: Vec<W> = Vec::new();
        let mut i = 0;
        let mut root_children = 0;
        while i < cols.len() {
            root_children += 1;
            let leaf = |c: &Col| {
                let mut f = vec![
                    (1, W::I32(physical_code(c.physical))),
                    (3, W::I32(i32::from(c.optional))),
                    (4, W::Bin(c.name.as_bytes().to_vec())),
                ];
                f.extend(logical_fields(c.logical));
                W::Struct(f)
            };
            match cols[i].group {
                Some((g, opt)) => {
                    let members: Vec<&Col> = cols[i..]
                        .iter()
                        .take_while(|c| c.group.map(|x| x.0) == Some(g))
                        .collect();
                    top.push(W::Struct(vec![
                        (3, W::I32(i32::from(opt))),
                        (4, W::Bin(g.as_bytes().to_vec())),
                        (5, W::I32(members.len() as i32)),
                    ]));
                    for c in &members {
                        top.push(leaf(c));
                    }
                    i += members.len();
                }
                None => {
                    top.push(leaf(&cols[i]));
                    i += 1;
                }
            }
        }
        schema.push(W::Struct(vec![
            (4, W::Bin(b"schema".to_vec())),
            (5, W::I32(root_children)),
        ]));
        schema.extend(top);
        let meta = W::Struct(vec![
            (1, W::I32(2)),
            (2, W::List(12, schema)),
            (3, W::I64(rows as i64)),
            (4, W::List(12, row_groups)),
            (6, W::Bin(b"fmine-io testkit".to_vec())),
        ]);
        let footer_start = out.len();
        meta.ser(&mut out);
        let footer_len = (out.len() - footer_start) as u32;
        out.extend_from_slice(&footer_len.to_le_bytes());
        out.extend_from_slice(MAGIC);
        out
    }

    pub(crate) fn required(
        name: &'static str,
        physical: Physical,
        logical: Logical,
        values: Values,
    ) -> Col {
        Col {
            name,
            group: None,
            physical,
            logical,
            optional: false,
            present: None,
            values,
        }
    }

    pub(crate) fn nullable(
        name: &'static str,
        physical: Physical,
        logical: Logical,
        present: Vec<bool>,
        values: Values,
    ) -> Col {
        Col {
            name,
            group: None,
            physical,
            logical,
            optional: true,
            present: Some(present),
            values,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::*;
    use super::*;

    fn sample() -> Vec<Col> {
        let present = vec![
            true, false, true, true, false, true, true, true, false, true, true,
        ];
        vec![
            required(
                "id",
                Physical::Int32,
                Logical::Integer {
                    bits: 32,
                    signed: false,
                },
                Values::Int32((0..11).collect()),
            ),
            nullable(
                "grade",
                Physical::Double,
                Logical::None,
                present.clone(),
                Values::Double(vec![0.5, 1.25, -2.0, 3.0, 1e-300, f64::MAX, 7.0, 8.5]),
            ),
            nullable(
                "name",
                Physical::ByteArray,
                Logical::String,
                present.clone(),
                Values::Bytes(
                    ["a", "", "bc", "déf", "g", "h", "i", "j"]
                        .map(|s| s.as_bytes().to_vec())
                        .to_vec(),
                ),
            ),
            nullable(
                "flag",
                Physical::Boolean,
                Logical::None,
                present,
                Values::Boolean(vec![true, false, true, true, false, false, true, true]),
            ),
            required(
                "t",
                Physical::Int64,
                Logical::TimestampMicrosUtc,
                Values::Int64((0..11).map(|i| i * 3_600_000_000 - 5).collect()),
            ),
            required(
                "f",
                Physical::Float,
                Logical::None,
                Values::Float((0..11).map(|i| i as f32 / 3.0).collect()),
            ),
        ]
    }

    fn check(cols: &[Col], rows: usize, o: Opts) {
        let t = read(&write(cols, rows, o)).unwrap();
        assert_eq!(t.rows, rows);
        assert_eq!(t.columns.len(), cols.len());
        for (got, want) in t.columns.iter().zip(cols) {
            assert_eq!(got.leaf.path.last().map(String::as_str), Some(want.name));
            assert_eq!(got.leaf.physical, want.physical);
            assert_eq!(got.leaf.logical, want.logical);
            assert_eq!(got.present, want.present);
            assert_eq!(got.values, want.values);
        }
    }

    #[test]
    fn every_page_form_reads_back() {
        let cols = sample();
        for v2 in [true, false] {
            for gzip in [true, false] {
                for (rows_per_page, rows_per_group) in
                    [(usize::MAX, usize::MAX), (3, usize::MAX), (2, 5), (1, 1)]
                {
                    for bitpacked_levels in [false, true] {
                        check(
                            &cols,
                            11,
                            Opts {
                                v2,
                                gzip,
                                rows_per_page,
                                rows_per_group,
                                bitpacked_levels,
                                crc: rows_per_page == 3,
                            },
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn an_optional_group_and_an_empty_file_read_back() {
        let member = |name, values| Col {
            name,
            group: Some(("vector", true)),
            physical: Physical::Double,
            logical: Logical::None,
            optional: false,
            present: Some(vec![true, false, true]),
            values: Values::Double(values),
        };
        let bytes = write(
            &[member("x", vec![1.0, 3.0]), member("y", vec![2.0, 4.0])],
            3,
            Opts::default(),
        );
        let t = read(&bytes).unwrap();
        assert_eq!(t.columns[0].leaf.path, ["vector", "x"]);
        assert!(t.columns[1].leaf.optional);
        assert_eq!(t.columns[1].values, Values::Double(vec![2.0, 4.0]));
        assert_eq!(t.columns[1].present, Some(vec![true, false, true]));
        // An empty file: its schema, one row group of no rows, an empty page.
        let empty = read(&write(
            &[required(
                "a",
                Physical::Int64,
                Logical::None,
                Values::Int64(vec![]),
            )],
            0,
            Opts::default(),
        ))
        .unwrap();
        assert_eq!((empty.rows, empty.columns[0].values.len()), (0, 0));
    }

    /// Converted types stand in when a writer gives no logical type.
    #[test]
    fn converted_types_map_to_the_subset() {
        let conv = |c: i64| logical(&[(6, T::Int(c))], "c");
        assert_eq!(conv(0), Ok(Logical::String));
        assert_eq!(conv(6), Ok(Logical::Date));
        assert_eq!(conv(10), Ok(Logical::TimestampMicrosUtc));
        assert_eq!(
            conv(11),
            Ok(Logical::Integer {
                bits: 8,
                signed: false
            })
        );
        assert_eq!(
            conv(13),
            Ok(Logical::Integer {
                bits: 32,
                signed: false
            })
        );
        assert_eq!(
            conv(14),
            Ok(Logical::Integer {
                bits: 64,
                signed: false
            })
        );
        assert_eq!(
            conv(15),
            Ok(Logical::Integer {
                bits: 8,
                signed: true
            })
        );
        assert_eq!(
            conv(18),
            Ok(Logical::Integer {
                bits: 64,
                signed: true
            })
        );
        assert!(conv(5).unwrap_err().contains("converted type 5"));
        assert_eq!(logical(&[], "c"), Ok(Logical::None));
        // A timestamp in milliseconds, or not adjusted to UTC.
        let ts = |utc: bool, unit: i16| {
            logical(
                &[(
                    10,
                    T::Struct(vec![(
                        8,
                        T::Struct(vec![
                            (1, T::Bool(utc)),
                            (2, T::Struct(vec![(unit, T::Struct(vec![]))])),
                        ]),
                    )]),
                )],
                "t",
            )
        };
        assert_eq!(ts(true, 2), Ok(Logical::TimestampMicrosUtc));
        assert!(ts(true, 1).unwrap_err().contains("microseconds"));
        assert!(ts(false, 2).unwrap_err().contains("UTC"));
    }

    #[test]
    fn levels_decode_runs_and_packed_groups() {
        // RLE run of 3 ones, then a bit-packed group of 8 holding 0b0000_0101 (only 2 used).
        assert_eq!(
            levels(&[6, 1, 3, 0b101], 5).unwrap(),
            vec![true, true, true, true, false]
        );
        assert!(levels(&[6, 2], 3).unwrap_err().contains("level 2"));
        assert!(levels(&[8, 1], 3).unwrap_err().contains("run of 4"));
        assert!(
            levels(&[6, 1, 0], 3)
                .unwrap_err()
                .contains("after the definition levels")
        );
        assert!(levels(&[6], 3).is_err());
    }

    /// Damage one thing in a good file and check the refusal names it.
    #[test]
    fn files_outside_the_subset_are_refused() {
        let good = write(&sample(), 11, Opts::default());
        let refused = |bytes: &[u8], why: &str| {
            let e = read(bytes).unwrap_err();
            assert!(e.contains(why), "{why}: {e}");
        };
        refused(&good[..good.len() - 1], "magic");
        let mut b = good.clone();
        b[0] = b'Q';
        refused(&b, "magic");
        let mut b = good.clone();
        let n = b.len();
        b[n - 8..n - 4].copy_from_slice(&u32::MAX.to_le_bytes());
        refused(&b, "footer is longer");

        // A page's CRC.
        let crc = write(
            &sample(),
            11,
            Opts {
                crc: true,
                ..Opts::default()
            },
        );
        assert!(read(&crc).is_ok());
        let mut b = crc.clone();
        let at = b.windows(4).position(|w| w == [0x1f, 0x8b, 8, 0]).unwrap() + 12;
        b[at] ^= 0x40;
        refused(&b, "CRC does not match");

        // The schema: a repeated column, two optional levels, a deeper group, an unknown logical type.
        let mut c = sample();
        c[0].optional = true;
        c[0].present = Some(vec![true; 11]);
        let rep = write(&c, 11, Opts::default());
        let pos = rep
            .windows(6)
            .position(|w| w == [0x15, 0x02, 0x25, 0x02, 0x18, 0x02])
            .unwrap();
        let mut b = rep.clone();
        b[pos + 3] = 0x04;
        refused(&b, "is repeated");
        let x = Col {
            name: "x",
            group: Some(("v", true)),
            physical: Physical::Double,
            logical: Logical::None,
            optional: true,
            present: Some(vec![true]),
            values: Values::Double(vec![1.0]),
        };
        refused(
            &write(&[x], 1, Opts::default()),
            "definition levels above 1",
        );
        let text = write(
            &[required(
                "s",
                Physical::ByteArray,
                Logical::String,
                Values::Bytes(vec![b"a".to_vec()]),
            )],
            1,
            Opts::default(),
        );
        // The logical type (field 10, six after the name: 0x6c) holds STRING (member 1, empty: 0x1c 0x00); it becomes
        // JSON (member 12), then DATE (member 6).
        let pos = text
            .windows(3)
            .position(|w| w == [0x6c, 0x1c, 0x00])
            .unwrap();
        let mut b = text.clone();
        b[pos + 1] = 0xcc;
        refused(&b, "logical type 12 is outside the subset");
        let mut b = text.clone();
        b[pos + 1] = 0x6c;
        refused(&b, "on physical ByteArray");

        // The data: a codec, a dictionary page, a value count, a page encoding.
        let one = write(
            &[required(
                "a",
                Physical::Int32,
                Logical::None,
                Values::Int32(vec![7, 8, 9]),
            )],
            3,
            Opts {
                gzip: false,
                ..Opts::default()
            },
        );
        assert_eq!(
            read(&one).unwrap().columns[0].values,
            Values::Int32(vec![7, 8, 9])
        );
        // The codec follows the column path ["a"] in the chunk metadata: 0x19 0x18 0x01 'a', then 0x15 and its value.
        let codec = one
            .windows(5)
            .position(|w| w == [0x19, 0x18, 0x01, b'a', 0x15])
            .unwrap()
            + 5;
        let mut b = one.clone();
        b[codec] = 0x06;
        refused(&b, "codec 3 is outside the subset");
        // The page type, the page header's first field at byte 5.
        assert_eq!(one[4..6], [0x15, 0x06]);
        let mut b = one.clone();
        b[5] = 0x04;
        refused(&b, "dictionary pages");
        let mut b = one.clone();
        b[5] = 0x08;
        refused(&b, "page type 4");
        // The v2 header (page header field 8, five after field 3: 0x5c) begins with the value count, then nulls,
        // rows and the value encoding.
        let v2 = one
            .windows(3)
            .position(|w| w == [0x5c, 0x15, 0x06])
            .unwrap();
        assert_eq!(
            one[v2 + 1..v2 + 9],
            [0x15, 0x06, 0x15, 0x00, 0x15, 0x06, 0x15, 0x00]
        );
        let mut b = one.clone();
        b[v2 + 2] = 0x04;
        refused(&b, "a page of 2 values in 3 rows");
        let mut b = one.clone();
        b[v2 + 2] = 0x08;
        refused(&b, "a page of 4 values where its chunk has 3 left");
        let mut b = one.clone();
        b[v2 + 4] = 0x02;
        refused(&b, "nulls in a required column");
        let mut b = one.clone();
        b[v2 + 8] = 0x10;
        refused(&b, "value encoding other than PLAIN");
        // The uncompressed size, the header's second field, from 12 bytes to 11.
        assert_eq!(one[6..8], [0x15, 0x18]);
        let mut b = one.clone();
        b[7] = 0x16;
        refused(&b, "sizes differ");
    }
}
