//! Canonical Copy/Move type possibility, using the original accepted nominal
//! definitions. This is not permission to clone a value-dependent carrier.

use super::{
    CheckedLocalUseError, CheckedLocalUseInstantiation, CheckedTypeCopyCapability,
    FinalSemanticAnalysis, policy_owner_is_copy,
};
use crate::{
    env::{
        EnumVariantPayload,
        nominal::AcceptedNominalSemantics,
        rust_metadata::{AcceptedRustStructShape, AcceptedRustTypeMetadataKind},
    },
    ownership::CheckedOwnershipLimits,
    types::{
        GenericScope, SemanticTypeDigest, TypeKind, TypeProjectionControl, TypeProjectionNodeKind,
        VariantPayloadShape,
    },
};
use arcweft_core::value::RuntimeOpaqueValueClass;
use arcweft_dialogue::CharacterDialoguePolicyTypeGraph;
use std::collections::BTreeSet;

#[derive(Default)]
struct TypeCopyTraversal {
    visiting: BTreeSet<SemanticTypeDigest>,
    nodes: u64,
    depth: u64,
    nominal_edges: u64,
}

impl TypeCopyTraversal {
    fn charge_node(&mut self, depth: u64) -> Result<(), CheckedLocalUseError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(CheckedLocalUseError::TypeOwnershipWorkLimit)?;
        let limits = CheckedOwnershipLimits::PRODUCTION;
        if self.nodes > limits.max_type_nodes || depth > limits.max_recursion_depth {
            return Err(CheckedLocalUseError::TypeOwnershipWorkLimit);
        }
        Ok(())
    }

    fn enter_type(&mut self) -> Result<(), CheckedLocalUseError> {
        self.charge_node(self.depth)?;
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(CheckedLocalUseError::TypeOwnershipWorkLimit)?;
        Ok(())
    }

    fn leave_type(&mut self) {
        self.depth -= 1;
    }

    fn enter_nominal(
        &mut self,
        identity: SemanticTypeDigest,
    ) -> Result<bool, CheckedLocalUseError> {
        self.nominal_edges = self
            .nominal_edges
            .checked_add(1)
            .ok_or(CheckedLocalUseError::TypeOwnershipWorkLimit)?;
        let limits = CheckedOwnershipLimits::PRODUCTION;
        if self.nominal_edges > limits.max_nominal_edges {
            return Err(CheckedLocalUseError::TypeOwnershipWorkLimit);
        }
        if self.visiting.contains(&identity) {
            return Ok(false);
        }
        if self.visiting.len() as u64 >= limits.max_active_nominal_depth {
            return Err(CheckedLocalUseError::TypeOwnershipWorkLimit);
        }
        self.visiting.insert(identity);
        Ok(true)
    }

    fn leave_nominal(&mut self, identity: SemanticTypeDigest) {
        self.visiting.remove(&identity);
    }
}

impl TypeProjectionControl for TypeCopyTraversal {
    type Error = CheckedLocalUseError;

