use std::{fmt::Write as _, sync::Arc};

use arcweft_lang_syntax::{
    incremental::{ParsedSource, SyntaxDatabase},
    parser::{ParseCompletion, ParseOptions, parse_expression_fragment},
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};

use crate::cell::{ReplCellId, ReplCellInput, ReplCellKind};
use crate::error::{ReplParseCoordinateSpace, ReplTransactionError, ReplTransactionPhase};
use crate::hash::stable_hash;

pub(crate) struct ParsedReplCell {
    pub(crate) id: ReplCellId,
    pub(crate) kind: ReplCellKind,
    pub(crate) source: String,
    pub(crate) source_hash: String,
    pub(crate) synthetic_source_hash: String,
    pub(crate) synthetic_entry_id: String,
    pub(crate) synthetic_controller_name: String,
    pub(crate) cell_source_range: SourceRange,
    pub(crate) parsed_source: ParsedSource,
}

struct ParsedCandidate {
    kind: ReplCellKind,
    synthetic: SyntheticCellSource,
    parsed: ParsedSource,
}

struct SyntheticCellSource {
    text: String,
    cell_range: SourceRange,
}

pub(crate) fn classify_repl_cell(
    id: ReplCellId,
    input: &ReplCellInput,
    live_binding_prelude: &str,
) -> Result<ParsedReplCell, ReplTransactionError> {
    let source = input.source_text().to_owned();
    if let Some(command) = source
        .trim()
        .strip_prefix(':')
        .and_then(|rest| rest.split_whitespace().next())
    {
        return Err(ReplTransactionError::CommandInputDelegated {
            command: format!(":{command}"),
        });
    }

    let synthetic_entry_id = format!("entry.agent.repl.cell_{}", id.as_u64());
    let synthetic_controller_name = format!("repl_cell_{}", id.as_u64());
    let candidate = match input.expected_kind() {
        Some(ReplCellKind::Expression) => {
            require_complete_expression(&source)?;
            parse_candidate(
                id,
                ReplCellKind::Expression,
                &source,
                live_binding_prelude,
                &synthetic_entry_id,
                &synthetic_controller_name,
            )?
        }
        Some(kind @ (ReplCellKind::Statement | ReplCellKind::Item)) => parse_candidate(
            id,
            kind,
            &source,
            live_binding_prelude,
            &synthetic_entry_id,
            &synthetic_controller_name,
        )?,
        None => classify_candidate(
            id,
            &source,
            live_binding_prelude,
            &synthetic_entry_id,
            &synthetic_controller_name,
        )?,
    };
    reject_recovered_candidate(&candidate)?;

    let synthetic_source_hash = stable_hash(
        "repl.cell.synthetic-source",
        candidate.synthetic.text.as_bytes(),
    );
    Ok(ParsedReplCell {
        id,
        kind: candidate.kind,
        source_hash: stable_hash("repl.cell.source", source.as_bytes()),
        synthetic_source_hash,
        source,
        synthetic_entry_id,
        synthetic_controller_name,
        cell_source_range: candidate.synthetic.cell_range,
        parsed_source: candidate.parsed,
    })
}

fn classify_candidate(
    id: ReplCellId,
    source: &str,
    live_binding_prelude: &str,
    synthetic_entry_id: &str,
    synthetic_controller_name: &str,
) -> Result<ParsedCandidate, ReplTransactionError> {
    let expression = parse_expression_fragment(source, ParseOptions::default());
    match expression.completion() {
        ParseCompletion::Complete => {
            return parse_candidate(
                id,
                ReplCellKind::Expression,
                source,
                live_binding_prelude,
                synthetic_entry_id,
                synthetic_controller_name,
            );
        }
        ParseCompletion::Incomplete { expected } => {
            return Err(ReplTransactionError::IncompleteOrInvalid {
                message: format!(
                    "expression is incomplete; expected {}",
                    expected
                        .iter()
                        .map(arcweft_lang_syntax::parser::ExpectedToken::text)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        ParseCompletion::Invalid => {}
    }

    let statement = parse_candidate(
        id,
        ReplCellKind::Statement,
        source,
        live_binding_prelude,
        synthetic_entry_id,
        synthetic_controller_name,
    )?;
    if statement.parsed.diagnostics().is_empty() {
        return Ok(statement);
    }
    let item = parse_candidate(
        id,
        ReplCellKind::Item,
        source,
        live_binding_prelude,
        synthetic_entry_id,
        synthetic_controller_name,
    )?;
    if item.parsed.diagnostics().is_empty() {
        return Ok(item);
    }
    if item.parsed.diagnostics().len() < statement.parsed.diagnostics().len() {
        Ok(item)
    } else {
        Ok(statement)
    }
}

fn require_complete_expression(source: &str) -> Result<(), ReplTransactionError> {
    let fragment = parse_expression_fragment(source, ParseOptions::default());
    match fragment.completion() {
        ParseCompletion::Complete if fragment.diagnostics().is_empty() => Ok(()),
        completion => Err(ReplTransactionError::IncompleteOrInvalid {
            message: if fragment.diagnostics().is_empty() {
                format!("expression fragment is {completion:?}")
            } else {
                fragment
                    .diagnostics()
                    .iter()
                    .map(arcweft_lang_syntax::parser::FragmentDiagnostic::message)
                    .collect::<Vec<_>>()
                    .join("; ")
            },
        }),
    }
}

fn parse_candidate(
    id: ReplCellId,
    kind: ReplCellKind,
    source: &str,
    live_binding_prelude: &str,
    synthetic_entry_id: &str,
    synthetic_controller_name: &str,
) -> Result<ParsedCandidate, ReplTransactionError> {
    let synthetic = cell_source(
        kind,
        synthetic_entry_id,
        synthetic_controller_name,
        source,
        live_binding_prelude,
    );
    let kind_label = match kind {
        ReplCellKind::Item => "item",
        ReplCellKind::Statement => "statement",
        ReplCellKind::Expression => "expression",
    };
    let source_hash = stable_hash("repl.synthetic.document", synthetic.text.as_bytes());
    let source_name = SourceName::path(format!("repl/{kind_label}/cell-{}.arcw", id.as_u64()));
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!(
                "arcweft-repl://cell/{}/{kind_label}/{source_hash}",
                id.as_u64()
            ))
            .map_err(|error| classify_error(&error))?,
            source_name.clone(),
            synthetic.text.clone(),
        )
        .map_err(|error| classify_error(&error))?,
    );
    let mut syntax = SyntaxDatabase::try_new().map_err(|error| classify_error(&error))?;
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(source_name),
            Arc::clone(&document),
            ParseOptions::default(),
        )
        .map_err(|error| classify_error(&error))?;
    Ok(ParsedCandidate {
        kind,
        synthetic,
        parsed,
    })
}

