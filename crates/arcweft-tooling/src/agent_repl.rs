//! Sans I/O helpers for Agent REPL frontends.

use arcweft_lang_syntax::parser::{
    FragmentKind, ParseCompletion, ParseOptions, ParsedFragment, ParsedFragmentKind, parse_fragment,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One project entity that can be offered to an Agent REPL completion frontend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentReplCompletionEntity {
    pub id: String,
    pub kind: String,
}

/// Runtime observation IDs and live REPL state available to completion.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentReplCompletionContext {
    pub entities: Vec<AgentReplCompletionEntity>,
    pub live_bindings: Vec<String>,
    pub action_targets: Vec<String>,
    pub layer_ids: Vec<String>,
    pub object_ids: Vec<String>,
    pub effect_capabilities: Vec<String>,
}

/// Completion item category independent from an editor protocol.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentReplCompletionKind {
    MetaCommand,
    PreludeFunction,
    NamedParameter,
    EntityId,
    LiveBinding,
    ActionTarget,
    LayerId,
    ObjectId,
    EffectCapability,
}

/// One deterministic Agent REPL completion candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentReplCompletionItem {
    pub label: String,
    pub kind: AgentReplCompletionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
}

/// Syntax token class for Agent REPL editor highlighting.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentReplHighlightKind {
    MetaCommand,
    Keyword,
    PreludeFunction,
    Identifier,
    EntityId,
    EffectCapability,
    String,
    Number,
    Boolean,
    Comment,
    Punctuation,
    Operator,
    Whitespace,
    Error,
}

/// One byte-range syntax token independent from a terminal or LSP protocol.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentReplHighlightToken {
    pub start: usize,
    pub end: usize,
    pub kind: AgentReplHighlightKind,
    pub text: String,
}

/// Cell parse state used by terminal editors to decide whether to continue a multiline cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentReplCellCompletionKind {
    Complete,
    Incomplete,
    Invalid,
}

/// Fragment family selected by the shared Agent REPL classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentReplFragmentKind {
    Expression,
    Statements,
    Items,
    Unknown,
}

/// Parse diagnostic shape independent from parser internals and terminal protocols.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentReplParseDiagnostic {
    pub message: String,
    pub expected: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found: Option<String>,
}

/// Shared Agent REPL cell classification report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentReplCellClassification {
    pub completion: AgentReplCellCompletion,
    pub fragment_kind: AgentReplFragmentKind,
    pub errors: Vec<AgentReplParseDiagnostic>,
}

/// Completion state with expected boundary tokens for incomplete cells.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentReplCellCompletion {
    pub kind: AgentReplCellCompletionKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected: Vec<String>,
}

/// Returns deterministic Agent REPL completion items for the source before the cursor.
pub fn agent_repl_completions(
    source_before_cursor: &str,
    context: &AgentReplCompletionContext,
) -> Vec<AgentReplCompletionItem> {
    let source = source_before_cursor.trim_start();
    let candidates = if source.starts_with(':') {
        meta_completion_candidates(source, context)
    } else {
        agent_source_completion_candidates(source, context)
    };
    dedupe_and_sort(candidates, completion_filter_prefix(source))
}

/// Returns lightweight byte-range syntax tokens for Agent REPL editor highlighting.
pub fn agent_repl_highlight_tokens(source: &str) -> Vec<AgentReplHighlightToken> {
    let mut tokens = Vec::new();
    let mut offset = 0;
    while offset < source.len() {
        let Some(ch) = source[offset..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            offset = push_while(
                source,
                offset,
                AgentReplHighlightKind::Whitespace,
                char::is_whitespace,
                &mut tokens,
            );
        } else if ch == '#' {
            push_token(
                source,
                offset,
                source.len(),
                AgentReplHighlightKind::Comment,
                &mut tokens,
            );
            break;
        } else if ch == '"' {
            offset = push_string_token(source, offset, &mut tokens);
        } else if ch == '@' {
            offset = push_prefixed_token(
                source,
                offset,
                AgentReplHighlightKind::EntityId,
                &mut tokens,
            );
        } else if ch == '.'
            && source[offset + ch.len_utf8()..]
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            offset = push_prefixed_token(
                source,
                offset,
                AgentReplHighlightKind::EffectCapability,
                &mut tokens,
            );
        } else if ch == ':' && offset == first_non_ws_offset(source) {
            offset = push_prefixed_token(
                source,
                offset,
                AgentReplHighlightKind::MetaCommand,
                &mut tokens,
            );
        } else if ch.is_ascii_digit() {
            offset = push_while(
                source,
                offset,
                AgentReplHighlightKind::Number,
                |ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'),
                &mut tokens,
            );
        } else if is_identifier_start(ch) {
            offset = push_identifier_token(source, offset, &mut tokens);
        } else if is_operator(ch) {
            let end = offset + ch.len_utf8();
            push_token(
                source,
                offset,
                end,
                AgentReplHighlightKind::Operator,
                &mut tokens,
            );
            offset = end;
        } else {
            let end = offset + ch.len_utf8();
            push_token(
                source,
                offset,
                end,
                AgentReplHighlightKind::Punctuation,
                &mut tokens,
            );
            offset = end;
        }
    }
    tokens
}

