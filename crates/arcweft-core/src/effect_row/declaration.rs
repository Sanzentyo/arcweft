//! Inert canonical graph declarations and checked admission.
//!
//! Decoders bound their table counts before allocation. Admission below is a
//! linear graph walk plus ordered uniqueness checks; it never normalizes an
//! untrusted graph or runs Boolean elimination. Only admitted graphs can enter
//! the shared algebra.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_id::EffectId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use super::{
    EffectFormula, EffectPredicate,
    decision::{EffectDecision, Node, Root},
    membership::Membership,
};

/// A canonical decision edge. Node ordinals are bounded independently of the
/// host's pointer width and always refer to an earlier node in the same graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "index", rename_all = "snake_case")]
pub enum EffectDecisionTarget {
    False,
    True,
    Node(u32),
}

/// One inert decision node. Public fields grant no algebra or scope authority.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectDecisionNodeDeclaration<V> {
    pub variable: V,
    pub low: EffectDecisionTarget,
    pub high: EffectDecisionTarget,
}

/// Nodes in exact reachable low-before-high postorder.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectDecisionDeclaration<V> {
    pub nodes: Box<[EffectDecisionNodeDeclaration<V>]>,
    pub root: EffectDecisionTarget,
}

/// Shared declaration grammar for a finite row and a universal predicate.
/// Overrides are strictly ordered by their canonical effect identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "V: Serialize", deserialize = "V: Deserialize<'de>")
)]
pub struct EffectMembershipDeclaration<V> {
    pub default: EffectDecisionDeclaration<V>,
    #[serde(with = "effect_overrides")]
    pub overrides: Box<[(EffectId, EffectDecisionDeclaration<V>)]>,
}

mod effect_overrides {
    use super::*;

    pub(super) fn serialize<V: Serialize, S: Serializer>(
        overrides: &[(EffectId, EffectDecisionDeclaration<V>)],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(
            overrides
                .iter()
                .map(|(label, decision)| (label.as_str(), decision)),
        )
    }

