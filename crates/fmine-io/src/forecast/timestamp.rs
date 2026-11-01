//! DESIGN (BD-076): offline RFC 3161 verification with caller-selected trust roots.
//! The receipt is read once; both OpenSSL processes consume that exact snapshot through stdin.
//! Decoded time is provisional until signature, imprint and certificate checks succeed at genTime.
//! This authenticates a TSA assertion under the supplied roots, not the caller's choice of roots.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;

use fmine_calib::record::{ProofTimes, parse_utc};
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_runtime::process::{ProcessError, output_with_input};

const MAX_INPUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Exact input fingerprints and the conservatively rounded, authenticated TSA time interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedTimestamp {
    pub times: ProofTimes,
    pub receipt_sha256: Digest32,
    pub trust_roots_sha256: Digest32,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::DigestMismatch505, detail)
}

fn unavailable(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, detail)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, Refusal> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| unavailable(format!("cannot inspect {}: {e}", path.display())))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_INPUT_BYTES {
        return Err(unavailable(format!(
            "{} must be a nonempty regular, non-symlink file of at most {MAX_INPUT_BYTES} bytes",
            path.display()
        )));
    }
    let file = File::open(path)
        .map_err(|e| unavailable(format!("cannot open {}: {e}", path.display())))?;
    let opened = file
        .metadata()
        .map_err(|e| unavailable(format!("cannot inspect opened {}: {e}", path.display())))?;
    if !opened.is_file() || opened.len() > MAX_INPUT_BYTES {
        return Err(unavailable("timestamp input changed while opening"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| unavailable(format!("cannot read {}: {e}", path.display())))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(unavailable(
            "timestamp input is empty or exceeds its byte bound",
        ));
    }
    Ok(bytes)
}

fn field<'a>(text: &'a str, prefix: &str) -> Result<&'a str, String> {
    let mut fields = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix(prefix).map(str::trim));
    let value = fields.next().ok_or_else(|| format!("no {prefix:?} line"))?;
    if value.is_empty() || fields.next().is_some() {
        return Err(format!("empty or duplicate {prefix:?} line"));
    }
    Ok(value)
}

fn accuracy_part(part: &str, unit: &str, per_second: u64) -> Result<u64, String> {
    let fields: Vec<&str> = part.split_whitespace().collect();
    let [value, actual_unit] = fields[..] else {
        return Err(format!("accuracy part {part:?} is not in {unit}"));
    };
    if actual_unit != unit {
        return Err(format!("accuracy part {part:?} is not in {unit}"));
    }
    if value == "unspecified" {
        return Ok(0);
    }
    let n = match value.strip_prefix("0x") {
        Some(hex) if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
            u64::from_str_radix(hex, 16)
        }
        None if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => value.parse(),
        _ => return Err(format!("invalid accuracy value {value:?}")),
    }
    .map_err(|_| format!("accuracy value {value:?} overflows"))?;
    // RFC 3161 constrains specified millis and micros to 1..=999.
    if per_second != 1 && !(1..=999).contains(&n) {
        return Err(format!("accuracy {unit} must be in 1..=999 when specified"));
    }
    Ok(n.div_ceil(per_second))
}

fn parsed_times(text: &str) -> Result<(ProofTimes, i64), String> {
    if text.len() > MAX_TEXT_BYTES || text.contains('\0') {
        return Err("timestamp text is oversized or contains NUL".to_string());
    }
    let stamp: Vec<&str> = field(text, "Time stamp:")?.split_whitespace().collect();
    let [month, day, time, year, "GMT"] = stamp[..] else {
        return Err(format!("unreadable time stamp {stamp:?}"));
    };
    let month = MONTHS
        .iter()
        .position(|candidate| *candidate == month)
        .ok_or_else(|| format!("unreadable month {month:?}"))?;
    let (whole, fraction) = match time.split_once('.') {
        Some((whole, fraction))
            if !fraction.is_empty() && fraction.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (whole, fraction)
        }
        Some(_) => return Err("invalid fractional timestamp seconds".to_string()),
        None => (time, ""),
    };
    let iso = format!("{year}-{:02}-{:0>2}T{whole}Z", month + 1, day);
    let generated = parse_utc(&iso).ok_or_else(|| format!("invalid timestamp {iso:?}"))?;
    let parts: Vec<&str> = field(text, "Accuracy:")?.split(',').collect();
    let accuracy = if parts == ["unspecified"] {
        0
    } else {
        let [seconds, millis, micros] = parts[..] else {
            return Err(format!("unreadable accuracy {parts:?}"));
        };
        [
            accuracy_part(seconds, "seconds", 1)?,
            accuracy_part(millis, "millis", 1_000)?,
            accuracy_part(micros, "micros", 1_000_000)?,
        ]
        .into_iter()
        .try_fold(0_u64, u64::checked_add)
        .ok_or("accuracy overflows")?
    };
    let accuracy = i64::try_from(accuracy).map_err(|_| "accuracy overflows".to_string())?;
    let partial = i64::from(fraction.bytes().any(|b| b != b'0'));
    match (
        generated.checked_sub(accuracy),
        generated
            .checked_add(partial)
            .and_then(|upper| upper.checked_add(accuracy)),
    ) {
        (Some(lower_utc), Some(upper_utc)) => Ok((
            ProofTimes {
                lower_utc,
                upper_utc,
            },
            generated,
        )),
        _ => Err(format!(
            "accuracy of {accuracy} s overflows the proof interval"
        )),
    }
}