fn reject_recovered_candidate(candidate: &ParsedCandidate) -> Result<(), ReplTransactionError> {
    if candidate.parsed.diagnostics().is_empty() {
        return Ok(());
    }
    Err(ReplTransactionError::AttachedParse {
        diagnostics: candidate.parsed.diagnostics().to_vec(),
        coordinate_space: ReplParseCoordinateSpace::SyntheticSourceUtf8Bytes,
    })
}

fn classify_error(error: &impl ToString) -> ReplTransactionError {
    ReplTransactionError::Compile {
        phase: ReplTransactionPhase::ClassifyParse,
        message: error.to_string(),
    }
}

fn cell_source(
    kind: ReplCellKind,
    synthetic_entry_id: &str,
    synthetic_controller_name: &str,
    input: &str,
    live_binding_prelude: &str,
) -> SyntheticCellSource {
    let mut text = String::new();
    let cell_range = if kind == ReplCellKind::Item {
        let start = text.len();
        text.push_str(input);
        let end = text.len();
        text.push_str("\n\n");
        SourceRange::new(start, end)
    } else {
        SourceRange::new(0, 0)
    };
    write!(
        text,
        "fn {synthetic_controller_name}() -> Result<Unit, AgentError>\neffects {{ agent.observe, agent.act.semantic, agent.act.physical, agent.wait, agent.capture, agent.resource.read, debug.read, debug.record, rag.query }}\n{{\n"
    )
    .expect("writing to String cannot fail");
    if !live_binding_prelude.trim().is_empty() {
        text.push_str(&indent_body(live_binding_prelude));
        text.push('\n');
    }
    let cell_range = if kind == ReplCellKind::Item {
        text.push_str("    Ok(())");
        cell_range
    } else {
        let start = text.len();
        text.push_str(&indent_body(input));
        let end = text.len();
        if !(input.starts_with("return ") || input.contains("\nreturn ")) {
            text.push_str("\n    Ok(())");
        }
        SourceRange::new(start, end)
    };
    write!(
        text,
        "\n}}\n\nentry agent @{synthetic_entry_id} {{\n    controller = {synthetic_controller_name}\n}}\n"
    )
    .expect("writing to String cannot fail");
    SyntheticCellSource { text, cell_range }
}

fn indent_body(input: &str) -> String {
    input
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use crate::cell::{ReplCellId, ReplCellInput, ReplCellKind};
    use crate::error::{ReplParseCoordinateSpace, ReplTransactionError};

    use super::classify_repl_cell;

    #[test]
    fn accepted_cell_retains_the_exact_synthetic_parse_lease() {
        let cell = classify_repl_cell(
            ReplCellId::new(1),
            &ReplCellInput::statement("let observed = try observe(@flow.opening)"),
            "",
        )
        .expect("statement candidate parses once");
        assert_eq!(cell.kind, ReplCellKind::Statement);
        assert_eq!(
            cell.parsed_source.document().text(),
            cell.parsed_source.source()
        );
        assert_eq!(
            cell.parsed_source.source_snapshot_id().name(),
            cell.parsed_source.document().display_name()
        );
    }

    #[test]
    fn ordinary_function_item_uses_a_whole_synthetic_document() {
        let cell = classify_repl_cell(
            ReplCellId::new(1),
            &ReplCellInput::item("fn helper(value: i64) -> i64 { value + 1 }"),
            "",
        )
        .expect("ordinary item parses as a whole document");
        assert_eq!(cell.kind, ReplCellKind::Item);
        assert_eq!(
            &cell.parsed_source.source()[cell.cell_source_range.as_range()],
            "fn helper(value: i64) -> i64 { value + 1 }"
        );
    }

    #[test]
    fn typed_item_failure_keeps_synthetic_source_coordinates() {
        let source = "\n  pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n  ";
        let Err(error) = classify_repl_cell(ReplCellId::new(1), &ReplCellInput::item(source), "")
        else {
            panic!("malformed View export must fail classification");
        };
        let ReplTransactionError::AttachedParse {
            diagnostics,
            coordinate_space,
        } = error
        else {
            panic!("classification must retain attached parser diagnostics");
        };
        assert_eq!(
            coordinate_space,
            ReplParseCoordinateSpace::SyntheticSourceUtf8Bytes
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code() == "view::export_part_missing_as" })
        );
    }
}
