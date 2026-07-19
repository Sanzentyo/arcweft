use crate::callable::{
    CallableGroupIndex, CallableParameter, CallableParameterPassing, CallableParameterType,
    PresentationArgumentValuePolicy, PresentationCallableId, PresentationNamedArgument,
    UnknownNamedArgumentPolicy,
};
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
        let callable = PresentationCallableId::resolve_surface_name(name)?;
        self.check_presentation_argument_values(callable, args);
        match callable {
            PresentationCallableId::View => {
                self.check_positional_entity_arg(args, 0, &EntityKind::View, "view mount");
                self.check_presentation_view_named_args(callable, args);
                Some(TypeKind::presentation_handle("View"))
            }
            PresentationCallableId::Menu => {
                self.check_positional_entity_arg(args, 0, &EntityKind::View, "menu mount");
                self.check_presentation_view_named_args(callable, args);
                Some(TypeKind::presentation_handle("Menu"))
            }
            PresentationCallableId::Overlay => {
                self.check_positional_entity_arg(args, 0, &EntityKind::View, "overlay mount");
                self.check_presentation_view_named_args(callable, args);
                Some(TypeKind::presentation_handle("Overlay"))
            }
            PresentationCallableId::Background => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Asset, "bg asset");
                self.check_presentation_background_named_args(args);
                Some(TypeKind::presentation_handle("BackgroundSurface"))
            }
            PresentationCallableId::Image => {
                self.check_presentation_image_source_arg(args);
                self.check_presentation_image_named_args(args);
                Some(TypeKind::presentation_handle("ImageSurface"))
            }
            PresentationCallableId::PlayerViewport => {
                self.check_presentation_viewport_args(callable, args);
                Some(TypeKind::presentation_handle("Viewport"))
            }
            PresentationCallableId::Show => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Character, "show character");
                self.check_presentation_named_args(callable, args, "character");
                Some(TypeKind::presentation_handle("CharacterSurface"))
            }
            PresentationCallableId::RefBackground => {
                self.check_presentation_named_args(callable, args, "background");
                Some(TypeKind::Named("SlotRef<BackgroundSurface>".to_owned()))
            }
            PresentationCallableId::RefShow => {
                self.check_positional_entity_arg(
                    args,
                    0,
                    &EntityKind::Character,
                    "ref show character",
                );
                self.check_presentation_named_args(callable, args, "character");
                Some(TypeKind::Named("SlotRef<CharacterSurface>".to_owned()))
            }
            PresentationCallableId::ClearBackground => {
                self.check_presentation_named_args(callable, args, "background");
                self.active_presentation_defaults.remove("background");
                Some(TypeKind::Named("Option<BackgroundSurface>".to_owned()))
            }
            PresentationCallableId::Hide => {
                self.check_positional_entity_arg(args, 0, &EntityKind::Character, "hide character");
                self.check_presentation_named_args(callable, args, "character");
                self.active_presentation_defaults.remove("character");
                Some(TypeKind::Named("Option<CharacterSurface>".to_owned()))
            }
        }
    }

    fn check_presentation_argument_values(
        &mut self,
        callable: PresentationCallableId,
        args: &[CallArg],
    ) {
        let Ok(schema) = callable.checker_signature_schema() else {
            for arg in args {
                self.check_expr(arg.value());
            }
            return;
        };
        let Some(group) = schema.group(CallableGroupIndex::ZERO) else {
            for arg in args {
                self.check_expr(arg.value());
            }
            return;
        };
        let parameters = group.parameters();
        let mut positional = 0usize;
        for arg in args {
            let parameter = match arg {
                CallArg::Positional(_) => {
                    let parameter = next_presentation_positional_parameter(parameters, positional);
                    if let Some(parameter) = parameter {
                        positional = parameter.index().get() + 1;
                    }
                    parameter
                }
                CallArg::Named { name, .. } => parameters.iter().find(|parameter| {
                    parameter
                        .name()
                        .is_some_and(|candidate| candidate.as_str() == name)
                }),
                CallArg::Spread { .. } => None,
            };
            self.check_presentation_argument_value(
                callable,
                arg,
                parameter,
                schema.argument_policy().unknown_named(),
            );
        }
    }

    fn check_presentation_argument_value(
        &mut self,
        callable: PresentationCallableId,
        arg: &CallArg,
        parameter: Option<&CallableParameter>,
        unknown_named: UnknownNamedArgumentPolicy,
    ) {
        let value = arg.value();
        let owned_policy = match arg {
            CallArg::Named { name, .. } => callable
                .resolve_named_argument(name)
                .map(PresentationNamedArgument::value_policy),
            CallArg::Positional(_) | CallArg::Spread { .. } => None,
        };
        let schema_policy = parameter.map(|parameter| match parameter.ty() {
            CallableParameterType::Exact(expected) => {
                PresentationArgumentValuePolicy::Exact(expected.clone())
            }
            CallableParameterType::Unchecked => PresentationArgumentValuePolicy::Unchecked,
        });
        let policy = match (owned_policy, schema_policy) {
            (
                Some(PresentationArgumentValuePolicy::Unchecked),
                Some(schema @ PresentationArgumentValuePolicy::Exact(_)),
            ) => Some(schema),
            (Some(owned), _) => Some(owned),
            (None, schema) => schema,
        };
        match policy {
            Some(PresentationArgumentValuePolicy::Exact(expected)) => {
                self.check_presentation_exact_argument(callable, arg, parameter, expected, false);
            }
            Some(PresentationArgumentValuePolicy::TokenScalar(expected)) => {
                self.check_presentation_exact_argument(callable, arg, parameter, expected, true);
            }
            Some(PresentationArgumentValuePolicy::Unchecked) => {
                self.check_presentation_unchecked_value(value);
            }
            Some(PresentationArgumentValuePolicy::MetadataScalar) => {
                self.check_presentation_extension_value(value);
            }
            None if matches!(arg, CallArg::Spread { .. })
                || unknown_named == UnknownNamedArgumentPolicy::OpenChecked =>
            {
                self.check_expr(value);
            }
            None => self.check_presentation_unchecked_value(value),
        }
    }

    fn check_presentation_exact_argument(
        &mut self,
        callable: PresentationCallableId,
        argument: &CallArg,
        parameter: Option<&CallableParameter>,
        expected: TypeKind,
        accepts_bare_token: bool,
    ) {
        let value = argument.value();
        if accepts_bare_token && self.is_unresolved_presentation_token(value) {
            self.reserve_presentation_leaf(value);
            return;
        }

        let errors_before = self.errors.len();
        let actual = self.check_expr_with_expected(value, Some(&expected));
        if accepts_bare_token && actual.is_none() && self.errors.len() == errors_before {
            let argument = presentation_argument_label(argument, parameter);
            self.errors.push(TypeCheckError::new(format!(
                "presentation call `{}` argument `{argument}` resolved through the normal expression path but has no value type compatible with {expected:?}",
                callable.surface_name()
            )));
            return;
        }
        if let Some(actual) = actual.as_ref()
            && !self.types_compatible(&expected, actual)
        {
            let argument = presentation_argument_label(argument, parameter);
            self.errors.push(TypeCheckError::argument_type_mismatch(
                callable.surface_name(),
                argument,
                expected,
                actual.clone(),
            ));
        }
    }

    fn check_presentation_unchecked_value(&mut self, value: &Expr) {
        if presentation_unchecked_leaf(value) {
            self.reserve_presentation_leaf(value);
        } else {
            self.check_expr(value);
        }
    }

    fn check_presentation_extension_value(&mut self, value: &Expr) {
        match value {
            Expr::Literal(Literal::Int(literal)) if literal.suffix().is_none() => {
                self.check_expr_with_expected(value, Some(&TypeKind::I64));
            }
            Expr::Literal(Literal::Float { suffix: None, .. }) => {
                self.check_expr_with_expected(value, Some(&TypeKind::F64));
            }
            _ => self.check_presentation_unchecked_value(value),
        }
    }

    fn reserve_presentation_leaf(&mut self, value: &Expr) {
        debug_assert!(presentation_unchecked_leaf(value));
        self.stats.expressions += 1;
    }

    fn is_unresolved_presentation_token(&self, value: &Expr) -> bool {
        match value {
            Expr::ShortVariant(variant) => self.symbol_type(&format!(".{variant}")).is_none(),
            // Bare author tokens are surface sugar only while the path has no
            // normal path resolution. Resolved locals, project functions,
            // builtins, and dotted targets must keep their actual type and pass
            // through normal expected-type checking.
            Expr::Path(path) => !self.path_has_known_resolution(path.as_label()),
            _ => false,
        }
    }

    fn check_presentation_viewport_args(
        &mut self,
        callable: PresentationCallableId,
        args: &[CallArg],
    ) {
        for arg in args {
            match arg {
                CallArg::Named { name, value }
                    if callable.resolve_named_argument(name).is_none() =>
                {
                    self.reject_unknown_presentation_argument("player_viewport", name, value);
                }
                CallArg::Positional(_) | CallArg::Spread { .. } | CallArg::Named { .. } => {}
            }
        }
    }

    fn check_presentation_view_named_args(
        &mut self,
        callable: PresentationCallableId,
        args: &[CallArg],
    ) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
                continue;
            };
            match callable.resolve_named_argument(name) {
                Some(PresentationNamedArgument::Lifetime) => {
                    self.check_presentation_lifetime_arg(value);
                }
                Some(PresentationNamedArgument::TargetPublicId) => {
                    self.check_presentation_image_id_value(value, &EntityKind::Target);
                }
                Some(PresentationNamedArgument::LayerPublicId) => {
                    self.check_presentation_image_id_value(value, &EntityKind::Layer);
                }
                _ => {}
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
            _ => {
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
            _ => {
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
            _ => {
                self.errors.push(TypeCheckError::new(format!(
                    "{context} must be a {expected:?} reference"
                )));
            }
        }
    }

    fn check_presentation_named_args(
        &mut self,
        callable: PresentationCallableId,
        args: &[CallArg],
        slot_family: &str,
    ) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
                continue;
            };
            match callable.resolve_named_argument(name) {
                Some(
                    PresentationNamedArgument::TargetEntity
                    | PresentationNamedArgument::TargetPublicId,
                ) => self.expect_entity_expr_kind(value, &EntityKind::Target, "target"),
                Some(PresentationNamedArgument::Slot) => {
                    self.expect_slot_family(value, slot_family);
                }
                Some(PresentationNamedArgument::Scope) => self.expect_entity_expr_kind(
                    value,
                    &EntityKind::Other("scope".to_owned()),
                    "scope",
                ),
                _ => {}
            }
        }
    }

    fn check_presentation_background_named_args(&mut self, args: &[CallArg]) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
                continue;
            };
            match PresentationCallableId::Background.resolve_named_argument(name) {
                Some(PresentationNamedArgument::TargetEntity) => {
                    self.expect_entity_expr_kind(value, &EntityKind::Target, "target");
                }
                Some(PresentationNamedArgument::Slot) => {
                    self.expect_slot_family(value, "background");
                }
                Some(PresentationNamedArgument::Scope) => self.expect_entity_expr_kind(
                    value,
                    &EntityKind::Other("scope".to_owned()),
                    "scope",
                ),
                Some(_) => {}
                None => self.reject_unknown_presentation_argument("bg", name, value),
            }
        }
    }

    fn check_presentation_image_named_args(&mut self, args: &[CallArg]) {
        for arg in args {
            let CallArg::Named { name, value } = arg else {
                continue;
            };
            match PresentationCallableId::Image.resolve_named_argument(name) {
                Some(PresentationNamedArgument::Lifetime) => {
                    self.check_presentation_lifetime_arg(value);
                }
                Some(PresentationNamedArgument::TargetPublicId) => {
                    self.check_presentation_image_id_value(value, &EntityKind::Target);
                }
                Some(
                    PresentationNamedArgument::LayerPublicId
                    | PresentationNamedArgument::ProxyLayer,
                ) => {
                    self.check_presentation_image_id_value(value, &EntityKind::Layer);
                }
                Some(_) => {}
                None => self.reject_unknown_presentation_argument("image", name, value),
            }
        }
    }

    fn reject_unknown_presentation_argument(
        &mut self,
        command: &str,
        argument: &str,
        _value: &Expr,
    ) {
        self.errors
            .push(TypeCheckError::unknown_presentation_argument(
                command, argument,
            ));
    }

    fn check_presentation_image_id_value(&mut self, expr: &Expr, expected: &EntityKind) {
        if let Expr::EntityRef(entity) = expr {
            match entity.as_absolute().and_then(entity_kind) {
                Some(kind) if &kind == expected => {}
                actual => self.errors.push(TypeCheckError::new(format!(
                    "presentation image id must be a {expected:?} reference or public-id string, found {actual:?}"
                ))),
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
            _ => {
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
            _ => {
                self.errors.push(TypeCheckError::new(
                    "presentation slot must be an entity reference".to_owned(),
                ));
            }
        }
    }
}

fn next_presentation_positional_parameter(
    parameters: &[CallableParameter],
    start: usize,
) -> Option<&CallableParameter> {
    parameters.iter().skip(start).find(|parameter| {
        matches!(
            parameter.passing(),
            CallableParameterPassing::PositionalOnly
                | CallableParameterPassing::PositionalOrNamed
                | CallableParameterPassing::RestPositional
        )
    })
}

fn presentation_argument_label(
    argument: &CallArg,
    parameter: Option<&CallableParameter>,
) -> String {
    match argument {
        CallArg::Named { name, .. } => name.clone(),
        CallArg::Positional(_) | CallArg::Spread { .. } => {
            parameter.and_then(CallableParameter::name).map_or_else(
                || "<positional>".to_owned(),
                |name| name.as_str().to_owned(),
            )
        }
    }
}

fn presentation_unchecked_leaf(value: &Expr) -> bool {
    matches!(
        value,
        Expr::EntityRef(_)
            | Expr::Path(_)
            | Expr::ShortVariant(_)
            | Expr::Literal(Literal::String(_))
    )
}
