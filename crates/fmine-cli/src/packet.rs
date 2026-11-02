//! DESIGN (BD-349): review packets built from the assurance ledger, with its source values and refusals.
//! A packet requests an evidence check, never a financial decision. These immutable drafts carry no human
//! check or external-use permission. Rendering does not change a ledger state or its established root.

mod mine_consistency;
mod transcription;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_core::codec::Encoder;
use fmine_core::hash::Digest32;
use fmine_core::id::{ObjectId, PrefixKind};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::xlsx::{CellValue, parse_a1_ref};

use fmine_public::aisc::{
    self, ExplanationBasis, IdentityInstance, IdentityState, OperandRole, OperandValue,
};
use fmine_public::citation::{self, Cited};
use fmine_public::consistency::{
    AiscConsistency, Appearance, IdentityHistory, TransitionKind, aisc_consistency,
};
use fmine_public::key::consistency::{
    Comparison, VintageItem, VintageReport, VintageScope, VintageState,
    compare_vintages_with_disclosures,
};
use fmine_public::key::{KeyedCell, PublicValue, raw_token};
use fmine_public::mine_ledger::{self, Component, Instance, State};
use fmine_public::sources::SourceKind;
use fmine_public::{Lane, PublicAssuranceReport};

pub const NO_AFFILIATION: &str =
    "From public filings; no affiliation with or endorsement by the issuer is implied.";
pub const SYNTHETIC_NO_AFFILIATION: &str = "From invented synthetic workbooks; no affiliation with or endorsement by any issuer is implied.";
pub const REVIEW_BOUNDARY: &str = "Internal review draft. Independent human checking and counsel's release sequencing remain outstanding; external use is not authorized.";
const RULES_VERSION: &str = "challenge-packets/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PacketKind {
    FootingDiscrepancy,
    IdentityDiscrepancy,
    JointDiscrepancy,
    Undecidable,
    ReconstructionUnstated,
    ExplanationStated,
    InputRefused,
    VintageChange,
    VintageConflict,
    IdentityTransition,
    TranscriptionConflict,
    MineStatsIdentityTransition,
}

impl PacketKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FootingDiscrepancy => "footing_discrepancy",
            Self::IdentityDiscrepancy => "identity_discrepancy",
            Self::JointDiscrepancy => "joint_discrepancy",
            Self::Undecidable => "undecidable",
            Self::ReconstructionUnstated => "reconstruction_unstated",
            Self::ExplanationStated => "explanation_stated",
            Self::InputRefused => "input_refused",
            Self::VintageChange => "vintage_change",
            Self::VintageConflict => "vintage_conflict",
            Self::IdentityTransition => "identity_transition",
            Self::TranscriptionConflict => "transcription_conflict",
            Self::MineStatsIdentityTransition => "mine_stats_identity_transition",
        }
    }

    const fn statement(self) -> &'static str {
        match self {
            Self::FootingDiscrepancy | Self::IdentityDiscrepancy | Self::JointDiscrepancy => {
                "Unexplained difference between cited cells under the declared construction."
            }
            Self::Undecidable => {
                "The cited public inputs do not decide this identity; the missing fact is requested below."
            }
            Self::ReconstructionUnstated => {
                "The arithmetic closes under a registered reconstruction; an issuer statement of the rule is requested."
            }
            Self::ExplanationStated => {
                "The arithmetic closes under the cited issuer statement. Retain that statement when quoting the finding."
            }
            Self::InputRefused => {
                "The identity was refused. The missing or unusable input must be resolved before a numerical conclusion is drawn."
            }
            Self::VintageChange => {
                "Unexplained difference between cited cells in registered releases. Neither assertion is selected as true."
            }
            Self::VintageConflict => {
                "The registered comparison retains conflicting source assertions; their value and ownership basis require a cited reconciliation."
            }
            Self::IdentityTransition => {
                "The AISC identity has an exceptional transition between registered appearances. Original states, operand comparisons and certificates are retained; no true assertion or issuer admission is inferred."
            }
            Self::TranscriptionConflict => {
                "The registered PDF transcriptions require reconciliation of their printed assertions or quantity context. No assertion is selected as true."
            }
            Self::MineStatsIdentityTransition => {
                "The Mine Statistics identity has an exceptional transition between registered appearances. Original states, source operands and declared rounding and ownership assumptions are retained; no true assertion or issuer admission is inferred."
            }
        }
    }

    const fn owner(self) -> &'static str {
        match self {
            Self::InputRefused
            | Self::IdentityTransition
            | Self::TranscriptionConflict
            | Self::MineStatsIdentityTransition => "project:source-curator",
            Self::ReconstructionUnstated | Self::ExplanationStated => "issuer:investor-relations",
            _ => "issuer:corporate-controller",
        }
    }

    fn blocked_uses(self) -> &'static [&'static str] {
        match self {
            Self::ReconstructionUnstated | Self::ExplanationStated => {
                &["external_quotation_without_packet"]
            }
            _ => &[
                "input_to_rollup",
                "waterfall_bar",
                "track_record_outcome",
                "external_quotation_without_packet",
            ],
        }
    }
}

/// A requested document is not represented as a verified source or an attestation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ClosingDocument {
    document: String,
    locator: Option<String>,
    would_show: String,
}

impl ClosingDocument {
    fn to_json(&self) -> String {
        format!(
            "{{\"document\":{},\"locator\":{},\"status\":\"requested\",\"would_show\":{}}}",
            js(&self.document),
            optional(&self.locator),
            js(&self.would_show)
        )
    }
}

#[derive(Debug, Clone)]
struct PacketOperand {
    at: String,
    sha256: String,
    roles: BTreeSet<String>,
    value: OperandValue,
    stored_text: Option<String>,
    displayed_text: Option<String>,
    display_refusal: Option<String>,
    label: Option<String>,
    claim: Option<KeyedCell>,
}

impl PacketOperand {
    fn to_json(&self) -> String {
        let cited = Cited::Extracted {
            v: self.value.clone(),
            at: self.at.clone(),
            sha256: self.sha256.clone(),
        }
        .to_json();
        format!(
            "{{\"roles\":{},\"cited\":{cited},\"stored_text\":{},\"displayed_text\":{},\
             \"display_refusal\":{},\"label_as_printed\":{},\"claim\":{}}}",
            strings(self.roles.iter().map(String::as_str)),
            optional(&self.stored_text),
            optional(&self.displayed_text),
            optional(&self.display_refusal),
            optional(&self.label),
            self.claim
                .as_ref()
                .map_or_else(|| "null".to_string(), KeyedCell::to_json)
        )
    }

