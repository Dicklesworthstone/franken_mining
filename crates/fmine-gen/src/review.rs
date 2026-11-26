//! Determinism: BitwisePortable
//! Salted internal-review catch-rate estimator (exact Clopper–Pearson via transcendental-free
//! binomial-tail bisection) and BLAKE3 commitment/verification (`BD-357.S1` / `bd-kftl.1`,
//! plan WP-00).
//!
//! Safety & anti-leak boundary (`BD-357`):
//! - `SeedClass` deliberately has NO basis-switch variant: an issuer-attributed false figure is
//!   never placed before an issuer reader.
//! - `generate_os_nonce` reads 32 bytes from `/dev/urandom` (never Philox or any committed repo
//!   seed) so candidate seed placements cannot be brute-forced from repository state.
//! - `clopper_pearson` uses strictly `+`, `-`, `*`, `/` and comparisons (ZERO std transcendentals,
//!   so `xtask check-det` certifies `BitwisePortable`).

use std::collections::BTreeSet;
use std::fmt;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use fmine_core::Digest32;
use fmine_core::hash::{blake3_hash, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};

/// Maximum `n` for which every binomial coefficient `C(n, k)` fits exactly in IEEE-754 `f64`
/// (`C(50, 25) = 126_410_606_437_752 < 2^53 = 9_007_199_254_740_992`).
pub const MAX_EXACT_CLOPPER_PEARSON_N: u32 = 50;

/// Fixed bisection iteration count (`2^-100 ≈ 7.9e-31`, well below `f64` precision).
const BISECTION_ITERATIONS: usize = 100;

/// Canonical domain-separation prefix for `ReviewKey` commitments (`BD-357.S1`).
const REVIEW_KEY_DOMAIN_PREFIX: &[u8] = b"fmine:review-key:v1\0";

/// Domain-separation prefix for nonce-keyed packet shuffling (`BD-357.S2`).
const REVIEW_SHUFFLE_DOMAIN_PREFIX: &[u8] = b"fmine:review-shuffle:v1\0";

/// Domain-separation prefix for `PacketManifest` SHA-256 hashing (`BD-357.S2`).
const REVIEW_PACKET_DOMAIN_PREFIX: &[u8] = b"fmine:review-packet:v1\0";

/// Filename written at the root of a materialized review packet directory (`BD-357.S2`).
pub const REVIEW_PACKET_MANIFEST: &str = "review-packet.toml";

/// Permitted internal-review seed classes (`BD-357.S1` / `BD-357.S2`).
///
/// Intentionally omits any basis-switch class so no issuer-attributed false number can ever be
/// synthesized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SeedClass {
    S1CloseCitesNonExercisingTest = 1,
    S2CloseCitesNonPassRecord = 2,
    S3MisCitedCell = 3,
    S4RefusedShownReproduced = 4,
    S5StaleShownCurrent = 5,
    S6WrongCauseClass = 6,
}

impl SeedClass {
    /// Stable single-byte canonical encoding for commitment hashing.
    #[must_use]
    pub const fn tag_byte(self) -> u8 {
        self as u8
    }
}

/// Exact integer power by repeated squaring using only `*` (no `.powi` / `.powf` / `.ln` / `.exp`).
fn pow_u32(mut base: f64, mut exp: u32) -> f64 {
    let mut acc = 1.0;
    while exp > 0 {
        if (exp & 1) == 1 {
            acc *= base;
        }
        exp >>= 1;
        if exp > 0 {
            base *= base;
        }
    }
    acc
}

/// Exact binomial coefficient `C(n, k)` as `f64` for `0 <= k <= n <= MAX_EXACT_CLOPPER_PEARSON_N`.
fn binom_coeff_exact_f64(n: u32, k: u32) -> f64 {
    let k = k.min(n - k);
    let mut acc: u128 = 1;
    let n128 = u128::from(n);
    for i in 0..u128::from(k) {
        acc = acc * (n128 - i) / (i + 1);
    }
    acc as f64
}

/// Binomial probability mass `P(X = k | n, p)` using only `+ - * /`.
fn binom_pmf(k: u32, n: u32, p: f64) -> f64 {
    binom_coeff_exact_f64(n, k) * pow_u32(p, k) * pow_u32(1.0 - p, n - k)
}

/// Upper binomial tail `P(X >= x | n, p)` (strictly increasing in `p` on `(0, 1)` for `1 <= x <= n`).
fn binom_upper_tail(x: u32, n: u32, p: f64) -> f64 {
    let mut sum = 0.0;
    for k in x..=n {
        sum += binom_pmf(k, n, p);
    }
    sum
}

/// Lower binomial tail `P(X <= x | n, p)` (strictly decreasing in `p` on `(0, 1)` for `0 <= x < n`).
fn binom_lower_tail(x: u32, n: u32, p: f64) -> f64 {
    let mut sum = 0.0;
    for k in 0..=x {
        sum += binom_pmf(k, n, p);
    }
    sum
}

