//! Compile-time expansion of reusable rich-text decoration declarations.
//!
//! Decorations are an authoring abstraction. This module resolves their
//! closed arguments and expands them to the existing `RichTextStyle` stack;
//! no declaration or invocation survives into bundle or session-save data.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::decoration::{
    DECORATION_EXPANSION_LIMITS, DecorationBuilderKind, DecorationBuilderShape, DecorationConst,
    DecorationConstKind, DecorationExpansionLimits,
};
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_hir::syntax::{
    ast::{
        common::TextRange,
        decoration::DecorationItem,
        dialogue::{DialogueTag, DialogueTagArg},
    },
    cst::is_identifier,
    expr::{CallArg, Expr, parse_expr},
};
use arcweft_render_text::RichTextStyle;

use crate::{errors::RuntimePlanLowerError, labels::expr_label};

use super::tag::lower_visual_decoration_layer;

mod budget;
mod contributions;
mod expander;

use budget::DecorationExpansionState;
pub(crate) use contributions::{
    DecorationInlineAssignment, append_decoration_inline_contributions,
};
pub(crate) use expander::DialogueDecorationExpander;

/// Module-scoped inventory of reusable rich-text decorations.
#[derive(Clone, Debug)]
pub(crate) struct DecorationCatalog {
    definitions: BTreeMap<String, DecorationItem>,
    limits: DecorationExpansionLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecorationValue {
    Authored {
        source: String,
        invocation_range: Option<TextRange>,
    },
    Unbound,
}

type NamedDecorationValues = Vec<(String, DecorationValue)>;

#[derive(Clone, Debug)]
struct DecorationBindings {
    fixed: BTreeMap<String, DecorationValue>,
    rest_name: Option<String>,
    rest: BTreeMap<String, DecorationValue>,
}

#[derive(Clone, Debug)]
struct ExpandedDecorationLayer {
    style: RichTextStyle,
    builder: DecorationBuilderKind,
    selector: Option<String>,
    attrs: String,
    arguments: Vec<ExpandedDecorationArgument>,
}

#[derive(Clone, Debug)]
struct ExpandedDecorationArgument {
    name: String,
    value: String,
    invocation_range: Option<TextRange>,
}

impl Default for DecorationCatalog {
    fn default() -> Self {
        Self {
            definitions: BTreeMap::new(),
            limits: DECORATION_EXPANSION_LIMITS,
        }
    }
}

impl DecorationCatalog {
    /// Builds and validates the complete declaration inventory for one HIR
    /// module. Validation is eager so an unused malformed declaration cannot
    /// become a latent runtime-plan failure.
    pub(crate) fn try_from_module(module: &HirModule) -> Result<Self, RuntimePlanLowerError> {
        Self::try_from_module_with_limits(module, DECORATION_EXPANSION_LIMITS)
    }

    fn try_from_module_with_limits(
        module: &HirModule,
        limits: DecorationExpansionLimits,
    ) -> Result<Self, RuntimePlanLowerError> {
        let mut definitions = BTreeMap::new();
        for declaration in module.declarations() {
            let HirTopLevelDecl::Decoration(item) = declaration else {
                continue;
            };
            if definitions
                .insert(item.name().to_owned(), item.clone())
                .is_some()
            {
                return Err(decoration_error(format!(
                    "duplicate rich-text decoration declaration `{}`",
                    item.name()
                )));
            }
        }

        let catalog = Self {
            definitions,
            limits,
        };
        for definition in catalog.definitions.values() {
            Self::validate_parameter_shape(definition)?;
            if definition.layers().is_empty() {
                return Err(decoration_error(format!(
                    "rich-text decoration `{}` must contain at least one visual layer",
                    definition.name()
                )));
            }
            let bindings = Self::validation_bindings(definition)?;
            let mut expansion = DecorationExpansionState::new(catalog.limits);
            let _ = catalog.expand_definition(definition.name(), &bindings, &mut expansion)?;
        }
        Ok(catalog)
    }

