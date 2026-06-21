use std::collections::{BTreeMap, btree_map::Entry};

use thiserror::Error;

use crate::effects::{EffectId, EffectIdError};

/// Scope arity accepted by one effect path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectScopeArity {
    Forbidden,
    OptionalOne,
    Exactly(usize),
    Any,
}

/// Catalog entry for one declared/builtin effect path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectDefinition {
    path: String,
    scope_arity: EffectScopeArity,
}

/// Known effect identities for one compilation environment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectCatalog(BTreeMap<String, EffectDefinition>);

/// Failure while registering or validating an effect definition.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EffectCatalogError {
    #[error(transparent)]
    InvalidDefinition(#[from] EffectIdError),
    #[error("effect catalog already defines `{path}`")]
    DuplicateDefinition { path: String },
    #[error("effect `{effect}` is not declared by a builtin, capability, or adapter")]
    UnknownEffect { effect: EffectId },
    #[error("effect `{effect}` has {actual} scope arguments, but `{path}` requires {expected}")]
    ScopeArityMismatch {
        effect: EffectId,
        path: String,
        expected: &'static str,
        actual: usize,
    },
}

impl EffectDefinition {
    pub fn new(
        path: impl AsRef<str>,
        scope_arity: EffectScopeArity,
    ) -> Result<Self, EffectIdError> {
        let effect = EffectId::parse(path.as_ref())?;
        if effect.scope_count() > 0 {
            return Err(EffectIdError::MalformedScope {
                value: path.as_ref().to_owned(),
            });
        }
        Ok(Self {
            path: effect.path().to_owned(),
            scope_arity,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn scope_arity(&self) -> EffectScopeArity {
        self.scope_arity
    }
}

impl EffectCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, definition: EffectDefinition) -> Result<(), EffectCatalogError> {
        let path = definition.path().to_owned();
        match self.0.entry(path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(definition);
                Ok(())
            }
            Entry::Occupied(_) => Err(EffectCatalogError::DuplicateDefinition { path }),
        }
    }

    pub fn validate_all<'a>(
        &self,
        effects: impl IntoIterator<Item = &'a EffectId>,
    ) -> Result<(), EffectCatalogError> {
        effects
            .into_iter()
            .try_for_each(|effect| self.validate(effect))
    }

    pub fn validate(&self, effect: &EffectId) -> Result<(), EffectCatalogError> {
        let definition =
            self.0
                .get(effect.path())
                .ok_or_else(|| EffectCatalogError::UnknownEffect {
                    effect: effect.clone(),
                })?;
        let actual = effect.scope_count();
        let valid = match definition.scope_arity() {
            EffectScopeArity::Forbidden => actual == 0,
            EffectScopeArity::OptionalOne => actual <= 1,
            EffectScopeArity::Exactly(expected) => actual == expected,
            EffectScopeArity::Any => true,
        };
        if valid {
            Ok(())
        } else {
            Err(EffectCatalogError::ScopeArityMismatch {
                effect: effect.clone(),
                path: definition.path().to_owned(),
                expected: definition.scope_arity().description(),
                actual,
            })
        }
    }
}

impl EffectScopeArity {
    const fn description(self) -> &'static str {
        match self {
            Self::Forbidden | Self::Exactly(0) => "no scope arguments",
            Self::OptionalOne => "zero or one scope argument",
            Self::Exactly(1) => "exactly one scope argument",
            Self::Exactly(_) => "an exact declared number of scope arguments",
            Self::Any => "any number of scope arguments",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_rejects_unknown_and_wrong_scope_effects() {
        let mut catalog = EffectCatalog::new();
        catalog
            .register(
                EffectDefinition::new("state.write", EffectScopeArity::Exactly(1))
                    .expect("valid definition"),
            )
            .expect("unique definition");

        assert!(
            catalog
                .validate(&EffectId::parse("state.write(flow)").unwrap())
                .is_ok()
        );
        assert!(
            catalog
                .validate(&EffectId::parse("state.write").unwrap())
                .is_err()
        );
        assert!(
            catalog
                .validate(&EffectId::parse("state.typo(flow)").unwrap())
                .is_err()
        );
    }
}
