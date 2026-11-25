//! DESIGN: shared opt-in certificate metadata and envelope for the planning verbs.

use std::path::{Component, Path};

use fmine_core::{
    Digest32,
    anchor::SourceAnchor,
    cert::InputDigest,
    plan_cert::{PlanCalibrationStatus, PlanCertificate, PlanHorizon},
};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_optimize::plan_cert::PlanCertMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CertificateBasis {
    currency_code: String,
    horizon: PlanHorizon,
}

impl CertificateBasis {
    pub(super) fn parse(
        verb: &str,
        currency: Option<&str>,
        horizon: Option<&str>,
    ) -> Result<Option<Self>, String> {
        let (code, horizon) = match (currency, horizon) {
            (None, None) => return Ok(None),
            (Some(code), Some(horizon)) => (code, horizon),
            _ => {
                return Err(format!(
                    "{verb}: --certificate-currency and --certificate-horizon must be supplied together"
                ));
            }
        };
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(format!(
                "{verb}: certificate currency must be a three-letter uppercase code"
            ));
        }
        let horizon = match horizon {
            "strategic" => PlanHorizon::Strategic,
            "tactical" => PlanHorizon::Tactical,
            "short_term" => PlanHorizon::ShortTerm,
            _ => {
                return Err(format!(
                    "{verb}: certificate horizon must be strategic, tactical or short_term"
                ));
            }
        };
        Ok(Some(Self {
            currency_code: code.into(),
            horizon,
        }))
    }

    pub(super) fn metadata<'a>(
        &self,
        plan_id: &str,
        opt_set_id: &str,
        scale: u128,
        root: &Path,
        sources: impl IntoIterator<Item = (&'a str, &'a SourceAnchor)>,
    ) -> Result<PlanCertMetadata, Refusal> {
        let inputs = sources
            .into_iter()
            .map(|(role, source)| certificate_input(root, role, source))
            .collect::<Result<_, _>>()?;
        Ok(PlanCertMetadata {
            plan_id: plan_id.into(),
            horizon: self.horizon,
            code_revision: env!("FMINE_BUILD_REVISION").into(),
            opt_set_id: opt_set_id.into(),
            currency_code: self.currency_code.clone(),
            value_scale_denominator: scale,
            saa: None,
            vss: None,
            calibration: PlanCalibrationStatus::Uncalibrated,
            inputs,
            extra_refusal_codes: Vec::new(),
        })
    }

    pub(super) fn check_omf_currency(&self, units: &str) -> Result<(), Refusal> {
        let units = units.trim();
        // DESIGN: preserve the caller's explicit declaration when the attribute has no code-shaped unit.
        if units.len() == 3
            && units.bytes().all(|byte| byte.is_ascii_alphabetic())
            && !units.eq_ignore_ascii_case(&self.currency_code)
        {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "OMF value attribute declares {units}, but the certificate declares {}",
                    self.currency_code
                ),
            ));
        }
        Ok(())
    }
}

fn certificate_input(
    root: &Path,
    role: &str,
    source: &SourceAnchor,
) -> Result<InputDigest, Refusal> {
    let locator = Path::new(&source.artifact_path)
        .strip_prefix(root)
        .ok()
        .and_then(|path| {
            // DESIGN: a relative locator uses forward slashes even when produced on Windows.
            path.components()
                .map(|component| match component {
                    Component::Normal(name) => name.to_str(),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()
                .filter(|components| !components.is_empty())
                .map(|components| components.join("/"))
        })
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("plan certificate {role} needs a workspace-relative input locator"),
            )
        })?;
    Ok(InputDigest::new(
        role,
        locator,
        Digest32(source.artifact_sha256),
    ))
}

pub(super) fn render(cert: &PlanCertificate) -> Result<(String, String), Refusal> {
    let bytes = cert
        .to_bytes()
        .map_err(|e| Refusal::new(RefusalCode::NumericalCertificateFailed207, e.to_string()))?;
    let digest = cert.certificate_digest;
    Ok((
        format!(
            ",\"plan_certificate\":{{\"format\":\"FMT-PCERT-1\",\"digest\":\"{digest}\",\"bytes_hex\":\"{}\"}}",
            fmine_io::forecast::encode_hex_field(&bytes)
        ),
        format!(
            "\nPlan certificate `{digest}` for {} ({}). JSON includes the canonical frame.\n",
            cert.horizon.as_str(),
            cert.currency_code
        ),
    ))
}

pub(super) fn attach(
    cert: &PlanCertificate,
    json: &mut String,
    markdown: &mut String,
) -> Result<(), Refusal> {
    let (cert_json, cert_markdown) = render(cert)?;
    if json.pop() != Some('}') {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "cannot attach a plan certificate to a malformed report",
        ));
    }
    json.push_str(&cert_json);
    json.push('}');
    markdown.push_str(&cert_markdown);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omf_currency_codes_must_match_and_unlabelled_values_keep_the_declaration() {
        let basis = CertificateBasis::parse("plan schedule", Some("USD"), Some("tactical"))
            .unwrap()
            .unwrap();
        for label in ["USD", " usd ", "", "$"] {
            assert!(basis.check_omf_currency(label).is_ok());
        }
        for label in ["EUR", " cad "] {
            assert_eq!(
                basis.check_omf_currency(label).unwrap_err().code,
                "FMINE-REF-105"
            );
        }
    }
}
