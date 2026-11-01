//! Site production calendars as data (plan §25.4, `BD-029`, `BD-032`): each `registries/calendars.toml` row with its
//! IANA zone's offsets from the snapshot `registries/zones.toml`.
//!
//! Both files are in the TOML subset. Every field is required and no other is accepted: a fixed
//! `utc_offset_minutes` on a calendar refuses, because offsets come only from the zone snapshot. Every zone a calendar
//! names needs a `[[zone]]` row, and every `[[zone]]` row must be named by a calendar, so the snapshot holds exactly
//! the zones in use. `fmine_time::ZoneRules` checks each zone's transitions. Like `metrics`, this is outside
//! `RegistryCatalog::REQUIRED_FILES`: `xtask registry-check` runs it.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use fmine_time::{SiteCalendar, ZoneRules};

use crate::RegistryError;
use crate::toml_subset::{TomlField, TomlValue, parse_subset};

const CALENDARS: &str = "calendars.toml";
const ZONES: &str = "zones.toml";
const CALENDAR_FIELDS: [&str; 5] = [
    "site_id",
    "tz_name",
    "day_start_local_hour",
    "shift_hours",
    "roster_pattern",
];

/// The site calendars in `calendars.toml` order, and the tzdata release the zone snapshot was taken from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteCalendars {
    pub tzdata: String,
    pub calendars: Vec<SiteCalendar>,
}

/// One `[[zone]]` row and the transitions listed for it, while `zones.toml` is read.
struct ZoneRows {
    /// The offset at the span's start.
    initial: i32,
    transitions: Vec<(i64, i32)>,
    /// Whether a calendar names the zone.
    named: bool,
}

/// Loads `<root>/registries/calendars.toml` with `<root>/registries/zones.toml`.
pub fn load_site_calendars(workspace_root: &Path) -> Result<SiteCalendars, RegistryError> {
    let dir = workspace_root.join("registries");
    let read = |name: &str| {
        let p = dir.join(name);
        fs::read_to_string(&p).map_err(|e| RegistryError::Io {
            path: p.display().to_string(),
            detail: e.to_string(),
        })
    };
    parse_site_calendars(&read(CALENDARS)?, &read(ZONES)?)
}

fn violation(registry: &str, detail: String) -> RegistryError {
    RegistryError::SchemaViolation {
        registry: registry.to_string(),
        detail,
    }
}

/// Refuses a field outside `keys` and a key of `keys` that is missing.
fn exact(
    registry: &str,
    what: &str,
    line: usize,
    fields: &[TomlField],
    keys: &[&str],
) -> Result<(), RegistryError> {
    if let Some(f) = fields.iter().find(|f| !keys.contains(&f.key.as_str())) {
        let why = if f.key == "utc_offset_minutes" {
            "; offsets come from registries/zones.toml"
        } else {
            ""
        };
        return Err(violation(
            registry,
            format!("line {}: {what} has no field {}{why}", f.line, f.key),
        ));
    }
    match keys.iter().find(|k| !fields.iter().any(|f| f.key == **k)) {
        Some(k) => Err(violation(
            registry,
            format!("line {line}: {what} needs {k}"),
        )),
        None => Ok(()),
    }
}

fn value<'a>(
    registry: &str,
    fields: &'a [TomlField],
    key: &str,
) -> Result<&'a TomlField, RegistryError> {
    fields
        .iter()
        .find(|f| f.key == key)
        .ok_or_else(|| violation(registry, format!("missing {key}")))
}

fn text<'a>(registry: &str, fields: &'a [TomlField], key: &str) -> Result<&'a str, RegistryError> {
    let f = value(registry, fields, key)?;
    match &f.value {
        TomlValue::Str(s) if !s.is_empty() => Ok(s),
        _ => Err(violation(
            registry,
            format!("line {}: {key} must be a non-empty string", f.line),
        )),
    }
}

