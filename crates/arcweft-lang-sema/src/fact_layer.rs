use arcweft_lang_syntax::{
    ast::{flow::ContractClause, proof::ProofClause},
    expr::{CallArg, Expr, LifetimeScopeKind},
};
use std::collections::BTreeSet;

/// Capability granted by an effects clause or an external checker environment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Capability(String);

/// Effect permissions active while checking a flow, function, or hook body.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EffectScope {
    capabilities: BTreeSet<Capability>,
}

/// Operation kind used by deterministic resource conflict checks.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum EffectAccess {
    Write,
}

/// Runtime or semantic resource touched by an effectful operation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum EffectResource {
    Lifetime(String),
    Signal(String),
    Metric(String),
}

/// One typed access fact used by semantic conflict checking.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResourceAccess {
    resource: EffectResource,
    access: EffectAccess,
}

/// Proof facts extracted from a top-level `proof` item.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProofFacts {
    checked_lifetime_targets: BTreeSet<String>,
    issues: Vec<ProofIssue>,
}

/// Validation issue found in a source proof body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProofIssue {
    message: String,
    subject: Option<String>,
}

impl Capability {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl EffectScope {
    pub(crate) fn from_contracts(contracts: &[ContractClause]) -> Self {
        let capabilities = contracts
            .iter()
            .filter_map(|contract| match contract {
                ContractClause::Effects(effects) => Some(effects),
                _ => None,
            })
            .flat_map(|effects| effects.iter().filter_map(capability_from_expr))
            .collect();
        Self { capabilities }
    }

    pub(crate) fn from_effects(effects: &[Expr]) -> Self {
        Self {
            capabilities: effects.iter().filter_map(capability_from_expr).collect(),
        }
    }

    pub(crate) fn contains(&self, capability: &Capability) -> bool {
        self.capabilities.contains(capability)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }
}

impl ResourceAccess {
    pub(crate) fn write(resource: EffectResource) -> Self {
        Self {
            resource,
            access: EffectAccess::Write,
        }
    }

    pub(crate) fn key(&self) -> String {
        format!("{}:{}", self.resource.family(), self.resource.label())
    }
}

impl EffectResource {
    fn family(&self) -> &'static str {
        match self {
            Self::Lifetime(_) => "lifetime",
            Self::Signal(_) => "signal",
            Self::Metric(_) => "metric",
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Lifetime(label) | Self::Signal(label) | Self::Metric(label) => label,
        }
    }
}

impl ProofFacts {
    pub(crate) fn from_clauses(clauses: &[ProofClause], known_axioms: &BTreeSet<String>) -> Self {
        collect_proof_facts(clauses, known_axioms)
    }

    pub(crate) fn discharges_target(&self, target: &str) -> bool {
        self.issues.is_empty() && self.checked_lifetime_targets.contains(target)
    }

    pub(crate) fn issues(&self) -> &[ProofIssue] {
        &self.issues
    }
}

impl ProofIssue {
    pub(crate) fn new(message: impl Into<String>, subject: Option<String>) -> Self {
        Self {
            message: message.into(),
            subject,
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }
}

pub(crate) fn write_capability_for_call(callee: &Expr) -> Option<Capability> {
    match expr_path_label(callee).as_deref() {
        Some("signal.set") => Some(Capability::new("signal.write")),
        Some("metric.set") => Some(Capability::new("metric.write")),
        _ => None,
    }
}

pub(crate) fn capability_from_expr(expr: &Expr) -> Option<Capability> {
    if let Expr::Call { callee, args } = expr
        && expr_path_label(callee).as_deref() == Some("state.write")
    {
        return state_write_capability(args);
    }
    if let Expr::Call { callee, .. } = expr
        && let Some(callee) = expr_path_label(callee)
    {
        return Some(Capability::new(callee));
    }
    expr_path_label(expr).map(Capability::new)
}

fn expr_path_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::ShortVariant(name) => Some(format!(".{name}")),
        _ => expr.dotted_selector_label(),
    }
}

fn lifetime_scope_arg(expr: &Expr) -> Option<LifetimeScopeKind> {
    match expr {
        Expr::LifetimePath { key, .. } => Some(key.scope().clone()),
        Expr::Path(path) => path.strip_prefix('\'').map(LifetimeScopeKind::parse),
        _ => None,
    }
}

fn state_write_capability(args: &[CallArg]) -> Option<Capability> {
    args.first()
        .and_then(|arg| lifetime_scope_arg(arg.value()))
        .map(|scope| Capability::new(format!("state.write({})", scope.as_str())))
}