/// Exact two-sided `(1 - alpha)` Clopper–Pearson confidence interval for `x` catches out of `n`
/// trials (`BD-357.S1`).
///
/// Computes `[B(alpha/2; x, n - x + 1), B(1 - alpha/2; x + 1, n - x)]` by exact binomial-tail
/// inversion using 100 iterations of bisection on `[0, 1]` with strictly `+ - * /` and comparisons.
///
/// Edge conventions:
/// - `x == 0` yields an exact `0.0` lower bound;
/// - `x == n` yields an exact `1.0` upper bound.
///
/// Refuses (never clamps) with `FMINE-REF-109` when `n == 0`, `x > n`, `n > MAX_EXACT_CLOPPER_PEARSON_N`,
/// or `alpha` is non-finite or not strictly in `(0, 1)`.
pub fn clopper_pearson(x: u32, n: u32, alpha: f64) -> Result<(f64, f64), Refusal> {
    if n == 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "clopper_pearson: trial count n must be >= 1, got 0",
        ));
    }
    if x > n {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("clopper_pearson: catches x={x} exceeds trial count n={n}"),
        ));
    }
    if n > MAX_EXACT_CLOPPER_PEARSON_N {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "clopper_pearson: n={n} exceeds exact f64 binomial coefficient bound \
                 MAX_EXACT_CLOPPER_PEARSON_N={MAX_EXACT_CLOPPER_PEARSON_N}"
            ),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("clopper_pearson: alpha must be finite and strictly in (0, 1), got {alpha}"),
        ));
    }

    let half_alpha = 0.5 * alpha;
    if half_alpha <= 0.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("clopper_pearson: alpha/2 underflowed to 0 for alpha={alpha}"),
        ));
    }

    let lower = if x == 0 {
        0.0
    } else {
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        for _ in 0..BISECTION_ITERATIONS {
            let mid = 0.5 * (lo + hi);
            if binom_upper_tail(x, n, mid) < half_alpha {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };

    let upper = if x == n {
        1.0
    } else {
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        for _ in 0..BISECTION_ITERATIONS {
            let mid = 0.5 * (lo + hi);
            if binom_lower_tail(x, n, mid) > half_alpha {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };

    Ok((lower, upper))
}

/// Sealed answer key for a salted internal review round (`BD-357.S1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewKey {
    pub round_id: String,
    pub nonce: [u8; 32],
    pub items: Vec<(String, SeedClass)>,
}

impl ReviewKey {
    /// Validate invariants before serialization or commitment:
    /// - `round_id` non-empty;
    /// - `nonce` not all-zero;
    /// - every `item_id` non-empty and unique.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.round_id.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: round_id must be non-empty",
            ));
        }
        if self.nonce == [0u8; 32] {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: all-zero nonce is refused; draw 32 bytes from OS entropy",
            ));
        }
        let mut seen = BTreeSet::new();
        for (item_id, _) in &self.items {
            if item_id.trim().is_empty() {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "ReviewKey: item_id must be non-empty",
                ));
            }
            if !seen.insert(item_id.as_str()) {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("ReviewKey: duplicate item_id '{item_id}'"),
                ));
            }
        }
        Ok(())
    }

    /// Encode canonical bytes independent of `self.items` input ordering (`BD-357.S1`):
    /// `"fmine:review-key:v1\0" || u32_le(round_id.len()) || round_id || nonce(32) || u32_le(items.len()) || sorted(u32_le(id.len()) || id || class_byte)`.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Refusal> {
        self.validate()?;

        let round_len = u32::try_from(self.round_id.len()).map_err(|_| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: round_id length exceeds u32::MAX",
            )
        })?;
        let item_count = u32::try_from(self.items.len()).map_err(|_| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "ReviewKey: item count exceeds u32::MAX",
            )
        })?;

        let mut sorted_items: Vec<(&str, SeedClass)> = self
            .items
            .iter()
            .map(|(id, cls)| (id.as_str(), *cls))
            .collect();
        sorted_items.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

        let mut out = Vec::new();
        out.extend_from_slice(REVIEW_KEY_DOMAIN_PREFIX);
        out.extend_from_slice(&round_len.to_le_bytes());
        out.extend_from_slice(self.round_id.as_bytes());
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&item_count.to_le_bytes());
        for (item_id, cls) in sorted_items {
            let id_len = u32::try_from(item_id.len()).map_err(|_| {
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "ReviewKey: item_id length exceeds u32::MAX",
                )
            })?;
            out.extend_from_slice(&id_len.to_le_bytes());
            out.extend_from_slice(item_id.as_bytes());
            out.push(cls.tag_byte());
        }
        Ok(out)
    }
}

/// Compute the BLAKE3 commitment digest for a validated [`ReviewKey`] (`BD-357.S1`).
pub fn commit(key: &ReviewKey) -> Result<Digest32, Refusal> {
    let bytes = key.canonical_bytes()?;
    Ok(blake3_hash(&bytes))
}

/// Verify a [`ReviewKey`] against a previously published BLAKE3 commitment digest (`BD-357.S1`).
///
/// Refuses with `FMINE-REF-505` (`DigestMismatch505`) if the digest differs, or `FMINE-REF-109` if
/// the key itself is malformed (e.g. all-zero nonce).
pub fn verify(key: &ReviewKey, commitment: &Digest32) -> Result<(), Refusal> {
    let computed = commit(key)?;
    let diff = computed
        .as_bytes()
        .iter()
        .zip(commitment.as_bytes().iter())
        .fold(0u8, |acc, (&a, &b)| acc | (a ^ b));
    if diff == 0 {
        Ok(())
    } else {
        Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "ReviewKey commitment mismatch for round '{}': expected {}, computed {}",
                key.round_id,
                commitment.to_hex(),
                computed.to_hex()
            ),
        ))
    }
}

/// Draw a fresh 32-byte commitment nonce from OS entropy (`/dev/urandom`).
///
/// Never uses `fmine_core::rand::PhiloxStream` or any committed repository seed (`BD-357.S1`).
pub fn generate_os_nonce() -> Result<[u8; 32], Refusal> {
    let mut file = std::fs::File::open("/dev/urandom").map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("unable to open /dev/urandom for review commitment nonce: {e}"),
        )
    })?;
    let mut nonce = [0u8; 32];
    file.read_exact(&mut nonce).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("unable to read 32 bytes from /dev/urandom: {e}"),
        )
    })?;
    if nonce == [0u8; 32] {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "OS entropy source returned an all-zero nonce; refusing to commit",
        ));
    }
    Ok(nonce)
}

/// Whether a materialized internal-review packet contains seeded errors (`BD-357.S2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    Salted,
    Unsalted,
}

impl PacketKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Salted => "salted",
            Self::Unsalted => "unsalted",
        }
    }
}

/// Manifest written as `review-packet.toml` at the root of a review packet directory (`BD-357.S2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketManifest {
    pub packet: PacketKind,
    pub round_id: String,
    pub item_ids: Vec<String>,
    pub sha256: Digest32,
}

fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let code = c as u32;
                out.push_str(&format!("\\u{code:04x}"));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl PacketManifest {
    /// Render deterministic `review-packet.toml` contents consumed by `fmine_public::refuse_if_salted`.
    #[must_use]
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "packet = {}\n",
            quote_string(self.packet.as_str())
        ));
        out.push_str(&format!("round_id = {}\n", quote_string(&self.round_id)));
        out.push_str(&format!(
            "sha256 = {}\n",
            quote_string(&self.sha256.to_hex())
        ));
        out.push_str("item_ids = [");
        for (idx, id) in self.item_ids.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&quote_string(id));
        }
        out.push_str("]\n");
        out
    }
}

