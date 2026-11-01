//! DESIGN (BD-349): review packets built from the assurance ledger, with its source values and refusals.
//! A packet requests an evidence check, never a financial decision. These immutable drafts carry no human
//! check or external-use permission. Rendering does not change a ledger state or its established root.

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
        }
    }

    const fn owner(self) -> &'static str {
        match self {
            Self::InputRefused | Self::IdentityTransition | Self::TranscriptionConflict => {
                "project:source-curator"
            }
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
/// `iii`/`iv`/`v` instances, jointly infeasible components, exceptional AISC identity transitions and unresolved
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
mod tests {
    use super::*;
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
    use fmine_public::{LoadedSource, assess, run_synthetic_assurance, synthetic};

    fn known_packet(packets: &[ChallengePacket]) -> &ChallengePacket {
        packets
            .iter()
            .find(|p| p.subject == "Site B" && p.family == "tcc_footing")
            .expect("seeded Site B footing packet")
    }

    #[test]
    fn seeded_footing_retains_operands_witness_and_cited_arithmetic() {
        let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let root = report.ledger_anchor.state_root;
        let packets = packets_from_report(&report).expect("packets");
        let p = known_packet(&packets);
        assert_eq!(p.kind, PacketKind::FootingDiscrepancy);
        assert_eq!(p.findings.len(), 1);
        assert_eq!(p.source_root, root);
        assert_eq!(
            p.operands
                .values()
                .filter(|o| o.roles.contains("line"))
                .count(),
            3
        );
        for (cell, value) in [("C5", 300), ("C6", -50), ("C7", 0), ("C8", 350)] {
            let at = format!(
                "{}:{}!{cell}",
                synthetic::SYNTHETIC_AISC_FILE,
                synthetic::SYNTHETIC_TABLE_SHEET
            );
            let operand = &p.operands[&at];
            assert_eq!(operand.value, OperandValue::Int(value));
            assert_eq!(operand.sha256.len(), 64);
            assert!(operand.claim.is_some());
        }
        let evidence = &p.findings[0].evidence_json;
        assert!(evidence.contains("\"component_sum\":{\"v\":250,"));
        assert!(evidence.contains("\"residual\":{\"v\":100,"));
        assert!(evidence.contains("\"sign_flip_witness\":\"C6\""));
        assert!(evidence.contains("\"rule\":\"aisc.footing.residual\""));
        p.validate().expect("complete packet");
        assert_eq!(report.ledger_anchor.state_root, root);
    }

    #[test]
    fn packets_cover_visible_exceptions_and_keep_distinct_states() {
        let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let packets = packets_from_report(&report).expect("packets");
        for instance in &report.instances {
            let at = format!("{}:{}!{}", instance.file, instance.sheet, instance.cell);
            let findings = packets
                .iter()
                .filter(|p| p.family == instance.family.as_str())
                .flat_map(|p| &p.findings)
                .filter(|f| f.at == at)
                .collect::<Vec<_>>();
            if matches!(instance.state.code(), "ii" | "iii" | "iv" | "v") {
                assert_eq!(findings.len(), 1, "{at} {}", instance.family.as_str());
                assert_eq!(findings[0].state, instance.state.label());
            } else {
                assert!(
                    findings.is_empty(),
                    "consistent/not-published item was promoted to an exception"
                );
            }
        }
        assert!(
            packets
                .iter()
                .any(|p| p.kind == PacketKind::ExplanationStated)
        );
        assert!(
            packets
                .iter()
                .any(|p| p.kind == PacketKind::ReconstructionUnstated)
        );
        assert!(packets.iter().any(|p| p.kind == PacketKind::Undecidable));
        assert!(packets.iter().any(|p| p.kind == PacketKind::InputRefused));
        for packet in &packets {
            assert!(!packet.external_use_allowed());
            let json = packet.to_json();
            assert!(json.contains("\"external_use_allowed\":false"));
            assert!(json.contains("\"human_check\":null"));
            assert!(json.contains("\"lane\":\"synthetic\""));
            assert!(!json.contains(synthetic::SYNTHETIC_HIDDEN_SHEET));
            let md = packet.to_markdown();
            assert!(md.contains(SYNTHETIC_NO_AFFILIATION));
            assert!(!md.contains("From public filings"));
            assert!(md.contains(REVIEW_BOUNDARY));
        }
    }

    #[test]
    fn content_address_binds_values_digests_and_closing_requests() {
        let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let packets = packets_from_report(&report).expect("packets");
        let original = known_packet(&packets);
        let mut value_changed = original.clone();
        value_changed
            .operands
            .values_mut()
            .next()
            .expect("operand")
            .value = OperandValue::Int(991);
        assert_ne!(original.id, value_changed.content_id());
        assert!(value_changed.validate().is_err());
        let mut digest_changed = original.clone();
        digest_changed
            .operands
            .values_mut()
            .next()
            .expect("operand")
            .sha256 = "a".repeat(64);
        assert_ne!(original.id, digest_changed.content_id());
        let mut no_request = original.clone();
        no_request.closing_documents.clear();
        assert!(no_request.validate().is_err());
        let mut no_source = original.clone();
        no_source
            .operands
            .values_mut()
            .next()
            .expect("operand")
            .sha256
            .clear();
        assert!(no_source.validate().is_err());
        let mut bad_word = original.clone();
        bad_word.closing_documents.insert(ClosingDocument {
            document: "requested calculation".into(),
            locator: None,
            would_show: "a likelihood and NPV".into(),
        });
        assert!(bad_word.validate().is_err());
    }

    fn summary(name: &str, months: &str, rows: &[(&str, [&str; 4])]) -> MinimalSheet {
        let mut cells = vec![
            (
                "A3".into(),
                MinimalCell::Text(format!("For the {months} months ended June 30,")),
            ),
            ("B3".into(), MinimalCell::Number(2030.0)),
        ];
        for (col, label) in [
            ("B", "Gold produced (000s oz 100%)"),
            ("C", "Gold produced (000s oz attributable)"),
            ("D", "Gold sold (000s oz attributable)"),
            ("E", "Cost of sales ($/oz)"),
        ] {
            cells.push((format!("{col}4"), MinimalCell::Text(label.to_string())));
        }
        for (index, (site, values)) in rows.iter().enumerate() {
            let row = index + 5;
            cells.push((format!("A{row}"), MinimalCell::Text(site.to_string())));
            for (col, value) in ["B", "C", "D", "E"].into_iter().zip(values) {
                if !value.is_empty() {
                    cells.push((
                        format!("{col}{row}"),
                        MinimalCell::Number(value.parse().expect("numeric fixture")),
                    ));
                }
            }
        }
        MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }
    }

    fn mine_source(file: &str, quarter: &str, sheets: Vec<MinimalSheet>) -> LoadedSource {
        let bytes = build_minimal_xlsx(&sheets);
        let workbook = Workbook::from_bytes(file, &bytes).expect("synthetic workbook");
        LoadedSource {
            file_name: file.to_string(),
            kind: SourceKind::MineStats,
            quarter: quarter.to_string(),
            url: "synthetic".into(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        }
    }

    fn vintage_fixture(values: &[Option<MinimalCell>], with_ytd: bool) -> PublicAssuranceReport {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        let releases = ["2030Q2", "2031Q2", "2032Q2", "2033Q2"];
        let sources = values
            .iter()
            .enumerate()
            .map(|(i, value)| {
                let mut sheets = vec![summary(
                    "QTD",
                    "three",
                    &[("Site A (50%)", ["100", "50", "50", "100"])],
                )];
                if with_ytd {
                    sheets.push(summary(
                        "YTD",
                        "six",
                        &[("Site A (50%)", ["200", "100", "100", "100"])],
                    ));
                }
                for sheet in &mut sheets {
                    sheet.cells.retain(|(at, _)| at != "E5");
                    if let Some(value) = value {
                        sheet.cells.push(("E5".into(), value.clone()));
                    }
                }
                mine_source(&format!("vintage-{i}.xlsx"), releases[i], sheets)
            })
            .collect();
        assess(&profile, Lane::Synthetic, sources)
    }

    fn vintage_packets(report: &PublicAssuranceReport) -> Vec<ChallengePacket> {
        packets_from_report(report)
            .expect("cited packets")
            .into_iter()
            .filter(|p| {
                matches!(
                    p.kind,
                    PacketKind::VintageChange | PacketKind::VintageConflict
                )
            })
            .collect()
    }

    #[test]
    fn vintage_changes_keep_first_previous_current_without_duplicate_findings() {
        let mut report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(200.0)),
                Some(MinimalCell::Number(100.0)),
            ],
            true,
        );
        let packets = vintage_packets(&report);
        assert_eq!(packets.len(), 2, "Q2 and H1 changes remain separate");
        let periods: BTreeSet<_> = packets
            .iter()
            .flat_map(|p| &p.findings)
            .map(|f| f.period.as_str())
            .collect();
        assert_eq!(periods, BTreeSet::from(["2030Q2", "2030H1"]));
        for packet in &packets {
            assert_eq!(
                packet.finding_count(),
                2,
                "each later appearance occurs once"
            );
            assert_eq!(packet.operands.len(), 3);
            let second = packet
                .findings
                .iter()
                .find(|f| f.at.starts_with("vintage-2"))
                .expect("reversion remains a finding against the prior release");
            assert!(
                second
                    .evidence_json
                    .contains("\"against_first\":{\"state\":\"unchanged\"")
            );
            assert!(
                second
                    .evidence_json
                    .contains("\"against_previous\":{\"state\":\"restated_silent\"")
            );
            assert!(second.evidence_json.contains("\"rounding_only\":null"));
            for operand in packet.operands.values() {
                let claim = operand.claim.as_ref().expect("canonical Claim");
                assert!(
                    second
                        .evidence_json
                        .contains(&claim.claim.claim_id.to_string())
                );
                assert!(second.evidence_json.contains(&operand.sha256));
            }
            assert!(!packet.external_use_allowed());
            assert!(
                packet
                    .to_markdown()
                    .contains("absence of disclosure elsewhere")
            );
            packet.validate().expect("valid vintage packet");
        }
        report.sources.reverse();
        report.mine_stats_vintages.items.reverse();
        for item in &mut report.mine_stats_vintages.items {
            item.first.reverse();
            item.previous.reverse();
            item.current.reverse();
        }
        assert_eq!(
            packets
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>(),
            vintage_packets(&report)
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn vintage_packets_preserve_number_nil_not_applicable_and_missing() {
        let report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Text("—".into())),
                Some(MinimalCell::Text("n/a".into())),
                None,
            ],
            false,
        );
        let packets = vintage_packets(&report);
        assert_eq!(packets.len(), 1);
        let packet = &packets[0];
        assert_eq!(packet.finding_count(), 3);
        assert_eq!(packet.operands.len(), 4);
        let values: Vec<_> = packet
            .operands
            .values()
            .map(|o| &o.claim.as_ref().expect("Claim").claim.value)
            .collect();
        assert!(
            values
                .iter()
                .any(|v| matches!(v, PublicValue::Number { .. }))
        );
        assert!(
            values
                .iter()
                .any(|v| matches!(v, PublicValue::Nil { token, .. } if token == "—"))
        );
        assert!(
            values
                .iter()
                .any(|v| matches!(v, PublicValue::NotApplicable { token } if token == "n/a"))
        );
        assert!(values.iter().any(|v| matches!(v, PublicValue::Missing)));
    }

    #[test]
    fn vintage_packets_reject_missing_forged_and_mutated_source_assertions() {
        let mut report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(200.0)),
            ],
            false,
        );
        let item = report
            .mine_stats_vintages
            .items
            .iter_mut()
            .find(|i| i.against_first.state == VintageState::RestatedSilent)
            .expect("change");
        let original = item.current[0].clone();
        item.current[0].claim.value = PublicValue::Number {
            stored: "999".into(),
            displayed: Ok("999".into()),
            under_status: None,
        };
        assert_eq!(
            packets_from_report(&report).expect_err("forged Claim").code,
            "FMINE-REF-208"
        );
        let forged = report
            .mine_stats_vintages
            .items
            .iter()
            .find(|i| i.current[0].locator == original.locator)
            .expect("change")
            .current[0]
            .clone();
        report
            .public_claims
            .cells
            .insert(original.locator.clone(), forged);
        assert_eq!(
            packets_from_report(&report)
                .expect_err("source value differs despite matching projection")
                .code,
            "FMINE-REF-208"
        );
        report
            .public_claims
            .cells
            .insert(original.locator.clone(), original.clone());
        report
            .mine_stats_vintages
            .items
            .iter_mut()
            .find(|i| i.current[0].locator == original.locator)
            .expect("change")
            .current[0] = original.clone();
        report.sources.retain(|s| s.file_name != "vintage-0.xlsx");
        assert_eq!(
            packets_from_report(&report)
                .expect_err("missing compared source")
                .code,
            "FMINE-REF-506"
        );
    }

    #[test]
    fn vintage_packets_refuse_forged_verdicts_and_predecessor_chronology() {
        let mut unchanged = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(100.0)),
            ],
            false,
        );
        assert!(vintage_packets(&unchanged).is_empty());
        let first = unchanged.public_claims.cells["vintage-0.xlsx:QTD!E5"].clone();
        let current = unchanged.public_claims.cells["vintage-1.xlsx:QTD!E5"].clone();
        let forged = Comparison {
            state: VintageState::RestatedSilent,
            reason: Some("value"),
            token_glyph_changed: false,
            disclosures: Vec::new(),
        };
        unchanged.mine_stats_vintages.items.push(VintageItem {
            first: vec![first.clone()],
            previous: vec![first],
            current: vec![current],
            against_first: forged.clone(),
            against_previous: Some(forged),
        });
        assert_eq!(
            packets_from_report(&unchanged)
                .expect_err("unchanged source pair cannot be called silent")
                .code,
            "FMINE-REF-208"
        );
        let mut report = vintage_fixture(
            &[
                Some(MinimalCell::Number(100.0)),
                Some(MinimalCell::Number(200.0)),
                Some(MinimalCell::Number(300.0)),
            ],
            false,
        );
        let third = report
            .mine_stats_vintages
            .items
            .iter_mut()
            .find(|i| i.current[0].locator.starts_with("vintage-2"))
            .expect("third release");
        third.previous = third.first.clone();
        assert_eq!(
            packets_from_report(&report)
                .expect_err("wrong predecessor")
                .code,
            "FMINE-REF-208"
        );
    }

    #[test]
    fn vintage_ownership_conflicts_keep_printed_and_registered_basis() {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        profile
            .printed_share_corrections
            .push(fmine_public::sources::PrintedShareCorrection {
                subject: "Site A".into(),
                printed: "50".into(),
                num: 1,
                den: 4,
                evidence: "SYNTHETIC-BASIS".into(),
            });
        let report = assess(
            &profile,
            Lane::Synthetic,
            vec![mine_source(
                "basis.xlsx",
                "2030Q2",
                vec![summary(
                    "QTD",
                    "three",
                    &[("Site A (50%)", ["100", "50", "50", "100"])],
                )],
            )],
        );
        let packets = vintage_packets(&report);
        assert!(!packets.is_empty());
        for packet in packets {
            assert_eq!(packet.kind, PacketKind::VintageConflict);
            assert_eq!(packet.finding_count(), 1);
            let json = packet.to_json();
            assert!(json.contains("registered_basis_correction"));
            assert!(json.contains("SYNTHETIC-BASIS"));
            assert!(json.contains("\"printed_basis\":"));
            assert!(!packet.external_use_allowed());
        }
    }

    #[test]
    fn aisc_vintage_change_uses_the_same_packet_path_and_canonical_claims() {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        let sources = [
            ("first-aisc.xlsx", "2030Q1", 300.0),
            ("later-aisc.xlsx", "2030Q2", 301.0),
        ]
        .into_iter()
        .map(|(file, release, value)| {
            let mut cells: Vec<_> = synthetic::synthetic_aisc_cells()
                .into_iter()
                .filter(|(at, _)| at.starts_with('A') || at.starts_with('B'))
                .collect();
            cells.retain(|(at, _)| at != "B5");
            cells.push(("B5".into(), MinimalCell::Number(value)));
            let mut source = mine_source(
                file,
                release,
                vec![MinimalSheet {
                    name: synthetic::SYNTHETIC_TABLE_SHEET.into(),
                    hidden: false,
                    cells,
                }],
            );
            source.kind = SourceKind::AiscReconciliation;
            source
        })
        .collect();
        let report = assess(&profile, Lane::Synthetic, sources);
        let packets = vintage_packets(&report);
        assert_eq!(packets.len(), 1);
        let packet = &packets[0];
        assert!(packet.family.starts_with("aisc:"));
        assert_eq!(packet.kind, PacketKind::VintageChange);
        assert_eq!(packet.finding_count(), 1);
        assert_eq!(packet.findings[0].state, "restated_silent");
        for (at, value) in [
            ("first-aisc.xlsx:Q1 2030!B5", 300),
            ("later-aisc.xlsx:Q1 2030!B5", 301),
        ] {
            assert_eq!(packet.operands[at].value, OperandValue::Int(value));
            assert_eq!(
                packet.operands[at].claim.as_ref(),
                report.public_claims.cells.get(at)
            );
        }
    }

    fn identity_transition_fixture(totals: &[i64]) -> PublicAssuranceReport {
        let mut profile = synthetic::synthetic_profile();
        profile.subjects = vec!["Site A".into()];
        profile.reconstructions.clear();
        let sources = totals
            .iter()
            .enumerate()
            .map(|(index, total)| {
                let column = ["B", "D", "C"][index % 3];
                let mut cells = vec![(format!("{column}2"), MinimalCell::Text("Site A".into()))];
                for (row, label, value) in [
                    (5, "Cost of sales applicable to gold production", 100),
                    (6, "Royalty adjustment", 20),
                    (7, "Total cash costs", *total),
                    (8, "Minesite sustaining", 30),
                    (9, "All-in sustaining costs", 150),
                    (10, "Ounces sold", 10),
                    (11, "Cost of sales per ounce", 10_000),
                    (12, "Total cash costs per ounce", 12_000),
                    (13, "All-in sustaining costs per ounce", 15_000),
                ] {
                    cells.push((format!("A{row}"), MinimalCell::Text(label.into())));
                    cells.push((format!("{column}{row}"), MinimalCell::Number(value as f64)));
                }
                let mut source = mine_source(
                    &format!("identity-{index}.xlsx"),
                    &format!("{}Q2", 2030 + index),
                    vec![MinimalSheet {
                        name: "Q2 2030".into(),
                        hidden: false,
                        cells,
                    }],
                );
                source.kind = SourceKind::AiscReconciliation;
                source
            })
            .collect();
        assess(&profile, Lane::Synthetic, sources)
    }

    #[test]
    fn identity_transitions_keep_corrected_broken_states_and_source_evidence() {
        let report = identity_transition_fixture(&[119, 120, 121]);
        let original = report.to_json();
        let packets = packets_from_report(&report).expect("source-verified transitions");
        let packet = packets
            .iter()
            .find(|p| p.family == "aisc_identity:tcc_footing")
            .expect("footing history");
        assert_eq!(packet.kind, PacketKind::IdentityTransition);
        assert_eq!(packet.findings.len(), 2);
        assert_eq!(
            packet
                .findings
                .iter()
                .map(|f| f.state.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["corrected", "broken"])
        );
        for finding in &packet.findings {
            assert!(finding.evidence_json.contains("\"state\":\"iii\""));
            assert!(finding.evidence_json.contains("\"state\":\"i\""));
            assert!(finding.evidence_json.contains("\"operands_changed\":true"));
            assert!(
                finding
                    .evidence_json
                    .contains("\"component_sum\":{\"v\":120,")
            );
        }
        for (file, cell, value) in [
            ("identity-0.xlsx", "B7", 119),
            ("identity-1.xlsx", "D7", 120),
            ("identity-2.xlsx", "C7", 121),
        ] {
            let at = format!("{file}:Q2 2030!{cell}");
            let operand = &packet.operands[&at];
            assert_eq!(operand.value, OperandValue::Int(value));
            assert_eq!(operand.claim.as_ref(), report.public_claims.cells.get(&at));
        }
        for pair in [
            "identity-0.xlsx:Q2 2030!B7 -> identity-1.xlsx:Q2 2030!D7",
            "identity-1.xlsx:Q2 2030!D7 -> identity-2.xlsx:Q2 2030!C7",
        ] {
            assert!(
                packet
                    .closing_documents
                    .iter()
                    .any(|r| r.locator.as_deref() == Some(pair))
            );
        }
        assert!(
            packet
                .to_json()
                .contains("\"disclosure_classification\":\"not_assigned_to_identity_transition\"")
        );
        assert!(packet.to_markdown().contains("Recorded transition"));
        assert!(packet.to_markdown().contains("not an issuer admission"));
        assert!(!packet.external_use_allowed());
        assert_eq!(
            report.to_json(),
            original,
            "original ledger and source root remain immutable"
        );
        for totals in [vec![120], vec![120, 120]] {
            assert!(
                packets_from_report(&identity_transition_fixture(&totals))
                    .expect("ordinary histories")
                    .iter()
                    .all(|p| p.kind != PacketKind::IdentityTransition)
            );
        }
    }

    #[test]
    fn identity_transitions_export_reclassification_without_printed_value_changes() {
        let mut profile = synthetic::synthetic_profile();
        // Without a registered map a region has a release-specific, non-comparable scope. Keep the
        // invented region's membership fixed so only the reconstruction evidence changes here.
        profile.region_members = vec![fmine_public::sources::RegionMemberRow {
            region: "Region North".into(),
            member: "Site G".into(),
            first: "2030Q1".into(),
            last: None,
        }];
        let mut sources = Vec::new();
        for (file, kind, release, bytes) in synthetic::synthetic_workbooks(&[]) {
            let identities = if kind == SourceKind::AiscReconciliation {
                vec![("earlier-aisc.xlsx", "2030Q1"), (file, "2031Q1")]
            } else {
                vec![(file, release)]
            };
            for (file, release) in identities {
                let workbook = Workbook::from_bytes(file, &bytes).expect("invented source bytes");
                sources.push(LoadedSource {
                    file_name: file.into(),
                    kind,
                    quarter: release.into(),
                    url: "synthetic".into(),
                    url_basis: "synthetic".into(),
                    bytes: bytes.len() as u64,
                    census: workbook.cell_census(),
                    workbook,
                });
            }
        }
        // The registered reconstruction applies to the later file only; both AISC workbooks contain
        // identical printed values. Its cited 13-unit component is in the retained Mine Stats workbook.
        let report = assess(&profile, Lane::Synthetic, sources);
        let earlier = &report.public_claims.cells
            [&format!("earlier-aisc.xlsx:{}!H16", synthetic::SYNTHETIC_TABLE_SHEET)];
        let current = &report.public_claims.cells[&format!(
            "{}:{}!H16",
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_TABLE_SHEET
        )];
        assert_eq!(earlier.key.series_key(), current.key.series_key());
        assert_eq!(earlier.claim.value, current.claim.value);
        let family = aisc::IdentityFamily::AiscPerOunce;
        let history = report
            .issuer_consistency
            .histories
            .iter()
            .find(|h| h.series.subject == "Region North" && h.family == family)
            .expect("region history");
        assert_eq!(
            history.sequence.iter().map(|a| a.state).collect::<Vec<_>>(),
            vec!["iv", "ii"]
        );
        assert_eq!(history.transitions[0].kind, TransitionKind::Reclassified);
        assert!(
            report
                .aisc_vintages
                .items
                .iter()
                .all(|i| i.current.iter().all(|c| c.key.subject != "Region North")),
            "the source values themselves did not change"
        );
        let packets = packets_from_report(&report).expect("reclassification packet");
        let packet = packets
            .iter()
            .find(|p| {
                p.kind == PacketKind::IdentityTransition
                    && p.subject == "Region North"
                    && p.family == format!("aisc_identity:{}", family.as_str())
            })
            .expect("state-only transition is retained even without a value-change packet");
        assert_eq!(packet.findings.len(), 1);
        assert_eq!(packet.findings[0].state, "reclassified");
        assert!(
            packet.findings[0]
                .evidence_json
                .contains("\"state\":\"iv\"")
        );
        assert!(
            packet.findings[0]
                .evidence_json
                .contains("\"state\":\"ii\"")
        );
        assert!(
            packet
                .operands
                .keys()
                .any(|at| at == &format!("{}:Minestats3!B32", synthetic::SYNTHETIC_STATS_FILE))
        );
        assert!(!packet.external_use_allowed());
    }

    #[test]
    fn identity_transitions_refuse_forged_history_and_keep_order_deterministic() {
        let report = identity_transition_fixture(&[119, 120, 121]);
        let original = packets_from_report(&report).expect("original packets");
        for mutation in ["classification", "predecessor", "omission", "state"] {
            let mut forged = report.clone();
            let history = forged
                .issuer_consistency
                .histories
                .iter_mut()
                .find(|h| h.family == aisc::IdentityFamily::TccFooting)
                .expect("footing");
            match mutation {
                "classification" => history.transitions[0].kind = TransitionKind::Persisted,
                "predecessor" => history.transitions[0].to = usize::MAX,
                "omission" => history.transitions.clear(),
                "state" => history.sequence[0].state = "i",
                _ => unreachable!(),
            }
            assert_eq!(
                packets_from_report(&forged).expect_err(mutation).code,
                "FMINE-REF-208"
            );
        }
        let mut reordered = report;
        reordered.sources.reverse();
        reordered.instances.reverse();
        reordered.issuer_consistency.histories.reverse();
        for instance in &mut reordered.instances {
            instance.operands.reverse();
        }
        for history in &mut reordered.issuer_consistency.histories {
            history.transitions.reverse();
        }
        let after = packets_from_report(&reordered).expect("same evidence in different order");
        assert_eq!(
            original
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>(),
            after
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            original
                .iter()
                .map(ChallengePacket::to_markdown)
                .collect::<Vec<_>>(),
            after
                .iter()
                .map(ChallengePacket::to_markdown)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn identity_transitions_keep_every_ambiguous_earliest_assertion() {
        let mut sources = identity_transition_fixture(&[119, 120, 121]).sources;
        sources
            .iter_mut()
            .find(|s| s.file_name == "identity-1.xlsx")
            .expect("second assertion")
            .quarter = "2030Q2".into();
        let mut profile = synthetic::synthetic_profile();
        profile.subjects = vec!["Site A".into()];
        profile.reconstructions.clear();
        let report = assess(&profile, Lane::Synthetic, sources);
        let packets = packets_from_report(&report).expect("ambiguous identity history");
        let packet = packets
            .iter()
            .find(|p| p.family == "aisc_identity:tcc_footing")
            .expect("footing history");
        assert_eq!(packet.findings.len(), 2);
        for finding in &packet.findings {
            assert!(finding.evidence_json.contains("\"first_registered\":null"));
            assert!(
                finding
                    .evidence_json
                    .contains("\"earliest_registered_assertions\":[")
            );
            assert!(finding.evidence_json.contains("identity-0.xlsx:Q2 2030!B7"));
            assert!(finding.evidence_json.contains("identity-1.xlsx:Q2 2030!D7"));
        }
        assert!(packet.operands.contains_key("identity-0.xlsx:Q2 2030!B7"));
        assert!(packet.operands.contains_key("identity-1.xlsx:Q2 2030!D7"));
        assert!(
            packet
                .closing_documents
                .iter()
                .any(|r| r.would_show.contains("without choosing one arbitrarily"))
        );
    }

    fn disclosed_vintage_fixture(first_uses_old_section: bool) -> PublicAssuranceReport {
        use fmine_public::sources::{RegisteredSource, RestatementNoteRow, RestatementScopeRow};
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        profile.mine_stats_labels = ["Costs per ounce ($/oz)", "Costs per ounce"]
            .into_iter()
            .map(|section| {
                (
                    Some(section.to_string()),
                    "Cost of sales ($/oz)".to_string(),
                )
            })
            .collect();
        let mut sources = Vec::new();
        for (i, release) in ["2030Q2", "2031Q2", "2032Q2"].into_iter().enumerate() {
            let section = if i == 0 && first_uses_old_section {
                "Costs per ounce ($/oz)"
            } else {
                "Costs per ounce"
            };
            let sheet = MinimalSheet {
                name: "Minestats1".into(),
                hidden: false,
                cells: vec![
                    ("B3".into(), MinimalCell::Text("Site A (50%)".into())),
                    (
                        "A4".into(),
                        MinimalCell::Text("For the three months ended June 30,".into()),
                    ),
                    ("B4".into(), MinimalCell::Number(2030.0)),
                    ("A5".into(), MinimalCell::Text(section.into())),
                    (
                        "A6".into(),
                        MinimalCell::Text("Cost of sales ($/oz)".into()),
                    ),
                    ("B6".into(), MinimalCell::Number([100.0, 200.0, 300.0][i])),
                    (
                        "A8".into(),
                        MinimalCell::Text(
                            "Comparative figures\n  reflect the stated calculation.".into(),
                        ),
                    ),
                ],
            };
            sources.push(mine_source(
                &format!("disclosure-{i}.xlsx"),
                release,
                vec![sheet],
            ));
        }
        profile.sources = sources
            .iter()
            .map(|s| RegisteredSource {
                file_name: s.file_name.clone(),
                kind: s.kind,
                quarter: s.quarter.clone(),
                url: s.url.clone(),
                url_basis: s.url_basis.clone(),
                bytes: s.bytes,
                sha256_hex: Digest32(s.workbook.sha256).to_hex(),
            })
            .collect();
        profile.restatement_notes.push(RestatementNoteRow {
            id: "SYNTHETIC-NOTE".into(),
            file_name: "disclosure-2.xlsx".into(),
            locator: "Minestats1!A8".into(),
            quoted_text: "Comparative figures reflect the stated calculation.".into(),
            from_quarter: "2032Q2".into(),
            evidence: "SYNTHETIC".into(),
            sha256: Digest32(sources[2].workbook.sha256).to_hex(),
        });
        profile.restatement_scopes.push(RestatementScopeRow {
            note: "SYNTHETIC-NOTE".into(),
            subject: "Site A".into(),
            period_from: "2030Q2".into(),
            period_to: "2030Q2".into(),
            line: "site:costs per ounce:cost of sales ($/oz)".into(),
            occurrence: 0,
        });
        let report = assess(&profile, Lane::Synthetic, sources);
        assert!(
            report.disclosures.is_complete(),
            "{}",
            report.disclosures.to_json()
        );
        report
    }

    #[test]
    fn vintage_packets_keep_scoped_disclosures_without_promoting_explained_changes() {
        let mut report = disclosed_vintage_fixture(true);
        let packets = vintage_packets(&report);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].finding_count(), 2);
        let third = packets[0]
            .findings
            .iter()
            .find(|f| f.at.starts_with("disclosure-2"))
            .expect("first comparison stays unexplained across section alias");
        assert!(
            third
                .evidence_json
                .contains("\"against_first\":{\"state\":\"restated_silent\"")
        );
        assert!(
            third
                .evidence_json
                .contains("\"against_previous\":{\"state\":\"restated_disclosed\"")
        );
        assert!(third.evidence_json.contains("SYNTHETIC-NOTE"));
        let note = &packets[0].operands["disclosure-2.xlsx:Minestats1!A8"];
        assert!(note.roles.contains("registered_disclosure"));
        assert_eq!(
            note.value,
            OperandValue::Token("Comparative figures\n  reflect the stated calculation.".into())
        );
        assert!(
            note.claim.is_none(),
            "source note is not a manufactured numerical Claim"
        );
        let disclosed_only = disclosed_vintage_fixture(false);
        let retained = vintage_packets(&disclosed_only);
        assert_eq!(retained.len(), 1);
        assert_eq!(
            retained[0].finding_count(),
            1,
            "only the earlier, uncovered change remains"
        );
        assert!(retained[0].findings[0].at.starts_with("disclosure-1"));
        let third = report
            .mine_stats_vintages
            .items
            .iter_mut()
            .find(|i| i.current[0].locator.starts_with("disclosure-2"))
            .expect("third");
        third.against_previous.as_mut().expect("prior").disclosures[0].quoted_text =
            "invented note".into();
        assert_eq!(
            packets_from_report(&report)
                .expect_err("forged disclosure")
                .code,
            "FMINE-REF-208"
        );
    }

    #[test]
    fn mine_stats_discrepancies_unknown_costs_and_blanks_remain_distinct() {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        profile.region_members = vec![fmine_public::sources::RegionMemberRow {
            region: "Region North".into(),
            member: "Site A".into(),
            first: "2030Q1".into(),
            last: None,
        }];
        let report = assess(
            &profile,
            Lane::Synthetic,
            vec![mine_source(
                "mine.xlsx",
                "2030Q1",
                vec![summary(
                    "QTD",
                    "three",
                    &[
                        ("Site A (50%)", ["100", "50", "50", "100"]),
                        ("Site B (50%)", ["100", "80", "80", "100"]),
                        ("Site C (50%)", ["", "30", "30", "100"]),
                        ("Region North", ["100", "50", "50", "500"]),
                    ],
                )],
            )],
        );
        assert!(
            report
                .mine_ledger
                .instances
                .iter()
                .any(|i| i.state == State::Discrepancy)
        );
        assert!(
            report
                .mine_ledger
                .instances
                .iter()
                .any(|i| matches!(i.state, State::Unresolved(_)))
        );
        let packets = packets_from_report(&report).expect("mine packets");
        for instance in report.mine_ledger.instances.iter().filter(|i| {
            matches!(
                i.state,
                State::Discrepancy | State::Unresolved(_) | State::Refused { .. }
            )
        }) {
            let family = format!("{}:{}", instance.family.as_str(), instance.field);
            let packet = packets
                .iter()
                .find(|p| p.subject == instance.subject && p.family == family)
                .expect("every exceptional identity has a packet");
            assert!(
                packet
                    .findings
                    .iter()
                    .any(|f| f.state == instance.state.code())
            );
            assert!(
                packet
                    .operands
                    .keys()
                    .all(|at| at.starts_with("mine.xlsx:"))
            );
        }
        let refused = packets
            .iter()
            .find(|p| p.subject == "Site C" && p.kind == PacketKind::InputRefused)
            .expect("blank stays refused");
        assert!(refused.to_json().contains("blank_operand"));
        assert!(
            refused
                .operands
                .values()
                .any(|o| o.value == OperandValue::Blank)
        );
        let rate = packets
            .iter()
            .find(|p| p.kind == PacketKind::Undecidable)
            .expect("missing costs");
        assert!(rate.to_json().contains("cost_totals_unpublished"));
        assert!(rate.to_markdown().contains("costs in dollars"));
    }

    #[test]
    fn joint_conflict_is_exported_even_when_every_identity_holds_alone() {
        let mut profile = synthetic::synthetic_profile();
        profile.reconstructions.clear();
        let report = assess(
            &profile,
            Lane::Synthetic,
            vec![
                mine_source(
                    "first.xlsx",
                    "2030Q1",
                    vec![summary(
                        "QTD",
                        "three",
                        &[("Site A (50%)", ["101", "51", "40", "100"])],
                    )],
                ),
                mine_source(
                    "second.xlsx",
                    "2030Q2",
                    vec![
                        summary(
                            "QTD",
                            "three",
                            &[("Site A (50%)", ["101", "51", "40", "100"])],
                        ),
                        summary(
                            "YTD",
                            "six",
                            &[("Site A (50%)", ["201", "101", "80", "100"])],
                        ),
                    ],
                ),
            ],
        );
        assert!(
            report
                .mine_ledger
                .instances
                .iter()
                .all(|i| i.state == State::Consistent)
        );
        assert_eq!(report.mine_ledger.infeasible.len(), 1);
        assert!(report.mine_ledger.infeasible[0].joint_only);
        let packets = packets_from_report(&report).expect("joint packet");
        assert_eq!(packets.len(), 1);
        let p = &packets[0];
        assert_eq!(p.kind, PacketKind::JointDiscrepancy);
        assert!(p.to_json().contains("\"joint_only\":true"));
        assert!(p.to_json().contains("\"minimum\":true"));
        assert!(p.to_json().contains("exact_component_projection"));
        assert_eq!(p.findings.iter().filter(|f| f.state == "i").count(), 5);
        assert!(p.operands.keys().any(|at| at.starts_with("first.xlsx:")));
        assert!(p.operands.keys().any(|at| at.starts_with("second.xlsx:")));
        assert!(!p.to_json().contains("\"in\":[\"2030Q"));
    }

    #[test]
    fn markdown_escapes_source_markup_and_cannot_close_a_json_fence() {
        let report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let mut packet = known_packet(&packets_from_report(&report).expect("packets")).clone();
        packet.subject = "<script>|[claim](elsewhere)".into();
        packet.findings[0].evidence_json = "{\"token\":\"```\\nunsafe markup\"}".into();
        let md = packet.to_markdown();
        assert!(!md.contains("<script>"));
        assert!(md.contains("&lt;script&gt;\\|\\[claim\\]"));
        assert!(md.contains("\\u0060\\u0060\\u0060"));
    }

    #[test]
    #[ignore = "requires the registered public workbook corpus"]
    fn barrick_vintage_packets_match_nine_silent_changes_and_e474() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = fmine_public::run_barrick_m0_assurance(&root).expect("registered assurance");
        let packets = vintage_packets(&report);
        let mine_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.family.starts_with("mine_stats:"))
            .collect();
        let silent: Vec<_> = mine_packets
            .iter()
            .flat_map(|p| &p.findings)
            .filter(|f| f.state == "restated_silent")
            .collect();
        assert_eq!(
            silent.len(),
            9,
            "E474 plus W1-DR-09's eight changes, each retained once"
        );
        for original in [
            "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23",
            "Barrick_Q3_2024_Mine_Stats.xlsx:Minestats3!F57",
            "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F107",
            "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F108",
            "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F14",
            "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F27",
            "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F63",
            "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F76",
            "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B8",
        ] {
            assert_eq!(
                silent
                    .iter()
                    .filter(|f| f.evidence_json.contains(original))
                    .count(),
                1,
                "one packet finding for independently recorded source {original}"
            );
        }
        assert_eq!(
            mine_packets
                .iter()
                .filter(|p| p.kind == PacketKind::VintageConflict)
                .map(|p| p.finding_count())
                .sum::<usize>(),
            60
        );
        let e474 = mine_packets
            .iter()
            .find(|p| {
                p.findings
                    .iter()
                    .any(|f| f.at == "Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22")
            })
            .expect("E474 packet");
        for (at, value) in [
            ("Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23", 1479),
            ("Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22", 1474),
        ] {
            let operand = &e474.operands[at];
            assert_eq!(operand.value, OperandValue::Int(value));
            let claim = operand.claim.as_ref().expect("source Claim");
            assert_eq!(claim, &report.public_claims.cells[at]);
            assert_eq!(operand.sha256, claim.key.reporting_vintage.file_sha256);
        }
        for packet in &packets {
            packet.validate().expect("complete source-cited packet");
            assert!(!packet.external_use_allowed());
            assert!(packet.findings.iter().all(|f| f.state != "not_comparable"));
        }
        let envelope = crate::challenge_envelope(&report).expect("end-to-end challenge envelope");
        let json = envelope.to_json();
        assert!(json.contains("\"schema\":\"fmine.challenge_packets.v1\""));
        assert!(json.contains(&e474.id().to_string()));
        assert!(json.contains("\"human_check\":null,\"external_use_allowed\":false"));
        assert!(!json.contains("\"external_use_allowed\":true"));
        assert!(
            envelope
                .rendered_markdown
                .as_ref()
                .expect("readable packets")
                .contains("Vintage comparisons and registered disclosures")
        );
        println!(
            "E474/W1: 9 silent changes and 60 ownership-conflict findings retain canonical source Claims; JSON/Markdown packet envelope checked"
        );
    }

    #[test]
    #[ignore = "requires the registered public workbook corpus"]
    fn barrick_footing_packets_keep_both_vintages_and_known_values() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = fmine_public::run_barrick_m0_assurance(&root).expect("registered assurance");
        let packets = packets_from_report(&report).expect("public packets");
        let expected_transitions = report
            .issuer_consistency
            .histories
            .iter()
            .flat_map(|h| &h.transitions)
            .filter(|t| {
                !matches!(
                    t.kind,
                    TransitionKind::Persisted | TransitionKind::SingleVintage
                )
            })
            .count();
        assert_eq!(
            packets
                .iter()
                .filter(|p| p.kind == PacketKind::IdentityTransition)
                .map(ChallengePacket::finding_count)
                .sum::<usize>(),
            expected_transitions,
            "every exceptional AISC transition is retained exactly once"
        );
        println!("AISC identity transition packet coverage: {expected_transitions}");
        for (subject, family, total, printed, residual) in [
            ("Kibali", "tcc_footing", "O11", 70, -2),
            ("Porgera", "aisc_footing", "L18", 13, 1),
        ] {
            let p = packets
                .iter()
                .find(|p| {
                    p.subject == subject
                        && p.family == family
                        && p.kind == PacketKind::FootingDiscrepancy
                })
                .expect("known footing");
            assert_eq!(p.findings.len(), 2);
            for file in ["Barrick_Q3_2024_AISC.xlsx", "Barrick_Q2_2025_AISC.xlsx"] {
                let at = format!("{file}:Q2 2024!{total}");
                assert_eq!(p.operands[&at].value, OperandValue::Int(printed));
                let source = report
                    .sources
                    .iter()
                    .find(|s| s.file_name == file)
                    .expect("registered source");
                assert_eq!(
                    p.operands[&at].sha256,
                    Digest32(source.workbook.sha256).to_hex()
                );
            }
            assert!(
                p.to_json()
                    .contains(&format!("\"residual\":{{\"v\":{residual},"))
            );
            assert!(!p.external_use_allowed());
        }
    }
}
