//! Semantic inventory and validation for reusable rich-text decorations.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use arcweft_lang_hir::{
    decoration::{DecorationBuilderKind, DecorationBuilderShape, DecorationConst},
    model::{HirModule, HirTopLevelDecl},
};
use arcweft_lang_syntax::{
    ast::{decoration::DecorationItem, dialogue::DialogueTag},
    cst::is_identifier,
    expr::{CallArg, Expr, Literal, Name, parse_expr},
    text::inferred_rich_text_tag_family,
};

use crate::diagnostics::TypeCheckError;

mod expansion;
mod span;

pub(super) use span::DecorationSpanState;

/// Module-local decoration signatures used by dialogue-content validation.
#[derive(Clone, Debug, Default)]
pub(super) struct DecorationCatalog {
    definitions: BTreeMap<String, DecorationDefinition>,
    text_proxy_types: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct DecorationDefinition {
    item: DecorationItem,
    params: Vec<DecorationParameter>,
    rest: Option<String>,
    dependencies: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct DecorationParameter {
    name: String,
    default: Option<DecorationValue>,
}

#[derive(Clone, Debug)]
struct DecorationValue {
    value: String,
    bound: bool,
}

#[derive(Clone, Debug)]
struct DecorationBindings {
    fixed: BTreeMap<String, DecorationValue>,
    rest_name: Option<String>,
    rest: BTreeMap<String, DecorationValue>,
}

impl DecorationCatalog {
    pub(super) fn from_module(module: &HirModule, errors: &mut Vec<TypeCheckError>) -> Self {
        let decorations = module
            .declarations()
            .iter()
            .filter_map(|declaration| match declaration {
                HirTopLevelDecl::Decoration(item) => Some(item),
                _ => None,
            })
            .collect::<Vec<_>>();
        let text_proxy_types = module
            .declarations()
            .iter()
            .filter_map(|declaration| match declaration {
                HirTopLevelDecl::Struct(item)
                    if item
                        .attrs()
                        .iter()
                        .any(|attr| matches!(attr.name(), "text_proxy" | "rich_text_proxy")) =>
                {
                    Some(item.name().to_owned())
                }
                _ => None,
            })
            .collect();
        let mut catalog = Self {
            definitions: BTreeMap::new(),
            text_proxy_types,
        };

        for item in &decorations {
            match catalog.definitions.entry(item.name().to_owned()) {
                Entry::Vacant(entry) => {
                    entry.insert(decoration_signature(item, errors));
                }
                Entry::Occupied(_) => {
                    errors.push(TypeCheckError::new(format!(
                        "duplicate decoration declaration `{}`",
                        item.name()
                    )));
                }
            }
        }

        let mut validated = BTreeSet::new();
        for item in decorations {
            // Duplicate declarations were already rejected. Validate only the
            // first body so a discarded duplicate cannot alter the graph.
            if !validated.insert(item.name()) {
                continue;
            }
            let dependencies = validate_decoration_body(item, &catalog, errors);
            if let Some(definition) = catalog.definitions.get_mut(item.name()) {
                definition.dependencies.extend(dependencies);
            }
        }

        catalog.report_cycles(errors);
        catalog.validate_default_expansions(errors);
        catalog
    }

    /// Validates one `[decorate .name ...]` opening tag and returns its name for
    /// unclosed-span diagnostics.
    pub(super) fn validate_dialogue_tag(
        &self,
        tag: &DialogueTag,
        errors: &mut Vec<TypeCheckError>,
    ) -> String {
        let arguments = tag.arguments();
        let Some(selector_arg) = arguments.first() else {
            errors.push(TypeCheckError::new(
                "`[decorate]` requires a `.name` selector".to_owned(),
            ));
            return "<missing>".to_owned();
        };
        if selector_arg.name().is_some() {
            errors.push(TypeCheckError::new(
                "`[decorate]` requires a `.name` selector before named arguments".to_owned(),
            ));
            return "<missing>".to_owned();
        }

        let authored_selector = selector_arg.value().value();
        if selector_arg.value().source().trim() != authored_selector {
            errors.push(TypeCheckError::new(format!(
                "decoration selector `{}` must use unquoted `.name` syntax",
                selector_arg.value().source()
            )));
            return authored_selector.to_owned();
        }
        let Some(name) = authored_selector.strip_prefix('.') else {
            errors.push(TypeCheckError::new(format!(
                "decoration selector `{authored_selector}` must use `.name` syntax"
            )));
            return authored_selector.to_owned();
        };
        if !is_identifier(name) {
            errors.push(TypeCheckError::new(
                "`[decorate]` selector must contain one canonical identifier after `.`".to_owned(),
            ));
            return "<missing>".to_owned();
        }

        let Some(definition) = self.definitions.get(name) else {
            errors.push(TypeCheckError::new(format!("unknown decoration `{name}`")));
            return name.to_owned();
        };

        let mut provided = BTreeSet::new();
        let mut supplied = BTreeMap::new();
        for argument in &arguments[1..] {
            let Some(argument_name) = argument.name() else {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{name}` does not accept positional arguments after its selector"
                )));
                continue;
            };
            if !is_identifier(argument_name) {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{name}` argument name `{argument_name}` must be a canonical identifier"
                )));
                continue;
            }
            if !provided.insert(argument_name.to_owned()) {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{name}` argument `{argument_name}` was provided more than once"
                )));
            }
            supplied
                .entry(argument_name.to_owned())
                .or_insert_with(|| DecorationValue::new(argument.value().value()));
            if !definition.has_param(argument_name) && definition.rest.is_none() {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{name}` has no parameter named `{argument_name}`"
                )));
            }
            if !dialogue_argument_is_closed(argument.value().source()) {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{name}` argument `{argument_name}` must be a compile-time closed value"
                )));
            }
        }
        report_missing_required(name, definition, &provided, errors);
        let bindings = definition.bind(supplied);
        self.validate_bound_invocation(name, &bindings, errors);
        name.to_owned()
    }

    pub(super) fn inferred_tag_is_mark(&self, tag: &DialogueTag) -> bool {
        !self.inferred_tag_is_text_proxy(tag)
            && inferred_rich_text_tag_family(tag.name().trim_start_matches('.'), tag.attrs())
                .is_none()
            && tag.attrs().trim().is_empty()
    }

    pub(super) fn inferred_tag_is_text_proxy(&self, tag: &DialogueTag) -> bool {
        let selector = tag.name().trim_start_matches('.');
        self.text_proxy_types.contains(selector)
            || tag.arguments().iter().any(|argument| {
                argument.name().is_some_and(|name| {
                    matches!(name, "type" | "struct" | "proxy")
                        && self
                            .text_proxy_types
                            .contains(argument.value().value().trim_start_matches('.'))
                })
            })
    }

    fn report_cycles(&self, errors: &mut Vec<TypeCheckError>) {
        #[derive(Clone, Copy, Eq, PartialEq)]
        enum Visit {
            Active,
            Complete,
        }

        struct Frame {
            name: String,
            dependencies: Vec<String>,
            next: usize,
        }

        let mut states = BTreeMap::new();
        let mut reported = BTreeSet::new();
        for root in self.definitions.keys() {
            if states.get(root) == Some(&Visit::Complete) {
                continue;
            }

            states.insert(root.clone(), Visit::Active);
            let mut path = vec![root.clone()];
            let mut stack = vec![Frame {
                name: root.clone(),
                dependencies: self.definitions[root]
                    .dependencies
                    .iter()
                    .cloned()
                    .collect(),
                next: 0,
            }];
            while let Some(frame) = stack.last_mut() {
                if let Some(dependency) = frame.dependencies.get(frame.next).cloned() {
                    frame.next += 1;
                    match states.get(&dependency) {
                        Some(Visit::Complete) => {}
                        Some(Visit::Active) => {
                            let start = path
                                .iter()
                                .position(|entry| entry == &dependency)
                                .unwrap_or(0);
                            let mut cycle = path[start..].to_vec();
                            cycle.push(dependency);
                            let mut identity = cycle[..cycle.len().saturating_sub(1)].to_vec();
                            identity.sort();
                            if reported.insert(identity) {
                                errors.push(TypeCheckError::new(format!(
                                    "decoration composition cycle: {}",
                                    cycle.join(" -> ")
                                )));
                            }
                        }
                        None => {
                            states.insert(dependency.clone(), Visit::Active);
                            path.push(dependency.clone());
                            let dependencies = self
                                .definitions
                                .get(&dependency)
                                .map(|definition| definition.dependencies.iter().cloned().collect())
                                .unwrap_or_default();
                            stack.push(Frame {
                                name: dependency,
                                dependencies,
                                next: 0,
                            });
                        }
                    }
                } else {
                    let frame = stack.pop().expect("cycle traversal frame exists");
                    let _ = path.pop();
                    states.insert(frame.name, Visit::Complete);
                }
            }
        }
    }
}

impl DecorationDefinition {
    fn has_param(&self, name: &str) -> bool {
        self.params.iter().any(|param| param.name == name)
    }

    fn bind(&self, mut supplied: BTreeMap<String, DecorationValue>) -> DecorationBindings {
        let fixed = self
            .params
            .iter()
            .map(|param| {
                let value = supplied
                    .remove(&param.name)
                    .or_else(|| param.default.clone())
                    .unwrap_or_else(DecorationValue::unbound);
                (param.name.clone(), value)
            })
            .collect();
        let rest = if self.rest.is_some() {
            supplied
        } else {
            BTreeMap::new()
        };
        DecorationBindings {
            fixed,
            rest_name: self.rest.clone(),
            rest,
        }
    }
}

impl DecorationValue {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: normalize_decoration_value(&value.into()),
            bound: true,
        }
    }

    const fn unbound() -> Self {
        Self {
            value: String::new(),
            bound: false,
        }
    }
}

fn decoration_signature(
    item: &DecorationItem,
    errors: &mut Vec<TypeCheckError>,
) -> DecorationDefinition {
    let mut names = BTreeSet::new();
    let mut params = Vec::new();
    let mut rest = None;
    for (index, param) in item.params().iter().enumerate() {
        if !names.insert(param.name().to_owned()) {
            errors.push(TypeCheckError::new(format!(
                "duplicate parameter `{}` in decoration `{}`",
                param.name(),
                item.name()
            )));
            continue;
        }
        if param.is_rest() {
            if param.default().is_some() {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{}` rest parameter `{}` cannot declare a default value",
                    item.name(),
                    param.name()
                )));
            }
            if index + 1 != item.params().len() {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{}` rest parameter `{}` must be final",
                    item.name(),
                    param.name()
                )));
            }
            if rest.is_some() {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{}` can declare at most one rest parameter",
                    item.name()
                )));
            } else {
                rest = Some(param.name().to_owned());
            }
            continue;
        }
        if let Some(default) = param.default()
            && DecorationConst::from_expr(default).is_none()
        {
            errors.push(TypeCheckError::new(format!(
                "default for decoration `{}` parameter `{}` must be a compile-time closed value",
                item.name(),
                param.name()
            )));
        }
        params.push(DecorationParameter {
            name: param.name().to_owned(),
            default: param.default_source().map(DecorationValue::new),
        });
    }
    DecorationDefinition {
        item: item.clone(),
        params,
        rest,
        dependencies: BTreeSet::new(),
    }
}