/// Isolated seeded item inside a [`ReviewPacket`] (`BD-357.S2`, `RISK-32`).
///
/// All fields are private. `SaltedItem` implements no `Display`, no `From`/`Into` conversion into
/// `fmine_claim::Claim`, workbook cell types, or `fmine_cli::Envelope`, and exposes no `as_claim()`.
///
/// # Compile-time isolation proofs (`BD-357.S2`)
///
/// `salted_item_is_not_a_claim`: a `SaltedItem` cannot be passed to `fmine_claim::Claim::new`.
/// ```compile_fail
/// use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
/// use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
/// use fmine_gen::review::SaltedItem;
///
/// fn leak_into_claim(item: SaltedItem) -> Claim<f64> {
///     Claim::new(
///         "gold.aisc",
///         item,
///         "1500",
///         Representation::Deterministic,
///         SpatialSupport::PeriodAggregate,
///         AdditivityClass::Ratio,
///         EvidenceClass::ISSUER,
///         EpistemicStatus::KnownKnown,
///         KnowledgeState::Known,
///         Confidentiality::public(),
///         vec![],
///         vec![],
///         vec![],
///     )
/// }
/// ```
///
/// `salted_item_is_not_a_workbook_cell`: a `SaltedItem` cannot be passed to
/// `fmine_public::cell_to_public_claim`.
/// ```compile_fail
/// use fmine_claim::AdditivityClass;
/// use fmine_gen::review::SaltedItem;
/// use fmine_io::xlsx::Workbook;
///
/// fn leak_into_cell_claim(wb: &Workbook, item: &SaltedItem) {
///     let _ = fmine_public::cell_to_public_claim(
///         wb,
///         "QTD",
///         "gold.aisc",
///         item,
///         AdditivityClass::Ratio,
///     );
/// }
/// ```
///
/// `salted_item_is_not_an_envelope`: a `SaltedItem` cannot be passed to `fmine_cli::Envelope::ok`.
/// ```compile_fail
/// use fmine_gen::review::SaltedItem;
///
/// fn leak_into_envelope(item: SaltedItem) -> fmine_cli::Envelope {
///     fmine_cli::Envelope::ok("assure public", item, vec![])
/// }
/// ```
///
/// `salted_item_fields_are_private`: direct field reads are rejected (`E0616`).
/// ```compile_fail,E0616
/// use fmine_gen::review::SaltedItem;
///
/// fn read_body(item: &SaltedItem) -> &str {
///     &item.body
/// }
/// ```
#[derive(PartialEq, Eq)]
pub struct SaltedItem {
    item_id: String,
    body: String,
}

impl fmt::Debug for SaltedItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SaltedItem(<redacted>)")
    }
}

/// Copied real record inside a [`ReviewPacket`] (`BD-357.S2`).
#[derive(PartialEq, Eq)]
pub struct RealRecordCopy {
    item_id: String,
    body: String,
}

impl fmt::Debug for RealRecordCopy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RealRecordCopy(<redacted>)")
    }
}

/// One reviewable item in a [`ReviewPacket`], hiding real-vs-salted provenance from reviewer reads.
#[derive(Debug, PartialEq, Eq)]
pub enum PacketItem {
    Real(RealRecordCopy),
    Salted(SaltedItem),
}

impl PacketItem {
    #[must_use]
    pub fn item_id(&self) -> &str {
        match self {
            Self::Real(r) => &r.item_id,
            Self::Salted(s) => &s.item_id,
        }
    }

    #[must_use]
    pub fn render_for_reviewer(&self) -> &str {
        match self {
            Self::Real(r) => &r.body,
            Self::Salted(s) => &s.body,
        }
    }
}

/// Input real record passed to [`build_packet`] / [`build_packet_with_log`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealRecordInput {
    pub body: String,
}

/// Text-form seed request passed to [`build_packet`] / [`build_packet_with_log`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedRequest {
    pub class_token: String,
    pub body: String,
}

/// Assembled salted or unsalted internal-review packet (`BD-357.S2`).
#[derive(Debug, PartialEq, Eq)]
pub struct ReviewPacket {
    pub round_id: String,
    pub manifest: PacketManifest,
    pub items: Vec<PacketItem>,
}

/// Parse a seed-class request token (`BD-357.S2`).
///
/// Explicitly refuses `"basis-switch"` (in any casing or separator form) with `FMINE-REF-301`
/// citing the permanent `RISK-32` exclusion: an issuer-attributed false figure is never placed
/// before an issuer reader.
pub fn parse_seed_class(token: &str) -> Result<SeedClass, Refusal> {
    let normalized = token.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    if normalized.contains("basis-switch") || normalized.contains("basisswitch") {
        return Err(Refusal::new(
            RefusalCode::MnpiWall301,
            "seed class 'basis-switch' is permanently excluded (BD-357, RISK-32): an issuer-attributed false figure is never placed before an issuer reader",
        ));
    }
    match normalized.as_str() {
        "s1" | "s1-close-cites-non-exercising-test" | "close-cites-non-exercising-test" => {
            Ok(SeedClass::S1CloseCitesNonExercisingTest)
        }
        "s2" | "s2-close-cites-non-pass-record" | "close-cites-non-pass-record" => {
            Ok(SeedClass::S2CloseCitesNonPassRecord)
        }
        "s3" | "s3-mis-cited-cell" | "mis-cited-cell" => Ok(SeedClass::S3MisCitedCell),
        "s4" | "s4-refused-shown-reproduced" | "refused-shown-reproduced" => {
            Ok(SeedClass::S4RefusedShownReproduced)
        }
        "s5" | "s5-stale-shown-current" | "stale-shown-current" => {
            Ok(SeedClass::S5StaleShownCurrent)
        }
        "s6" | "s6-wrong-cause-class" | "wrong-cause-class" => Ok(SeedClass::S6WrongCauseClass),
        _ => Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "unknown review seed class '{token}'; allowed classes are S1..S6 (basis-switch is permanently excluded)"
            ),
        )),
    }
}

