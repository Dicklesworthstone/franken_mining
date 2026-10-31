//! DESIGN (BD-349): review packets built from the assurance ledger, with its source values and refusals.
//! A packet requests an evidence check, never a financial decision. These immutable drafts carry no human
//! check or external-use permission. Rendering does not change a ledger state or its established root.

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
use fmine_public::key::KeyedCell;
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
        }
    }

    const fn owner(self) -> &'static str {
        match self {
            Self::InputRefused => "project:source-curator",
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
        format!(
            "{{\"period\":{},\"at\":{},\"state\":{},\"ledger_evidence\":{}}}",
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
             \"findings\":[{}],\"operands\":[{}]}}",
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
                .map(Finding::to_json)
                .collect::<Vec<_>>()
                .join(","),
            self.operands
                .values()
                .map(PacketOperand::to_json)
                .collect::<Vec<_>>()
                .join(",")
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
        if self.findings.is_empty() || self.operands.is_empty() || self.closing_documents.is_empty()
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
        out.push_str("\n## Findings retained\n\n| Period | Ledger state | Cited anchor |\n| --- | --- | --- |\n");
        for finding in &self.findings {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                md(&finding.period),
                md(&finding.state),
                md(&finding.at)
            );
        }
        out.push_str("\nThe ledger states are retained. A joint conflict does not change an identity that was consistent alone.\n\n## Source operands\n\n| Locator | Value or token as printed | Stored text | Role | Claim | SHA-256 |\n| --- | --- | --- | --- | --- | --- |\n");
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
        out.push_str("\n## Arithmetic and retained evidence\n\nThe cited records below retain each construction, operand role, missing input and available certificate. A sign-flip witness is a possible explanation, not a determination of cause. Correction ranges are alternatives, not replacement values.\n\n");
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
/// `iii`/`iv`/`v` instances and jointly infeasible components. Hidden and not-published instances remain outside
/// this population. Grouping never drops a finding or assumes that two different missing facts have one cause.
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
    fn ordering_does_not_change_packet_ids_or_bytes() {
        let mut report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let original = packets_from_report(&report).expect("packets");
        report.instances.reverse();
        report.sources.reverse();
        for instance in &mut report.instances {
            instance.operands.reverse();
            if let IdentityState::IssuerDiscrepancy(c) = &mut instance.state {
                c.line_cells.reverse();
            }
        }
        let reordered = packets_from_report(&report).expect("reordered packets");
        assert_eq!(
            original
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>(),
            reordered
                .iter()
                .map(ChallengePacket::to_json)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            original
                .iter()
                .map(ChallengePacket::to_markdown)
                .collect::<Vec<_>>(),
            reordered
                .iter()
                .map(ChallengePacket::to_markdown)
                .collect::<Vec<_>>()
        );
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

    #[test]
    fn missing_source_and_mismatched_ledger_operand_refuse() {
        let mut report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        report
            .sources
            .retain(|s| s.kind != SourceKind::AiscReconciliation);
        assert_eq!(
            packets_from_report(&report)
                .expect_err("source missing")
                .code,
            "FMINE-REF-506"
        );
        let mut report = run_synthetic_assurance(&[]).expect("synthetic assurance");
        let instance = report
            .instances
            .iter_mut()
            .find(|i| i.subject == "Site B" && i.family == aisc::IdentityFamily::TccFooting)
            .expect("instance");
        instance.operands[0].value = OperandValue::Int(999);
        assert_eq!(
            packets_from_report(&report)
                .expect_err("source differs")
                .code,
            "FMINE-REF-208"
        );
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
    fn barrick_footing_packets_keep_both_vintages_and_known_values() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = fmine_public::run_barrick_m0_assurance(&root).expect("registered assurance");
        let packets = packets_from_report(&report).expect("public packets");
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