/// Parses a REPL cell with the shared Agent fragment parser and syntax-family selection.
pub fn agent_repl_parse_fragment(source: &str) -> ParsedFragment {
    agent_repl_parse_fragment_with_kind(source).0
}

fn agent_repl_parse_fragment_with_kind(source: &str) -> (ParsedFragment, AgentReplFragmentKind) {
    if source.starts_with("let ")
        || source.starts_with("try ")
        || source.starts_with("expect(")
        || source.starts_with("deny(")
        || source.starts_with("wait(")
    {
        return (
            parse_agent_fragment(source, AgentReplFragmentKind::Statements),
            AgentReplFragmentKind::Statements,
        );
    }
    let expression = parse_agent_fragment(source, AgentReplFragmentKind::Expression);
    if matches!(
        expression.completion(),
        ParseCompletion::Complete | ParseCompletion::Incomplete { .. }
    ) {
        return (expression, AgentReplFragmentKind::Expression);
    }
    let items = parse_agent_fragment(source, AgentReplFragmentKind::Items);
    if matches!(items.kind(), Some(ParsedFragmentKind::Items))
        && matches!(items.completion(), ParseCompletion::Complete)
    {
        return (items, AgentReplFragmentKind::Items);
    }
    (
        parse_agent_fragment(source, AgentReplFragmentKind::Statements),
        AgentReplFragmentKind::Statements,
    )
}

fn parse_agent_fragment(source: &str, kind: AgentReplFragmentKind) -> ParsedFragment {
    parse_fragment(
        source,
        match kind {
            AgentReplFragmentKind::Expression => FragmentKind::Expression,
            AgentReplFragmentKind::Statements | AgentReplFragmentKind::Unknown => {
                FragmentKind::Statements
            }
            AgentReplFragmentKind::Items => FragmentKind::Items,
        },
        ParseOptions::default(),
    )
}

/// Classifies one REPL cell for editor completeness validation and scripted inspection.
pub fn agent_repl_classify_cell(source: &str) -> AgentReplCellClassification {
    let (fragment, fragment_kind) = agent_repl_parse_fragment_with_kind(source);
    agent_repl_classification_from_fragment_with_kind(&fragment, fragment_kind)
}

/// Converts a parsed fragment into the stable REPL classification report.
pub fn agent_repl_classification_from_fragment(
    fragment: &ParsedFragment,
) -> AgentReplCellClassification {
    agent_repl_classification_from_fragment_with_kind(fragment, agent_repl_fragment_kind(fragment))
}

fn agent_repl_classification_from_fragment_with_kind(
    fragment: &ParsedFragment,
    fragment_kind: AgentReplFragmentKind,
) -> AgentReplCellClassification {
    let completion = match fragment.completion() {
        ParseCompletion::Complete => AgentReplCellCompletion {
            kind: AgentReplCellCompletionKind::Complete,
            expected: Vec::new(),
        },
        ParseCompletion::Incomplete { expected } => AgentReplCellCompletion {
            kind: AgentReplCellCompletionKind::Incomplete,
            expected: expected
                .iter()
                .map(|token| token.text().to_owned())
                .collect(),
        },
        ParseCompletion::Invalid => AgentReplCellCompletion {
            kind: AgentReplCellCompletionKind::Invalid,
            expected: Vec::new(),
        },
    };
    AgentReplCellClassification {
        completion,
        fragment_kind,
        errors: fragment
            .errors()
            .iter()
            .map(|error| AgentReplParseDiagnostic {
                message: error.message().to_owned(),
                expected: error.expected().to_vec(),
                found: error.found().map(str::to_owned),
            })
            .collect(),
    }
}