fn validate_decoration_body(
    item: &DecorationItem,
    catalog: &DecorationCatalog,
    errors: &mut Vec<TypeCheckError>,
) -> BTreeSet<String> {
    if item.layers().is_empty() {
        errors.push(TypeCheckError::new(format!(
            "decoration `{}` must contain at least one visual builder layer",
            item.name()
        )));
    }
    let params = item
        .params()
        .iter()
        .map(|param| (param.name(), param.is_rest()))
        .collect::<BTreeMap<_, _>>();
    let mut dependencies = BTreeSet::new();

    for layer in item.layers() {
        let Expr::Call { callee, args } = layer.expr() else {
            errors.push(TypeCheckError::new(format!(
                "decoration `{}` layer `{}` must be a rich-text builder call",
                item.name(),
                layer.source()
            )));
            continue;
        };
        let Some(builder) = simple_path_name(callee) else {
            errors.push(TypeCheckError::new(format!(
                "decoration `{}` layer `{}` must call a simple rich-text builder name",
                item.name(),
                layer.source()
            )));
            continue;
        };

        if forbidden_builder(builder) {
            errors.push(TypeCheckError::new(format!(
                "decoration `{}` cannot use `{builder}` because it is not a reusable visual span builder",
                item.name()
            )));
            continue;
        }
        let Some(builder_kind) = DecorationBuilderKind::from_name(builder) else {
            errors.push(TypeCheckError::new(format!(
                "unsupported decoration builder `{builder}` in decoration `{}`",
                item.name()
            )));
            continue;
        };

        validate_builder_values(item.name(), args, &params, errors);
        if builder != "decorate" {
            validate_visual_builder_shape(item.name(), builder, builder_kind, args, errors);
        }
        if builder == "effect" && effect_uses_host_event_phase(args, &params) {
            errors.push(TypeCheckError::new(format!(
                "decoration `{}` cannot hide an effect with `phase=host_event`",
                item.name()
            )));
        }
        if builder == "decorate"
            && let Some(dependency) =
                validate_nested_decoration(item.name(), args, &params, catalog, errors)
        {
            dependencies.insert(dependency);
        }
    }
    dependencies
}

