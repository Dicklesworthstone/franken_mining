//! Determinism: BitwisePortable
//! Concentrate and doré product custody, IMSBC liquefaction safety refusal (`FMINE-REF-403`),
//! append-only doré seal/vault log, refinery outturn audit, and dual-human-attested shipment
//! release (`BD-179`, `BD-180`, plan §17.6, `G-CRUC-4`, `INV-002`, `PO-SEC-3`).
//!
//! All physical quantities use exact `fmine_units::ExactQty` (zero bare `f64` parameters; L-6).
//!
//! Hard boundaries:
//! - Missing, stale, or above-TML concentrate moisture ALWAYS refuses with `FMINE-REF-403` and
//!   never emits a "ready to ship" clearance (`G-CRUC-4`).
//! - Shipment release requires dual human attestation from two distinct named human principals via
//!   `fmine_authz::human_token::HumanTokenVerifier::verify_dual_attestation` (`<= 15 min` TTL,
//!   single-use nonces, site & `effect_digest` binding), and re-verifies moisture freshness at the
//!   instant of release BEFORE consuming any human authority token.
//! - `execution_authorized` is permanently `false` (`INV-002`): FrankenMining prepares and records
//!   the attested certificate; humans execute physical shipment release.

use std::collections::BTreeSet;

use fmine_authz::human_token::{DualAttestationGrant, HumanTokenVerifier};
use fmine_authz::macaroon::{FmKeyRing, Macaroon};
use fmine_core::Digest32;
use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{ExactQty, UnitsError};

/// Canonical domain prefix for prepared shipment-release effect digests (`BD-180`).
const SHIPMENT_EFFECT_DOMAIN: &[u8] = b"fmine:crucible-shipment-release-effect:v1\0";

/// Canonical domain prefix for attested shipment-release receipt digests (`BD-180`).
const SHIPMENT_RECEIPT_DOMAIN: &[u8] = b"fmine:crucible-shipment-release-receipt:v1\0";

/// Canonical domain prefix for append-only doré vault log entries (`BD-179`).
const DORE_VAULT_DOMAIN: &[u8] = b"fmine:crucible-dore-vault-entry:v1\0";

/// Physical product custody lane (plan §17.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum CustodyProductKind {
    CopperConcentrate = 1,
    TinConcentrate = 2,
    GoldSilverDore = 3,
    CopperCathode = 4,
}

impl CustodyProductKind {
    #[must_use]
    pub const fn tag_byte(self) -> u8 {
        self as u8
    }

    /// Returns `true` for bulk marine concentrates subject to IMSBC TML liquefaction checks.
    #[must_use]
    pub const fn requires_tml_check(self) -> bool {
        matches!(self, Self::CopperConcentrate | Self::TinConcentrate)
    }
}

/// Measured moisture determination for one concentrate lot (wet-mass fraction `H2O / wet_mass`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoistureDetermination {
    /// Point moisture fraction in `[0, 1)`.
    pub moisture_fraction: ExactQty,
    /// Conservative upper confidence bound in `[moisture_fraction, 1)` compared against TML.
    pub upper_confidence_fraction: ExactQty,
    /// Timestamp when the moisture sample was certified (Unix nanoseconds).
    pub measured_at_ns: i64,
    /// Maximum permitted age before the moisture determination becomes stale (nanoseconds > 0).
    pub max_age_ns: i64,
    /// Assay certificate / laboratory lot reference.
    pub certificate_id: String,
}

/// Input specification for evaluating concentrate shipment clearance against IMSBC TML (`BD-179`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrateLotSpec {
    pub site_id: String,
    pub lot_id: String,
    pub product_kind: CustodyProductKind,
    /// Total wet mass of the lot in kilograms (strictly positive).
    pub wet_mass_kg: ExactQty,
    /// Moisture determination for this specific lot (`None` → `FMINE-REF-403`).
    pub moisture: Option<MoistureDetermination>,
    /// Transportable Moisture Limit as wet-mass fraction in `(0, 1)` (`None` → `FMINE-REF-403`).
    pub tml_fraction: Option<ExactQty>,
    /// Evaluation time in Unix nanoseconds.
    pub evaluated_at_ns: i64,
}

/// Verified concentrate shipment clearance carrying exact dry/water mass split and prepared effect digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedShipmentRelease {
    pub site_id: String,
    pub lot_id: String,
    pub product_kind: CustodyProductKind,
    pub wet_mass_kg: ExactQty,
    pub dry_mass_kg: ExactQty,
    pub water_mass_kg: ExactQty,
    pub moisture_fraction: ExactQty,
    pub moisture_upper_bound: ExactQty,
    pub tml_fraction: ExactQty,
    pub safety_margin_below_tml: ExactQty,
    pub moisture_measured_at_ns: i64,
    pub moisture_max_age_ns: i64,
    pub moisture_certificate_id: String,
    /// Canonical SHA-256 digest bound into both `HumanAuthorityToken`s (`BD-180`).
    pub effect_digest: [u8; 32],
}

/// Final attested shipment-release record produced ONLY after passing `FMINE-REF-403` AND dual
/// human attestation (`BD-180`, `INV-002`, `PO-SEC-3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipmentReleaseReceipt {
    pub prepared: PreparedShipmentRelease,
    pub dual_attestation: DualAttestationGrant,
    pub released_at_ns: i64,
    /// Permanently `false` (`INV-002`): FrankenMining records human attestation, never executes control.
    pub execution_authorized: bool,
    pub receipt_digest: Digest32,
}