fn agent_repl_fragment_kind(fragment: &ParsedFragment) -> AgentReplFragmentKind {
    match fragment.kind() {
        Some(ParsedFragmentKind::Expression) => AgentReplFragmentKind::Expression,
        Some(ParsedFragmentKind::Statements(_)) => AgentReplFragmentKind::Statements,
        Some(ParsedFragmentKind::Items) => AgentReplFragmentKind::Items,
        None => AgentReplFragmentKind::Unknown,
    }
}

fn meta_completion_candidates(
    source: &str,
    context: &AgentReplCompletionContext,
) -> Vec<AgentReplCompletionItem> {
    if source.starts_with(":capture") {
        return capture_completion_candidates(source, context);
    }
    let mut candidates = agent_repl_meta_commands()
        .into_iter()
        .map(|label| completion(label, AgentReplCompletionKind::MetaCommand));
    if source.starts_with(":connect") {
        return vec![
            completion("current", AgentReplCompletionKind::NamedParameter),
            completion("source", AgentReplCompletionKind::NamedParameter),
            completion("profile", AgentReplCompletionKind::NamedParameter),
        ];
    }
    candidates.by_ref().collect()
}

fn capture_completion_candidates(
    source: &str,
    context: &AgentReplCompletionContext,
) -> Vec<AgentReplCompletionItem> {
    let words = source.split_whitespace().collect::<Vec<_>>();
    match words.as_slice() {
        [":capture"] => vec![
            completion("viewport", AgentReplCompletionKind::NamedParameter),
            completion("layer", AgentReplCompletionKind::NamedParameter),
            completion("object", AgentReplCompletionKind::NamedParameter),
        ],
        [":capture", "layer"] => context
            .layer_ids
            .iter()
            .map(|id| completion(id, AgentReplCompletionKind::LayerId))
            .collect(),
        [":capture", "object"] => context
            .object_ids
            .iter()
            .map(|id| completion(id, AgentReplCompletionKind::ObjectId))
            .collect(),
        _ => Vec::new(),
    }
}

fn agent_source_completion_candidates(
    source: &str,
    context: &AgentReplCompletionContext,
) -> Vec<AgentReplCompletionItem> {
    let token = current_prefix(source);
    if token.starts_with('@') {
        return entity_completion_candidates(source, context);
    }
    if token.starts_with('.') {
        return effect_completion_candidates(context);
    }
    let mut candidates = agent_prelude_completion_candidates();
    candidates.extend(named_parameter_candidates(source));
    candidates.extend(
        context
            .live_bindings
            .iter()
            .map(|name| completion(name, AgentReplCompletionKind::LiveBinding)),
    );
    candidates.extend(
        context
            .action_targets
            .iter()
            .map(|target| completion(target, AgentReplCompletionKind::ActionTarget)),
    );
    candidates
}

fn entity_completion_candidates(
    source: &str,
    context: &AgentReplCompletionContext,
) -> Vec<AgentReplCompletionItem> {
    let expected = expected_entity_kind(source);
    context
        .entities
        .iter()
        .filter(|entity| {
            expected
                .as_deref()
                .is_none_or(|expected| entity.kind.eq_ignore_ascii_case(expected))
        })
        .map(|entity| AgentReplCompletionItem {
            label: format!("@{}", entity.id),
            kind: AgentReplCompletionKind::EntityId,
            detail: Some(entity.kind.clone()),
            insert_text: None,
        })
        .collect()
}

fn effect_completion_candidates(
    context: &AgentReplCompletionContext,
) -> Vec<AgentReplCompletionItem> {
    context
        .effect_capabilities
        .iter()
        .map(|name| {
            completion(
                format!(".{name}"),
                AgentReplCompletionKind::EffectCapability,
            )
        })
        .collect()
}

fn expected_entity_kind(source: &str) -> Option<String> {
    let lowered = source.to_ascii_lowercase();
    if lowered.rsplit_once("choose(").is_some() || lowered.rsplit_once("choice_action(").is_some() {
        return Some("choice_option".to_owned());
    }
    if lowered.rsplit_once("signal(").is_some() {
        return Some("signal".to_owned());
    }
    if lowered.rsplit_once("metric(").is_some() {
        return Some("metric".to_owned());
    }
    None
}