fn validate_visual_builder_shape(
    decoration: &str,
    builder: &str,
    kind: DecorationBuilderKind,
    args: &[CallArg],
    errors: &mut Vec<TypeCheckError>,
) {
    let mut named = BTreeSet::new();
    let mut spread_count = 0usize;
    for arg in args {
        match arg {
            CallArg::Named { name, .. } if !named.insert(name.as_str()) => {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` builder `{builder}` repeats named argument `{name}`"
                )));
            }
            CallArg::Spread { .. } => spread_count += 1,
            CallArg::Positional(_) | CallArg::Named { .. } => {}
        }
    }
    if spread_count > 1 {
        errors.push(TypeCheckError::new(format!(
            "decoration `{decoration}` builder `{builder}` may spread its rest parameter at most once"
        )));
    }

    match kind.shape() {
        DecorationBuilderShape::Empty if !args.is_empty() => {
            errors.push(TypeCheckError::new(format!(
                "decoration `{decoration}` builder `{builder}` does not accept arguments"
            )));
        }
        DecorationBuilderShape::Scalar => {
            let valid = match args {
                [CallArg::Positional(_)] => true,
                [CallArg::Named { name, .. }] => name == "value",
                _ => false,
            };
            if !valid {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` builder `{builder}` requires exactly one scalar positional value or named `value=...` argument"
                )));
            }
        }
        DecorationBuilderShape::ClosedSelector | DecorationBuilderShape::OpenSelector => {
            match args.first() {
                Some(CallArg::Positional(Expr::ShortVariant(selector))) => {
                    if kind.shape() == DecorationBuilderShape::ClosedSelector
                        && !kind.supports_selector(selector)
                    {
                        errors.push(TypeCheckError::new(format!(
                            "decoration `{decoration}` builder `{builder}` has unknown selector `.{selector}`"
                        )));
                    }
                }
                Some(CallArg::Positional(_)) => errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` builder `{builder}` selector must use `.name` syntax"
                ))),
                Some(CallArg::Named { .. } | CallArg::Spread { .. }) | None => {
                    errors.push(TypeCheckError::new(format!(
                        "decoration `{decoration}` builder `{builder}` requires one leading `.name` selector"
                    )));
                }
            }
            if args
                .iter()
                .skip(1)
                .any(|arg| matches!(arg, CallArg::Positional(_)))
            {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` builder `{builder}` does not accept positional arguments after its selector"
                )));
            }
        }
        _ => {}
    }
}

