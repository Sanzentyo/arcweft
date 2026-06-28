use arcweft_lang_syntax::parser::{ParseCompletion, ParsedFragment, ParsedFragmentKind};
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
    pub(crate) synthetic_agent_id: String,
    pub(crate) bindings: Vec<ReplBindingRecord>,
}

pub(crate) fn classify_repl_cell(
    id: ReplCellId,
    input: &ReplCellInput,
    live_binding_prelude: &str,
) -> Result<ParsedReplCell, ReplTransactionError> {
    let source = input.source_text().trim().to_owned();
    if let Some(command) = source
        .strip_prefix(':')
        .and_then(|rest| rest.split_whitespace().next())
    {
        return Err(ReplTransactionError::CommandInputDelegated {
            command: format!(":{command}"),
        });
    }
    let fragment = agent_repl_parse_fragment(&source);
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
    let synthetic_agent_id = format!("agent.repl.cell_{}", id.as_u64());
    let synthetic_source = cell_source(
        id,
        &synthetic_agent_id,
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
        synthetic_agent_id,
        bindings,
    })
}

fn ensure_fragment_complete(fragment: &ParsedFragment) -> Result<(), ReplTransactionError> {
    let classification = agent_repl_classification_from_fragment(fragment);
    if matches!(fragment.completion(), ParseCompletion::Complete) && fragment.errors().is_empty() {
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
    id: ReplCellId,
    synthetic_agent_id: &str,
    input: &str,
    fragment: &ParsedFragment,
    live_binding_prelude: &str,
) -> String {
    if matches!(fragment.kind(), Some(ParsedFragmentKind::Items(_))) {
        return input.to_owned();
    }
    let cell_body = if matches!(fragment.kind(), Some(ParsedFragmentKind::Expression(_))) {
        format!("    return {input}")
    } else if input.starts_with("return ") || input.contains("\nreturn ") {
        indent_body(input)
    } else {
        format!("{}\n    return \"ok\"", indent_body(input))
    };
    let body = if live_binding_prelude.trim().is_empty() {
        cell_body
    } else {
        format!("{}\n{}", indent_body(live_binding_prelude), cell_body)
    };
    format!(
        "#[agent(version = 1)]\nagent @{synthetic_agent_id} repl_cell_{}()\neffects {{ agent.observe, agent.act.semantic, agent.act.physical, agent.wait, agent.capture, agent.resource.read, debug.read, debug.record, rag.query }}\n{{\n{body}\n}}\n",
        id.as_u64()
    )
}

fn indent_body(input: &str) -> String {
    input
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
