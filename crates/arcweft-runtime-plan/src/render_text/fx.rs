//! Typed applications of compiled Fx graphs inside `RichText` spans.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    fx::FxConst,
    model::HirModule,
    syntax::{
        ast::dialogue::DialogueTag,
        expr::{CallArg, Expr, parse_expr},
    },
};
use arcweft_presentation::fx::{FxApplication, FxDefinition, FxRuntimeValue, FxSourceRange};

use crate::{
    errors::RuntimePlanLowerError,
    fx::{closed_expr_to_fx_value, lower_fx_definitions_for_package},
};

pub(crate) mod builtins;
mod contributions;
mod expander;

#[cfg(test)]
mod tests;

pub(crate) use contributions::{FxInlineAssignment, append_fx_inline_contributions};
pub(crate) use expander::DialogueFxExpander;

/// Module-scoped compiled Fx definitions used by dialogue lowering.
#[derive(Clone, Debug, Default)]
pub(crate) struct FxCatalog {
    definitions: BTreeMap<String, FxDefinition>,
    definitions_by_id: BTreeMap<arcweft_presentation::fx::FxId, FxDefinition>,
}

impl FxCatalog {
    #[cfg(test)]
    pub(crate) fn try_from_module(module: &HirModule) -> Result<Self, RuntimePlanLowerError> {
        Self::try_from_module_for_package(module, "crate")
    }

    pub(crate) fn try_from_module_for_package(
        module: &HirModule,
        package: &str,
    ) -> Result<Self, RuntimePlanLowerError> {
        let mut definitions = BTreeMap::new();
        let mut definitions_by_id = BTreeMap::new();
        for definition in lower_fx_definitions_for_package(module, package)? {
            if definitions_by_id
                .insert(definition.id().clone(), definition.clone())
                .is_some()
            {
                return Err(fx_error(format!(
                    "duplicate Fx definition `{}`",
                    definition.id()
                )));
            }
            if definition.id().package() != package {
                continue;
            }
            let name = definition
                .id()
                .function()
                .rsplit('.')
                .next()
                .unwrap_or(definition.id().function())
                .to_owned();
            if definitions.insert(name.clone(), definition).is_some() {
                return Err(fx_error(format!("duplicate Fx function `{name}`")));
            }
        }
        Ok(Self {
            definitions,
            definitions_by_id,
        })
    }

    fn bind_tag(
        &self,
        tag: &DialogueTag,
        authored_ordinal: u32,
    ) -> Result<(String, FxApplication), RuntimePlanLowerError> {
        let expr = parse_expr(tag.attrs().trim())
            .map_err(|error| fx_error(format!("invalid `[fx]` invocation: {error}")))?;
        let Expr::Call { callee, args } = expr else {
            return Err(fx_error("`[fx]` requires one Fx function call"));
        };
        let name = callee_name(&callee)
            .ok_or_else(|| fx_error("`[fx]` target must be a canonical function path"))?;
        let definition = self
            .definitions
            .get(name)
            .ok_or_else(|| fx_error(format!("unknown Fx function `{name}`")))?;
        let parameters = bind_invocation(definition, &args)?;
        let range = tag.attrs_range();
        let start = u32::try_from(range.start())
            .map_err(|_| fx_error("Fx source range start exceeds u32"))?;
        let end =
            u32::try_from(range.end()).map_err(|_| fx_error("Fx source range end exceeds u32"))?;
        let source_range = FxSourceRange::try_new(start, end).map_err(fx_error)?;
        let application = FxApplication::try_new(
            definition.id().clone(),
            parameters,
            authored_ordinal,
            Some(source_range),
        )
        .map_err(|error| fx_error(error.to_string()))?;
        Ok((name.to_owned(), application))
    }