    fn shown(&self) -> String {
        if let Some(reason) = &self.display_refusal {
            return format!(
                "display refused: {reason}; stored {}",
                self.stored_text.as_deref().unwrap_or("unavailable")
            );
        }
        if let Some(shown) = &self.displayed_text {
            return shown.clone();
        }
        match &self.value {
            OperandValue::Int(v) => v.to_string(),
            OperandValue::Token(t) => format!("token {t:?}"),
            OperandValue::Unrounded(t) => format!("stored {t}; no whole displayed quantity"),
            OperandValue::Blank => "blank; no quantity".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
struct Finding {
    period: String,
    at: String,
    state: String,
    /// Existing cited ledger output, or the same typed Mine Stats result with resolved file locators.
    evidence_json: String,
}

impl Finding {
    fn to_json(&self) -> String {
        self.to_json_with_key("ledger_evidence")
    }

    fn to_json_with_key(&self, evidence_key: &str) -> String {
        format!(
            "{{\"period\":{},\"at\":{},\"state\":{},\"{evidence_key}\":{}}}",
            js(&self.period),
            js(&self.at),
            js(&self.state),
            self.evidence_json
        )
    }
}

/// Immutable, content-addressed internal review material. There is deliberately no authorization field
/// or setter: a program cannot manufacture the independent human check required by BD-349.S2.
///
/// ```compile_fail
/// fn authorize(packet: &mut fmine_cli::packet::ChallengePacket) {
///     packet.external_use_allowed = true;
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ChallengePacket {
    id: ObjectId,
    issuer: String,
    lane: Lane,
    source_root: Digest32,
    source_anchor: ObjectId,
    kind: PacketKind,
    subject: String,
    family: String,
    construction: &'static str,
    findings: Vec<Finding>,
    operands: BTreeMap<String, PacketOperand>,
    transcription_evidence: Option<transcription::Evidence>,
    closing_documents: BTreeSet<ClosingDocument>,
}

impl ChallengePacket {
    #[must_use]
    pub const fn id(&self) -> ObjectId {
        self.id
    }

    #[must_use]
    pub const fn kind(&self) -> PacketKind {
        self.kind
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    #[must_use]
    pub fn finding_count(&self) -> usize {
        self.findings.len()
    }

    #[must_use]
    pub const fn external_use_allowed(&self) -> bool {
        false
    }

    fn affiliation_line(&self) -> &'static str {
        match self.lane {
            Lane::Public => NO_AFFILIATION,
            Lane::Synthetic if self.kind == PacketKind::TranscriptionConflict => {
                "From invented synthetic documents and transcriptions; no affiliation with or endorsement by any issuer is implied."
            }
            Lane::Synthetic => SYNTHETIC_NO_AFFILIATION,
        }
    }

    fn body_json(&self) -> String {
        format!(
            "{{\"schema\":\"fmine.challenge_packet.v1\",\"rules_version\":{},\"issuer\":{},\
             \"lane\":{},\"source_root\":{},\"source_anchor\":{},\"kind\":{},\"subject\":{},\
             \"family\":{},\"construction\":{},\"statement\":{},\"owner_role\":{},\
             \"blocked_uses\":{},\"closing_documents\":[{}],\"status\":\"open\",\
             \"built_by_role\":\"project:packet-builder\",\"human_check\":null,\"external_use_allowed\":false,\
             \"review_boundary\":{},\"no_affiliation\":{},\"no_claim\":{},\
             \"flags\":{{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}},\
             \"findings\":[{}],\"operands\":[{}]{}}}",
            js(RULES_VERSION),
            js(&self.issuer),
            js(self.lane.as_str()),
            js(&self.source_root.to_hex()),
            js(&self.source_anchor.to_string()),
            js(self.kind.as_str()),
            js(&self.subject),
            js(&self.family),
            js(self.construction),
            js(self.kind.statement()),
            js(self.kind.owner()),
            strings(self.kind.blocked_uses().iter().copied()),
            self.closing_documents
                .iter()
                .map(ClosingDocument::to_json)
                .collect::<Vec<_>>()
                .join(","),
            js(REVIEW_BOUNDARY),
            js(self.affiliation_line()),
            js(fmine_public::NO_CLAIM),
            self.findings
                .iter()
                .map(|finding| {
                    if self.kind == PacketKind::TranscriptionConflict {
                        finding.to_json_with_key("transcription_evidence")
                    } else {
                        finding.to_json()
                    }
                })
                .collect::<Vec<_>>()
                .join(","),
            self.operands
                .values()
                .map(PacketOperand::to_json)
                .collect::<Vec<_>>()
                .join(","),
            self.transcription_evidence
                .as_ref()
                .map_or_else(String::new, |evidence| format!(
                    ",\"transcription_evidence\":{}",
                    evidence.to_json()
                ))
        )
    }

    fn content_id(&self) -> ObjectId {
        let mut frame = Encoder::new_frame(*b"CPKT", 1);
        frame.put_str(&self.body_json());
        ObjectId::from_canonical_bytes(PrefixKind::Discrepancy, &frame.finish())
    }

    /// Validate the complete evidence packet, including its content address. Source text is preserved
    /// verbatim; the wording guard applies to authored requests, not to words quoted from a source.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.findings.is_empty()
            || (self.operands.is_empty() && self.transcription_evidence.is_none())
            || self.closing_documents.is_empty()
        {
            return Err(unusable(
                "packet needs findings, cited operands and a closing document request",
            ));
        }
        for closing in &self.closing_documents {
            if closing.document.trim().is_empty()
                || closing.would_show.trim().is_empty()
                || prohibited_word(&closing.would_show)
            {
                return Err(unusable(
                    "packet closing request is empty or uses unsupported decision wording",
                ));
            }
        }
        for operand in self.operands.values() {
            if operand.at.is_empty()
                || operand.sha256.len() != 64
                || !operand.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(unusable(
                    "packet operand has no complete source locator and digest",
                ));
            }
        }
        if let Some(evidence) = &self.transcription_evidence {
            evidence.validate()?;
        }
        if self.content_id() != self.id {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                "packet body differs from its content address",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},{}",
            js(&self.id.to_string()),
            self.body_json().trim_start_matches('{')
        )
    }

