use std::{collections::BTreeSet, fmt, iter::FromIterator};

pub use arcweft_id::{EffectId, EffectIdError, EffectSemanticDigest};
use arcweft_lang_hir::{
    expr::{HirCallArgument, HirCallCallee, HirCallValue, HirExprKind, HirSelectedMember},
    identity::ExprId,
    leaf::{HirPathRoot, HirPathSegment},
    module::HirModule,
};
use thiserror::Error;

/// Invalid final-HIR projection of one authored effect capability.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirEffectProjectionError {
    #[error("effect expression {owner:?} is absent from its accepted HIR module")]
    InvalidOwner { owner: ExprId },
    #[error("effect expression {owner:?} contains recovered path structure")]
    Recovered { owner: ExprId },
    #[error("effect expression {owner:?} uses an explicit project root")]
    ExplicitRoot { owner: ExprId },
    #[error("effect expression {owner:?} is not a path/select chain")]
    Unsupported { owner: ExprId },
    #[error(transparent)]
    InvalidIdentity(#[from] EffectIdError),
}

/// Parse failure while constructing an effect set from source labels.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid effect at index {index}: {source}")]
pub struct EffectSetParseError {
    index: usize,
    #[source]
    source: EffectIdError,
}

/// Deterministically ordered set of canonical effects.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectSet(BTreeSet<EffectId>);

/// Projects one final-HIR path/select chain into its canonical semantic effect
/// identity and returns every participating expression owner.
///
/// Canonical identity parsing/digesting belongs to `arcweft-id`; this context
/// owns only the final-HIR traversal needed to issue that identity.
pub(crate) fn project_hir_effect_id(
    module: &HirModule,
    owner: ExprId,
) -> Result<(EffectId, Vec<ExprId>), HirEffectProjectionError> {
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| HirEffectProjectionError::InvalidOwner { owner })?;
    let mut segments = Vec::new();
    let mut owners = Vec::new();
    let identity = if let HirExprKind::Call(call) = expression.kind() {
        match call.callee() {
            HirCallCallee::Value { value } => {
                collect_hir_effect_path(module, *value, &mut segments, &mut owners)?;
            }
            HirCallCallee::UnresolvedDot {
                value_receiver,
                member,
                ..
            } => {
                collect_hir_effect_path(module, *value_receiver, &mut segments, &mut owners)?;
                let member = member
                    .resolved()
                    .ok_or(HirEffectProjectionError::Recovered { owner })?;
                segments.push(member.as_str().to_owned());
            }
            HirCallCallee::Associated { .. } => {
                return Err(HirEffectProjectionError::Unsupported { owner });
            }
        }
        let scopes = call
            .arguments()
            .iter()
            .map(|argument| {
                let HirCallArgument::Positional {
                    value: HirCallValue::Present { value },
                } = argument
                else {
                    return Err(HirEffectProjectionError::Unsupported { owner });
                };
                let mut scope_segments = Vec::new();
                collect_hir_effect_path(module, *value, &mut scope_segments, &mut owners)?;
                Ok(scope_segments.join("."))
            })
            .collect::<Result<Vec<_>, _>>()?;
        owners.push(owner);
        format!("{}({})", segments.join("."), scopes.join(","))
    } else {
        collect_hir_effect_path(module, owner, &mut segments, &mut owners)?;
        segments.join(".")
    };
    Ok((EffectId::parse(identity)?, owners))
}

fn collect_hir_effect_path(
    module: &HirModule,
    owner: ExprId,
    segments: &mut Vec<String>,
    owners: &mut Vec<ExprId>,
) -> Result<(), HirEffectProjectionError> {
    let expression = module
        .resolve_expr(owner)
        .map_err(|_| HirEffectProjectionError::InvalidOwner { owner })?;
    match expression.kind() {
        HirExprKind::Path(path) => {
            let path = path
                .as_resolved()
                .ok_or(HirEffectProjectionError::Recovered { owner })?;
            if path.root() != HirPathRoot::ImplicitCrate {
                return Err(HirEffectProjectionError::ExplicitRoot { owner });
            }
            segments.extend(path.segments().iter().map(|segment| match segment {
                HirPathSegment::Identifier(name) => name.as_str().to_owned(),
                HirPathSegment::ProjectSymbol(name) => name.as_str().to_owned(),
            }));
        }
        HirExprKind::Select(select) => {
            collect_hir_effect_path(module, select.target(), segments, owners)?;
            let HirSelectedMember::Name(name) = select.member() else {
                return Err(HirEffectProjectionError::Recovered { owner });
            };
            segments.push(name.as_str().to_owned());
        }
        _ => return Err(HirEffectProjectionError::Unsupported { owner }),
    }
    owners.push(owner);
    Ok(())
}