fn named_parameter_candidates(source: &str) -> Vec<AgentReplCompletionItem> {
    let lowered = source.to_ascii_lowercase();
    let params: &[&str] = if lowered.contains("wait(") {
        &["timeout", "stable_frames", "poll_frames"]
    } else if lowered.contains("capture(") {
        &["target", "format", "kind", "name"]
    } else if lowered.contains("rag.query(") {
        &["roots", "graph_depth", "limit"]
    } else if lowered.contains("invoke(") {
        &["args"]
    } else if lowered.contains("pointer.click(") {
        &["button"]
    } else if lowered.contains("read_resource(") {
        &["uri"]
    } else {
        &[]
    };
    params
        .iter()
        .map(|param| AgentReplCompletionItem {
            label: (*param).to_owned(),
            kind: AgentReplCompletionKind::NamedParameter,
            detail: Some("named parameter".to_owned()),
            insert_text: Some(format!("{param} = ")),
        })
        .collect()
}

fn agent_prelude_completion_candidates() -> Vec<AgentReplCompletionItem> {
    agent_prelude_functions()
        .into_iter()
        .map(|label| completion(label, AgentReplCompletionKind::PreludeFunction))
        .collect()
}

fn push_identifier_token(
    source: &str,
    start: usize,
    tokens: &mut Vec<AgentReplHighlightToken>,
) -> usize {
    let end = scan_while(source, start, is_identifier_continue);
    let text = &source[start..end];
    let kind = if matches!(text, "true" | "false") {
        AgentReplHighlightKind::Boolean
    } else if agent_keywords().contains(&text) {
        AgentReplHighlightKind::Keyword
    } else if agent_prelude_functions().contains(&text) {
        AgentReplHighlightKind::PreludeFunction
    } else {
        AgentReplHighlightKind::Identifier
    };
    push_token(source, start, end, kind, tokens);
    end
}

fn push_string_token(
    source: &str,
    start: usize,
    tokens: &mut Vec<AgentReplHighlightToken>,
) -> usize {
    let mut escaped = false;
    for (relative, ch) in source[start + 1..].char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            let end = start + 1 + relative + ch.len_utf8();
            push_token(source, start, end, AgentReplHighlightKind::String, tokens);
            return end;
        }
    }
    push_token(
        source,
        start,
        source.len(),
        AgentReplHighlightKind::Error,
        tokens,
    );
    source.len()
}

fn push_prefixed_token(
    source: &str,
    start: usize,
    kind: AgentReplHighlightKind,
    tokens: &mut Vec<AgentReplHighlightToken>,
) -> usize {
    let prefix_len = source[start..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or_default();
    let end = scan_while(source, start + prefix_len, |ch| {
        is_identifier_continue(ch) || matches!(ch, '.' | '-' | '/')
    });
    push_token(source, start, end.max(start + prefix_len), kind, tokens);
    end.max(start + prefix_len)
}

fn push_while(
    source: &str,
    start: usize,
    kind: AgentReplHighlightKind,
    mut predicate: impl FnMut(char) -> bool,
    tokens: &mut Vec<AgentReplHighlightToken>,
) -> usize {
    let end = scan_while(source, start, &mut predicate);
    push_token(source, start, end, kind, tokens);
    end
}

fn push_token(
    source: &str,
    start: usize,
    end: usize,
    kind: AgentReplHighlightKind,
    tokens: &mut Vec<AgentReplHighlightToken>,
) {
    tokens.push(AgentReplHighlightToken {
        start,
        end,
        kind,
        text: source[start..end].to_owned(),
    });
}

fn scan_while(source: &str, start: usize, mut predicate: impl FnMut(char) -> bool) -> usize {
    let mut end = start;
    for ch in source[start..].chars() {
        if !predicate(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    end
}

fn first_non_ws_offset(source: &str) -> usize {
    source
        .char_indices()
        .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some(offset))
        .unwrap_or(source.len())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn is_operator(ch: char) -> bool {
    matches!(
        ch,
        '=' | '!' | '<' | '>' | '+' | '-' | '*' | '/' | '&' | '|'
    )
}

fn agent_keywords() -> Vec<&'static str> {
    vec![
        "effects", "let", "mut", "return", "try", "if", "else", "match", "for", "in", "while",
        "await", "break", "continue",
    ]
}

fn agent_repl_meta_commands() -> Vec<&'static str> {
    vec![
        ":help",
        ":type",
        ":ast",
        ":hir",
        ":bytecode",
        ":observe",
        ":actions",
        ":trace",
        ":capture",
        ":query",
        ":history",
        ":bindings",
        ":drop",
        ":load",
        ":save",
        ":parse",
        ":classify",
        ":reset",
        ":connect",
        ":complete",
        ":highlight",
        ":quit",
    ]
}

fn agent_prelude_functions() -> Vec<&'static str> {
    vec![
        "observe",
        "wait",
        "choose",
        "choice_action",
        "advance_text",
        "invoke",
        "capture",
        "read_resource",
        "attach",
        "checkpoint",
        "note",
        "expect",
        "deny",
        "signal",
        "metric",
        "state_path",
        "observation_path",
        "state",
        "observation",
        "diagnostics",
        "exists",
        "action_enabled",
        "all",
        "any",
        "not",
        "viewport_point",
        "pointer.click",
        "rag.query",
    ]
}

