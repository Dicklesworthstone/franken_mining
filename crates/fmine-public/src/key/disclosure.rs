//! Registered disclosure coverage (BD-504e.2/.3). A note can explain a comparison only after its
//! complete quote is re-read from the registered, visible source cell. Coverage is explicit data;
//! neither text search nor a changed value invents a scope.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::xlsx::parse_a1_ref;

use super::{KeyedCell, Period, PublicKey, parse_period_code};
use crate::sources::{IssuerProfile, RestatementNoteRow, RestatementScopeRow};
use crate::{LoadedSource, js};

/// The verified note and the exact scope that made it applicable. A disclosure is evidence of
/// registered coverage, not proof that either of the compared assertions is true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureCitation {
    pub note_id: String,
    pub at: String,
    pub sha256: String,
    pub url: String,
    pub source_release: String,
    pub from_quarter: String,
    pub quoted_text: String,
    pub evidence: String,
    pub scope: RestatementScopeRow,
    pub unit: String,
}

impl DisclosureCitation {
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"note_id\":{},\"at\":{},\"sha256\":{},\"url\":{},\"source_release\":{},\
             \"from_quarter\":{},\"quoted_text\":{},\"evidence\":{},\"scope\":{{\"subject\":{},\
             \"period_from\":{},\"period_to\":{},\"line\":{},\"occurrence\":{},\"unit\":{}}}}}",
            js(&self.note_id),
            js(&self.at),
            js(&self.sha256),
            js(&self.url),
            js(&self.source_release),
            js(&self.from_quarter),
            js(&self.quoted_text),
            js(&self.evidence),
            js(&self.scope.subject),
            js(&self.scope.period_from),
            js(&self.scope.period_to),
            js(&self.scope.line),
            self.scope.occurrence,
            js(&self.unit),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureRefusal {
    pub note_id: String,
    pub at: String,
    pub refusal: Refusal,
}

impl DisclosureRefusal {
    fn to_json(&self) -> String {
        format!(
            "{{\"note_id\":{},\"at\":{},\"code\":{},\"what\":{},\"fix\":{}}}",
            js(&self.note_id),
            js(&self.at),
            js(self.refusal.code),
            js(&self.refusal.what),
            js(self.refusal.fix),
        )
    }
}

type Selector = (String, String, u32);
type SupportIndex = BTreeMap<Selector, BTreeMap<Period, BTreeSet<String>>>;

/// Only the verifier can populate this index. Invalid registered notes remain visible as typed
/// refusals and prevent an uncovered change from being classified as silent.
#[derive(Debug, Clone)]
pub struct VerifiedDisclosureRegistry {
    issuer: String,
    scopes: BTreeMap<Selector, Vec<DisclosureCitation>>,
    registered_notes: usize,
    registered_scopes: usize,
    verified_notes: usize,
    pub refusals: Vec<DisclosureRefusal>,
    digest: Digest32,
}

fn selector(scope: &RestatementScopeRow) -> Selector {
    (scope.subject.clone(), scope.line.clone(), scope.occurrence)
}

fn contains_period(scope: &RestatementScopeRow, period: Period) -> bool {
    let (Some(first), Some(last)) = (
        parse_period_code(&scope.period_from),
        parse_period_code(&scope.period_to),
    ) else {
        return false;
    };
    let (start, end) = period.months();
    first.months().0 <= start && end <= last.months().1
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn verify_note<'a>(
    note: &RestatementNoteRow,
    profile: &'a IssuerProfile,
    sources: &[LoadedSource],
) -> Result<&'a crate::sources::RegisteredSource, Refusal> {
    let invalid = |reason: &str| {
        Refusal::new(
            RefusalCode::SourceCellUnusable208,
            format!(
                "disclosure {} at {}:{}: {reason}",
                note.id, note.file_name, note.locator
            ),
        )
    };
    let mut registered = profile
        .sources
        .iter()
        .filter(|s| s.file_name == note.file_name);
    let Some(source) = registered.next() else {
        return Err(invalid("file is not registered in this run"));
    };
    if registered.next().is_some() || note.from_quarter < source.quarter {
        return Err(invalid("ambiguous source or activation before its release"));
    }
    let mut matches = sources.iter().filter(|s| s.file_name == note.file_name);
    let Some(loaded) = matches.next() else {
        return Err(Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("disclosure {} requires {}", note.id, note.file_name),
        ));
    };
    if matches.next().is_some() || loaded.quarter != source.quarter || loaded.kind != source.kind {
        return Err(invalid(
            "loaded source identity is ambiguous or differs from its registration",
        ));
    }
    if loaded.bytes != source.bytes
        || note.sha256 != source.sha256_hex
        || Digest32(loaded.workbook.sha256).to_hex() != source.sha256_hex
    {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "disclosure {} source {} differs from its registered bytes",
                note.id, note.file_name
            ),
        ));
    }
    let Some((sheet_name, cell_ref)) = note.locator.rsplit_once('!') else {
        return Err(invalid("locator must name one sheet and cell"));
    };
    let cell = parse_a1_ref(cell_ref).and_then(|(row, col)| {
        loaded
            .workbook
            .sheet(sheet_name)
            .and_then(|sheet| sheet.cell(row, col))
    });
    let Some(cell) = cell.filter(|cell| cell.formula.is_none()) else {
        return Err(invalid("quote cell is missing, hidden or a formula"));
    };
    let Some(text) = cell.value.as_text() else {
        return Err(invalid("quote cell is not text"));
    };
    if note.quoted_text.trim().is_empty() || collapse(text) != collapse(&note.quoted_text) {
        return Err(invalid(
            "complete quoted text does not match the source cell",
        ));
    }
    Ok(source)
}

