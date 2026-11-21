//! Determinism: BitwisePortable
//!
//! MNPI & joint-venture wall caveat compilation, wall-scoped query non-interference (`PO-SEC-1`),
//! monotone taint export guard (`PO-SEC-2`, `INV-007`), and hash-chained insider-list-ready
//! `AccessEvent` logging (`BD-093`, plan §31.1–§31.2).

use fmine_claim::{Confidentiality, LabelRegistry, WallId};
use fmine_core::codec::Encoder;
use fmine_core::hash::sha256;
use fmine_evidence::RefusalCode;

use crate::AuthzError;
use crate::macaroon::{Caveat, FmKeyRing, Macaroon, PrincipalKind};

/// Embargo / disclosure lifecycle attached to sensitive exploration or assay records (§31.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisclosureState {
    /// Embargoed behind an MNPI wall since `since_ns`.
    Embargoed { wall: WallId, since_ns: i64 },
    /// Publicly released citing a filing or news-release reference and its valid time.
    Released {
        public_ref: String,
        released_at_ns: i64,
    },
}

/// Principal clearance compiled from a verified first-party macaroon before query expansion (§31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledClearance {
    pub principal_kind: PrincipalKind,
    pub principal_id: String,
    pub clearance: Confidentiality,
    /// Only `PrincipalKind::Human` carrying `Caveat::MnpiRelease` may release embargoed rows.
    pub can_release_mnpi: bool,
    /// Whether the principal holds an explicit cross-venture capability (`Caveat::CrossVenture`).
    pub can_cross_venture: bool,
}

/// Compiles a verified first-party macaroon into a [`CompiledClearance`] against `registry` at `now_ns`.
pub fn compile_clearance(
    keyring: &FmKeyRing,
    token: &Macaroon,
    registry: &LabelRegistry,
    now_ns: i64,
) -> Result<CompiledClearance, AuthzError> {
    keyring.verify(token)?;

    let mut principal: Option<(PrincipalKind, String)> = None;
    let mut clearance = Confidentiality::public();
    let mut has_internal_scope = false;
    let mut wants_mnpi_release = false;
    let mut can_cross_venture = false;
    let mut issued_at_ns: Option<i64> = None;
    let mut expires_at_ns: Option<i64> = None;

    for caveat in &token.caveats {
        match caveat {
            Caveat::Principal { kind, id } => {
                if principal.replace((*kind, id.clone())).is_some() {
                    return Err(AuthzError::SignatureInvalid {
                        detail: "multiple Principal caveats in clearance token".to_string(),
                    });
                }
            }
            Caveat::Scope(s) if s != "public_read" => {
                has_internal_scope = true;
            }
            Caveat::Venture(v) => {
                let vid = registry
                    .venture(v)
                    .map_err(|_| AuthzError::VentureWallRefused {
                        detail: format!("unregistered venture caveat '{v}'"),
                    })?;
                clearance = clearance.join(&Confidentiality::venture(vid));
            }
            Caveat::MnpiWall(w) => {
                let wid = registry.wall(w).map_err(|_| AuthzError::MnpiWallRefused {
                    detail: format!("unregistered MNPI wall caveat '{w}'"),
                })?;
                clearance = clearance.join(&Confidentiality::mnpi(wid));
            }
            Caveat::MnpiRelease => wants_mnpi_release = true,
            Caveat::CrossVenture => can_cross_venture = true,
            Caveat::IssuedAtNs(ns) => {
                issued_at_ns = Some(issued_at_ns.map_or(*ns, |prev| prev.max(*ns)));
            }
            Caveat::ExpiresAtNs(ns) => {
                expires_at_ns = Some(expires_at_ns.map_or(*ns, |prev| prev.min(*ns)));
            }
            _ => {}
        }
    }

    if let Some(issued) = issued_at_ns
        && now_ns < issued
    {
        return Err(AuthzError::TokenTemporalInvalid {
            detail: format!("clearance token not yet valid: {now_ns} < {issued}"),
        });
    }
    if let Some(exp) = expires_at_ns
        && now_ns > exp
    {
        return Err(AuthzError::TokenTemporalInvalid {
            detail: format!("clearance token expired: {now_ns} > {exp}"),
        });
    }

    let (principal_kind, principal_id) = principal.ok_or_else(|| AuthzError::SignatureInvalid {
        detail: "clearance token missing Principal caveat".to_string(),
    })?;

    if has_internal_scope && clearance.is_public() {
        clearance = Confidentiality::internal();
    }

    // §31.1 / §30.4: only a human principal may hold `mnpi_release`.
    if wants_mnpi_release && principal_kind != PrincipalKind::Human {
        return Err(AuthzError::MnpiWallRefused {
            detail: format!(
                "non-human principal ({principal_kind:?} '{principal_id}') cannot hold mnpi_release"
            ),
        });
    }

    Ok(CompiledClearance {
        principal_kind,
        principal_id,
        clearance,
        can_release_mnpi: wants_mnpi_release,
        can_cross_venture,
    })
}

