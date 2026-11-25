//! The `FMT-SEG-1` segment scanner (`fmine_ledger::segment::inspect_segment_bytes`; BD-082, BD-088, `F7`).
//!
//! The target writes one clean segment once: six batches whose frames run from well inside one 512-byte sector
//! to over two sectors long, then a seal. Each input damages a copy of it: a mode byte, a selector byte, a
//! 16-bit cut, then edits, each `[kind, offset lo, offset hi, n]`, whose offset counts back from the end of its
//! range when its top bit is set. Outside a crash, by `kind % 3`, an edit fills `n` bytes with the next input
//! byte, writes the next `n` input bytes, or inserts them, as stray bytes would. Modes, by `mode % 3`:
//! - 0, a crash: keep the first `k` batches, cut the write in flight (batch `k`, or the seal) anywhere, and set
//!   `n`-byte ranges of what reached the disk to zeros (`kind` even) or 0xFF (`kind` odd). Inspection must
//!   succeed, keep the `k` batches with their head hash, and read any damage as a torn tail at the old end.
//! - 1, damage inside one record: edits land only on batch `r`'s bytes, and insertions anywhere from its first
//!   byte to just after its last, so a whole record (a batch or the footer) still follows. Inspection must
//!   never read a torn tail, which repair would truncate (`INV-003`).
//! - 2, any damage: a sealed or open segment, a cut anywhere, up to seven edits anywhere, and the rest of the
//!   input appended.
//!
//! In every mode a torn tail must be consistent (its offset is the verified length, and the verified prefix
//! inspects as an open segment with the same batches and head hash) and a clean read verifies every byte.
//! Inputs stay small (libFuzzer's default limit is 4 KiB), so this target cannot show a scan that slows with
//! file size; `segment::tests::test_crafted_overlapping_candidates_refuse_within_the_check_budget` covers that.
#![no_main]

use std::sync::OnceLock;

use fmine_core::anchor::{FMT_ANCHOR_V1, LedgerAnchor};
use fmine_core::hash::{Digest32, sha256};
use fmine_ledger::batch::{
    DeltaBatch, DeltaKind, DeltaOp, FMT_LEDGER_V1, GENESIS_PREV_HASH, Witness, WitnessKind,
};
use fmine_ledger::segment::{
    SegmentInspection, SegmentState, SegmentWriter, inspect_segment_bytes,
};
use libfuzzer_sys::fuzz_target;

const CELL: &str = "CELL-SITE(fuzz)";
const BATCHES: usize = 6;

/// The clean sealed segment.
struct Clean {
    bytes: Vec<u8>,
    /// Where batch `i` starts for `i < BATCHES`, then where the footer starts, then the end of the file.
    starts: Vec<usize>,
    /// The head hash after `k` batches, for `k` in `0..=BATCHES`.
    heads: Vec<Digest32>,
}

/// Batch `seq`: payloads of 24, 300 and 900 bytes in turn, and a fencing epoch that rises every fourth batch.
fn batch(seq: u64, prev_hash: Digest32) -> DeltaBatch {
    let epoch = 1 + seq / 4;
    let payload_len = [24_usize, 300, 900][(seq % 3) as usize];
    let payload = (0..payload_len)
        .map(|i| (seq as u8).wrapping_mul(31).wrapping_add(i as u8))
        .collect();
    DeltaBatch {
        format_version: FMT_LEDGER_V1,
        cell_id: CELL.to_string(),
        epoch,
        seq,
        prev_hash,
        anchor: LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: CELL.to_string(),
            ledger_epoch: epoch,
            commit_sequence: seq,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 1,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: sha256(format!("{CELL}:{seq}").as_bytes()),
        },
        deltas: vec![DeltaOp::new(
            format!("sample/{seq:04}"),
            DeltaKind::Put,
            payload,
        )],
        witnesses: vec![Witness::new(
            WitnessKind::Continuity,
            format!("receipt/{seq:04}"),
            sha256(&seq.to_le_bytes()),
        )],
        cross_cell_refs: Vec::new(),
    }
}