fn map_units_err(e: UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("exact custody arithmetic error: {e}"),
    )
}

fn cmp_qty(a: ExactQty, b: ExactQty) -> Result<core::cmp::Ordering, Refusal> {
    a.checked_cmp(b).map_err(map_units_err)
}

fn push_len_str(buf: &mut Vec<u8>, s: &str) -> Result<(), Refusal> {
    let len = u32::try_from(s.len()).map_err(|_| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "custody string length exceeds u32::MAX",
        )
    })?;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    Ok(())
}

fn push_exact_qty(buf: &mut Vec<u8>, q: ExactQty) {
    buf.extend_from_slice(&q.num().to_le_bytes());
    buf.extend_from_slice(&q.den().to_le_bytes());
}

fn check_moisture_fresh(
    lot_id: &str,
    measured_at_ns: i64,
    max_age_ns: i64,
    now_ns: i64,
) -> Result<(), Refusal> {
    if max_age_ns <= 0 {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!("lot '{lot_id}': moisture max_age_ns must be > 0, got {max_age_ns}"),
        ));
    }
    let Some(age_ns) = now_ns.checked_sub(measured_at_ns) else {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!("lot '{lot_id}': timestamp overflow evaluating moisture age"),
        ));
    };
    if age_ns < 0 {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{lot_id}': moisture measurement timestamp {measured_at_ns} lies in the \
                 future relative to evaluation time {now_ns}"
            ),
        ));
    }
    if age_ns > max_age_ns {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{lot_id}': moisture determination is stale (age {age_ns} ns > max {max_age_ns} ns)"
            ),
        ));
    }
    Ok(())
}

fn compute_prepared_effect_digest(p: &PreparedShipmentRelease) -> Result<[u8; 32], Refusal> {
    let mut buf = Vec::new();
    buf.extend_from_slice(SHIPMENT_EFFECT_DOMAIN);
    push_len_str(&mut buf, &p.site_id)?;
    push_len_str(&mut buf, &p.lot_id)?;
    buf.push(p.product_kind.tag_byte());
    push_exact_qty(&mut buf, p.wet_mass_kg);
    push_exact_qty(&mut buf, p.dry_mass_kg);
    push_exact_qty(&mut buf, p.water_mass_kg);
    push_exact_qty(&mut buf, p.moisture_fraction);
    push_exact_qty(&mut buf, p.moisture_upper_bound);
    push_exact_qty(&mut buf, p.tml_fraction);
    buf.extend_from_slice(&p.moisture_measured_at_ns.to_le_bytes());
    buf.extend_from_slice(&p.moisture_max_age_ns.to_le_bytes());
    push_len_str(&mut buf, &p.moisture_certificate_id)?;
    Ok(sha256(&buf).0)
}

/// Evaluate concentrate shipment eligibility against IMSBC Code TML safety rules (`BD-179`,
/// `G-CRUC-4`, `FMINE-REF-403`).
///
/// Refuses with `FMINE-REF-403` whenever:
/// - moisture determination is missing;
/// - TML is missing or outside `(0, 1)`;
/// - moisture determination is stale or future-dated;
/// - moisture point estimate or conservative upper bound is `>= tml_fraction`.
pub fn prepare_concentrate_shipment(
    lot: &ConcentrateLotSpec,
) -> Result<PreparedShipmentRelease, Refusal> {
    if lot.site_id.trim().is_empty() || lot.lot_id.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "concentrate lot requires non-empty site_id and lot_id",
        ));
    }
    if !lot.product_kind.requires_tml_check() {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            format!(
                "product {:?} is not a bulk concentrate lot requiring TML clearance",
                lot.product_kind
            ),
        ));
    }
    if lot.wet_mass_kg.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "concentrate wet_mass_kg must be strictly positive",
        ));
    }

    let moisture = lot.moisture.as_ref().ok_or_else(|| {
        Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{}': moisture determination is missing; shipment release refused",
                lot.lot_id
            ),
        )
    })?;
    if moisture.certificate_id.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!("lot '{}': moisture certificate_id is empty", lot.lot_id),
        ));
    }

    let tml = lot.tml_fraction.ok_or_else(|| {
        Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{}': transportable moisture limit (TML) is missing; shipment release refused",
                lot.lot_id
            ),
        )
    })?;

    let zero = ExactQty::from_integer(0);
    let one = ExactQty::from_integer(1);

    if cmp_qty(tml, zero)?.is_le() || cmp_qty(tml, one)?.is_ge() {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{}': TML fraction must lie strictly in (0, 1), got {}/{}",
                lot.lot_id,
                tml.num(),
                tml.den()
            ),
        ));
    }

    if cmp_qty(moisture.moisture_fraction, zero)?.is_lt()
        || cmp_qty(moisture.moisture_fraction, one)?.is_ge()
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("lot '{}': moisture fraction outside [0, 1)", lot.lot_id),
        ));
    }

    if cmp_qty(
        moisture.upper_confidence_fraction,
        moisture.moisture_fraction,
    )?
    .is_lt()
        || cmp_qty(moisture.upper_confidence_fraction, one)?.is_ge()
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "lot '{}': moisture upper confidence bound must lie in [moisture_fraction, 1)",
                lot.lot_id
            ),
        ));
    }

    check_moisture_fresh(
        &lot.lot_id,
        moisture.measured_at_ns,
        moisture.max_age_ns,
        lot.evaluated_at_ns,
    )?;

    if cmp_qty(moisture.upper_confidence_fraction, tml)?.is_ge() {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{}': moisture upper confidence bound {}/{} >= TML {}/{}; liquefaction hazard",
                lot.lot_id,
                moisture.upper_confidence_fraction.num(),
                moisture.upper_confidence_fraction.den(),
                tml.num(),
                tml.den()
            ),
        ));
    }

    let safety_margin_below_tml = tml
        .checked_sub(moisture.upper_confidence_fraction)
        .map_err(map_units_err)?;
    let water_mass_kg = lot
        .wet_mass_kg
        .checked_mul(moisture.moisture_fraction)
        .map_err(map_units_err)?;
    let dry_fraction = one
        .checked_sub(moisture.moisture_fraction)
        .map_err(map_units_err)?;
    let dry_mass_kg = lot
        .wet_mass_kg
        .checked_mul(dry_fraction)
        .map_err(map_units_err)?;

    let mut prepared = PreparedShipmentRelease {
        site_id: lot.site_id.clone(),
        lot_id: lot.lot_id.clone(),
        product_kind: lot.product_kind,
        wet_mass_kg: lot.wet_mass_kg,
        dry_mass_kg,
        water_mass_kg,
        moisture_fraction: moisture.moisture_fraction,
        moisture_upper_bound: moisture.upper_confidence_fraction,
        tml_fraction: tml,
        safety_margin_below_tml,
        moisture_measured_at_ns: moisture.measured_at_ns,
        moisture_max_age_ns: moisture.max_age_ns,
        moisture_certificate_id: moisture.certificate_id.clone(),
        effect_digest: [0u8; 32],
    };
    prepared.effect_digest = compute_prepared_effect_digest(&prepared)?;
    Ok(prepared)
}

fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0_u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn validate_prepared_shipment_invariants(
    prepared: &PreparedShipmentRelease,
) -> Result<(), Refusal> {
    if prepared.site_id.trim().is_empty()
        || prepared.lot_id.trim().is_empty()
        || prepared.moisture_certificate_id.trim().is_empty()
    {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "PreparedShipmentRelease requires non-empty site_id, lot_id, and moisture_certificate_id",
        ));
    }
    if !prepared.product_kind.requires_tml_check() {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "lot '{}': product kind {:?} does not belong on the IMSBC concentrate shipment gate",
                prepared.lot_id, prepared.product_kind
            ),
        ));
    }
    let zero = ExactQty::from_integer(0);
    let one = ExactQty::from_integer(1);
    if prepared.wet_mass_kg.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "lot '{}': wet_mass_kg must be strictly positive",
                prepared.lot_id
            ),
        ));
    }
    if cmp_qty(prepared.tml_fraction, zero)?.is_le() || cmp_qty(prepared.tml_fraction, one)?.is_ge()
    {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{}': TML fraction must lie strictly in (0, 1)",
                prepared.lot_id
            ),
        ));
    }
    if cmp_qty(prepared.moisture_fraction, zero)?.is_lt()
        || cmp_qty(prepared.moisture_fraction, one)?.is_ge()
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "lot '{}': moisture_fraction must lie in [0, 1)",
                prepared.lot_id
            ),
        ));
    }
    if cmp_qty(prepared.moisture_upper_bound, prepared.moisture_fraction)?.is_lt()
        || cmp_qty(prepared.moisture_upper_bound, one)?.is_ge()
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "lot '{}': moisture_upper_bound must lie in [moisture_fraction, 1)",
                prepared.lot_id
            ),
        ));
    }
    if cmp_qty(prepared.moisture_upper_bound, prepared.tml_fraction)?.is_ge() {
        return Err(Refusal::new(
            RefusalCode::UnknownMoistureVsTml403,
            format!(
                "lot '{}': prepared moisture upper bound >= TML",
                prepared.lot_id
            ),
        ));
    }
    let expected_margin = prepared
        .tml_fraction
        .checked_sub(prepared.moisture_upper_bound)
        .map_err(map_units_err)?;
    if cmp_qty(prepared.safety_margin_below_tml, expected_margin)?.is_ne() {
        return Err(Refusal::new(
            RefusalCode::Conservation101,
            format!(
                "lot '{}': safety_margin_below_tml inconsistent with tml - moisture_upper_bound",
                prepared.lot_id
            ),
        ));
    }
    let expected_water = prepared
        .wet_mass_kg
        .checked_mul(prepared.moisture_fraction)
        .map_err(map_units_err)?;
    let expected_dry = prepared
        .wet_mass_kg
        .checked_sub(expected_water)
        .map_err(map_units_err)?;
    if cmp_qty(prepared.water_mass_kg, expected_water)?.is_ne()
        || cmp_qty(prepared.dry_mass_kg, expected_dry)?.is_ne()
    {
        return Err(Refusal::new(
            RefusalCode::Conservation101,
            format!(
                "lot '{}': wet/dry/water mass conservation violated",
                prepared.lot_id
            ),
        ));
    }
    Ok(())
}

