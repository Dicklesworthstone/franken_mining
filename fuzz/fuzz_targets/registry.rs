//! The TOML-subset registries (`fmine_registry`): the subset parser itself and each registry built on it.
//!
//! Input: a selector byte, then UTF-8 text. `selector % 6` picks the parser. Parsers that need a second registry
//! take the committed one (the source registry's documents, the time-zone registry).
#![no_main]

use std::sync::OnceLock;

use fmine_registry::calendars::parse_site_calendars;
use fmine_registry::metrics::MetricRegistry;
use fmine_registry::sources::{DocumentRow, parse_sources_registry};
use fmine_registry::table_identities::parse_table_identities;
use fmine_registry::toml_subset::parse_subset;
use fmine_registry::transcriptions::parse_transcriptions;
use libfuzzer_sys::fuzz_target;

const SOURCES: &str = include_str!("../../registries/sources/barrick.toml");
const ZONES: &str = include_str!("../../registries/zones.toml");
const CALENDARS: &str = include_str!("../../registries/calendars.toml");

fn documents() -> &'static [DocumentRow] {
    static DOCS: OnceLock<Vec<DocumentRow>> = OnceLock::new();
    DOCS.get_or_init(|| {
        parse_sources_registry(SOURCES)
            .expect("the committed source registry parses")
            .documents
    })
}

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    match selector % 6 {
        0 => drop(parse_subset(text)),
        1 => drop(parse_sources_registry(text)),
        2 => drop(parse_table_identities(text, "barrick", documents())),
        3 => drop(parse_transcriptions(text, "barrick", documents())),
        4 => {
            drop(parse_site_calendars(text, ZONES));
            drop(parse_site_calendars(CALENDARS, text));
        }
        _ => drop(MetricRegistry::parse(text)),
    }
});
