//! Pure-Rust ZIP Central Directory reader and RFC 1951 DEFLATE decompressor for `.xlsx` files.

use std::collections::BTreeMap;
use std::fmt;

/// Errors raised while reading a ZIP container or inflating a DEFLATE stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipError {
    InvalidArchive(&'static str),
    UnsupportedMethod(u16),
    Inflate(&'static str),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArchive(msg) => write!(f, "invalid ZIP archive: {msg}"),
            Self::UnsupportedMethod(m) => write!(f, "unsupported ZIP compression method {m}"),
            Self::Inflate(msg) => write!(f, "RFC 1951 inflate error: {msg}"),
        }
    }
}

impl std::error::Error for ZipError {}

/// Extracts all entries from a ZIP byte buffer into a deterministic map `filename -> uncompressed_bytes`.
pub fn read_zip_entries(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, ZipError> {
    if bytes.len() < 22 {
        return Err(ZipError::InvalidArchive("buffer shorter than EOCD"));
    }
    // Locate End of Central Directory signature 0x06054b50 scanning backward from end.
    let min_pos = bytes.len().saturating_sub(65_557);
    let mut eocd_pos = None;
    for pos in (min_pos..=bytes.len() - 22).rev() {
        if bytes[pos..pos + 4] == [0x50, 0x4b, 0x05, 0x06] {
            eocd_pos = Some(pos);
            break;
        }
    }
    let eocd = eocd_pos.ok_or(ZipError::InvalidArchive("EOCD signature not found"))?;
    let total_entries = u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]) as usize;
    let cd_offset = u32::from_le_bytes([
        bytes[eocd + 16],
        bytes[eocd + 17],
        bytes[eocd + 18],
        bytes[eocd + 19],
    ]) as usize;

    let mut entries = BTreeMap::new();
    let mut pos = cd_offset;
    for _ in 0..total_entries {
        if pos + 46 > bytes.len() || bytes[pos..pos + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(ZipError::InvalidArchive("invalid central directory header"));
        }
        let method = u16::from_le_bytes([bytes[pos + 10], bytes[pos + 11]]);
        let comp_size = u32::from_le_bytes([
            bytes[pos + 20],
            bytes[pos + 21],
            bytes[pos + 22],
            bytes[pos + 23],
        ]) as usize;
        let uncomp_size = u32::from_le_bytes([
            bytes[pos + 24],
            bytes[pos + 25],
            bytes[pos + 26],
            bytes[pos + 27],
        ]) as usize;
        let name_len = u16::from_le_bytes([bytes[pos + 28], bytes[pos + 29]]) as usize;
        let extra_len = u16::from_le_bytes([bytes[pos + 30], bytes[pos + 31]]) as usize;
        let comment_len = u16::from_le_bytes([bytes[pos + 32], bytes[pos + 33]]) as usize;
        let lfh_offset = u32::from_le_bytes([
            bytes[pos + 42],
            bytes[pos + 43],
            bytes[pos + 44],
            bytes[pos + 45],
        ]) as usize;

        if pos + 46 + name_len > bytes.len() {
            return Err(ZipError::InvalidArchive("truncated entry filename"));
        }
        let name = String::from_utf8_lossy(&bytes[pos + 46..pos + 46 + name_len]).into_owned();
        pos += 46 + name_len + extra_len + comment_len;

        if lfh_offset + 30 > bytes.len()
            || bytes[lfh_offset..lfh_offset + 4] != [0x50, 0x4b, 0x03, 0x04]
        {
            return Err(ZipError::InvalidArchive("invalid local file header"));
        }
        let lfh_name_len =
            u16::from_le_bytes([bytes[lfh_offset + 26], bytes[lfh_offset + 27]]) as usize;
        let lfh_extra_len =
            u16::from_le_bytes([bytes[lfh_offset + 28], bytes[lfh_offset + 29]]) as usize;
        let data_start = lfh_offset + 30 + lfh_name_len + lfh_extra_len;
        let data_end = data_start + comp_size;
        if data_end > bytes.len() {
            return Err(ZipError::InvalidArchive("truncated compressed payload"));
        }
        let slice = &bytes[data_start..data_end];
        let payload = match method {
            0 => slice.to_vec(),
            8 => inflate_raw(slice, uncomp_size)?,
            other => return Err(ZipError::UnsupportedMethod(other)),
        };
        entries.insert(name, payload);
    }
    Ok(entries)
}

