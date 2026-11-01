//! Mutation robustness of the workbook reader (BD-073, threat T1). A downloaded `.xlsx` is hostile input, and a
//! panic in the reader is exit 101 instead of a typed refusal.
//!
//! Two synthetic seeds are mutated under fixed Philox keys: a stored workbook from `build_minimal_xlsx_styled`,
//! and a deflated one made by Python's `zipfile` whose entries use dynamic-Huffman, fixed-Huffman and stored
//! DEFLATE blocks. A case takes one of three forms: byte edits anywhere in the archive (the container), edits to
//! one entry's contents re-zipped intact (the XML, styles and number formats), or overwrites inside the deflated
//! payloads with every header intact (the inflater). Each mutant goes through `read_zip_entries`,
//! `Workbook::from_bytes`, the three censuses and every numeric cell's displayed text. No mutant may panic, and
//! an archive the reader accepts stays within `MAX_ENTRY_BYTES` and `MAX_ARCHIVE_BYTES`.
//!
//! This is a bounded random search, not a proof and not a coverage-guided fuzzer. All content is invented.
#![cfg(test)]

use std::collections::BTreeMap;
use std::ops::Range;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
};
use fmine_io::zip_inflate::{
    MAX_ARCHIVE_BYTES, MAX_ENTRY_BYTES, ZipError, read_zip_entries, write_stored_zip,
};

/// Mutants per seed.
const CASES: u32 = 4_000;