    /// A standalone review document; source tokens and labels are escaped as text, not interpreted as markup.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "# Evidence challenge: {}\n\n{}\n\n**{}**\n\n{}\n\n\
             - Packet: {}\n- Lane: {}\n- Issuer: {}\n- Finding: {}\n- Family: {}\n\
             - Construction: {}\n- Owner role: {}\n- Source root: {}\n- Source anchor: {}\n\n\
             ## Closing evidence requested\n\n",
            md(&self.subject),
            md(self.kind.statement()),
            REVIEW_BOUNDARY,
            self.affiliation_line(),
            self.id,
            self.lane.as_str(),
            md(&self.issuer),
            self.kind.as_str(),
            md(&self.family),
            md(self.construction),
            self.kind.owner(),
            self.source_root.to_hex(),
            self.source_anchor
        );
        for request in &self.closing_documents {
            let _ = writeln!(
                out,
                "- {}{}: {}",
                md(&request.document),
                request
                    .locator
                    .as_ref()
                    .map_or_else(String::new, |at| format!(" ({})", md(at))),
                md(&request.would_show)
            );
        }
        out.push_str("\nRequested evidence has not been supplied or independently checked by this packet.\n\n## Uses blocked pending resolution\n\n");
        for use_name in self.kind.blocked_uses() {
            let _ = writeln!(out, "- {}", md(use_name));
        }
        if self.kind == PacketKind::TranscriptionConflict {
            out.push_str("\n## Findings retained\n\n| Period | Transcription finding | Cited anchor |\n| --- | --- | --- |\n");
        } else if self.kind == PacketKind::IdentityTransition {
            out.push_str("\n## Findings retained\n\n| Period | Recorded transition | Cited anchor |\n| --- | --- | --- |\n");
        } else {
            out.push_str("\n## Findings retained\n\n| Period | Ledger state | Cited anchor |\n| --- | --- | --- |\n");
        }
        for finding in &self.findings {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                md(&finding.period),
                md(&finding.state),
                md(&finding.at)
            );
        }
        if let Some(evidence) = &self.transcription_evidence {
            out.push_str(&evidence.to_markdown());
        } else {
            out.push_str("\nThe ledger states are retained. A joint conflict does not change an identity that was consistent alone.\n\n## Source operands\n\n| Locator | Value or token as printed | Stored text | Role | Claim | SHA-256 |\n| --- | --- | --- | --- | --- | --- |\n");
        }
        for operand in self.operands.values() {
            let claim = operand.claim.as_ref().map_or_else(
                || "source context; no keyed Claim".to_string(),
                |c| c.claim.claim_id.to_string(),
            );
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                md(&operand.at),
                md(&operand.shown()),
                md(operand.stored_text.as_deref().unwrap_or("")),
                md(&operand.roles.iter().cloned().collect::<Vec<_>>().join(", ")),
                md(&claim),
                operand.sha256
            );
        }
        if self.kind == PacketKind::TranscriptionConflict {
            out.push_str("\n## Transcription comparisons\n\nA numeric conflict means that no supported rounding assignment reconciles the agreed point assertions under their registered quantity key. Incompatible unit, basis or period labels instead leave arithmetic comparability unestablished. Transcription and quantity-key corrections remain alternatives; no issuer admission or disclosure status is inferred. Reader agreement is not an independent human review of this packet.\n\n");
        } else if self.kind == PacketKind::IdentityTransition {
            out.push_str("\n## AISC identity transitions\n\nThe earliest registered, immediately previous and current identity records retain their source Claims, original states and arithmetic certificates. A corrected or broken classification describes arithmetic under the declared construction, not an issuer admission. Disclosure classifications belong to the separate source-value comparisons; this transition does not invent one. The earliest registered release need not be the issuer's first publication.\n\n");
        } else if self.kind == PacketKind::MineStatsIdentityTransition {
            out.push_str("\n## Mine Statistics identity transitions\n\nThe earliest registered, immediately previous and current identity records retain their source Claims, original states, ownership assumptions and printed precision. A corrected or broken classification describes arithmetic under the declared construction, not an issuer admission. Source-value disclosure classifications remain separate. The earliest registered release need not be the issuer's first publication.\n\n");
        } else if matches!(
            self.kind,
            PacketKind::VintageChange | PacketKind::VintageConflict
        ) {
            out.push_str("\n## Vintage comparisons and registered disclosures\n\nEach finding retains the earliest registered, immediately previous and current assertions, with both comparisons. The earliest registered release need not be the issuer's first publication. Silence means no applicable verified note in the registered documents; it does not establish absence of disclosure elsewhere. Registered notes describe their exact coverage. No assertion is selected as true, and rounding-only status is unassessed.\n\n");
        } else {
            out.push_str("\n## Arithmetic and retained evidence\n\nThe cited records below retain each construction, operand role, missing input and available certificate. A sign-flip witness is a possible explanation, not a determination of cause. Correction ranges are alternatives, not replacement values.\n\n");
        }
        for finding in &self.findings {
            let _ = writeln!(
                out,
                "### {} — {}\n\n```json\n{}\n```\n",
                md(&finding.period),
                md(&finding.at),
                // JSON string escapes do not escape Markdown fences. Escaping backticks as JSON Unicode
                // prevents a quoted source from terminating the evidence block.
                finding.evidence_json.replace('`', "\\u0060")
            );
        }
        let _ = writeln!(out, "{}\n\n{}", fmine_public::NO_CLAIM, REVIEW_BOUNDARY);
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    origin: &'static str,
    subject: String,
    family: String,
    kind: PacketKind,
    discriminator: String,
}

fn optional(value: &Option<String>) -> String {
    value.as_deref().map_or_else(|| "null".to_string(), js)
}

fn js(value: &str) -> String {
    format!("\"{}\"", crate::escape_json(value))
}

fn input_refusal(what: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, what)
}

fn strings<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    format!(
        "[{}]",
        values.into_iter().map(js).collect::<Vec<_>>().join(",")
    )
}

fn unusable(what: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::SourceCellUnusable208, what)
}

fn prohibited_word(text: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric()).any(|word| {
        [
            "npv",
            "cvar",
            "likelihood",
            "probability",
            "error",
            "misstatement",
        ]
        .iter()
        .any(|bad| word.eq_ignore_ascii_case(bad))
    })
}

