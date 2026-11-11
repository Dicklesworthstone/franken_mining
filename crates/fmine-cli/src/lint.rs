//! Local draft adapter for the cited and discovery equality checkers (BD-345). A draft never leaves the process, and only
//! number spans and their verdicts reach stdout. The source snapshot is verified once for the entire draft.

use std::io::Read;
use std::path::Path;

use crate::lint_publicness::{self, Publication};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::sources::{IssuerProfile, is_issuer_name, registry_path};

use crate::{Envelope, NextAction, escape_json};

/// Bound the untrusted local draft before allocating or parsing it. This is a text checker, not a document
/// conversion engine; a larger document can be checked in explicit sections.
const MAX_DRAFT_BYTES: u64 = 4 * 1024 * 1024;

pub(super) fn read_draft(path: &Path) -> Result<String, Refusal> {
    let refuse = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("cannot read draft {}: {why}", path.display()),
        )
    };
    let file = std::fs::File::open(path).map_err(|e| refuse(e.to_string()))?;
    let mut draft = String::new();
    file.take(MAX_DRAFT_BYTES + 1)
        .read_to_string(&mut draft)
        .map_err(|e| refuse(e.to_string()))?;
    if draft.len() as u64 > MAX_DRAFT_BYTES {
        return Err(refuse("draft exceeds the four MiB text limit".to_string()));
    }
    Ok(draft)
}

fn load_profile(
    root: &Path,
    issuer: &str,
    through: Option<&str>,
) -> Result<(IssuerProfile, Vec<Publication>), Refusal> {
    let path = registry_path(root, issuer);
    let bad = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("source registry {}: {why}", path.display()),
        )
    };
    // Parse once: the publication dates and numerical projection must refer to the same registry.
    let registry =
        fmine_registry::sources::load_sources_registry(&path).map_err(|e| bad(e.to_string()))?;
    let publications = registry
        .sources
        .iter()
        .filter_map(|source| {
            let (Some(date), Some(basis)) = (&source.published_at, &source.published_at_basis)
            else {
                return None;
            };
            Some(Publication {
                file: source.file.clone(),
                sha256: source.sha256.clone(),
                published_at: date.clone(),
                published_at_basis: basis.clone(),
            })
        })
        .collect();
    let profile = IssuerProfile::from_registry(registry).map_err(bad)?;
    let profile = match through {
        Some(cut) => profile.through(cut)?,
        None => profile,
    };
    Ok((profile, publications))
}

