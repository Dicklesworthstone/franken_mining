//! DESIGN (BD-047): bounded, opaque forecast-operation journal frames. Determinism: BitwisePortable.
//! This S0 codec owns the durable bytes, sequence/hash chain and clock-bound arithmetic. It does not
//! interpret forecast requests, obtain a clock, grant authority or establish an external time proof.

use std::fmt::Display;

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};

pub const MAX_CELL_ID_BYTES: usize = 256;
pub const MAX_OPERATION_BYTES: usize = 64 * 1024;
pub const MAX_RECORD_BYTES: usize = MAX_OPERATION_BYTES + 128;
pub const MAX_JOURNAL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RECORDS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub cell_id: String,
    /// Operator-declared bound on the injected wall clock; never inferred from journal contents.
    pub clock_skew_ns: u64,
}

impl Header {
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.cell_id.trim().is_empty()
            || self.cell_id.len() > MAX_CELL_ID_BYTES
            || self.cell_id.chars().any(char::is_control)
            || self.clock_skew_ns > i64::MAX as u64
        {
            return Err(invalid("invalid cell id or clock-skew bound"));
        }
        Ok(())
    }

    /// Checked earliest/latest instants under the header's declared clock uncertainty.
    pub fn time_bounds(&self, observed_ns: i64) -> Result<(i64, i64), Refusal> {
        self.validate()?;
        if observed_ns < 0 {
            return Err(invalid("observed wall time precedes the Unix epoch"));
        }
        let observed = i128::from(observed_ns);
        let skew = i128::from(self.clock_skew_ns);
        Ok((
            i64::try_from(observed - skew).map_err(|_| invalid("earliest time overflows"))?,
            i64::try_from(observed + skew).map_err(|_| invalid("latest time overflows"))?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Genesis(Header),
    /// Exact UTF-8 request bytes; the consumer owns their schema and business semantics.
    Operation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub sequence: u64,
    /// BLAKE3 of the exact preceding lowercase hex frame plus LF; zero for genesis.
    pub previous: Digest32,
    pub observed_ns: i64,
    pub entry: Entry,
}

impl Record {
    pub fn to_bytes(&self) -> Result<Vec<u8>, Refusal> {
        if self.observed_ns < 0 {
            return Err(invalid("observed wall time precedes the Unix epoch"));
        }
        let mut enc = Encoder::new_frame(*b"FJR1", 1);
        enc.put_u64(self.sequence);
        enc.put_fixed(&self.previous.0);
        enc.put_i64(self.observed_ns);
        match &self.entry {
            Entry::Genesis(header) => {
                if self.sequence != 0 || self.previous != Digest32([0; 32]) {
                    return Err(integrity(
                        "genesis needs sequence zero and a zero previous head",
                    ));
                }
                header.time_bounds(self.observed_ns)?;
                enc.put_u8(0);
                enc.put_str(&header.cell_id);
                enc.put_u64(header.clock_skew_ns);
            }
            Entry::Operation(request) => {
                if self.sequence == 0 {
                    return Err(integrity("an operation cannot be genesis"));
                }
                if request.is_empty() || request.len() > MAX_OPERATION_BYTES {
                    return Err(invalid("operation must contain 1..=65536 UTF-8 bytes"));
                }
                enc.put_u8(1);
                enc.put_str(request);
            }
        }
        Ok(enc.finish())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Refusal> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid("frame exceeds its byte bound"));
        }
        let mut dec = Decoder::open_frame(bytes, *b"FJR1", 1, 1).map_err(codec)?;
        let sequence = dec.get_u64().map_err(codec)?;
        let previous = Digest32(dec.get_fixed().map_err(codec)?);
        let observed_ns = dec.get_i64().map_err(codec)?;
        let entry = match dec.get_u8().map_err(codec)? {
            0 => Entry::Genesis(Header {
                cell_id: dec.get_str(MAX_CELL_ID_BYTES).map_err(codec)?.to_owned(),
                clock_skew_ns: dec.get_u64().map_err(codec)?,
            }),
            1 => Entry::Operation(dec.get_str(MAX_OPERATION_BYTES).map_err(codec)?.to_owned()),
            _ => return Err(invalid("unknown entry tag")),
        };
        dec.finish().map_err(codec)?;
        let record = Self {
            sequence,
            previous,
            observed_ns,
            entry,
        };
        if record.to_bytes()? != bytes {
            return Err(integrity("noncanonical frame"));
        }
        Ok(record)
    }

    pub fn to_line(&self) -> Result<Vec<u8>, Refusal> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let frame = self.to_bytes()?;
        let mut line = Vec::with_capacity(frame.len() * 2 + 1);
        for byte in frame {
            line.push(HEX[usize::from(byte >> 4)]);
            line.push(HEX[usize::from(byte & 15)]);
        }
        line.push(b'\n');
        Ok(line)
    }

    /// Decode exactly one canonical line, including its mandatory final LF.
    pub fn from_line(line: &[u8]) -> Result<Self, Refusal> {
        let body = line
            .strip_suffix(b"\n")
            .ok_or_else(|| integrity("incomplete final line"))?;
        if body.is_empty() || body.len() % 2 != 0 || body.len() > 2 * MAX_RECORD_BYTES {
            return Err(integrity("invalid-sized hexadecimal line"));
        }
        let mut frame = Vec::with_capacity(body.len() / 2);
        for pair in body.as_chunks::<2>().0 {
            let high = nibble(pair[0]).ok_or_else(|| integrity("line must use lowercase hex"))?;
            let low = nibble(pair[1]).ok_or_else(|| integrity("line must use lowercase hex"))?;
            frame.push(high << 4 | low);
        }
        Self::from_bytes(&frame)
    }

    pub fn chain_head(&self) -> Result<Digest32, Refusal> {
        Ok(blake3_hash(&self.to_line()?))
    }

    pub fn time_bounds(&self, header: &Header) -> Result<(i64, i64), Refusal> {
        header.time_bounds(self.observed_ns)
    }
}