    fn validate_parameter_shape(definition: &DecorationItem) -> Result<(), RuntimePlanLowerError> {
        let mut names = BTreeSet::new();
        let mut rest_seen = false;
        for (index, parameter) in definition.params().iter().enumerate() {
            if !names.insert(parameter.name()) {
                return Err(decoration_error(format!(
                    "rich-text decoration `{}` declares duplicate parameter `{}`",
                    definition.name(),
                    parameter.name()
                )));
            }
            if parameter.is_rest() {
                if rest_seen {
                    return Err(decoration_error(format!(
                        "rich-text decoration `{}` declares more than one rest parameter",
                        definition.name()
                    )));
                }
                if index + 1 != definition.params().len() {
                    return Err(decoration_error(format!(
                        "rich-text decoration `{}` rest parameter `{}` must be last",
                        definition.name(),
                        parameter.name()
                    )));
                }
                if parameter.default().is_some() {
                    return Err(decoration_error(format!(
                        "rich-text decoration `{}` rest parameter `{}` cannot have a default",
                        definition.name(),
                        parameter.name()
                    )));
                }
                rest_seen = true;
            } else if let Some(default) = parameter.default() {
                let _ = closed_expr_value(
                    default,
                    None,
                    &format!(
                        "default for decoration `{}.{}`",
                        definition.name(),
                        parameter.name()
                    ),
                )?;
            }
        }
        Ok(())
    }

    fn validation_bindings(
        definition: &DecorationItem,
    ) -> Result<DecorationBindings, RuntimePlanLowerError> {
        let mut fixed = BTreeMap::new();
        let mut rest_name = None;
        for parameter in definition.params() {
            if parameter.is_rest() {
                rest_name = Some(parameter.name().to_owned());
                continue;
            }
            let value = parameter.default().map_or_else(
                || Ok(DecorationValue::Unbound),
                |default| {
                    closed_expr_value(
                        default,
                        None,
                        &format!(
                            "default for decoration `{}.{}`",
                            definition.name(),
                            parameter.name()
                        ),
                    )
                },
            )?;
            fixed.insert(parameter.name().to_owned(), value);
        }
        Ok(DecorationBindings {
            fixed,
            rest_name,
            rest: BTreeMap::new(),
        })
    }

    fn expand_tag(
        &self,
        tag: &DialogueTag,
    ) -> Result<Vec<ExpandedDecorationLayer>, RuntimePlanLowerError> {
        let mut selector = None;
        let mut named = Vec::new();
        let mut seen_names = BTreeSet::new();
        for (index, argument) in tag.arguments().iter().enumerate() {
            match argument {
                DialogueTagArg::Positional { value } if index == 0 && selector.is_none() => {
                    if value.source().trim() != value.value() {
                        return Err(decoration_error(format!(
                            "decoration invocation selector `{}` must use unquoted `.name` syntax",
                            value.source()
                        )));
                    }
                    selector = Some(decoration_selector(value.value(), "decoration invocation")?);
                }
                DialogueTagArg::Positional { .. } => {
                    return Err(decoration_error(
                        "`[decorate]` accepts one leading `.name` selector and named arguments only",
                    ));
                }
                DialogueTagArg::Named { name, value, .. } => {
                    if !is_identifier(name) {
                        return Err(decoration_error(format!(
                            "decoration invocation argument name `{name}` must be one canonical identifier"
                        )));
                    }
                    if !seen_names.insert(name.as_str()) {
                        return Err(decoration_error(format!(
                            "decoration invocation supplies duplicate argument `{name}`"
                        )));
                    }
                    named.push((
                        name.clone(),
                        closed_source_value(
                            value.source(),
                            value.range(),
                            &format!("decoration invocation argument `{name}`"),
                        )?,
                    ));
                }
            }
        }
        let name = selector.ok_or_else(|| {
            decoration_error("`[decorate]` requires one leading `.name` selector")
        })?;
        let definition = self.definition(&name)?;
        let bindings = Self::bind_definition(definition, named)?;
        self.expand_definition(
            &name,
            &bindings,
            &mut DecorationExpansionState::new(self.limits),
        )
    }

    fn definition(&self, name: &str) -> Result<&DecorationItem, RuntimePlanLowerError> {
        self.definitions
            .get(name)
            .ok_or_else(|| decoration_error(format!("unknown rich-text decoration `.{name}`")))
    }