pub(super) fn execute(
    path: &Path,
    issuer: Option<&str>,
    through: Option<&str>,
    discover: bool,
    as_of: Option<&str>,
    reserve_bindings: Option<&Path>,
    root: &Path,
) -> Envelope {
    let actions = || vec![NextAction::new("fmine robot-docs", crate::ROBOT_DOCS_WHY)];
    if issuer.is_none() && (through.is_some() || as_of.is_some()) {
        return Envelope::usage_error("lint", "--through and --as-of require --issuer");
    }
    if as_of.is_some_and(|cut| !fmine_registry::sources::is_date(cut)) {
        return Envelope::usage_error("lint", "lint --as-of requires YYYY-MM-DD");
    }
    if let Some(name) = issuer
        && (!is_issuer_name(name) || !registry_path(root, name).is_file())
    {
        return Envelope::usage_error("lint", &format!("unknown issuer '{name}'"));
    }
    let draft = match read_draft(&root.join(path)) {
        Ok(draft) => draft,
        Err(refusal) => return Envelope::refused("lint", refusal, actions()),
    };
    let registered = match issuer {
        Some(name) => match load_profile(root, name, through) {
            Ok(profile) => Some(profile),
            Err(refusal) => return Envelope::refused("lint", refusal, actions()),
        },
        None => None,
    };
    let snapshot = match &registered {
        Some((profile, _)) => {
            // Lint consumes workbook Claims only. Optional guidance documents are selected and
            // authenticated separately at their evidence cut; unrelated transcripts cannot prevent
            // checking a historical equality, and future bytes cannot enter through assurance extras.
            let sources = if profile.sources.is_empty() {
                Ok(Vec::new())
            } else {
                fmine_public::load_registered_sources(profile, root)
            };
            sources.map(|sources| {
                let lane = if profile.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER {
                    fmine_public::Lane::Synthetic
                } else {
                    fmine_public::Lane::Public
                };
                fmine_public::assess(profile, lane, sources)
            })
        }
        None => fmine_public::run_synthetic_assurance(&[]),
    };
    let snapshot = match snapshot {
        Ok(snapshot) => snapshot,
        Err(refusal) => {
            return Envelope::refused(
                "lint",
                refusal,
                vec![NextAction::new("fmine doctor", crate::DOCTOR_WHY)],
            );
        }
    };
    let claims: Vec<_> = snapshot.public_claims.cells.values().cloned().collect();
    let lint = if discover {
        fmine_public::lint::lint_discover(&draft, &claims)
    } else {
        fmine_public::lint::lint_cited(&draft, &claims)
    };
    let public_record = match (as_of, registered.as_ref()) {
        (Some(cut), Some((profile, publications))) => {
            match lint_publicness::assess(&draft, &lint, &claims, profile, publications, cut) {
                Ok(report) => Some(report),
                Err(refusal) => return Envelope::refused("lint", refusal, actions()),
            }
        }
        _ => None,
    };
    let forward_guidance = match (as_of, registered.as_ref()) {
        (Some(cut), Some((profile, _))) => {
            Some(crate::lint_guidance::assess(&draft, profile, root, cut))
        }
        _ => None,
    };
    let reserve_replacement = match (reserve_bindings, as_of, registered.as_ref()) {
        (Some(bindings), Some(cut), Some((profile, _))) => Some(crate::lint_reserve::assess(
            &draft, profile, bindings, cut, discover, root,
        )),
        (Some(_), _, _) => {
            return Envelope::usage_error(
                "lint",
                "--reserve-bindings requires --issuer and --as-of",
            );
        }
        _ => None,
    };
    let cut = through.map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_json(s)));
    let as_of = as_of.map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_json(s)));
    let public_json = public_record
        .as_ref()
        .map_or_else(|| "null".into(), |r| r.to_json());
    let guidance_json = forward_guidance
        .as_ref()
        .map_or_else(|| "null".into(), |r| r.to_json());
    let reserve_json = reserve_replacement.as_ref().map_or_else(String::new, |r| {
        format!(",\"reserve_replacement\":{}", r.to_json())
    });
    let has_workbooks = registered
        .as_ref()
        .is_none_or(|(profile, _)| !profile.sources.is_empty());
    let state_root = if has_workbooks {
        format!("\"{}\"", snapshot.ledger_anchor.state_root.to_hex())
    } else {
        "null".into()
    };
    let payload = format!(
        "{{\"schema\":\"fmine.lint_snapshot.v1\",\"issuer\":\"{}\",\"lane\":\"{}\",\
         \"through\":{cut},\"as_of\":{as_of},\"read_mode\":\"verified_source_projection\",\
         \"source_state_root\":{state_root},\"source_mapping_refusals\":{{\"count\":{}}},\"lint\":{},\
         \"public_record\":{public_json},\"forward_guidance\":{guidance_json}{reserve_json}}}",
        escape_json(&snapshot.issuer),
        snapshot.lane.as_str(),
        snapshot.public_claims.refusals.len(),
        lint.to_json(),
    );
    let mut envelope = Envelope::report(
        "lint",
        snapshot.ledger_anchor.anchor_id().to_string(),
        snapshot.ledger_anchor.state_root.to_hex(),
        payload,
        actions(),
    );
    if !has_workbooks {
        envelope.anchor_id = None;
        envelope.certificate = None;
    }
    let mut markdown = lint.to_markdown();
    if let Some(report) = public_record {
        markdown.push('\n');
        markdown.push_str(&report.to_markdown());
        envelope.no_claim = Some(lint_publicness::NO_CLAIM);
    } else {
        envelope.no_claim = Some(lint.mode.no_claim());
    }
    if let Some(report) = forward_guidance {
        markdown.push('\n');
        markdown.push_str(&report.to_markdown());
    }
    if let Some(report) = reserve_replacement {
        markdown.push('\n');
        markdown.push_str(&report.to_markdown());
    }
    envelope.rendered_markdown = Some(markdown);
    envelope
}

// Source-loading acceptance checks live in xtask/tests/public_lint_commands.rs.