pub(crate) fn resource_write_for_lifetime(label: impl Into<String>) -> ResourceAccess {
    ResourceAccess::write(EffectResource::Lifetime(label.into()))
}

pub(crate) fn resource_write_for_signal(label: impl Into<String>) -> ResourceAccess {
    ResourceAccess::write(EffectResource::Signal(label.into()))
}

pub(crate) fn resource_accesses_from_expr(expr: &Expr) -> BTreeSet<ResourceAccess> {
    let mut accesses = BTreeSet::new();
    collect_resource_accesses_from_expr(expr, &mut accesses);
    accesses
}

fn collect_resource_accesses_from_expr(expr: &Expr, accesses: &mut BTreeSet<ResourceAccess>) {
    match expr {
        Expr::Call { callee, args }
            if matches!(
                expr_path_label(callee).as_deref(),
                Some("signal.set" | "metric.set")
            ) =>
        {
            if let Some(target) = args.first() {
                match expr_path_label(callee).as_deref() {
                    Some("signal.set") => {
                        accesses.insert(ResourceAccess::write(EffectResource::Signal(expr_label(
                            target.value(),
                        ))));
                    }
                    Some("metric.set") => {
                        accesses.insert(ResourceAccess::write(EffectResource::Metric(expr_label(
                            target.value(),
                        ))));
                    }
                    _ => {}
                }
            }
            collect_resource_accesses_from_expr(callee, accesses);
            for arg in args {
                collect_resource_accesses_from_expr(arg.value(), accesses);
            }
        }
        Expr::Call { callee, args } => {
            collect_resource_accesses_from_expr(callee, accesses);
            for arg in args {
                collect_resource_accesses_from_expr(arg.value(), accesses);
            }
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_resource_accesses_from_expr(item, accesses);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_resource_accesses_from_expr(value, accesses);
            collect_resource_accesses_from_expr(len, accesses);
        }
        Expr::Select(select) => collect_resource_accesses_from_expr(select.target(), accesses),
        Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. } => collect_resource_accesses_from_expr(value, accesses),
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => {
            collect_resource_accesses_from_expr(lhs, accesses);
            collect_resource_accesses_from_expr(rhs, accesses);
        }
        _ => {}
    }
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.as_label().to_owned(),
        Expr::ShortVariant(name) => format!(".{name}"),
        Expr::EntityRef(entity) => entity.body().to_owned(),
        Expr::LifetimePath { key, .. } => key.as_dotted(),
        Expr::Select(select) => format!("{}.{}", expr_label(select.target()), select.member()),
        Expr::Literal(literal) => format!("{literal:?}"),
        _ => format!("{expr:?}"),
    }
}

fn collect_proof_facts(clauses: &[ProofClause], known_axioms: &BTreeSet<String>) -> ProofFacts {
    let checked_lifetime_targets = collect_checked_lifetime_targets(clauses);
    let mut issues = Vec::new();
    if checked_lifetime_targets.is_empty() {
        issues.push(ProofIssue::new(
            "proof body must contain an `ensures` or `check` clause for the proven lifetime target",
            None,
        ));
    }
    for clause in clauses {
        match clause {
            ProofClause::Assume {
                source,
                reason,
                axiom,
            } => {
                if reason.is_none() && axiom.is_none() {
                    issues.push(ProofIssue::new(
                        "proof `assume` must cite a reason or trusted axiom",
                        Some(source.to_owned()),
                    ));
                }
                if let Some(axiom) = axiom
                    && !known_axioms.contains(axiom)
                {
                    issues.push(ProofIssue::new(
                        format!("proof references unknown trusted axiom `{axiom}`"),
                        Some(source.to_owned()),
                    ));
                }
            }
            ProofClause::UseAxiom { id } if !known_axioms.contains(id) => {
                issues.push(ProofIssue::new(
                    format!("proof references unknown trusted axiom `{id}`"),
                    Some(id.to_owned()),
                ));
            }
            ProofClause::Raw { source } => issues.push(ProofIssue::new(
                "unrecognized proof clause",
                Some(source.to_owned()),
            )),
            _ => {}
        }
    }
    ProofFacts {
        checked_lifetime_targets,
        issues,
    }
}

fn collect_checked_lifetime_targets(clauses: &[ProofClause]) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    for clause in clauses {
        match clause {
            ProofClause::Ensures {
                lifetime_targets, ..
            }
            | ProofClause::Check {
                lifetime_targets, ..
            } => {
                targets.extend(lifetime_targets.iter().cloned());
            }
            _ => {}
        }
    }
    targets
}
