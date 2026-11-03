//! DESIGN (BD-229, §22.5): source-bound weighted bootstrap configuration.
//! Determinism: Exact (declared integer controls and original source anchors).

use fmine_registry::toml_subset::TomlRow;

use super::*;
use crate::stability::{METHOD, SelectionStabilityDesign};

fn integer<T: std::str::FromStr>(fields: &Fields<'_>, name: &str) -> Result<T> {
    let (value, line) = fields
        .get(name)
        .ok_or_else(|| input(1, format!("missing {name}")))?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(input(
            *line,
            format!("{name} must be a quoted unsigned decimal integer"),
        ));
    }
    value
        .parse()
        .map_err(|_| input(*line, format!("{name} exceeds its integer range")))
}

pub(super) fn declaration(row: &TomlRow, model_key: &str) -> Result<SelectionStabilityDesign> {
    let fields = fields(
        &row.fields,
        &[
            "method",
            "seed",
            "replicates",
            "draws_per_replicate",
            "sampling_basis",
        ],
        &[],
        row.line,
    )?;
    if get(&fields, "method")? != METHOD {
        return Err(input(
            row.line,
            "selection stability requires method = weighted_multinomial",
        ));
    }
    let replicates: usize = integer(&fields, "replicates")?;
    let draws_per_replicate: usize = integer(&fields, "draws_per_replicate")?;
    if !(200..=2000).contains(&replicates) || !(1..=4096).contains(&draws_per_replicate) {
        return Err(input(
            row.line,
            "selection stability requires 200..=2000 replicates and 1..=4096 draws per replicate",
        ));
    }
    Ok(SelectionStabilityDesign {
        model_key: model_key.into(),
        method: METHOD.into(),
        seed: integer(&fields, "seed")?,
        replicates,
        draws_per_replicate,
        sampling_basis: get(&fields, "sampling_basis")?.into(),
    })
}

pub(super) fn finish(
    row: Option<(SelectionStabilityDesign, usize)>,
    source: &SourceAnchor,
    top: &[TomlField],
    unknowns: &[UuTerm],
    uu_anchors: &[SourceAnchor],
) -> Option<Claim<SelectionStabilityDesign>> {
    row.map(|(declaration, line)| {
        let mut anchors = vec![located(
            source,
            line,
            "selection stability method, seed, replicate count, draw count and basis",
        )];
        anchors.extend(
            top.iter()
                .filter(|field| field.key == "model_key")
                .map(|field| located(source, field.line, &field.key)),
        );
        anchors.extend(uu_anchors.iter().cloned());
        let tag = declaration.canonical_tag();
        assumed(
            "capital.selection-stability/1".into(),
            declaration,
            &tag,
            AdditivityClass::NonAdditive,
            unknowns,
            anchors,
        )
    })
}