/// Release a prepared concentrate shipment ONLY with valid dual human attestation (`BD-180`,
/// plan §17.6 & §31.3, `PO-SEC-3`, `INV-002`).
///
/// Re-verifies prepared digest integrity, physical mass conservation, TML clearance, and moisture
/// freshness at `now_ns` BEFORE verifying or consuming human tokens so tampered or stale clearances
/// refuse cleanly without burning tokens.
pub fn release_shipment_with_dual_attestation(
    prepared: &PreparedShipmentRelease,
    verifier: &mut HumanTokenVerifier,
    keyring: &FmKeyRing,
    first_token: &Macaroon,
    second_token: &Macaroon,
    now_ns: i64,
) -> Result<ShipmentReleaseReceipt, Refusal> {
    // Re-verify prepared shipment integrity, mass conservation, and TML safety BEFORE consuming tokens.
    let expected_digest = compute_prepared_effect_digest(prepared)?;
    if !constant_time_eq_32(&expected_digest, &prepared.effect_digest) {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "lot '{}': PreparedShipmentRelease fields do not match effect_digest",
                prepared.lot_id
            ),
        ));
    }
    validate_prepared_shipment_invariants(prepared)?;
    check_moisture_fresh(
        &prepared.lot_id,
        prepared.moisture_measured_at_ns,
        prepared.moisture_max_age_ns,
        now_ns,
    )?;

    let dual_attestation = verifier
        .verify_dual_attestation(
            keyring,
            first_token,
            second_token,
            &prepared.site_id,
            &prepared.effect_digest,
            now_ns,
        )
        .map_err(|e| e.to_refusal())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(SHIPMENT_RECEIPT_DOMAIN);
    buf.extend_from_slice(&prepared.effect_digest);
    push_len_str(&mut buf, &dual_attestation.first.human_id)?;
    buf.extend_from_slice(&dual_attestation.first.nonce);
    push_len_str(&mut buf, &dual_attestation.second.human_id)?;
    buf.extend_from_slice(&dual_attestation.second.nonce);
    buf.extend_from_slice(&now_ns.to_le_bytes());
    let receipt_digest = sha256(&buf);

    Ok(ShipmentReleaseReceipt {
        prepared: prepared.clone(),
        dual_attestation,
        released_at_ns: now_ns,
        execution_authorized: false,
        receipt_digest,
    })
}

/// One gold/silver doré bar admitted into the append-only vault custody log (`BD-179`, plan §17.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoreBarInput {
    pub site_id: String,
    pub bar_id: String,
    pub seal_number: String,
    /// Gross bar weight in grams (strictly positive).
    pub gross_mass_g: ExactQty,
    /// Top pin/drill fire-assay Au mass fraction in `(0, 1]`.
    pub top_pin_au_fraction: ExactQty,
    /// Bottom pin/drill fire-assay Au mass fraction in `(0, 1]`.
    pub bottom_pin_au_fraction: ExactQty,
    /// Silver mass fraction in `[0, 1)`.
    pub ag_fraction: ExactQty,
    /// Maximum tolerated `|top_pin - bottom_pin|` heterogeneity spread before refusing bar admission.
    pub max_pin_spread_fraction: ExactQty,
}

/// Hash-chained immutable doré vault custody entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoreVaultEntry {
    pub seq: u64,
    pub site_id: String,
    pub bar_id: String,
    pub seal_number: String,
    pub gross_mass_g: ExactQty,
    pub mean_au_fraction: ExactQty,
    pub contained_au_g: ExactQty,
    pub contained_ag_g: ExactQty,
    pub prev_hash: Digest32,
    pub entry_hash: Digest32,
}

/// Append-only doré seal and vault custody log (`BD-179`, plan §17.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoreVaultLog {
    site_id: String,
    entries: Vec<DoreVaultEntry>,
    seen_bar_ids: BTreeSet<String>,
    seen_seals: BTreeSet<String>,
    total_au_g: ExactQty,
    total_ag_g: ExactQty,
}