/// Decode OpenSSL's English text, without authenticating it. Each stated accuracy component rounds
/// upward to whole seconds; a nonzero genTime fraction widens the upper endpoint by one second.
/// An unspecified component contributes zero, preserving the frozen-record verifier's rounding rule.
pub fn proof_times_from_text(text: &str) -> Result<ProofTimes, String> {
    parsed_times(text).map(|(times, _)| times)
}

fn openssl(command: &mut Command, receipt: &[u8], roots: &Path) -> Result<String, Refusal> {
    // Explicit local configuration and trust inputs prevent environment-supplied stores/providers.
    // A regular file cannot serve as a default hashed CA directory. ts 3.x has no -no-CApath option.
    command
        .env("LC_ALL", "C")
        .env("OPENSSL_CONF", "/dev/null")
        .env("SSL_CERT_FILE", roots)
        .env("SSL_CERT_DIR", roots)
        .env_remove("OPENSSL_CONF_INCLUDE")
        .env_remove("OPENSSL_MODULES")
        .env_remove("OPENSSL_ENGINES");
    let captured = output_with_input(command, receipt, MAX_TEXT_BYTES).map_err(|e| match e {
        ProcessError::Spawn(_) => Refusal::new(
            RefusalCode::NotObservable202,
            format!("cannot start offline OpenSSL timestamp verifier: {e}"),
        ),
        ProcessError::OutputLimit { .. } => invalid(format!("OpenSSL timestamp response: {e}")),
        _ => unavailable(format!("cannot run OpenSSL on timestamp snapshot: {e}")),
    })?;
    let output = captured.output;
    if !output.status.success() {
        return Err(invalid(format!(
            "openssl ts failed: {} {}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    if let Some(e) = captured.input_error {
        return Err(unavailable(format!(
            "cannot feed complete timestamp snapshot: {e}"
        )));
    }
    String::from_utf8(output.stdout).map_err(|_| invalid("OpenSSL timestamp text is not UTF-8"))
}

/// Verify a SHA-256 RFC 3161 reply offline against explicitly supplied PEM trust roots.
/// Requires OpenSSL 3.x and Unix `/dev/stdin`; unsupported installations refuse. Certificate
/// validity is checked at the authenticated genTime, never today's date, and is never disabled.
/// Roots are fingerprinted before and after verification; a changed file invalidates the result.
pub fn verify_rfc3161(
    receipt: &Path,
    trusted_roots: &Path,
    digest: Digest32,
) -> Result<VerifiedTimestamp, Refusal> {
    verify_rfc3161_with_intermediates(receipt, trusted_roots, digest, None)
}

/// The same verifier with an optional bounded PEM file of untrusted intermediate certificates.
/// Intermediate certificates may complete a chain but never become additional trust anchors.
pub fn verify_rfc3161_with_intermediates(
    receipt: &Path,
    trusted_roots: &Path,
    digest: Digest32,
    intermediates: Option<&Path>,
) -> Result<VerifiedTimestamp, Refusal> {
    let receipt = read_bounded(receipt)?;
    let roots = read_bounded(trusted_roots)?;
    let root_path = fs::canonicalize(trusted_roots)
        .map_err(|e| unavailable(format!("cannot resolve timestamp trust roots: {e}")))?;
    let trust_roots_sha256 = sha256(&roots);
    if sha256(&read_bounded(&root_path)?) != trust_roots_sha256 {
        return Err(invalid(
            "timestamp trust roots changed while resolving their path",
        ));
    }
    let text = openssl(
        Command::new("openssl").args(["ts", "-reply", "-in", "/dev/stdin", "-text"]),
        &receipt,
        &root_path,
    )?;
    if field(&text, "Hash Algorithm:").map_err(invalid)? != "sha256" {
        return Err(invalid(
            "timestamp message imprint algorithm must be SHA-256",
        ));
    }
    let (times, generated) = parsed_times(&text).map_err(invalid)?;
    let mut command = Command::new("openssl");
    command
        .args(["ts", "-verify", "-in", "/dev/stdin", "-digest"])
        .arg(digest.to_hex())
        .arg("-CAfile")
        .arg(&root_path)
        .arg("-CAstore")
        .arg(&root_path)
        .arg("-attime")
        .arg(generated.to_string());
    let intermediate_snapshot = intermediates
        .map(|path| {
            let bytes = read_bounded(path)?;
            let resolved = fs::canonicalize(path)
                .map_err(|e| unavailable(format!("cannot resolve timestamp intermediates: {e}")))?;
            let hash = sha256(&bytes);
            if sha256(&read_bounded(&resolved)?) != hash {
                return Err(invalid(
                    "timestamp intermediates changed while resolving their path",
                ));
            }
            command.arg("-untrusted").arg(&resolved);
            Ok((path, resolved, hash))
        })
        .transpose()?;
    let verified = openssl(&mut command, &receipt, &root_path)?;
    if verified.trim() != "Verification: OK" {
        return Err(invalid("openssl ts did not report Verification: OK"));
    }
    if sha256(&read_bounded(trusted_roots)?) != trust_roots_sha256
        || sha256(&read_bounded(&root_path)?) != trust_roots_sha256
    {
        return Err(invalid("timestamp trust roots changed during verification"));
    }
    if let Some((path, resolved, hash)) = intermediate_snapshot
        && (sha256(&read_bounded(path)?) != hash || sha256(&read_bounded(&resolved)?) != hash)
    {
        return Err(invalid(
            "timestamp intermediates changed during verification",
        ));
    }
    Ok(VerifiedTimestamp {
        times,
        receipt_sha256: sha256(&receipt),
        trust_roots_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_calib::record::{format_utc, parse_hex32};

    const TEXT: &str = "Time stamp: Oct 30 14:04:42 2026 GMT\nAccuracy: 0x01 seconds, unspecified millis, unspecified micros\n";

    #[test]
    fn timestamp_accuracy_rounds_outward_without_floating_point() {
        let plain = proof_times_from_text(TEXT).expect("whole seconds");
        assert_eq!(format_utc(plain.lower_utc), "2026-10-30T14:04:41Z");
        assert_eq!(format_utc(plain.upper_utc), "2026-10-30T14:04:43Z");
        let fraction = TEXT.replace("42 2026", "42.0001 2026");
        assert_eq!(
            proof_times_from_text(&fraction).unwrap().upper_utc,
            plain.upper_utc + 1
        );
        let zero = TEXT.replace("42 2026", "42.0000 2026");
        assert_eq!(proof_times_from_text(&zero).unwrap(), plain);
        let components = TEXT
            .replace("unspecified millis", "1 millis")
            .replace("unspecified micros", "0x01 micros");
        let wider = proof_times_from_text(&components).unwrap();
        assert_eq!(wider.lower_utc, plain.lower_utc - 2);
        assert_eq!(wider.upper_utc, plain.upper_utc + 2);
    }

    #[test]
    fn timestamp_text_refuses_ambiguous_malformed_and_overflowing_fields() {
        for malformed in [
            format!("{TEXT}{TEXT}"),
            format!("{TEXT}Accuracy: unspecified\n"),
            TEXT.replace("42 2026", "42. 2026"),
            TEXT.replace("42 2026", "42.nan 2026"),
            TEXT.replace("42 2026", "42.1.2 2026"),
            TEXT.replace("Oct 30", "Oct 32"),
            TEXT.replace("GMT", "UTC"),
            TEXT.replace("0x01 seconds", "+1 seconds"),
            TEXT.replace("0x01 seconds", "0x01seconds"),
            TEXT.replace("0x01 seconds", "-1 seconds"),
            TEXT.replace("0x01 seconds", "18446744073709551616 seconds"),
            TEXT.replace("0x01 seconds", "0x7fffffffffffffff seconds"),
            TEXT.replace("unspecified millis", "1000 millis"),
            TEXT.replace("unspecified micros", "0 micros"),
            TEXT.replace("Time stamp:", "Other stamp:"),
            format!("{TEXT}\0"),
        ] {
            assert!(proof_times_from_text(&malformed).is_err(), "{malformed:?}");
        }
    }

    #[test]
    #[ignore = "requires OpenSSL 3.x; exercises committed public timestamp and roots offline"]
    fn timestamp_public_receipt_binds_digest_and_explicit_roots() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let receipt = root.join("forecasts/fy2026/rules.tsr");
        let roots = root.join("forecasts/tsa/digicert-trusted-root-g4.pem");
        let hex = fs::read_to_string(root.join("forecasts/fy2026/rules.sha256")).unwrap();
        let digest = Digest32(parse_hex32(hex.trim()).unwrap());
        let verified = verify_rfc3161(&receipt, &roots, digest).unwrap();
        assert_eq!(format_utc(verified.times.lower_utc), "2026-10-31T16:28:41Z");
        assert_eq!(verified.times.lower_utc, verified.times.upper_utc);
        assert_eq!(
            verified.receipt_sha256,
            sha256(&fs::read(&receipt).unwrap())
        );
        assert_eq!(
            verified.trust_roots_sha256,
            sha256(&fs::read(&roots).unwrap())
        );
        let mut wrong = digest;
        wrong.0[0] ^= 1;
        assert_eq!(
            verify_rfc3161(&receipt, &roots, wrong).unwrap_err().code,
            "FMINE-REF-505"
        );
        let other = root.join("forecasts/tsa/freetsa-root.pem");
        assert_eq!(
            verify_rfc3161(&receipt, &other, digest).unwrap_err().code,
            "FMINE-REF-505"
        );
    }
}