fn md(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\n' | '\r' => out.push(' '),
            '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '|' | '#' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Resolve the ledger's full-file or Mine Stats release locator without selecting an arbitrary source.
fn read_operand(
    report: &PublicAssuranceReport,
    at: &str,
    role: &str,
    expected: Option<&OperandValue>,
) -> Result<PacketOperand, Refusal> {
    let (file, rest) = at
        .split_once(':')
        .ok_or_else(|| unusable(format!("invalid packet source locator {at}")))?;
    let (sheet_name, cell_ref) = rest
        .rsplit_once('!')
        .ok_or_else(|| unusable(format!("invalid packet cell locator {at}")))?;
    let sources: Vec<_> = report
        .sources
        .iter()
        .filter(|s| s.file_name == file || (s.kind == SourceKind::MineStats && s.quarter == file))
        .collect();
    if sources.len() != 1 {
        return Err(input_refusal(format!(
            "packet operand {at} needs one verified source; found {}",
            sources.len()
        )));
    }
    let source = sources[0];
    let sheet = source
        .workbook
        .sheet(sheet_name)
        .ok_or_else(|| input_refusal(format!("packet operand {at} has no visible source sheet")))?;
    let (row, col) =
        parse_a1_ref(cell_ref).ok_or_else(|| unusable(format!("invalid packet cell {at}")))?;
    let read = aisc::operand_at(&source.file_name, sheet, row, col, OperandRole::LabelCheck);
    if expected.is_some_and(|v| *v != read.value) {
        return Err(unusable(format!(
            "packet operand {at} differs from the loaded source cell"
        )));
    }
    let at = citation::locator(&read);
    let sha256 = Digest32(source.workbook.sha256).to_hex();
    let claim = report.public_claims.cells.get(&at).cloned();
    if claim
        .as_ref()
        .is_some_and(|c| c.key.reporting_vintage.file_sha256 != sha256)
    {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!("packet Claim and source digest differ at {at}"),
        ));
    }
    let cell = sheet.cell(row, col);
    let shown = cell.and_then(|c| c.displayed());
    let label = claim
        .as_ref()
        .and_then(|c| c.label_as_printed.clone())
        .or_else(|| {
            sheet.cell(row, 1).and_then(|c| match &c.value {
                CellValue::Text(s) => Some(s.clone()),
                _ => None,
            })
        });
    Ok(PacketOperand {
        at,
        sha256,
        roles: [role.to_string()].into_iter().collect(),
        value: read.value,
        stored_text: cell.and_then(|c| c.number_text.clone()),
        displayed_text: shown
            .as_ref()
            .and_then(|s| s.as_ref().ok().map(|d| d.text())),
        display_refusal: shown.and_then(|s| s.err().map(|e| e.code.to_string())),
        label,
        claim,
    })
}

fn packet_for<'a>(
    groups: &'a mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
    key: GroupKey,
    construction: &'static str,
) -> &'a mut ChallengePacket {
    groups
        .entry(key.clone())
        .or_insert_with(|| ChallengePacket {
            id: ObjectId::from_raw(PrefixKind::Discrepancy, [0; 16]),
            issuer: report.issuer.clone(),
            lane: report.lane,
            source_root: report.ledger_anchor.state_root,
            source_anchor: report.ledger_anchor.anchor_id(),
            kind: key.kind,
            subject: key.subject,
            family: key.family,
            construction,
            findings: Vec::new(),
            operands: BTreeMap::new(),
            transcription_evidence: None,
            closing_documents: BTreeSet::new(),
        })
}

fn add_operand(packet: &mut ChallengePacket, operand: PacketOperand) -> Result<(), Refusal> {
    if let Some(old) = packet.operands.get_mut(&operand.at) {
        if old.value != operand.value || old.sha256 != operand.sha256 {
            return Err(unusable(format!(
                "packet has conflicting source assertions at {}",
                operand.at
            )));
        }
        old.roles.extend(operand.roles);
    } else {
        packet.operands.insert(operand.at.clone(), operand);
    }
    Ok(())
}

/// Produce complete internal-review drafts from all visible AISC `ii`/`iii`/`iv`/`v` instances, Mine Stats
/// `iii`/`iv`/`v` instances, jointly infeasible components, exceptional identity transitions and unresolved
/// cross-vintage changes/conflicts. Persisted and single-vintage identity histories add no packets.
/// Accepted PDF transcription conflicts are separately bound to their verified document metadata and
/// accepted-row digest; they are report-only and never enter the source ledger root.
/// Disclosed-only changes and non-comparable scopes remain informational in the assurance report. Hidden and
/// not-published instances remain outside this population. Grouping retains every current-release finding,
/// including both first/prior comparisons, without combining different periods, units or ownership bases.
/// Missing registered source bytes refuse the build; a missing input already recorded by the ledger stays `v`.
pub fn packets_from_report(
    report: &PublicAssuranceReport,
) -> Result<Vec<ChallengePacket>, Refusal> {
    let mut groups = BTreeMap::new();
    for instance in &report.instances {
        add_aisc(&mut groups, report, instance)?;
    }
    for instance in &report.mine_ledger.instances {
        if matches!(
            instance.state,
            State::Discrepancy | State::Unresolved(_) | State::Refused { .. }
        ) {
            add_mine(&mut groups, report, instance)?;
        }
    }
    for component in &report.mine_ledger.infeasible {
        add_component(&mut groups, report, component)?;
    }
    validate_identity_transitions(report)?;
    for history in &report.issuer_consistency.histories {
        add_identity_transitions(&mut groups, report, history)?;
    }
    mine_consistency::add_packets(&mut groups, report)?;
    for (vintages, scope, kind) in [
        (
            &report.mine_stats_vintages,
            VintageScope::MineStats,
            SourceKind::MineStats,
        ),
        (
            &report.aisc_vintages,
            VintageScope::Aisc,
            SourceKind::AiscReconciliation,
        ),
    ] {
        validate_vintage_report(report, vintages, scope, kind)?;
        for item in &vintages.items {
            add_vintage(&mut groups, report, vintages, item)?;
        }
    }
    transcription::add_packets(&mut groups, report)?;
    let mut packets = Vec::with_capacity(groups.len());
    for mut packet in groups.into_values() {
        packet.findings.sort_by_cached_key(Finding::to_json);
        packet.id = packet.content_id();
        packet.validate()?;
        packets.push(packet);
    }
    packets.sort_by_key(ChallengePacket::id);
    Ok(packets)
}

/// Source-bound Claims alone do not authenticate a comparison verdict. Recompute each family once from
/// the canonical projection, then compare the complete result, including population and chronology.
fn validate_vintage_report(
    report: &PublicAssuranceReport,
    supplied: &VintageReport,
    scope: VintageScope,
    kind: SourceKind,
) -> Result<(), Refusal> {
    let all_files: BTreeSet<_> = report
        .sources
        .iter()
        .map(|s| s.file_name.as_str())
        .collect();
    if let Some(claim) = report.public_claims.cells.values().find(|c| {
        c.locator
            .split_once(':')
            .is_none_or(|(file, _)| !all_files.contains(file))
    }) {
        return Err(input_refusal(format!(
            "canonical vintage Claim needs its source at {}",
            claim.locator
        )));
    }
    let files: BTreeSet<_> = report
        .sources
        .iter()
        .filter(|s| s.kind == kind)
        .map(|s| s.file_name.as_str())
        .collect();
    let claims: Vec<_> = report
        .public_claims
        .cells
        .values()
        .filter(|c| {
            c.locator
                .split_once(':')
                .is_some_and(|(file, _)| files.contains(file))
        })
        .cloned()
        .collect();
    let checked = compare_vintages_with_disclosures(&claims, scope, Some(&report.disclosures));
    let canonical = |mut r: VintageReport| {
        for item in &mut r.items {
            for cells in [&mut item.first, &mut item.previous, &mut item.current] {
                cells.sort_by_cached_key(KeyedCell::to_json);
            }
            for c in
                std::iter::once(&mut item.against_first).chain(item.against_previous.iter_mut())
            {
                c.disclosures.sort_by_cached_key(|d| d.to_json());
            }
        }
        r.items.sort_by_cached_key(|i| {
            format!(
                "{:?}:{:?}:{:?}:{}:{}",
                i.first.iter().map(KeyedCell::to_json).collect::<Vec<_>>(),
                i.previous
                    .iter()
                    .map(KeyedCell::to_json)
                    .collect::<Vec<_>>(),
                i.current.iter().map(KeyedCell::to_json).collect::<Vec<_>>(),
                comparison_json(&i.against_first),
                i.against_previous
                    .as_ref()
                    .map_or_else(|| "null".into(), comparison_json)
            )
        });
        r.to_json()
    };
    if canonical(supplied.clone()) != canonical(checked) {
        return Err(unusable(format!(
            "{} vintage findings differ from the comparison of the canonical source Claims",
            scope.as_str()
        )));
    }
    Ok(())
}

