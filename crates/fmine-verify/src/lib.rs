//! Std-only offline auditor verifier (`fmine-verify`, `BD-140`, `GOAL-024`, plan §33.6).

pub mod checks;

use std::fmt::Write as _;
use std::path::Path;

use checks::{
    VerifyError, parse_digest32_hex, verify_cas_root, verify_certificate_file, verify_segment_file,
    verify_sync_cert_file,
};

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn format_error(err: &VerifyError) -> (String, u8) {
    let exit_code = err.exit_code();
    let code = err.to_refusal().map_or("USAGE_ERROR", |r| r.code);
    let json = format!(
        "{{\"status\":\"refused\",\"code\":\"{}\",\"exit_code\":{},\"detail\":\"{}\"}}",
        escape_json(code),
        exit_code,
        escape_json(&err.to_string())
    );
    (json, exit_code)
}

/// Execute `fmine-verify` arguments (excluding `argv[0]`), returning `(json_line, exit_code)`.
#[must_use]
pub fn run_cli(args: &[String]) -> (String, u8) {
    let res = match args.first().map(String::as_str) {
        Some("cert") => run_cert(&args[1..]),
        Some("cas") => run_cas(&args[1..]),
        Some("synccert") => run_synccert(&args[1..]),
        Some("segment") => run_segment(&args[1..]),
        _ => Err(VerifyError::Usage(
            "expected: fmine-verify <cert <file> [--inputs-dir <dir>] | cas <dir> <root-hex> | synccert <file> | segment <file>>"
                .to_string(),
        )),
    };
    match res {
        Ok(json) => (json, 0),
        Err(err) => format_error(&err),
    }
}

fn run_cert(rest: &[String]) -> Result<String, VerifyError> {
    let (file, inputs_dir) = match rest {
        [f] => (Path::new(f.as_str()), None),
        [f, flag, dir] if flag == "--inputs-dir" => {
            (Path::new(f.as_str()), Some(Path::new(dir.as_str())))
        }
        _ => {
            return Err(VerifyError::Usage(
                "expected: fmine-verify cert <file> [--inputs-dir <dir>]".to_string(),
            ));
        }
    };
    let cert = verify_certificate_file(file, inputs_dir)?;
    Ok(format!(
        "{{\"status\":\"verified\",\"kind\":\"fmt-cert-1\",\"format_version\":{},\"subject_kind\":\"{}\",\"code_revision\":\"{}\",\"determinism_class\":\"{}\",\"inputs\":{},\"output_digest\":\"{}\",\"certificate_digest\":\"{}\"}}",
        cert.format_version,
        escape_json(&cert.subject_kind),
        escape_json(&cert.code_revision),
        cert.determinism_class.as_str(),
        cert.inputs.len(),
        cert.output_digest.to_hex(),
        cert.certificate_digest.to_hex()
    ))
}

fn run_cas(rest: &[String]) -> Result<String, VerifyError> {
    let [dir, root_hex] = rest else {
        return Err(VerifyError::Usage(
            "expected: fmine-verify cas <cas-dir> <root-sha256-hex>".to_string(),
        ));
    };
    let expected_root = parse_digest32_hex(root_hex)?;
    let manifest = verify_cas_root(Path::new(dir), &expected_root)?;
    Ok(format!(
        "{{\"status\":\"verified\",\"kind\":\"fmt-cas-1\",\"media_type\":\"{}\",\"chunks\":{},\"total_bytes\":{},\"root_digest\":\"{}\"}}",
        escape_json(&manifest.media_type),
        manifest.chunks.len(),
        manifest.total_bytes,
        manifest.root_digest.to_hex()
    ))
}

fn run_synccert(rest: &[String]) -> Result<String, VerifyError> {
    let [file] = rest else {
        return Err(VerifyError::Usage(
            "expected: fmine-verify synccert <file>".to_string(),
        ));
    };
    let cert = verify_sync_cert_file(Path::new(file))?;
    Ok(format!(
        "{{\"status\":\"verified\",\"kind\":\"synccert\",\"cell_id\":\"{}\",\"epoch\":{},\"from_seq\":{},\"to_seq\":{},\"mmr_root_before\":\"{}\",\"mmr_root_after\":\"{}\",\"proof_hashes\":{}}}",
        escape_json(&cert.cell_id),
        cert.epoch,
        cert.from_seq,
        cert.to_seq,
        cert.mmr_root_before.to_hex(),
        cert.mmr_root_after.to_hex(),
        cert.consistency_proof.len()
    ))
}

fn run_segment(rest: &[String]) -> Result<String, VerifyError> {
    let [file] = rest else {
        return Err(VerifyError::Usage(
            "expected: fmine-verify segment <file>".to_string(),
        ));
    };
    let report = verify_segment_file(Path::new(file))?;
    Ok(format!(
        "{{\"status\":\"verified\",\"kind\":\"fmt-seg-1\",\"cell_id\":\"{}\",\"segment_status\":\"{}\",\"record_count\":{},\"head_hash\":\"{}\",\"mmr_root\":\"{}\"}}",
        escape_json(&report.cell_id),
        report.status,
        report.record_count,
        report.head_hash.to_hex(),
        report.mmr_root.to_hex()
    ))
}
