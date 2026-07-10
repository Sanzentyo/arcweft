//! Binding-sensitive validation of decoration expansion.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::decoration::{
    DECORATION_EXPANSION_LIMITS, DecorationBuilderKind, DecorationBuilderShape,
};
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal};

use super::{
    DecorationBindings, DecorationCatalog, DecorationValue, TypeCheckError,
    normalize_decoration_value, simple_path_name,
};

#[derive(Clone, Debug)]
enum ResolvedBuilderArg {
    Positional {
        selector: bool,
    },
    Named {
        name: String,
        value: DecorationValue,
    },
}

#[derive(Debug)]
struct ExpansionBudget {
    root: String,
    visits: usize,
    layers: usize,
    exhausted: bool,
}

impl ExpansionBudget {
    fn new(root: &str) -> Self {
        Self {
            root: root.to_owned(),
            visits: 0,
            layers: 0,
            exhausted: false,
        }
    }

    fn enter_definition(
        &mut self,
        name: &str,
        depth: usize,
        errors: &mut Vec<TypeCheckError>,
    ) -> bool {
        if self.exhausted {
            return false;
        }
        if depth >= DECORATION_EXPANSION_LIMITS.max_depth {
            errors.push(TypeCheckError::new(format!(
                "decoration `{}` expansion exceeds maximum composition depth of {} at `{name}`",
                self.root, DECORATION_EXPANSION_LIMITS.max_depth
            )));
            self.exhausted = true;
            return false;
        }
        if self.visits >= DECORATION_EXPANSION_LIMITS.max_visits {
            errors.push(TypeCheckError::new(format!(
                "decoration `{}` expansion exceeds maximum visit count of {}",
                self.root, DECORATION_EXPANSION_LIMITS.max_visits
            )));
            self.exhausted = true;
            return false;
        }
        self.visits += 1;
        true
    }

    fn add_layer(&mut self, errors: &mut Vec<TypeCheckError>) -> bool {
        if self.exhausted {
            return false;
        }
        if self.layers >= DECORATION_EXPANSION_LIMITS.max_layers {
            errors.push(TypeCheckError::new(format!(
                "decoration `{}` expansion exceeds maximum expanded layer count of {}",
                self.root, DECORATION_EXPANSION_LIMITS.max_layers
            )));
            self.exhausted = true;
            return false;
        }
        self.layers += 1;
        true
    }
}

impl DecorationCatalog {
    pub(super) fn validate_default_expansions(&self, errors: &mut Vec<TypeCheckError>) {
        for (name, definition) in &self.definitions {
            self.validate_bound_invocation(name, &definition.bind(BTreeMap::new()), errors);
        }
    }

    pub(super) fn validate_bound_invocation(
        &self,
        name: &str,
        bindings: &DecorationBindings,
        errors: &mut Vec<TypeCheckError>,
    ) {
        self.validate_bound_definition(
            name,
            bindings,
            &mut Vec::new(),
            &mut ExpansionBudget::new(name),
            errors,
        );
    }

    fn validate_bound_definition(
        &self,
        name: &str,
        bindings: &DecorationBindings,
        chain: &mut Vec<String>,
        budget: &mut ExpansionBudget,
        errors: &mut Vec<TypeCheckError>,
    ) {
        if chain.iter().any(|active| active == name) {
            return;
        }
        if !budget.enter_definition(name, chain.len(), errors) {
            return;
        }
        let Some(definition) = self.definitions.get(name) else {
            return;
        };
        chain.push(name.to_owned());
        for layer in definition.item.layers() {
            let Expr::Call { callee, args } = layer.expr() else {
                continue;
            };
            let Some(builder) = simple_path_name(callee) else {
                continue;
            };
            if builder == "decorate" {
                self.validate_bound_nested_decoration(name, args, bindings, chain, budget, errors);
            } else if let Some(builder_kind) = DecorationBuilderKind::from_name(builder) {
                if !budget.add_layer(errors) {
                    break;
                }
                let resolved = resolve_builder_args(args, bindings);
                validate_resolved_visual_builder(name, builder, builder_kind, &resolved, errors);
            }
        }
        let _ = chain.pop();
    }

    fn validate_bound_nested_decoration(
        &self,
        owner: &str,
        args: &[CallArg],
        bindings: &DecorationBindings,
        chain: &mut Vec<String>,
        budget: &mut ExpansionBudget,
        errors: &mut Vec<TypeCheckError>,
    ) {
        let Some(CallArg::Positional(Expr::ShortVariant(selector))) = args.first() else {
            return;
        };
        let target_name = selector.as_str();
        let Some(target) = self.definitions.get(target_name) else {
            return;
        };
        let mut supplied = BTreeMap::new();
        for arg in &args[1..] {
            match arg {
                CallArg::Named {
                    name: argument_name,
                    value,
                } => {
                    let value = resolve_expr_value(value, bindings);
                    if supplied.insert(argument_name.clone(), value).is_some() {
                        errors.push(TypeCheckError::new(format!(
                            "nested decoration `{target_name}` in `{owner}` receives duplicate bound argument `{argument_name}`"
                        )));
                    }
                }
                CallArg::Spread { value }
                    if simple_path_name(value) == bindings.rest_name.as_deref() =>
                {
                    for (argument_name, value) in &bindings.rest {
                        if supplied
                            .insert(argument_name.clone(), value.clone())
                            .is_some()
                        {
                            errors.push(TypeCheckError::new(format!(
                                "nested decoration `{target_name}` in `{owner}` receives `{argument_name}` both explicitly and through its rest bag"
                            )));
                        }
                    }
                }
                CallArg::Positional(_) | CallArg::Spread { .. } => {}
            }
        }
        self.validate_bound_definition(target_name, &target.bind(supplied), chain, budget, errors);
    }
}

