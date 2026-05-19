use crate::diagnostics::TypeCheckError;
use crate::types::{EntityKind, TypeKind};
use arcweft_lang_syntax::expr::Expr;

use super::{TypeChecker, entity_kind};

impl TypeChecker<'_> {
    pub(super) fn check_presentation_call(
        &mut self,
        name: &str,
        args: &[Expr],
    ) -> Option<TypeKind> {
        match name {
            "bg" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Asset, "bg asset");
                self.check_presentation_named_args(args, "background");
                Some(TypeKind::Named(
                    "PresentationHandle<BackgroundSurface>".to_owned(),
                ))
            }
            "show" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Character, "show character");
                self.check_presentation_named_args(args, "character");
                Some(TypeKind::Named(
                    "PresentationHandle<CharacterSurface>".to_owned(),
                ))
            }
            "ref.bg" => {
                self.check_presentation_named_args(args, "background");
                Some(TypeKind::Named("SlotRef<BackgroundSurface>".to_owned()))
            }
            "ref.show" => {
                self.check_positional_entity_arg(
                    args,
                    0,
                    &EntityKind::Character,
                    "ref show character",
                );
                self.check_presentation_named_args(args, "character");
                Some(TypeKind::Named("SlotRef<CharacterSurface>".to_owned()))
            }
            "clear.bg" => {
                self.check_presentation_named_args(args, "background");
                self.active_presentation_defaults.remove("background");
                Some(TypeKind::Named("Option<BackgroundSurface>".to_owned()))
            }
            "hide" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Character, "hide character");
                self.check_presentation_named_args(args, "character");
                self.active_presentation_defaults.remove("character");
                Some(TypeKind::Named("Option<CharacterSurface>".to_owned()))
            }
            _ => None,
        }
    }

    fn check_positional_entity_arg(
        &mut self,
        args: &[Expr],
        index: usize,
        expected: &EntityKind,
        context: &str,
    ) {
        let Some(arg) = args
            .iter()
            .filter(|arg| !matches!(arg, Expr::NamedArg { .. }))
            .nth(index)
        else {
            self.errors.push(TypeCheckError::new(format!(
                "{context} argument is required"
            )));
            return;
        };
        match arg {
            Expr::EntityRef(entity) => match entity.as_absolute().and_then(entity_kind) {
                Some(kind) if &kind == expected => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "{context} must be a {expected:?} reference, found {actual:?}"
                ))),
            },
            Expr::Path(path) if self.locals.get(path) == Some(&TypeKind::Ref(expected.clone())) => {
            }
            Expr::Path(path)
                if self.env.symbol_type(path) == Some(&TypeKind::Ref(expected.clone())) => {}
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(format!(
                    "{context} must be a {expected:?} reference"
                )));
            }
        }
    }

    fn check_presentation_named_args(&mut self, args: &[Expr], slot_family: &str) {
        for arg in args {
            let Expr::NamedArg { name, value } = arg else {
                continue;
            };
            match name.as_str() {
                "target" => self.expect_entity_expr_kind(value, &EntityKind::Target, "target"),
                "slot" => self.expect_slot_family(value, slot_family),
                "scope" => self.expect_entity_expr_kind(
                    value,
                    &EntityKind::Other("scope".to_owned()),
                    "scope",
                ),
                _ => {
                    self.check_expr(value);
                }
            }
        }
    }

    fn expect_entity_expr_kind(&mut self, expr: &Expr, expected: &EntityKind, context: &str) {
        match expr {
            Expr::EntityRef(entity) => match entity.as_absolute().and_then(entity_kind) {
                Some(kind) if &kind == expected => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "presentation {context} must be a {expected:?} reference, found {actual:?}"
                ))),
            },
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(format!(
                    "presentation {context} must be an entity reference"
                )));
            }
        }
    }

    fn expect_slot_family(&mut self, expr: &Expr, slot_family: &str) {
        match expr {
            Expr::EntityRef(entity) => {
                let Some(entity) = entity.as_absolute() else {
                    self.errors.push(TypeCheckError::new(
                        "presentation slot must be an absolute slot reference".to_owned(),
                    ));
                    return;
                };
                if entity_kind(entity) != Some(EntityKind::Slot)
                    || !entity.body().starts_with(&format!("slot.{slot_family}."))
                {
                    self.errors.push(TypeCheckError::new(format!(
                        "presentation slot `{}` must be in `@slot.{slot_family}.*`",
                        entity.body()
                    )));
                }
            }
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(
                    "presentation slot must be an entity reference".to_owned(),
                ));
            }
        }
    }
}