fn validate_builder_values(
    decoration: &str,
    args: &[CallArg],
    params: &BTreeMap<&str, bool>,
    errors: &mut Vec<TypeCheckError>,
) {
    for arg in args {
        match arg {
            CallArg::Positional(value) => {
                validate_builder_value(decoration, value, params, errors);
            }
            CallArg::Named { value, .. } => {
                validate_builder_value(decoration, value.as_ref(), params, errors);
            }
            CallArg::Spread { value } => {
                let Some(name) = simple_path_name(value) else {
                    errors.push(TypeCheckError::new(format!(
                        "decoration `{decoration}` can only spread its declared rest parameter"
                    )));
                    continue;
                };
                match params.get(name) {
                    Some(true) => {}
                    Some(false) => errors.push(TypeCheckError::new(format!(
                        "decoration `{decoration}` parameter `{name}` is not a rest parameter"
                    ))),
                    None => errors.push(TypeCheckError::new(format!(
                        "unknown decoration parameter `{name}` in decoration `{decoration}`"
                    ))),
                }
            }
        }
    }
}

fn validate_builder_value(
    decoration: &str,
    value: &Expr,
    params: &BTreeMap<&str, bool>,
    errors: &mut Vec<TypeCheckError>,
) {
    if let Some(name) = simple_path_name(value) {
        // A bare path in a declaration body is always a parameter reference.
        // Authors can quote registry-owned raw identifiers when they do not
        // intend a binding; this keeps misspelled parameter names diagnosable.
        match params.get(name) {
            Some(false) => {}
            Some(true) => errors.push(TypeCheckError::new(format!(
                "decoration `{decoration}` rest parameter `{name}` can only be used with spread syntax"
            ))),
            None => errors.push(TypeCheckError::new(format!(
                "unknown decoration parameter `{name}` in decoration `{decoration}`"
            ))),
        }
        return;
    }
    if DecorationConst::from_expr(value).is_none() {
        errors.push(TypeCheckError::new(format!(
            "decoration `{decoration}` builder arguments must be compile-time closed values or declared parameters"
        )));
    }
}