/// The deflated seed, 3,659 bytes, from a Python script outside the tree (`zipfile`, fixed timestamps, invented
/// content): xl/workbook.xml with a visible `Summary` and a hidden `Workings`, its relationships with a styles
/// relationship, styles with two custom formats, shared strings with a rich-text run and entities, a 68-row sheet
/// (inline strings, styled numbers, `#REF!` errors), a 300-byte random entry in a stored block and a 14-byte
/// entry in a fixed-Huffman block.
const DEFLATED_HEX: &str = concat!(
    "504b03041400000008000000615d39e601bac90000004a0100000f000000786c2f776f726b626f6f6b2e786d6c8d504d",
    "6fc2300cfd2b91ef90d2039aaab65cd024ce63da396b5c125127951d36f8f7f36048ecc6c95fcfeff9b9dd9c69325fc8",
    "1273ea60b5acc0601ab28fe9d0c1fbfe75f1029bbefdce7cfcccf968149da48350cadc582b434072b2cc33269d8c99c9",
    "152df9606566745e0262a1c9d655b5b6e462821b43c3cf70e4718c036ef370224ce546c238b9a2b74a88b340df5e15e4",
    "2f9ae4083b783b1139be80b936775e7d81e1266ac23bbf02fb1ffea1e6d4ae3ce06bcd8b2b3a0cd17b4c0febf5efbabd",
    "cbdafb67fa1f504b03041400000008000000615d373768e5b8000000790100001a000000786c2f5f72656c732f776f72",
    "6b626f6f6b2e786d6c2e72656c738d90dd0ac2300c855fa5e4de659b2022d6dd88e0ade803942efbc1ad2d4dfdd9db5b",
    "14d1818a57e1907c27395916d7be1367f2dc5a23214b521064b42d5b534b38ec37933914abe58e3a15e20437ad631111",
    "c3129a10dc02917543bde2c43a32b15359dfab10a5afd1297d5435619ea633f4ef1e30f614db5282df961988bdf23505",
    "0917eb8fdc1005c67bc992680ef899cbbf73f92f6e1ab9c1d13f596c55b59ad6569f7a32e14324e43074c4af4b1efab9",
    "1e473f5cdd00504b03041400000008000000615d056ca0cdc30000004d0100000d000000786c2f7374796c65732e786d",
    "6c6d90490bc2301085ff4a98e24d4d5c1169e241287856c16b69a7b690a524a9e8bf375d70414fc9bc7cf3de23f1eeae",
    "24b9a17595d11c665306047566f24a5f399c4fc964033b113bff90782c113d09bc761c4aefeb2da52e2b51a56e6a6ad4",
    "e1a53056a53e8cf64a5d6d31cd5dbba4249d33b6a62aad348858372a51de91cc34da7398bf24d21f873c34592f81f476",
    "7b932387681c452cd4a37fe1d5371c40366a513a64893843292fc52b741142efc58747e7fca5b4157eb555673bb885db",
    "fb6bc413504b03041400000008000000615d00eef147cc0000001501000014000000786c2f736861726564537472696e",
    "67732e786d6c458fdd4a033110855f250c52f4c2665510e92629a5a0785d7d80908cbbc1cd8f99d9227d7ab3fed48bb9",
    "38e79be19c51dbcf388923560a3969b859772030b9ec431a34bcbe3c5e3fc0d62822166d31918691b96ca4243762b4b4",
    "ce0553236fb946cb4dd64152a9683d8d881c2779db75f732da9040b83c27d67007624ee163c6fd9f6e01c1283687c028",
    "764ab251727196a90b78ca93173f7efdb54acd7e76e8c5e57b3e5d9dd9f9eebbf0868a75a8a13522ac4704b37b3eecc5",
    "cac6d24fdc5fc87c5a0ddcff27caf6a9f902504b03041400000008000000615dbba7a669c3050000e924000018000000",
    "786c2f776f726b7368656574732f7368656574312e786d6c85daed6e1a571006e05b71e9ef9a9dcfc5954dd426cd05f4",
    "e3022c872656631c014a7af9258ecb9931ef4cfec19e5dde41ac1ecd9ee1fad5bf0f1f2f3e6f76fbfbc7edcd822ea7c5",
    "c5667bf7f8ee7efbfe66f1d79f6f7f5a2d5eadafbf3ceefed97fd86c0e17c7d3b7fb9bc587c3e1d3cfcbe5feeec3e6e1",
    "767ff9f869b33daefcfdb87bb83d1cdfeede2ff79f769bdb774f173d7c5cf234f9f2e1f67ebb585f3f1d7b737bb85d5f",
    "ef1ebf5cec8eb1c7a3775f5ffc428b8bc3cd627f7cff793d5d2f3fafaf9777cf6bbfc635ca6bafe31a9fd696c7cf3f85",
    "f029849f4ebedf7ebcdf6efe38ec8ec7eff7ebebc3faf7e389c78b0fc70bbf1e18c9c70bf6dfaa3c7eba5dbe28ecf5b7",
    "65fe56f4e554e4cb295f9a7c41f912f265beb417f992f305e7eb295f9b7c45f91af26d3afbfe9af315e7db29df9a7c43",
    "f916f29dcfbebfe57cc3f97ecaf726df51be87fcf9fcf7f79cef387f3ee5cf4dfe8cf2e790bf3afffde79c3fe3fcd529",
    "7fd5e4af50fe2ae4d3747e03ac72012b5cc0d5a980aba6802b54c0552c80ceef80ab5cc0152e80a6e1ccd4944013aae1",
    "eb25a308c0c0f309ff5741535145d08eba2a085641b10a80c1f309a72aa8a86270489d870441a428220112289b488589",
    "3450a44e45822c527491000c9465a442461a34526723411c29ea488007ca3e52e1230d20a913922091148d2480046525",
    "a950920693d43949104a8a52329082b295545849034beab424c825452f197041594c9af3fa9be7cccdd3fa8fbffff6f6",
    "87a2cc612a75a812549522ab8c40c9ae52e12a0d58a9939520ad146d65044ac6950a5c79e0ca1dae0c71e529b558e737",
    "0e675cb9c09507aedce1ca10578eb832008533ae5ce0caa1d76c9b4ddc6da6761380c21957ae1ade812b77b832c49523",
    "ae0c40e18c2b17b8f2c0953b5c19e2ca115701a070c6950b5c79e0ca1dae0c71e588ab005038e3ca05ae3c70e50e5786",
    "b872c455d07348c6950b5c79e0ca1dae0c71e588ab002f38e3ca453bcac34eeeec646827473b057991ede4c24e1e7672",
    "6727433b39da29c88b6c271776cab0533a3b05da29537a3c3cbf2f24db29859d32ec94ce4e81764ab453801792ed94c2",
    "4e19764a67a7403b25daa9c00bc9764af5b01e9ed6dbc775fcbc1eed54e085643ba5b053869dd2d929d04e89762af042",
    "b29da22fdaa0e7ccefb6413270950e5781b84ac455d1f646c6550a5c65e02a1dae02719588ab025024e32a05ae327095",
    "0e5781b84ac4550128927195025719b84a87ab405c25e2aa08948cab14b8cac0553a5c05e22a115745a0645ca5c05507",
    "aedae1aa10579dd2de17d8fccab86a81ab0e5cb5c35521ae1a7135008a665cb5c05507aedae1aa10578db81a004533ae",
    "5ae0aa0357ed705588ab465c0d78a11957adf643c38668bb238ab744d39e28da14cdb86ad198eab0533b3b15daa9d14e",
    "035e68b6530b3b75d8a99d9d0aedd468a7012f34dba9859d3aecd4ce4e85766ab4d380179aedd4c24e1d766a67a7423b",
    "35dae9c88b6ca71676eab0533b3b15daa9d14e475e643bb5b0d3869dd6d969d04e9bd2befdf97d61d94e2becb461a775",
    "761ab4d3a29d0ebcb06ca7bd982dbd79cefc6e1b640357eb703588ab455c1d806219572b70b581ab75b81ac4d522ae8e",
    "662d19572b70b581ab75b81ac4d522ae0e40b18cab5523a730736a874e78ea94c64e0014cbb85a81ab0d5cadc3d520ae",
    "96864f0014cbb85a81ab0d5cadc3d520ae16719d01289671b502571bb85a87ab415c2de23a235032ae56e06a0357eb70",
    "3588ab455c67044ac6d50a5c7de0ea1dae0e71f5290d25cfef0bcfb87a81ab0f5cbdc3d521ae1e719d81179e71f5a231",
    "f561a777763ab4d3a39d339ad0663bbdb0d3879dded9e9d04e8f76cec00bcf767a61a70f3bbdb3d3a19d1eed5c012f3c",
    "dbe9859d3eecf4ce4e87767ab47305bcf06ca75743fb30b56fc7f6786e1fed5c012f3cdbe9859d3eecf4ce4e87767a1a",
    "df032f3cdbe9859d3eecf4ce4e87767ab47385bcc876faea451bf49cf9dd36c807aedee1ea10578fb8ae102819577f89",
    "eb32fcbd6879fadfd2fa3f504b03041400000008000000615ddaa1d0df91000000bd00000018000000786c2f776f726b",
    "7368656574732f7368656574322e786d6c4d4ecb0ec2300cfb95aa7796c10110ea3a2121be003ea0eac25ab13e94561b",
    "9f4fb603e210cb716c39aaff8449cc48c5a7d8c97dd34a81d1a6c1c7b193cfc77d7796bd564ba277718855b03d964eba",
    "5af305a05887c19426658c7c79250aa6f24a23944c68862d142638b4ed1182f1516ab56937538d569416415ccbaa5dc9",
    "7565b33e2998b502cbc30ec6bf08fc7ed15f504b03041400000008000000615da04b6337310100002c0100000e000000",
    "786c2f6d656469612f632e62696e012c01d3fe2e3bc91e1619708ca84048e4fa2e4500b50dfbc634fd035f37a87e5d9d",
    "a60e507ad398ae7febd1181fe4e351abf95efeea37c049f9fb023a4c4cf06bf7ed5b6e125593c9d5ae1e3e5d819ba507",
    "99b0b0f0de5611c5b764df40e23b125029f65523958bac3ef15c5e01cca15417ebf985d8efebfa4f4822130ec16612c8",
    "1002ef254bf3b58e3e39c37426759ff8045c6d124b1af9c5b95913f28fc9d10b8f11a29c125656bb30eef80d1d512193",
    "4cf7ede0c5838bd4c5e2d5d740086caaa0fed0a8a020541e1051ab551cc8a1b0a79e24af137e4c966811ac50a8cd9fdc",
    "d118bec72a41e6489bb6e86ae44d12d79db05f19741c41aeb83d82ded07aac3ea3e142c11afcbf1d7699368ffc5c0005",
    "ba29a157de80f289f4d6d3fac66c09ade8ee5848dcee7c62b38252fbfe78cb504b03041400000008000000615d8b7fde",
    "3d100000000e00000010000000646f6350726f70732f6170702e7478742baecc2bc9482dc94c56284e4d4d0100504b01",
    "0214031400000008000000615d39e601bac90000004a0100000f0000000000000000000000800100000000786c2f776f",
    "726b626f6f6b2e786d6c504b010214031400000008000000615d373768e5b8000000790100001a000000000000000000",
    "00008001f6000000786c2f5f72656c732f776f726b626f6f6b2e786d6c2e72656c73504b010214031400000008000000",
    "615d056ca0cdc30000004d0100000d00000000000000000000008001e6010000786c2f7374796c65732e786d6c504b01",
    "0214031400000008000000615d00eef147cc000000150100001400000000000000000000008001d4020000786c2f7368",
    "61726564537472696e67732e786d6c504b010214031400000008000000615dbba7a669c3050000e92400001800000000",
    "000000000000008001d2030000786c2f776f726b7368656574732f7368656574312e786d6c504b010214031400000008",
    "000000615ddaa1d0df91000000bd0000001800000000000000000000008001cb090000786c2f776f726b736865657473",
    "2f7368656574322e786d6c504b010214031400000008000000615da04b6337310100002c0100000e0000000000000000",
    "0000008001920a0000786c2f6d656469612f632e62696e504b010214031400000008000000615d8b7fde3d100000000e",
    "0000001000000000000000000000008001ef0b0000646f6350726f70732f6170702e747874504b050600000000080008",
    "00080200002d0c00000000",
);