impl DoreVaultLog {
    /// Create a new append-only vault log for one named site.
    pub fn new(site_id: impl Into<String>) -> Result<Self, Refusal> {
        let site_id = site_id.into();
        if site_id.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "DoreVaultLog site_id must be non-empty",
            ));
        }
        Ok(Self {
            site_id,
            entries: Vec::new(),
            seen_bar_ids: BTreeSet::new(),
            seen_seals: BTreeSet::new(),
            total_au_g: ExactQty::from_integer(0),
            total_ag_g: ExactQty::from_integer(0),
        })
    }

    /// Append one doré bar after validating physical assay bounds, top/bottom pin spread, and
    /// unique bar/seal identifiers.
    pub fn append_bar(&mut self, bar: &DoreBarInput) -> Result<DoreVaultEntry, Refusal> {
        if bar.site_id != self.site_id {
            return Err(Refusal::new(
                RefusalCode::VentureWall302,
                format!(
                    "bar '{}' belongs to site '{}', not vault site '{}'",
                    bar.bar_id, bar.site_id, self.site_id
                ),
            ));
        }
        if bar.bar_id.trim().is_empty() || bar.seal_number.trim().is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "doré bar requires non-empty bar_id and seal_number",
            ));
        }
        if self.seen_bar_ids.contains(&bar.bar_id) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("duplicate doré bar_id '{}'", bar.bar_id),
            ));
        }
        if self.seen_seals.contains(&bar.seal_number) {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                format!("duplicate tamper-evident seal '{}'", bar.seal_number),
            ));
        }

        let zero = ExactQty::from_integer(0);
        let one = ExactQty::from_integer(1);
        let two = ExactQty::from_integer(2);
        if bar.gross_mass_g.signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "doré gross_mass_g must be strictly positive",
            ));
        }
        for (name, val) in [
            ("top_pin_au_fraction", bar.top_pin_au_fraction),
            ("bottom_pin_au_fraction", bar.bottom_pin_au_fraction),
        ] {
            if cmp_qty(val, zero)?.is_le() || cmp_qty(val, one)?.is_gt() {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("doré {name} must lie in (0, 1]"),
                ));
            }
        }
        if cmp_qty(bar.ag_fraction, zero)?.is_lt() || cmp_qty(bar.ag_fraction, one)?.is_ge() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "doré ag_fraction must lie in [0, 1)",
            ));
        }
        if cmp_qty(bar.max_pin_spread_fraction, zero)?.is_lt() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "max_pin_spread_fraction must be >= 0",
            ));
        }

        let pin_diff = bar
            .top_pin_au_fraction
            .checked_sub(bar.bottom_pin_au_fraction)
            .map_err(map_units_err)?;
        let abs_pin_diff = if pin_diff.signum() < 0 {
            zero.checked_sub(pin_diff).map_err(map_units_err)?
        } else {
            pin_diff
        };
        if cmp_qty(abs_pin_diff, bar.max_pin_spread_fraction)?.is_gt() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "bar '{}': top/bottom pin Au spread {}/{} exceeds max {}/{}; re-assay required",
                    bar.bar_id,
                    abs_pin_diff.num(),
                    abs_pin_diff.den(),
                    bar.max_pin_spread_fraction.num(),
                    bar.max_pin_spread_fraction.den()
                ),
            ));
        }

        let mean_au_fraction = bar
            .top_pin_au_fraction
            .checked_add(bar.bottom_pin_au_fraction)
            .map_err(map_units_err)?
            .checked_div(two)
            .map_err(map_units_err)?;
        let total_precious = mean_au_fraction
            .checked_add(bar.ag_fraction)
            .map_err(map_units_err)?;
        if cmp_qty(total_precious, one)?.is_gt() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("bar '{}': mean Au + Ag fraction exceeds 1", bar.bar_id),
            ));
        }

        let contained_au_g = bar
            .gross_mass_g
            .checked_mul(mean_au_fraction)
            .map_err(map_units_err)?;
        let contained_ag_g = bar
            .gross_mass_g
            .checked_mul(bar.ag_fraction)
            .map_err(map_units_err)?;

        let seq = u64::try_from(self.entries.len()).map_err(|_| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "vault log length overflow",
            )
        })?;
        let prev_hash = self
            .entries
            .last()
            .map_or(Digest32([0u8; 32]), |e| e.entry_hash);

        let mut buf = Vec::new();
        buf.extend_from_slice(DORE_VAULT_DOMAIN);
        buf.extend_from_slice(&seq.to_le_bytes());
        buf.extend_from_slice(prev_hash.as_bytes());
        push_len_str(&mut buf, &bar.site_id)?;
        push_len_str(&mut buf, &bar.bar_id)?;
        push_len_str(&mut buf, &bar.seal_number)?;
        push_exact_qty(&mut buf, bar.gross_mass_g);
        push_exact_qty(&mut buf, mean_au_fraction);
        push_exact_qty(&mut buf, contained_au_g);
        push_exact_qty(&mut buf, contained_ag_g);
        let entry_hash = sha256(&buf);

        let new_total_au = self
            .total_au_g
            .checked_add(contained_au_g)
            .map_err(map_units_err)?;
        let new_total_ag = self
            .total_ag_g
            .checked_add(contained_ag_g)
            .map_err(map_units_err)?;

        let entry = DoreVaultEntry {
            seq,
            site_id: bar.site_id.clone(),
            bar_id: bar.bar_id.clone(),
            seal_number: bar.seal_number.clone(),
            gross_mass_g: bar.gross_mass_g,
            mean_au_fraction,
            contained_au_g,
            contained_ag_g,
            prev_hash,
            entry_hash,
        };
        self.seen_bar_ids.insert(bar.bar_id.clone());
        self.seen_seals.insert(bar.seal_number.clone());
        self.total_au_g = new_total_au;
        self.total_ag_g = new_total_ag;
        self.entries.push(entry.clone());
        Ok(entry)
    }

    #[must_use]
    pub fn entries(&self) -> &[DoreVaultEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn total_au_g(&self) -> ExactQty {
        self.total_au_g
    }

    #[must_use]
    pub const fn total_ag_g(&self) -> ExactQty {
        self.total_ag_g
    }
}

/// Outturn reconciliation verdict comparing refinery credited Au vs mine fire-assay Au (`BD-179`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineryOutturnAudit {
    pub shipment_id: String,
    pub mine_au_g: ExactQty,
    pub refinery_credited_au_g: ExactQty,
    pub outturn_minus_mine_g: ExactQty,
    pub relative_bias: ExactQty,
    pub tolerance: ExactQty,
    pub investigation_case_required: bool,
    /// Per plan §17.6: doré custody discrepancies above tolerance carry restricted visibility.
    pub restricted_visibility: bool,
}

