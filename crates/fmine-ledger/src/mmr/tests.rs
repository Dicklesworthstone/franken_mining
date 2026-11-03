//! Known answers and published probes for the MMR (BD-083).
//!
//! `LEAF_INPUTS`, `NODE_HASHES`, `ROOTS` and `PEAKS` are transcribed from transparency-dev/merkle
//! `testonly/constants.go` (`LeafInputs`, `NodeHashes`, `RootHashes`, `CompactTrees`) at commit
//! c39fbf350a405f68571bf4c42a6a518eb0a46a4b, Apache-2.0. The probe table is the same commit's
//! `testdata/inclusion` and `testdata/consistency`, converted by `scripts/oracle/ct_merkle_probes.py`
//! (fixtures/fmine-ledger/rfc6962_probes.txt, capture record beside it).

use std::collections::BTreeMap;

use super::*;

const PROBES: &str = include_str!("../../../../fixtures/fmine-ledger/rfc6962_probes.txt");

const LEAF_INPUTS: [&str; 8] = [
    "",
    "00",
    "10",
    "2021",
    "3031",
    "40414243",
    "5051525354555657",
    "606162636465666768696a6b6c6d6e6f",
];

/// `NODE_HASHES[h][i]`: the complete subtree of height `h` at position `i` over `LEAF_INPUTS`.
const NODE_HASHES: [&[&str]; 4] = [
    &[
        "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
        "96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7",
        "0298d122906dcfc10892cb53a73992fc5b9f493ea4c9badb27b791b4127a7fe7",
        "07506a85fd9dd2f120eb694f86011e5bb4662e5c415a62917033d4a9624487e7",
        "bc1a0643b12e4d2d7c77918f44e0f4f79a838b6cf9ec5b5c283e1f4d88599e6b",
        "4271a26be0d8a84f0bd54c8c302e7cb3a3b5d1fa6780a40bcce2873477dab658",
        "b08693ec2e721597130641e8211e7eedccb4c26413963eee6c1e2ed16ffb1a5f",
        "46f6ffadd3d06a09ff3c5860d2755c8b9819db7df44251788c7d8e3180de8eb1",
    ],
    &[
        "fac54203e7cc696cf0dfcb42c92a1d9dbaf70ad9e621f4bd8d98662f00e3c125",
        "5f083f0a1a33ca076a95279832580db3e0ef4584bdff1f54c8a360f50de3031e",
        "0ebc5d3437fbe2db158b9f126a1d118e308181031d0a949f8dededebc558ef6a",
        "ca854ea128ed050b41b35ffc1b87b8eb2bde461e9e3b5596ece6b9d5975a0ae0",
    ],
    &[
        "d37ee418976dd95753c1c73862b9398fa2a2cf9b4ff0fdfe8b30cd95209614b7",
        "6b47aaf29ee3c2af9af889bc1fb9254dabd31177f16232dd6aab035ca39bf6e4",
    ],
    &["5dc9da79a70659a9ad559cb701ded9a2ab9d823aad2f4960cfe370eff4604328"],
];

/// `ROOTS[n]`: the root of the first `n` of `LEAF_INPUTS`.
const ROOTS: [&str; 9] = [
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
    "fac54203e7cc696cf0dfcb42c92a1d9dbaf70ad9e621f4bd8d98662f00e3c125",
    "aeb6bcfe274b70a14fb067a5e5578264db0fa9b51af5e0ba159158f329e06e77",
    "d37ee418976dd95753c1c73862b9398fa2a2cf9b4ff0fdfe8b30cd95209614b7",
    "4e3bbb1f7b478dcfe71fb631631519a3bca12c9aefca1612bfce4c13a86264d4",
    "76e67dadbcdf1e10e1b74ddc608abd2f98dfb16fbce75277b5232a127f2087ef",
    "ddb89be403809e325750d3d263cd78929c2942b7942a34b77e122c9594a74c8c",
    "5dc9da79a70659a9ad559cb701ded9a2ab9d823aad2f4960cfe370eff4604328",
];