/// Byte strings the mutator splices in: ZIP signatures, the XML the reader looks for, and number-format and
/// stored-decimal syntax, so stored mutants reach the format interpreter and not only the tokenizer.
const DICTIONARY: [&[u8]; 28] = [
    b"PK\x01\x02",
    b"PK\x03\x04",
    b"PK\x05\x06",
    b"<c ",
    b"</c>",
    b"<v>",
    b"</v>",
    b"<si>",
    b"<t>",
    b" s=\"",
    b" t=\"s\"",
    b" t=\"e\"",
    b"&amp;",
    b"&lt;",
    b"\"",
    b";",
    b"[",
    b"]",
    b"\\",
    b"_",
    b"%",
    b".",
    b"0",
    b"#,##0",
    b"e999999999999",
    b"E-9223372036854775808",
    b"99999999999999999999999999999999999999999",
    "\u{2014}".as_bytes(),
];

/// Values written over little-endian header fields.
const BOUNDARY: [u32; 8] = [0, 1, 2, 0x7F, 0xFFFF, 0x0001_0000, 0x7FFF_FFFF, 0xFFFF_FFFF];

struct Draw(PhiloxStream);

impl Draw {
    /// Uniform-ish in `0..n`; `0` when `n` is 0.
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.0.next_u64() % n as u64) as usize
    }
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("seed hex"))
        .collect()
}