/// Recompute the supplied transitions from the canonical keys and original ledger. Reordering findings is
/// immaterial; changing a state, transition kind or predecessor index without its evidence refuses.
fn validate_identity_transitions(report: &PublicAssuranceReport) -> Result<(), Refusal> {
    let canonical = |mut value: AiscConsistency| {
        value
            .histories
            .sort_by(|a, b| (&a.series, a.family).cmp(&(&b.series, b.family)));
        for history in &mut value.histories {
            history
                .transitions
                .sort_by_key(|t| (t.from, t.to, t.kind.as_str(), t.operands_changed));
            history.ambiguous_vintages.sort();
        }
        value
            .unkeyed
            .sort_by(|a, b| (&a.at, a.family, &a.reason).cmp(&(&b.at, b.family, &b.reason)));
        value
    };
    let checked = aisc_consistency(&report.public_claims, &report.instances);
    if canonical(report.issuer_consistency.clone()) != canonical(checked) {
        return Err(unusable(
            "AISC identity transitions differ from the canonical source Claims and original identity ledger",
        ));
    }
    Ok(())
}

fn transition_appearance(
    packet: &mut ChallengePacket,
    report: &PublicAssuranceReport,
    history: &IdentityHistory,
    appearance: &Appearance,
    role: &str,
) -> Result<String, Refusal> {
    let claim = report
        .public_claims
        .cells
        .get(&appearance.at)
        .ok_or_else(|| {
            unusable(format!(
                "AISC identity appearance has no source Claim at {}",
                appearance.at
            ))
        })?;
    if claim.key.reporting_vintage.file_sha256 != appearance.sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "AISC identity appearance has a different source digest at {}",
                appearance.at
            ),
        ));
    }
    if claim.key.reporting_vintage.release != appearance.release
        || claim.key.series_key() != history.series
    {
        return Err(unusable(format!(
            "AISC identity appearance has a different semantic key at {}",
            appearance.at
        )));
    }
    let mut matches = report.instances.iter().filter(|i| {
        i.family == history.family && format!("{}:{}!{}", i.file, i.sheet, i.cell) == appearance.at
    });
    let instance = matches.next().ok_or_else(|| {
        unusable(format!(
            "AISC appearance has no original identity at {}",
            appearance.at
        ))
    })?;
    if matches.any(|other| other != instance) || instance.state.code() != appearance.state {
        return Err(unusable(format!(
            "AISC appearance has ambiguous or different original states at {}",
            appearance.at
        )));
    }
    add_operand(packet, vintage_operand(report, claim, role)?)?;
    let mut instance = instance.clone();
    instance
        .operands
        .sort_by_key(|o| (citation::locator(o), o.role.as_str()));
    if let IdentityState::IssuerDiscrepancy(certificate) = &mut instance.state {
        certificate.line_cells.sort();
    }
    for operand in &instance.operands {
        let role = format!("{role}:{}", operand.role.as_str());
        let read = read_operand(
            report,
            &citation::locator(operand),
            &role,
            Some(&operand.value),
        )?;
        let checked = if let Some(claim) = &read.claim {
            vintage_operand(report, claim, &role)?
        } else {
            read
        };
        add_operand(packet, checked)?;
    }
    Ok(format!(
        "{{\"release\":{},\"claim\":{},\"identity\":{}}}",
        js(&appearance.release),
        claim.to_json(),
        report.exception_json(&instance, None)
    ))
}

fn add_identity_transitions(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
    history: &IdentityHistory,
) -> Result<(), Refusal> {
    for transition in history.transitions.iter().filter(|t| {
        !matches!(
            t.kind,
            TransitionKind::Persisted | TransitionKind::SingleVintage
        )
    }) {
        let appearance = |index| {
            history
                .sequence
                .get(index)
                .ok_or_else(|| unusable("AISC identity transition references an absent appearance"))
        };
        let first = appearance(0)?;
        let previous = appearance(transition.from)?;
        let current = appearance(transition.to)?;
        let period = history.series.valid_period.code();
        let packet = packet_for(
            groups,
            report,
            GroupKey {
                origin: "aisc_identity_transition",
                subject: history.series.subject.clone(),
                family: format!("aisc_identity:{}", history.family.as_str()),
                kind: PacketKind::IdentityTransition,
                discriminator: first.at.clone(),
            },
            "existing adjacent-appearance identity comparison; original constructions and states retained",
        );
        let mut earliest = history
            .sequence
            .iter()
            .filter(|a| a.release == first.release)
            .map(|a| transition_appearance(packet, report, history, a, "earliest_registered"))
            .collect::<Result<Vec<_>, _>>()?;
        earliest.sort();
        let first_json = if history.ambiguous_vintages.contains(&first.release) {
            "null"
        } else {
            earliest.first().map_or("null", String::as_str)
        };
        let previous_json =
            transition_appearance(packet, report, history, previous, "immediately_previous")?;
        let current_json =
            transition_appearance(packet, report, history, current, "current_release")?;
        let ambiguous: BTreeSet<_> = history
            .ambiguous_vintages
            .iter()
            .map(String::as_str)
            .collect();
        let request = if ambiguous.contains(previous.release.as_str())
            || ambiguous.contains(current.release.as_str())
        {
            "resolve the coexisting assertions in the ambiguous registered releases without choosing one arbitrarily"
        } else {
            match transition.operands_changed {
                None => {
                    "supply the missing semantic key or inclusion definition needed to compare the arithmetic operands"
                }
                Some(false) => {
                    "identify the evidence or registered rule responsible for the reclassification despite unchanged arithmetic operands"
                }
                Some(true) => {
                    "bridge the changed supporting operands and explain the resulting identity classifications"
                }
            }
        };
        packet.closing_documents.insert(ClosingDocument {
            document: "source-review record and reconciliation of the cited AISC identities".into(),
            locator: Some(format!("{} -> {}", previous.at, current.at)),
            would_show: format!("For {} {period} {} between {} and {}: {request}; retain both original assertions and cite the evidence supporting any resolution.",
                history.series.subject, history.family.as_str(), previous.release, current.release),
        });
        packet.findings.push(Finding {
            period,
            at: current.at.clone(),
            state: transition.kind.as_str().into(),
            evidence_json: format!(
                "{{\"schema\":\"fmine.challenge_identity_transition.v1\",\"first_registered_caveat\":{},\"transition\":{},\"operands_changed\":{},\"ambiguous_vintages\":{},\"disclosure_classification\":\"not_assigned_to_identity_transition\",\"interpretation_limit\":\"The transition describes original arithmetic states, not an issuer admission or selection of a true assertion. Source-value disclosure classifications remain separate.\",\"first_registered\":{first_json},\"earliest_registered_assertions\":[{}],\"previous\":{previous_json},\"current\":{current_json}}}",
                js(fmine_public::consistency::FIRST_REGISTERED_CAVEAT),
                js(transition.kind.as_str()),
                transition.operands_changed.map_or_else(|| "null".into(), |b| b.to_string()),
                strings(ambiguous),
                earliest.join(","),
            ),
        });
    }
    Ok(())
}

