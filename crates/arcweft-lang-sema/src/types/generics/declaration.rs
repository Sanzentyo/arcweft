//! Declaration parameters and the lexical slots owned by their callable value.

use super::{
    GenericBinder, GenericConstReference, GenericEffectReference, GenericParameterKind,
    GenericScope, GenericScopeError, GenericTypeReference,
};

/// One ordinal mapping is used for scheme projection, parameter ABI projection
/// and the inverse mapping of a checked specialization back to body parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenericDeclarationBinder {
    template_scope: GenericScope,
    scope: GenericScope,
    types: Box<[GenericTypeReference]>,
    consts: Box<[GenericConstReference]>,
    effects: Box<[GenericEffectReference]>,
}

impl GenericDeclarationBinder {
    pub(crate) fn new(
        template_scope: GenericScope,
        types: Box<[GenericTypeReference]>,
        consts: Box<[GenericConstReference]>,
        effects: Box<[GenericEffectReference]>,
    ) -> Result<Self, GenericScopeError> {
        let overflow = |kind, count| GenericScopeError::BinderArityOverflow { kind, count };
        let binder = GenericBinder::new(
            u16::try_from(types.len())
                .map_err(|_| overflow(GenericParameterKind::Type, types.len()))?,
            u16::try_from(consts.len())
                .map_err(|_| overflow(GenericParameterKind::Const, consts.len()))?,
            u32::try_from(effects.len())
                .map_err(|_| overflow(GenericParameterKind::Effect, effects.len()))?,
        );
        Ok(Self {
            template_scope,
            scope: GenericScope::default().with_binder(binder),
            types,
            consts,
            effects,
        })
    }

    pub(crate) const fn template_scope(&self) -> &GenericScope {
        &self.template_scope
    }
    pub(crate) const fn scope(&self) -> &GenericScope {
        &self.scope
    }
    pub(crate) fn type_parameters(&self) -> &[GenericTypeReference] {
        &self.types
    }
    pub(crate) fn const_parameters(&self) -> &[GenericConstReference] {
        &self.consts
    }
    pub(crate) fn effect_parameters(&self) -> &[GenericEffectReference] {
        &self.effects
    }
}