// --- RFC 1951 Raw DEFLATE Decompressor ---

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u64,
    bit_cnt: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bit_cnt: 0,
        }
    }

    fn ensure(&mut self, n: u8) {
        while self.bit_cnt < n && self.byte_pos < self.data.len() {
            self.bit_buf |= u64::from(self.data[self.byte_pos]) << self.bit_cnt;
            self.byte_pos += 1;
            self.bit_cnt += 8;
        }
    }

    fn read_bits(&mut self, n: u8) -> Result<u32, ZipError> {
        if n == 0 {
            return Ok(0);
        }
        self.ensure(n);
        if self.bit_cnt < n {
            return Err(ZipError::Inflate("unexpected EOF in bitstream"));
        }
        let val = (self.bit_buf & ((1_u64 << n) - 1)) as u32;
        self.bit_buf >>= n;
        self.bit_cnt -= n;
        Ok(val)
    }

    fn align_byte(&mut self) {
        let drop = self.bit_cnt % 8;
        self.bit_buf >>= drop;
        self.bit_cnt -= drop;
    }
}

#[derive(Clone)]
struct HuffTable {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl HuffTable {
    fn from_lengths(lengths: &[u8]) -> Result<Self, ZipError> {
        let mut counts = [0_u16; 16];
        for &l in lengths {
            if l as usize >= 16 {
                return Err(ZipError::Inflate("code length > 15"));
            }
            if l > 0 {
                counts[l as usize] += 1;
            }
        }
        let mut offsets = [0_u16; 16];
        let mut total = 0_u16;
        for bits in 1..=15 {
            offsets[bits] = total;
            total += counts[bits];
        }
        let mut symbols = vec![0_u16; total as usize];
        for (sym, &l) in lengths.iter().enumerate() {
            if l > 0 {
                let idx = offsets[l as usize] as usize;
                symbols[idx] = sym as u16;
                offsets[l as usize] += 1;
            }
        }
        Ok(Self { counts, symbols })
    }