/// An integer field in `range`, converted to `T`.
fn int<T: TryFrom<i64>>(
    registry: &str,
    fields: &[TomlField],
    key: &str,
    range: std::ops::RangeInclusive<i64>,
) -> Result<T, RegistryError> {
    let f = value(registry, fields, key)?;
    let refuse = || {
        violation(
            registry,
            format!(
                "line {}: {key} must be an integer in {}..={}",
                f.line,
                range.start(),
                range.end()
            ),
        )
    };
    match f.value {
        TomlValue::Int(n) if range.contains(&n) => T::try_from(n).map_err(|_| refuse()),
        _ => Err(refuse()),
    }
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Parses both files and joins them: each calendar gets its zone's [`ZoneRules`].
pub fn parse_site_calendars(calendars: &str, zones: &str) -> Result<SiteCalendars, RegistryError> {
    let zdoc = parse_subset(zones).map_err(|e| violation(ZONES, e.to_string()))?;
    let cdoc = parse_subset(calendars).map_err(|e| violation(CALENDARS, e.to_string()))?;

    exact(
        ZONES,
        "the header",
        1,
        &zdoc.top,
        &["schema_version", "tzdata", "from_utc_s", "to_utc_s"],
    )?;
    int::<i64>(ZONES, &zdoc.top, "schema_version", 1..=1)?;
    let tzdata = text(ZONES, &zdoc.top, "tzdata")?.to_string();
    let from = int::<i64>(ZONES, &zdoc.top, "from_utc_s", i64::MIN..=i64::MAX)?;
    let to = int::<i64>(ZONES, &zdoc.top, "to_utc_s", i64::MIN..=i64::MAX)?;
    let offset_range = -(18 * 60)..=18 * 60;

    let mut table: BTreeMap<String, ZoneRows> = BTreeMap::new();
    for row in &zdoc.rows {
        match row.table.as_str() {
            "zone" => {
                exact(
                    ZONES,
                    "[[zone]]",
                    row.line,
                    &row.fields,
                    &["tz_name", "tzif_sha256", "offset_minutes"],
                )?;
                let name = text(ZONES, &row.fields, "tz_name")?;
                if !is_sha256_hex(text(ZONES, &row.fields, "tzif_sha256")?) {
                    return Err(violation(
                        ZONES,
                        format!(
                            "line {}: tzif_sha256 must be 64 lowercase hex digits",
                            row.line
                        ),
                    ));
                }
                let initial = int(ZONES, &row.fields, "offset_minutes", offset_range.clone())?;
                let rows = ZoneRows {
                    initial,
                    transitions: Vec::new(),
                    named: false,
                };
                if table.insert(name.to_string(), rows).is_some() {
                    return Err(RegistryError::DuplicateId {
                        registry: ZONES.to_string(),
                        id: name.to_string(),
                    });
                }
            }
            "transition" => {
                exact(
                    ZONES,
                    "[[transition]]",
                    row.line,
                    &row.fields,
                    &["tz_name", "at_utc_s", "offset_minutes"],
                )?;
                let name = text(ZONES, &row.fields, "tz_name")?;
                let at = int(ZONES, &row.fields, "at_utc_s", i64::MIN..=i64::MAX)?;
                let offset = int(ZONES, &row.fields, "offset_minutes", offset_range.clone())?;
                let Some(zone) = table.get_mut(name) else {
                    return Err(violation(
                        ZONES,
                        format!(
                            "line {}: transition for {name}, which has no [[zone]] row above it",
                            row.line
                        ),
                    ));
                };
                zone.transitions.push((at, offset));
            }
            other => {
                return Err(violation(
                    ZONES,
                    format!("line {}: unknown table [[{other}]]", row.line),
                ));
            }
        }
    }
    let mut rules = BTreeMap::new();
    for (name, z) in &table {
        let z = ZoneRules::new(name, from, to, z.initial, z.transitions.clone())
            .map_err(|e| violation(ZONES, e.to_string()))?;
        rules.insert(name.clone(), z);
    }

    exact(CALENDARS, "the header", 1, &cdoc.top, &["schema_version"])?;
    int::<i64>(CALENDARS, &cdoc.top, "schema_version", 1..=1)?;
    let mut out: Vec<SiteCalendar> = Vec::new();
    for row in &cdoc.rows {
        if row.table != "calendar" {
            return Err(violation(
                CALENDARS,
                format!("line {}: unknown table [[{}]]", row.line, row.table),
            ));
        }
        exact(
            CALENDARS,
            "[[calendar]]",
            row.line,
            &row.fields,
            &CALENDAR_FIELDS,
        )?;
        let site_id = text(CALENDARS, &row.fields, "site_id")?;
        if out.iter().any(|c| c.site_id == site_id) {
            return Err(RegistryError::DuplicateId {
                registry: CALENDARS.to_string(),
                id: site_id.to_string(),
            });
        }
        text(CALENDARS, &row.fields, "roster_pattern")?;
        let tz_name = text(CALENDARS, &row.fields, "tz_name")?;
        let (Some(zone), Some(entry)) = (rules.get(tz_name), table.get_mut(tz_name)) else {
            return Err(violation(
                CALENDARS,
                format!(
                    "line {}: {site_id} names {tz_name}, which registries/zones.toml lacks",
                    row.line
                ),
            ));
        };
        entry.named = true;
        out.push(SiteCalendar {
            site_id: site_id.to_string(),
            zone: zone.clone(),
            day_start_hour_local: int(CALENDARS, &row.fields, "day_start_local_hour", 0..=23)?,
            shift_hours: int(CALENDARS, &row.fields, "shift_hours", 1..=24)?,
        });
    }
    if let Some((name, _)) = table.iter().find(|(_, z)| !z.named) {
        return Err(violation(
            ZONES,
            format!("zone {name} is named by no calendar"),
        ));
    }
    Ok(SiteCalendars {
        tzdata,
        calendars: out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_time::{CivilDate, NS_PER_SEC, TimeError};
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn test_workspace_calendars_load_with_their_zone_snapshot() {
        let loaded = load_site_calendars(&root()).expect("registries load");
        assert!(!loaded.tzdata.is_empty());
        let ids: Vec<&str> = loaded
            .calendars
            .iter()
            .map(|c| c.site_id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "barrick-corp",
                "ngm-carlin",
                "ngm-cortez",
                "pueblo-viejo",
                "kibali",
                "lumwana",
                "loulo-gounkoto",
                "reko-diq"
            ]
        );
        let carlin = &loaded.calendars[1];
        assert_eq!(carlin.zone.tz_name(), "America/Los_Angeles");
        // Daylight saving is in the data: 1 January 2025 is PST, 1 July 2025 is PDT.
        assert_eq!(carlin.zone.offset_minutes_at(1_735_689_600), Ok(-480));
        assert_eq!(carlin.zone.offset_minutes_at(1_751_328_000), Ok(-420));
    }

    /// `PO-TIME-3`: every offset, local-time and production-day-cut fact in the pinned CPython zoneinfo table
    /// (fixtures/fmine-time/zone_oracle_1970_2100.txt; capture record beside it) holds through the loaded
    /// registries. The table has two offset rows and four local-time rows per transition in the snapshot.
    #[test]
    fn test_po_time_3_zone_snapshot_matches_the_pinned_oracle_table() {
        let loaded = load_site_calendars(&root()).expect("registries load");
        let table =
            fs::read_to_string(root().join("fixtures/fmine-time/zone_oracle_1970_2100.txt"))
                .expect("oracle table");
        let zone = |name: &str| {
            loaded
                .calendars
                .iter()
                .map(|c| &c.zone)
                .find(|z| z.tz_name() == name)
                .unwrap_or_else(|| panic!("zone {name}"))
        };
        let mut counts = BTreeMap::new();
        for line in table.lines() {
            let f: Vec<&str> = line.split(' ').collect();
            let n = |i: usize| f[i].parse::<i64>().unwrap_or_else(|_| panic!("{line}"));
            match (f[0], f.len()) {
                ("offset", 4) => {
                    assert_eq!(
                        zone(f[1]).offset_minutes_at(n(2)),
                        Ok(n(3) as i32),
                        "{line}"
                    );
                }
                ("local", _) => {
                    let got = zone(f[1]).local_to_utc_s(n(2));
                    match f[3] {
                        "unique" => assert_eq!(got, Ok(n(4)), "{line}"),
                        "gap" => assert!(
                            matches!(got, Err(TimeError::LocalTimeNonexistent { .. })),
                            "{line}"
                        ),
                        "ambiguous" => {
                            assert!(
                                matches!(got, Err(TimeError::LocalTimeAmbiguous { .. })),
                                "{line}"
                            );
                        }
                        other => panic!("kind {other}"),
                    }
                }
                ("cut", 4) => {
                    let site = loaded
                        .calendars
                        .iter()
                        .find(|c| c.site_id == f[1])
                        .unwrap_or_else(|| panic!("site {line}"));
                    let d: Vec<i64> = f[2].split('-').map(|p| p.parse().unwrap()).collect();
                    let date = CivilDate::new(d[0] as i32, d[1] as u8, d[2] as u8).unwrap();
                    assert_eq!(
                        site.site_day_cut_utc_ns(date),
                        Ok(n(3) * NS_PER_SEC),
                        "{line}"
                    );
                }
                _ => panic!("unexpected oracle line {line}"),
            }
            *counts.entry(f[0]).or_insert(0usize) += 1;
        }
        let mut transitions = 0;
        for name in loaded
            .calendars
            .iter()
            .map(|c| c.zone.tz_name())
            .collect::<std::collections::BTreeSet<_>>()
        {
            transitions += zone(name).transitions().len();
        }
        assert!(transitions > 500, "{transitions} transitions");
        assert_eq!(counts["offset"], 2 * transitions);
        assert_eq!(counts["local"], 4 * transitions);
        assert!(counts["cut"] >= 8 * 130 * 2, "{counts:?}");
        assert!(table.contains(" gap\n") && table.contains(" ambiguous\n"));
    }

    const ZONES_OK: &str = "schema_version = 1\ntzdata = \"test\"\nfrom_utc_s = 0\nto_utc_s = 1000\n\n\
        [[zone]]\ntz_name = \"Test/A\"\ntzif_sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\noffset_minutes = 60\n\n\
        [[transition]]\ntz_name = \"Test/A\"\nat_utc_s = 500\noffset_minutes = 120\n";
    const CALENDARS_OK: &str = "schema_version = 1\n\n[[calendar]]\nsite_id = \"s1\"\ntz_name = \"Test/A\"\n\
        day_start_local_hour = 6\nshift_hours = 12\nroster_pattern = \"4/4\"\n";

    #[test]
    fn test_calendar_and_zone_rows_outside_the_schema_refuse() {
        let ok = parse_site_calendars(CALENDARS_OK, ZONES_OK).expect("minimal registries");
        assert_eq!(ok.calendars[0].zone.transitions(), &[(500, 120)]);
        let cal = |from: &str, to: &str| CALENDARS_OK.replace(from, to);
        let zon = |from: &str, to: &str| ZONES_OK.replace(from, to);
        let other_zone = "[[zone]]\ntz_name = \"Test/B\"\ntzif_sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\noffset_minutes = 0\n";
        for (calendars, zones, needle) in [
            (
                cal("shift_hours", "utc_offset_minutes = 60\nshift_hours"),
                ZONES_OK.to_string(),
                "offsets come from",
            ),
            (cal("Test/A", "Test/B"), ZONES_OK.to_string(), "lacks"),
            (
                CALENDARS_OK.to_string(),
                format!("{ZONES_OK}\n{other_zone}"),
                "named by no calendar",
            ),
            (
                CALENDARS_OK.to_string(),
                zon(
                    "tz_name = \"Test/A\"\nat_utc_s",
                    "tz_name = \"Test/B\"\nat_utc_s",
                ),
                "no [[zone]] row",
            ),
            (
                CALENDARS_OK.to_string(),
                zon("at_utc_s = 500", "at_utc_s = 1000"),
                "inside the span",
            ),
            (
                CALENDARS_OK.to_string(),
                zon("offset_minutes = 120", "offset_minutes = 60"),
                "keeps offset",
            ),
            (
                cal("day_start_local_hour = 6", "day_start_local_hour = 24"),
                ZONES_OK.to_string(),
                "day_start_local_hour",
            ),
            (
                cal("shift_hours = 12", "shift_hours = 0"),
                ZONES_OK.to_string(),
                "shift_hours",
            ),
            (
                cal("schema_version = 1", "schema_version = 2"),
                ZONES_OK.to_string(),
                "schema_version",
            ),
            (
                CALENDARS_OK.to_string(),
                zon("tzdata = \"test\"\n", ""),
                "needs tzdata",
            ),
            (
                CALENDARS_OK.to_string(),
                zon("\"0000", "\"X000"),
                "tzif_sha256",
            ),
            (
                CALENDARS_OK.to_string(),
                zon("offset_minutes = 60", "offset_minutes = 1100"),
                "offset_minutes",
            ),
            (
                cal("[[calendar]]", "[[site]]"),
                ZONES_OK.to_string(),
                "unknown table",
            ),
            (
                format!(
                    "{CALENDARS_OK}\n{}",
                    CALENDARS_OK.split_once("\n\n").unwrap().1
                ),
                ZONES_OK.to_string(),
                "duplicate id 's1'",
            ),
        ] {
            let err = parse_site_calendars(&calendars, &zones)
                .expect_err(needle)
                .to_string();
            assert!(err.contains(needle), "{needle}: {err}");
        }
    }
}
