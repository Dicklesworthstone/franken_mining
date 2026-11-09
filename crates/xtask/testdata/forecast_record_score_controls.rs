#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| (*s).into()).collect()
    }

    #[test]
    fn record_scoring_requires_explicit_record_cut_and_trust_roots() {
        let valid = args(&[
            "forecast",
            "record",
            "score",
            "--record",
            "record",
            "--as-of",
            "2030-02-01T00:00:00Z",
            "--tsa-roots",
            "trusted.pem",
        ]);
        let (command, format, _) = parse(&valid, Path::new(".")).expect("explicit inputs");
        assert_eq!(command.record, Path::new("record"));
        assert_eq!(format, OutputFormat::Json);
        for flag in ["--record", "--as-of", "--tsa-roots"] {
            let mut missing = valid.clone();
            let at = missing.iter().position(|s| s == flag).expect("flag");
            missing.drain(at..at + 2);
            assert!(parse(&missing, Path::new(".")).is_err());
        }
        for extra in [
            args(&["--trust-host-clock"]),
            args(&["--selector", "latest-as-of"]),
            args(&["--as-of", "2030-02-01T00:00:00Z"]),
            args(&["--json", "--format", "markdown"]),
        ] {
            let mut invalid = valid.clone();
            invalid.extend(extra);
            assert!(parse(&invalid, Path::new(".")).is_err());
        }
        let mut short = valid;
        short[6] = "2030-02-01".into();
        assert!(parse(&short, Path::new(".")).is_err());
    }
}