fn validate_nested_decoration(
    owner: &str,
    args: &[CallArg],
    parent_params: &BTreeMap<&str, bool>,
    catalog: &DecorationCatalog,
    errors: &mut Vec<TypeCheckError>,
) -> Option<String> {
    let Some(CallArg::Positional(Expr::ShortVariant(selector))) = args.first() else {
        errors.push(TypeCheckError::new(format!(
            "decoration `{owner}` nested `decorate` builder requires a leading `.name` selector"
        )));
        return None;
    };
    let name = selector.as_str();
    let Some(definition) = catalog.definitions.get(name) else {
        errors.push(TypeCheckError::new(format!(
            "decoration `{owner}` references unknown decoration `{name}`"
        )));
        return None;
    };

    let mut provided = BTreeSet::new();
    let mut spread_count = 0usize;
    for arg in &args[1..] {
        match arg {
            CallArg::Positional(_) => errors.push(TypeCheckError::new(format!(
                "nested decoration `{name}` in `{owner}` does not accept positional arguments"
            ))),
            CallArg::Named {
                name: argument_name,
                ..
            } => {
                if !provided.insert(argument_name.as_str()) {
                    errors.push(TypeCheckError::new(format!(
                        "nested decoration `{name}` argument `{argument_name}` was provided more than once"
                    )));
                }
                if !definition.has_param(argument_name) && definition.rest.is_none() {
                    errors.push(TypeCheckError::new(format!(
                        "nested decoration `{name}` has no parameter named `{argument_name}`"
                    )));
                }
            }
            CallArg::Spread { value } => {
                spread_count += 1;
                if spread_count > 1 {
                    errors.push(TypeCheckError::new(format!(
                        "nested decoration `{name}` in `{owner}` may spread its rest parameter at most once"
                    )));
                }
                let forwarded = simple_path_name(value);
                if !forwarded.is_some_and(|forwarded| {
                    parent_params.get(forwarded) == Some(&true) && definition.rest.is_some()
                }) {
                    errors.push(TypeCheckError::new(format!(
                        "nested decoration `{name}` in `{owner}` cannot accept this custom argument spread"
                    )));
                }
            }
        }
    }
    let provided = provided.into_iter().map(str::to_owned).collect();
    report_missing_required(name, definition, &provided, errors);
    Some(name.to_owned())
}