impl EffectSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_labels<I, S>(labels: I) -> Result<Self, EffectSetParseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                EffectId::parse(label).map_err(|source| EffectSetParseError { index, source })
            })
            .collect()
    }

    pub fn insert(&mut self, effect: EffectId) -> bool {
        self.0.insert(effect)
    }

    pub fn contains(&self, effect: &EffectId) -> bool {
        self.0.contains(effect)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &EffectId> + DoubleEndedIterator {
        self.0.iter()
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    pub fn union_with(&mut self, other: &Self) -> bool {
        let previous_len = self.len();
        self.0.extend(other.iter().cloned());
        self.len() != previous_len
    }

    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        self.iter().chain(other.iter()).cloned().collect()
    }

    #[must_use]
    pub fn difference(&self, other: &Self) -> Self {
        self.0.difference(&other.0).cloned().collect()
    }

    #[must_use]
    pub fn effects_not_covered_by(&self, covering: &Self) -> Self {
        self.iter()
            .filter(|effect| !covering.iter().any(|candidate| candidate.covers(effect)))
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        self.0.intersection(&other.0).cloned().collect()
    }

    pub fn to_labels(&self) -> Vec<String> {
        self.iter().map(ToString::to_string).collect()
    }
}

impl FromIterator<EffectId> for EffectSet {
    fn from_iter<T: IntoIterator<Item = EffectId>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for EffectSet {
    type Item = EffectId;
    type IntoIter = std::collections::btree_set::IntoIter<EffectId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a EffectSet {
    type Item = &'a EffectId;
    type IntoIter = std::collections::btree_set::Iter<'a, EffectId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("{")?;
        for (index, effect) in self.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{effect}")?;
        }
        formatter.write_str("}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_canonicalizes_effect_ids() {
        assert_eq!(
            EffectId::parse("state.write(flow)")
                .expect("valid effect")
                .as_str(),
            "state.write(flow)"
        );
        assert_eq!(
            EffectId::parse("agent.act.semantic")
                .expect("valid effect")
                .family(),
            "agent"
        );
    }

    #[test]
    fn rejects_noncanonical_effect_ids() {
        assert!(EffectId::parse("read").is_err());
        assert!(EffectId::parse("Fs.read").is_err());
        assert!(EffectId::parse("fs.read( )").is_err());
        assert!(EffectId::parse(" fs.read").is_err());
        assert!(EffectId::parse("fs.read ").is_err());
        assert!(EffectId::parse("fs.read(save").is_err());
    }

    #[test]
    fn effect_coverage_matches_scoped_and_unscoped_path_bounds() {
        let read = EffectId::parse("fs.read").expect("valid effect");
        let read_save = EffectId::parse("fs.read(save)").expect("valid effect");
        let read_asset = EffectId::parse("fs.read(asset)").expect("valid effect");

        assert!(read.covers(&read_save));
        assert!(read_save.covers(&read));
        assert!(!read_save.covers(&read_asset));
        assert!(!read_asset.covers(&read_save));
    }

    #[test]
    fn effect_set_reports_only_uncovered_effects() {
        let inferred =
            EffectSet::from_labels(["fs.read", "log.write"]).expect("valid inferred effects");
        let declared = EffectSet::from_labels(["fs.read(save)"]).expect("valid declared effects");

        assert_eq!(
            inferred.effects_not_covered_by(&declared).to_labels(),
            vec!["log.write"]
        );
    }

    #[test]
    fn effect_sets_are_sorted_and_deduplicated() {
        let effects = EffectSet::from_labels(["view.show", "fs.read", "view.show"])
            .expect("valid effect set");
        assert_eq!(effects.to_labels(), vec!["fs.read", "view.show"]);
    }

    #[test]
    fn effect_semantic_digest_is_canonical_and_payload_sensitive() {
        let first = EffectId::parse("fs.read(save)").expect("valid effect");
        let same = EffectId::parse("fs.read(save)").expect("same valid effect");
        let other_scope = EffectId::parse("fs.read(asset)").expect("valid effect");
        let other_path = EffectId::parse("fs.write(save)").expect("valid effect");

        assert_eq!(first.semantic_digest(), same.semantic_digest());
        assert_ne!(first.semantic_digest(), other_scope.semantic_digest());
        assert_ne!(first.semantic_digest(), other_path.semantic_digest());
    }
}
