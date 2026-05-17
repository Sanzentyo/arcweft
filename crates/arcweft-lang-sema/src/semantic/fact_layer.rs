use arcweft_lang_syntax::{ContractClause, Expr};
use std::collections::BTreeSet;

/// Capability granted by an effects clause or an external checker environment.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct Capability(String);

/// Effect permissions active while checking a flow, function, or hook body.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct EffectScope {
    capabilities: BTreeSet<Capability>,
}

/// Proof facts extracted from a top-level `proof` item.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProofFacts {
    lifetime_targets: BTreeSet<String>,
}

impl Capability {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl EffectScope {
    pub(super) fn from_contracts(contracts: &[ContractClause]) -> Self {
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

    pub(super) fn from_effects(effects: &[Expr]) -> Self {
        Self {
            capabilities: effects.iter().filter_map(capability_from_expr).collect(),
        }
    }

    pub(super) fn contains(&self, capability: &Capability) -> bool {
        self.capabilities.contains(capability)
    }
}

impl ProofFacts {
    pub(super) fn from_body(body: &str) -> Self {
        Self {
            lifetime_targets: collect_proof_lifetime_targets(body),
        }
    }

    pub(super) fn discharges_target(&self, target: &str) -> bool {
        self.lifetime_targets.contains(target)
    }
}

pub(super) fn write_capability_for_method(receiver: &Expr, method: &str) -> Option<Capability> {
    if method != "set" {
        return None;
    }
    match receiver {
        Expr::Path(path) if matches!(path.as_str(), "signal" | "metric") => {
            Some(Capability::new(format!("{path}.write")))
        }
        _ => None,
    }
}

pub(super) fn write_capability_for_call(callee: &Expr) -> Option<Capability> {
    match expr_path_label(callee).as_deref() {
        Some("signal.set") => Some(Capability::new("signal.write")),
        Some("metric.set") => Some(Capability::new("metric.write")),
        _ => None,
    }
}

pub(super) fn capability_from_expr(expr: &Expr) -> Option<Capability> {
    expr_path_label(expr).map(Capability::new)
}

fn expr_path_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.clone()),
        Expr::Field { target, field } => {
            expr_path_label(target).map(|target| format!("{target}.{field}"))
        }
        _ => None,
    }
}

fn collect_proof_lifetime_targets(body: &str) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    let bytes = body.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\'' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'.' | b':'))
        {
            index += 1;
        }
        if index > start + 1 {
            targets.insert(body[start..index].to_owned());
        }
    }
    targets
}
