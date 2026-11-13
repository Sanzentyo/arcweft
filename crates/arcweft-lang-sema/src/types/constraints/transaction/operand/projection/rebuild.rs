//! Move-only projection results keep a proof at every corresponding value port.

use super::*;
use crate::types::constraints::normalization::{
    ConstraintClosedEffects, ConstraintProjectedEffectHeader,
};
use std::marker::PhantomData;

pub(in crate::types::constraints) enum ConstraintOperandEffects<D: ConstraintDomain> {
    Original,
    Closed(Arc<ConstraintClosedEffects<D>>),
}

pub(in crate::types::constraints) struct ConstraintOperandDerivation<D: ConstraintDomain, T> {
    pub(in crate::types::constraints::transaction::operand) origin: Arc<ConstraintOperandOrigin<D>>,
    pub(in crate::types::constraints::transaction::operand) scope: Arc<ConstraintLexicalScope>,
    kind: PhantomData<fn() -> T>,
}

impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    pub(in crate::types::constraints) fn derivation(&self) -> ConstraintOperandDerivation<D, T> {
        ConstraintOperandDerivation::new(Arc::clone(&self.origin), Arc::clone(&self.scope))
    }
}

impl<D: ConstraintDomain, T> ConstraintOperandDerivation<D, T> {
    pub(super) fn new(
        origin: Arc<ConstraintOperandOrigin<D>>,
        scope: Arc<ConstraintLexicalScope>,
    ) -> Self {
        Self {
            origin,
            scope,
            kind: PhantomData,
        }
    }

    #[cfg(test)]
    pub(in crate::types::constraints::transaction::operand) fn source_receipt(
        &self,
    ) -> Option<&ConstraintSourceReceipt<D>> {
        self.origin.source_receipt()
    }
}

/// Values are moved into their parent shape. Only persistence allocates the
/// final Arc value; proofs never clone a rebuilt subtree for each ancestor.
pub(in crate::types::constraints) struct ConstraintProjectedOperand<D: ConstraintDomain, T> {
    value: T,
    scope: Arc<ConstraintLexicalScope>,
    derivation: ConstraintOperandDerivation<D, T>,
    // Binding frames unwind inside-out. Flush once at the next shape or root.
    aliases: Vec<ConstraintAliasStep<D, T>>,
}

impl<D: ConstraintDomain, T> ConstraintProjectedOperand<D, T> {
    fn new(value: T, derivation: ConstraintOperandDerivation<D, T>) -> Self {
        Self {
            value,
            scope: Arc::clone(&derivation.scope),
            derivation,
            aliases: Vec::new(),
        }
    }

    fn parts(
        mut self,
        alias: impl FnOnce(
            Box<[ConstraintAliasStep<D, T>]>,
            ConstraintOperandDerivation<D, T>,
        ) -> ConstraintOperandOrigin<D>,
    ) -> (T, ConstraintOperandDerivation<D, T>) {
        let derivation = if self.aliases.is_empty() {
            self.derivation
        } else {
            self.aliases.reverse();
            ConstraintOperandDerivation::new(
                Arc::new(alias(self.aliases.into_boxed_slice(), self.derivation)),
                self.scope,
            )
        };
        (self.value, derivation)
    }
}

impl<D: ConstraintDomain> ConstraintOperandView<'_, D> {
    pub(in crate::types::constraints) fn projected_leaf(
        &self,
    ) -> ConstraintProjectedOperand<D, TypeKind> {
        ConstraintProjectedOperand::new(
            self.value.clone(),
            ConstraintOperandDerivation::new(self.occurrence_origin(), Arc::clone(&self.scope)),
        )
    }
}

impl<D: ConstraintDomain> ConstraintConstOperandView<'_, D> {
    pub(in crate::types::constraints) fn projected_leaf(
        &self,
    ) -> ConstraintProjectedOperand<D, ArrayLength> {
        let origin = match &self.origin {
            ConstraintConstOperandOrigin::Root(root) => {
                Arc::new(ConstraintOperandOrigin::ConstOccurrence((*root).clone()))
            }
            ConstraintConstOperandOrigin::Array(parent) => parent.projected_origin(
                Arc::new(ConstraintOperandProjection::ArrayLength {
                    parent: parent.address.clone(),
                }),
                Arc::clone(&self.value_origin),
            ),
        };
        ConstraintProjectedOperand::new(
            self.value.clone(),
            ConstraintOperandDerivation::new(origin, Arc::clone(&self.scope)),
        )
    }
}

pub(in crate::types::constraints) struct ConstraintOperandRebuild<'a, D: ConstraintDomain> {
    source: ConstraintOperandView<'a, D>,
    contents_scope: Arc<ConstraintLexicalScope>,
}

impl<'a, D: ConstraintDomain> ConstraintOperandChildren<'a, D> {
    pub(in crate::types::constraints) fn rebuild_seed(&self) -> ConstraintOperandRebuild<'a, D> {
        ConstraintOperandRebuild {
            source: self.parent.clone(),
            contents_scope: Arc::clone(&self.scope),
        }
    }
}