fn unresolved_vintage(state: VintageState) -> bool {
    matches!(
        state,
        VintageState::ChangedWithoutRegisteredExplanation
            | VintageState::RestatedSilent
            | VintageState::Conflict
    )
}

/// The comparison's Claim must be the canonical projection of the cell being cited, not merely share its id.
/// Re-reading the stored token also catches a mutated projection whose digest still names the original file.
fn vintage_operand(
    report: &PublicAssuranceReport,
    claim: &KeyedCell,
    role: &str,
) -> Result<PacketOperand, Refusal> {
    let operand = read_operand(report, &claim.locator, role, None)?;
    if operand.claim.as_ref() != Some(claim) {
        return Err(unusable(format!(
            "vintage assertion is absent from or differs from the canonical Claim at {}",
            claim.locator
        )));
    }
    let token = operand
        .stored_text
        .as_deref()
        .unwrap_or_else(|| match &operand.value {
            OperandValue::Token(token) | OperandValue::Unrounded(token) => token,
            OperandValue::Int(_) | OperandValue::Blank => "",
        });
    let display_matches = match &claim.claim.value {
        PublicValue::Number {
            displayed: Ok(text),
            ..
        } => operand.displayed_text.as_ref() == Some(text) && operand.display_refusal.is_none(),
        PublicValue::Number {
            displayed: Err(code),
            ..
        } => operand.display_refusal.as_deref() == Some(*code),
        PublicValue::Missing => operand.value == OperandValue::Blank,
        _ => true,
    };
    if token != raw_token(&claim.claim.value) || !display_matches {
        return Err(unusable(format!(
            "vintage Claim value differs from the loaded source at {}",
            claim.locator
        )));
    }
    Ok(operand)
}

fn comparison_json(comparison: &Comparison) -> String {
    let mut disclosures: Vec<_> = comparison.disclosures.iter().map(|d| d.to_json()).collect();
    disclosures.sort();
    format!(
        "{{\"state\":{},\"reason\":{},\"token_glyph_changed\":{},\"rounding_only\":null,\"disclosures\":[{}]}}",
        js(comparison.state.as_str()),
        comparison.reason.map_or_else(|| "null".into(), js),
        comparison.token_glyph_changed,
        disclosures.join(",")
    )
}

fn add_vintage(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
    vintages: &VintageReport,
    item: &VintageItem,
) -> Result<(), Refusal> {
    let comparisons: Vec<_> = std::iter::once(&item.against_first)
        .chain(item.against_previous.iter())
        .collect();
    let unresolved: Vec<_> = comparisons
        .iter()
        .copied()
        .filter(|c| unresolved_vintage(c.state))
        .collect();
    if unresolved.is_empty() {
        return Ok(());
    }
    let current = item
        .current
        .iter()
        .min_by_key(|c| &c.locator)
        .ok_or_else(|| unusable("vintage finding has no current assertion"))?;
    if item.first.is_empty() || (item.against_previous.is_some() != !item.previous.is_empty()) {
        return Err(unusable(
            "vintage finding lacks its compared source assertions",
        ));
    }
    if vintages
        .disclosure_registry_digest
        .is_some_and(|d| d != report.disclosures.digest())
        || vintages
            .disclosure_registry_complete
            .is_some_and(|c| c != report.disclosures.is_complete())
    {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "vintage finding names a different disclosure registry",
        ));
    }
    let kind = if unresolved.iter().any(|c| c.state == VintageState::Conflict) {
        PacketKind::VintageConflict
    } else {
        PacketKind::VintageChange
    };
    let state = if kind == PacketKind::VintageConflict {
        VintageState::Conflict
    } else if unresolved
        .iter()
        .any(|c| c.state == VintageState::RestatedSilent)
    {
        VintageState::RestatedSilent
    } else {
        VintageState::ChangedWithoutRegisteredExplanation
    };
    if state == VintageState::RestatedSilent && vintages.disclosure_registry_complete != Some(true)
    {
        return Err(unusable(
            "a silent vintage finding requires a complete verified disclosure registry",
        ));
    }
    let reasons: BTreeSet<_> = unresolved.iter().filter_map(|c| c.reason).collect();
    let packet = packet_for(
        groups,
        report,
        GroupKey {
            origin: "vintage",
            subject: current.key.subject.clone(),
            family: format!("{}:{}", vintages.scope.as_str(), current.key.line.label),
            kind,
            // Private grouping key only; the canonical packet body binds each complete key and source Claim.
            discriminator: format!("{:?}:{reasons:?}", current.key.series_key()),
        },
        "registered semantic-key comparison; first and immediately previous releases retained; no preferred vintage",
    );
    for (role, claims) in [
        ("first_registered", &item.first),
        ("immediately_previous", &item.previous),
        ("current_release", &item.current),
    ] {
        for claim in claims {
            add_operand(packet, vintage_operand(report, claim, role)?)?;
        }
    }
    for (comparison, from) in std::iter::once((&item.against_first, &item.first))
        .chain(item.against_previous.as_ref().map(|c| (c, &item.previous)))
    {
        for note in &comparison.disclosures {
            if from.len() != 1
                || item.current.len() != 1
                || !report
                    .disclosures
                    .matching(&from[0].key, &current.key)
                    .contains(note)
            {
                return Err(unusable(
                    "vintage disclosure does not match the verified source scope",
                ));
            }
            let operand = read_operand(report, &note.at, "registered_disclosure", None)?;
            if operand.sha256 != note.sha256 {
                return Err(Refusal::new(
                    RefusalCode::DigestMismatch505,
                    format!("vintage disclosure source digest differs at {}", note.at),
                ));
            }
            // Registry verification permits whitespace-only differences in a complete quotation. Keep the
            // exact source token in the operand and the registered quote in its scoped citation.
            if !matches!(&operand.value, OperandValue::Token(text)
                if text.split_whitespace().eq(note.quoted_text.split_whitespace()))
            {
                return Err(unusable(format!(
                    "vintage disclosure quote differs at {}",
                    note.at
                )));
            }
            add_operand(packet, operand)?;
        }
    }
    let claims_json = |claims: &[KeyedCell]| {
        let mut claims: Vec<_> = claims.iter().map(KeyedCell::to_json).collect();
        claims.sort();
        format!("[{}]", claims.join(","))
    };
    packet.findings.push(Finding {
        period: current.key.valid_period.code(),
        at: current.locator.clone(),
        state: state.as_str().to_string(),
        evidence_json: format!(
            "{{\"scope\":{},\"input_claims_digest\":{},\"disclosure_registry_digest\":{},\
             \"disclosure_registry_complete\":{},\"against_first\":{},\"against_previous\":{},\
             \"first\":{},\"previous\":{},\"current\":{},\
             \"interpretation_limit\":\"Neither assertion is selected as true. Silence means no applicable verified note in the registered documents, not absence of disclosure elsewhere.\"}}",
            js(vintages.scope.as_str()), js(&vintages.input_claims_digest.to_hex()),
            vintages.disclosure_registry_digest.map_or_else(|| "null".into(), |d| js(&d.to_hex())),
            vintages.disclosure_registry_complete.map_or_else(|| "null".into(), |c| c.to_string()),
            comparison_json(&item.against_first),
            item.against_previous.as_ref().map_or_else(|| "null".into(), comparison_json),
            claims_json(&item.first), claims_json(&item.previous), claims_json(&item.current)
        ),
    });
    packet.closing_documents.insert(ClosingDocument {
        document: if kind == PacketKind::VintageConflict {
            "issuer statement of ownership basis or reconciliation of concurrent assertions"
        } else {
            "issuer reconciliation or applicable disclosure note for the cited comparative figures"
        }.to_string(),
        locator: Some(current.locator.clone()),
        would_show: "the reason for the cited difference and its applicable period, unit and ownership basis, with a source for the resolution".to_string(),
    });
    Ok(())
}