    fn bind_builtin(
        &self,
        selector: &str,
        attrs: &str,
        tag: &DialogueTag,
        authored_ordinal: u32,
    ) -> Result<Option<(String, FxApplication, Option<FxDefinition>)>, RuntimePlanLowerError> {
        let compiled = builtins::compile_builtin_rich_text_fx(selector, attrs)?;
        let (id, definition) = match compiled {
            builtins::BuiltinRichTextFx::HostEvent => return Ok(None),
            builtins::BuiltinRichTextFx::Definition(definition) => {
                let retained = self.definitions_by_id.get(definition.id()).ok_or_else(|| {
                    fx_error(format!(
                        "bundled definition `{}` was not collected from the dialogue inventory",
                        definition.id()
                    ))
                })?;
                if retained != &definition {
                    return Err(fx_error(format!(
                        "bundled definition `{}` changed between inventory and line lowering",
                        definition.id()
                    )));
                }
                (definition.id().clone(), Some(definition))
            }
            builtins::BuiltinRichTextFx::MissingDefinition(id) => (id, None),
        };
        let range = tag.attrs_range();
        let start = u32::try_from(range.start())
            .map_err(|_| fx_error("Fx source range start exceeds u32"))?;
        let end =
            u32::try_from(range.end()).map_err(|_| fx_error("Fx source range end exceeds u32"))?;
        let source_range = FxSourceRange::try_new(start, end).map_err(fx_error)?;
        let application =
            FxApplication::try_new(id, Vec::new(), authored_ordinal, Some(source_range))
                .map_err(|error| fx_error(error.to_string()))?;
        Ok(Some((selector.to_owned(), application, definition)))
    }
}

fn bind_invocation(
    definition: &FxDefinition,
    args: &[CallArg],
) -> Result<Vec<FxRuntimeValue>, RuntimePlanLowerError> {
    let mut supplied = BTreeMap::new();
    for arg in args {
        let CallArg::Named { name, value } = arg else {
            return Err(fx_error(format!(
                "Fx function `{}` accepts named arguments only",
                definition.id().function()
            )));
        };
        if !rich_text_value_is_closed(value) {
            return Err(fx_error(format!(
                "RichText Fx argument `{name}` must be closed, found `{}`",
                crate::labels::expr_label(value)
            )));
        }
        if supplied.insert(name.clone(), value.as_ref()).is_some() {
            return Err(fx_error(format!(
                "Fx function `{}` receives duplicate argument `{name}`",
                definition.id().function()
            )));
        }
    }
    let mut bindings = Vec::with_capacity(definition.parameters().len());
    for parameter in definition.parameters() {
        let value = supplied.remove(parameter.name()).map_or_else(
            || {
                parameter.default().copied().ok_or_else(|| {
                    fx_error(format!(
                        "Fx function `{}` is missing required argument `{}`",
                        definition.id().function(),
                        parameter.name()
                    ))
                })
            },
            |expr| closed_expr_to_fx_value(expr, parameter.value_type()),
        )?;
        bindings.push(value);
    }
    if let Some(unknown) = supplied.keys().next() {
        return Err(fx_error(format!(
            "Fx function `{}` has no parameter named `{unknown}`",
            definition.id().function()
        )));
    }
    Ok(bindings)
}

fn callee_name(expr: &Expr) -> Option<&str> {
    let Expr::Path(path) = expr else {
        return None;
    };
    path.segments()
        .last()
        .map(arcweft_lang_hir::syntax::expr::Name::as_str)
}

fn rich_text_value_is_closed(expr: &Expr) -> bool {
    if FxConst::from_expr(expr).is_some() {
        return true;
    }
    matches!(
        expr,
        Expr::Call { callee, args }
            if matches!(callee_name(callee), Some("rgb" | "vec2" | "vec3" | "vec4"))
                && args.iter().all(|arg| {
                    !matches!(arg, CallArg::Spread { .. })
                        && rich_text_value_is_closed(arg.value())
                })
    )
}

fn fx_error(message: impl Into<String>) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(format!("rich-text Fx: {}", message.into()))
}