/// The stored seed: every cell kind the writer has, a format with three sections and a quoted zero token, a
/// percentage, an exponent in a stored decimal, a multi-byte character, and a hidden sheet.
fn stored_seed() -> Vec<u8> {
    let styles = MinimalStyles {
        num_fmts: vec![
            (164, "#,##0.0_);(#,##0.0);\"-\"_)".to_string()),
            (165, "0.00%".to_string()),
        ],
        xfs: vec![0, 164, 165, 4],
    };
    let cell = |r: &str, c: MinimalCell| (r.to_string(), c);
    let summary = MinimalSheet {
        name: "Summary".to_string(),
        hidden: false,
        cells: vec![
            cell(
                "A1",
                MinimalCell::Text("Site \u{2014} North & <East>".to_string()),
            ),
            cell("B1", MinimalCell::Number(1234.5)),
            cell("C1", MinimalCell::Error("#REF!".to_string())),
            cell("A2", MinimalCell::Text("N/A".to_string())),
            cell(
                "B2",
                MinimalCell::Styled {
                    raw: "-37.899642999999998".to_string(),
                    xf: 1,
                },
            ),
            cell(
                "C2",
                MinimalCell::Styled {
                    raw: "0".to_string(),
                    xf: 1,
                },
            ),
            cell(
                "D2",
                MinimalCell::Styled {
                    raw: "0.0425".to_string(),
                    xf: 2,
                },
            ),
            cell(
                "E2",
                MinimalCell::Styled {
                    raw: "1.5E+3".to_string(),
                    xf: 3,
                },
            ),
            cell(
                "F2",
                MinimalCell::Styled {
                    raw: "12345678901".to_string(),
                    xf: 0,
                },
            ),
            cell("AA102", MinimalCell::Number(-0.125)),
        ],
    };
    let workings = MinimalSheet {
        name: "Workings".to_string(),
        hidden: true,
        cells: vec![cell("A1", MinimalCell::Number(7.0))],
    };
    build_minimal_xlsx_styled(&[summary, workings], Some(&styles))
}