fn clean() -> &'static Clean {
    static CLEAN: OnceLock<Clean> = OnceLock::new();
    CLEAN.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("fmine-fuzz-segment-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("clean.fmseg");
        let _ = std::fs::remove_file(&path);
        let len = |path: &std::path::Path| std::fs::metadata(path).expect("length").len() as usize;
        let mut writer =
            SegmentWriter::create(&path, CELL, 0, GENESIS_PREV_HASH, Vec::new()).expect("create");
        let mut starts = vec![len(&path)];
        let mut heads = vec![GENESIS_PREV_HASH];
        let mut prev = GENESIS_PREV_HASH;
        for seq in 0..BATCHES as u64 {
            prev = writer.append(&batch(seq, prev)).expect("append");
            heads.push(prev);
            starts.push(len(&path));
        }
        writer.seal().expect("seal");
        let bytes = std::fs::read(&path).expect("read");
        starts.push(bytes.len());
        let _ = std::fs::remove_dir_all(&dir);
        Clean {
            bytes,
            starts,
            heads,
        }
    })
}

struct Input<'a> {
    data: &'a [u8],
}

impl<'a> Input<'a> {
    fn byte(&mut self) -> Option<u8> {
        let (&b, rest) = self.data.split_first()?;
        self.data = rest;
        Some(b)
    }

    fn take(&mut self, n: usize) -> &'a [u8] {
        let (head, rest) = self.data.split_at(n.min(self.data.len()));
        self.data = rest;
        head
    }
}

/// Applies up to `max` edits from `input` inside `bytes[lo..hi]` and returns where that range now ends. With
/// `crash`, an edit sets its range to zeros (`kind` even) or 0xFF (`kind` odd), as a lost sector reads back.
fn edit(
    bytes: &mut Vec<u8>,
    lo: usize,
    mut hi: usize,
    input: &mut Input<'_>,
    crash: bool,
    max: usize,
) -> usize {
    for _ in 0..max {
        let (Some(kind), Some(a), Some(b), Some(n)) =
            (input.byte(), input.byte(), input.byte(), input.byte())
        else {
            break;
        };
        // With its top bit set, an offset counts back from the end of the range, so the bytes where two
        // records meet are as easy to reach as the start.
        let offset = u16::from_le_bytes([a, b]);
        let from_end = offset & 0x8000 != 0;
        let offset = usize::from(offset & 0x7FFF);
        let n = usize::from(n);
        if !crash && kind % 3 == 2 {
            let room = hi - lo + 1;
            let at = if from_end {
                hi - offset % room
            } else {
                lo + offset % room
            };
            let src = input.take(n);
            bytes.splice(at..at, src.iter().copied());
            hi += src.len();
            continue;
        }
        if hi == lo {
            continue;
        }
        let at = if from_end {
            hi - 1 - offset % (hi - lo)
        } else {
            lo + offset % (hi - lo)
        };
        let n = n.min(hi - at);
        if crash {
            bytes[at..at + n].fill(if kind % 2 == 0 { 0x00 } else { 0xFF });
        } else if kind % 3 == 0 {
            let fill = input.byte().unwrap_or(0);
            bytes[at..at + n].fill(fill);
        } else {
            let src = input.take(n);
            bytes[at..at + src.len()].copy_from_slice(src);
        }
    }
    hi
}

/// A torn tail is consistent; a clean read verifies every byte.
fn consistent(bytes: &[u8], inspected: &SegmentInspection) {
    assert_eq!(inspected.total_bytes, bytes.len() as u64);
    match &inspected.state {
        SegmentState::TornTail {
            torn_offset,
            torn_bytes,
            ..
        } => {
            assert_eq!(*torn_offset, inspected.verified_bytes);
            assert_eq!(inspected.verified_bytes + torn_bytes, inspected.total_bytes);
            let prefix = &bytes[..inspected.verified_bytes as usize];
            let again = inspect_segment_bytes(prefix)
                .expect("the verified prefix of a torn segment must inspect");
            assert!(
                matches!(again.state, SegmentState::OpenUnsealed),
                "the verified prefix must be an open segment: {:?}",
                again.state
            );
            assert_eq!(again.batches.len(), inspected.batches.len());
            assert_eq!(again.head_hash, inspected.head_hash);
        }
        SegmentState::Sealed(_) | SegmentState::OpenUnsealed => {
            assert_eq!(inspected.verified_bytes, inspected.total_bytes);
        }
    }
}

