use arcweft_lang_syntax::parser::{
    FragmentKind, ParseCompletion, ParseOptions, ParsedFragment, ParsedFragmentKind, parse_fragment,
};
use arcweft_tooling::agent_repl::{
    agent_repl_classification_from_fragment, agent_repl_parse_fragment,
};

use crate::binding::{ReplBindingRecord, committed_bindings};
use crate::cell::{ReplCellId, ReplCellInput, ReplCellKind};
use crate::error::ReplTransactionError;
use crate::hash::stable_hash;

pub(crate) struct ParsedReplCell {
    pub(crate) kind: ReplCellKind,
    pub(crate) source: String,
    pub(crate) source_hash: String,
    pub(crate) synthetic_source: String,
    pub(crate) synthetic_source_hash: String,
    pub(crate) synthetic_entry_id: String,
    pub(crate) synthetic_controller_name: String,
    pub(crate) bindings: Vec<ReplBindingRecord>,
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
    let fragment = input.expected_kind().map_or_else(
        || agent_repl_parse_fragment(&source),
        |kind| {
            parse_fragment(
                &source,
                match kind {
                    ReplCellKind::Item => FragmentKind::Items,
                    ReplCellKind::Statement => FragmentKind::Statements,
                    ReplCellKind::Expression => FragmentKind::Expression,
                },
                ParseOptions::default(),
            )
        },
    );
    ensure_fragment_complete(&fragment)?;
    let kind = repl_cell_kind(&fragment)?;
    if let Some(expected) = input.expected_kind()
        && expected != kind
    {
        return Err(ReplTransactionError::UnexpectedCellKind {
            expected,
            actual: kind,
        });
    }
    let synthetic_entry_id = format!("entry.agent.repl.cell_{}", id.as_u64());
    let synthetic_controller_name = format!("repl_cell_{}", id.as_u64());
    let synthetic_source = cell_source(
        &synthetic_entry_id,
        &synthetic_controller_name,
        &source,
        &fragment,
        live_binding_prelude,
    );
    let bindings = committed_bindings(id, &fragment);
    Ok(ParsedReplCell {
        kind,
        source_hash: stable_hash("repl.cell.source", source.as_bytes()),
        synthetic_source_hash: stable_hash(
            "repl.cell.synthetic-source",
            synthetic_source.as_bytes(),
        ),
        source,
        synthetic_source,
        synthetic_entry_id,
        synthetic_controller_name,
        bindings,
    })
}

fn ensure_fragment_complete(fragment: &ParsedFragment) -> Result<(), ReplTransactionError> {
    if !fragment.errors().is_empty() {
        return Err(ReplTransactionError::Parse {
            diagnostics: fragment.errors().to_vec(),
            coordinate_space: crate::error::ReplParseCoordinateSpace::CellSourceUtf8Bytes,
        });
    }
    let classification = agent_repl_classification_from_fragment(fragment);
    if matches!(fragment.completion(), ParseCompletion::Complete) {
        return Ok(());
    }
    let message = if classification.errors.is_empty() {
        "cell completion is not complete".to_owned()
    } else {
        classification
            .errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ")
    };
    Err(ReplTransactionError::IncompleteOrInvalid { message })
}

fn repl_cell_kind(fragment: &ParsedFragment) -> Result<ReplCellKind, ReplTransactionError> {
    match fragment.kind() {
        Some(ParsedFragmentKind::Items(_)) => Ok(ReplCellKind::Item),
        Some(ParsedFragmentKind::Statements(_)) => Ok(ReplCellKind::Statement),
        Some(ParsedFragmentKind::Expression(_)) => Ok(ReplCellKind::Expression),
        None => Err(ReplTransactionError::IncompleteOrInvalid {
            message: "fragment did not produce a parsed REPL cell family".to_owned(),
        }),
    }
}

fn cell_source(
    synthetic_entry_id: &str,
    synthetic_controller_name: &str,
    input: &str,
    fragment: &ParsedFragment,
    live_binding_prelude: &str,
) -> String {
    let item_prefix = matches!(fragment.kind(), Some(ParsedFragmentKind::Items(_)))
        .then(|| format!("{input}\n\n"))
        .unwrap_or_default();
    let cell_body = if matches!(fragment.kind(), Some(ParsedFragmentKind::Items(_))) {
        "    Ok(())".to_owned()
    } else if input.starts_with("return ") || input.contains("\nreturn ") {
        indent_body(input)
    } else {
        format!("{}\n    Ok(())", indent_body(input))
    };
    let body = if live_binding_prelude.trim().is_empty() {
        cell_body
    } else {
        format!("{}\n{}", indent_body(live_binding_prelude), cell_body)
    };
    format!(
        "{item_prefix}fn {synthetic_controller_name}() -> Result<Unit, AgentError>\neffects {{ agent.observe, agent.act.semantic, agent.act.physical, agent.wait, agent.capture, agent.resource.read, debug.read, debug.record, rag.query }}\n{{\n{body}\n}}\n\nentry agent @{synthetic_entry_id} {{\n    controller = {synthetic_controller_name}\n}}\n"
    )
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
    use crate::cell::{ReplCellId, ReplCellInput};
    use crate::error::{ReplParseCoordinateSpace, ReplTransactionError};
    use arcweft_lang_syntax::parser::recovery::ParseErrorKind;

    use super::classify_repl_cell;

    #[test]
    fn repl_item_consumer_rejects_removed_role_declarations_before_synthesis() {
        for source in [
            "state GameState {\n    value: i32\n}\n",
            "reducer update(state: GameState, event: GameEvent) -> GameState {\n    state\n}\n",
            "agent @agent.smoke smoke() {\n    Ok(())\n}\n",
        ] {
            let Err(error) =
                classify_repl_cell(ReplCellId::new(1), &ReplCellInput::item(source), "")
            else {
                panic!("removed declaration must fail the REPL item consumer: {source}");
            };
            assert_eq!(
                error.phase(),
                crate::error::ReplTransactionPhase::ClassifyParse
            );
        }
    }

    #[test]
    fn typed_item_parse_failure_keeps_cell_source_coordinates() {
        let source = "\n  pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n  ";
        let Err(error) = classify_repl_cell(ReplCellId::new(1), &ReplCellInput::item(source), "")
        else {
            panic!("malformed View export must fail classification");
        };
        let ReplTransactionError::Parse {
            diagnostics,
            coordinate_space,
        } = error
        else {
            panic!("classification must retain the typed parser payload");
        };
        assert_eq!(
            coordinate_space,
            ReplParseCoordinateSpace::CellSourceUtf8Bytes
        );
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.kind() == ParseErrorKind::ViewExportPartMissingAs)
            .expect("missing-as parser diagnostic");
        let heading = source.find("heading").expect("heading source");
        assert_eq!(
            diagnostic.range().as_range(),
            heading..heading + "heading".len()
        );
        assert_eq!(&source[diagnostic.range().as_range()], "heading");
    }
}