/// Offsets just past each ZIP signature, so field overwrites land in headers more often than chance.
fn signature_offsets(bytes: &[u8]) -> Vec<usize> {
    bytes
        .windows(4)
        .enumerate()
        .filter(|(_, w)| {
            w[0] == b'P' && w[1] == b'K' && matches!((w[2], w[3]), (1, 2) | (3, 4) | (5, 6))
        })
        .map(|(i, _)| i)
        .collect()
}

/// The compressed payload of each local file header, by the header's own sizes.
fn payloads(bytes: &[u8]) -> Vec<Range<usize>> {
    signature_offsets(bytes)
        .into_iter()
        .filter(|&i| bytes[i + 2] == 3)
        .map(|i| {
            let u16_at =
                |o: usize| usize::from(u16::from_le_bytes([bytes[i + o], bytes[i + o + 1]]));
            let comp = u32::from_le_bytes(bytes[i + 18..i + 22].try_into().expect("4 bytes"));
            let start = i + 30 + u16_at(26) + u16_at(28);
            start..start + comp as usize
        })
        .collect()
}

struct Seed {
    name: &'static str,
    bytes: Vec<u8>,
    entries: BTreeMap<String, Vec<u8>>,
    /// Deflated payloads; empty for the stored seed.
    payloads: Vec<Range<usize>>,
}

impl Seed {
    fn new(name: &'static str, bytes: Vec<u8>) -> Self {
        let entries = read_zip_entries(&bytes).expect("seed entries");
        let payloads = if name == "deflated" {
            payloads(&bytes)
        } else {
            Vec::new()
        };
        Self {
            name,
            bytes,
            entries,
            payloads,
        }
    }
}

/// One entry's contents mutated, then the archive rebuilt stored, so the container is intact.
fn mutate_entry(seed: &Seed, draw: &mut Draw) -> (Vec<u8>, Vec<String>) {
    let names: Vec<&String> = seed.entries.keys().collect();
    let pick = names[draw.below(names.len())];
    let (body, mut ops) = mutate(&seed.entries[pick], draw, 4);
    ops.insert(0, format!("entry {pick}"));
    let parts: Vec<(&str, &[u8])> = seed
        .entries
        .iter()
        .map(|(n, b)| {
            (
                n.as_str(),
                if n == pick {
                    body.as_slice()
                } else {
                    b.as_slice()
                },
            )
        })
        .collect();
    (write_stored_zip(&parts), ops)
}

/// One to four bytes overwritten inside the deflated payloads; every header and size stays as it was.
fn mutate_stream(seed: &Seed, draw: &mut Draw) -> (Vec<u8>, Vec<String>) {
    let mut m = seed.bytes.clone();
    let mut ops = Vec::new();
    for _ in 0..=draw.below(4) {
        let r = &seed.payloads[draw.below(seed.payloads.len())];
        let at = r.start + draw.below(r.len());
        if draw.below(2) == 0 {
            let bit = draw.below(8);
            m[at] ^= 1 << bit;
            ops.push(format!("stream flip {at}.{bit}"));
        } else {
            let b = draw.below(256) as u8;
            m[at] = b;
            ops.push(format!("stream set {at}={b:#04x}"));
        }
    }
    (m, ops)
}

