#[cfg(test)]
mod tests {
    use super::*;
    use fmine_io::xlsx::MinimalCell;
    use std::path::PathBuf;

    /// bd-sjrh.1 part 4: the frame binds a state's fields. Two (iv) states that differ only in a reconstruction
    /// check's quoted text bind differently, an absent closing document binds apart from an empty one, and an input
    /// that moves without changing D16's state code still moves its bound fields (q).
    #[test]
    fn test_frame_v4_binds_fields_and_operands() {
        let bytes = |s: &IdentityState| {
            let mut enc = Encoder::new_frame(*b"TEST", 1);
            encode_state(&mut enc, s);
            enc.finish()
        };
        let mismatch = |found: &str| IdentityState::Undecidable {
            why: Undecided::ReconstructionNotApplied {
                evidence_id: "E1".to_string(),
                check: ReconstructionCheck::LabelMismatch {
                    at: "S!A1".to_string(),
                    found: found.to_string(),
                    expected_prefix: "x".to_string(),
                },
            },
            missing_fact: "m".to_string(),
            closing_document: None,
        };
        assert_eq!(bytes(&mismatch("a")), bytes(&mismatch("a")));
        assert_ne!(bytes(&mismatch("a")), bytes(&mismatch("b")));
        // None and Some("") are told apart by their tags.
        let with_doc = |d: Option<&str>| match mismatch("a") {
            IdentityState::Undecidable {
                why, missing_fact, ..
            } => IdentityState::Undecidable {
                why,
                missing_fact,
                closing_document: d.map(str::to_string),
            },
            other => other,
        };
        assert_ne!(bytes(&with_doc(None)), bytes(&with_doc(Some(""))));
        assert_source_fields_affect_frame(bytes);
    }

    // Source registry and historical root integration contract, retaining the private anchor seam.
    include!("public_registered_roots_contract.rs");
}