fn report_missing_required(
    name: &str,
    definition: &DecorationDefinition,
    provided: &BTreeSet<String>,
    errors: &mut Vec<TypeCheckError>,
) {
    for param in &definition.params {
        if param.default.is_none() && !provided.contains(&param.name) {
            errors.push(TypeCheckError::new(format!(
                "decoration `{name}` is missing required argument `{}`",
                param.name
            )));
        }
    }
}

fn normalize_decoration_value(source: &str) -> String {
    let source = source.trim();
    source
        .strip_prefix('"')
        .and_then(|source| source.strip_suffix('"'))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|source| source.strip_suffix('\''))
        })
        .unwrap_or(source)
        .to_owned()
}

fn forbidden_builder(name: &str) -> bool {
    matches!(
        name,
        "p" | "l"
            | "r"
            | "br"
            | "w"
            | "clear"
            | "er"
            | "cm"
            | "reset"
            | "speed"
            | "object"
            | "voice"
            | "face"
            | "pose"
            | "show"
            | "hide"
            | "move"
            | "scale"
            | "rotate"
            | "anim"
            | "shake"
            | "at"
            | "call"
            | "signal"
            | "if"
            | "else"
            | "endif"
            | "raw"
            | "mark"
    )
}

fn effect_uses_host_event_phase(args: &[CallArg], params: &BTreeMap<&str, bool>) -> bool {
    args.iter().any(|arg| {
        let CallArg::Named { name, value } = arg else {
            return false;
        };
        name == "phase"
            && match value.as_ref() {
                Expr::Path(path) => {
                    path.is_single("host_event") && !params.contains_key("host_event")
                }
                Expr::ShortVariant(value) => value == "host_event",
                Expr::Literal(Literal::String(value)) => value == "host_event",
                _ => false,
            }
    })
}

fn dialogue_argument_is_closed(source: &str) -> bool {
    let source = source.trim();
    if source.is_empty() {
        return false;
    }
    if (source.starts_with('"') && source.ends_with('"'))
        || (source.starts_with('\'') && source.ends_with('\''))
    {
        return true;
    }
    if let Ok(expr) = parse_expr(source)
        && DecorationConst::from_expr(&expr).is_some()
    {
        return true;
    }
    if source.contains("#[")
        || source.contains("$(")
        || source.chars().any(char::is_whitespace)
        || source.chars().any(|ch| {
            matches!(
                ch,
                '(' | ')' | '[' | ']' | '{' | '}' | '+' | '*' | '/' | '"' | '\''
            )
        })
    {
        return false;
    }
    // Preserve registry-owned tokens such as `0,1`, `#ff4050`, and
    // `source-shader`, while rejecting dotted runtime paths.
    !looks_like_dotted_runtime_path(source)
}

fn looks_like_dotted_runtime_path(source: &str) -> bool {
    let mut segments = source.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let rest = segments.collect::<Vec<_>>();
    !rest.is_empty() && is_identifier(first) && rest.iter().all(|segment| is_identifier(segment))
}

fn simple_path_name(expr: &Expr) -> Option<&str> {
    let Expr::Path(path) = expr else {
        return None;
    };
    path.segments()
        .first()
        .filter(|_| path.segments().len() == 1)
        .map(Name::as_str)
}
