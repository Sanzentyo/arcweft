//! Typed source-role vocabulary for native Style item owners.

use super::flow_role::HirFlowSourceRole;

/// Source-order path to one native Style body nested through environment bodies.
///
/// The empty path selects the outer sheet body. Every ordinal is an index into
/// the current body's retained `HirStyleBodyItem` array and must select an
/// environment before descending into that environment's nested body. Payload
/// validation owns applicability; this value retains only the exact typed
/// ordinal sequence and invents no depth limit or secondary Style identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirStyleBodyPath(Box<[u32]>);

impl HirStyleBodyPath {
    /// Selects the outer body of one Style item.
    pub(crate) fn root() -> Self {
        Self(Box::new([]))
    }

    /// Owns the exact source-order environment path supplied by Style lowering.
    pub(crate) fn from_ordinals(ordinals: Box<[u32]>) -> Self {
        Self(ordinals)
    }

    /// Returns the source-order environment ordinals from outermost to innermost.
    pub(crate) const fn ordinals(&self) -> &[u32] {
        &self.0
    }
}

/// Source component of one top-level native Style token.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirStyleTokenSourcePart {
    Whole,
    Key,
    Assignment,
}

/// Source component selected inside one native Style body.
///
/// `HirStyleBodyPath` selects the containing body. Ordinals in these variants
/// then select direct members of that body's retained `HirStyleBodyItem`
/// array and their ordered descendants. The `rule` and `environment` fields
/// are therefore body-item indices, not family-filtered or raw syntax
/// ordinals; sequence, predicate, declaration, and clause fields index their
/// respective semantic arrays.
/// Expression initializers and token type annotations deliberately do not
/// appear here: their existing `ExprId` and `TypeId` source roles remain the
/// sole owners of those component trees.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirStyleBodySourcePart {
    BodyWhole,
    RuleSelector {
        rule: u32,
    },
    RuleSequence {
        rule: u32,
        sequence: u32,
    },
    RuleElement {
        rule: u32,
        sequence: u32,
    },
    RulePart {
        rule: u32,
        sequence: u32,
    },
    RulePredicate {
        rule: u32,
        sequence: u32,
        predicate: u32,
    },
    DeclarationWhole {
        rule: u32,
        declaration: u32,
    },
    DeclarationProperty {
        rule: u32,
        declaration: u32,
    },
    DeclarationAssignment {
        rule: u32,
        declaration: u32,
    },
    EnvironmentWhole {
        environment: u32,
    },
    EnvironmentCondition {
        environment: u32,
    },
    EnvironmentBody {
        environment: u32,
    },
    ClauseWhole {
        environment: u32,
        clause: u32,
    },
    ClauseField {
        environment: u32,
        clause: u32,
    },
    ClauseComparison {
        environment: u32,
        clause: u32,
    },
}

/// Typed native Style component owned by one Style item.
///
/// Token ordinals index the source-ordered `HirStyleItem::tokens` inventory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirStyleSourceRole {
    ItemId,
    Token {
        ordinal: u32,
        part: HirStyleTokenSourcePart,
    },
    Body {
        path: HirStyleBodyPath,
        part: HirStyleBodySourcePart,
    },
}

/// Typed item source-role family admitted by the sole source index.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirItemSourceRole {
    Flow(HirFlowSourceRole),
    Style(HirStyleSourceRole),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        HirStyleBodyPath, HirStyleBodySourcePart, HirStyleSourceRole, HirStyleTokenSourcePart,
    };

    #[test]
    fn body_path_owns_the_complete_unbounded_environment_ordinal_sequence() {
        let root = HirStyleBodyPath::root();
        let nested = HirStyleBodyPath::from_ordinals(vec![2, 4, 8, 16].into_boxed_slice());

        assert!(root.ordinals().is_empty());
        assert_eq!(nested.ordinals(), &[2, 4, 8, 16]);
        assert_eq!(nested.clone(), nested);
        assert_ne!(root, nested);
    }

    #[test]
    fn style_role_vocabulary_is_closed_and_each_part_is_a_distinct_typed_key() {
        let root = HirStyleBodyPath::root();
        let body_parts = [
            HirStyleBodySourcePart::BodyWhole,
            HirStyleBodySourcePart::RuleSelector { rule: 1 },
            HirStyleBodySourcePart::RuleSequence {
                rule: 1,
                sequence: 2,
            },
            HirStyleBodySourcePart::RuleElement {
                rule: 1,
                sequence: 2,
            },
            HirStyleBodySourcePart::RulePart {
                rule: 1,
                sequence: 2,
            },
            HirStyleBodySourcePart::RulePredicate {
                rule: 1,
                sequence: 2,
                predicate: 3,
            },
            HirStyleBodySourcePart::DeclarationWhole {
                rule: 1,
                declaration: 4,
            },
            HirStyleBodySourcePart::DeclarationProperty {
                rule: 1,
                declaration: 4,
            },
            HirStyleBodySourcePart::DeclarationAssignment {
                rule: 1,
                declaration: 4,
            },
            HirStyleBodySourcePart::EnvironmentWhole { environment: 5 },
            HirStyleBodySourcePart::EnvironmentCondition { environment: 5 },
            HirStyleBodySourcePart::EnvironmentBody { environment: 5 },
            HirStyleBodySourcePart::ClauseWhole {
                environment: 5,
                clause: 6,
            },
            HirStyleBodySourcePart::ClauseField {
                environment: 5,
                clause: 6,
            },
            HirStyleBodySourcePart::ClauseComparison {
                environment: 5,
                clause: 6,
            },
        ];
        let mut roles = BTreeSet::from([
            HirStyleSourceRole::ItemId,
            HirStyleSourceRole::Token {
                ordinal: 0,
                part: HirStyleTokenSourcePart::Whole,
            },
            HirStyleSourceRole::Token {
                ordinal: 0,
                part: HirStyleTokenSourcePart::Key,
            },
            HirStyleSourceRole::Token {
                ordinal: 0,
                part: HirStyleTokenSourcePart::Assignment,
            },
        ]);
        roles.extend(body_parts.map(|part| HirStyleSourceRole::Body {
            path: root.clone(),
            part,
        }));

        assert_eq!(roles.len(), 19);
    }
}