fn add_aisc(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
    instance: &IdentityInstance,
) -> Result<(), Refusal> {
    let period = fmine_public::key::parse_sheet_period(&instance.sheet)
        .map_or_else(|_| instance.sheet.clone(), |p| p.code());
    let (kind, discriminator, request, document) = match &instance.state {
        IdentityState::IssuerDiscrepancy(_) => (PacketKind::FootingDiscrepancy, period.clone(),
            "a corrected reconciliation or an issuer explanation of the printed total and its component lines".to_string(), instance.file.clone()),
        IdentityState::Explained { evidence_id, rule, basis, .. } => {
            let stated = matches!(basis, ExplanationBasis::Stated { .. });
            (if stated { PacketKind::ExplanationStated } else { PacketKind::ReconstructionUnstated },
                format!("{evidence_id}\n{rule}"),
                if stated { "independent confirmation of the cited inclusion rule and its application to these operands" }
                else { "an issuer statement confirming or replacing the registered inclusion rule" }.to_string(),
                match basis { ExplanationBasis::Stated { doc, .. } => doc.clone(), _ => instance.file.clone() })
        }
        IdentityState::Undecidable { missing_fact, closing_document, .. } =>
            (PacketKind::Undecidable, missing_fact.clone(), missing_fact.clone(), closing_document.clone()
                .unwrap_or_else(|| "issuer calculation supporting the cited rate".to_string())),
        IdentityState::Refused { reason } | IdentityState::RefusedAt { reason, .. } =>
            (PacketKind::InputRefused, reason.code().to_string(),
                format!("the missing input or interpretation needed to resolve: {}", reason.why()), instance.file.clone()),
        _ => return Ok(()),
    };
    let key = GroupKey {
        origin: "aisc",
        subject: instance.subject.clone(),
        family: instance.family.as_str().to_string(),
        kind,
        discriminator,
    };
    let packet = packet_for(
        groups,
        report,
        key,
        "component_sum (declared); rounded_once reported separately",
    );
    let mut instance = instance.clone();
    instance
        .operands
        .sort_by_key(|o| (citation::locator(o), o.role.as_str()));
    if let IdentityState::IssuerDiscrepancy(certificate) = &mut instance.state {
        certificate.line_cells.sort();
    }
    for operand in &instance.operands {
        add_operand(
            packet,
            read_operand(
                report,
                &citation::locator(operand),
                operand.role.as_str(),
                Some(&operand.value),
            )?,
        )?;
    }
    let anchor = format!("{}:{}!{}", instance.file, instance.sheet, instance.cell);
    if !packet.operands.contains_key(&anchor) {
        add_operand(packet, read_operand(report, &anchor, "anchor", None)?)?;
    }
    packet.closing_documents.insert(ClosingDocument {
        document,
        locator: match &instance.state {
            IdentityState::Explained {
                basis: ExplanationBasis::Stated { locator, .. },
                ..
            } => Some(locator.clone()),
            IdentityState::Undecidable { .. } | IdentityState::Explained { .. } => None,
            _ => Some(format!("{}!{}", instance.sheet, instance.cell)),
        },
        would_show: request,
    });
    packet.findings.push(Finding {
        period,
        at: anchor,
        state: instance.state.label().to_string(),
        evidence_json: report.exception_json(&instance, None),
    });
    Ok(())
}

fn mine_reason(instance: &Instance) -> String {
    match &instance.state {
        State::Unresolved(need) => need.code().to_string(),
        State::Refused { reason, .. } => reason.code().to_string(),
        _ => instance.state.code().to_string(),
    }
}