fn crash(clean: &Clean, acked: usize, cut_draw: usize, input: &mut Input<'_>) {
    let old_end = clean.starts[acked];
    let new_end = clean.starts[acked + 1];
    let cut = old_end + cut_draw % (new_end - old_end + 1);
    let mut bytes = clean.bytes[..cut].to_vec();
    edit(&mut bytes, old_end, cut, input, true, usize::MAX);
    let label = format!("acked {acked}, cut at {cut} of {old_end}..{new_end}");
    let inspected = inspect_segment_bytes(&bytes)
        .unwrap_or_else(|e| panic!("{label}: a crash state was refused: {e}"));
    consistent(&bytes, &inspected);
    let (records, torn) = if bytes[..] == clean.bytes[..new_end] {
        ((acked + 1).min(BATCHES), false)
    } else if bytes[..] == clean.bytes[..old_end] {
        (acked, false)
    } else {
        (acked, true)
    };
    assert_eq!(inspected.batches.len(), records, "{label}");
    assert_eq!(inspected.head_hash, clean.heads[records], "{label}");
    match inspected.state {
        SegmentState::TornTail { torn_offset, .. } => {
            assert!(torn, "{label}: an intact state read as torn");
            assert_eq!(torn_offset, old_end as u64, "{label}");
        }
        SegmentState::Sealed(_) => {
            assert!(
                !torn && acked == BATCHES && cut == new_end,
                "{label}: sealed"
            )
        }
        SegmentState::OpenUnsealed => assert!(!torn, "{label}: damage read as clean"),
    }
}

fn one_record(clean: &Clean, record: usize, input: &mut Input<'_>) {
    let mut bytes = clean.bytes.clone();
    let lo = clean.starts[record];
    let hi = edit(
        &mut bytes,
        lo,
        clean.starts[record + 1],
        input,
        false,
        usize::MAX,
    );
    if let Ok(inspected) = inspect_segment_bytes(&bytes) {
        consistent(&bytes, &inspected);
        assert!(
            !matches!(inspected.state, SegmentState::TornTail { .. }),
            "damage inside batch {record} ({lo}..{hi}), with whole records after it, read as a torn tail: {:?}",
            inspected.state
        );
    }
}

fn any_damage(clean: &Clean, select: u8, cut_draw: usize, input: &mut Input<'_>) {
    let base = if select % 2 == 0 {
        &clean.bytes[..]
    } else {
        &clean.bytes[..clean.starts[BATCHES]]
    };
    let cut = cut_draw % (base.len() + 1);
    let mut bytes = base[..cut].to_vec();
    edit(
        &mut bytes,
        0,
        cut,
        input,
        false,
        usize::from(select / 2 % 8),
    );
    bytes.extend_from_slice(input.take(usize::MAX));
    if let Ok(inspected) = inspect_segment_bytes(&bytes) {
        consistent(&bytes, &inspected);
    }
}

fuzz_target!(|data: &[u8]| {
    let clean = clean();
    let mut input = Input { data };
    let (Some(mode), Some(select), Some(c0), Some(c1)) =
        (input.byte(), input.byte(), input.byte(), input.byte())
    else {
        return;
    };
    let cut_draw = usize::from(u16::from_le_bytes([c0, c1]));
    match mode % 3 {
        0 => crash(
            clean,
            usize::from(select) % (BATCHES + 1),
            cut_draw,
            &mut input,
        ),
        1 => one_record(clean, usize::from(select) % BATCHES, &mut input),
        _ => any_damage(clean, select, cut_draw, &mut input),
    }
});