/// `PEAKS[n]`: the peaks of the first `n` leaves as `(height, index)` into `NODE_HASHES`, largest first.
const PEAKS: [&[(usize, usize)]; 9] = [
    &[],
    &[(0, 0)],
    &[(1, 0)],
    &[(1, 0), (0, 2)],
    &[(2, 0)],
    &[(2, 0), (0, 4)],
    &[(2, 0), (1, 2)],
    &[(2, 0), (1, 2), (0, 6)],
    &[(3, 0)],
];

fn bytes(hex: &str) -> Vec<u8> {
    if hex == "-" {
        return Vec::new();
    }
    assert!(hex.len().is_multiple_of(2), "odd hex {hex}");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn digest(raw: &[u8]) -> Option<Digest32> {
    raw.try_into().ok().map(Digest32)
}

fn d(hex: &str) -> Digest32 {
    digest(&bytes(hex)).unwrap()
}

fn known_log() -> Mmr {
    let mut mmr = Mmr::new();
    for (i, input) in LEAF_INPUTS.iter().enumerate() {
        assert_eq!(mmr.push(&bytes(input)), Ok(i as u64));
    }
    mmr
}

/// One probe line: kind, path, two sizes, two hashes, the proof's hashes and whether it must verify.
struct Probe<'a> {
    kind: &'a str,
    path: &'a str,
    a: u64,
    b: u64,
    h1: Vec<u8>,
    h2: Vec<u8>,
    proof: Vec<Vec<u8>>,
    ok: bool,
}

fn probes() -> Vec<Probe<'static>> {
    PROBES
        .lines()
        .map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            let &[kind, path, a, b, h1, h2, proof, want] = fields.as_slice() else {
                panic!("bad probe line {line}");
            };
            Probe {
                kind,
                path,
                a: a.parse().unwrap(),
                b: b.parse().unwrap(),
                h1: bytes(h1),
                h2: bytes(h2),
                proof: match proof {
                    "none" => Vec::new(),
                    list => list.split(',').map(bytes).collect(),
                },
                ok: match want {
                    "ok" => true,
                    "err" => false,
                    other => panic!("bad want {other} in {path}"),
                },
            }
        })
        .collect()
}

#[test]
fn leaves_levels_peaks_and_roots_match_the_rfc6962_known_answers() {
    let mut mmr = Mmr::new();
    assert_eq!(mmr.root(), d(ROOTS[0]));
    assert_eq!(empty_root(), d(ROOTS[0]));
    for (i, input) in LEAF_INPUTS.iter().enumerate() {
        assert_eq!(leaf_hash(&bytes(input)), d(NODE_HASHES[0][i]), "leaf {i}");
        mmr.push(&bytes(input)).unwrap();
        assert_eq!(mmr.root(), d(ROOTS[i + 1]), "root after {} pushes", i + 1);
    }
    let levels: Vec<Vec<Digest32>> = NODE_HASHES
        .iter()
        .map(|level| level.iter().map(|h| d(h)).collect())
        .collect();
    assert_eq!(mmr.levels, levels);
    for (n, root) in ROOTS.iter().enumerate() {
        let n64 = n as u64;
        let peaks: Vec<Digest32> = PEAKS[n]
            .iter()
            .map(|&(h, i)| d(NODE_HASHES[h][i]))
            .collect();
        assert_eq!(mmr.peaks_at(n64), Ok(peaks.clone()), "peaks of {n}");
        assert_eq!(root_from_peaks(&peaks), d(root), "bagged peaks of {n}");
        assert_eq!(mmr.root_at(n64), Ok(d(root)), "root at {n}");
    }
}

#[test]
fn generated_proofs_equal_the_published_happy_paths() {
    let mmr = known_log();
    let mut seen = 0;
    for p in probes()
        .iter()
        .filter(|p| p.path.ends_with("/happy-path.json"))
    {
        assert!(p.ok, "{}", p.path);
        let proof: Vec<Digest32> = p.proof.iter().map(|h| digest(h).unwrap()).collect();
        let (h1, h2) = (digest(&p.h1).unwrap(), digest(&p.h2).unwrap());
        match p.kind {
            "inclusion" => {
                let leaf = LEAF_INPUTS[usize::try_from(p.a).unwrap()];
                assert_eq!(leaf_hash(&bytes(leaf)), h1, "{}", p.path);
                assert_eq!(mmr.root_at(p.b), Ok(h2), "{}", p.path);
                assert_eq!(mmr.inclusion_proof(p.a, p.b), Ok(proof), "{}", p.path);
            }
            "consistency" => {
                assert_eq!(mmr.root_at(p.a), Ok(h1), "{}", p.path);
                assert_eq!(mmr.root_at(p.b), Ok(h2), "{}", p.path);
                assert_eq!(mmr.consistency_proof(p.a, p.b), Ok(proof), "{}", p.path);
            }
            other => panic!("kind {other}"),
        }
        seen += 1;
    }
    assert_eq!(seen, 10);
}

