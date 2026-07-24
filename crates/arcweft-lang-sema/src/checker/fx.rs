//! Semantic inventory for ordinary functions marked with `#[fx]`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use arcweft_lang_hir::{
    fx::{FX_EXPANSION_LIMITS, FxConst, FxConstructorKind},
    model::{HirFunction, HirModule, HirTopLevelDecl},
};
use arcweft_lang_syntax::{
    ast::{dialogue::DialogueTag, flow::Stmt, pattern::Pattern, view::ViewFxApplication},
    cst::is_identifier,
    expr::{CallArg, Expr, parse_expr},
    types::FnParamKind,
};
use arcweft_presentation::rich_text::inferred_tag_family;

use crate::{diagnostics::TypeCheckError, types::TypeKind};

use super::helpers::type_kind_label;

mod span;

pub(super) use span::FxSpanState;

/// Fx function signatures shared by ordinary calls and `RichText` tags.
#[derive(Clone, Debug, Default)]
pub(super) struct FxCatalog {
    definitions: BTreeMap<String, FxDefinition>,
    text_proxy_types: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct FxDefinition {
    params: Vec<FxParameter>,
    dependencies: Vec<String>,
    direct_nodes: usize,
}

#[derive(Clone, Debug)]
struct FxParameter {
    name: String,
    ty: TypeKind,
    has_default: bool,
}

impl FxCatalog {
    pub(super) fn from_module(
        module: &HirModule,
        resolved_signatures: &BTreeMap<String, (Vec<TypeKind>, TypeKind)>,
        errors: &mut Vec<TypeCheckError>,
    ) -> Self {
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
        let fx_names = module
            .functions()
            .iter()
            .filter(|function| function.has_attribute("fx"))
            .map(|function| function.name().to_owned())
            .collect::<BTreeSet<_>>();
        let mut catalog = Self {
            definitions: BTreeMap::new(),
            text_proxy_types,
        };

        for function in module.functions() {
            if !function.has_attribute("fx") {
                continue;
            }
            let Some((parameter_types, return_type)) =
                resolved_signatures.get(&function.qualified_name())
            else {
                errors.push(TypeCheckError::new(format!(
                    "Fx function `{}` has no resolved semantic signature",
                    function.name()
                )));
                continue;
            };
            let definition = validate_fx_signature(function, parameter_types, return_type, errors);
            if catalog
                .definitions
                .insert(function.name().to_owned(), definition)
                .is_some()
            {
                errors.push(TypeCheckError::new(format!(
                    "duplicate Fx function `{}`",
                    function.name()
                )));
            }
        }

        for function in module
            .functions()
            .iter()
            .filter(|function| function.has_attribute("fx"))
        {
            let mut dependencies = Vec::new();
            let mut direct_nodes = 0;
            if let Some(root) = fx_function_value(function) {
                validate_graph_expr(
                    function.name(),
                    root,
                    &fx_names,
                    &mut dependencies,
                    &mut direct_nodes,
                    errors,
                );
            } else {
                errors.push(TypeCheckError::new(format!(
                    "Fx function `{}` must return one typed Fx graph value",
                    function.name()
                )));
            }
            if let Some(definition) = catalog.definitions.get_mut(function.name()) {
                definition.dependencies = dependencies;
                definition.direct_nodes = direct_nodes;
            }
        }
        catalog.report_cycles_and_limits(errors);
        catalog
    }

