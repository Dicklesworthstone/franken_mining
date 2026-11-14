//! DESIGN (BD-331.S8): five-state accounting of later numeric source appearances.
//! This is a projection of the existing comparison, never a second restatement classifier.

use super::{Comparison, KeyedCell, VintageState, js};

/// Compact links into the complete canonical source-Claim inventory retained by the assurance report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceReference {
    pub claim_id: String,
    pub at: String,
    pub sha256: String,
    pub release: String,
}

impl SourceReference {
    fn from_cell(cell: &KeyedCell) -> Self {
        Self {
            claim_id: cell.claim.claim_id.to_string(),
            at: cell.locator.clone(),
            sha256: cell.key.reporting_vintage.file_sha256.clone(),
            release: cell.key.reporting_vintage.release.clone(),
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"claim_id\":{},\"at\":{},\"sha256\":{},\"release\":{}}}",
            js(&self.claim_id),
            js(&self.at),
            js(&self.sha256),
            js(&self.release),
        )
    }
}

/// One later appearance against its earliest registered release. Exclusions have no ledger state.
#[derive(Debug, Clone)]
pub struct CrossVintageEntry {
    pub subject: String,
    pub period: String,
    pub metric: String,
    pub state: Option<&'static str>,
    pub explanation: Option<&'static str>,
    pub exclusion: Option<&'static str>,
    pub comparison: Comparison,
    /// Conflicts keep every assertion; this set is not asserted to be inclusion-minimal.
    pub first: Vec<SourceReference>,
    pub current: Vec<SourceReference>,
}

impl CrossVintageEntry {
    #[must_use]
    pub fn missing_fact(&self) -> Option<&'static str> {
        (self.state == Some("iv")).then_some(
            "an applicable verified restatement note or a complete registered disclosure review",
        )
    }

    fn to_json(&self) -> String {
        let sources = |values: &[SourceReference]| {
            values
                .iter()
                .map(SourceReference::to_json)
                .collect::<Vec<_>>()
                .join(",")
        };
        let optional = |value: Option<&str>| value.map_or_else(|| "null".into(), js);
        let disclosed = match self.explanation {
            Some("stated") => "true",
            Some("reconstructed") => "false",
            _ => "null",
        };
        format!(
            "{{\"subject\":{},\"period\":{},\"metric\":{},\"state\":{},\"explanation\":{},\"layer\":{},\"disclosed\":{disclosed},\"missing_fact\":{},\"exclusion\":{},\"comparison\":{},\"first\":[{}],\"current\":[{}]}}",
            js(&self.subject),
            js(&self.period),
            js(&self.metric),
            optional(self.state),
            optional(self.explanation),
            optional((self.state == Some("ii")).then_some("restatement")),
            optional(self.missing_fact()),
            optional(self.exclusion),
            self.comparison.to_json(),
            sources(&self.first),
            sources(&self.current),
        )
    }
}

/// Separate denominator from all within-vintage arithmetic identities. Every later appearance is
/// either one numeric instance or one named exclusion, including unchanged nonnumeric placeholders.
#[derive(Debug, Clone, Default)]
pub struct CrossVintageLedger {
    pub entries: Vec<CrossVintageEntry>,
}

impl CrossVintageLedger {
    pub(super) fn record(
        &mut self,
        first: &[&KeyedCell],
        current: &[&KeyedCell],
        comparison: &Comparison,
    ) {
        let numeric = !first.is_empty() && !current.is_empty() && first.iter().chain(current).all(|c| {
            matches!(&c.claim.value, super::PublicValue::Number { displayed: Ok(value), stored, .. }
                if super::displayed_number(value, stored, &c.key.unit).is_ok())
        });
        let (state, explanation, exclusion) = if comparison.state == VintageState::NotComparable {
            (
                None,
                None,
                Some(comparison.reason.unwrap_or("not_comparable")),
            )
        } else if !numeric {
            (None, None, Some("non_numeric_comparison"))
        } else {
            match comparison.state {
                VintageState::Unchanged => (Some("i"), None, None),
                VintageState::RestatedDisclosed => (Some("ii"), Some("stated"), None),
                VintageState::RestatedSilent => (Some("ii"), Some("reconstructed"), None),
                VintageState::Conflict => (Some("iii"), None, None),
                // Absence of verified disclosure coverage does not establish a silent restatement.
                VintageState::ChangedWithoutRegisteredExplanation => (Some("iv"), None, None),
                VintageState::FirstRegistered | VintageState::SingleVintage => {
                    (None, None, Some("not_a_later_appearance"))
                }
                VintageState::NotComparable => (None, None, Some("not_comparable")),
            }
        };
        let key = current.first().or_else(|| first.first()).map(|c| &c.key);
        self.entries.push(CrossVintageEntry {
            subject: key.map_or_else(String::new, |k| k.subject.clone()),
            period: key.map_or_else(String::new, |k| k.valid_period.code()),
            metric: key.map_or_else(String::new, |k| k.line.label.clone()),
            state,
            explanation,
            exclusion,
            comparison: comparison.clone(),
            first: first
                .iter()
                .map(|c| SourceReference::from_cell(c))
                .collect(),
            current: current
                .iter()
                .map(|c| SourceReference::from_cell(c))
                .collect(),
        });
    }

    #[must_use]
    pub fn count(&self, state: &str) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.state == Some(state))
            .count()
    }

    #[must_use]
    pub fn instances(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.state.is_some())
            .count()
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let counts = ["i", "ii", "iii", "iv", "v"]
            .map(|state| format!("{}:{}", js(state), self.count(state)))
            .join(",");
        let explanation_count = |basis| {
            self.entries
                .iter()
                .filter(|entry| entry.explanation == Some(basis))
                .count()
        };
        let entries = self
            .entries
            .iter()
            .map(CrossVintageEntry::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.cross-vintage-ledger.v1\",\"family\":\"cross_vintage\",\"basis\":\"a later registered assertion against its earliest registered appearance\",\"denominator\":\"numeric later appearances only; separate from within-vintage totals\",\"interpretation\":\"i: unchanged numeric assertion; ii: stated or reconstructed restatement; iii: source assertion conflict; iv: numeric change with disclosure coverage unresolved. No true vintage, arithmetic certificate or minimal conflict set is inferred. First and single appearances remain in the source-value report.\",\"comparisons\":{},\"instances\":{},\"excluded\":{},\"by_state\":{{{counts}}},\"ii_stated\":{},\"ii_reconstructed\":{},\"entries\":[{entries}]}}",
            self.entries.len(),
            self.instances(),
            self.entries.len() - self.instances(),
            explanation_count("stated"),
            explanation_count("reconstructed"),
        )
    }
}