/// Kind of access recorded in the insider-list-ready [`AccessLog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessAction {
    Query,
    ReadObject,
    PublicExport,
}

impl AccessAction {
    const fn tag(self) -> u8 {
        match self {
            Self::Query => 1,
            Self::ReadObject => 2,
            Self::PublicExport => 3,
        }
    }
}

/// Recorded disposition of an access attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    Allowed,
    Refused(RefusalCode),
}

/// One hash-chained entry in the insider-list-ready access audit log (§31.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessEvent {
    pub seq: u64,
    pub timestamp_ns: i64,
    pub principal_kind: PrincipalKind,
    pub principal_id: String,
    pub action: AccessAction,
    pub target_id: String,
    pub walls_touched: Vec<String>,
    pub ventures_touched: Vec<String>,
    pub decision: AccessDecision,
    pub prev_digest: [u8; 32],
    pub event_digest: [u8; 32],
}

/// Append-only hash-chained access log supporting insider-list audits per MNPI wall (§31.1).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AccessLog {
    events: Vec<AccessEvent>,
    head_digest: [u8; 32],
}

impl AccessLog {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: Vec::new(),
            head_digest: [0u8; 32],
        }
    }

    #[must_use]
    pub fn events(&self) -> &[AccessEvent] {
        &self.events
    }

    #[must_use]
    pub const fn head_digest(&self) -> [u8; 32] {
        self.head_digest
    }

    /// Appends a hash-chained `AccessEvent` and returns a reference to it.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        timestamp_ns: i64,
        clearance: &CompiledClearance,
        action: AccessAction,
        target_id: impl Into<String>,
        walls_touched: Vec<String>,
        ventures_touched: Vec<String>,
        decision: AccessDecision,
    ) -> &AccessEvent {
        let seq = (self.events.len() as u64) + 1;
        let target_id = target_id.into();
        let prev_digest = self.head_digest;
        let event_digest = compute_event_digest(
            seq,
            timestamp_ns,
            clearance.principal_kind,
            &clearance.principal_id,
            action,
            &target_id,
            &walls_touched,
            &ventures_touched,
            &decision,
            &prev_digest,
        );
        self.head_digest = event_digest;
        self.events.push(AccessEvent {
            seq,
            timestamp_ns,
            principal_kind: clearance.principal_kind,
            principal_id: clearance.principal_id.clone(),
            action,
            target_id,
            walls_touched,
            ventures_touched,
            decision,
            prev_digest,
            event_digest,
        });
        &self.events[self.events.len() - 1]
    }

    /// Recomputes every link in the chain and returns `true` iff every digest is intact.
    #[must_use]
    pub fn verify_chain(&self) -> bool {
        let mut expected_prev = [0u8; 32];
        for (idx, ev) in self.events.iter().enumerate() {
            if ev.seq != (idx as u64) + 1 || ev.prev_digest != expected_prev {
                return false;
            }
            let recomputed = compute_event_digest(
                ev.seq,
                ev.timestamp_ns,
                ev.principal_kind,
                &ev.principal_id,
                ev.action,
                &ev.target_id,
                &ev.walls_touched,
                &ev.ventures_touched,
                &ev.decision,
                &ev.prev_digest,
            );
            if recomputed != ev.event_digest {
                return false;
            }
            expected_prev = ev.event_digest;
        }
        expected_prev == self.head_digest
    }

    /// Returns all insider-list access events touching a specific MNPI wall id.
    #[must_use]
    pub fn events_for_wall(&self, wall_id: &str) -> Vec<&AccessEvent> {
        self.events
            .iter()
            .filter(|ev| ev.walls_touched.iter().any(|w| w == wall_id))
            .collect()
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_event_digest(
    seq: u64,
    timestamp_ns: i64,
    principal_kind: PrincipalKind,
    principal_id: &str,
    action: AccessAction,
    target_id: &str,
    walls: &[String],
    ventures: &[String],
    decision: &AccessDecision,
    prev_digest: &[u8; 32],
) -> [u8; 32] {
    let mut enc = Encoder::new_frame(*b"ACEV", 1);
    enc.put_u64(seq);
    enc.put_i64(timestamp_ns);
    enc.put_u8(match principal_kind {
        PrincipalKind::Human => 1,
        PrincipalKind::Agent => 2,
        PrincipalKind::Service => 3,
    });
    enc.put_str(principal_id);
    enc.put_u8(action.tag());
    enc.put_str(target_id);
    enc.put_u32(walls.len() as u32);
    for w in walls {
        enc.put_str(w);
    }
    enc.put_u32(ventures.len() as u32);
    for v in ventures {
        enc.put_str(v);
    }
    match decision {
        AccessDecision::Allowed => enc.put_u8(1),
        AccessDecision::Refused(code) => {
            enc.put_u8(2);
            enc.put_str(code.spec().code);
        }
    }
    enc.put_fixed(prev_digest);
    sha256(&enc.finish()).0
}

/// One labeled item stored inside a wall-scoped catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallRecord<T> {
    pub object_id: String,
    pub label: Confidentiality,
    pub disclosure: Option<DisclosureState>,
    pub payload: T,
}

