//! Stateful expansion of authored decoration spans within one dialogue line.

use std::collections::BTreeMap;

use arcweft_lang_hir::syntax::{
    ast::dialogue::{DialogueTag, DialogueTagArg, DialogueToken},
    text::canonical_rich_text_tag_name,
};
use arcweft_render_text::{InlineFailurePolicy, RichTextControl, RichTextNode, RichTextStyle};

use crate::errors::RuntimePlanLowerError;

use super::{
    DecorationCatalog, DecorationInlineAssignment, contributions::inline_assignments,
    decoration_error,
};
use crate::render_text::{defaults::TextProxyTypeDefaults, tag::lower_dialogue_token_parts};

/// Treats a decoration as one authored span even though its product
/// representation contains several ordinary style nodes.
pub(crate) struct DialogueDecorationExpander<'catalog> {
    catalog: &'catalog DecorationCatalog,
    open_spans: Vec<OpenSpan>,
    inline_assignments: Vec<DecorationInlineAssignment>,
}

#[derive(Clone, Debug)]
enum OpenSpan {
    Style { name: String },
    Decoration { name: String, ends: Vec<String> },
}

impl<'catalog> DialogueDecorationExpander<'catalog> {
    pub(crate) fn new(catalog: &'catalog DecorationCatalog) -> Self {
        Self {
            catalog,
            open_spans: Vec::new(),
            inline_assignments: Vec::new(),
        }
    }

    pub(crate) fn lower_token(
        &mut self,
        token: &DialogueToken,
        default_inline_failure_policy: Option<&InlineFailurePolicy>,
        text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
    ) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        match token {
            DialogueToken::Tag(tag) if tag.name() == "decorate" => {
                let layers = self.catalog.expand_tag(tag)?;
                let invocation_range = tag
                    .arguments()
                    .first()
                    .map_or_else(|| tag.range(), |argument| argument.value().range());
                self.inline_assignments
                    .extend(inline_assignments(&layers, invocation_range));
                let ends = layers
                    .iter()
                    .rev()
                    .map(|layer| layer.style.tag_name().to_owned())
                    .collect::<Vec<_>>();
                self.open_spans.push(OpenSpan::Decoration {
                    name: decoration_name_from_tag(tag),
                    ends,
                });
                Ok(layers
                    .into_iter()
                    .map(|layer| RichTextNode::StyleStart { style: layer.style })
                    .collect())
            }
            DialogueToken::EndTag(end) if end.name() == "decorate" => self.close_decoration(),
            DialogueToken::InferredEndTag if self.has_open_decoration() => self
                .close_inferred_inside_decoration(
                    token,
                    default_inline_failure_policy,
                    text_proxies,
                ),
            DialogueToken::Tag(tag) if tag.name() == "reset" && self.has_open_decoration() => {
                Err(decoration_error(
                    "`[reset]` cannot clear styles from inside an open decoration span",
                ))
            }
            DialogueToken::EndTag(end) if self.has_open_decoration() => self.close_nested_style(
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

    pub(crate) fn finish(self) -> Result<Vec<DecorationInlineAssignment>, RuntimePlanLowerError> {
        let unclosed = self
            .open_spans
            .iter()
            .filter_map(|span| match span {
                OpenSpan::Decoration { name, .. } => Some(name.as_str()),
                OpenSpan::Style { .. } => None,
            })
            .collect::<Vec<_>>();
        if unclosed.is_empty() {
            Ok(self.inline_assignments)
        } else {
            Err(decoration_error(format!(
                "unclosed rich-text decoration span(s): {}",
                unclosed.join(", ")
            )))
        }
    }

    fn close_decoration(&mut self) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        match self.open_spans.pop() {
            Some(OpenSpan::Decoration { ends, .. }) => Ok(ends
                .into_iter()
                .map(|name| RichTextNode::StyleEnd { name })
                .collect()),
            Some(span @ OpenSpan::Style { .. }) => {
                self.open_spans.push(span);
                Err(decoration_error(
                    "`[/decorate]` crosses a rich-text span opened inside the decoration",
                ))
            }
            None => Err(decoration_error(
                "`[/decorate]` has no matching `[decorate ...]` opener",
            )),
        }
    }

    fn close_nested_style(
        &mut self,
        name: &str,
        token: &DialogueToken,
        default_inline_failure_policy: Option<&InlineFailurePolicy>,
        text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
    ) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        let canonical = canonical_rich_text_tag_name(name);
        match self.open_spans.last() {
            Some(OpenSpan::Style { name }) if name == canonical => {
                let _ = self.open_spans.pop();
                lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies)
            }
            Some(OpenSpan::Decoration { .. }) => Err(decoration_error(format!(
                "`[/{name}]` cannot close an internal layer of an open decoration; close `[/decorate]`"
            ))),
            Some(OpenSpan::Style { name: open }) => Err(decoration_error(format!(
                "mismatched rich-text close `[/{name}]` inside decoration; expected `[/{open}]`"
            ))),
            None => lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies),
        }
    }

    fn close_inferred_inside_decoration(
        &mut self,
        token: &DialogueToken,
        default_inline_failure_policy: Option<&InlineFailurePolicy>,
        text_proxies: &BTreeMap<String, TextProxyTypeDefaults>,
    ) -> Result<Vec<RichTextNode>, RuntimePlanLowerError> {
        match self.open_spans.last() {
            Some(OpenSpan::Style { .. }) => {
                let _ = self.open_spans.pop();
                lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies)
            }
            Some(OpenSpan::Decoration { .. }) => Err(decoration_error(
                "an explicit `[decorate ...]` span must close with `[/decorate]`",
            )),
            None => lower_dialogue_token_parts(token, default_inline_failure_policy, text_proxies),
        }
    }

    fn has_open_decoration(&self) -> bool {
        self.open_spans
            .iter()
            .any(|span| matches!(span, OpenSpan::Decoration { .. }))
    }

    fn track_open_styles(&mut self, nodes: &[RichTextNode]) {
        self.open_spans.extend(nodes.iter().filter_map(|node| {
            let RichTextNode::StyleStart { style } = node else {
                return None;
            };
            // Speed is a point modifier that remains active until another
            // speed/reset boundary or the end of the line. It is not a span
            // that must close before the surrounding decoration.
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
                    let _ = self.open_spans.pop();
                }
                RichTextNode::StyleEnd { name } => {
                    let canonical = canonical_rich_text_tag_name(name);
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

fn decoration_name_from_tag(tag: &DialogueTag) -> String {
    tag.arguments()
        .first()
        .and_then(|argument| match argument {
            DialogueTagArg::Positional { value } => value.value().trim().strip_prefix('.'),
            DialogueTagArg::Named { .. } => None,
        })
        .unwrap_or("<unknown>")
        .to_owned()
}