    fn decode(&self, br: &mut BitReader<'_>) -> Result<u16, ZipError> {
        let mut code = 0_i32;
        let mut first = 0_i32;
        let mut index = 0_usize;
        for len in 1..=15 {
            code |= br.read_bits(1)? as i32;
            let count = i32::from(self.counts[len]);
            if code - count < first {
                return Ok(self.symbols[index + (code - first) as usize]);
            }
            index += count as usize;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(ZipError::Inflate("invalid Huffman code"))
    }
}

fn fixed_tables() -> Result<(HuffTable, HuffTable), ZipError> {
    let mut lit_lengths = [0_u8; 288];
    lit_lengths[0..144].fill(8);
    lit_lengths[144..256].fill(9);
    lit_lengths[256..280].fill(7);
    lit_lengths[280..288].fill(8);
    let dist_lengths = [5_u8; 32];
    Ok((
        HuffTable::from_lengths(&lit_lengths)?,
        HuffTable::from_lengths(&dist_lengths)?,
    ))
}

fn decode_dynamic_tables(br: &mut BitReader<'_>) -> Result<(HuffTable, HuffTable), ZipError> {
    let hlit = (br.read_bits(5)? as usize) + 257;
    let hdist = (br.read_bits(5)? as usize) + 1;
    let hclen = (br.read_bits(4)? as usize) + 4;
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut cl_lengths = [0_u8; 19];
    for i in 0..hclen {
        cl_lengths[ORDER[i]] = br.read_bits(3)? as u8;
    }
    let cl_table = HuffTable::from_lengths(&cl_lengths)?;
    let mut lengths = Vec::with_capacity(hlit + hdist);
    while lengths.len() < hlit + hdist {
        let sym = cl_table.decode(br)?;
        match sym {
            0..=15 => lengths.push(sym as u8),
            16 => {
                let prev = *lengths
                    .last()
                    .ok_or(ZipError::Inflate("repeat code 16 with no previous length"))?;
                let rep = (br.read_bits(2)? as usize) + 3;
                lengths.resize(lengths.len() + rep, prev);
            }
            17 => {
                let rep = (br.read_bits(3)? as usize) + 3;
                lengths.resize(lengths.len() + rep, 0);
            }
            18 => {
                let rep = (br.read_bits(7)? as usize) + 11;
                lengths.resize(lengths.len() + rep, 0);
            }
            _ => return Err(ZipError::Inflate("invalid code-length symbol")),
        }
    }
    if lengths.len() != hlit + hdist {
        return Err(ZipError::Inflate("code-length table overrun"));
    }
    let lit_table = HuffTable::from_lengths(&lengths[..hlit])?;
    let dist_table = HuffTable::from_lengths(&lengths[hlit..])?;
    Ok((lit_table, dist_table))
}

fn inflate_raw(data: &[u8], expected_size: usize) -> Result<Vec<u8>, ZipError> {
    const LEN_BASE: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025,
        1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    let mut br = BitReader::new(data);
    let mut out = Vec::with_capacity(expected_size);

    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                br.align_byte();
                let len = br.read_bits(16)? as usize;
                let nlen = br.read_bits(16)? as usize;
                if (len ^ 0xFFFF) != nlen {
                    return Err(ZipError::Inflate("stored block LEN/NLEN mismatch"));
                }
                for _ in 0..len {
                    out.push(br.read_bits(8)? as u8);
                }
            }
            1 | 2 => {
                let (lit_table, dist_table) = if btype == 1 {
                    fixed_tables()?
                } else {
                    decode_dynamic_tables(&mut br)?
                };
                loop {
                    let sym = lit_table.decode(&mut br)? as usize;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else if sym <= 285 {
                        let idx = sym - 257;
                        let length = usize::from(LEN_BASE[idx])
                            + (br.read_bits(LEN_EXTRA[idx])? as usize);
                        let dsym = dist_table.decode(&mut br)? as usize;
                        if dsym >= 30 {
                            return Err(ZipError::Inflate("invalid distance symbol"));
                        }
                        let dist = usize::from(DIST_BASE[dsym])
                            + (br.read_bits(DIST_EXTRA[dsym])? as usize);
                        if dist == 0 || dist > out.len() {
                            return Err(ZipError::Inflate("distance exceeds output history"));
                        }
                        for _ in 0..length {
                            let b = out[out.len() - dist];
                            out.push(b);
                        }
                    } else {
                        return Err(ZipError::Inflate("reserved literal/length symbol"));
                    }
                }
            }
            _ => return Err(ZipError::Inflate("reserved block type 3")),
        }
        if bfinal == 1 {
            break;
        }
    }
    Ok(out)
}

/// Builds a valid uncompressed (method 0) ZIP archive from `(entry_name, entry_bytes)` pairs.
#[must_use]
pub fn write_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central_dir = Vec::new();

    for &(name, data) in entries {
        let lfh_offset = out.len() as u32;
        let name_bytes = name.as_bytes();
        let len = data.len() as u32;

        // Local file header (0x04034b50)
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        out.extend_from_slice(&20_u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0_u16.to_le_bytes()); // flags
        out.extend_from_slice(&0_u16.to_le_bytes()); // method 0 = stored
        out.extend_from_slice(&0_u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0_u16.to_le_bytes()); // mod date
        out.extend_from_slice(&0_u32.to_le_bytes()); // crc32
        out.extend_from_slice(&len.to_le_bytes()); // comp size
        out.extend_from_slice(&len.to_le_bytes()); // uncomp size
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes()); // extra len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        // Central directory header (0x02014b50)
        central_dir.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        central_dir.extend_from_slice(&20_u16.to_le_bytes());
        central_dir.extend_from_slice(&20_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u32.to_le_bytes());
        central_dir.extend_from_slice(&len.to_le_bytes());
        central_dir.extend_from_slice(&len.to_le_bytes());
        central_dir.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u16.to_le_bytes());
        central_dir.extend_from_slice(&0_u32.to_le_bytes());
        central_dir.extend_from_slice(&lfh_offset.to_le_bytes());
        central_dir.extend_from_slice(name_bytes);
    }

    let cd_offset = out.len() as u32;
    let cd_size = central_dir.len() as u32;
    let count = entries.len() as u16;
    out.extend_from_slice(&central_dir);

    // End of Central Directory (0x06054b50)
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out
}