/// Reconcile refinery credited Au against mine bar fire assays (never clamping discrepancies).
pub fn audit_refinery_outturn(
    shipment_id: &str,
    mine_au_g: ExactQty,
    refinery_credited_au_g: ExactQty,
    tolerance_bps: u32,
) -> Result<RefineryOutturnAudit, Refusal> {
    if shipment_id.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "refinery outturn shipment_id must be non-empty",
        ));
    }
    if mine_au_g.signum() <= 0 || refinery_credited_au_g.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "mine_au_g and refinery_credited_au_g must be strictly positive",
        ));
    }
    if tolerance_bps == 0 || tolerance_bps >= 10_000 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("tolerance_bps must be in 1..=9999, got {tolerance_bps}"),
        ));
    }
    let tolerance = ExactQty::new(i128::from(tolerance_bps), 10_000).map_err(map_units_err)?;
    let outturn_minus_mine_g = refinery_credited_au_g
        .checked_sub(mine_au_g)
        .map_err(map_units_err)?;
    let relative_bias = outturn_minus_mine_g
        .checked_div(mine_au_g)
        .map_err(map_units_err)?;
    let abs_bias = if relative_bias.signum() < 0 {
        ExactQty::from_integer(0)
            .checked_sub(relative_bias)
            .map_err(map_units_err)?
    } else {
        relative_bias
    };
    let investigation_case_required = cmp_qty(abs_bias, tolerance)?.is_gt();
    Ok(RefineryOutturnAudit {
        shipment_id: shipment_id.to_string(),
        mine_au_g,
        refinery_credited_au_g,
        outturn_minus_mine_g,
        relative_bias,
        tolerance,
        investigation_case_required,
        restricted_visibility: investigation_case_required,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_authz::human_token::{
        HumanAuthorityMintRequest, MAX_HUMAN_TOKEN_TTL_NS, mint_human_authority_token,
    };
    use fmine_authz::macaroon::{KeyId, SecretKey};

    fn q(n: i128, d: i128) -> ExactQty {
        ExactQty::new(n, d).unwrap()
    }

    fn base_lot() -> ConcentrateLotSpec {
        ConcentrateLotSpec {
            site_id: "barrick_lumwana".to_string(),
            lot_id: "CONC-2026-041".to_string(),
            product_kind: CustodyProductKind::CopperConcentrate,
            wet_mass_kg: ExactQty::from_integer(1_000_000),
            moisture: Some(MoistureDetermination {
                moisture_fraction: q(82, 1000),         // 8.2%
                upper_confidence_fraction: q(87, 1000), // 8.7% conservative upper bound
                measured_at_ns: 1_000_000_000_000,
                max_age_ns: 24 * 3_600 * 1_000_000_000, // 24 h
                certificate_id: "IMSBC-CERT-2026-041".to_string(),
            }),
            tml_fraction: Some(q(95, 1000)), // 9.5% TML
            evaluated_at_ns: 1_000_000_000_000 + 3_600 * 1_000_000_000, // 1 h old
        }
    }

    #[test]
    fn test_concentrate_moisture_tml_refusals_fmine_ref_403() {
        // 1. Valid concentrate lot below TML produces exact dry/water split & margin.
        let valid = prepare_concentrate_shipment(&base_lot()).unwrap();
        assert_eq!(valid.dry_mass_kg, ExactQty::from_integer(918_000));
        assert_eq!(valid.water_mass_kg, ExactQty::from_integer(82_000));
        assert_eq!(valid.safety_margin_below_tml, q(8, 1000)); // 9.5% - 8.7% = 0.8%

        // 2. Missing moisture determination -> FMINE-REF-403.
        let mut missing_m = base_lot();
        missing_m.moisture = None;
        assert_eq!(
            prepare_concentrate_shipment(&missing_m).unwrap_err().code,
            RefusalCode::UnknownMoistureVsTml403.spec().code
        );

        // 3. Missing TML -> FMINE-REF-403.
        let mut missing_tml = base_lot();
        missing_tml.tml_fraction = None;
        assert_eq!(
            prepare_concentrate_shipment(&missing_tml).unwrap_err().code,
            RefusalCode::UnknownMoistureVsTml403.spec().code
        );

        // 4. Stale moisture determination (> max_age_ns) -> FMINE-REF-403.
        let mut stale = base_lot();
        stale.evaluated_at_ns = 1_000_000_000_000 + 25 * 3_600 * 1_000_000_000;
        assert_eq!(
            prepare_concentrate_shipment(&stale).unwrap_err().code,
            RefusalCode::UnknownMoistureVsTml403.spec().code
        );

        // 5. Point moisture below TML, BUT upper confidence bound == TML -> FMINE-REF-403!
        let mut ucb_at_tml = base_lot();
        ucb_at_tml
            .moisture
            .as_mut()
            .unwrap()
            .upper_confidence_fraction = q(95, 1000);
        assert_eq!(
            prepare_concentrate_shipment(&ucb_at_tml).unwrap_err().code,
            RefusalCode::UnknownMoistureVsTml403.spec().code
        );

        // 6. Moisture above TML -> FMINE-REF-403.
        let mut above_tml = base_lot();
        above_tml.moisture.as_mut().unwrap().moisture_fraction = q(96, 1000);
        above_tml
            .moisture
            .as_mut()
            .unwrap()
            .upper_confidence_fraction = q(99, 1000);
        assert_eq!(
            prepare_concentrate_shipment(&above_tml).unwrap_err().code,
            RefusalCode::UnknownMoistureVsTml403.spec().code
        );
    }

    #[test]
    fn test_dore_vault_append_only_seal_log_and_refinery_outturn_discrepancy() {
        let mut vault = DoreVaultLog::new("barrick_kibali").unwrap();
        let bar1 = DoreBarInput {
            site_id: "barrick_kibali".to_string(),
            bar_id: "KIB-2026-B001".to_string(),
            seal_number: "SEAL-9001".to_string(),
            gross_mass_g: ExactQty::from_integer(25_000),
            top_pin_au_fraction: q(880, 1000),
            bottom_pin_au_fraction: q(882, 1000),
            ag_fraction: q(95, 1000),
            max_pin_spread_fraction: q(5, 1000),
        };
        let e1 = vault.append_bar(&bar1).unwrap();
        assert_eq!(e1.seq, 0);
        assert_eq!(e1.mean_au_fraction, q(881, 1000));
        assert_eq!(e1.contained_au_g, ExactQty::from_integer(22_025));
        assert_eq!(vault.total_au_g(), ExactQty::from_integer(22_025));

        // Duplicate seal number refused with FMINE-REF-507.
        let mut dup_seal = bar1.clone();
        dup_seal.bar_id = "KIB-2026-B002".to_string();
        assert_eq!(
            vault.append_bar(&dup_seal).unwrap_err().code,
            RefusalCode::FrozenRecordMutation507.spec().code
        );

        // Excessive top/bottom pin spread refused with FMINE-REF-109.
        let mut bad_spread = bar1;
        bad_spread.bar_id = "KIB-2026-B003".to_string();
        bad_spread.seal_number = "SEAL-9003".to_string();
        bad_spread.bottom_pin_au_fraction = q(895, 1000); // spread 1.5% > 0.5%
        assert_eq!(
            vault.append_bar(&bad_spread).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // Refinery outturn: within 15 bps passes cleanly; -35 bps opens restricted InvestigationCase.
        let ok_outturn = audit_refinery_outturn(
            "SHIP-001",
            ExactQty::from_integer(22_025),
            q(22_000, 1), // -25/22025 ≈ -11.35 bps <= 15 bps
            15,
        )
        .unwrap();
        assert!(!ok_outturn.investigation_case_required);
        assert!(!ok_outturn.restricted_visibility);

        let bad_outturn = audit_refinery_outturn(
            "SHIP-002",
            ExactQty::from_integer(22_025),
            q(21_940, 1), // -85/22025 ≈ -38.6 bps > 15 bps
            15,
        )
        .unwrap();
        assert!(bad_outturn.investigation_case_required);
        assert!(bad_outturn.restricted_visibility);
    }

    #[test]
    fn test_shipment_release_dual_attestation_tokens_and_staleness_guard() {
        let prepared = prepare_concentrate_shipment(&base_lot()).unwrap();
        let active_key = SecretKey::from_bytes([0x5A; 32], "active").unwrap();
        let retired_key = SecretKey::from_bytes([0xA5; 32], "retired").unwrap();
        let keyring = FmKeyRing::new(
            KeyId::new("k-site-active").unwrap(),
            active_key,
            KeyId::new("k-site-retired").unwrap(),
            retired_key,
        )
        .unwrap();
        let mut verifier = HumanTokenVerifier::new();

        let now_ns = 1_000_000_000_000 + 4_000 * 1_000_000_000; // within 24h moisture age
        let ttl = MAX_HUMAN_TOKEN_TTL_NS; // 15 min

        let tok_alice = mint_human_authority_token(
            &keyring,
            &HumanAuthorityMintRequest {
                token_id: "t-alice".to_string(),
                human_id: "qp.alice".to_string(),
                site: "barrick_lumwana".to_string(),
                effect_digest: prepared.effect_digest,
                issued_at_ns: now_ns - 10_000_000_000,
                ttl_ns: ttl,
                single_use_nonce: [0x11; 16],
            },
        )
        .unwrap();

        let tok_bob = mint_human_authority_token(
            &keyring,
            &HumanAuthorityMintRequest {
                token_id: "t-bob".to_string(),
                human_id: "met.bob".to_string(),
                site: "barrick_lumwana".to_string(),
                effect_digest: prepared.effect_digest,
                issued_at_ns: now_ns - 5_000_000_000,
                ttl_ns: ttl,
                single_use_nonce: [0x22; 16],
            },
        )
        .unwrap();

        // 1. Same human presenting two distinct tokens refused with FMINE-REF-304.
        let tok_alice_2 = mint_human_authority_token(
            &keyring,
            &HumanAuthorityMintRequest {
                token_id: "t-alice-2".to_string(),
                human_id: "qp.alice".to_string(),
                site: "barrick_lumwana".to_string(),
                effect_digest: prepared.effect_digest,
                issued_at_ns: now_ns - 5_000_000_000,
                ttl_ns: ttl,
                single_use_nonce: [0x33; 16],
            },
        )
        .unwrap();
        assert_eq!(
            release_shipment_with_dual_attestation(
                &prepared,
                &mut verifier,
                &keyring,
                &tok_alice,
                &tok_alice_2,
                now_ns,
            )
            .unwrap_err()
            .code,
            RefusalCode::MissingHumanToken304.spec().code
        );
        assert_eq!(verifier.consumed_count(), 0);

        // 2. Stale moisture at release time refuses with FMINE-REF-403 WITHOUT consuming tokens!
        let stale_now = prepared.moisture_measured_at_ns + prepared.moisture_max_age_ns + 1;
        assert_eq!(
            release_shipment_with_dual_attestation(
                &prepared,
                &mut verifier,
                &keyring,
                &tok_alice,
                &tok_bob,
                stale_now,
            )
            .unwrap_err()
            .code,
            RefusalCode::UnknownMoistureVsTml403.spec().code
        );
        assert_eq!(verifier.consumed_count(), 0);

        // 2b. Tampered PreparedShipmentRelease refuses with FMINE-REF-505 WITHOUT consuming tokens!
        let mut tampered_prepared = prepared.clone();
        tampered_prepared.wet_mass_kg = ExactQty::from_integer(2_000_000);
        assert_eq!(
            release_shipment_with_dual_attestation(
                &tampered_prepared,
                &mut verifier,
                &keyring,
                &tok_alice,
                &tok_bob,
                now_ns,
            )
            .unwrap_err()
            .code,
            RefusalCode::DigestMismatch505.spec().code
        );
        assert_eq!(verifier.consumed_count(), 0);

        // 2c. Re-signed PreparedShipmentRelease with violated mass conservation refuses with FMINE-REF-101 WITHOUT burning tokens!
        let mut bad_mass = prepared.clone();
        bad_mass.dry_mass_kg = ExactQty::from_integer(1);
        bad_mass.effect_digest = compute_prepared_effect_digest(&bad_mass).unwrap();
        assert_eq!(
            release_shipment_with_dual_attestation(
                &bad_mass,
                &mut verifier,
                &keyring,
                &tok_alice,
                &tok_bob,
                now_ns,
            )
            .unwrap_err()
            .code,
            RefusalCode::Conservation101.spec().code
        );
        assert_eq!(verifier.consumed_count(), 0);

        // Fresh-eyes regression: doré bar with zero Au pin assay or 100% Ag refuses with FMINE-REF-109.
        let mut vault = DoreVaultLog::new("barrick_lumwana").unwrap();
        assert_eq!(
            vault
                .append_bar(&DoreBarInput {
                    site_id: "barrick_lumwana".to_string(),
                    bar_id: "ZERO-AU".to_string(),
                    seal_number: "SEAL-ZERO".to_string(),
                    gross_mass_g: ExactQty::from_integer(10_000),
                    top_pin_au_fraction: ExactQty::from_integer(0),
                    bottom_pin_au_fraction: ExactQty::from_integer(0),
                    ag_fraction: q(50, 100),
                    max_pin_spread_fraction: q(5, 1000),
                })
                .unwrap_err()
                .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );

        // 3. Wrong effect digest refused with FMINE-REF-304.
        let mut wrong_digest = prepared.effect_digest;
        wrong_digest[0] ^= 0xFF;
        let tok_wrong_digest = mint_human_authority_token(
            &keyring,
            &HumanAuthorityMintRequest {
                token_id: "t-wrong".to_string(),
                human_id: "met.bob".to_string(),
                site: "barrick_lumwana".to_string(),
                effect_digest: wrong_digest,
                issued_at_ns: now_ns - 5_000_000_000,
                ttl_ns: ttl,
                single_use_nonce: [0x44; 16],
            },
        )
        .unwrap();
        assert_eq!(
            release_shipment_with_dual_attestation(
                &prepared,
                &mut verifier,
                &keyring,
                &tok_alice,
                &tok_wrong_digest,
                now_ns,
            )
            .unwrap_err()
            .code,
            RefusalCode::MissingHumanToken304.spec().code
        );

        // 4. Expired token (> 15 min) refused with FMINE-REF-304.
        let expired_now = now_ns + ttl + 1;
        assert_eq!(
            release_shipment_with_dual_attestation(
                &prepared,
                &mut verifier,
                &keyring,
                &tok_alice,
                &tok_bob,
                expired_now,
            )
            .unwrap_err()
            .code,
            RefusalCode::MissingHumanToken304.spec().code
        );

        // 5. Valid dual human attestation succeeds, sets execution_authorized = false, and consumes both nonces.
        let receipt = release_shipment_with_dual_attestation(
            &prepared,
            &mut verifier,
            &keyring,
            &tok_alice,
            &tok_bob,
            now_ns,
        )
        .unwrap();
        assert!(!receipt.execution_authorized);
        assert_eq!(receipt.dual_attestation.first.human_id, "qp.alice");
        assert_eq!(receipt.dual_attestation.second.human_id, "met.bob");
        assert_eq!(verifier.consumed_count(), 2);

        // 6. Replay of consumed tokens refused with FMINE-REF-304.
        assert_eq!(
            release_shipment_with_dual_attestation(
                &prepared,
                &mut verifier,
                &keyring,
                &tok_alice,
                &tok_bob,
                now_ns,
            )
            .unwrap_err()
            .code,
            RefusalCode::MissingHumanToken304.spec().code
        );
    }
}