fn completion(label: impl Into<String>, kind: AgentReplCompletionKind) -> AgentReplCompletionItem {
    AgentReplCompletionItem {
        label: label.into(),
        kind,
        detail: None,
        insert_text: None,
    }
}

fn dedupe_and_sort(
    candidates: Vec<AgentReplCompletionItem>,
    prefix: &str,
) -> Vec<AgentReplCompletionItem> {
    let mut seen = BTreeSet::new();
    let mut items = candidates
        .into_iter()
        .filter(|item| item.label.starts_with(prefix))
        .filter(|item| seen.insert((item.kind, item.label.clone())))
        .collect::<Vec<_>>();
    items.sort_by(|lhs, rhs| {
        lhs.label
            .cmp(&rhs.label)
            .then_with(|| format!("{:?}", lhs.kind).cmp(&format!("{:?}", rhs.kind)))
    });
    items
}

fn current_prefix(source: &str) -> &str {
    source
        .rsplit(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ')' | '[' | ']' | ',' | '='))
        .next()
        .unwrap_or_default()
}

fn completion_filter_prefix(source: &str) -> &str {
    let words = source.split_whitespace().collect::<Vec<_>>();
    match words.as_slice() {
        [":capture", "layer" | "object"] | [":connect"] => "",
        [":capture", "layer" | "object", prefix] | [":connect", prefix] => prefix,
        _ => current_prefix(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repl_completion_filters_choice_refs_for_choose_call() {
        let items = agent_repl_completions(
            "choose(@",
            &AgentReplCompletionContext {
                entities: vec![
                    AgentReplCompletionEntity {
                        id: "choice.opening.listen".to_owned(),
                        kind: "choice_option".to_owned(),
                    },
                    AgentReplCompletionEntity {
                        id: "flow.opening".to_owned(),
                        kind: "flow".to_owned(),
                    },
                ],
                ..AgentReplCompletionContext::default()
            },
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "@choice.opening.listen");
    }

    #[test]
    fn repl_completion_uses_observed_layer_and_object_ids_for_capture_meta() {
        let context = AgentReplCompletionContext {
            layer_ids: vec!["dialogue.rich_text".to_owned()],
            object_ids: vec!["object.dialogue.0.0".to_owned()],
            ..AgentReplCompletionContext::default()
        };

        let layers = agent_repl_completions(":capture layer ", &context);
        let objects = agent_repl_completions(":capture object ", &context);

        assert_eq!(layers[0].label, "dialogue.rich_text");
        assert_eq!(layers[0].kind, AgentReplCompletionKind::LayerId);
        assert_eq!(objects[0].label, "object.dialogue.0.0");
        assert_eq!(objects[0].kind, AgentReplCompletionKind::ObjectId);
    }

    #[test]
    fn repl_completion_exposes_current_agent_prelude_intrinsics() {
        let state = agent_repl_completions("state_", &AgentReplCompletionContext::default());
        let observation =
            agent_repl_completions("observation_", &AgentReplCompletionContext::default());
        let action = agent_repl_completions("action_", &AgentReplCompletionContext::default());

        assert!(state.iter().any(|item| item.label == "state_path"
            && item.kind == AgentReplCompletionKind::PreludeFunction));
        assert!(
            observation
                .iter()
                .any(|item| item.label == "observation_path"
                    && item.kind == AgentReplCompletionKind::PreludeFunction)
        );
        assert!(action.iter().any(|item| item.label == "action_enabled"
            && item.kind == AgentReplCompletionKind::PreludeFunction));
    }

    #[test]
    fn repl_completion_exposes_read_resource_uri_parameter() {
        let items =
            agent_repl_completions("read_resource(", &AgentReplCompletionContext::default());

        assert!(items.iter().any(|item| item.label == "uri"
            && item.kind == AgentReplCompletionKind::NamedParameter
            && item.insert_text.as_deref() == Some("uri = ")));
    }

    #[test]
    fn repl_highlight_tokens_classify_agent_fragment_surface() {
        let tokens = agent_repl_highlight_tokens("let frame = try observe(@flow.opening)");

        assert!(
            tokens.iter().any(|token| {
                token.kind == AgentReplHighlightKind::Keyword && token.text == "let"
            })
        );
        assert!(tokens.iter().any(|token| {
            token.kind == AgentReplHighlightKind::PreludeFunction && token.text == "observe"
        }));
        assert!(tokens.iter().any(|token| {
            token.kind == AgentReplHighlightKind::EntityId && token.text == "@flow.opening"
        }));
    }

    #[test]
    fn repl_highlight_tokens_classify_meta_command_and_unclosed_string() {
        let meta = agent_repl_highlight_tokens(":capture layer");
        let broken = agent_repl_highlight_tokens("note(\"unterminated");

        assert_eq!(meta[0].kind, AgentReplHighlightKind::MetaCommand);
        assert_eq!(meta[0].text, ":capture");
        assert!(
            broken
                .iter()
                .any(|token| token.kind == AgentReplHighlightKind::Error)
        );
    }

    #[test]
    fn repl_classify_cell_uses_shared_agent_fragment_parser() {
        let expression = agent_repl_classify_cell("signal(@signal.ready)");
        let statement = agent_repl_classify_cell("let frame = try observe()");

        assert_eq!(
            expression.completion.kind,
            AgentReplCellCompletionKind::Complete
        );
        assert_eq!(expression.fragment_kind, AgentReplFragmentKind::Expression);
        assert_eq!(
            statement.completion.kind,
            AgentReplCellCompletionKind::Complete
        );
        assert_eq!(statement.fragment_kind, AgentReplFragmentKind::Statements);
    }

    #[test]
    fn repl_classify_cell_recognizes_ordinary_function_items() {
        let classified = agent_repl_classify_cell("fn helper(value: i64) -> i64 { value + 1 }");

        assert_eq!(
            classified.completion.kind,
            AgentReplCellCompletionKind::Complete
        );
        assert_eq!(classified.fragment_kind, AgentReplFragmentKind::Items);
        assert!(classified.errors.is_empty());
    }

    #[test]
    fn repl_classify_cell_reports_incomplete_string_boundary() {
        let classified = agent_repl_classify_cell("note(\"unterminated");

        assert_eq!(
            classified.completion.kind,
            AgentReplCellCompletionKind::Incomplete
        );
        assert_eq!(classified.completion.expected, ["\""]);
    }

    #[test]
    fn repl_classify_cell_reports_incomplete_expression_boundaries() {
        for source in [
            "let value =",
            "return",
            "try",
            "wait(",
            "any([signal(@signal.ready),",
            "signal(@signal.ready).",
            "metric(@metric.score) >",
            "state(\"route.phase\").eq(",
            "try observe() with { error e =>",
        ] {
            let classified = agent_repl_classify_cell(source);

            assert_eq!(
                classified.completion.kind,
                AgentReplCellCompletionKind::Incomplete,
                "{source} should be incomplete"
            );
            assert_eq!(classified.completion.expected, ["expression"]);
        }
    }

    #[test]
    fn repl_classify_cell_reports_incomplete_block_introducers() {
        for source in [
            "try observe() with",
            "if diagnostics().has_error() { return \"bad\" } else",
        ] {
            let classified = agent_repl_classify_cell(source);

            assert_eq!(
                classified.completion.kind,
                AgentReplCellCompletionKind::Incomplete,
                "{source} should be incomplete"
            );
            assert_eq!(classified.completion.expected, ["{"]);
        }
    }
}
