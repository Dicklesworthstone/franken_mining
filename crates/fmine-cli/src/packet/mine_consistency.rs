//! Cited Mine Statistics identity transitions, reconstructed before packet export.

use super::*;
use fmine_public::consistency::mine_stats::{MineStatsConsistency, assess};

fn canonical(mut value: MineStatsConsistency) -> MineStatsConsistency {
    value
        .identities
        .histories
        .sort_by(|a, b| (&a.series, a.family).cmp(&(&b.series, b.family)));
    for history in &mut value.identities.histories {
        history
            .transitions
            .sort_by_key(|t| (t.from, t.to, t.kind.as_str(), t.operands_changed));
        history.ambiguous_vintages.sort();
    }
    value
        .identities
        .unkeyed
        .sort_by(|a, b| (&a.at, a.family, &a.reason).cmp(&(&b.at, b.family, &b.reason)));
    value
        .bindings
        .sort_by_cached_key(|b| format!("{}:{:?}", b.source.to_json(), b.instance));
    value.source_claims.sort_by_cached_key(KeyedCell::to_json);
    value
}

fn source_operand(
    report: &PublicAssuranceReport,
    claim: &KeyedCell,
    role: &str,
) -> Result<PacketOperand, Refusal> {
    claim.claim.check_id()?;
    if claim.claim.metric_key != claim.key.line.label
        || claim.claim.value_tag()
            != fmine_public::key::canonical_value_tag(&claim.key, &claim.claim.value)
    {
        return Err(unusable(
            "Mine Statistics source Claim differs from its canonical quantity interpretation",
        ));
    }
    vintage_operand(report, claim, role)
}

// Repeated source rows can survive keying even when the ledger retained only one identity. Preserve
// every additional candidate without inventing an arithmetic state for a discarded identity.
fn additional_assertions(
    packet: &mut ChallengePacket,
    report: &PublicAssuranceReport,
    history: &IdentityHistory<mine_ledger::Family>,
    release: &str,
    role: &str,
) -> Result<String, Refusal> {
    let mut out = Vec::new();
    for claim in report
        .mine_stats_consistency
        .source_claims
        .iter()
        .filter(|c| {
            c.key.reporting_vintage.release == release
                && c.key.series_key() == history.series
                && !history.sequence.iter().any(|a| a.at == c.locator)
        })
    {
        add_operand(packet, source_operand(report, claim, role)?)?;
        out.push(format!("{{\"claim\":{},\"identity\":null,\"reason\":\"source assertion retained; no separate original identity in the ledger\"}}", claim.to_json()));
    }
    out.sort();
    Ok(format!("[{}]", out.join(",")))
}

/// Validating the complete projection also catches omitted transitions and changed population counts.
pub(super) fn add_packets(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
) -> Result<(), Refusal> {
    let checked = assess(&report.mine_stats_claims, &report.mine_ledger);
    if canonical(report.mine_stats_consistency.clone()) != canonical(checked) {
        return Err(unusable(
            "Mine Statistics identity transitions differ from the canonical source Claims and original identity ledger",
        ));
    }
    for history in &report.mine_stats_consistency.histories {
        add_history(groups, report, history)?;
    }
    Ok(())
}

fn appearance_json(
    packet: &mut ChallengePacket,
    report: &PublicAssuranceReport,
    history: &IdentityHistory<mine_ledger::Family>,
    appearance: &Appearance,
    role: &str,
) -> Result<String, Refusal> {
    let mut candidates = report
        .mine_stats_consistency
        .bindings
        .iter()
        .filter(|b| b.source.locator == appearance.at && b.instance.family == history.family);
    let binding = candidates.next().ok_or_else(|| {
        unusable(format!(
            "Mine Statistics appearance has no original identity at {}",
            appearance.at
        ))
    })?;
    if candidates.any(|other| other != binding) {
        return Err(unusable(format!(
            "Mine Statistics appearance has ambiguous original identities at {}",
            appearance.at
        )));
    }
    let claim = &binding.source;
    if claim.key.reporting_vintage.file_sha256 != appearance.sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "Mine Statistics appearance has a different source digest at {}",
                appearance.at
            ),
        ));
    }
    if claim.key.reporting_vintage.release != appearance.release
        || claim.key.series_key() != history.series
        || binding.instance.state.code() != appearance.state
    {
        return Err(unusable(format!(
            "Mine Statistics appearance differs from its original semantic key or state at {}",
            appearance.at
        )));
    }
    add_operand(packet, source_operand(report, claim, role)?)?;
    // Existing Mine Stats rendering retains recorded missing inputs, share labels, nil interpretation,
    // refusal details and joint-component references without replacing a source value.
    let finding = mine_finding(packet, report, &binding.instance)?;
    if finding.at != appearance.at {
        return Err(unusable(
            "Mine Statistics source alias resolves to a different identity anchor",
        ));
    }
    for locator in binding.instance.cells.iter().flatten() {
        let role = format!("{role}:identity_operand");
        let operand = read_operand(report, locator, &role, None)?;
        let checked = if let Some(claim) = &operand.claim {
            source_operand(report, claim, &role)?
        } else {
            operand
        };
        add_operand(packet, checked)?;
    }
    Ok(format!(
        "{{\"release\":{},\"claim\":{},\"identity\":{}}}",
        js(&appearance.release),
        claim.to_json(),
        finding.evidence_json
    ))
}