    fn bind_definition(
        definition: &DecorationItem,
        arguments: NamedDecorationValues,
    ) -> Result<DecorationBindings, RuntimePlanLowerError> {
        let mut supplied = BTreeMap::new();
        for (name, value) in arguments {
            if supplied.insert(name.clone(), value).is_some() {
                return Err(decoration_error(format!(
                    "decoration `.{}` receives duplicate argument `{name}`",
                    definition.name()
                )));
            }
        }

        let rest_name = definition
            .params()
            .iter()
            .find(|parameter| parameter.is_rest())
            .map(|parameter| parameter.name().to_owned());
        let fixed_names = definition
            .params()
            .iter()
            .filter(|parameter| !parameter.is_rest())
            .map(arcweft_lang_hir::syntax::ast::decoration::DecorationParam::name)
            .collect::<BTreeSet<_>>();
        let unknown = supplied
            .keys()
            .filter(|name| !fixed_names.contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if rest_name.is_none()
            && let Some(name) = unknown.first()
        {
            return Err(decoration_error(format!(
                "decoration `.{}` has no argument named `{name}`",
                definition.name()
            )));
        }

        let mut fixed = BTreeMap::new();
        for parameter in definition
            .params()
            .iter()
            .filter(|parameter| !parameter.is_rest())
        {
            let value = supplied.remove(parameter.name()).map_or_else(
                || {
                    parameter.default().map_or_else(
                        || {
                            Err(decoration_error(format!(
                                "decoration `.{}` is missing required argument `{}`",
                                definition.name(),
                                parameter.name()
                            )))
                        },
                        |default| {
                            closed_expr_value(
                                default,
                                None,
                                &format!(
                                    "default for decoration `{}.{}`",
                                    definition.name(),
                                    parameter.name()
                                ),
                            )
                        },
                    )
                },
                Ok,
            )?;
            fixed.insert(parameter.name().to_owned(), value);
        }
        Ok(DecorationBindings {
            fixed,
            rest_name,
            rest: supplied,
        })
    }

    fn expand_definition(
        &self,
        name: &str,
        bindings: &DecorationBindings,
        expansion: &mut DecorationExpansionState,
    ) -> Result<Vec<ExpandedDecorationLayer>, RuntimePlanLowerError> {
        let definition = self.definition(name)?;
        expansion.enter(name)?;
        let result = definition
            .layers()
            .iter()
            .try_fold(Vec::new(), |mut styles, layer| {
                styles.extend(self.expand_layer(definition, layer.expr(), bindings, expansion)?);
                Ok::<_, RuntimePlanLowerError>(styles)
            });
        expansion.leave(name);
        result
    }

    fn expand_layer(
        &self,
        definition: &DecorationItem,
        expr: &Expr,
        bindings: &DecorationBindings,
        expansion: &mut DecorationExpansionState,
    ) -> Result<Vec<ExpandedDecorationLayer>, RuntimePlanLowerError> {
        let Expr::Call { callee, args } = expr else {
            return Err(layer_error(
                definition,
                "body layers must be builder calls such as `strong()` or `effect(.wave, ...)`",
            ));
        };
        let Expr::Path(builder_path) = callee.as_ref() else {
            return Err(layer_error(
                definition,
                "decoration layer builder must be an unqualified name",
            ));
        };
        let builder = builder_path.as_label();
        if builder_path.segments().len() != 1 {
            return Err(layer_error(
                definition,
                "decoration layer builder must be an unqualified name",
            ));
        }
        let Some(builder_kind) = DecorationBuilderKind::from_name(builder) else {
            return Err(layer_error(
                definition,
                format!(
                    "unsupported decoration layer builder `{builder}`; visual decorations may use em, strong, color, font, size, style, layout, transform, effect, or decorate"
                ),
            ));
        };
        if builder_kind == DecorationBuilderKind::Decorate {
            return self.expand_nested_decoration(definition, args, bindings, expansion);
        }
        let (selector, named) =
            Self::resolve_visual_builder_args(definition, builder, builder_kind, args, bindings)?;
        let attrs = attrs_source(builder_kind, &named);
        let style = lower_visual_decoration_layer(builder, selector.as_deref(), &attrs)
            .map_err(|reason| layer_error(definition, reason))?;
        expansion.record_layer(definition.name())?;
        let arguments = expanded_decoration_arguments(builder_kind, &named);
        Ok(vec![ExpandedDecorationLayer {
            style,
            builder: builder_kind,
            selector,
            attrs,
            arguments,
        }])
    }

    fn resolve_visual_builder_args(
        definition: &DecorationItem,
        builder: &str,
        builder_kind: DecorationBuilderKind,
        args: &[CallArg],
        bindings: &DecorationBindings,
    ) -> Result<(Option<String>, NamedDecorationValues), RuntimePlanLowerError> {
        let shape = builder_kind.shape();
        if shape == DecorationBuilderShape::Empty {
            if args.is_empty() {
                return Ok((None, Vec::new()));
            }
            return Err(layer_error(
                definition,
                format!("decoration builder `{builder}` does not accept arguments"),
            ));
        }
        if shape == DecorationBuilderShape::Scalar {
            return Self::resolve_scalar_builder_arg(definition, builder, args, bindings);
        }

        let mut selector = None;
        let mut named = Vec::new();
        let mut seen = BTreeSet::new();
        let mut spread_used = false;
        for (index, argument) in args.iter().enumerate() {
            match argument {
                CallArg::Positional(value) if index == 0 => {
                    selector = Some(short_variant_selector(value, definition, builder)?);
                }
                CallArg::Positional(_) => {
                    return Err(layer_error(
                        definition,
                        format!(
                            "decoration builder `{builder}` does not accept this positional argument"
                        ),
                    ));
                }
                CallArg::Named { name, value } => {
                    if !seen.insert(name.clone()) {
                        return Err(layer_error(
                            definition,
                            format!("decoration builder `{builder}` repeats argument `{name}`"),
                        ));
                    }
                    named.push((
                        name.clone(),
                        closed_expr_value(
                            value,
                            Some(bindings),
                            &format!(
                                "decoration `{}` {builder} argument `{name}`",
                                definition.name()
                            ),
                        )?,
                    ));
                }
                CallArg::Spread { value } => {
                    if spread_used {
                        return Err(layer_error(
                            definition,
                            format!(
                                "decoration builder `{builder}` may spread its rest parameter at most once"
                            ),
                        ));
                    }
                    spread_used = true;
                    let spread = rest_spread(bindings, value, definition, builder)?;
                    for (name, value) in spread {
                        if !seen.insert(name.clone()) {
                            return Err(layer_error(
                                definition,
                                format!(
                                    "decoration builder `{builder}` receives `{name}` both explicitly and through the rest bag"
                                ),
                            ));
                        }
                        named.push((name, value));
                    }
                }
            }
        }

        let Some(selector) = selector else {
            return Err(layer_error(
                definition,
                format!(
                    "decoration builder `{builder}` requires a leading selector such as `.wave`"
                ),
            ));
        };
        if shape == DecorationBuilderShape::ClosedSelector
            && !builder_kind.supports_selector(&selector)
        {
            return Err(layer_error(
                definition,
                format!("decoration builder `{builder}` does not support selector `.{selector}`"),
            ));
        }
        Ok((Some(selector), named))
    }

    fn resolve_scalar_builder_arg(
        definition: &DecorationItem,
        builder: &str,
        args: &[CallArg],
        bindings: &DecorationBindings,
    ) -> Result<(Option<String>, NamedDecorationValues), RuntimePlanLowerError> {
        let value = match args {
            [CallArg::Positional(value)] => value,
            [CallArg::Named { name, value }] if name == "value" => value,
            _ => {
                return Err(layer_error(
                    definition,
                    format!("decoration builder `{builder}` requires exactly one `value` argument"),
                ));
            }
        };
        Ok((
            None,
            vec![(
                String::new(),
                closed_expr_value(
                    value,
                    Some(bindings),
                    &format!("decoration `{}` {builder} value", definition.name()),
                )?,
            )],
        ))
    }

    fn expand_nested_decoration(
        &self,
        owner: &DecorationItem,
        args: &[CallArg],
        bindings: &DecorationBindings,
        expansion: &mut DecorationExpansionState,
    ) -> Result<Vec<ExpandedDecorationLayer>, RuntimePlanLowerError> {
        let mut selector = None;
        let mut named = Vec::new();
        let mut seen = BTreeSet::new();
        let mut spread_used = false;
        for (index, argument) in args.iter().enumerate() {
            match argument {
                CallArg::Positional(value) if index == 0 && selector.is_none() => {
                    selector = Some(short_variant_selector(value, owner, "decorate")?);
                }
                CallArg::Positional(_) => {
                    return Err(layer_error(
                        owner,
                        "nested `decorate` accepts one leading selector and named arguments only",
                    ));
                }
                CallArg::Named { name, value } => {
                    if !seen.insert(name.clone()) {
                        return Err(layer_error(
                            owner,
                            format!("nested decoration repeats argument `{name}`"),
                        ));
                    }
                    named.push((
                        name.clone(),
                        closed_expr_value(
                            value,
                            Some(bindings),
                            &format!("nested decoration argument `{name}` in `{}`", owner.name()),
                        )?,
                    ));
                }
                CallArg::Spread { value } => {
                    if spread_used {
                        return Err(layer_error(
                            owner,
                            "nested `decorate` may spread its rest parameter at most once",
                        ));
                    }
                    spread_used = true;
                    for (name, value) in rest_spread(bindings, value, owner, "decorate")? {
                        if !seen.insert(name.clone()) {
                            return Err(layer_error(
                                owner,
                                format!(
                                    "nested decoration receives `{name}` both explicitly and through the rest bag"
                                ),
                            ));
                        }
                        named.push((name, value));
                    }
                }
            }
        }
        let name = selector.ok_or_else(|| {
            layer_error(
                owner,
                "nested `decorate` requires a leading `.name` selector",
            )
        })?;
        let target = self.definition(&name)?;
        if spread_used
            && !target
                .params()
                .iter()
                .any(arcweft_lang_hir::syntax::ast::decoration::DecorationParam::is_rest)
        {
            return Err(layer_error(
                owner,
                format!(
                    "nested decoration `.{name}` cannot receive a forwarded rest bag because it declares no rest parameter"
                ),
            ));
        }
        let target_bindings = Self::bind_definition(target, named)?;
        self.expand_definition(&name, &target_bindings, expansion)
    }
}

impl DecorationValue {
    fn authored(value: impl Into<String>) -> Self {
        Self::Authored {
            source: value.into(),
            invocation_range: None,
        }
    }

