//! Function headers borrow their exact operand port and actual contents scope.

use super::*;
use crate::effect_row::{EffectPredicate, EffectRow};

pub(in crate::types) struct ConstraintFunctionHeaderView<'a, D: ConstraintDomain> {
    source: ConstraintOperandView<'a, D>,
    contents_scope: Arc<ConstraintLexicalScope>,
    row: &'a EffectRow,
    predicate: &'a EffectPredicate,
}

pub(in crate::types::constraints) struct ConstraintEffectHeaderDerivation<D: ConstraintDomain> {
    pub(in crate::types::constraints::transaction::operand) source:
        ConstraintOperandDerivation<D, TypeKind>,
    pub(in crate::types::constraints::transaction::operand) contents_scope:
        Arc<ConstraintLexicalScope>,
}

impl<'a, D: ConstraintDomain> ConstraintOperandView<'a, D> {
    pub(super) fn function_header_in_scope(
        &self,
        contents_scope: &Arc<ConstraintLexicalScope>,
    ) -> Result<Option<ConstraintFunctionHeaderView<'a, D>>, TypeConstraintError> {
        let TypeKind::Function {
            binder,
            effects,
            predicate,
            ..
        } = self.value
        else {
            return Ok(None);
        };
        if !self.scope.admits_children_scope(contents_scope, *binder) {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        Ok(Some(ConstraintFunctionHeaderView {
            source: self.clone(),
            contents_scope: Arc::clone(contents_scope),
            row: effects,
            predicate,
        }))
    }
}

impl<'a, D: ConstraintDomain> ConstraintOperandChildren<'a, D> {
    pub(in crate::types) fn function_header(
        &self,
    ) -> Result<Option<ConstraintFunctionHeaderView<'a, D>>, TypeConstraintError> {
        self.parent.function_header_in_scope(&self.scope)
    }
}

impl<'a, D: ConstraintDomain> ConstraintFunctionHeaderView<'a, D> {
    pub(in crate::types) fn row(&self) -> &'a EffectRow {
        self.row
    }

    pub(in crate::types) fn predicate(&self) -> &'a EffectPredicate {
        self.predicate
    }

    pub(in crate::types::constraints) fn lexical_scope(&self) -> &Arc<ConstraintLexicalScope> {
        &self.contents_scope
    }

    pub(in crate::types::constraints) fn derivation(&self) -> ConstraintEffectHeaderDerivation<D> {
        ConstraintEffectHeaderDerivation {
            source: ConstraintOperandDerivation::new(
                self.source.occurrence_origin(),
                Arc::clone(&self.source.scope),
            ),
            contents_scope: Arc::clone(&self.contents_scope),
        }
    }
}