    pub(super) fn deserialize<'de, V: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Box<[(EffectId, EffectDecisionDeclaration<V>)]>, D::Error> {
        Box::<[(String, EffectDecisionDeclaration<V>)]>::deserialize(deserializer)?
            .into_vec()
            .into_iter()
            .map(|(label, decision)| {
                Ok((
                    EffectId::parse(label).map_err(serde::de::Error::custom)?,
                    decision,
                ))
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum EffectDeclarationError {
    #[error("effect decision graph exceeds the u32 ordinal domain")]
    TooManyNodes,
    #[error("effect decision edge is not an earlier node in the same graph")]
    InvalidEdge,
    #[error("effect decision node has equal low and high edges")]
    UnreducedNode,
    #[error("effect decision variables are not ordered along an edge")]
    UnorderedVariable,
    #[error("effect decision graph repeats a node")]
    DuplicateNode,
    #[error("effect decision nodes are not in canonical reachable postorder")]
    NonCanonicalPostorder,
    #[error("effect membership overrides are not strictly ordered and unique")]
    NonCanonicalOverrides,
    #[error("effect membership override equals the default decision")]
    RedundantOverride,
    #[error("effect row is not finite for finite row inputs")]
    InfiniteRow,
    #[error("globally impossible effect predicate is not canonical")]
    NonCanonicalPredicate,
}

impl EffectDecisionTarget {
    fn internal(self) -> Result<Root, EffectDeclarationError> {
        Ok(match self {
            Self::False => Root::False,
            Self::True => Root::True,
            Self::Node(index) => Root::Branch(
                usize::try_from(index).map_err(|_| EffectDeclarationError::InvalidEdge)?,
            ),
        })
    }

    fn from_internal(root: Root) -> Result<Self, EffectDeclarationError> {
        Ok(match root {
            Root::False => Self::False,
            Root::True => Self::True,
            Root::Branch(index) => {
                Self::Node(u32::try_from(index).map_err(|_| EffectDeclarationError::TooManyNodes)?)
            }
        })
    }
}

impl<V: Clone + Ord> TryFrom<EffectDecisionDeclaration<V>> for EffectDecision<V> {
    type Error = EffectDeclarationError;

    fn try_from(declaration: EffectDecisionDeclaration<V>) -> Result<Self, Self::Error> {
        let count = declaration.nodes.len();
        u32::try_from(count).map_err(|_| EffectDeclarationError::TooManyNodes)?;
        let mut unique = BTreeSet::new();
        for (index, node) in declaration.nodes.iter().enumerate() {
            if node.low == node.high {
                return Err(EffectDeclarationError::UnreducedNode);
            }
            for edge in [node.low, node.high] {
                if let Root::Branch(child) = edge.internal()? {
                    if child >= index {
                        return Err(EffectDeclarationError::InvalidEdge);
                    }
                    if declaration.nodes[child].variable <= node.variable {
                        return Err(EffectDeclarationError::UnorderedVariable);
                    }
                }
            }
            if !unique.insert(node) {
                return Err(EffectDeclarationError::DuplicateNode);
            }
        }
        drop(unique);
        let root = declaration.root.internal()?;
        if matches!(root, Root::Branch(index) if index >= count) {
            return Err(EffectDeclarationError::InvalidEdge);
        }
        let mut visited = vec![false; count];
        let mut pending = vec![(root, false)];
        let mut ordinal = 0;
        while let Some((current, finish)) = pending.pop() {
            let Root::Branch(index) = current else {
                continue;
            };
            if finish {
                if index != ordinal {
                    return Err(EffectDeclarationError::NonCanonicalPostorder);
                }
                ordinal += 1;
            } else if !visited[index] {
                visited[index] = true;
                let node = &declaration.nodes[index];
                pending.push((current, true));
                pending.push((node.high.internal()?, false));
                pending.push((node.low.internal()?, false));
            }
        }
        if ordinal != count {
            return Err(EffectDeclarationError::NonCanonicalPostorder);
        }
        Ok(Self {
            nodes: declaration
                .nodes
                .into_vec()
                .into_iter()
                .map(|node| {
                    Ok(Node {
                        variable: node.variable,
                        low: node.low.internal()?,
                        high: node.high.internal()?,
                    })
                })
                .collect::<Result<Box<[_]>, Self::Error>>()?,
            root,
        })
    }
}

impl<V: Clone + Ord> TryFrom<&EffectDecision<V>> for EffectDecisionDeclaration<V> {
    type Error = EffectDeclarationError;

    fn try_from(decision: &EffectDecision<V>) -> Result<Self, Self::Error> {
        u32::try_from(decision.nodes.len()).map_err(|_| EffectDeclarationError::TooManyNodes)?;
        Ok(Self {
            nodes: decision
                .nodes
                .iter()
                .map(|node| {
                    Ok(EffectDecisionNodeDeclaration {
                        variable: node.variable.clone(),
                        low: EffectDecisionTarget::from_internal(node.low)?,
                        high: EffectDecisionTarget::from_internal(node.high)?,
                    })
                })
                .collect::<Result<Box<[_]>, Self::Error>>()?,
            root: EffectDecisionTarget::from_internal(decision.root)?,
        })
    }
}

impl<V: Clone + Ord> TryFrom<EffectMembershipDeclaration<V>> for Membership<V> {
    type Error = EffectDeclarationError;

    fn try_from(declaration: EffectMembershipDeclaration<V>) -> Result<Self, Self::Error> {
        if declaration
            .overrides
            .windows(2)
            .any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(EffectDeclarationError::NonCanonicalOverrides);
        }
        let default = EffectDecision::try_from(declaration.default)?;
        let mut overrides = BTreeMap::new();
        for (label, declaration) in declaration.overrides {
            let decision = EffectDecision::try_from(declaration)?;
            if decision == default {
                return Err(EffectDeclarationError::RedundantOverride);
            }
            overrides.insert(label, decision);
        }
        Ok(Self { default, overrides })
    }
}

impl<V: Clone + Ord> TryFrom<&Membership<V>> for EffectMembershipDeclaration<V> {
    type Error = EffectDeclarationError;

    fn try_from(membership: &Membership<V>) -> Result<Self, Self::Error> {
        Ok(Self {
            default: EffectDecisionDeclaration::try_from(&membership.default)?,
            overrides: membership
                .overrides
                .iter()
                .map(|(label, decision)| {
                    Ok((
                        label.clone(),
                        EffectDecisionDeclaration::try_from(decision)?,
                    ))
                })
                .collect::<Result<Box<[_]>, Self::Error>>()?,
        })
    }
}

impl<V: Clone + Ord> TryFrom<EffectMembershipDeclaration<V>> for EffectFormula<V> {
    type Error = EffectDeclarationError;

    fn try_from(declaration: EffectMembershipDeclaration<V>) -> Result<Self, Self::Error> {
        let membership = Membership::try_from(declaration)?;
        if membership.default.evaluate(|_| false) {
            return Err(EffectDeclarationError::InfiniteRow);
        }
        Ok(Self(membership))
    }
}

impl<V: Clone + Ord> TryFrom<EffectMembershipDeclaration<V>> for EffectPredicate<V> {
    type Error = EffectDeclarationError;

    fn try_from(declaration: EffectMembershipDeclaration<V>) -> Result<Self, Self::Error> {
        let membership = Membership::try_from(declaration)?;
        if !membership.default.evaluate(|_| false)
            && !(membership.default.is_constant(false) && membership.overrides.is_empty())
        {
            return Err(EffectDeclarationError::NonCanonicalPredicate);
        }
        Ok(Self(membership))
    }
}

impl<V: Clone + Ord> TryFrom<&EffectFormula<V>> for EffectMembershipDeclaration<V> {
    type Error = EffectDeclarationError;

    fn try_from(formula: &EffectFormula<V>) -> Result<Self, Self::Error> {
        Self::try_from(&formula.0)
    }
}

impl<V: Clone + Ord> TryFrom<&EffectPredicate<V>> for EffectMembershipDeclaration<V> {
    type Error = EffectDeclarationError;

    fn try_from(predicate: &EffectPredicate<V>) -> Result<Self, Self::Error> {
        Self::try_from(&predicate.0)
    }
}

impl<V: Clone + Ord + Serialize> Serialize for EffectFormula<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        EffectMembershipDeclaration::try_from(self)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de, V: Clone + Ord + Deserialize<'de>> Deserialize<'de> for EffectFormula<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(EffectMembershipDeclaration::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

impl<V: Clone + Ord + Serialize> Serialize for EffectPredicate<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        EffectMembershipDeclaration::try_from(self)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de, V: Clone + Ord + Deserialize<'de>> Deserialize<'de> for EffectPredicate<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(EffectMembershipDeclaration::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests;