#[test]
fn published_probes_verify_or_refuse_as_upstream_says() {
    let all = probes();
    assert_eq!(all.len(), 196);
    let mut checked = BTreeMap::<(&str, bool), usize>::new();
    let mut reasons = BTreeMap::<String, usize>::new();
    let mut untyped_refused = 0;
    let mut untyped_ok = Vec::new();
    for p in &all {
        let proof: Option<Vec<Digest32>> = p.proof.iter().map(|h| digest(h)).collect();
        let (Some(h1), Some(h2), Some(proof)) = (digest(&p.h1), digest(&p.h2), proof) else {
            // A hash that is not 32 bytes has no `Digest32`, so the API cannot be handed it at all.
            if p.ok {
                untyped_ok.push(p.path);
            } else {
                untyped_refused += 1;
            }
            continue;
        };
        let got = match p.kind {
            "inclusion" => verify_inclusion(p.a, p.b, &h1, &h2, &proof),
            "consistency" => verify_consistency(p.a, p.b, &h1, &h2, &proof),
            other => panic!("kind {other}"),
        };
        assert_eq!(got.is_ok(), p.ok, "{}: {got:?}", p.path);
        *checked.entry((p.kind, p.ok)).or_default() += 1;
        if let Err(e) = got {
            let name = format!("{e:?}");
            let name = name
                .split([' ', '{'])
                .next()
                .unwrap_or_default()
                .to_string();
            *reasons.entry(name).or_default() += 1;
        }
    }
    eprintln!(
        "probes: checked {checked:?}; refusals {reasons:?}; untyped refused {untyped_refused}; untyped ok {untyped_ok:?}"
    );
    // Counted from the table by a separate script: 53 probes hold a hash that is not 32 bytes, one of them `ok`.
    assert_eq!(untyped_refused, 52);
    assert_eq!(
        untyped_ok,
        ["consistency/additional/sizes-are-equal-one-and-proof-is-empty.json"]
    );
    assert_eq!(checked.values().sum::<usize>(), 143);
    assert_eq!(
        checked.get(&("inclusion", true)).copied().unwrap_or(0)
            + checked.get(&("consistency", true)).copied().unwrap_or(0),
        11
    );
    // That probe's case, equal sizes with an empty proof and equal roots, with a 32-byte root in place of its
    // 12-byte "don't care 2".
    let root = sha256(b"don't care 2");
    assert_eq!(verify_consistency(1, 1, &root, &root, &[]), Ok(()));
}

/// RFC 6962 §2.1 straight from its definitions over a slice of leaf hashes, with its own split.
mod reference {
    use super::*;

    fn k_of(n: usize) -> usize {
        let mut k = 1;
        while k * 2 < n {
            k *= 2;
        }
        k
    }

    pub fn mth(d: &[Digest32]) -> Digest32 {
        match d {
            [] => sha256(&[]),
            [one] => *one,
            _ => {
                let k = k_of(d.len());
                mmr_parent(&mth(&d[..k]), &mth(&d[k..]))
            }
        }
    }

    pub fn path(m: usize, d: &[Digest32]) -> Vec<Digest32> {
        if d.len() == 1 {
            return Vec::new();
        }
        let k = k_of(d.len());
        let (mut p, sibling) = if m < k {
            (path(m, &d[..k]), mth(&d[k..]))
        } else {
            (path(m - k, &d[k..]), mth(&d[..k]))
        };
        p.push(sibling);
        p
    }

    pub fn subproof(m: usize, d: &[Digest32], whole: bool) -> Vec<Digest32> {
        if m == d.len() {
            return if whole { Vec::new() } else { vec![mth(d)] };
        }
        let k = k_of(d.len());
        let (mut p, sibling) = if m <= k {
            (subproof(m, &d[..k], whole), mth(&d[k..]))
        } else {
            (subproof(m - k, &d[k..], false), mth(&d[..k]))
        };
        p.push(sibling);
        p
    }
}