impl<T> WallRecord<T> {
    /// Effective label including any active [`DisclosureState::Embargoed`] wall.
    #[must_use]
    pub fn effective_label(&self) -> Confidentiality {
        match &self.disclosure {
            Some(DisclosureState::Embargoed { wall, .. }) => {
                self.label.join(&Confidentiality::mnpi(wall.clone()))
            }
            _ => self.label.clone(),
        }
    }
}

/// Wall-scoped query output (`PO-SEC-1`): contains ONLY visible records and their count,
/// never exposing total store size or hidden-wall existence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallQueryResult<T> {
    pub visible_count: usize,
    pub items: Vec<(String, T)>,
}

/// In-memory wall-scoped store enforcing `PO-SEC-1`, `PO-SEC-2`, and `AccessLog` recording.
#[derive(Debug, Clone)]
pub struct WallScopedCatalog<T> {
    records: Vec<WallRecord<T>>,
    log: AccessLog,
}

impl<T> Default for WallScopedCatalog<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> WallScopedCatalog<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
            log: AccessLog::new(),
        }
    }

    pub fn insert(&mut self, record: WallRecord<T>) {
        self.records.push(record);
    }

    #[must_use]
    pub fn access_log(&self) -> &AccessLog {
        &self.log
    }

    /// Runs a wall-scoped query (`PO-SEC-1`): items outside `clearance` are completely excluded from
    /// both `items` and `visible_count` so outputs are indistinguishable with or without hidden walls.
    pub fn query(&mut self, clearance: &CompiledClearance, now_ns: i64) -> WallQueryResult<T>
    where
        T: Clone,
    {
        let mut items = Vec::new();
        let mut touched_walls = Vec::new();
        let mut touched_ventures = Vec::new();
        for rec in &self.records {
            let eff = rec.effective_label();
            if eff.leq(&clearance.clearance) {
                for w in eff.walls() {
                    let s = w.as_str().to_string();
                    if !touched_walls.contains(&s) {
                        touched_walls.push(s);
                    }
                }
                for v in eff.ventures() {
                    let s = v.as_str().to_string();
                    if !touched_ventures.contains(&s) {
                        touched_ventures.push(s);
                    }
                }
                items.push((rec.object_id.clone(), rec.payload.clone()));
            }
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));
        touched_walls.sort();
        touched_ventures.sort();
        let visible_count = items.len();
        self.log.record(
            now_ns,
            clearance,
            AccessAction::Query,
            format!("query:visible={visible_count}"),
            touched_walls,
            touched_ventures,
            AccessDecision::Allowed,
        );
        WallQueryResult {
            visible_count,
            items,
        }
    }

    /// Exports one object to the public lane (`PO-SEC-2`, `INV-007`):
    /// - Any MNPI-tainted or embargoed object is refused (`FMINE-REF-301`) unless the principal is a
    ///   human holding `can_release_mnpi` AND full clearance for the object's label.
    /// - Any venture-labeled or internal object without public clearance is refused (`FMINE-REF-302`
    ///   for venture boundary, `FMINE-REF-301` for internal/MNPI boundary).
    pub fn export_public(
        &mut self,
        clearance: &CompiledClearance,
        object_id: &str,
        now_ns: i64,
    ) -> Result<T, AuthzError>
    where
        T: Clone,
    {
        let Some(rec) = self.records.iter().find(|r| r.object_id == object_id) else {
            return Err(AuthzError::MnpiWallRefused {
                detail: format!("object '{object_id}' not found or not visible"),
            });
        };
        let eff = rec.effective_label();
        let walls: Vec<String> = eff.walls().map(|w| w.as_str().to_string()).collect();
        let ventures: Vec<String> = eff.ventures().map(|v| v.as_str().to_string()).collect();

        if eff.is_public() {
            self.log.record(
                now_ns,
                clearance,
                AccessAction::PublicExport,
                object_id,
                walls,
                ventures,
                AccessDecision::Allowed,
            );
            return Ok(rec.payload.clone());
        }

        // First check whether the principal can even see this label.
        if let Some(code) = eff.refusal_over(&clearance.clearance) {
            self.log.record(
                now_ns,
                clearance,
                AccessAction::PublicExport,
                object_id,
                walls,
                ventures,
                AccessDecision::Refused(code),
            );
            return Err(match code {
                RefusalCode::VentureWall302 => AuthzError::VentureWallRefused {
                    detail: format!("object '{object_id}' crosses venture wall ({eff})"),
                },
                _ => AuthzError::MnpiWallRefused {
                    detail: format!("object '{object_id}' crosses MNPI/internal wall ({eff})"),
                },
            });
        }

        // Even when the reader holds clearance, non-public / embargoed objects NEVER reach a public
        // export unless a human explicitly holds `mnpi_release` and the object is not venture-walled
        // without cross-venture authority.
        let has_ventures = eff.ventures().next().is_some();
        if has_ventures && !clearance.can_cross_venture {
            self.log.record(
                now_ns,
                clearance,
                AccessAction::PublicExport,
                object_id,
                walls,
                ventures,
                AccessDecision::Refused(RefusalCode::VentureWall302),
            );
            return Err(AuthzError::VentureWallRefused {
                detail: format!(
                    "venture-scoped object '{object_id}' cannot export publicly without cross_venture"
                ),
            });
        }
        if !clearance.can_release_mnpi {
            self.log.record(
                now_ns,
                clearance,
                AccessAction::PublicExport,
                object_id,
                walls,
                ventures,
                AccessDecision::Refused(RefusalCode::MnpiWall301),
            );
            return Err(AuthzError::MnpiWallRefused {
                detail: format!(
                    "embargoed/non-public object '{object_id}' ({eff}) refused on public export without human mnpi_release"
                ),
            });
        }

        self.log.record(
            now_ns,
            clearance,
            AccessAction::PublicExport,
            object_id,
            walls,
            ventures,
            AccessDecision::Allowed,
        );
        Ok(rec.payload.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macaroon::{KeyId, SecretKey};

    fn test_ring() -> FmKeyRing {
        FmKeyRing::with_derived_retired(
            KeyId::new("k-wall-1").expect("id"),
            SecretKey::from_bytes([0x77; 32], "active").expect("key"),
            b"wall-test-entropy",
        )
        .expect("ring")
    }

    fn test_registry() -> LabelRegistry {
        LabelRegistry::new(["ngm", "reko_diq"], ["W-DRILL-26", "W-MA-26"]).expect("registry")
    }

    /// PO-SEC-1: differential test proving a store WITH hidden wall/venture objects produces
    /// byte-identical query results and counts as a store WITHOUT them for an outside principal.
    #[test]
    fn test_po_sec_1_wall_differential_hides_data_counts_and_absence_across_walls() {
        let ring = test_ring();
        let reg = test_registry();
        let ngm = reg.venture("ngm").expect("ngm");
        let reko = reg.venture("reko_diq").expect("reko");
        let w_drill = reg.wall("W-DRILL-26").expect("w_drill");
        let w_ma = reg.wall("W-MA-26").expect("w_ma");

        let ngm_partner_tok = ring
            .mint(
                "tok-ngm-partner",
                "fmine://ngm",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Human,
                        id: "partner-newmont".into(),
                    },
                    Caveat::Scope("read".into()),
                    Caveat::Venture("ngm".into()),
                ],
            )
            .expect("mint");
        let ngm_clearance =
            compile_clearance(&ring, &ngm_partner_tok, &reg, 1_000).expect("compile");

        // Store A: only public + NGM venture data.
        let mut store_a = WallScopedCatalog::new();
        store_a.insert(WallRecord {
            object_id: "obj-pub-1".into(),
            label: Confidentiality::public(),
            disclosure: None,
            payload: "Q2 2026 published gold oz",
        });
        store_a.insert(WallRecord {
            object_id: "obj-ngm-1".into(),
            label: Confidentiality::venture(ngm.clone()),
            disclosure: None,
            payload: "NGM Carlin mine plan",
        });

        // Store B: Store A PLUS hidden Reko Diq venture data, MNPI drill intercept behind W-DRILL-26,
        // and cross-tainted NGM+W-MA-26 valuation!
        let mut store_b = store_a.clone();
        store_b.insert(WallRecord {
            object_id: "obj-reko-secret".into(),
            label: Confidentiality::venture(reko),
            disclosure: None,
            payload: "Reko Diq confidential budget",
        });
        store_b.insert(WallRecord {
            object_id: "obj-mnpi-intercept".into(),
            label: Confidentiality::internal(),
            disclosure: Some(DisclosureState::Embargoed {
                wall: w_drill.clone(),
                since_ns: 500,
            }),
            payload: "Hole RD-99 120m @ 1.8% Cu",
        });
        store_b.insert(WallRecord {
            object_id: "obj-ngm-ma-tainted".into(),
            label: Confidentiality::venture(ngm).join(&Confidentiality::mnpi(w_ma)),
            disclosure: None,
            payload: "Confidential M&A valuation",
        });

        let res_a = store_a.query(&ngm_clearance, 1_000);
        let res_b = store_b.query(&ngm_clearance, 1_000);
        // PO-SEC-1: identical results, identical visible_count, zero leakage of hidden objects!
        assert_eq!(res_a, res_b);
        assert_eq!(res_b.visible_count, 2);
        assert!(store_b.access_log().verify_chain());

        // An insider cleared for W-DRILL-26 sees the embargoed intercept and leaves an insider-list event.
        let insider_tok = ring
            .mint(
                "tok-insider",
                "fmine://enterprise",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Human,
                        id: "geo-chief".into(),
                    },
                    Caveat::Scope("read".into()),
                    Caveat::MnpiWall("W-DRILL-26".into()),
                ],
            )
            .expect("mint insider");
        let insider_clearance =
            compile_clearance(&ring, &insider_tok, &reg, 1_000).expect("compile insider");
        let res_insider = store_b.query(&insider_clearance, 1_100);
        assert_eq!(res_insider.visible_count, 2); // obj-pub-1 + obj-mnpi-intercept
        assert_eq!(
            store_b.access_log().events_for_wall(w_drill.as_str()).len(),
            1
        );
        assert!(store_b.access_log().verify_chain());
    }

    /// PO-SEC-2: embargoed and MNPI-derived bytes never reach a public export without a human
    /// `mnpi_release` capability; agents cannot hold `mnpi_release`; venture walls emit FMINE-REF-302.
    #[test]
    fn test_po_sec_2_embargoed_and_mnpi_derived_bytes_refused_on_public_export() {
        let ring = test_ring();
        let reg = test_registry();
        let ngm = reg.venture("ngm").expect("ngm");
        let w_drill = reg.wall("W-DRILL-26").expect("w_drill");

        let mut catalog = WallScopedCatalog::new();
        catalog.insert(WallRecord {
            object_id: "pub-1".into(),
            label: Confidentiality::public(),
            disclosure: None,
            payload: "public",
        });
        catalog.insert(WallRecord {
            object_id: "embargo-1".into(),
            label: Confidentiality::public().join(&Confidentiality::mnpi(w_drill.clone())),
            disclosure: Some(DisclosureState::Embargoed {
                wall: w_drill,
                since_ns: 100,
            }),
            payload: "embargoed-assay",
        });
        catalog.insert(WallRecord {
            object_id: "venture-1".into(),
            label: Confidentiality::venture(ngm),
            disclosure: None,
            payload: "ngm-private",
        });

        let public_tok = ring
            .mint(
                "tok-pub",
                "fmine://public",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Agent,
                        id: "agent-pub".into(),
                    },
                    Caveat::Scope("public_read".into()),
                ],
            )
            .expect("mint");
        let pub_clear = compile_clearance(&ring, &public_tok, &reg, 500).expect("compile");
        assert_eq!(
            catalog
                .export_public(&pub_clear, "pub-1", 500)
                .expect("public ok"),
            "public"
        );
        let err_mnpi = catalog
            .export_public(&pub_clear, "embargo-1", 500)
            .expect_err("embargo refused");
        assert_eq!(
            (err_mnpi.to_refusal().code, err_mnpi.to_refusal().exit_code),
            ("FMINE-REF-301", 7)
        );

        // Internal reader without ngm venture hits FMINE-REF-302 (venture wall).
        let internal_tok = ring
            .mint(
                "tok-int",
                "fmine://corp",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Human,
                        id: "analyst-1".into(),
                    },
                    Caveat::Scope("read".into()),
                ],
            )
            .expect("mint");
        let int_clear = compile_clearance(&ring, &internal_tok, &reg, 500).expect("compile");
        let err_vent = catalog
            .export_public(&int_clear, "venture-1", 500)
            .expect_err("venture refused");
        assert_eq!(
            (err_vent.to_refusal().code, err_vent.to_refusal().exit_code),
            ("FMINE-REF-302", 7)
        );

        // Agent attempting to hold MnpiRelease is refused at clearance compilation!
        let bad_agent = ring
            .mint(
                "tok-bad-agent",
                "fmine://corp",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Agent,
                        id: "agent-rogue".into(),
                    },
                    Caveat::MnpiWall("W-DRILL-26".into()),
                    Caveat::MnpiRelease,
                ],
            )
            .expect("mint");
        assert!(matches!(
            compile_clearance(&ring, &bad_agent, &reg, 500),
            Err(AuthzError::MnpiWallRefused { .. })
        ));

        // Human cleared for W-DRILL-26 WITHOUT MnpiRelease is still refused on public export.
        let insider_no_rel = ring
            .mint(
                "tok-human-no-rel",
                "fmine://corp",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Human,
                        id: "geo-chief".into(),
                    },
                    Caveat::MnpiWall("W-DRILL-26".into()),
                ],
            )
            .expect("mint");
        let no_rel_clear = compile_clearance(&ring, &insider_no_rel, &reg, 500).expect("compile");
        assert!(matches!(
            catalog.export_public(&no_rel_clear, "embargo-1", 500),
            Err(AuthzError::MnpiWallRefused { .. })
        ));

        // Human WITH W-DRILL-26 AND MnpiRelease may release.
        let releaser = insider_no_rel
            .attenuate(Caveat::MnpiRelease)
            .expect("attenuate");
        let rel_clear = compile_clearance(&ring, &releaser, &reg, 500).expect("compile");
        assert_eq!(
            catalog
                .export_public(&rel_clear, "embargo-1", 500)
                .expect("human release"),
            "embargoed-assay"
        );
        assert!(catalog.access_log().verify_chain());
    }
}