    pub(super) fn is_definition(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    pub(super) fn call_errors(&self, name: &str, args: &[CallArg]) -> Vec<TypeCheckError> {
        if !self.definitions.contains_key(name) {
            return Vec::new();
        }
        args.iter()
            .filter(|arg| !matches!(arg, CallArg::Named { .. }))
            .map(|_| {
                TypeCheckError::new(format!("Fx function `{name}` accepts named arguments only"))
            })
            .collect()
    }

    pub(super) fn validate_view_application(
        &self,
        application: &ViewFxApplication,
        errors: &mut Vec<TypeCheckError>,
    ) {
        let Expr::Call(call) = application.call() else {
            errors.push(TypeCheckError::new(
                "View `.fx(...)` requires an Fx function call".to_owned(),
            ));
            return;
        };
        let Some(name) = simple_path(call.callee()) else {
            errors.push(TypeCheckError::new(
                "View `.fx(...)` target must resolve from a canonical function path".to_owned(),
            ));
            return;
        };
        let Some(definition) = self.definitions.get(name) else {
            errors.push(TypeCheckError::new(format!(
                "unknown Fx function `{name}` in View `.fx(...)`"
            )));
            return;
        };
        validate_named_call(name, definition, call.args(), false, errors);
    }

    /// Validates `[fx name(...)]` and returns the definition name for span diagnostics.
    pub(super) fn validate_dialogue_tag(
        &self,
        tag: &DialogueTag,
        errors: &mut Vec<TypeCheckError>,
    ) -> String {
        let source = tag.attrs().trim();
        if source.is_empty() {
            errors.push(TypeCheckError::new(
                "`[fx]` requires one Fx function call".to_owned(),
            ));
            return "<missing>".to_owned();
        }
        let expr = match parse_expr(source) {
            Ok(expr) => expr,
            Err(error) => {
                errors.push(TypeCheckError::new(format!(
                    "invalid `[fx]` invocation: {error}"
                )));
                return "<invalid>".to_owned();
            }
        };
        let Expr::Call(call) = expr else {
            errors.push(TypeCheckError::new(
                "`[fx]` requires a function call such as `notice(accent=...)`".to_owned(),
            ));
            return "<invalid>".to_owned();
        };
        let Some(name) = simple_path(call.callee()) else {
            errors.push(TypeCheckError::new(
                "`[fx]` target must resolve from a canonical function path".to_owned(),
            ));
            return "<invalid>".to_owned();
        };
        let Some(definition) = self.definitions.get(name) else {
            errors.push(TypeCheckError::new(format!(
                "unknown Fx function `{name}` in `[fx]`"
            )));
            return name.to_owned();
        };
        validate_named_call(name, definition, call.args(), true, errors);
        name.to_owned()
    }

    pub(super) fn inferred_tag_is_mark(&self, tag: &DialogueTag) -> bool {
        !self.inferred_tag_is_text_proxy(tag)
            && inferred_tag_family(tag.name().trim_start_matches('.'), tag.attrs()).is_none()
            && tag.attrs().trim().is_empty()
    }

    fn inferred_tag_is_text_proxy(&self, tag: &DialogueTag) -> bool {
        let selector = tag.name().trim_start_matches('.');
        self.text_proxy_types.contains(selector)
            || tag.arguments().iter().any(|argument| {
                argument.name().is_some_and(|name| {
                    matches!(name, "type" | "struct" | "proxy")
                        && argument.value().is_some_and(|value| {
                            self.text_proxy_types
                                .contains(value.value().trim_start_matches('.'))
                        })
                })
            })
    }

    fn report_cycles_and_limits(&self, errors: &mut Vec<TypeCheckError>) {
        #[derive(Clone, Copy, Eq, PartialEq)]
        enum Visit {
            Active,
            Complete,
        }

        fn visit(
            name: &str,
            catalog: &FxCatalog,
            states: &mut HashMap<String, Visit>,
            stack: &mut Vec<String>,
            errors: &mut Vec<TypeCheckError>,
        ) {
            match states.get(name) {
                Some(Visit::Complete) => return,
                Some(Visit::Active) => {
                    let start = stack.iter().position(|entry| entry == name).unwrap_or(0);
                    let mut cycle = stack[start..].to_vec();
                    cycle.push(name.to_owned());
                    errors.push(TypeCheckError::new(format!(
                        "Fx composition cycle: {}",
                        cycle.join(" -> ")
                    )));
                    return;
                }
                None => {}
            }
            states.insert(name.to_owned(), Visit::Active);
            stack.push(name.to_owned());
            if stack.len() > FX_EXPANSION_LIMITS.max_depth {
                errors.push(TypeCheckError::new(format!(
                    "Fx `{}` exceeds maximum composition depth of {}",
                    stack.first().map_or(name, String::as_str),
                    FX_EXPANSION_LIMITS.max_depth
                )));
            } else if let Some(definition) = catalog.definitions.get(name) {
                for dependency in &definition.dependencies {
                    visit(dependency, catalog, states, stack, errors);
                }
            }
            stack.pop();
            states.insert(name.to_owned(), Visit::Complete);
        }

        let mut states = HashMap::new();
        for name in self.definitions.keys() {
            visit(name, self, &mut states, &mut Vec::new(), errors);
            let mut visits = 0usize;
            let mut nodes = 0usize;
            self.count_expansion(
                name,
                &mut visits,
                &mut nodes,
                &mut BTreeSet::new(),
                1,
                errors,
            );
        }
    }

    fn count_expansion(
        &self,
        name: &str,
        visits: &mut usize,
        nodes: &mut usize,
        active: &mut BTreeSet<String>,
        depth: usize,
        errors: &mut Vec<TypeCheckError>,
    ) {
        if depth > FX_EXPANSION_LIMITS.max_depth || !active.insert(name.to_owned()) {
            return;
        }
        *visits = visits.saturating_add(1);
        if *visits > FX_EXPANSION_LIMITS.max_visits {
            errors.push(TypeCheckError::new(format!(
                "Fx `{name}` expansion exceeds maximum visit count of {}",
                FX_EXPANSION_LIMITS.max_visits
            )));
            active.remove(name);
            return;
        }
        if let Some(definition) = self.definitions.get(name) {
            *nodes = nodes.saturating_add(definition.direct_nodes);
            if *nodes > FX_EXPANSION_LIMITS.max_nodes {
                errors.push(TypeCheckError::new(format!(
                    "Fx `{name}` expansion exceeds maximum node count of {}",
                    FX_EXPANSION_LIMITS.max_nodes
                )));
                active.remove(name);
                return;
            }
            for dependency in &definition.dependencies {
                self.count_expansion(dependency, visits, nodes, active, depth + 1, errors);
                if *visits > FX_EXPANSION_LIMITS.max_visits
                    || *nodes > FX_EXPANSION_LIMITS.max_nodes
                {
                    active.remove(name);
                    return;
                }
            }
        }
        active.remove(name);
    }
}

fn validate_fx_signature(
    function: &HirFunction,
    parameter_types: &[TypeKind],
    return_type: &TypeKind,
    errors: &mut Vec<TypeCheckError>,
) -> FxDefinition {
    validate_fx_declaration(function, return_type, errors);

    let signature = function.signature();
    let mut names = BTreeSet::new();
    let mut params = Vec::new();
    let authored_params = signature
        .param_groups()
        .iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .collect::<Vec<_>>();
    if authored_params.len() != parameter_types.len() {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{}` resolved {} parameter type(s) for {} authored parameter(s)",
            function.name(),
            parameter_types.len(),
            authored_params.len()
        )));
        return FxDefinition {
            params,
            dependencies: Vec::new(),
            direct_nodes: 0,
        };
    }
    for (param, resolved_type) in authored_params.into_iter().zip(parameter_types) {
        let Pattern::Ident(name) = param.pattern() else {
            errors.push(TypeCheckError::new(format!(
                "Fx function `{}` parameters must be simple identifiers",
                function.name()
            )));
            continue;
        };
        if param.kind() == FnParamKind::Rest {
            errors.push(TypeCheckError::new(format!(
                "Fx function `{}` cannot declare a rest parameter",
                function.name()
            )));
        }
        if !names.insert(name.to_owned()) {
            errors.push(TypeCheckError::new(format!(
                "Fx function `{}` repeats parameter `{name}`",
                function.name()
            )));
        }
        if let Some(default) = param.default()
            && !closed_fx_value(default)
        {
            errors.push(TypeCheckError::new(format!(
                "default for Fx parameter `{}.{name}` must be const-evaluable and cannot reference parameters or runtime state",
                function.name()
            )));
        }
        params.push(FxParameter {
            name: name.to_owned(),
            ty: resolved_type.clone(),
            has_default: param.default().is_some(),
        });
    }
    FxDefinition {
        params,
        dependencies: Vec::new(),
        direct_nodes: 0,
    }
}

fn validate_fx_declaration(
    function: &HirFunction,
    return_type: &TypeKind,
    errors: &mut Vec<TypeCheckError>,
) {
    for attribute in function
        .attributes()
        .iter()
        .filter(|attribute| attribute.name() == "fx")
    {
        if attribute.args().is_some() {
            errors.push(TypeCheckError::new(format!(
                "`#[fx]` on `{}` is an argument-free marker",
                function.name()
            )));
        }
    }
    if function.has_attribute("pure") {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{}` must use `#[fx]` alone; Fx already implies purity",
            function.name()
        )));
    }
    let signature = function.signature();
    if !signature.generic_params().is_empty() {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{}` cannot declare generic parameters",
            function.name()
        )));
    }
    if signature.param_groups().len() != 1 {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{}` must use one parameter group",
            function.name()
        )));
    }
    if return_type != &TypeKind::Named("Fx".to_owned()) {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{}` must declare return type `Fx`",
            function.name()
        )));
    }
}

fn fx_function_value(function: &HirFunction) -> Option<&Expr> {
    function
        .value()
        .map(arcweft_lang_syntax::ast::flow::AuthoredExpr::expr)
        .or_else(|| {
            function
                .statements()
                .last()
                .and_then(|statement| match statement {
                    Stmt::Return { expr, .. } => Some(expr),
                    _ => None,
                })
        })
}

fn validate_graph_expr(
    owner: &str,
    expr: &Expr,
    fx_names: &BTreeSet<String>,
    dependencies: &mut Vec<String>,
    direct_nodes: &mut usize,
    errors: &mut Vec<TypeCheckError>,
) {
    let Expr::Call(call) = expr else {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{owner}` must return an Fx constructor or another Fx function call"
        )));
        return;
    };
    if let Some(member) = fx_constructor_member(call.callee()) {
        *direct_nodes = direct_nodes.saturating_add(1);
        let Some(kind) = FxConstructorKind::from_member(member) else {
            errors.push(TypeCheckError::new(format!(
                "unknown Fx constructor `Fx.{member}` in `{owner}`"
            )));
            return;
        };
        validate_constructor_args(
            owner,
            kind,
            call.args(),
            fx_names,
            dependencies,
            direct_nodes,
            errors,
        );
        return;
    }
    let Some(name) = simple_path(call.callee()) else {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{owner}` graph calls must use `Fx.name(...)` or an Fx function symbol"
        )));
        return;
    };
    if !fx_names.contains(name) {
        errors.push(TypeCheckError::new(format!(
            "Fx function `{owner}` graph references non-Fx function `{name}`"
        )));
        return;
    }
    dependencies.push(name.to_owned());
    for arg in call.args() {
        if !matches!(arg, CallArg::Named { .. }) {
            errors.push(TypeCheckError::new(format!(
                "Fx function call `{name}` in `{owner}` accepts named arguments only"
            )));
        }
    }
}

fn validate_constructor_args(
    owner: &str,
    kind: FxConstructorKind,
    args: &[CallArg],
    fx_names: &BTreeSet<String>,
    dependencies: &mut Vec<String>,
    direct_nodes: &mut usize,
    errors: &mut Vec<TypeCheckError>,
) {
    match kind {
        FxConstructorKind::Stack => {
            let [CallArg::Positional(value)] = args else {
                errors.push(TypeCheckError::new(format!(
                    "Fx function `{owner}` must call `Fx.stack` with one ordered graph list"
                )));
                return;
            };
            let Expr::BracketSeq(children) = value.as_ref() else {
                errors.push(TypeCheckError::new(format!(
                    "Fx function `{owner}` must call `Fx.stack` with one ordered graph list"
                )));
                return;
            };
            for child in children {
                validate_graph_expr(owner, child, fx_names, dependencies, direct_nodes, errors);
            }
        }
        FxConstructorKind::Conditional => {
            let mut branches = BTreeMap::new();
            for arg in args {
                if let CallArg::Named { name, value } = arg {
                    branches.insert(name.as_str(), value.as_ref());
                } else {
                    errors.push(TypeCheckError::new(format!(
                        "`Fx.conditional` in `{owner}` accepts named arguments only"
                    )));
                }
            }
            for branch in ["then", "else"] {
                if let Some(value) = branches.get(branch) {
                    validate_graph_expr(owner, value, fx_names, dependencies, direct_nodes, errors);
                } else {
                    errors.push(TypeCheckError::new(format!(
                        "`Fx.conditional` in `{owner}` requires `{branch} = ...`"
                    )));
                }
            }
            if !branches.contains_key("condition") {
                errors.push(TypeCheckError::new(format!(
                    "`Fx.conditional` in `{owner}` requires `condition = ...`"
                )));
            }
        }
        FxConstructorKind::Shader => {
            for (index, arg) in args.iter().enumerate() {
                if index > 0 && !matches!(arg, CallArg::Named { .. }) {
                    errors.push(TypeCheckError::new(format!(
                        "`Fx.shader` in `{owner}` accepts only its leading resource positionally"
                    )));
                }
                if matches!(arg, CallArg::Spread { .. }) {
                    errors.push(TypeCheckError::new(format!(
                        "Fx constructors do not accept spread arguments in `{owner}`"
                    )));
                }
            }
        }
        _ => {
            for arg in args {
                if !matches!(arg, CallArg::Named { .. }) {
                    errors.push(TypeCheckError::new(format!(
                        "Fx constructor arguments in `{owner}` must be named"
                    )));
                }
            }
        }
    }
}

fn validate_named_call(
    name: &str,
    definition: &FxDefinition,
    args: &[CallArg],
    require_closed_values: bool,
    errors: &mut Vec<TypeCheckError>,
) {
    let mut provided = BTreeSet::new();
    for arg in args {
        let CallArg::Named {
            name: argument_name,
            value,
        } = arg
        else {
            errors.push(TypeCheckError::new(format!(
                "Fx function `{name}` accepts named arguments only"
            )));
            continue;
        };
        if !is_identifier(argument_name) {
            errors.push(TypeCheckError::new(format!(
                "Fx argument `{argument_name}` must be a canonical identifier"
            )));
        }
        if !provided.insert(argument_name.as_str()) {
            errors.push(TypeCheckError::new(format!(
                "Fx function `{name}` receives duplicate argument `{argument_name}`"
            )));
        }
        let parameter = definition
            .params
            .iter()
            .find(|parameter| parameter.name == *argument_name);
        if parameter.is_none() {
            errors.push(TypeCheckError::new(format!(
                "Fx function `{name}` has no parameter named `{argument_name}`"
            )));
        }
        if let Some(parameter) = parameter
            && closed_value_matches_type(value, &parameter.ty) == Some(false)
        {
            errors.push(TypeCheckError::new(format!(
                "Fx argument `{name}.{argument_name}` must have type {}, found an incompatible closed value",
                type_kind_label(&parameter.ty)
            )));
        }
        if require_closed_values && !closed_fx_value(value) {
            errors.push(TypeCheckError::new(format!(
                "RichText Fx argument `{name}.{argument_name}` must be a closed value"
            )));
        }
    }
    for parameter in &definition.params {
        if !parameter.has_default && !provided.contains(parameter.name.as_str()) {
            errors.push(TypeCheckError::new(format!(
                "Fx function `{name}` is missing required argument `{}`",
                parameter.name
            )));
        }
    }
}

fn closed_fx_value(expr: &Expr) -> bool {
    if FxConst::from_expr(expr).is_some() {
        return true;
    }
    match expr {
        Expr::Call(call)
            if matches!(
                simple_path(call.callee()),
                Some("rgb" | "vec2" | "vec3" | "vec4")
            ) =>
        {
            call.args()
                .iter()
                .all(|arg| !matches!(arg, CallArg::Spread { .. }) && closed_fx_value(arg.value()))
        }
        _ => false,
    }
}

fn closed_value_matches_type(expr: &Expr, expected: &TypeKind) -> Option<bool> {
    match expr {
        Expr::Literal(arcweft_lang_syntax::expr::Literal::Int(literal))
            if literal.suffix().is_none() =>
        {
            Some(expected.is_integer() || expected.is_float())
        }
        Expr::Literal(arcweft_lang_syntax::expr::Literal::Float { suffix: None, .. }) => {
            Some(expected.is_float())
        }
        Expr::Literal(literal) => {
            super::helpers::literal_type(literal).map(|actual| actual == *expected)
        }
        Expr::ShortVariant(_) => Some(matches!(expected, TypeKind::Named(_))),
        Expr::Call(call) => match simple_path(call.callee()) {
            Some("rgb") => Some(expected == &TypeKind::Named("Color".to_owned())),
            Some("vec2") => Some(expected == &TypeKind::Named("Vec2".to_owned())),
            Some("vec3") => Some(expected == &TypeKind::Named("Vec3".to_owned())),
            Some("vec4") => Some(expected == &TypeKind::Named("Vec4".to_owned())),
            _ => None,
        },
        Expr::BracketSeq(values) => match expected {
            TypeKind::Vec(item) | TypeKind::Seq(item) | TypeKind::Slice(item) => Some(
                values
                    .iter()
                    .all(|value| closed_value_matches_type(value, item) != Some(false)),
            ),
            _ => Some(false),
        },
        Expr::NumericBracketSeq(_) => Some(matches!(
            expected,
            TypeKind::Vec(_) | TypeKind::Seq(_) | TypeKind::Slice(_)
        )),
        _ => None,
    }
}

fn fx_constructor_member(expr: &Expr) -> Option<&str> {
    let Expr::Select(select) = expr else {
        return None;
    };
    matches!(select.target(), Expr::Path(path) if path.is_single("Fx"))
        .then(|| select.member().as_str())
}

fn simple_path(expr: &Expr) -> Option<&str> {
    let Expr::Path(path) = expr else {
        return None;
    };
    (path.segments().len() == 1).then(|| path.as_label())
}

#[cfg(test)]
mod tests {
    use super::{FxCatalog, FxDefinition};
    use crate::diagnostics::TypeCheckError;
    use std::collections::{BTreeMap, BTreeSet};

    fn definition(dependencies: Vec<String>, direct_nodes: usize) -> FxDefinition {
        FxDefinition {
            params: Vec::new(),
            dependencies,
            direct_nodes,
        }
    }

    #[test]
    fn expansion_count_stops_at_an_already_reported_cycle() {
        let catalog = FxCatalog {
            definitions: BTreeMap::from([(
                "recursive".to_owned(),
                definition(vec!["recursive".to_owned()], 1),
            )]),
            text_proxy_types: BTreeSet::new(),
        };
        let mut errors = Vec::<TypeCheckError>::new();

        catalog.report_cycles_and_limits(&mut errors);

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("Fx composition cycle"))
        );
    }

    #[test]
    fn expansion_budget_counts_repeated_calls_and_constructor_nodes() {
        let catalog = FxCatalog {
            definitions: BTreeMap::from([
                (
                    "root".to_owned(),
                    definition(vec!["leaf".to_owned(); 4_097], 1),
                ),
                ("leaf".to_owned(), definition(Vec::new(), 1)),
            ]),
            text_proxy_types: BTreeSet::new(),
        };
        let mut errors = Vec::<TypeCheckError>::new();

        catalog.report_cycles_and_limits(&mut errors);

        assert!(errors.iter().any(|error| {
            error
                .message()
                .contains("exceeds maximum node count of 4096")
        }));
    }
}