#[test]
fn every_tree_up_to_64_leaves_matches_the_definitions_and_verifies() {
    let mut mmr = Mmr::new();
    let mut leaves = Vec::new();
    for i in 0..64u64 {
        let entry = sha256(&i.to_le_bytes());
        mmr.push(entry.as_bytes()).unwrap();
        leaves.push(leaf_hash(entry.as_bytes()));
    }
    let (mut worst_inclusion, mut worst_consistency) = (0i64, (0i64, 0, 0));
    for n in 0..=64usize {
        let n64 = n as u64;
        let root = reference::mth(&leaves[..n]);
        assert_eq!(mmr.root_at(n64), Ok(root), "root at {n}");
        // ⌈log₂ n⌉, and 0 for one leaf.
        let log2 = i64::from(u64::BITS - n64.saturating_sub(1).leading_zeros());
        for m in 0..n {
            let proof = mmr.inclusion_proof(m as u64, n64).unwrap();
            assert_eq!(proof, reference::path(m, &leaves[..n]), "path {m} of {n}");
            assert_eq!(
                verify_inclusion(m as u64, n64, &leaves[m], &root, &proof),
                Ok(()),
                "inclusion {m} of {n}"
            );
            worst_inclusion = worst_inclusion.max(proof.len() as i64 - log2);
        }
        for m in 1..=n {
            let first = reference::mth(&leaves[..m]);
            let proof = mmr.consistency_proof(m as u64, n64).unwrap();
            let want = if m == n {
                Vec::new()
            } else {
                reference::subproof(m, &leaves[..n], true)
            };
            assert_eq!(proof, want, "consistency {m} to {n}");
            assert_eq!(
                verify_consistency(m as u64, n64, &first, &root, &proof),
                Ok(()),
                "consistency {m} to {n}"
            );
            worst_consistency = worst_consistency.max((proof.len() as i64 - log2, m, n));
        }
    }
    // An inclusion proof takes at most ⌈log₂ n⌉ hashes. A consistency proof takes up to one more: 63 to 64 takes 7.
    assert_eq!(worst_inclusion, 0);
    assert_eq!(worst_consistency, (1, 63, 64));
}

fn flipped(h: &Digest32, bit: usize) -> Digest32 {
    let mut out = *h;
    out.0[bit / 8] ^= 1 << (bit % 8);
    out
}

#[test]
fn a_changed_hash_or_a_resized_proof_is_refused() {
    let mut mmr = Mmr::new();
    for i in 0..20u64 {
        mmr.push(&i.to_be_bytes()).unwrap();
    }
    let extra = sha256(b"extra");
    let mut cases = 0;
    for n in 1..=20u64 {
        let root = mmr.root_at(n).unwrap();
        for m in 0..n {
            let leaf = leaf_hash(&m.to_be_bytes());
            let proof = mmr.inclusion_proof(m, n).unwrap();
            let check = |leaf: &Digest32, root: &Digest32, proof: &[Digest32]| {
                verify_inclusion(m, n, leaf, root, proof)
            };
            assert_eq!(check(&leaf, &root, &proof), Ok(()));
            for bit in [0, 77, 255] {
                assert_eq!(
                    check(&flipped(&leaf, bit), &root, &proof),
                    Err(MmrError::RootMismatch)
                );
                assert_eq!(
                    check(&leaf, &flipped(&root, bit), &proof),
                    Err(MmrError::RootMismatch)
                );
                for i in 0..proof.len() {
                    let mut bad = proof.clone();
                    bad[i] = flipped(&bad[i], bit);
                    assert_eq!(check(&leaf, &root, &bad), Err(MmrError::RootMismatch));
                    cases += 1;
                }
            }
            let mut longer = proof.clone();
            longer.push(extra);
            assert_eq!(check(&leaf, &root, &longer), Err(MmrError::ProofTooLong));
            if let Some((_, shorter)) = proof.split_last() {
                assert_eq!(check(&leaf, &root, shorter), Err(MmrError::ProofTooShort));
            }
            cases += 2;
        }
        for m in 1..n {
            let first = mmr.root_at(m).unwrap();
            let proof = mmr.consistency_proof(m, n).unwrap();
            let check = |a: &Digest32, b: &Digest32, proof: &[Digest32]| {
                verify_consistency(m, n, a, b, proof)
            };
            assert_eq!(check(&first, &root, &proof), Ok(()));
            for bit in [0, 130, 255] {
                assert!(
                    check(&flipped(&first, bit), &root, &proof).is_err(),
                    "first root {m} to {n}"
                );
                assert_eq!(
                    check(&first, &flipped(&root, bit), &proof),
                    Err(MmrError::SecondRootMismatch)
                );
                for i in 0..proof.len() {
                    let mut bad = proof.clone();
                    bad[i] = flipped(&bad[i], bit);
                    assert!(
                        check(&first, &root, &bad).is_err(),
                        "proof hash {i} of {m} to {n}"
                    );
                    cases += 1;
                }
            }
            assert!(
                check(&root, &first, &proof).is_err(),
                "swapped roots {m} to {n}"
            );
            let mut longer = proof.clone();
            longer.push(extra);
            assert_eq!(check(&first, &root, &longer), Err(MmrError::ProofTooLong));
            if let Some((_, shorter)) = proof.split_last() {
                assert!(check(&first, &root, shorter).is_err(), "shorter {m} to {n}");
            }
            cases += 3;
        }
    }
    eprintln!("tamper cases refused: {cases}");
}