fn shuffle_rng_from_nonce(round_id: &str, nonce: &[u8; 32]) -> PhiloxStream {
    let mut seed_buf =
        Vec::with_capacity(REVIEW_SHUFFLE_DOMAIN_PREFIX.len() + 4 + round_id.len() + nonce.len());
    seed_buf.extend_from_slice(REVIEW_SHUFFLE_DOMAIN_PREFIX);
    seed_buf.extend_from_slice(&(round_id.len() as u32).to_le_bytes());
    seed_buf.extend_from_slice(round_id.as_bytes());
    seed_buf.extend_from_slice(nonce);
    let h = blake3_hash(&seed_buf);
    let b = h.as_bytes();
    let key = RngKey {
        seed_root: u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
        partition: u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
        domain: u32::from_le_bytes([b[12], b[13], b[14], b[15]]),
        var_group: u32::from_le_bytes([b[16], b[17], b[18], b[19]]),
        realization: u32::from_le_bytes([b[20], b[21], b[22], b[23]]),
        brick: u64::from_le_bytes([b[24], b[25], b[26], b[27], b[28], b[29], b[30], b[31]]),
    };
    PhiloxStream::new(key)
}

fn next_unbiased_usize(stream: &mut PhiloxStream, upper_inclusive: usize) -> usize {
    let bound = (upper_inclusive as u64) + 1;
    let limit = u64::MAX - (u64::MAX % bound);
    loop {
        let sample = stream.next_u64();
        if sample < limit {
            return (sample % bound) as usize;
        }
    }
}

fn compute_packet_sha256(round_id: &str, kind: PacketKind, items: &[PacketItem]) -> Digest32 {
    let mut buf = Vec::new();
    buf.extend_from_slice(REVIEW_PACKET_DOMAIN_PREFIX);
    buf.extend_from_slice(&(round_id.len() as u32).to_le_bytes());
    buf.extend_from_slice(round_id.as_bytes());
    buf.extend_from_slice(&(kind.as_str().len() as u32).to_le_bytes());
    buf.extend_from_slice(kind.as_str().as_bytes());
    buf.extend_from_slice(&(items.len() as u32).to_le_bytes());
    for item in items {
        let id = item.item_id();
        let body = item.render_for_reviewer();
        buf.extend_from_slice(&(id.len() as u32).to_le_bytes());
        buf.extend_from_slice(id.as_bytes());
        buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
        buf.extend_from_slice(body.as_bytes());
    }
    sha256(&buf)
}

enum PendingEntry {
    Real(String),
    Salted(SeedClass, String),
}

/// Build a shuffled internal-review packet, its sealed [`ReviewKey`], and an indistinguishable
/// public NDJSON build log (`BD-357.S2`).
///
/// The permutation is keyed strictly by `(round_id, nonce)` so seed positions never leak; opaque
/// sequential IDs (`item-0001`, …) are assigned after shuffling. Each NDJSON item line carries
/// `{"round_id","item_id","position","packet_sha256"}` with no class or salted field, followed by
/// one summary line `{"round_id","items":N,"commitment":"<blake3 hex>"}`.
pub fn build_packet_with_log(
    round_id: &str,
    records: &[RealRecordInput],
    seeds: &[SeedRequest],
    nonce: [u8; 32],
) -> Result<(ReviewPacket, ReviewKey, String), Refusal> {
    if round_id.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "build_packet: round_id must be non-empty",
        ));
    }
    if nonce == [0u8; 32] {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "build_packet: all-zero nonce is refused; draw 32 bytes from OS entropy",
        ));
    }
    if records.is_empty() && seeds.is_empty() {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "build_packet: packet must contain at least one item",
        ));
    }

    let mut pending = Vec::with_capacity(records.len() + seeds.len());
    for (idx, rec) in records.iter().enumerate() {
        if rec.body.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("build_packet: real record #{idx} has empty body"),
            ));
        }
        pending.push(PendingEntry::Real(rec.body.clone()));
    }
    for (idx, seed) in seeds.iter().enumerate() {
        let cls = parse_seed_class(&seed.class_token)?;
        if seed.body.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("build_packet: seed request #{idx} has empty body"),
            ));
        }
        pending.push(PendingEntry::Salted(cls, seed.body.clone()));
    }

    // Fisher–Yates permutation keyed strictly by (round_id, nonce).
    let mut rng = shuffle_rng_from_nonce(round_id, &nonce);
    for i in (1..pending.len()).rev() {
        let j = next_unbiased_usize(&mut rng, i);
        pending.swap(i, j);
    }

    let mut items = Vec::with_capacity(pending.len());
    let mut item_ids = Vec::with_capacity(pending.len());
    let mut key_items = Vec::with_capacity(seeds.len());
    for (idx, entry) in pending.into_iter().enumerate() {
        let item_id = format!("item-{:04}", idx + 1);
        item_ids.push(item_id.clone());
        match entry {
            PendingEntry::Real(body) => {
                items.push(PacketItem::Real(RealRecordCopy { item_id, body }));
            }
            PendingEntry::Salted(cls, body) => {
                key_items.push((item_id.clone(), cls));
                items.push(PacketItem::Salted(SaltedItem { item_id, body }));
            }
        }
    }

    let kind = if seeds.is_empty() {
        PacketKind::Unsalted
    } else {
        PacketKind::Salted
    };
    let packet_sha = compute_packet_sha256(round_id, kind, &items);
    let key = ReviewKey {
        round_id: round_id.to_string(),
        nonce,
        items: key_items,
    };
    let commitment = commit(&key)?;

    let round_q = quote_string(round_id);
    let sha_q = quote_string(&packet_sha.to_hex());
    let mut log = String::new();
    for (position, id) in item_ids.iter().enumerate() {
        log.push_str(&format!(
            "{{\"round_id\":{round_q},\"item_id\":{},\"position\":{position},\"packet_sha256\":{sha_q}}}\n",
            quote_string(id)
        ));
    }
    log.push_str(&format!(
        "{{\"round_id\":{round_q},\"items\":{},\"commitment\":{}}}\n",
        item_ids.len(),
        quote_string(&commitment.to_hex())
    ));

    let packet = ReviewPacket {
        round_id: round_id.to_string(),
        manifest: PacketManifest {
            packet: kind,
            round_id: round_id.to_string(),
            item_ids,
            sha256: packet_sha,
        },
        items,
    };
    Ok((packet, key, log))
}

/// Convenience wrapper over [`build_packet_with_log`] returning `(ReviewPacket, ReviewKey)`.
pub fn build_packet(
    round_id: &str,
    records: &[RealRecordInput],
    seeds: &[SeedRequest],
    nonce: [u8; 32],
) -> Result<(ReviewPacket, ReviewKey), Refusal> {
    let (packet, key, _log) = build_packet_with_log(round_id, records, seeds, nonce)?;
    Ok((packet, key))
}

