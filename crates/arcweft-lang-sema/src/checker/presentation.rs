use crate::diagnostics::TypeCheckError;
use crate::types::{EntityKind, TypeKind};
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal};

use super::{TypeChecker, entity_kind};

impl TypeChecker<'_> {
    pub(super) fn check_presentation_call(
        &mut self,
        name: &str,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        match name {
            "bg" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Asset, "bg asset");
                self.check_presentation_named_args(args, "background");
                Some(TypeKind::Named(
                    "PresentationHandle<BackgroundSurface>".to_owned(),
                ))
            }
            "image" | "image.show" => {
                self.check_presentation_asset_arg(args, "image asset");
                self.check_presentation_image_named_args(args);
                Some(TypeKind::Named(
                    "PresentationHandle<ImageSurface>".to_owned(),
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

    fn check_presentation_asset_arg(&mut self, args: &[CallArg], context: &str) {
        let arg = args
            .iter()
            .find_map(|arg| match arg {
                CallArg::Named { name, value } if name == "asset" => Some(value.as_ref()),
                _ => None,
            })
            .or_else(|| {
                args.iter().find_map(|arg| match arg {
                    CallArg::Positional(value) => Some(value),
                    CallArg::Named { .. } | CallArg::Spread { .. } => None,
                })
            });
        let Some(arg) = arg else {
            self.errors.push(TypeCheckError::new(format!(
                "{context} argument is required"
            )));
            return;
        };
        match arg {
            Expr::EntityRef(entity) => match entity.as_absolute().and_then(entity_kind) {
                Some(EntityKind::Asset) => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "{context} must be an Asset reference, found {actual:?}"
                ))),
            },
            Expr::Path(path)
                if self.locals.get(path) == Some(&TypeKind::Ref(EntityKind::Asset)) => {}
            Expr::Path(path)
                if self.env.symbol_type(path) == Some(&TypeKind::Ref(EntityKind::Asset)) => {}
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(format!(
                    "{context} must be an Asset reference"
                )));
            }
        }
    }

    fn check_positional_entity_arg(
        &mut self,
        args: &[CallArg],
        index: usize,
        expected: &EntityKind,
        context: &str,
    ) {
        let Some(arg) = args
            .iter()
            .filter_map(|arg| match arg {
                CallArg::Positional(value) => Some(value),
                CallArg::Named { .. } | CallArg::Spread { .. } => None,
            })
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

    fn check_presentation_named_args(&mut self, args: &[CallArg], slot_family: &str) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
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

    fn check_presentation_image_named_args(&mut self, args: &[CallArg]) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
                continue;
            };
            match name.as_str() {
                "asset" => {}
                "target" => self.check_presentation_image_id_value(value, &EntityKind::Target),
                "layer" => self.check_presentation_image_id_value(value, &EntityKind::Layer),
                "id" | "action" | "actions" | "fit" | "proxy.id" | "proxy.type" | "proxy.role" => {
                    self.check_presentation_image_loose_value(value);
                }
                "depth" => {
                    self.expect_expr_type(value, &TypeKind::I32, "image depth");
                }
                "opacity" => {
                    self.check_presentation_image_opacity_value(value);
                }
                "enabled" | "visible" => {
                    self.expect_expr_type(value, &TypeKind::Bool, "image lifecycle flag");
                }
                "x" | "y" | "width" | "height" | "transform.tx" | "transform.ty" => {
                    self.check_expr(value);
                }
                "transform.m11" | "transform.m12" | "transform.m21" | "transform.m22" => {
                    self.check_presentation_image_transform_component_value(value);
                }
                "proxy.layer" => {
                    self.check_presentation_image_id_value(value, &EntityKind::Layer);
                }
                "proxy.depth" => {
                    self.expect_expr_type(value, &TypeKind::I32, "image proxy depth");
                }
                "proxy.hit_test" => {
                    self.expect_expr_type(value, &TypeKind::Bool, "image proxy hit-test flag");
                }
                custom if custom.starts_with("param.") => {
                    self.check_presentation_image_param_value(value);
                }
                custom if custom.starts_with("proxy.param.") => {
                    self.check_presentation_image_param_value(value);
                }
                _ => {
                    self.check_expr(value);
                }
            }
        }
    }

    fn check_presentation_image_id_value(&mut self, expr: &Expr, expected: &EntityKind) {
        match expr {
            Expr::EntityRef(entity) => match entity.as_absolute().and_then(entity_kind) {
                Some(kind) if &kind == expected => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "presentation image id must be a {expected:?} reference or public-id string, found {actual:?}"
                ))),
            },
            Expr::Literal(Literal::String(_)) => {}
            other => {
                self.check_expr(other);
            }
        }
    }

    fn check_presentation_image_loose_value(&mut self, expr: &Expr) {
        match expr {
            Expr::EntityRef(_) | Expr::Literal(Literal::String(_)) | Expr::Path(_) => {}
            other => {
                self.check_expr(other);
            }
        }
    }

    fn check_presentation_image_param_value(&mut self, expr: &Expr) {
        match expr {
            Expr::EntityRef(_)
            | Expr::Literal(Literal::String(_) | Literal::Bool(_))
            | Expr::Path(_) => {}
            Expr::Literal(Literal::Int { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::I64, "image param integer");
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::F64, "image param float");
            }
            other => {
                self.check_expr(other);
            }
        }
    }

    fn check_presentation_image_opacity_value(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(Literal::Int { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::I32, "image opacity milli");
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::F64, "image opacity ratio");
            }
            other => {
                self.check_expr(other);
            }
        }
    }

    fn check_presentation_image_transform_component_value(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(Literal::Int { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::I32, "image transform component milli");
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::F64, "image transform component ratio");
            }
            other => {
                self.check_expr(other);
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
