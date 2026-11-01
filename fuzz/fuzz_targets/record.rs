//! The forecast record's text inputs (`fmine_calib::record`): the rules, the manifest and `forecasts.csv`.
//!
//! Input: a selector byte, then the bytes. `selector % 3` picks the parser; `forecasts.csv` is checked against the
//! committed FY2026 rules.
#![no_main]

use std::sync::OnceLock;

use fmine_calib::record::{RecordRules, parse_forecasts, parse_manifest, parse_rules};
use libfuzzer_sys::fuzz_target;

const RULES: &str = include_str!("../../forecasts/fy2026/rules.toml");

fn rules() -> &'static RecordRules {
    static PARSED: OnceLock<RecordRules> = OnceLock::new();
    PARSED.get_or_init(|| parse_rules(RULES).expect("the committed FY2026 rules parse"))
}

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    match selector % 3 {
        0 => {
            if let Ok(text) = std::str::from_utf8(rest) {
                drop(parse_rules(text));
            }
        }
        1 => {
            if let Ok(text) = std::str::from_utf8(rest) {
                drop(parse_manifest(text));
            }
        }
        _ => drop(parse_forecasts(rest, rules())),
    }
});
