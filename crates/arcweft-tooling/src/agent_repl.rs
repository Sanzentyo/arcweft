//! Sans I/O completion helpers for Agent REPL frontends.

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

fn agent_repl_meta_commands() -> Vec<&'static str> {
    vec![
        ":help",
        ":type",
        ":ast",
        ":hir",
        ":bytecode",
        ":observe",
        ":actions",
        ":capture",
        ":query",
        ":history",
        ":bindings",
        ":drop",
        ":load",
        ":save",
        ":parse",
        ":reset",
        ":connect",
        ":complete",
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
        "state",
        "observation",
        "diagnostics",
        "exists",
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
}