fn add_history(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
    history: &IdentityHistory<mine_ledger::Family>,
) -> Result<(), Refusal> {
    for transition in history.transitions.iter().filter(|t| {
        !matches!(
            t.kind,
            TransitionKind::Persisted | TransitionKind::SingleVintage
        )
    }) {
        let appearance = |index| {
            history.sequence.get(index).ok_or_else(|| {
                unusable("Mine Statistics transition references an absent appearance")
            })
        };
        let first = appearance(0)?;
        let earliest_release = history
            .ambiguous_vintages
            .iter()
            .filter(|release| *release <= &first.release)
            .min()
            .unwrap_or(&first.release);
        let previous = appearance(transition.from)?;
        let current = appearance(transition.to)?;
        let period = history.series.valid_period.code();
        let packet = packet_for(
            groups,
            report,
            GroupKey {
                origin: "mine_stats_identity_transition",
                subject: history.series.subject.clone(),
                family: format!("mine_stats_identity:{}", history.family.as_str()),
                kind: PacketKind::MineStatsIdentityTransition,
                discriminator: first.at.clone(),
            },
            "rounded_once; original identity states and input precision retained",
        );
        let mut earliest = history
            .sequence
            .iter()
            .filter(|a| &a.release == earliest_release)
            .map(|a| appearance_json(packet, report, history, a, "earliest_registered"))
            .collect::<Result<Vec<_>, _>>()?;
        earliest.sort();
        let first_json = if history
            .ambiguous_vintages
            .iter()
            .any(|release| release <= &first.release)
        {
            "null"
        } else {
            earliest.first().map_or("null", String::as_str)
        };
        let previous_json =
            appearance_json(packet, report, history, previous, "previous_bound_identity")?;
        let current_json = appearance_json(packet, report, history, current, "current_release")?;
        let extra_first = additional_assertions(
            packet,
            report,
            history,
            earliest_release,
            "earliest_registered:coexisting_assertion",
        )?;
        let extra_previous = additional_assertions(
            packet,
            report,
            history,
            &previous.release,
            "previous_bound_identity:coexisting_assertion",
        )?;
        let extra_current = additional_assertions(
            packet,
            report,
            history,
            &current.release,
            "current_release:coexisting_assertion",
        )?;
        let ambiguous: BTreeSet<_> = history
            .ambiguous_vintages
            .iter()
            .map(String::as_str)
            .collect();
        let mut intermediate = Vec::new();
        for release in &ambiguous {
            if *release <= previous.release.as_str() || *release >= current.release.as_str() {
                continue;
            }
            let assertions = additional_assertions(
                packet,
                report,
                history,
                release,
                "intervening_ambiguous_release:coexisting_assertion",
            )?;
            intermediate.push(format!(
                "{{\"release\":{},\"source_assertions\":{assertions}}}",
                js(release)
            ));
        }
        let request = if ambiguous.contains(previous.release.as_str())
            || ambiguous.contains(current.release.as_str())
            || ambiguous.contains(earliest_release.as_str())
            || !intermediate.is_empty()
        {
            "resolve the coexisting assertions without selecting one arbitrarily"
        } else {
            match transition.operands_changed {
                None => {
                    "supply the missing source key or input needed to compare the identity operands"
                }
                Some(false) => {
                    "identify the evidence or registered rule behind the changed identity classification despite unchanged arithmetic operands"
                }
                Some(true) => {
                    "bridge the changed operands, precision or ownership assumptions and explain the resulting identity classifications"
                }
            }
        };
        packet.closing_documents.insert(ClosingDocument {
            document: "source-review record and reconciliation of the cited Mine Statistics identities".into(),
            locator: Some(format!("{} -> {}", previous.at, current.at)),
            would_show: format!("For {} {period} {} between {} and {}: {request}; retain every cited assertion and cite the evidence supporting any resolution.", history.series.subject, history.family.as_str(), previous.release, current.release),
        });
        packet.findings.push(Finding {
            period,
            at: current.at.clone(),
            state: transition.kind.as_str().into(),
            evidence_json: format!("{{\"schema\":\"fmine.challenge_mine_stats_identity_transition.v1\",\"first_registered_caveat\":{},\"transition\":{},\"operands_changed\":{},\"ambiguous_vintages\":{},\"disclosure_classification\":\"not_assigned_to_identity_transition\",\"interpretation_limit\":\"Original arithmetic states under declared rounding and ownership assumptions; no issuer admission or selection of a true assertion. Source-value disclosure classifications remain separate. Previous means the previous bound identity; intervening ambiguous registered releases are retained separately.\",\"first_registered\":{first_json},\"earliest_registered_release\":{},\"earliest_registered_assertions\":[{}],\"previous_bound_identity\":{previous_json},\"current\":{current_json},\"intervening_ambiguous_releases\":[{}],\"additional_source_assertions\":{{\"earliest_registered\":{extra_first},\"previous_bound_identity\":{extra_previous},\"current\":{extra_current}}}}}",
                js(fmine_public::consistency::FIRST_REGISTERED_CAVEAT), js(transition.kind.as_str()),
                transition.operands_changed.map_or_else(|| "null".into(), |b| b.to_string()), strings(ambiguous), js(earliest_release), earliest.join(","), intermediate.join(",")),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../xtask/testdata/mine_consistency_packets_contract.rs"]
mod tests;
