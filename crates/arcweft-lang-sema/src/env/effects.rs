/// Canonical effect capability label tracked by semantic environments.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectCapability {
    label: String,
}

/// Parsed views of a canonical effect capability label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectCapabilityParts {
    family: String,
    operation: String,
    scope: Option<String>,
}

impl EffectCapability {
    /// Creates a canonical effect capability label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// Source-level capability label.
    pub fn as_str(&self) -> &str {
        &self.label
    }

    /// Returns the parsed family/operation/scope shape.
    pub fn parts(&self) -> EffectCapabilityParts {
        parse_effect_capability_parts(&self.label)
    }
}

impl EffectCapabilityParts {
    /// Capability namespace such as `fs` or `system`.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Operation such as `read` or `write`.
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Optional scoped selector from labels such as `state.write(flow)`.
    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }
}

impl From<&str> for EffectCapability {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for EffectCapability {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

fn parse_effect_capability_parts(label: &str) -> EffectCapabilityParts {
    let (body, scope) = label
        .strip_suffix(')')
        .and_then(|value| value.rsplit_once('('))
        .map_or((label, None), |(body, scope)| {
            (body, Some(scope.to_owned()))
        });
    let (family, operation) = body
        .split_once('.')
        .map_or((body, ""), |(family, operation)| (family, operation));
    EffectCapabilityParts {
        family: family.to_owned(),
        operation: operation.to_owned(),
        scope,
    }
}