    fn check(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit_node(&mut self, _: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        self.charge_node(self.depth.saturating_sub(1).saturating_add(depth))
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        self.charge_node(self.depth)
    }
}

pub(super) fn classify(
    ty: &TypeKind,
    analysis: &FinalSemanticAnalysis,
    instance: Option<CheckedLocalUseInstantiation<'_>>,
    scope: &GenericScope,
) -> Result<CheckedTypeCopyCapability, CheckedLocalUseError> {
    classify_at(
        ty,
        analysis,
        instance,
        scope,
        &mut TypeCopyTraversal::default(),
    )
}

fn classify_at(
    ty: &TypeKind,
    analysis: &FinalSemanticAnalysis,
    instance: Option<CheckedLocalUseInstantiation<'_>>,
    scope: &crate::types::GenericScope,
    traversal: &mut TypeCopyTraversal,
) -> Result<CheckedTypeCopyCapability, CheckedLocalUseError> {
    use CheckedTypeCopyCapability::{Unavailable, Unrestricted, ValueDependent};
    traversal.enter_type()?;
    let result = (|| match ty {
        TypeKind::Bool
        | TypeKind::I8
        | TypeKind::I16
        | TypeKind::I32
        | TypeKind::I64
        | TypeKind::I128
        | TypeKind::ISize
        | TypeKind::U8
        | TypeKind::U16
        | TypeKind::U32
        | TypeKind::U64
        | TypeKind::U128
        | TypeKind::USize
        | TypeKind::F32
        | TypeKind::F64
        | TypeKind::String
        | TypeKind::Char
        | TypeKind::Bytes
        | TypeKind::Duration
        | TypeKind::Progress
        | TypeKind::DenseNumeric(_)
        | TypeKind::Unit
        | TypeKind::Never
        | TypeKind::Ref(_)
        | TypeKind::CompileTimeScalar(_) => Ok(Unrestricted),
        TypeKind::Function { .. } | TypeKind::CharacterDialogue(_) => Ok(ValueDependent),
        TypeKind::AcceptedNominal(nominal) => {
            let plain = match analysis.accepted_nominal_semantics(nominal) {
                Some(AcceptedNominalSemantics::RustAdt) => {
                    return rust_nominal_capability(
                        ty, nominal, analysis, instance, scope, traversal,
                    );
                }
                Some(AcceptedNominalSemantics::Opaque(carrier)) => {
                    carrier.value_class() == RuntimeOpaqueValueClass::Plain
                }
                Some(AcceptedNominalSemantics::Record(record)) => record
                    .runtime_carrier()
                    .is_some_and(|carrier| carrier.value_class() == RuntimeOpaqueValueClass::Plain),
                _ => false,
            };
            if !plain {
                return Ok(Unavailable);
            }
            nominal
                .arguments()
                .iter()
                .try_fold(ValueDependent, |all, argument| {
                    classify_at(argument, analysis, instance, scope, traversal)
                        .map(|next| all.join(next))
                })
        }
        TypeKind::GenericParam(_) => {
            let Some(instance) = instance else {
                return Ok(Unavailable);
            };
            let closed = instance.instantiate_type(ty)?;
            if closed == *ty {
                return Err(CheckedLocalUseError::InvalidTopology);
            }
            classify_at(&closed, analysis, None, scope, traversal)
        }
        TypeKind::Tuple(items) | TypeKind::Choice(items) => {
            items.iter().try_fold(Unrestricted, |all, item| {
                classify_at(item, analysis, instance, scope, traversal).map(|next| all.join(next))
            })
        }
        TypeKind::Vec(item)
        | TypeKind::Array { item, .. }
        | TypeKind::Slice(item)
        | TypeKind::Seq(item)
        | TypeKind::Option(item)
        | TypeKind::Range(item) => classify_at(item, analysis, instance, scope, traversal),
        TypeKind::Result { ok, error }
        | TypeKind::Map {
            key: ok,
            value: error,
            ..
        } => Ok(classify_at(ok, analysis, instance, scope, traversal)?
            .join(classify_at(error, analysis, instance, scope, traversal)?)),
        TypeKind::ProjectNominal(_) => {
            let Ok(digest) = ty
                .semantic_identity_digest()
                .or_else(|_| ty.semantic_identity_digest_in_scope(scope))
            else {
                return Ok(Unavailable);
            };
            if !traversal.enter_nominal(digest)? {
                return Ok(Unrestricted);
            }
            let result = (|| {
                let Some(definition) = analysis.project_nominal_semantic(digest) else {
                    return Ok(Unavailable);
                };
                let mut capability = Unrestricted;
                if let Some(fields) = definition.fields() {
                    for field in fields {
                        capability = capability.join(classify_at(
                            field.ty(),
                            analysis,
                            instance,
                            definition.nominal().scope(),
                            traversal,
                        )?);
                    }
                    return Ok(capability);
                }
                let Some(cases) = definition.cases() else {
                    return Ok(Unavailable);
                };
                for case in cases {
                    match case.payload() {
                        VariantPayloadShape::Unit => {}
                        VariantPayloadShape::Tuple(fields) => {
                            for field in fields {
                                capability = capability.join(classify_at(
                                    field.ty(),
                                    analysis,
                                    instance,
                                    definition.nominal().scope(),
                                    traversal,
                                )?);
                            }
                        }
                        VariantPayloadShape::Record(fields) => {
                            for field in fields {
                                capability = capability.join(classify_at(
                                    field.ty(),
                                    analysis,
                                    instance,
                                    definition.nominal().scope(),
                                    traversal,
                                )?);
                            }
                        }
                    }
                }
                Ok(capability)
            })();
            traversal.leave_nominal(digest);
            result
        }
        TypeKind::Named(name) => {
            let Some(graph) = analysis.character_dialogue_policy_types() else {
                return Ok(Unavailable);
            };
            let Some(owner) = CharacterDialoguePolicyTypeGraph::owner_for_language_type(name)
            else {
                return Ok(Unavailable);
            };
            Ok(
                if policy_owner_is_copy(graph, owner, &mut BTreeSet::new()) {
                    Unrestricted
                } else {
                    Unavailable
                },
            )
        }
        _ => Ok(Unavailable),
    })();
    traversal.leave_type();
    result
}

fn rust_nominal_capability(
    ty: &TypeKind,
    nominal: &crate::types::AcceptedNominalType,
    analysis: &FinalSemanticAnalysis,
    instance: Option<CheckedLocalUseInstantiation<'_>>,
    scope: &GenericScope,
    traversal: &mut TypeCopyTraversal,
) -> Result<CheckedTypeCopyCapability, CheckedLocalUseError> {
    use crate::{env::rust_metadata::RustMetadataInstantiationError, types::TypeProjectionError};
    let identity = ty
        .semantic_identity_digest_in_scope_with_control(scope, traversal)
        .map_err(|error| match error {
            TypeProjectionError::Instantiation(error) => error.into(),
            TypeProjectionError::Control(error) => error,
        })?;
    if !traversal.enter_nominal(identity)? {
        // The same exact instantiated body is already being checked. Its
        // back-edge adds no leaves; all other members still participate below.
        return Ok(CheckedTypeCopyCapability::Unrestricted);
    }
    let result = (|| {
        let metadata = analysis
            .instantiate_accepted_rust_nominal(nominal, traversal)
            .map_err(|error| match error {
                RustMetadataInstantiationError::Control(error) => error,
                RustMetadataInstantiationError::Type(error) => error.into(),
                RustMetadataInstantiationError::UnknownNominal { .. }
                | RustMetadataInstantiationError::WrongArity { .. } => {
                    CheckedLocalUseError::InvalidTopology
                }
            })?
            .ok_or(CheckedLocalUseError::InvalidTopology)?;
        let mut capability = CheckedTypeCopyCapability::Unrestricted;
        let mut visit = |member: &TypeKind| {
            capability =
                capability.join(classify_at(member, analysis, instance, scope, traversal)?);
            Ok::<_, CheckedLocalUseError>(())
        };
        nominal.arguments().iter().try_for_each(&mut visit)?;
        match metadata.kind() {
            AcceptedRustTypeMetadataKind::Struct { shape } => match shape {
                AcceptedRustStructShape::Unit => {}
                AcceptedRustStructShape::Tuple(fields) => {
                    fields.iter().try_for_each(&mut visit)?;
                }
                AcceptedRustStructShape::Record(fields) => {
                    fields
                        .iter()
                        .map(crate::env::EnvironmentRecordField::ty)
                        .try_for_each(&mut visit)?;
                }
            },
            AcceptedRustTypeMetadataKind::Newtype { inner } => visit(inner)?,
            AcceptedRustTypeMetadataKind::Enum { variants } => {
                for variant in variants {
                    match variant.payload() {
                        EnumVariantPayload::Unit => {}
                        EnumVariantPayload::Tuple(fields) => {
                            fields.iter().try_for_each(&mut visit)?;
                        }
                        EnumVariantPayload::Record(fields) => {
                            fields
                                .iter()
                                .map(crate::env::EnvironmentRecordField::ty)
                                .try_for_each(&mut visit)?;
                        }
                    }
                }
            }
        }
        Ok(capability)
    })();
    traversal.leave_nominal(identity);
    result
}