/// Validate the complete chain. A self-consistent prefix is not evidence that no suffix was removed;
/// the filesystem adapter must compare an independently retained final-head pin when one is supplied.
pub fn validate_chain(records: &[Record]) -> Result<Header, Refusal> {
    if records.is_empty() || records.len() > MAX_RECORDS {
        return Err(integrity("journal must contain 1..=100000 records"));
    }
    let Entry::Genesis(header) = &records[0].entry else {
        return Err(integrity("first record is not genesis"));
    };
    let mut previous = Digest32([0; 32]);
    let mut observed = 0;
    let mut size = 0_usize;
    for (sequence, record) in records.iter().enumerate() {
        if record.sequence != sequence as u64 || record.previous != previous {
            return Err(integrity(
                "sequence or previous head does not extend the journal",
            ));
        }
        if sequence > 0 && !matches!(record.entry, Entry::Operation(_)) {
            return Err(integrity("genesis may occur only once"));
        }
        if record.observed_ns < observed {
            return Err(integrity("observed wall time regressed"));
        }
        record.time_bounds(header)?;
        let line = record.to_line()?;
        size = size
            .checked_add(line.len())
            .filter(|n| *n <= MAX_JOURNAL_BYTES)
            .ok_or_else(|| integrity("journal exceeds its byte bound"))?;
        previous = blake3_hash(&line);
        observed = record.observed_ns;
    }
    Ok(header.clone())
}

/// Bounded canonical line decoding and complete chain validation; no operation text is interpreted.
pub fn decode_journal(bytes: &[u8]) -> Result<Vec<Record>, Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_JOURNAL_BYTES || !bytes.ends_with(b"\n") {
        return Err(integrity(
            "journal is empty, oversized or has an incomplete tail",
        ));
    }
    let mut records = Vec::new();
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        if records.len() >= MAX_RECORDS {
            return Err(integrity("journal exceeds its record bound"));
        }
        records.push(Record::from_line(line)?);
    }
    validate_chain(&records)?;
    Ok(records)
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn invalid(detail: impl Display) -> Refusal {
    Refusal::new(
        RefusalCode::UndeclaredUnitBasisOrTz107,
        format!("forecast journal: {detail}"),
    )
}

fn integrity(detail: impl Display) -> Refusal {
    Refusal::new(
        RefusalCode::StaleAnchor502,
        format!("forecast journal: {detail}"),
    )
}