    fn invocation(value: impl Into<String>, range: TextRange) -> Self {
        Self::Authored {
            source: value.into(),
            invocation_range: Some(range),
        }
    }

    fn source(&self) -> Option<&str> {
        match self {
            Self::Authored { source, .. } => Some(source),
            Self::Unbound => None,
        }
    }

    const fn invocation_range(&self) -> Option<TextRange> {
        match self {
            Self::Authored {
                invocation_range, ..
            } => *invocation_range,
            Self::Unbound => None,
        }
    }
}

fn closed_source_value(
    source: &str,
    range: TextRange,
    context: &str,
) -> Result<DecorationValue, RuntimePlanLowerError> {
    let source = source.trim();
    if matching_outer_quote(source) {
        return Ok(DecorationValue::invocation(source, range));
    }
    let expression_error = if let Ok(expr) = parse_expr(source) {
        match closed_expr_value(&expr, None, context) {
            Ok(_) => {
                // Preserve quotes and exact unit spelling from dialogue-tag source.
                return Ok(DecorationValue::invocation(source, range));
            }
            Err(error) => Some(error),
        }
    } else {
        None
    };
    if safe_raw_decoration_token(source) {
        return Ok(DecorationValue::invocation(source, range));
    }
    if let Some(error) = expression_error {
        return Err(error);
    }
    Err(decoration_error(format!(
        "{context} must be a closed literal, selector, or safe raw token; found `{source}`"
    )))
}

fn closed_expr_value(
    expr: &Expr,
    bindings: Option<&DecorationBindings>,
    context: &str,
) -> Result<DecorationValue, RuntimePlanLowerError> {
    let Some(value) = DecorationConst::from_expr(expr) else {
        return Err(decoration_error(format!(
            "{context} must be a closed literal, selector, or raw identifier token; runtime expression `{}` is not allowed",
            expr_label(expr)
        )));
    };
    match value.kind() {
        DecorationConstKind::Identifier => {
            let Expr::Path(path) = value.expr() else {
                unreachable!("identifier decoration constants are path expressions")
            };
            let name = path.as_label();
            if let Some(bindings) = bindings {
                if let Some(value) = bindings.fixed.get(name) {
                    return Ok(value.clone());
                }
                if bindings.rest_name.as_deref() == Some(name) {
                    return Err(decoration_error(format!(
                        "{context} must spread rest parameter `{name}...` instead of using it as one value"
                    )));
                }
                return Err(decoration_error(format!(
                    "{context} references unknown decoration parameter `{name}`; raw builder values must be quoted or use selector syntax"
                )));
            }
            Ok(DecorationValue::authored(name))
        }
        DecorationConstKind::Literal
        | DecorationConstKind::SignedNumber
        | DecorationConstKind::Selector => Ok(DecorationValue::authored(expr_label(expr))),
    }
}

fn safe_raw_decoration_token(source: &str) -> bool {
    if source.is_empty()
        || source.contains("#[")
        || source.contains("$(")
        || source.chars().any(char::is_whitespace)
        || source.chars().any(|ch| {
            matches!(
                ch,
                '(' | ')' | '[' | ']' | '{' | '}' | '+' | '*' | '/' | '\'' | '"'
            )
        })
    {
        return false;
    }
    !looks_like_dotted_runtime_path(source)
}

fn matching_outer_quote(source: &str) -> bool {
    source.len() >= 2
        && ((source.starts_with('"') && source.ends_with('"'))
            || (source.starts_with('\'') && source.ends_with('\'')))
}

fn looks_like_dotted_runtime_path(source: &str) -> bool {
    let mut segments = source.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let rest = segments.collect::<Vec<_>>();
    !rest.is_empty() && is_identifier(first) && rest.iter().all(|segment| is_identifier(segment))
}

fn rest_spread(
    bindings: &DecorationBindings,
    value: &Expr,
    definition: &DecorationItem,
    builder: &str,
) -> Result<Vec<(String, DecorationValue)>, RuntimePlanLowerError> {
    let Expr::Path(path) = value else {
        return Err(layer_error(
            definition,
            format!("`{builder}` spread must name the declaration's rest parameter"),
        ));
    };
    let name = path.as_label();
    if path.segments().len() != 1 || bindings.rest_name.as_deref() != Some(name) {
        return Err(layer_error(
            definition,
            format!("`{builder}` may spread only the declaration rest parameter"),
        ));
    }
    Ok(bindings
        .rest
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect())
}

fn short_variant_selector(
    expr: &Expr,
    definition: &DecorationItem,
    builder: &str,
) -> Result<String, RuntimePlanLowerError> {
    let Expr::ShortVariant(name) = expr else {
        return Err(layer_error(
            definition,
            format!("decoration builder `{builder}` selector must use `.name` syntax"),
        ));
    };
    decoration_selector(
        &format!(".{name}"),
        &format!("decoration builder `{builder}`"),
    )
}

fn decoration_selector(source: &str, context: &str) -> Result<String, RuntimePlanLowerError> {
    let Some(name) = source.trim().strip_prefix('.') else {
        return Err(decoration_error(format!(
            "{context} selector must use `.name` syntax"
        )));
    };
    if !is_identifier(name) {
        return Err(decoration_error(format!(
            "{context} has invalid decoration selector `{source}`"
        )));
    }
    Ok(name.to_owned())
}

fn attrs_source(builder: DecorationBuilderKind, values: &[(String, DecorationValue)]) -> String {
    values
        .iter()
        .filter_map(|(name, value)| {
            let source = value.source()?;
            if name.is_empty() {
                Some(source.to_owned())
            } else {
                let source = rendered_decoration_value_source(builder, name, source);
                Some(format!("{name}={source}"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn expanded_decoration_arguments(
    builder: DecorationBuilderKind,
    values: &[(String, DecorationValue)],
) -> Vec<ExpandedDecorationArgument> {
    values
        .iter()
        .filter_map(|(name, value)| {
            let source = value.source()?;
            let source = rendered_decoration_value_source(builder, name, source);
            Some(ExpandedDecorationArgument {
                name: name.clone(),
                value: semantic_decoration_value(source).to_owned(),
                invocation_range: value.invocation_range(),
            })
        })
        .collect()
}

fn rendered_decoration_value_source<'a>(
    builder: DecorationBuilderKind,
    name: &str,
    source: &'a str,
) -> &'a str {
    if decoration_metadata_selector(builder, name) {
        source.strip_prefix('.').unwrap_or(source)
    } else {
        source
    }
}

fn semantic_decoration_value(source: &str) -> &str {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(source)
}

fn decoration_metadata_selector(builder: DecorationBuilderKind, name: &str) -> bool {
    match builder {
        DecorationBuilderKind::Effect => matches!(
            name,
            "phase" | "target" | "state" | "scope" | "state_scope" | "origin"
        ),
        DecorationBuilderKind::Transform => matches!(name, "target" | "origin"),
        DecorationBuilderKind::Layout => matches!(
            name,
            "mode" | "dir" | "direction" | "latin" | "jlreq" | "strictness" | "kinsoku"
        ),
        _ => false,
    }
}

fn layer_error(
    definition: &DecorationItem,
    reason: impl std::fmt::Display,
) -> RuntimePlanLowerError {
    decoration_error(format!(
        "invalid layer in rich-text decoration `{}`: {reason}",
        definition.name()
    ))
}

pub(super) fn decoration_error(message: impl Into<String>) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(message)
}

#[cfg(test)]
mod tests;
