//! The verb parser and the `FMT-ENV-1` envelope (`fmine_cli`; plan §33.4, PO-ADIT-4).
//!
//! Input: a selector byte, then UTF-8 text split at NUL bytes into arguments or fields. Nothing is executed:
//! `parse_args` only builds a command, and it reads no file but the cut-off registry.
//! - Even selector: the arguments go to `parse_args`. A parse error becomes a usage-error envelope, which must exit
//!   2 and print as one line of JSON.
//! - Odd selector: the fields fill every free-text slot of a refused, a usage-error and an ok envelope. Each must
//!   print as one line of JSON, and a refusal never exits 1.
#![no_main]

use std::path::Path;

use fmine_cli::{Envelope, NextAction, parse_args};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_fuzz::is_json_line;
use libfuzzer_sys::fuzz_target;

/// The workspace a parse falls back to: absent, so nothing under it can be read.
const ROOT: &str = "/nonexistent/fmine-fuzz-workspace";

fn check(env: &Envelope) {
    let json = env.to_json();
    assert!(is_json_line(&json), "not one line of JSON: {json:?}");
    assert_ne!(env.exit_code, 1, "exit code 1 is reserved (PO-ADIT-4)");
}

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    let parts: Vec<String> = text.split('\0').map(str::to_owned).collect();
    if selector % 2 == 0 {
        if let Err(message) = parse_args(&parts, Path::new(ROOT)) {
            let env = Envelope::usage_error("fuzz", &message);
            assert_eq!(env.exit_code, 2);
            check(&env);
        }
        return;
    }
    let field = |i: usize| parts.get(i).cloned().unwrap_or_default();
    let code = RefusalCode::ALL[usize::from(selector / 2) % RefusalCode::ALL.len()];
    let action = vec![NextAction::new(field(2), field(3))];
    let mut refused = Envelope::refused(field(0), Refusal::new(code, field(1)), action.clone());
    refused.warnings = parts.iter().skip(4).cloned().collect();
    refused.anchor_id = Some(field(5));
    refused.certificate = Some(field(6));
    check(&refused);
    check(&Envelope::usage_error(field(0), &field(1)));
    check(&Envelope::ok(field(0), "null".to_owned(), action));
});