fn mine_finding(
    packet: &mut ChallengePacket,
    report: &PublicAssuranceReport,
    instance: &Instance,
) -> Result<Finding, Refusal> {
    let anchor = read_operand(report, &instance.anchor, "anchor", None)?;
    let at = anchor.at.clone();
    add_operand(packet, anchor)?;
    let mut cells = Vec::new();
    for cell in &instance.cells {
        match cell {
            None => cells.push("null".to_string()),
            Some(locator) => {
                let operand = read_operand(report, locator, "identity_operand", None)?;
                cells.push(js(&operand.at));
                add_operand(packet, operand)?;
            }
        }
    }
    // A printed ownership share belongs to the source row's label; retain that cell as context too.
    if instance.share.is_some() {
        let (prefix, cell) = instance
            .anchor
            .rsplit_once('!')
            .ok_or_else(|| unusable("ownership anchor has no cell"))?;
        let (row, _) =
            parse_a1_ref(cell).ok_or_else(|| unusable("ownership anchor has an invalid cell"))?;
        add_operand(
            packet,
            read_operand(report, &format!("{prefix}!A{row}"), "ownership_label", None)?,
        )?;
    }
    let refusal_at = match &instance.state {
        State::Refused { at, .. } => at.clone(),
        _ => None,
    };
    let detail = match &instance.state {
        State::Refused {
            reason: mine_ledger::Reason::KernelIndeterminate(why),
            ..
        } => Some(why.clone()),
        State::Refused {
            reason: mine_ledger::Reason::Withheld { evidence },
            ..
        } => Some(evidence.clone()),
        _ => None,
    };
    let evidence_json = format!(
        "{{\"family\":{},\"subject\":{},\"field\":{},\"group\":{},\"period\":{},\"at\":{},\
         \"sha256\":{},\"state\":{},\"construction\":\"rounded_once\",\"reason\":{},\
         \"refusal_at\":{},\"detail\":{},\"cells_in_operand_order\":[{}],\"share\":{},\
         \"printed_share\":{},\"nil_token\":{},\"component\":{}}}",
        js(instance.family.as_str()),
        js(&instance.subject),
        js(&instance.field),
        js(instance.group.as_str()),
        js(&instance.period.code()),
        js(&at),
        js(&packet.operands[&at].sha256),
        js(instance.state.code()),
        js(&mine_reason(instance)),
        optional(&refusal_at),
        optional(&detail),
        cells.join(","),
        instance
            .share
            .map_or_else(|| "null".to_string(), |s| js(&mine_ledger::rat_text(s))),
        optional(&instance.printed_share),
        instance.nil_token,
        optional(&instance.component)
    );
    Ok(Finding {
        period: instance.period.code(),
        at,
        state: instance.state.code().to_string(),
        evidence_json,
    })
}

fn add_mine(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
    instance: &Instance,
) -> Result<(), Refusal> {
    let (kind, request) = match instance.state {
        State::Discrepancy => (PacketKind::IdentityDiscrepancy,
            "an issuer reconciliation of the cited operands under the declared rounding and ownership basis".to_string()),
        State::Unresolved(_) => (PacketKind::Undecidable,
            "the region's and its members' costs in dollars, together with the quantities used to calculate the reported rate".to_string()),
        _ => (PacketKind::InputRefused, format!("the missing input or interpretation required by {}", mine_reason(instance))),
    };
    let key = GroupKey {
        origin: "mine_stats",
        subject: instance.subject.clone(),
        family: format!("{}:{}", instance.family.as_str(), instance.field),
        kind,
        discriminator: if kind == PacketKind::IdentityDiscrepancy {
            instance.period.code()
        } else {
            mine_reason(instance)
        },
    };
    let packet = packet_for(groups, report, key, "rounded_once");
    let finding = mine_finding(packet, report, instance)?;
    packet.closing_documents.insert(ClosingDocument {
        document: "issuer calculation supporting the cited Mine Stats identity".to_string(),
        locator: Some(finding.at.clone()),
        would_show: request,
    });
    packet.findings.push(finding);
    Ok(())
}

fn add_component(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
    component: &Component,
) -> Result<(), Refusal> {
    let instances = component
        .instances
        .iter()
        .map(|&index| {
            report
                .mine_ledger
                .instances
                .get(index)
                .ok_or_else(|| unusable("joint component refers to an absent identity"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let subject = instances
        .iter()
        .map(|i| i.subject.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("; ");
    let key = GroupKey {
        origin: "mine_stats_joint",
        subject,
        family: "joint_identity_component".to_string(),
        kind: PacketKind::JointDiscrepancy,
        discriminator: component.id.clone(),
    };
    let packet = packet_for(
        groups,
        report,
        key,
        "rounded_once; connected identities checked together",
    );
    for instance in instances {
        let finding = mine_finding(packet, report, instance)?;
        packet.findings.push(finding);
    }
    let mut component_cells = Vec::new();
    for cell in &component.cells {
        let operand = read_operand(report, cell, "component_cell", None)?;
        component_cells.push(operand.at.clone());
        add_operand(packet, operand)?;
    }
    component_cells.sort();
    let mut corrections = Vec::new();
    for correction in &component.corrections {
        let mut cells = Vec::new();
        for cell in &correction.cells {
            let operand = read_operand(report, cell, "correction_candidate", None)?;
            cells.push(operand.at.clone());
            add_operand(packet, operand)?;
        }
        cells.sort();
        let mut implied = Vec::new();
        for (cell, range) in &correction.implied {
            let operand = read_operand(report, cell, "correction_candidate", None)?;
            implied.push(format!(
                "{{\"at\":{},\"range\":{},\"rule\":\"exact_component_projection\",\
                \"rules_version\":{},\"in\":{}}}",
                js(&operand.at),
                js(&mine_ledger::range_text(range)),
                js(RULES_VERSION),
                strings(component_cells.iter().map(String::as_str))
            ));
            add_operand(packet, operand)?;
        }
        implied.sort();
        corrections.push(format!(
            "{{\"cells\":{},\"implied\":[{}]}}",
            strings(cells.iter().map(String::as_str)),
            implied.join(",")
        ));
    }
    corrections.sort();
    let anchor = read_operand(report, &component.id, "component_anchor", None)?;
    let at = anchor.at.clone();
    add_operand(packet, anchor)?;
    packet.findings.push(Finding { period: "joint component".to_string(), at: at.clone(), state: "joint_infeasible".to_string(),
        evidence_json: format!("{{\"component\":{},\"joint_only\":{},\"minimum\":{},\
            \"interpretation\":\"Correction sets and exact implied ranges from the checked ledger run; no replacement value is selected.\",\
            \"corrections\":[{}]}}", js(&at), component.joint_only, component.minimum, corrections.join(",")) });
    packet.closing_documents.insert(ClosingDocument { document: "issuer reconciliation of the connected published identities".to_string(),
        locator: Some(at), would_show: "which cited assertions or their declared scope should be revised so that all connected identities can hold together".to_string() });
    Ok(())
}

#[cfg(test)]
#[path = "../../xtask/testdata/challenge_packets_contract.rs"]
mod tests;