fn codec(error: impl Display) -> Refusal {
    integrity(error)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn records() -> Vec<Record> {
        let genesis = Record {
            sequence: 0,
            previous: Digest32([0; 32]),
            observed_ns: 100,
            entry: Entry::Genesis(Header {
                cell_id: "test-cell".into(),
                clock_skew_ns: 5,
            }),
        };
        let operation = Record {
            sequence: 1,
            previous: genesis.chain_head().unwrap(),
            observed_ns: 101,
            entry: Entry::Operation("kind = \"register\"\nvalue = \"é\"\n".into()),
        };
        vec![genesis, operation]
    }

    #[test]
    fn canonical_round_trip_preserves_exact_requests_and_line_heads() {
        let records = records();
        let bytes: Vec<_> = records.iter().flat_map(|r| r.to_line().unwrap()).collect();
        assert_eq!(decode_journal(&bytes).unwrap(), records);
        for record in records {
            let frame = record.to_bytes().unwrap();
            assert_eq!(&frame[..10], b"FMNEFJR1\x01\x00");
            assert_eq!(&frame[10..18], &record.sequence.to_le_bytes());
            assert_eq!(&frame[18..50], &record.previous.0);
            assert_eq!(&frame[50..58], &record.observed_ns.to_le_bytes());
            assert_eq!(
                record.chain_head().unwrap(),
                blake3_hash(&record.to_line().unwrap())
            );
            assert_ne!(record.chain_head().unwrap(), blake3_hash(&frame));
            assert_eq!(Record::from_bytes(&frame).unwrap(), record);
        }
    }

    #[test]
    fn corrupted_noncanonical_and_truncated_records_refuse() {
        for record in records() {
            let frame = record.to_bytes().unwrap();
            for i in 0..frame.len() {
                let mut changed = frame.clone();
                changed[i] ^= 1;
                assert!(Record::from_bytes(&changed).is_err(), "byte {i}");
            }
            let mut line = record.to_line().unwrap();
            line.pop();
            assert!(Record::from_line(&line).is_err());
            line.push(b'\r');
            line.push(b'\n');
            assert!(Record::from_line(&line).is_err());
            assert!(Record::from_line(&record.to_line().unwrap().to_ascii_uppercase()).is_err());
        }
        for invalid in [b"".as_slice(), b"\n", b"0\n", b"zz\n"] {
            assert!(decode_journal(invalid).is_err());
        }
    }

    #[test]
    fn sequences_genesis_time_bounds_and_payload_limits_are_checked() {
        let good = records();
        assert_eq!(
            validate_chain(&good).unwrap().time_bounds(101).unwrap(),
            (96, 106)
        );
        for mutate in [0, 1, 2, 3] {
            let mut bad = good.clone();
            match mutate {
                0 => bad[1].sequence = 2,
                1 => bad[1].previous = Digest32([0; 32]),
                2 => bad[1].observed_ns = 99,
                _ => bad[1].entry = bad[0].entry.clone(),
            }
            assert!(validate_chain(&bad).is_err());
        }
        let Entry::Genesis(header) = &good[0].entry else {
            panic!("genesis")
        };
        assert!(header.time_bounds(i64::MAX).is_err());
        assert!(header.time_bounds(-1).is_err());
        let mut operation = good[1].clone();
        operation.entry = Entry::Operation("x".repeat(MAX_OPERATION_BYTES));
        assert!(operation.to_bytes().unwrap().len() <= MAX_RECORD_BYTES);
        operation.entry = Entry::Operation("x".repeat(MAX_OPERATION_BYTES + 1));
        assert!(operation.to_bytes().is_err());
        operation.entry = Entry::Operation(String::new());
        assert!(operation.to_bytes().is_err());
        let mut bad = header.clone();
        bad.cell_id = "x".repeat(MAX_CELL_ID_BYTES + 1);
        assert!(bad.validate().is_err());
        bad.cell_id = "cell\nother".into();
        assert!(bad.validate().is_err());
        assert!(Record::from_line(&vec![b'0'; MAX_RECORD_BYTES * 2 + 2]).is_err());
        assert!(decode_journal(&vec![b'\n'; MAX_JOURNAL_BYTES + 1]).is_err());
    }
}
