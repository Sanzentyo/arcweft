//! Atomic authored-span validation for decoration invocations.

use arcweft_lang_syntax::{
    ast::dialogue::{DialogueTag, DialogueToken},
    text::{RichTextTagFamily, canonical_rich_text_tag_name, inferred_rich_text_tag_family},
};

use super::{DecorationCatalog, TypeCheckError};

#[derive(Clone, Debug)]
enum OpenSpan {
    Style { name: String },
    Decoration { name: String },
}

/// Mirrors the runtime-plan nesting rules that make an expanded decoration one
/// atomic authored span even though it produces several internal style layers.
#[derive(Clone, Debug, Default)]
pub(in crate::checker) struct DecorationSpanState {
    open: Vec<OpenSpan>,
}

impl DecorationSpanState {
    pub(in crate::checker) fn observe(
        &mut self,
        token: &DialogueToken,
        catalog: &DecorationCatalog,
        errors: &mut Vec<TypeCheckError>,
    ) {
        match token {
            DialogueToken::Tag(tag) if tag.name() == "decorate" => {
                self.open.push(OpenSpan::Decoration {
                    name: catalog.validate_dialogue_tag(tag, errors),
                });
            }
            DialogueToken::EndTag(tag) if tag.name() == "decorate" => {
                self.close_decoration(errors);
            }
            DialogueToken::Tag(tag) if tag.name() == "reset" => self.reset(errors),
            DialogueToken::Tag(tag) => {
                if let Some(name) = explicit_style_name(tag) {
                    self.open.push(OpenSpan::Style {
                        name: name.to_owned(),
                    });
                }
            }
            DialogueToken::InferredTag(tag) => {
                let name = if catalog.inferred_tag_is_text_proxy(tag) {
                    Some("object")
                } else {
                    inferred_style_name(tag)
                };
                if let Some(name) = name {
                    self.open.push(OpenSpan::Style {
                        name: name.to_owned(),
                    });
                }
            }
            DialogueToken::EndTag(tag) => self.close_style(tag.name(), errors),
            DialogueToken::InferredEndTag => self.close_inferred(errors),
            DialogueToken::Text(_)
            | DialogueToken::Raw(_)
            | DialogueToken::Expr(_)
            | DialogueToken::Mark(_)
            | DialogueToken::Ruby { .. }
            | DialogueToken::Escape(_) => {}
        }
    }

    pub(in crate::checker) fn finish(self, errors: &mut Vec<TypeCheckError>) {
        for span in self.open.into_iter().rev() {
            if let OpenSpan::Decoration { name } = span {
                errors.push(TypeCheckError::new(format!(
                    "unclosed `[decorate .{name}]` span"
                )));
            }
        }
    }

    fn has_open_decoration(&self) -> bool {
        self.open
            .iter()
            .any(|span| matches!(span, OpenSpan::Decoration { .. }))
    }

    fn close_decoration(&mut self, errors: &mut Vec<TypeCheckError>) {
        match self.open.last() {
            Some(OpenSpan::Decoration { .. }) => {
                let _ = self.open.pop();
            }
            Some(OpenSpan::Style { .. }) => errors.push(TypeCheckError::new(
                "`[/decorate]` crosses a rich-text span opened inside the decoration".to_owned(),
            )),
            None => errors.push(TypeCheckError::new("unmatched `[/decorate]`".to_owned())),
        }
    }

    fn close_style(&mut self, authored_name: &str, errors: &mut Vec<TypeCheckError>) {
        let canonical = canonical_rich_text_tag_name(authored_name);
        if !self.has_open_decoration() {
            if let Some(index) = self
                .open
                .iter()
                .rposition(|span| matches!(span, OpenSpan::Style { name } if name == canonical))
            {
                self.open.remove(index);
            }
            return;
        }

        match self.open.last() {
            Some(OpenSpan::Style { name }) if name == canonical => {
                let _ = self.open.pop();
            }
            Some(OpenSpan::Decoration { .. }) => errors.push(TypeCheckError::new(format!(
                "`[/{authored_name}]` cannot close an internal layer of an open decoration; close `[/decorate]`"
            ))),
            Some(OpenSpan::Style { name }) => errors.push(TypeCheckError::new(format!(
                "mismatched rich-text close `[/{authored_name}]` inside decoration; expected `[/{name}]`"
            ))),
            None => {}
        }
    }

    fn close_inferred(&mut self, errors: &mut Vec<TypeCheckError>) {
        if !self.has_open_decoration() {
            let _ = self.open.pop();
            return;
        }
        match self.open.last() {
            Some(OpenSpan::Style { .. }) => {
                let _ = self.open.pop();
            }
            Some(OpenSpan::Decoration { .. }) => errors.push(TypeCheckError::new(
                "an explicit `[decorate ...]` span must close with `[/decorate]`".to_owned(),
            )),
            None => {}
        }
    }

    fn reset(&mut self, errors: &mut Vec<TypeCheckError>) {
        if self.has_open_decoration() {
            errors.push(TypeCheckError::new(
                "`[reset]` cannot clear styles from inside an open decoration span".to_owned(),
            ));
        } else {
            self.open.clear();
        }
    }
}

fn explicit_style_name(tag: &DialogueTag) -> Option<&'static str> {
    match tag.name() {
        "em" => Some("em"),
        "strong" => Some("strong"),
        "color" => Some("color"),
        "font" => Some("font"),
        "size" => Some("size"),
        "i" | "italic" | "oblique" | "slant" | "style" => Some("style"),
        "layout" => Some("layout"),
        "transform" => Some("transform"),
        "object" => Some("object"),
        "effect" | "fx" if !uses_host_event_phase(tag) => Some("effect"),
        _ => None,
    }
}

fn inferred_style_name(tag: &DialogueTag) -> Option<&'static str> {
    let selector = tag.name().trim_start_matches('.');
    if uses_host_event_phase(tag) {
        return None;
    }
    match inferred_rich_text_tag_family(selector, tag.attrs()) {
        Some(RichTextTagFamily::Style) => Some("style"),
        Some(RichTextTagFamily::Layout) => Some("layout"),
        Some(RichTextTagFamily::Transform) => Some("transform"),
        Some(RichTextTagFamily::Effect) => Some("effect"),
        Some(RichTextTagFamily::Marker) | None => None,
    }
}

fn uses_host_event_phase(tag: &DialogueTag) -> bool {
    tag.arguments().iter().any(|argument| {
        argument.name() == Some("phase") && argument.value().value() == "host_event"
    })
}