fn path_touches_forbidden_dir(path: &Path) -> Option<&'static str> {
    for comp in path.components() {
        if let Component::Normal(name) = comp {
            let s = name.to_string_lossy();
            if s == ".beads" {
                return Some(".beads");
            }
            if s.eq_ignore_ascii_case("corpus_of_input_docs") {
                return Some("corpus_of_input_docs");
            }
        }
    }
    None
}

/// Materialize a [`ReviewPacket`] to `dir` (`review-packet.toml` + `items/<item_id>.txt`).
///
/// Enforces:
/// - manifest integrity: a packet containing any [`SaltedItem`] can never be written with an
///   unsalted manifest (`FMINE-REF-301`), and `manifest.sha256` / `round_id` / `item_ids` must
///   match the packet contents (`FMINE-REF-505`);
/// - forbidden-path isolation: refuses (`FMINE-REF-301`) any path inside `.beads/` or
///   `corpus_of_input_docs/` (including relative paths rooted in those dirs or symlinked ancestors).
pub fn write_packet_dir(dir: &Path, packet: &ReviewPacket) -> Result<PathBuf, Refusal> {
    let has_salted = packet
        .items
        .iter()
        .any(|it| matches!(it, PacketItem::Salted(_)));
    if has_salted && packet.manifest.packet != PacketKind::Salted {
        return Err(Refusal::new(
            RefusalCode::MnpiWall301,
            "salted review packet cannot be written with an unsalted manifest (BD-357.S2, RISK-32)",
        ));
    }
    let expected_ids: Vec<&str> = packet.items.iter().map(PacketItem::item_id).collect();
    let manifest_ids: Vec<&str> = packet
        .manifest
        .item_ids
        .iter()
        .map(String::as_str)
        .collect();
    let expected_sha =
        compute_packet_sha256(&packet.round_id, packet.manifest.packet, &packet.items);
    if packet.manifest.round_id != packet.round_id
        || manifest_ids != expected_ids
        || packet.manifest.sha256 != expected_sha
    {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "review packet manifest does not match packet contents; refusing to materialize",
        ));
    }

    if let Some(forbidden) = path_touches_forbidden_dir(dir) {
        return Err(Refusal::new(
            RefusalCode::MnpiWall301,
            format!(
                "refusing to write review packet into forbidden directory '{forbidden}': {}",
                dir.display()
            ),
        ));
    }
    let abs_dir = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| {
                Refusal::new(
                    RefusalCode::InputMissingOrUnreadable506,
                    format!("unable to resolve current working directory for packet output: {e}"),
                )
            })?
            .join(dir)
    };
    if let Some(forbidden) = path_touches_forbidden_dir(&abs_dir) {
        return Err(Refusal::new(
            RefusalCode::MnpiWall301,
            format!(
                "refusing to write review packet into forbidden directory '{forbidden}': {}",
                abs_dir.display()
            ),
        ));
    }
    for ancestor in abs_dir.ancestors() {
        if !ancestor.as_os_str().is_empty()
            && ancestor.exists()
            && let Ok(canon) = ancestor.canonicalize()
            && let Some(forbidden) = path_touches_forbidden_dir(&canon)
        {
            return Err(Refusal::new(
                RefusalCode::MnpiWall301,
                format!(
                    "refusing to write review packet under symlinked forbidden directory '{forbidden}': {}",
                    canon.display()
                ),
            ));
        }
    }

    let items_dir = dir.join("items");
    std::fs::create_dir_all(&items_dir).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!(
                "unable to create review packet items dir {}: {e}",
                items_dir.display()
            ),
        )
    })?;
    let manifest_path = dir.join(REVIEW_PACKET_MANIFEST);
    std::fs::write(&manifest_path, packet.manifest.to_toml()).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!(
                "unable to write review packet manifest {}: {e}",
                manifest_path.display()
            ),
        )
    })?;
    for item in &packet.items {
        let item_path = items_dir.join(format!("{}.txt", item.item_id()));
        std::fs::write(&item_path, item.render_for_reviewer()).map_err(|e| {
            Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!(
                    "unable to write review packet item {}: {e}",
                    item_path.display()
                ),
            )
        })?;
    }
    Ok(manifest_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng_key_hex(k: RngKey) -> String {
        let mut buf = [0u8; 32];
        buf[0..8].copy_from_slice(&k.seed_root.to_le_bytes());
        buf[8..12].copy_from_slice(&k.partition.to_le_bytes());
        buf[12..16].copy_from_slice(&k.domain.to_le_bytes());
        buf[16..20].copy_from_slice(&k.var_group.to_le_bytes());
        buf[20..24].copy_from_slice(&k.realization.to_le_bytes());
        buf[24..32].copy_from_slice(&k.brick.to_le_bytes());
        let mut s = String::with_capacity(64);
        for b in buf {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    #[test]
    fn test_clopper_pearson_known_answers() {
        // Known answers from BD-357.S1 at alpha = 0.05 (exact to 1e-4).
        let cases: &[(u32, u32, f64, f64)] = &[
            (0, 5, 0.0, 0.5218),
            (2, 5, 0.0527, 0.8534),
            (3, 5, 0.1466, 0.9473),
            (5, 5, 0.4782, 1.0),
            (0, 10, 0.0, 0.3085),
            (8, 10, 0.4439, 0.9748),
            (10, 10, 0.6915, 1.0),
        ];
        for &(x, n, exp_lo, exp_hi) in cases {
            let (lo, hi) = clopper_pearson(x, n, 0.05).unwrap();
            assert!(
                (lo - exp_lo).abs() <= 1e-4,
                "x={x}, n={n}: lower {lo} != expected {exp_lo}"
            );
            assert!(
                (hi - exp_hi).abs() <= 1e-4,
                "x={x}, n={n}: upper {hi} != expected {exp_hi}"
            );
            assert!(lo <= hi, "interval inverted for {x}/{n}");

            // Exact symmetry: lo(x, n) == 1 - hi(n - x, n).
            let (sym_lo, sym_hi) = clopper_pearson(n - x, n, 0.05).unwrap();
            assert!((lo - (1.0 - sym_hi)).abs() < 1e-12);
            assert!((hi - (1.0 - sym_lo)).abs() < 1e-12);
        }
    }

    #[test]
    fn test_clopper_pearson_edges_exact() {
        for n in [1_u32, 5, 10, 40, MAX_EXACT_CLOPPER_PEARSON_N] {
            let (lo0, hi0) = clopper_pearson(0, n, 0.05).unwrap();
            assert_eq!(lo0.to_bits(), 0.0_f64.to_bits(), "x=0 must yield exact 0.0");
            assert!(hi0 > 0.0 && hi0 < 1.0);

            let (lon, hin) = clopper_pearson(n, n, 0.05).unwrap();
            assert_eq!(hin.to_bits(), 1.0_f64.to_bits(), "x=n must yield exact 1.0");
            assert!(lon > 0.0 && lon < 1.0);
        }
    }

    #[test]
    fn test_clopper_pearson_refuses_bad_inputs() {
        let bad_cases: &[(u32, u32, f64)] = &[
            (0, 0, 0.05),
            (6, 5, 0.05),
            (0, MAX_EXACT_CLOPPER_PEARSON_N + 1, 0.05),
            (2, 5, 0.0),
            (2, 5, 1.0),
            (2, 5, -0.05),
            (2, 5, 1.05),
            (2, 5, f64::NAN),
            (2, 5, f64::INFINITY),
            (2, 5, f64::NEG_INFINITY),
        ];
        for &(x, n, alpha) in bad_cases {
            let err = clopper_pearson(x, n, alpha).unwrap_err();
            assert_eq!(
                err.code,
                RefusalCode::OutsidePhysicalDomain109.spec().code,
                "bad input ({x}, {n}, {alpha}) must refuse with FMINE-REF-109"
            );
        }
    }

    #[test]
    fn test_clopper_pearson_exact_coverage_ge_95() {
        let mut min_cov = 1.0_f64;
        let mut cov_10_05 = 0.0_f64;
        for n in [5_u32, 10] {
            let intervals: Vec<(f64, f64)> = (0..=n)
                .map(|x| clopper_pearson(x, n, 0.05).unwrap())
                .collect();
            for p in [0.2_f64, 0.5, 0.9] {
                let mut coverage = 0.0_f64;
                for (x, &(lo, hi)) in intervals.iter().enumerate() {
                    if lo <= p && p <= hi {
                        coverage += binom_pmf(x as u32, n, p);
                    }
                }
                assert!(
                    coverage >= 0.95,
                    "exact Clopper-Pearson coverage {coverage} < 0.95 for n={n}, p={p}"
                );
                if coverage < min_cov {
                    min_cov = coverage;
                }
                if n == 10 && (p - 0.5).abs() < 1e-12 {
                    cov_10_05 = coverage;
                }
            }
        }
        // BD-357.S1 acceptance criterion: computed minimum is 0.9785 at n=10, p=0.5.
        assert!(
            (cov_10_05 - 0.9785).abs() <= 1e-4,
            "expected exact coverage ~0.9785 at n=10, p=0.5, got {cov_10_05}"
        );
        assert!((min_cov - cov_10_05).abs() < 1e-12);
    }

    #[test]
    fn test_clopper_pearson_simulated_coverage_10k() {
        const ROUNDS: u32 = 10_000;
        let mut min_coverage = 1.0_f64;
        let mut logs = Vec::new();

        for (n_idx, &n) in [5_u32, 10].iter().enumerate() {
            let intervals: Vec<(f64, f64)> = (0..=n)
                .map(|x| clopper_pearson(x, n, 0.05).unwrap())
                .collect();
            for (p_idx, &p) in [0.2_f64, 0.5, 0.9].iter().enumerate() {
                let key = RngKey {
                    seed_root: 0xBD35_7510_0000_0001,
                    partition: 357,
                    domain: n,
                    var_group: (p_idx as u32) + 1,
                    realization: n_idx as u32,
                    brick: 0,
                };
                let mut stream = PhiloxStream::new(key);
                let mut covered = 0_u32;
                for _ in 0..ROUNDS {
                    let mut x = 0_u32;
                    for _ in 0..n {
                        if stream.next_f64_open01() < p {
                            x += 1;
                        }
                    }
                    let (lo, hi) = intervals[x as usize];
                    if lo <= p && p <= hi {
                        covered += 1;
                    }
                }
                let coverage = f64::from(covered) / f64::from(ROUNDS);
                let line = format!(
                    "{{\"p\":{p},\"n\":{n},\"rounds\":{ROUNDS},\"covered\":{covered},\
                     \"coverage\":{coverage:.6},\"key\":\"{}\"}}",
                    rng_key_hex(key)
                );
                logs.push(line);
                assert!(
                    coverage >= 0.95,
                    "simulated coverage {coverage} < 0.95 for n={n}, p={p}"
                );
                if coverage < min_coverage {
                    min_coverage = coverage;
                }
            }
        }
        let summary = format!("{{\"min_coverage\":{min_coverage:.6},\"threshold\":0.95}}");
        assert_eq!(logs.len(), 6);
        assert!(summary.contains("\"threshold\":0.95"));
        assert!(min_coverage >= 0.95);
    }

    fn sample_key() -> ReviewKey {
        ReviewKey {
            round_id: "round-1-bead-closes".to_string(),
            nonce: [
                0x19, 0xa2, 0x3b, 0x4c, 0x5d, 0x6e, 0x7f, 0x80, 0x91, 0xa2, 0xb3, 0xc4, 0xd5, 0xe6,
                0xf7, 0x08, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff, 0x42,
            ],
            items: vec![
                (
                    "item-007".to_string(),
                    SeedClass::S1CloseCitesNonExercisingTest,
                ),
                ("item-019".to_string(), SeedClass::S3MisCitedCell),
                ("item-033".to_string(), SeedClass::S5StaleShownCurrent),
            ],
        }
    }

    #[test]
    fn test_review_key_commit_verify_roundtrip() {
        let key = sample_key();
        let digest = commit(&key).unwrap();
        verify(&key, &digest).unwrap();

        // Also exercise real OS entropy nonce generation.
        let os_nonce = generate_os_nonce().unwrap();
        assert_ne!(os_nonce, [0u8; 32]);
        let mut os_key = key;
        os_key.nonce = os_nonce;
        let os_digest = commit(&os_key).unwrap();
        verify(&os_key, &os_digest).unwrap();
    }

    #[test]
    fn test_review_key_tamper_fails() {
        let key = sample_key();
        let digest = commit(&key).unwrap();

        // 1. Tamper seed class.
        let mut bad_class = key.clone();
        bad_class.items[0].1 = SeedClass::S2CloseCitesNonPassRecord;
        assert_eq!(
            verify(&bad_class, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // 2. Tamper item id.
        let mut bad_id = key.clone();
        bad_id.items[1].0 = "item-020".to_string();
        assert_eq!(
            verify(&bad_id, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // 3. Flip a single bit in nonce.
        let mut bad_nonce = key.clone();
        bad_nonce.nonce[31] ^= 0x01;
        assert_eq!(
            verify(&bad_nonce, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // 4. Tamper round_id.
        let mut bad_round = key;
        bad_round.round_id = "round-2-m0-packet".to_string();
        assert_eq!(
            verify(&bad_round, &digest).unwrap_err().code,
            RefusalCode::DigestMismatch505.spec().code
        );
    }

    #[test]
    fn test_review_key_zero_nonce_refused() {
        let mut key = sample_key();
        key.nonce = [0u8; 32];
        assert_eq!(
            commit(&key).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // Empty round_id and duplicate item_id also refused.
        let mut empty_round = sample_key();
        empty_round.round_id = " ".to_string();
        assert_eq!(
            commit(&empty_round).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        let mut dup_item = sample_key();
        dup_item
            .items
            .push(("item-007".to_string(), SeedClass::S6WrongCauseClass));
        assert_eq!(
            commit(&dup_item).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }

    #[test]
    fn test_canonical_bytes_independent_of_item_order() {
        let k1 = sample_key();
        let mut k2 = sample_key();
        k2.items.reverse();
        let mut k3 = sample_key();
        k3.items.rotate_left(1);

        let b1 = k1.canonical_bytes().unwrap();
        let b2 = k2.canonical_bytes().unwrap();
        let b3 = k3.canonical_bytes().unwrap();
        assert_eq!(b1, b2);
        assert_eq!(b2, b3);
        assert_eq!(commit(&k1).unwrap(), commit(&k2).unwrap());
        assert_eq!(commit(&k2).unwrap(), commit(&k3).unwrap());
    }

    fn sample_real_records() -> Vec<RealRecordInput> {
        (1..=6)
            .map(|i| RealRecordInput {
                body: format!("Real bead close record #{i}: cited test exercises invariant."),
            })
            .collect()
    }

    fn sample_seed_requests() -> Vec<SeedRequest> {
        vec![
            SeedRequest {
                class_token: "S1".to_string(),
                body: "Candidate close record A: cites test that skips target branch.".to_string(),
            },
            SeedRequest {
                class_token: "s3_mis_cited_cell".to_string(),
                body: "Candidate close record B: cites wrong sheet coordinate.".to_string(),
            },
        ]
    }

    #[test]
    fn test_builder_rejects_basis_switch_request() {
        for forbidden in [
            "basis-switch",
            "BASIS_SWITCH",
            "Basis Switch",
            "basisswitch",
            "issuer-basis-switch",
        ] {
            let err = parse_seed_class(forbidden).unwrap_err();
            assert_eq!(err.code, RefusalCode::MnpiWall301.spec().code);
            assert!(
                err.what.contains("excluded") && err.what.contains("issuer"),
                "unexpected message: {}",
                err.what
            );
            let build_err = build_packet(
                "round-1",
                &sample_real_records(),
                &[SeedRequest {
                    class_token: forbidden.to_string(),
                    body: "Attempted basis-switch seed.".to_string(),
                }],
                [0x42; 32],
            )
            .unwrap_err();
            assert_eq!(build_err.code, RefusalCode::MnpiWall301.spec().code);
        }
    }

    #[test]
    fn test_shuffle_is_nonce_keyed_and_complete() {
        let records = sample_real_records();
        let seeds = sample_seed_requests();
        let nonce_a = [0x11; 32];
        let nonce_b = [0x22; 32];

        let (pkt_a1, key_a1) = build_packet("round-1", &records, &seeds, nonce_a).unwrap();
        let (pkt_a2, key_a2) = build_packet("round-1", &records, &seeds, nonce_a).unwrap();
        let (pkt_b, key_b) = build_packet("round-1", &records, &seeds, nonce_b).unwrap();

        // Same nonce -> identical packet, order, digest, and key.
        assert_eq!(pkt_a1, pkt_a2);
        assert_eq!(key_a1, key_a2);
        assert_eq!(pkt_a1.manifest.packet, PacketKind::Salted);

        // Different nonce -> different shuffled order and different digest.
        let bodies_a: Vec<&str> = pkt_a1
            .items
            .iter()
            .map(PacketItem::render_for_reviewer)
            .collect();
        let bodies_b: Vec<&str> = pkt_b
            .items
            .iter()
            .map(PacketItem::render_for_reviewer)
            .collect();
        assert_ne!(bodies_a, bodies_b);
        assert_ne!(pkt_a1.manifest.sha256, pkt_b.manifest.sha256);

        // Completeness: every real and salted body appears exactly once in both packets.
        let mut expected: Vec<&str> = records
            .iter()
            .map(|r| r.body.as_str())
            .chain(seeds.iter().map(|s| s.body.as_str()))
            .collect();
        expected.sort_unstable();

        for (pkt, key) in [(&pkt_a1, &key_a1), (&pkt_b, &key_b)] {
            assert_eq!(pkt.items.len(), expected.len());
            let mut got: Vec<&str> = pkt
                .items
                .iter()
                .map(PacketItem::render_for_reviewer)
                .collect();
            got.sort_unstable();
            assert_eq!(got, expected);

            // Key names exactly the SaltedItem ids and verifies against its commitment.
            let salted_ids: BTreeSet<&str> = pkt
                .items
                .iter()
                .filter_map(|it| match it {
                    PacketItem::Salted(_) => Some(it.item_id()),
                    PacketItem::Real(_) => None,
                })
                .collect();
            let key_ids: BTreeSet<&str> = key.items.iter().map(|(id, _)| id.as_str()).collect();
            assert_eq!(salted_ids, key_ids);
            assert_eq!(key_ids.len(), seeds.len());
            let commitment = commit(key).unwrap();
            verify(key, &commitment).unwrap();
        }
    }

    #[test]
    fn test_build_log_is_indistinguishable() {
        let records = sample_real_records();
        let seeds = sample_seed_requests();
        let (pkt, key, log) =
            build_packet_with_log("round-1-bead-closes", &records, &seeds, [0x77; 32]).unwrap();

        // No mention of salted/class anywhere in the public build log.
        let lower = log.to_ascii_lowercase();
        assert!(!lower.contains("salted"), "log leaked 'salted': {log}");
        assert!(!lower.contains("class"), "log leaked 'class': {log}");
        assert!(!lower.contains("s1"), "log leaked seed token: {log}");
        assert!(!lower.contains("s3"), "log leaked seed token: {log}");

        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(lines.len(), pkt.items.len() + 1);
        let item_lines = &lines[..pkt.items.len()];
        let summary_line = lines[pkt.items.len()];

        // Every per-item NDJSON line has the exact same key set and identical length (8 items => 1-digit positions).
        let first_len = item_lines[0].len();
        for (idx, line) in item_lines.iter().enumerate() {
            assert_eq!(
                line.len(),
                first_len,
                "item line {idx} length {} differs from first line length {first_len}: {line}",
                line.len()
            );
            for required_key in [
                "\"round_id\":",
                "\"item_id\":",
                "\"position\":",
                "\"packet_sha256\":",
            ] {
                assert!(
                    line.contains(required_key),
                    "missing {required_key}: {line}"
                );
            }
        }
        let expected_commitment = commit(&key).unwrap().to_hex();
        assert!(summary_line.contains(&format!("\"items\":{}", pkt.items.len())));
        assert!(summary_line.contains(&format!("\"commitment\":\"{expected_commitment}\"")));

        // Debug representation of SaltedItem is redacted and never reveals body or class.
        for item in &pkt.items {
            if let PacketItem::Salted(s) = item {
                assert_eq!(format!("{s:?}"), "SaltedItem(<redacted>)");
            }
        }
    }

    #[test]
    fn test_write_packet_refuses_beads_and_corpus_paths() {
        let (salted_pkt, _) = build_packet(
            "round-1",
            &sample_real_records(),
            &sample_seed_requests(),
            [0x55; 32],
        )
        .unwrap();
        let (unsalted_pkt, _) =
            build_packet("round-1", &sample_real_records(), &[], [0x55; 32]).unwrap();
        assert_eq!(unsalted_pkt.manifest.packet, PacketKind::Unsalted);

        for forbidden in [
            Path::new("ws/.beads/packet"),
            Path::new("ws/corpus_of_input_docs/packet"),
            Path::new("ws/CORPUS_OF_INPUT_DOCS/sub/packet"),
        ] {
            let err = write_packet_dir(forbidden, &salted_pkt).unwrap_err();
            assert_eq!(err.code, RefusalCode::MnpiWall301.spec().code);
        }

        // Round-trip materialized salted & unsalted packets through fmine_public::refuse_if_salted.
        let pid = std::process::id();
        let dir_salted = std::env::temp_dir().join(format!("fmine-review-salted-{pid}"));
        let dir_unsalted = std::env::temp_dir().join(format!("fmine-review-unsalted-{pid}"));
        let _ = std::fs::remove_dir_all(&dir_salted);
        let _ = std::fs::remove_dir_all(&dir_unsalted);

        let salted_manifest = write_packet_dir(&dir_salted, &salted_pkt).unwrap();
        assert!(salted_manifest.is_file());
        let refusal = fmine_public::refuse_if_salted(&dir_salted).unwrap_err();
        assert_eq!(refusal.code, RefusalCode::MnpiWall301.spec().code);

        let unsalted_manifest = write_packet_dir(&dir_unsalted, &unsalted_pkt).unwrap();
        assert!(unsalted_manifest.is_file());
        fmine_public::refuse_if_salted(&dir_unsalted).unwrap();

        // Tampering a salted packet's manifest to Unsalted refuses with FMINE-REF-301.
        let (mut tampered_kind, _) = build_packet(
            "round-1",
            &sample_real_records(),
            &sample_seed_requests(),
            [0x55; 32],
        )
        .unwrap();
        tampered_kind.manifest.packet = PacketKind::Unsalted;
        assert_eq!(
            write_packet_dir(&dir_salted, &tampered_kind)
                .unwrap_err()
                .code,
            RefusalCode::MnpiWall301.spec().code
        );

        // Tampering a manifest digest refuses with FMINE-REF-505.
        let (mut tampered_sha, _) = build_packet(
            "round-1",
            &sample_real_records(),
            &sample_seed_requests(),
            [0x55; 32],
        )
        .unwrap();
        tampered_sha.manifest.sha256 = Digest32([0u8; 32]);
        assert_eq!(
            write_packet_dir(&dir_salted, &tampered_sha)
                .unwrap_err()
                .code,
            RefusalCode::DigestMismatch505.spec().code
        );

        // Section-smuggled `packet = "unsalted"` inside a TOML table fails closed with FMINE-REF-301,
        // and BOM-prefixed manifests are parsed cleanly.
        let dir_edge = std::env::temp_dir().join(format!("fmine-review-edge-{pid}"));
        let _ = std::fs::remove_dir_all(&dir_edge);
        std::fs::create_dir_all(&dir_edge).unwrap();
        std::fs::write(
            dir_edge.join(REVIEW_PACKET_MANIFEST),
            "[metadata]\npacket = \"unsalted\"\n",
        )
        .unwrap();
        assert_eq!(
            fmine_public::refuse_if_salted(&dir_edge).unwrap_err().code,
            RefusalCode::MnpiWall301.spec().code
        );
        std::fs::write(
            dir_edge.join(REVIEW_PACKET_MANIFEST),
            "\u{FEFF}packet = \"salted\"\n",
        )
        .unwrap();
        assert_eq!(
            fmine_public::refuse_if_salted(&dir_edge).unwrap_err().code,
            RefusalCode::MnpiWall301.spec().code
        );
        std::fs::write(
            dir_edge.join(REVIEW_PACKET_MANIFEST),
            "\u{FEFF}packet = \"unsalted\"\n",
        )
        .unwrap();
        fmine_public::refuse_if_salted(&dir_edge).unwrap();

        let _ = std::fs::remove_dir_all(&dir_salted);
        let _ = std::fs::remove_dir_all(&dir_unsalted);
        let _ = std::fs::remove_dir_all(&dir_edge);
    }
}
