//! Stateful expansion of authored Fx spans within one dialogue line.

use std::collections::BTreeMap;

use arcweft_dialogue::InlineFailurePolicy;
use arcweft_lang_hir::syntax::ast::dialogue::{DialogueTagKind, DialogueToken};
use arcweft_presentation::rich_text::canonical_tag_name;
use arcweft_render_text::{RichTextControl, RichTextNode, RichTextStyle};

use crate::errors::RuntimePlanLowerError;
use crate::render_text::{
    defaults::TextProxyTypeDefaults,
    tag::{inferred_text_proxy_type, lower_dialogue_token_parts},
};

use super::{FxCatalog, FxInlineAssignment, builtins::builtin_selector, fx_error};

/// Expands one authored `[fx ...]` span atomically into ordinary style nodes.
pub(crate) struct DialogueFxExpander<'catalog> {
    catalog: &'catalog FxCatalog,
    open_spans: Vec<OpenSpan>,
    inline_assignments: Vec<FxInlineAssignment>,
    next_fx_ordinal: u32,
}

#[derive(Clone, Debug)]
enum OpenSpan {
    Style {
        name: String,
    },
    Fx {
        name: String,
        close: FxClose,
        ends: Vec<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FxClose {
    Explicit(String),
    Inferred,
}

impl<'catalog> DialogueFxExpander<'catalog> {
    pub(crate) fn new(catalog: &'catalog FxCatalog) -> Self {
        Self {
            catalog,
            open_spans: Vec::new(),
            inline_assignments: Vec::new(),
            next_fx_ordinal: 0,
        }
    }

    pub(crate) fn lower_token(
        &mut self,
        token: &DialogueToken,
        default_inline_failure_policy: Option<&InlineFailurePolicy>,
        text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
    ) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        let inferred_proxy = matches!(
            token,
            DialogueToken::InferredTag(tag)
                if inferred_text_proxy_type(
                    tag.name().trim_start_matches('.'),
                    tag.attrs(),
                    text_proxies,
                )
        );
        if !inferred_proxy && let Some((selector, attrs)) = builtin_selector(token) {
            let (DialogueToken::Tag(tag) | DialogueToken::InferredTag(tag)) = token else {
                unreachable!("builtin selector only classifies opener tags");
            };
            let ordinal = self.next_fx_ordinal;
            if let Some((name, application, definition)) =
                self.catalog.bind_builtin(selector, attrs, tag, ordinal)?
            {
                self.next_fx_ordinal = self
                    .next_fx_ordinal
                    .checked_add(1)
                    .ok_or_else(|| fx_error("too many Fx applications in one dialogue line"))?;
                if let Some(definition) = definition {
                    self.inline_assignments.push(FxInlineAssignment::new(
                        &definition,
                        &application,
                        tag.attrs_range(),
                    ));
                }
                self.open_spans.push(OpenSpan::Fx {
                    name,
                    close: match token {
                        DialogueToken::InferredTag(_) => FxClose::Inferred,
                        DialogueToken::Tag(tag) => FxClose::Explicit(tag.name().to_owned()),
                        _ => unreachable!("builtin selector only classifies opener tags"),
                    },
                    ends: vec!["fx".to_owned()],
                });
                return Ok(vec![RichTextNode::StyleStart {
                    style: RichTextStyle::Fx { application },
                }]);
            }
        }
        match token {
            DialogueToken::Tag(tag) if tag.kind() == DialogueTagKind::Fx => {
                let ordinal = self.next_fx_ordinal;
                self.next_fx_ordinal = self
                    .next_fx_ordinal
                    .checked_add(1)
                    .ok_or_else(|| fx_error("too many Fx applications in one dialogue line"))?;
                let (name, application) = self.catalog.bind_tag(tag, ordinal)?;
                let definition = self
                    .catalog
                    .definitions
                    .get(&name)
                    .ok_or_else(|| fx_error(format!("unknown Fx function `{name}`")))?;
                self.inline_assignments.push(FxInlineAssignment::new(
                    definition,
                    &application,
                    tag.attrs_range(),
                ));
                self.open_spans.push(OpenSpan::Fx {
                    name,
                    close: FxClose::Explicit("fx".to_owned()),
                    ends: vec!["fx".to_owned()],
                });
                Ok(vec![RichTextNode::StyleStart {
                    style: RichTextStyle::Fx { application },
                }])
            }
            DialogueToken::EndTag(end) if self.top_fx_closes_explicitly(end.name()) => {
                self.close_fx()
            }
            DialogueToken::InferredEndTag if self.top_fx_closes_inferred() => self.close_fx(),
            DialogueToken::EndTag(end) if end.kind() == DialogueTagKind::Fx => self.close_fx(),
            DialogueToken::InferredEndTag if self.has_open_fx() => {
                self.close_inferred_inside_fx(token, default_inline_failure_policy, text_proxies)
            }
            DialogueToken::Tag(tag)
                if tag.kind() == DialogueTagKind::Reset && self.has_open_fx() =>
            {
                Err(fx_error(
                    "`[reset]` cannot clear styles from inside an open Fx span",
                ))
            }
            DialogueToken::EndTag(end) if self.has_open_fx() => self.close_nested_style(
                end.name(),
                token,
                default_inline_failure_policy,
                text_proxies,
            ),
            _ => {
                let nodes =
                    lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies)?;
                self.track_open_styles(&nodes);
                self.track_permissive_close(&nodes);
                Ok(nodes)
            }
        }
    }