#[test]
fn sizes_outside_the_log_or_the_proof_rules_are_refused() {
    let mmr = known_log();
    let root = mmr.root();
    let beyond = MmrError::BeyondLog { size: 9, len: 8 };
    assert_eq!(mmr.root_at(9), Err(beyond));
    assert_eq!(mmr.peaks_at(9), Err(beyond));
    assert_eq!(mmr.inclusion_proof(0, 9), Err(beyond));
    assert_eq!(mmr.consistency_proof(1, 9), Err(beyond));
    assert_eq!(
        mmr.inclusion_proof(8, 8),
        Err(MmrError::IndexOutOfRange { index: 8, size: 8 })
    );
    assert_eq!(
        mmr.inclusion_proof(0, 0),
        Err(MmrError::IndexOutOfRange { index: 0, size: 0 })
    );
    assert_eq!(mmr.consistency_proof(0, 8), Err(MmrError::EmptyFirst));
    assert_eq!(
        mmr.consistency_proof(5, 4),
        Err(MmrError::FirstAfterSecond {
            first: 5,
            second: 4
        })
    );
    assert_eq!(mmr.consistency_proof(8, 8), Ok(Vec::new()));

    let leaf = leaf_hash(&[]);
    assert_eq!(
        verify_inclusion(8, 8, &leaf, &root, &[]),
        Err(MmrError::IndexOutOfRange { index: 8, size: 8 })
    );
    assert_eq!(
        verify_inclusion(0, 0, &leaf, &root, &[]),
        Err(MmrError::IndexOutOfRange { index: 0, size: 0 })
    );
    assert_eq!(
        verify_consistency(0, 8, &empty_root(), &root, &[]),
        Err(MmrError::EmptyFirst)
    );
    assert_eq!(
        verify_consistency(5, 4, &root, &root, &[]),
        Err(MmrError::FirstAfterSecond {
            first: 5,
            second: 4
        })
    );
    assert_eq!(
        verify_consistency(8, 8, &root, &root, &[root]),
        Err(MmrError::ProofTooLong)
    );
    assert_eq!(
        verify_consistency(8, 8, &root, &empty_root(), &[]),
        Err(MmrError::SecondRootMismatch)
    );
    assert_eq!(
        verify_consistency(4, 8, &mmr.root_at(4).unwrap(), &root, &[]),
        Err(MmrError::ProofTooShort)
    );
    assert_eq!(
        verify_consistency(3, 8, &mmr.root_at(3).unwrap(), &root, &[]),
        Err(MmrError::ProofTooShort)
    );

    let mut full = Mmr {
        levels: Vec::new(),
        len: u64::MAX,
    };
    assert_eq!(full.push(b"one more"), Err(MmrError::Full));
    assert_eq!((full.len(), full.levels.len()), (u64::MAX, 0));
}
