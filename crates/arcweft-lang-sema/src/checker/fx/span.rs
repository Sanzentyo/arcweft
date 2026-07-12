//! Atomic `RichText` span validation for `[fx ...]` invocations.

use arcweft_lang_syntax::ast::dialogue::{DialogueTag, DialogueTagKind, DialogueToken};

use crate::diagnostics::TypeCheckError;

use super::FxCatalog;

#[derive(Clone, Debug)]
enum OpenSpan {
    Fx { name: String },
    RichText { name: String },
}

/// Tracks authored span nesting while keeping one expanded Fx graph atomic.
#[derive(Clone, Debug, Default)]
pub(in crate::checker) struct FxSpanState {
    open: Vec<OpenSpan>,
}

impl FxSpanState {
    pub(in crate::checker) fn observe(
        &mut self,
        token: &DialogueToken,
        catalog: &FxCatalog,
        errors: &mut Vec<TypeCheckError>,
    ) {
        match token {
            DialogueToken::Tag(tag) if tag.kind() == DialogueTagKind::Fx => {
                let name = catalog.validate_dialogue_tag(tag, errors);
                self.open.push(OpenSpan::Fx { name });
            }
            DialogueToken::EndTag(tag) if tag.kind() == DialogueTagKind::Fx => {
                self.close_fx(errors);
            }
            DialogueToken::Tag(tag) if tag.kind() == DialogueTagKind::Reset => {
                if self.has_open_fx() {
                    errors.push(TypeCheckError::new(
                        "`[reset]` cannot clear styles from inside an open Fx span".to_owned(),
                    ));
                } else {
                    self.open.clear();
                }
            }
            DialogueToken::Tag(tag) | DialogueToken::InferredTag(tag) => {
                self.open_rich_text(tag, catalog);
            }
            DialogueToken::EndTag(tag) => self.close_named(tag.name(), errors),
            DialogueToken::InferredEndTag => self.close_inferred(errors),
            _ => {}
        }
    }

    pub(in crate::checker) fn finish(self, errors: &mut Vec<TypeCheckError>) {
        for span in self.open.into_iter().rev() {
            if let OpenSpan::Fx { name } = span {
                errors.push(TypeCheckError::new(format!(
                    "unclosed `[fx {name}(...)]` span"
                )));
            }
        }
    }

    fn open_rich_text(&mut self, tag: &DialogueTag, catalog: &FxCatalog) {
        if tag.kind() != DialogueTagKind::Span
            || (tag.name().starts_with('.') && catalog.inferred_tag_is_mark(tag))
        {
            return;
        }
        self.open.push(OpenSpan::RichText {
            name: tag.name().trim_start_matches('.').to_owned(),
        });
    }

    fn close_fx(&mut self, errors: &mut Vec<TypeCheckError>) {
        match self.open.last() {
            Some(OpenSpan::Fx { .. }) => {
                self.open.pop();
            }
            Some(OpenSpan::RichText { .. }) => errors.push(TypeCheckError::new(
                "`[/fx]` crosses a RichText span opened inside the Fx span".to_owned(),
            )),
            None => errors.push(TypeCheckError::new("unmatched `[/fx]`".to_owned())),
        }
    }

    fn close_named(&mut self, authored_name: &str, errors: &mut Vec<TypeCheckError>) {
        let name = authored_name.trim_start_matches('.');
        match self.open.last() {
            Some(OpenSpan::RichText { name: open }) if open == name => {
                self.open.pop();
            }
            Some(OpenSpan::Fx { .. }) => errors.push(TypeCheckError::new(format!(
                "`[/{authored_name}]` cannot close an internal layer of an open Fx span; close `[/fx]`"
            ))),
            Some(OpenSpan::RichText { name: expected }) => errors.push(TypeCheckError::new(
                format!("mismatched RichText close `[/{authored_name}]`; expected `[/{expected}]`"),
            )),
            None => {}
        }
    }

    fn close_inferred(&mut self, errors: &mut Vec<TypeCheckError>) {
        match self.open.last() {
            Some(OpenSpan::RichText { .. }) => {
                self.open.pop();
            }
            Some(OpenSpan::Fx { .. }) => errors.push(TypeCheckError::new(
                "an explicit `[fx ...]` span must close with `[/fx]`".to_owned(),
            )),
            None => {}
        }
    }

    fn has_open_fx(&self) -> bool {
        self.open
            .iter()
            .any(|span| matches!(span, OpenSpan::Fx { .. }))
    }
}