/// Applies one to `max_ops` mutations anywhere and names them, so a failure says what was done.
fn mutate(seed: &[u8], draw: &mut Draw, max_ops: usize) -> (Vec<u8>, Vec<String>) {
    let mut m = seed.to_vec();
    let mut ops = Vec::new();
    for _ in 0..=draw.below(max_ops) {
        let len = m.len();
        let at = draw.below(len.max(1));
        match draw.below(8) {
            0 if len > 0 => {
                let bit = draw.below(8);
                m[at] ^= 1 << bit;
                ops.push(format!("flip {at}.{bit}"));
            }
            1 if len > 0 => {
                let b = [0x00, 0xFF, 0x7F, 0x80, 0x01, 0xFE][draw.below(6)];
                m[at] = b;
                ops.push(format!("set {at}={b:#04x}"));
            }
            2 => {
                let sigs = signature_offsets(&m);
                if sigs.is_empty() {
                    continue;
                }
                let pos = sigs[draw.below(sigs.len())] + draw.below(46);
                let width = if draw.below(2) == 0 { 2 } else { 4 };
                let v = BOUNDARY[draw.below(BOUNDARY.len())];
                if pos + width <= m.len() {
                    m[pos..pos + width].copy_from_slice(&v.to_le_bytes()[..width]);
                    ops.push(format!("field {pos}/{width}={v:#x}"));
                }
            }
            3 => {
                m.truncate(at);
                ops.push(format!("truncate {at}"));
            }
            4 => {
                let n = 1 + draw.below(16);
                let bytes: Vec<u8> = (0..n).map(|_| draw.below(256) as u8).collect();
                m.splice(at..at, bytes);
                ops.push(format!("insert {at}+{n}"));
            }
            5 if len > 0 => {
                let n = 1 + draw.below(64.min(len - at));
                m.drain(at..at + n);
                ops.push(format!("delete {at}+{n}"));
            }
            6 if len > 0 => {
                let n = 1 + draw.below(64.min(len - at));
                let run = m[at..at + n].to_vec();
                let to = draw.below(len);
                m.splice(to..to, run);
                ops.push(format!("duplicate {at}+{n}->{to}"));
            }
            _ => {
                let token = DICTIONARY[draw.below(DICTIONARY.len())];
                let to = draw.below(len + 1);
                m.splice(to..to, token.iter().copied());
                ops.push(format!("splice {to} {:?}", String::from_utf8_lossy(token)));
            }
        }
    }
    (m, ops)
}

#[derive(Default)]
struct Tally {
    zip_refused: u32,
    /// Of `zip_refused`, the refusals by the inflater.
    inflate_refused: u32,
    workbook_refused: u32,
    workbook_accepted: u32,
    shown: usize,
    refused_cells: usize,
}

/// Reads one mutant the way `fmine` does. Returns a violated bound; a panic propagates to the caller.
fn read(bytes: &[u8], tally: &mut Tally) -> Option<String> {
    match read_zip_entries(bytes) {
        Err(e) => {
            tally.zip_refused += 1;
            tally.inflate_refused += u32::from(matches!(e, ZipError::Inflate(_)));
        }
        Ok(entries) => {
            let total: usize = entries.values().map(Vec::len).sum();
            if let Some((name, v)) = entries.iter().find(|(_, v)| v.len() > MAX_ENTRY_BYTES) {
                return Some(format!("entry {name} holds {} bytes", v.len()));
            }
            if total > MAX_ARCHIVE_BYTES {
                return Some(format!("archive holds {total} bytes"));
            }
        }
    }
    match Workbook::from_bytes("mutant.xlsx", bytes) {
        Err(_) => tally.workbook_refused += 1,
        Ok(wb) => {
            tally.workbook_accepted += 1;
            let _ = wb.cell_census();
            let _ = wb.format_census();
            let census = wb.displayed_census();
            tally.shown += census.shown;
            tally.refused_cells += census.refused;
            for sheet in wb.sheets.iter().chain(&wb.hidden_sheets) {
                for cell in sheet.cells_by_coord.values() {
                    if let Some(Ok(d)) = cell.displayed() {
                        let _ = d.text();
                    }
                }
            }
        }
    }
    None
}

