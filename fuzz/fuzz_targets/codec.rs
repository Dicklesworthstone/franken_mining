//! The `FMNE` codec kit (`fmine_core::codec`, BD-017).
//!
//! Input: a mode byte, a script length byte, the script, then the payload. Each script byte names one read.
//! - Decode: the payload, shaped by the mode (see `fmine_fuzz::shape`), is opened and read by the script. No read
//!   may panic, and a length-prefixed read never returns more than its bound.
//! - Round trip: the script and payload define typed values. The encoder writes them, and the decoder must read back
//!   exactly the same values and then find no trailing bytes. Flipping any one bit of the frame must be refused,
//!   since CRC32C detects every single-bit error.
//! - Canonical order: a sequence read back is strictly ascending and within its count bound, and the encoder takes a
//!   sequence exactly when it is strictly ascending, writing nothing otherwise.
#![no_main]

use std::collections::BTreeSet;

use fmine_core::codec::{Decoder, Encoder, MAX_FRAME_BYTES};
use fmine_fuzz::{MIN_FRAME, shape};
use libfuzzer_sys::fuzz_target;

/// The number of read kinds a script byte chooses from.
const OPS: u8 = 11;

const TAG: [u8; 4] = *b"FUZZ";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I64(i64),
    I128(i128),
    Fixed4([u8; 4]),
    Fixed32([u8; 32]),
    Bytes(Vec<u8>),
    Str(String),
    Seq(Vec<u16>),
}

/// The bound a length-prefixed read uses for script byte `op`, from 0 to past `MAX_FRAME_BYTES`.
fn bound(op: u8) -> usize {
    match op >> 4 {
        0 => 0,
        15 => usize::MAX,
        k => 1usize << (2 * usize::from(k)).min(28),
    }
}

fn read(dec: &mut Decoder<'_>, op: u8) -> Option<Value> {
    let max = bound(op);
    let value = match op % OPS {
        0 => Value::U8(dec.get_u8().ok()?),
        1 => Value::U16(dec.get_u16().ok()?),
        2 => Value::U32(dec.get_u32().ok()?),
        3 => Value::U64(dec.get_u64().ok()?),
        4 => Value::I64(dec.get_i64().ok()?),
        5 => Value::I128(dec.get_i128().ok()?),
        6 => Value::Fixed4(dec.get_fixed::<4>().ok()?),
        7 => Value::Fixed32(dec.get_fixed::<32>().ok()?),
        8 => {
            let bytes = dec.get_bytes(max).ok()?;
            assert!(
                bytes.len() <= max.min(MAX_FRAME_BYTES),
                "get_bytes returned past its bound"
            );
            Value::Bytes(bytes.to_vec())
        }
        9 => {
            let s = dec.get_str(max).ok()?;
            assert!(
                s.len() <= max.min(MAX_FRAME_BYTES),
                "get_str returned past its bound"
            );
            Value::Str(s.to_owned())
        }
        _ => {
            let seq = dec.get_canonical_seq(max, Decoder::get_u16).ok()?;
            assert!(
                seq.len() <= max,
                "get_canonical_seq returned past its bound"
            );
            assert!(
                seq.windows(2).all(|w| w[0] < w[1]),
                "get_canonical_seq returned a sequence out of order"
            );
            Value::Seq(seq)
        }
    };
    Some(value)
}

/// The encoder must take `raw` exactly when it is strictly ascending, and write nothing when it refuses.
fn check_encoder_order(raw: &[u16]) {
    let ascending = raw.windows(2).all(|w| w[0] < w[1]);
    let mut enc = Encoder::new_frame(TAG, 1);
    let taken = enc.put_canonical_seq(raw, |e, v| e.put_u16(*v)).is_ok();
    assert_eq!(
        taken, ascending,
        "put_canonical_seq misjudged the order of {raw:?}"
    );
    if !taken {
        assert_eq!(
            enc.finish().len(),
            MIN_FRAME,
            "a refused sequence wrote bytes"
        );
    }
}

