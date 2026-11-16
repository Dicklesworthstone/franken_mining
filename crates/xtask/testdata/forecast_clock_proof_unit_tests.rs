#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_upper_bound_is_strict_at_nanosecond_boundaries_and_integer_extremes() {
        assert!(strictly_before(2, 2_000_000_001, "deadline").is_ok());
        for boundary in [1_999_999_999, 2_000_000_000] {
            assert_eq!(
                strictly_before(2, boundary, "deadline").unwrap_err().code,
                "FMINE-REF-206"
            );
        }
        assert!(strictly_before(i64::MIN, i64::MIN, "deadline").is_ok());
        assert_eq!(
            strictly_before(i64::MAX, i64::MAX, "deadline")
                .unwrap_err()
                .code,
            "FMINE-REF-206"
        );
        assert!(strictly_before(9_223_372_036, i64::MAX, "deadline").is_ok());
        assert!(strictly_before(9_223_372_037, i64::MAX, "deadline").is_err());
    }

    #[test]
    fn timestamp_proof_uses_retained_plant_start_including_inherited_corrections() {
        use fmine_calib::ForecastPayload;

        let mut state = Replay::default();
        for model in ["plant-production:synthetic", "synthetic-corrected"] {
            let id = state
                .ledger
                .register_payload(
                    model,
                    1_000_000_000,
                    20_000_000_000,
                    ForecastPayload::Interval {
                        lo: 40.0,
                        hi: 50.0,
                        nominal: None,
                    },
                )
                .unwrap();
            let forecast = state.ledger.forecast(id).unwrap();
            // Ordinary forecasts retain the historical outcome deadline.
            assert_eq!(registration_deadline(&state, forecast), 20_000_000_000);
            assert!(strictly_before(15, forecast.freeze_time_ns, "outcome").is_ok());

            // Both original plant projections and their corrections retain this earlier bound.
            state.plant_registration_starts.insert(id, 10_000_000_000);
            let deadline = registration_deadline(&state, state.ledger.forecast(id).unwrap());
            assert_eq!(deadline, 10_000_000_000);
            assert!(strictly_before(9, deadline, "registration").is_ok());
            for timestamp in [10, 15] {
                assert_eq!(
                    strictly_before(timestamp, deadline, "registration")
                        .unwrap_err()
                        .code,
                    "FMINE-REF-206"
                );
            }
        }
    }

    #[test]
    fn timestamp_proof_uses_assumption_physical_start_despite_later_outcome_deadline() {
        use crate::forecast::{Row, assumption, parse_subset, time};

        let hash = "11".repeat(32);
        let retained = parse_subset(&format!(
            r#"[[assumption_forecast]]
id = "report"
issuer = "synthetic"
document = "Technical.pdf"
quantity = "cost"
subject = "Example Mine"
period = "2030FY"
unit = "usd_per_oz"
basis = "100%"
physical_start = "2030-01-01T00:00:00Z"
physical_end = "2031-01-01T00:00:00Z"
outcome_document = "Outcome.pdf"
outcome_quantity = "cost"
author_role = "analyst"
lane = "synthetic"
source_document = "Technical.pdf"
source_quantity = "cost"
source_token = "100 - 120"
source_sha256 = "{hash}"
source_locator = "Technical.pdf page 1"
source_path = "corpus/Technical.pdf"
source_published_at = "2029-01-01"
source_published_at_basis = "FACT: synthetic declared publication"
source_readers = "reader-a,reader-b"
sources_sha256 = "{hash}"
transcriptions_sha256 = "{hash}"
input_sha256 = "{hash}"
"#
        ))
        .unwrap();
        let registered = time("2029-06-01T00:00:00Z").unwrap();
        let start = time("2030-01-01T00:00:00Z").unwrap();
        let end = time("2031-01-01T00:00:00Z").unwrap();
        let mut state = Replay::default();
        assumption::apply_forecast(&mut state, Row(&retained.rows[0]), registered, registered)
            .unwrap();
        let forecast = state
            .ledger
            .forecast(state.forecast_id("report").unwrap())
            .unwrap();
        assert_eq!(forecast.freeze_time_ns, end);
        let deadline = registration_deadline(&state, forecast);
        assert_eq!(deadline, start);
        assert!(strictly_before(start / 1_000_000_000 - 1, deadline, "registration").is_ok());
        for timestamp in [start / 1_000_000_000, end / 1_000_000_000 - 1] {
            assert!(strictly_before(timestamp, forecast.freeze_time_ns, "outcome").is_ok());
            assert_eq!(
                strictly_before(timestamp, deadline, "registration")
                    .unwrap_err()
                    .code,
                "FMINE-REF-206"
            );
        }
    }
}