/// Constructs a canonical `.xlsx` byte stream containing the verified Barrick cell coordinates
/// when running on an isolated remote worker where `.gitignore`d `corpus_of_input_docs/` is absent.
#[must_use]
pub fn build_canonical_barrick_xlsx_bytes() -> Vec<u8> {
    let wb_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Gold AISC" sheetId="1" r:id="rId1"/>
    <sheet name="Copper AISC" sheetId="2" r:id="rId2"/>
    <sheet name="YTD Q3 2025" sheetId="3" state="hidden" r:id="rId3"/>
  </sheets>
</workbook>"#;

    let rels_xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Target="worksheets/sheet2.xml"/>
  <Relationship Id="rId3" Target="worksheets/sheet3.xml"/>
</Relationships>"#;

    let mut sheet1 = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><worksheet><sheetData>
<row r="14"><c r="L14"><v>4492</v></c><c r="N14"><v>4869</v></c></row>
<row r="191"><c r="L191"><v>1699</v></c><c r="N191"><v>1999</v></c></row>
<row r="192"><c r="L192"><v>724</v></c><c r="N192"><v>639</v></c></row>
<row r="193"><c r="L193"><v>547</v></c><c r="N193"><v>542</v></c></row>
<row r="194"><c r="L194"><v>200</v></c><c r="N194"><v>234</v></c></row>
<row r="195"><c r="L195"><v>9</v></c><c r="N195"><v>15</v></c></row>
<row r="196"><c r="L196"><v>314</v></c><c r="N196"><v>372</v></c></row>
<row r="197"><c r="L197"><v>216</v></c><c r="N197"><v>234</v></c></row>
<row r="198"><c r="L198"><v>562</v></c><c r="N198"><v>497</v></c></row>
<row r="199"><c r="L199"><v>138</v></c><c r="N199"><v>757</v></c></row>
<row r="200"><c r="L200"><v>409</v></c><c r="N200"><v>348</v></c></row>
<row r="201"><c r="L201"><v>170</v></c><c r="N201"><v>132</v></c></row>
<row r="202"><c r="L202"><v>158</v></c><c r="N202"><v>78</v></c></row>
<row r="203"><c r="L203"><v>110</v></c><c r="N203"><v>84</v></c></row>
<row r="204"><c r="L204"><v>84</v></c><c r="N204"><v>0</v></c></row>
<row r="206"><c r="L206"><v>5332</v></c><c r="N206"><v>5882</v></c></row>
<row r="208"><c r="J208"><v>2582</v></c><c r="L208"><v>1395</v></c><c r="N208"><v>1380</v></c></row>
<row r="218"><c r="L218"><v>1638</v></c><c r="N218"><v>1484</v></c></row>
<row r="228"><c r="L228"><v>3255</v></c><c r="N228"><v>3973</v></c></row>
<row r="230"><c r="A230" t="e"><v>#REF!</v></c><c r="B230" t="inlineStr"><is><t>-</t></is></c></row>"#,
    );
    for r in 231..=350 {
        for col_idx in 0..20 {
            let col_ch = (b'A' + (col_idx as u8)) as char;
            sheet1.push_str(&format!(
                "<row r=\"{r}\"><c r=\"{col_ch}{r}\"><v>{}</v></c></row>",
                r * 10 + col_idx
            ));
        }
    }
    sheet1.push_str("</sheetData></worksheet>");

    let sheet2 = br#"<?xml version="1.0" encoding="UTF-8"?><worksheet><sheetData>
<row r="14"><c r="L14"><v>961</v></c><c r="N14"><v>871</v></c><c r="P14" t="inlineStr"><is><t>N/A</t></is></c></row>
</sheetData></worksheet>"#;

    write_stored_zip(&[
        ("xl/workbook.xml", wb_xml),
        ("xl/_rels/workbook.xml.rels", rels_xml),
        ("xl/worksheets/sheet1.xml", sheet1.as_bytes()),
        ("xl/worksheets/sheet2.xml", sheet2),
    ])
}