/// Takes `n` bytes from the front of `rest`.
fn take<'a>(rest: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    if rest.len() < n {
        return None;
    }
    let (head, tail) = rest.split_at(n);
    *rest = tail;
    Some(head)
}

fn arr<const N: usize>(rest: &mut &[u8]) -> Option<[u8; N]> {
    take(rest, N)?.try_into().ok()
}

/// The value script byte `op` takes from the payload, or `None` once the payload runs out.
fn value(op: u8, rest: &mut &[u8]) -> Option<Value> {
    Some(match op % OPS {
        0 => Value::U8(arr::<1>(rest)?[0]),
        1 => Value::U16(u16::from_le_bytes(arr(rest)?)),
        2 => Value::U32(u32::from_le_bytes(arr(rest)?)),
        3 => Value::U64(u64::from_le_bytes(arr(rest)?)),
        4 => Value::I64(i64::from_le_bytes(arr(rest)?)),
        5 => Value::I128(i128::from_le_bytes(arr(rest)?)),
        6 => Value::Fixed4(arr(rest)?),
        7 => Value::Fixed32(arr(rest)?),
        k @ (8 | 9) => {
            let n = usize::from(arr::<1>(rest)?[0]).min(bound(op));
            let bytes = take(rest, n)?;
            if k == 8 {
                Value::Bytes(bytes.to_vec())
            } else {
                Value::Str(std::str::from_utf8(bytes).ok()?.to_owned())
            }
        }
        _ => {
            let n = usize::from(arr::<1>(rest)?[0]).min(bound(op));
            let raw = (0..n)
                .map(|_| arr(rest).map(u16::from_le_bytes))
                .collect::<Option<Vec<u16>>>()?;
            check_encoder_order(&raw);
            Value::Seq(
                raw.into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            )
        }
    })
}

fn write(enc: &mut Encoder, v: &Value) {
    match v {
        Value::U8(x) => enc.put_u8(*x),
        Value::U16(x) => enc.put_u16(*x),
        Value::U32(x) => enc.put_u32(*x),
        Value::U64(x) => enc.put_u64(*x),
        Value::I64(x) => enc.put_i64(*x),
        Value::I128(x) => enc.put_i128(*x),
        Value::Fixed4(x) => enc.put_fixed(x),
        Value::Fixed32(x) => enc.put_fixed(x),
        Value::Bytes(x) => enc.put_bytes(x),
        Value::Str(x) => enc.put_str(x),
        Value::Seq(x) => enc
            .put_canonical_seq(x, |e, v| e.put_u16(*v))
            .expect("a set's elements are in canonical order"),
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let Some((&len, rest)) = rest.split_first() else {
        return;
    };
    let len = usize::from(len).min(rest.len());
    let (script, payload) = rest.split_at(len);

    // Decode arbitrary bytes.
    let bytes = shape(mode, TAG, u16::from(mode >> 2), payload);
    if let Ok(mut dec) = Decoder::open_frame(&bytes, TAG, 0, 1) {
        assert!(dec.version() <= 1);
        for &op in script {
            if read(&mut dec, op).is_none() {
                break;
            }
        }
        let _ = dec.finish();
    }

    // Round trip.
    let mut source = payload;
    let mut values = Vec::new();
    for &op in script {
        let Some(v) = value(op, &mut source) else {
            break;
        };
        values.push((op, v));
    }
    let mut enc = Encoder::new_frame(TAG, 1);
    for (_, v) in &values {
        write(&mut enc, v);
    }
    let mut encoded = enc.finish();
    let mut dec = Decoder::open_frame(&encoded, TAG, 1, 1).expect("an encoded frame must open");
    for (op, v) in &values {
        let got = read(&mut dec, *op).expect("an encoded value must read back");
        assert_eq!(&got, v, "round trip changed a value");
    }
    dec.finish()
        .expect("an encoded frame has no trailing bytes");
    let bit = usize::from(mode) * 8 + script.len();
    let at = bit / 8 % encoded.len();
    encoded[at] ^= 1 << (bit % 8);
    assert!(
        Decoder::open_frame(&encoded, TAG, 1, 1).is_err(),
        "a one-bit change at byte {at} was accepted"
    );
});
