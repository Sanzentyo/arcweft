use crate::diagnostics::TypeCheckError;
use crate::types::{EntityKind, TypeKind};
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal};

use super::{TypeChecker, entity_kind, entity_syntax_kind};

impl TypeChecker<'_> {
    pub(super) fn check_presentation_call(
        &mut self,
        name: &str,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        match name {
            "view" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::View, "view mount");
                self.check_presentation_view_named_args(args);
                Some(TypeKind::presentation_handle("View"))
            }
            "menu" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::View, "menu mount");
                self.check_presentation_view_named_args(args);
                Some(TypeKind::presentation_handle("Menu"))
            }
            "overlay" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::View, "overlay mount");
                self.check_presentation_view_named_args(args);
                Some(TypeKind::presentation_handle("Overlay"))
            }
            "bg" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Asset, "bg asset");
                self.check_presentation_background_named_args(args);
                Some(TypeKind::presentation_handle("BackgroundSurface"))
            }
            "image" => {
                self.check_presentation_image_source_arg(args);
                self.check_presentation_image_named_args(args);
                Some(TypeKind::presentation_handle("ImageSurface"))
            }
            "player_viewport" => {
                self.check_presentation_viewport_args(args);
                Some(TypeKind::presentation_handle("Viewport"))
            }
            "show" => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Character, "show character");
                self.check_character_look_arg(args);
                self.check_presentation_named_args(args, "character");
                Some(TypeKind::presentation_handle("CharacterSurface"))
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

    fn check_presentation_viewport_args(&mut self, args: &[CallArg]) {
        for arg in args {
            match arg {
                CallArg::Positional(value) => self.check_presentation_image_loose_value(value),
                CallArg::Named { name, value } => match name.as_str() {
                    "width" | "height" => self.check_presentation_viewport_dimension_value(value),
                    "fit" => {
                        self.check_presentation_image_loose_value(value);
                    }
                    _ => self.reject_unknown_presentation_argument("player_viewport", name, value),
                },
                CallArg::Spread { value } => {
                    self.check_expr(value);
                }
            }
        }
    }

    fn check_presentation_view_named_args(&mut self, args: &[CallArg]) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
                continue;
            };
            match name.as_str() {
                "lifetime" => self.check_presentation_lifetime_arg(value),
                "target" => self.check_presentation_image_id_value(value, &EntityKind::Target),
                "layer" => self.check_presentation_image_id_value(value, &EntityKind::Layer),
                "id" | "handle" | "key" | "mount" => {
                    self.check_presentation_image_loose_value(value);
                }
                "depth" => self.expect_expr_type(value, &TypeKind::I32, "view depth"),
                "visible" | "enabled" => {
                    self.expect_expr_type(value, &TypeKind::Bool, "view lifecycle flag");
                }
                _ => {
                    self.check_expr(value);
                }
            }
        }
    }

    fn check_presentation_lifetime_arg(&mut self, expr: &Expr) {
        match expr {
            Expr::ShortVariant(value)
                if matches!(
                    value.as_str(),
                    "scope" | "manual" | "detached" | "global" | "line" | "flow"
                ) => {}
            Expr::Path(value)
                if matches!(
                    value.as_label().trim_start_matches('.'),
                    "scope" | "manual" | "detached" | "global" | "line" | "flow"
                ) => {}
            Expr::Literal(Literal::String(value))
                if matches!(
                    value.as_str(),
                    "scope" | "manual" | "detached" | "global" | "line" | "flow"
                ) => {}
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(
                    "view/image lifetime must be one of `.scope`, `.manual`, `.detached`, `.global`, `.line`, or `.flow`"
                        .to_owned(),
                ));
            }
        }
    }

    fn check_presentation_image_source_arg(&mut self, args: &[CallArg]) {
        if Self::first_positional_entity_kind(args) == Some(EntityKind::Image) {
            return;
        }
        self.check_presentation_asset_arg(args, "image asset");
    }

    fn first_positional_entity_kind(args: &[CallArg]) -> Option<EntityKind> {
        args.iter().find_map(|arg| match arg {
            CallArg::Positional(Expr::EntityRef(entity)) => entity_syntax_kind(entity),
            CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
        })
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
            Expr::EntityRef(entity) => match entity_syntax_kind(entity) {
                Some(EntityKind::Asset) => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "{context} must be an Asset reference, found {actual:?}"
                ))),
            },
            Expr::Path(path)
                if self.locals.get(path.as_label())
                    == Some(&TypeKind::entity_ref(EntityKind::Asset)) => {}
            Expr::Path(path)
                if self.env.symbol_type(path.as_label())
                    == Some(&TypeKind::entity_ref(EntityKind::Asset)) => {}
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
            Expr::EntityRef(entity) => match entity_syntax_kind(entity) {
                Some(kind) if &kind == expected => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "{context} must be a {expected:?} reference, found {actual:?}"
                ))),
            },
            Expr::Path(path)
                if self.locals.get(path.as_label())
                    == Some(&TypeKind::entity_ref(expected.clone())) => {}
            Expr::Path(path)
                if self.env.symbol_type(path.as_label())
                    == Some(&TypeKind::entity_ref(expected.clone())) => {}
            other => {
                self.check_expr(other);
                self.errors.push(TypeCheckError::new(format!(
                    "{context} must be a {expected:?} reference"
                )));
            }
        }
    }

    fn check_character_look_arg(&mut self, args: &[CallArg]) {
        let look = args
            .iter()
            .find_map(|arg| match arg {
                CallArg::Named { name, value } if name == "look" => Some(value.as_ref()),
                _ => None,
            })
            .or_else(|| {
                args.iter()
                    .filter_map(|arg| match arg {
                        CallArg::Positional(value) => Some(value),
                        CallArg::Named { .. } | CallArg::Spread { .. } => None,
                    })
                    .nth(1)
            });
        let Some(look) = look else {
            return;
        };
        self.check_expr(look);
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
                "look" if slot_family == "character" => {}
                _ => {
                    self.check_expr(value);
                }
            }
        }
    }

    fn check_presentation_background_named_args(&mut self, args: &[CallArg]) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
                continue;
            };
            match name.as_str() {
                "target" => self.expect_entity_expr_kind(value, &EntityKind::Target, "target"),
                "slot" => self.expect_slot_family(value, "background"),
                "scope" => self.expect_entity_expr_kind(
                    value,
                    &EntityKind::Other("scope".to_owned()),
                    "scope",
                ),
                "fade" => {
                    self.check_expr(value);
                }
                _ if self.check_presentation_image_common_named_arg(name, value) => {}
                _ => self.reject_unknown_presentation_argument("bg", name, value),
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
                "lifetime" => self.check_presentation_lifetime_arg(value),
                "target" => self.check_presentation_image_id_value(value, &EntityKind::Target),
                "layer" => self.check_presentation_image_id_value(value, &EntityKind::Layer),
                "id" | "action" | "actions" | "fit" | "proxy.id" | "proxy.type" | "proxy.role"
                | "focus" | "input_capture" | "owner" | "drop" => {
                    self.check_presentation_image_loose_value(value);
                }
                "alignment.x" | "alignment.y" => {
                    self.check_presentation_image_ratio_or_milli_value(value, "image alignment");
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
                    self.check_presentation_image_transform_view_value(value);
                }
                "playback.start" | "playback.paused_at" | "playback.local_time" => {
                    self.check_presentation_image_time_value(value);
                }
                "playback.rate" => {
                    self.check_presentation_image_ratio_or_milli_value(
                        value,
                        "image playback rate",
                    );
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
                _ => self.reject_unknown_presentation_argument("image", name, value),
            }
        }
    }

    fn check_presentation_image_common_named_arg(&mut self, name: &str, value: &Expr) -> bool {
        match name {
            "fit" => {
                self.check_presentation_image_loose_value(value);
                true
            }
            "opacity" => {
                self.check_presentation_image_opacity_value(value);
                true
            }
            "alignment.x" | "alignment.y" => {
                self.check_presentation_image_ratio_or_milli_value(value, "image alignment");
                true
            }
            "playback.start" | "playback.paused_at" | "playback.local_time" => {
                self.check_presentation_image_time_value(value);
                true
            }
            "playback.rate" => {
                self.check_presentation_image_ratio_or_milli_value(value, "image playback rate");
                true
            }
            _ => false,
        }
    }

    fn reject_unknown_presentation_argument(
        &mut self,
        command: &str,
        argument: &str,
        value: &Expr,
    ) {
        self.check_expr(value);
        self.errors
            .push(TypeCheckError::unknown_presentation_argument(
                command, argument,
            ));
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
            Expr::Literal(Literal::Int(literal)) if literal.suffix().is_none() => {
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
        self.check_presentation_image_ratio_or_milli_value(expr, "image opacity");
    }

    fn check_presentation_image_ratio_or_milli_value(&mut self, expr: &Expr, context: &str) {
        match expr {
            Expr::Literal(Literal::Int(literal)) if literal.suffix().is_none() => {
                self.expect_expr_type(expr, &TypeKind::I32, context);
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::F64, context);
            }
            Expr::Literal(Literal::String(_)) | Expr::Path(_) => {}
            other => {
                self.check_expr(other);
            }
        }
    }

    fn check_presentation_viewport_dimension_value(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(Literal::UnitNumber { .. } | Literal::String(_)) => {}
            Expr::Literal(Literal::Int(literal)) if literal.suffix().is_none() => {
                self.expect_expr_type(expr, &TypeKind::I32, "viewport dimension");
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::F64, "viewport dimension");
            }
            other => {
                self.check_expr(other);
            }
        }
    }

    fn check_presentation_image_time_value(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(Literal::Duration { .. }) => {
                self.expect_expr_type(expr, &TypeKind::Duration, "image playback time");
            }
            Expr::Literal(Literal::Int(literal)) if literal.suffix().is_none() => {
                self.expect_expr_type(expr, &TypeKind::I32, "image playback time seconds");
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::F64, "image playback time seconds");
            }
            Expr::Literal(Literal::String(_)) | Expr::Path(_) => {}
            other => {
                self.check_expr(other);
            }
        }
    }

    fn check_presentation_image_transform_view_value(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(Literal::Int(literal)) if literal.suffix().is_none() => {
                self.expect_expr_type(expr, &TypeKind::I32, "image transform view milli");
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.expect_expr_type(expr, &TypeKind::F64, "image transform view ratio");
            }
            Expr::Literal(Literal::String(_)) | Expr::Path(_) => {}
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
