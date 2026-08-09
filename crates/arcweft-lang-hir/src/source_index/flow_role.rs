//! Typed source-role vocabulary for one ordinary Flow item.

/// Source component of one source-ordered Flow parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowParameterSourcePart {
    Whole,
    Pattern,
    Colon,
    Type,
}

/// Source component of the optional authored Flow return wrapper.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowReturnSourcePart {
    Whole,
    Arrow,
    Type,
}

/// Source component of one heterogeneous Flow contract clause.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowContractSourcePart {
    Whole,
    ClauseKeyword,
    NoEffectKeyword,
    Mode,
    Operand { ordinal: u16 },
    OpenDelimiter,
    CloseDelimiter,
}

/// Closed item-owned source family for an ordinary Flow declaration.
///
/// Child expression, pattern, and type trees remain owned by their existing
/// typed source families. Ordinals retain accepted semantic source order and
/// are validated against the final Flow payload before source identity is
/// checked.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowSourceRole {
    Whole,
    Keyword,
    Visibility,
    PublicId,
    Name,
    GenericGroup,
    GenericParameter {
        ordinal: u16,
    },
    ParameterGroup,
    Parameter {
        ordinal: u16,
        part: HirFlowParameterSourcePart,
    },
    Return {
        part: HirFlowReturnSourcePart,
    },
    WhereClause,
    WherePredicate {
        ordinal: u16,
    },
    ContractClause {
        ordinal: u16,
        part: HirFlowContractSourcePart,
    },
    Body,
    BodyOpen,
    BodyClose,
    TrailingRecovery {
        ordinal: u32,
    },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        HirFlowContractSourcePart, HirFlowParameterSourcePart, HirFlowReturnSourcePart,
        HirFlowSourceRole,
    };

    #[test]
    fn flow_role_vocabulary_keeps_clause_and_no_effect_keywords_distinct() {
        let roles = BTreeSet::from([
            HirFlowSourceRole::ContractClause {
                ordinal: 3,
                part: HirFlowContractSourcePart::ClauseKeyword,
            },
            HirFlowSourceRole::ContractClause {
                ordinal: 3,
                part: HirFlowContractSourcePart::NoEffectKeyword,
            },
            HirFlowSourceRole::Parameter {
                ordinal: 2,
                part: HirFlowParameterSourcePart::Colon,
            },
            HirFlowSourceRole::Return {
                part: HirFlowReturnSourcePart::Arrow,
            },
        ]);

        assert_eq!(roles.len(), 4);
    }
}