fn run(seed: &Seed, partition: u32) -> Tally {
    let seed_name = seed.name;
    let mut tally = Tally::default();
    for case in 0..CASES {
        // One key per case, so any case regenerates on its own.
        let mut draw = Draw(PhiloxStream::new(RngKey {
            seed_root: 0x0B07_3A11,
            partition,
            domain: 0,
            var_group: 0,
            realization: case,
            brick: 0,
        }));
        let (mutant, ops) = match case % 3 {
            0 => mutate(&seed.bytes, &mut draw, 8),
            2 if !seed.payloads.is_empty() => mutate_stream(seed, &mut draw),
            _ => mutate_entry(seed, &mut draw),
        };
        match catch_unwind(AssertUnwindSafe(|| read(&mutant, &mut tally))) {
            Ok(None) => {}
            Ok(Some(bound)) => panic!("{seed_name} case {case} ({ops:?}): {bound}"),
            Err(_) => panic!(
                "{seed_name} case {case} ({ops:?}): the reader panicked on {} bytes",
                mutant.len()
            ),
        }
    }
    eprintln!(
        "{seed_name}: {CASES} mutants; zip refused {} (inflate {}), workbook refused {}, accepted {}; cells shown \
         {}, refused {}",
        tally.zip_refused,
        tally.inflate_refused,
        tally.workbook_refused,
        tally.workbook_accepted,
        tally.shown,
        tally.refused_cells
    );
    tally
}

#[test]
fn test_both_seeds_parse_before_mutation() {
    let stored = Workbook::from_bytes("stored.xlsx", &stored_seed()).expect("stored seed");
    assert_eq!(stored.sheets.len(), 1);
    assert_eq!(stored.hidden_sheets.len(), 1);
    assert_eq!(stored.total_cells(), 10);
    // Shown: B2 (negative section), D2 (percentage), E2 (exponent). Token: C2 (zero section). Refused: B1 and
    // AA102 (General non-integer), F2 (General, 11 digits).
    let census = stored.displayed_census();
    assert_eq!((census.shown, census.token, census.refused), (3, 1, 3));

    let deflated =
        Workbook::from_bytes("deflated.xlsx", &hex(DEFLATED_HEX)).expect("deflated seed");
    assert_eq!(deflated.sheets.len(), 1);
    assert_eq!(deflated.sheets[0].name, "Summary");
    assert_eq!(deflated.hidden_sheets.len(), 1);
    assert_eq!(deflated.total_cells(), 3 + 68 * 3 + 4);
    assert_eq!(
        deflated
            .sheet("Summary")
            .and_then(|s| s.cell(1, 3))
            .and_then(|c| c.value.as_text()),
        Some("AISC &lt;$/oz>")
    );
    let census = deflated.displayed_census();
    assert_eq!((census.shown, census.token, census.refused), (136, 0, 0));
    let seed = Seed::new("deflated", hex(DEFLATED_HEX));
    assert_eq!(seed.entries.len(), 8);
    assert_eq!(seed.entries["xl/media/c.bin"].len(), 300);
    assert_eq!(seed.entries["docProps/app.txt"], b"synthetic seed");
    // One payload per entry, ending where the next header starts or where the central directory does.
    assert_eq!(seed.payloads.len(), 8);
    assert_eq!(seed.payloads.iter().map(Range::len).sum::<usize>(), 2_725);
}

/// Each seed's mutants must be both refused and accepted at least one time in twenty, and the deflated seed's
/// must reach the inflater's own refusals as often, so the search is not vacuous: a mutator that only ever broke
/// the end-of-central-directory record would pass the no-panic check while testing almost nothing. Both must
/// reach shown cells and display refusals, so the number-format interpreter sees mutated codes and decimals.
#[test]
fn test_mutants_never_panic_and_stay_within_the_bounds() {
    for (seed, partition) in [
        (Seed::new("stored", stored_seed()), 1),
        (Seed::new("deflated", hex(DEFLATED_HEX)), 2),
    ] {
        let name = seed.name;
        let t = run(&seed, partition);
        let floor = CASES / 20;
        assert!(
            t.zip_refused >= floor,
            "{name}: too few zip refusals ({})",
            t.zip_refused
        );
        assert!(
            t.workbook_accepted >= floor,
            "{name}: too few accepted ({})",
            t.workbook_accepted
        );
        assert!(
            t.shown > 0 && t.refused_cells > 0,
            "{name}: display outcomes not reached"
        );
        if !seed.payloads.is_empty() {
            assert!(
                t.inflate_refused >= floor,
                "{name}: too few inflate refusals ({})",
                t.inflate_refused
            );
        }
    }
}