    pub(crate) fn finish(self) -> Result<Vec<FxInlineAssignment>, RuntimePlanLowerError> {
        let unclosed = self
            .open_spans
            .iter()
            .filter_map(|span| match span {
                OpenSpan::Fx { name, .. } => Some(name.as_str()),
                OpenSpan::Style { .. } => None,
            })
            .collect::<Vec<_>>();
        if unclosed.is_empty() {
            Ok(self.inline_assignments)
        } else {
            Err(fx_error(format!(
                "unclosed RichText Fx span(s): {}",
                unclosed.join(", ")
            )))
        }
    }

    fn close_fx(&mut self) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        match self.open_spans.pop() {
            Some(OpenSpan::Fx { ends, .. }) => Ok(ends
                .into_iter()
                .map(|name| RichTextNode::StyleEnd { name })
                .collect()),
            Some(span @ OpenSpan::Style { .. }) => {
                self.open_spans.push(span);
                Err(fx_error(
                    "`[/fx]` crosses a RichText span opened inside the Fx span",
                ))
            }
            None => Err(fx_error("`[/fx]` has no matching `[fx ...]` opener")),
        }
    }

    fn close_nested_style(
        &mut self,
        name: &str,
        token: &DialogueToken,
        default_inline_failure_policy: Option<&InlineFailurePolicy>,
        text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
    ) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        let canonical = canonical_tag_name(name);
        match self.open_spans.last() {
            Some(OpenSpan::Style { name }) if name == canonical => {
                self.open_spans.pop();
                lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies)
            }
            Some(OpenSpan::Fx { .. }) => Err(fx_error(format!(
                "`[/{name}]` cannot close an internal Fx layer; close `[/fx]`"
            ))),
            Some(OpenSpan::Style { name: open }) => Err(fx_error(format!(
                "mismatched RichText close `[/{name}]` inside Fx span; expected `[/{open}]`"
            ))),
            None => lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies),
        }
    }

    fn close_inferred_inside_fx(
        &mut self,
        token: &DialogueToken,
        default_inline_failure_policy: Option<&InlineFailurePolicy>,
        text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
    ) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        match self.open_spans.last() {
            Some(OpenSpan::Style { .. }) => {
                self.open_spans.pop();
                lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies)
            }
            Some(OpenSpan::Fx { .. }) => Err(fx_error(
                "an explicit `[fx ...]` span must close with `[/fx]`",
            )),
            None => lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies),
        }
    }

    fn has_open_fx(&self) -> bool {
        self.open_spans
            .iter()
            .any(|span| matches!(span, OpenSpan::Fx { .. }))
    }

    fn top_fx_closes_explicitly(&self, name: &str) -> bool {
        matches!(
            self.open_spans.last(),
            Some(OpenSpan::Fx {
                close: FxClose::Explicit(expected),
                ..
            }) if expected == canonical_tag_name(name)
        )
    }

    fn top_fx_closes_inferred(&self) -> bool {
        matches!(
            self.open_spans.last(),
            Some(OpenSpan::Fx {
                close: FxClose::Inferred,
                ..
            })
        )
    }

    fn track_open_styles(&mut self, nodes: &[RichTextNode]) {
        self.open_spans.extend(nodes.iter().filter_map(|node| {
            let RichTextNode::StyleStart { style } = node else {
                return None;
            };
            if matches!(style, RichTextStyle::Speed { .. }) {
                return None;
            }
            Some(OpenSpan::Style {
                name: style.tag_name().to_owned(),
            })
        }));
    }

    fn track_permissive_close(&mut self, nodes: &[RichTextNode]) {
        for node in nodes {
            match node {
                RichTextNode::StyleEnd { name } if name == "/" => {
                    self.open_spans.pop();
                }
                RichTextNode::StyleEnd { name } => {
                    let canonical = canonical_tag_name(name);
                    if let Some(index) = self.open_spans.iter().rposition(
                        |span| matches!(span, OpenSpan::Style { name } if name == canonical),
                    ) {
                        self.open_spans.remove(index);
                    }
                }
                RichTextNode::Control {
                    control: RichTextControl::Reset,
                } => self.open_spans.clear(),
                _ => {}
            }
        }
    }
}