fn resolve_builder_args(
    args: &[CallArg],
    bindings: &DecorationBindings,
) -> Vec<ResolvedBuilderArg> {
    let mut resolved = Vec::new();
    for arg in args {
        match arg {
            CallArg::Positional(value) => resolved.push(ResolvedBuilderArg::Positional {
                selector: matches!(value, Expr::ShortVariant(_)),
            }),
            CallArg::Named { name, value } => resolved.push(ResolvedBuilderArg::Named {
                name: name.clone(),
                value: resolve_expr_value(value, bindings),
            }),
            CallArg::Spread { value }
                if simple_path_name(value) == bindings.rest_name.as_deref() =>
            {
                resolved.extend(bindings.rest.iter().map(|(name, value)| {
                    ResolvedBuilderArg::Named {
                        name: name.clone(),
                        value: value.clone(),
                    }
                }));
            }
            CallArg::Spread { .. } => {}
        }
    }
    resolved
}

fn resolve_expr_value(expr: &Expr, bindings: &DecorationBindings) -> DecorationValue {
    if let Some(name) = simple_path_name(expr) {
        return bindings
            .fixed
            .get(name)
            .cloned()
            .unwrap_or_else(DecorationValue::unbound);
    }
    match expr {
        Expr::Literal(Literal::String(value)) => DecorationValue::new(value),
        Expr::Literal(Literal::Bool(value)) => DecorationValue::new(value.to_string()),
        Expr::ShortVariant(value) => DecorationValue::new(format!(".{value}")),
        _ => DecorationValue::new("<closed>"),
    }
}

fn validate_resolved_visual_builder(
    decoration: &str,
    builder: &str,
    kind: DecorationBuilderKind,
    args: &[ResolvedBuilderArg],
    errors: &mut Vec<TypeCheckError>,
) {
    let mut named = BTreeSet::new();
    for arg in args {
        if let ResolvedBuilderArg::Named { name, .. } = arg
            && !named.insert(name)
        {
            errors.push(TypeCheckError::new(format!(
                "decoration `{decoration}` builder `{builder}` receives duplicate bound argument `{name}`"
            )));
        }
    }

    match kind.shape() {
        DecorationBuilderShape::Empty if !args.is_empty() => errors.push(TypeCheckError::new(format!(
            "decoration `{decoration}` builder `{builder}` expands to arguments, but emphasis builders require zero arguments"
        ))),
        DecorationBuilderShape::Scalar => {
            let valid = match args {
                [ResolvedBuilderArg::Positional { .. }] => true,
                [ResolvedBuilderArg::Named { name, .. }] => name == "value",
                _ => false,
            };
            if !valid {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` builder `{builder}` must expand to exactly one scalar value"
                )));
            }
        }
        DecorationBuilderShape::ClosedSelector | DecorationBuilderShape::OpenSelector => {
            if !matches!(
                args.first(),
                Some(ResolvedBuilderArg::Positional { selector: true })
            ) {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` builder `{builder}` must expand with one leading `.name` selector"
                )));
            }
            if args
                .iter()
                .skip(1)
                .any(|arg| matches!(arg, ResolvedBuilderArg::Positional { .. }))
            {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` builder `{builder}` expands with an extra positional argument"
                )));
            }
            if kind == DecorationBuilderKind::Effect
                && args.iter().any(|arg| {
                    matches!(
                        arg,
                        ResolvedBuilderArg::Named { name, value }
                            if name == "phase"
                                && value.bound
                                && normalize_decoration_value(&value.value)
                                    .trim_start_matches('.') == "host_event"
                    )
                })
            {
                errors.push(TypeCheckError::new(format!(
                    "decoration `{decoration}` expands an effect with `phase=host_event`, which is not a visual layer"
                )));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod budget_tests {
    use super::{DECORATION_EXPANSION_LIMITS, ExpansionBudget, TypeCheckError};

    #[test]
    fn exact_sema_expansion_limits_are_inclusive() {
        let mut errors = Vec::<TypeCheckError>::new();
        let mut depth = ExpansionBudget::new("depth");
        for current in 0..DECORATION_EXPANSION_LIMITS.max_depth {
            assert!(depth.enter_definition("node", current, &mut errors));
        }
        assert!(errors.is_empty());
        assert!(!depth.enter_definition(
            "overflow",
            DECORATION_EXPANSION_LIMITS.max_depth,
            &mut errors
        ));

        let mut visits = ExpansionBudget::new("visits");
        let mut visit_errors = Vec::new();
        for _ in 0..DECORATION_EXPANSION_LIMITS.max_visits {
            assert!(visits.enter_definition("node", 0, &mut visit_errors));
        }
        assert!(visit_errors.is_empty());
        assert!(!visits.enter_definition("overflow", 0, &mut visit_errors));

        let mut layers = ExpansionBudget::new("layers");
        let mut layer_errors = Vec::new();
        for _ in 0..DECORATION_EXPANSION_LIMITS.max_layers {
            assert!(layers.add_layer(&mut layer_errors));
        }
        assert!(layer_errors.is_empty());
        assert!(!layers.add_layer(&mut layer_errors));
    }
}