impl VerifiedDisclosureRegistry {
    /// Binds notes to source bytes and every scope to an observed semantic line, occurrence, period
    /// and unique unit. The index is built once for both AISC and Mine Stats comparisons.
    #[must_use]
    pub fn verify<'a>(
        profile: &IssuerProfile,
        sources: &[LoadedSource],
        claims: impl IntoIterator<Item = &'a KeyedCell>,
    ) -> Self {
        let mut result = Self {
            issuer: profile.issuer.clone(),
            scopes: BTreeMap::new(),
            registered_notes: profile.restatement_notes.len(),
            registered_scopes: profile.restatement_scopes.len(),
            verified_notes: 0,
            refusals: Vec::new(),
            digest: Digest32([0; 32]),
        };
        let mut support: SupportIndex = profile
            .restatement_scopes
            .iter()
            .map(|scope| (selector(scope), BTreeMap::new()))
            .collect();
        for claim in claims {
            let key = &claim.key;
            if key.issuer == profile.issuer
                && let Some(periods) = support.get_mut(&(
                    key.subject.clone(),
                    key.line.label.clone(),
                    key.line.occurrence,
                ))
            {
                periods
                    .entry(key.valid_period)
                    .or_default()
                    .insert(key.unit.clone());
            }
        }
        let mut notes = BTreeMap::new();
        for note in &profile.restatement_notes {
            let at = format!("{}:{}", note.file_name, note.locator);
            match verify_note(note, profile, sources) {
                Ok(source) => {
                    notes.insert(note.id.clone(), (note, source));
                }
                Err(refusal) => result.refusals.push(DisclosureRefusal {
                    note_id: note.id.clone(),
                    at,
                    refusal,
                }),
            }
        }
        result.verified_notes = notes.len();
        for scope in &profile.restatement_scopes {
            let Some((note, source)) = notes.get(&scope.note) else {
                if !result
                    .refusals
                    .iter()
                    .any(|refusal| refusal.note_id == scope.note)
                {
                    result.refusals.push(DisclosureRefusal {
                        note_id: scope.note.clone(),
                        at: String::new(),
                        refusal: Refusal::new(
                            RefusalCode::SourceCellUnusable208,
                            format!("disclosure scope references absent note {}", scope.note),
                        ),
                    });
                }
                continue;
            };
            let units: BTreeSet<&str> = support
                .get(&selector(scope))
                .into_iter()
                .flat_map(|periods| periods.iter())
                .filter(|(period, _)| contains_period(scope, **period))
                .flat_map(|(_, units)| units.iter().map(String::as_str))
                .collect();
            let unit = if units.len() == 1 {
                units.iter().next().copied()
            } else {
                None
            };
            let Some(unit) = unit else {
                result.refusals.push(DisclosureRefusal {
                    note_id: scope.note.clone(), at: format!("{}:{}", note.file_name, note.locator),
                    refusal: Refusal::new(RefusalCode::SourceCellUnusable208,
                        format!("disclosure scope {} / {} occurrence {} in {}..{} has no observed key or has ambiguous units",
                            scope.subject, scope.line, scope.occurrence, scope.period_from, scope.period_to)),
                });
                continue;
            };
            result
                .scopes
                .entry(selector(scope))
                .or_default()
                .push(DisclosureCitation {
                    note_id: note.id.clone(),
                    at: format!("{}:{}", note.file_name, note.locator),
                    sha256: source.sha256_hex.clone(),
                    url: source.url.clone(),
                    source_release: source.quarter.clone(),
                    from_quarter: note.from_quarter.clone(),
                    quoted_text: note.quoted_text.clone(),
                    evidence: note.evidence.clone(),
                    scope: scope.clone(),
                    unit: unit.to_string(),
                });
        }
        for citations in result.scopes.values_mut() {
            citations.sort_by(|a, b| {
                (&a.note_id, &a.scope.period_from, &a.scope.period_to).cmp(&(
                    &b.note_id,
                    &b.scope.period_from,
                    &b.scope.period_to,
                ))
            });
        }
        result.refusals.sort_by(|a, b| {
            (&a.note_id, &a.at, &a.refusal.what).cmp(&(&b.note_id, &b.at, &b.refusal.what))
        });
        let mut enc = Encoder::new_frame(*b"RDSC", 1);
        enc.put_str(&result.issuer);
        let mut note_rows: Vec<_> = profile.restatement_notes.iter().collect();
        note_rows.sort_by_key(|note| &note.id);
        enc.put_u64(note_rows.len() as u64);
        for note in note_rows {
            for field in [
                &note.id,
                &note.file_name,
                &note.locator,
                &note.sha256,
                &note.quoted_text,
                &note.from_quarter,
                &note.evidence,
            ] {
                enc.put_str(field);
            }
        }
        let mut scope_rows: Vec<_> = profile.restatement_scopes.iter().collect();
        scope_rows.sort_by(|a, b| {
            (
                &a.note,
                &a.subject,
                &a.line,
                a.occurrence,
                &a.period_from,
                &a.period_to,
            )
                .cmp(&(
                    &b.note,
                    &b.subject,
                    &b.line,
                    b.occurrence,
                    &b.period_from,
                    &b.period_to,
                ))
        });
        enc.put_u64(scope_rows.len() as u64);
        for scope in scope_rows {
            for field in [
                &scope.note,
                &scope.subject,
                &scope.line,
                &scope.period_from,
                &scope.period_to,
            ] {
                enc.put_str(field);
            }
            enc.put_u32(scope.occurrence);
        }
        // The digest also binds successful semantic unit resolution and all verification refusals.
        enc.put_str(&result.body_json());
        result.digest = blake3_hash(&enc.finish());
        result
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.refusals.is_empty()
    }

    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }

    /// Returns only notes whose registered coverage includes both exact semantic keys and whose
    /// activation is no later than the target release. Region-map differences stay in the keys.
    #[must_use]
    pub fn matching(&self, from: &PublicKey, to: &PublicKey) -> Vec<DisclosureCitation> {
        if from.issuer != self.issuer
            || to.issuer != self.issuer
            || from.subject != to.subject
            || from.valid_period != to.valid_period
            || from.line != to.line
            || from.unit != to.unit
            || from.reporting_vintage.release > to.reporting_vintage.release
        {
            return Vec::new();
        }
        self.scopes
            .get(&(
                to.subject.clone(),
                to.line.label.clone(),
                to.line.occurrence,
            ))
            .into_iter()
            .flatten()
            .filter(|citation| {
                citation.from_quarter <= to.reporting_vintage.release
                    && citation.unit == to.unit
                    && contains_period(&citation.scope, to.valid_period)
            })
            .cloned()
            .collect()
    }

    fn body_json(&self) -> String {
        let scopes = self
            .scopes
            .values()
            .flatten()
            .map(DisclosureCitation::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let refusals = self
            .refusals
            .iter()
            .map(DisclosureRefusal::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "\"status\":{},\"issuer\":{},\"registered_notes\":{},\"verified_notes\":{},\
             \"registered_scopes\":{},\"verified_scopes\":[{scopes}],\"refusals\":[{refusals}]",
            js(if self.is_complete() {
                "verified"
            } else {
                "refused"
            }),
            js(&self.issuer),
            self.registered_notes,
            self.verified_notes,
            self.registered_scopes,
        )
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"fmine.disclosures.v1\",\"digest\":{}, {}}}",
            js(&self.digest.to_hex()),
            self.body_json()
        )
    }
}

// Cross-crate source and scope checks live in xtask/tests/public_disclosures.rs.