impl<D: ConstraintDomain> ConstraintOperandRebuild<'_, D> {
    pub(in crate::types::constraints) fn rebuild<A: TypeConstraintAccounting>(
        self,
        children: Vec<ConstraintProjectedOperand<D, TypeKind>>,
        length: Option<ConstraintProjectedOperand<D, ArrayLength>>,
        effects: &ConstraintOperandEffects<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ConstraintProjectedOperand<D, TypeKind>, TypeConstraintError> {
        let mut values = Vec::with_capacity(children.len());
        let mut proofs = Vec::with_capacity(children.len());
        for child in children {
            let (value, derivation) = child.type_parts();
            if derivation.scope != self.contents_scope {
                return Err(TypeConstraintInvariant::OriginalProducer.into());
            }
            values.push(value);
            proofs.push(derivation);
        }
        let (length_value, length_proof) = match length {
            Some(length) => {
                let (value, derivation) = length.const_parts();
                if derivation.scope != self.contents_scope {
                    return Err(TypeConstraintInvariant::OriginalProducer.into());
                }
                (Some(value), Some(derivation))
            }
            None => (None, None),
        };
        let input_header = self.source.function_header_in_scope(&self.contents_scope)?;
        let header = match effects {
            ConstraintOperandEffects::Closed(owner) => match input_header {
                Some(header) => owner.project_header(&header, context)?,
                None => ConstraintProjectedEffectHeader::Original,
            },
            ConstraintOperandEffects::Original => {
                if let Some(header) = input_header {
                    header.admit(context)?;
                }
                ConstraintProjectedEffectHeader::Original
            }
        };
        let mut fields = (length_value, header);
        let value = self.source.value().constraint_shape().rebuild_with(
            values,
            &mut fields,
            |(length, _), _| {
                Ok::<_, TypeConstraintError>(
                    length.take().expect("array length was projected first"),
                )
            },
            |(_, header), row| Ok(header.take_row(row)),
            |(_, header), predicate| Ok(header.take_predicate(predicate)),
        )?;
        let header_proof = fields.1.finish()?;
        let scope = Arc::clone(&self.source.scope);
        let origin = Arc::new(ConstraintOperandOrigin::Rebuilt {
            source: self.source.occurrence_origin(),
            contents_scope: self.contents_scope,
            children: proofs.into_boxed_slice(),
            length: length_proof,
            header: header_proof,
        });
        Ok(ConstraintProjectedOperand::new(
            value,
            ConstraintOperandDerivation::new(origin, scope),
        ))
    }
}

impl<D: ConstraintDomain> ConstraintProjectedOperand<D, TypeKind> {
    fn type_parts(self) -> (TypeKind, ConstraintOperandDerivation<D, TypeKind>) {
        self.parts(|steps, terminal| ConstraintOperandOrigin::TypeAlias { steps, terminal })
    }

    pub(in crate::types::constraints) fn into_operand(self) -> ConstraintOperand<D> {
        let (value, derivation) = self.type_parts();
        ConstraintOperand {
            value: Arc::new(value),
            scope: derivation.scope,
            origin: derivation.origin,
        }
    }

    pub(in crate::types::constraints) fn substitute<A: TypeConstraintAccounting>(
        mut self,
        occurrence: &ConstraintOperandView<'_, D>,
        binding: &ConstraintOperand<D>,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericTypeReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        if occurrence.lexical_scope().as_ref() != enclosing || self.scope.as_ref() != owner {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        if owner != enclosing {
            self.value = context.transport_binding_type(
                enclosing,
                owner,
                &self.value,
                parameter.clone().into(),
            )?;
        }
        self.scope = Arc::clone(occurrence.lexical_scope());
        self.aliases.push(ConstraintAliasStep {
            occurrence: occurrence.retained(),
            replacement: binding.clone(),
        });
        Ok(self)
    }
}

impl<D: ConstraintDomain> ConstraintProjectedOperand<D, ArrayLength> {
    fn const_parts(self) -> (ArrayLength, ConstraintOperandDerivation<D, ArrayLength>) {
        self.parts(|steps, terminal| ConstraintOperandOrigin::ConstAlias { steps, terminal })
    }

    pub(in crate::types::constraints) fn into_operand(self) -> ConstraintOperand<D, ArrayLength> {
        let (value, derivation) = self.const_parts();
        ConstraintOperand {
            value: Arc::new(value),
            scope: derivation.scope,
            origin: derivation.origin,
        }
    }

    pub(in crate::types::constraints) fn substitute<A: TypeConstraintAccounting>(
        mut self,
        occurrence: &ConstraintConstOperandView<'_, D>,
        binding: &ConstraintOperand<D, ArrayLength>,
        enclosing: &ConstraintLexicalScope,
        owner: &ConstraintLexicalScope,
        parameter: &GenericConstReference,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        if occurrence.lexical_scope().as_ref() != enclosing || self.scope.as_ref() != owner {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        if owner != enclosing {
            self.value = context.transport_binding_length(
                enclosing,
                owner,
                &self.value,
                parameter.clone().into(),
            )?;
        }
        self.scope = Arc::clone(occurrence.lexical_scope());
        self.aliases.push(ConstraintAliasStep {
            occurrence: occurrence.retained(),
            replacement: binding.clone(),
        });
        Ok(self)
    }
}
