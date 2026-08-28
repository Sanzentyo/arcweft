//! Pattern analysis, binding seeding, and variant ownership.

use super::{
    Analyzer, ArrayLength, BTreeMap, BTreeSet, BuiltinTypeConstructor, CheckedPattern,
    CheckedPatternResolution, CheckedTypedBinding, CheckedVariantOwner, EnumVariantPayload,
    EnvironmentEnumSchema, ExprId, FinalSemanticAnalysisError, FinalSemanticAnalysisInput, HirItem,
    HirItemKind, HirModule, HirPathRoot, HirPathSegment, HirPatternBinding, HirPatternField,
    HirPatternKind, HirPatternRecordPath, HirPatternSequenceRest, HirVariantPattern,
    HirVariantPatternHead, HirVariantPatternHeadValue, HirVariantPatternName,
    HirVariantPatternPayload, LocalId, PatternId, ProjectNominalBody, ProjectNominalType,
    ProjectSymbolTable, ProjectTypeTarget, TypeCheckEnv, TypeId, TypeKind,
    calls::{checked_project_nominal, nominal_substitutions},
    expression_types::literal_type,
    statements::pattern_span,
};
use crate::final_analysis::{
    PreparedRecordPattern, PreparedRecordPatternField, PreparedRecordPatternFieldIdentity,
    PreparedRecordPatternOwner, PreparedRecordPatternRest, PreparedRecordPatternSource,
};
use crate::record_field::CheckedRecordFieldSemanticId;
use arcweft_lang_hir::item::{HirCapabilityMember, HirImplMember};

use super::entities::EntityReferenceResolutionError;

impl Analyzer<'_, '_, '_> {
    pub(super) fn analyze_patterns(
        &self,
        input: &mut FinalSemanticAnalysisInput,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for module in self.modules.values() {
            for (owner, pattern) in module.patterns() {
                if pattern.is_poisoned() {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
                let ty = self
                    .facts
                    .patterns()
                    .get(&owner)
                    .cloned()
                    .or_else(|| pattern_local_type(module, owner, self.facts.locals()))
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
                let resolution = match pattern.kind() {
                    HirPatternKind::Literal(literal) => {
                        CheckedPatternResolution::Literal(literal.clone())
                    }
                    HirPatternKind::Record { path, fields } => {
                        let schema = resolve_record_pattern_schema(
                            PatternSeedContext {
                                module,
                                types: &self.types,
                                symbols: self.symbols,
                                environment: self.catalogs.world.environment().typecheck_env(),
                            },
                            owner,
                            path,
                            &ty,
                        )?;
                        let prepared = self.prepare_record_pattern(owner, &schema, fields, &ty)?;
                        input.push_prepared_pattern(
                            owner,
                            super::PreparedPatternFact::Record(prepared),
                        );
                        continue;
                    }
                    HirPatternKind::Variant(variant) => {
                        let resolved = resolve_variant_pattern(
                            PatternSeedContext {
                                module,
                                types: &self.types,
                                symbols: self.symbols,
                                environment: self.catalogs.world.environment().typecheck_env(),
                            },
                            owner,
                            variant,
                            &ty,
                        )?;
                        match resolved {
                            ResolvedVariantPattern::Complete {
                                owner: variant_owner,
                                ordinal,
                                ..
                            } => CheckedPatternResolution::Variant(
                                super::CheckedVariantResolution::try_new(variant_owner, ordinal)
                                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
                            ),
                            ResolvedVariantPattern::Project {
                                owner: seed,
                                ordinal,
                                ..
                            } => {
                                let prepared = super::PreparedProjectVariantPattern::try_new(
                                    ty.clone(),
                                    seed,
                                    ordinal,
                                )
                                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                                input.push_prepared_pattern(
                                    owner,
                                    super::PreparedPatternFact::ProjectVariant(prepared),
                                );
                                continue;
                            }
                        }
                    }
                    HirPatternKind::Error(_) => {
                        return Err(FinalSemanticAnalysisError::RecoveredOwner);
                    }
                    HirPatternKind::EntityReference(reference) => {
                        let reference = reference
                            .as_resolved()
                            .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                        let source = pattern_span(module, owner)?;
                        let item = self
                            .resolve_checked_entity_reference(module, reference, source)
                            .map_err(|error| match error {
                                EntityReferenceResolutionError::Lookup => {
                                    FinalSemanticAnalysisError::PatternTypeUnavailable { owner }
                                }
                                EntityReferenceResolutionError::WrongFamily => {
                                    FinalSemanticAnalysisError::WrongPayloadFamily
                                }
                            })?;
                        if !ty.accepts(&item.ty()) {
                            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                                owner,
                            });
                        }
                        CheckedPatternResolution::Entity(item)
                    }
                    HirPatternKind::TypedBinding { ty: annotation, .. } => {
                        let annotation = self.types.get(annotation).cloned().ok_or(
                            FinalSemanticAnalysisError::TypeResolutionFailed { owner: *annotation },
                        )?;
                        CheckedPatternResolution::TypedBinding(
                            CheckedTypedBinding::try_new(annotation, &ty)
                                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?,
                        )
                    }
                    _ => CheckedPatternResolution::Structural,
                };
                input.push_pattern(owner, CheckedPattern::new(ty, resolution));
            }
        }
        Ok(())
    }

    fn prepare_record_pattern(
        &self,
        owner: PatternId,
        schema: &ResolvedRecordPatternSchema<'_>,
        authored: &[HirPatternField],
        ty: &TypeKind,
    ) -> Result<PreparedRecordPattern, FinalSemanticAnalysisError> {
        let mut fields = Vec::with_capacity(authored.len());
        let mut seen = BTreeSet::new();
        let mut rest = PreparedRecordPatternRest::Absent;
        for (source_ordinal, field) in authored.iter().enumerate() {
            let source_ordinal = u32::try_from(source_ordinal)
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let (name, source) = match field {
                HirPatternField::Explicit {
                    name,
                    pattern: child,
                } => (name, PreparedRecordPatternSource::Pattern(*child)),
                HirPatternField::Shorthand { name, local } => {
                    (name, PreparedRecordPatternSource::Binding(*local))
                }
                HirPatternField::Rest { binding }
                    if matches!(rest, PreparedRecordPatternRest::Absent) =>
                {
                    rest = if let Some(binding) = binding {
                        PreparedRecordPatternRest::Binding(*binding)
                    } else {
                        PreparedRecordPatternRest::Ignore
                    };
                    if let Some(binding) = binding
                        && self.facts.locals().get(binding) != Some(ty)
                    {
                        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
                    }
                    continue;
                }
                HirPatternField::Rest { .. } | HirPatternField::Invalid { .. } => {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
            };
            if !seen.insert(name.clone()) {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
            let resolved = schema.field(owner, name, &self.types)?;
            let field_type = resolved.ty;
            let observed = match source {
                PreparedRecordPatternSource::Pattern(child) => self.facts.patterns().get(&child),
                PreparedRecordPatternSource::Binding(local) => self.facts.locals().get(&local),
            };
            if observed != Some(&field_type) {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
            fields.push(match resolved.identity {
                PreparedRecordPatternFieldIdentity::Project {
                    declaration_ordinal,
                } => PreparedRecordPatternField::project(
                    source_ordinal,
                    declaration_ordinal,
                    field_type,
                    source,
                ),
                PreparedRecordPatternFieldIdentity::Environment {
                    declaration_ordinal,
                    semantic_id,
                } => PreparedRecordPatternField::environment(
                    source_ordinal,
                    declaration_ordinal,
                    semantic_id,
                    field_type,
                    source,
                ),
                PreparedRecordPatternFieldIdentity::VariantPayload {
                    declaration_ordinal,
                    semantic_id,
                } => PreparedRecordPatternField::variant_payload(
                    source_ordinal,
                    declaration_ordinal,
                    semantic_id,
                    field_type,
                    source,
                ),
            });
        }
        if matches!(rest, PreparedRecordPatternRest::Absent) && seen.len() != schema.field_count() {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
        }
        PreparedRecordPattern::try_new(
            ty.clone(),
            schema.prepared_owner()?,
            fields.into_boxed_slice(),
            rest,
        )
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)
    }

    pub(super) fn seed_contextual_pattern_locals(
        &mut self,
        module: &HirModule,
        pattern: PatternId,
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut locals = BTreeMap::new();
        let mut patterns = BTreeMap::new();
        seed_pattern_locals(
            PatternSeedContext {
                module,
                types: &self.types,
                symbols: self.symbols,
                environment: self.catalogs.world.environment().typecheck_env(),
            },
            pattern,
            ty,
            &mut locals,
            &mut patterns,
        )?;
        for (owner, value) in locals {
            if self
                .facts
                .locals()
                .get(&owner)
                .is_some_and(|existing| existing != &value)
            {
                return Err(FinalSemanticAnalysisError::LocalTypeUnavailable { owner });
            }
            if !self.facts.locals().contains_key(&owner) {
                self.facts
                    .set_local_type(owner, value)
                    .map_err(FinalSemanticAnalysisError::from)?;
            }
        }
        for (owner, value) in patterns {
            if self
                .facts
                .patterns()
                .get(&owner)
                .is_some_and(|existing| existing != &value)
            {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
            if !self.facts.patterns().contains_key(&owner) {
                self.facts
                    .set_pattern_type(owner, value)
                    .map_err(FinalSemanticAnalysisError::from)?;
            }
        }
        Ok(())
    }
}

pub(super) fn seed_item_parameter_types(
    item: &HirItem,
    types: &BTreeMap<TypeId, TypeKind>,
    locals: &mut BTreeMap<LocalId, TypeKind>,
) -> Result<(), FinalSemanticAnalysisError> {
    fn seed_parameter(
        parameter: &arcweft_lang_hir::item::HirParameter,
        types: &BTreeMap<TypeId, TypeKind>,
        locals: &mut BTreeMap<LocalId, TypeKind>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let ty = types.get(&parameter.ty()).cloned().ok_or(
            FinalSemanticAnalysisError::TypeResolutionFailed {
                owner: parameter.ty(),
            },
        )?;
        for local in parameter.locals() {
            locals.insert(*local, ty.clone());
        }
        Ok(())
    }
    match item.kind() {
        HirItemKind::Flow(flow) => {
            for parameter in flow.parameters() {
                seed_parameter(parameter, types, locals)?;
            }
        }
        HirItemKind::Function(function) => {
            for group in function.parameter_groups() {
                for parameter in group.parameters() {
                    seed_parameter(parameter, types, locals)?;
                }
            }
        }
        HirItemKind::Predicate(predicate) => {
            for parameter in predicate.parameters() {
                seed_parameter(parameter, types, locals)?;
            }
        }
        HirItemKind::Proof(proof) => {
            for parameter in proof.parameters() {
                seed_parameter(parameter, types, locals)?;
            }
        }
        HirItemKind::View(view) => {
            for parameter in view.parameters() {
                seed_parameter(parameter, types, locals)?;
            }
        }
        HirItemKind::ExternCapability(capability) => {
            for member in capability.members() {
                let HirCapabilityMember::Function(function) = member else {
                    continue;
                };
                for group in function.parameter_groups() {
                    for parameter in group.parameters() {
                        seed_parameter(parameter, types, locals)?;
                    }
                }
            }
        }
        HirItemKind::Impl(implementation) => {
            let self_ty = types.get(&implementation.target()).cloned().ok_or(
                FinalSemanticAnalysisError::TypeResolutionFailed {
                    owner: implementation.target(),
                },
            )?;
            for member in implementation.members() {
                let HirImplMember::Function(function) = member else {
                    continue;
                };
                for group in function.parameter_groups() {
                    for parameter in group.parameters() {
                        match parameter {
                            arcweft_lang_hir::item::HirMethodParameter::Receiver(receiver) => {
                                for local in receiver.locals() {
                                    locals.insert(*local, self_ty.clone());
                                }
                            }
                            arcweft_lang_hir::item::HirMethodParameter::Typed(parameter) => {
                                seed_parameter(parameter, types, locals)?;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct PatternSeedContext<'a> {
    pub(super) module: &'a HirModule,
    pub(super) types: &'a BTreeMap<TypeId, TypeKind>,
    pub(super) symbols: &'a ProjectSymbolTable,
    pub(super) environment: &'a TypeCheckEnv,
}

struct ResolvedRecordPatternField {
    identity: PreparedRecordPatternFieldIdentity,
    ty: TypeKind,
}

enum ResolvedRecordPatternSchema<'a> {
    Project {
        nominal: super::CheckedProjectNominal,
        substitutions: super::TypeParameterSubstitutions,
        fields: &'a [arcweft_lang_hir::symbol::nominal::ProjectNominalField],
    },
    Environment {
        identity: crate::env::nominal::AcceptedEnvironmentRecordIdentity,
        semantics: &'a crate::env::nominal::AcceptedEnvironmentRecordSemantics,
    },
    VariantPayload {
        payload: &'a crate::types::VariantPayloadType,
        fields: &'a [crate::types::VariantPayloadRecordField],
    },
}

impl ResolvedRecordPatternSchema<'_> {
    fn field_count(&self) -> usize {
        match self {
            Self::Project { fields, .. } => fields.len(),
            Self::Environment { semantics, .. } => semantics.fields().len(),
            Self::VariantPayload { fields, .. } => fields.len(),
        }
    }

    fn prepared_owner(&self) -> Result<PreparedRecordPatternOwner, FinalSemanticAnalysisError> {
        Ok(match self {
            Self::Project { nominal, .. } => PreparedRecordPatternOwner::Project(nominal.clone()),
            Self::Environment { identity, .. } => PreparedRecordPatternOwner::Environment {
                record: identity.clone(),
            },
            Self::VariantPayload { payload, .. } => {
                PreparedRecordPatternOwner::variant_payload((*payload).clone())
                    .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?
            }
        })
    }

    fn field(
        &self,
        owner: PatternId,
        name: &super::HirName,
        types: &BTreeMap<TypeId, TypeKind>,
    ) -> Result<ResolvedRecordPatternField, FinalSemanticAnalysisError> {
        match self {
            Self::Project {
                substitutions,
                fields,
                ..
            } => {
                let (declaration_ordinal, field) = fields
                    .iter()
                    .enumerate()
                    .find(|(_, field)| field.name().as_str() == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
                let declaration_ordinal = u32::try_from(declaration_ordinal)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                let declared = types.get(&field.ty()).ok_or(
                    FinalSemanticAnalysisError::TypeResolutionFailed { owner: field.ty() },
                )?;
                Ok(ResolvedRecordPatternField {
                    identity: PreparedRecordPatternFieldIdentity::Project {
                        declaration_ordinal,
                    },
                    ty: substitutions.apply(declared),
                })
            }
            Self::Environment { semantics, .. } => {
                let field = semantics
                    .field(name.as_str())
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
                Ok(ResolvedRecordPatternField {
                    identity: PreparedRecordPatternFieldIdentity::Environment {
                        declaration_ordinal: field.ordinal(),
                        semantic_id: CheckedRecordFieldSemanticId::Environment(field.semantic_id()),
                    },
                    ty: field.ty().clone(),
                })
            }
            Self::VariantPayload { fields, .. } => {
                let field = fields
                    .iter()
                    .find(|field| field.diagnostic_name() == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
                Ok(ResolvedRecordPatternField {
                    identity: PreparedRecordPatternFieldIdentity::VariantPayload {
                        declaration_ordinal: field.ordinal(),
                        semantic_id: CheckedRecordFieldSemanticId::VariantPayload(
                            field.semantic_id(),
                        ),
                    },
                    ty: field.ty().clone(),
                })
            }
        }
    }
}

pub(super) fn seed_pattern_locals(
    context: PatternSeedContext<'_>,
    pattern: PatternId,
    ty: &TypeKind,
    locals: &mut BTreeMap<LocalId, TypeKind>,
    patterns: &mut BTreeMap<PatternId, TypeKind>,
) -> Result<(), FinalSemanticAnalysisError> {
    PatternSeeder {
        module: context.module,
        types: context.types,
        symbols: context.symbols,
        environment: context.environment,
        locals,
        patterns,
    }
    .seed(pattern, ty)
}

struct PatternSeeder<'context, 'facts> {
    module: &'context HirModule,
    types: &'context BTreeMap<TypeId, TypeKind>,
    symbols: &'context ProjectSymbolTable,
    environment: &'context TypeCheckEnv,
    locals: &'facts mut BTreeMap<LocalId, TypeKind>,
    patterns: &'facts mut BTreeMap<PatternId, TypeKind>,
}

impl PatternSeeder<'_, '_> {
    fn seed(
        &mut self,
        pattern: PatternId,
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let record = self
            .module
            .resolve_pattern(pattern)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if record.is_poisoned() {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
        let kind = record.kind().clone();
        insert_pattern_type(pattern, ty, self.patterns)?;
        match &kind {
            HirPatternKind::Binding(binding) | HirPatternKind::MutableBinding(binding) => {
                seed_pattern_binding(binding, ty, self.locals)?;
            }
            HirPatternKind::Literal(literal) => Self::validate_literal(pattern, literal, ty)?,
            HirPatternKind::EntityReference(_) | HirPatternKind::Discard => {}
            HirPatternKind::Variant(variant) => self.seed_variant(pattern, variant, ty)?,
            HirPatternKind::Tuple { elements } => self.seed_tuple(pattern, elements, ty)?,
            HirPatternKind::Record { path, fields } => {
                self.seed_record(pattern, path, fields, ty)?;
            }
            HirPatternKind::BracketSequence { elements, rest } => {
                self.seed_sequence(pattern, elements, rest, ty)?;
            }
            HirPatternKind::WholeBinding {
                binding,
                pattern: child,
            } => {
                seed_pattern_binding(binding, ty, self.locals)?;
                self.seed(*child, ty)?;
            }
            HirPatternKind::Or { alternatives } => {
                for child in alternatives {
                    self.seed(*child, ty)?;
                }
            }
            HirPatternKind::TypedBinding {
                binding,
                ty: annotation,
            } => self.seed_typed_binding(pattern, binding, *annotation, ty)?,
            HirPatternKind::Error(_) => {
                return Err(FinalSemanticAnalysisError::RecoveredOwner);
            }
        }
        Ok(())
    }

    fn validate_literal(
        pattern: PatternId,
        literal: &arcweft_lang_hir::leaf::HirLiteral,
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let (literal_ty, _) = literal_type(literal, Some(ty))
            .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })?;
        if !ty.accepts(&literal_ty) {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        Ok(())
    }

    fn seed_variant(
        &mut self,
        pattern: PatternId,
        variant: &HirVariantPattern,
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let payload = resolve_variant_pattern(
            PatternSeedContext {
                module: self.module,
                types: self.types,
                symbols: self.symbols,
                environment: self.environment,
            },
            pattern,
            variant,
            ty,
        )?
        .payload()
        .cloned();
        match (variant.payload(), payload) {
            (HirVariantPatternPayload::Absent, None) => Ok(()),
            (HirVariantPatternPayload::Pattern(child), Some(payload_ty)) => {
                self.seed(*child, &payload_ty)
            }
            (HirVariantPatternPayload::Recovered { .. }, _) => {
                Err(FinalSemanticAnalysisError::RecoveredOwner)
            }
            (HirVariantPatternPayload::Absent, Some(_))
            | (HirVariantPatternPayload::Pattern(_), None) => {
                Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })
            }
        }
    }

    fn seed_tuple(
        &mut self,
        pattern: PatternId,
        elements: &[PatternId],
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        match ty {
            TypeKind::Tuple(element_types) => {
                if elements.len() != element_types.len() {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                        owner: pattern,
                    });
                }
                for (child, child_ty) in elements.iter().zip(element_types) {
                    self.seed(*child, child_ty)?;
                }
            }
            TypeKind::VariantPayload(payload) => {
                let fields = payload
                    .shape()
                    .tuple_fields()
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })?;
                if elements.len() != fields.len() {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                        owner: pattern,
                    });
                }
                for (child, field) in elements.iter().zip(fields) {
                    self.seed(*child, field.ty())?;
                }
            }
            _ => {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
            }
        }
        Ok(())
    }

    fn seed_record(
        &mut self,
        pattern: PatternId,
        path: &HirPatternRecordPath,
        fields: &[HirPatternField],
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let schema = resolve_record_pattern_schema(
            PatternSeedContext {
                module: self.module,
                types: self.types,
                symbols: self.symbols,
                environment: self.environment,
            },
            pattern,
            path,
            ty,
        )?;
        let mut seen = BTreeSet::new();
        let mut has_rest = false;
        for field in fields {
            match field {
                HirPatternField::Explicit {
                    name,
                    pattern: child,
                } => {
                    if !seen.insert(name.clone()) {
                        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                            owner: pattern,
                        });
                    }
                    let field = schema.field(pattern, name, self.types)?;
                    self.seed(*child, &field.ty)?;
                }
                HirPatternField::Shorthand { name, local } => {
                    if !seen.insert(name.clone()) {
                        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                            owner: pattern,
                        });
                    }
                    let field = schema.field(pattern, name, self.types)?;
                    insert_local_type(*local, &field.ty, self.locals)?;
                }
                HirPatternField::Rest { binding } if !has_rest => {
                    has_rest = true;
                    if let Some(local) = binding {
                        insert_local_type(*local, ty, self.locals)?;
                    }
                }
                HirPatternField::Rest { .. } | HirPatternField::Invalid { .. } => {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
            }
        }
        if !has_rest && seen.len() != schema.field_count() {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        Ok(())
    }

    fn seed_sequence(
        &mut self,
        pattern: PatternId,
        elements: &[PatternId],
        rest: &HirPatternSequenceRest,
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let item = match ty {
            TypeKind::Vec(item)
            | TypeKind::Array { item, .. }
            | TypeKind::Slice(item)
            | TypeKind::Seq(item) => item.as_ref(),
            _ => {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
            }
        };
        if let TypeKind::Array {
            len: ArrayLength::Const(len),
            ..
        } = ty
            && match rest {
                HirPatternSequenceRest::Absent => elements.len() != *len,
                HirPatternSequenceRest::Unbound | HirPatternSequenceRest::Bound(_) => {
                    elements.len() > *len
                }
                HirPatternSequenceRest::Recovered(_) => true,
            }
        {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        for child in elements {
            self.seed(*child, item)?;
        }
        match rest {
            HirPatternSequenceRest::Absent | HirPatternSequenceRest::Unbound => Ok(()),
            HirPatternSequenceRest::Bound(local) => {
                insert_local_type(*local, &TypeKind::Vec(Box::new(item.clone())), self.locals)
            }
            HirPatternSequenceRest::Recovered(_) => Err(FinalSemanticAnalysisError::RecoveredOwner),
        }
    }

    fn seed_typed_binding(
        &mut self,
        pattern: PatternId,
        binding: &HirPatternBinding,
        annotation: TypeId,
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let annotation_ty = self
            .types
            .get(&annotation)
            .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner: annotation })?;
        let compatible = match ty {
            TypeKind::Choice(_) => ty.accepts(annotation_ty),
            _ => annotation_ty.accepts(ty),
        };
        if !compatible {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        seed_pattern_binding(binding, annotation_ty, self.locals)
    }
}

fn insert_pattern_type(
    owner: PatternId,
    ty: &TypeKind,
    patterns: &mut BTreeMap<PatternId, TypeKind>,
) -> Result<(), FinalSemanticAnalysisError> {
    if patterns.get(&owner).is_some_and(|existing| existing != ty) {
        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
    }
    patterns.entry(owner).or_insert_with(|| ty.clone());
    Ok(())
}

fn seed_pattern_binding(
    binding: &HirPatternBinding,
    ty: &TypeKind,
    locals: &mut BTreeMap<LocalId, TypeKind>,
) -> Result<(), FinalSemanticAnalysisError> {
    match binding {
        HirPatternBinding::Bound { local, .. } => insert_local_type(*local, ty, locals),
        HirPatternBinding::Recovered { .. } => Err(FinalSemanticAnalysisError::RecoveredOwner),
    }
}

fn insert_local_type(
    owner: LocalId,
    ty: &TypeKind,
    locals: &mut BTreeMap<LocalId, TypeKind>,
) -> Result<(), FinalSemanticAnalysisError> {
    if locals.get(&owner).is_some_and(|existing| existing != ty) {
        return Err(FinalSemanticAnalysisError::LocalTypeUnavailable { owner });
    }
    locals.entry(owner).or_insert_with(|| ty.clone());
    Ok(())
}

fn resolve_record_pattern_schema<'a>(
    context: PatternSeedContext<'a>,
    owner: PatternId,
    path: &HirPatternRecordPath,
    ty: &'a TypeKind,
) -> Result<ResolvedRecordPatternSchema<'a>, FinalSemanticAnalysisError> {
    match ty {
        TypeKind::ProjectNominal(nominal) => {
            let declaration = match path {
                HirPatternRecordPath::Absent => context.symbols.nominal(nominal.declaration()),
                HirPatternRecordPath::Resolved(path) => match context
                    .symbols
                    .resolve_hir_type_target(
                        context.module.key().path(),
                        path,
                        pattern_span(context.module, owner)?,
                    )
                    .map_err(|_| FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?
                {
                    ProjectTypeTarget::Nominal(declaration) => Some(declaration),
                    ProjectTypeTarget::External(_) => None,
                },
                HirPatternRecordPath::Recovered(_) => {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
            }
            .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            if declaration.id() != nominal.declaration() {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
            let ProjectNominalBody::Struct { fields } = declaration.body() else {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            };
            let substitutions = nominal_substitutions(declaration, nominal)
                .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            Ok(ResolvedRecordPatternSchema::Project {
                nominal: checked_project_nominal(declaration, ty)?,
                substitutions,
                fields,
            })
        }
        TypeKind::Named(name) => {
            let accepted = context
                .environment
                .accepted_environment_record(name)
                .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            let semantics = accepted
                .environment_record()
                .filter(|record| record.ty() == ty)
                .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            match path {
                HirPatternRecordPath::Absent => {}
                HirPatternRecordPath::Resolved(path)
                    if crate::nominal::hir_path_matches_type_path(
                        path,
                        accepted.id().canonical_path(),
                    ) => {}
                HirPatternRecordPath::Recovered(_) => {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
                HirPatternRecordPath::Resolved(_) => {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
                }
            }
            let identity = accepted
                .environment_record_identity()
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            Ok(ResolvedRecordPatternSchema::Environment {
                identity,
                semantics,
            })
        }
        TypeKind::VariantPayload(payload) => {
            if !matches!(path, HirPatternRecordPath::Absent) {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
            let fields = payload
                .shape()
                .record_fields()
                .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            Ok(ResolvedRecordPatternSchema::VariantPayload { payload, fields })
        }
        _ => Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner }),
    }
}

pub(super) fn checked_builtin_closed_owner(
    schema: &EnvironmentEnumSchema,
    ty: &TypeKind,
    owner: ExprId,
) -> Result<CheckedVariantOwner, FinalSemanticAnalysisError> {
    CheckedVariantOwner::try_environment(schema, ty)
        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })
}

pub(super) fn resolve_closed_variant_path(
    environment: &TypeCheckEnv,
    path: &arcweft_lang_hir::leaf::HirPath,
    owner: ExprId,
) -> Result<Option<(TypeKind, super::CheckedVariantResolution)>, FinalSemanticAnalysisError> {
    if path.root() != HirPathRoot::ImplicitCrate {
        return Ok(None);
    }
    let [
        HirPathSegment::Identifier(nominal),
        HirPathSegment::Identifier(name),
    ] = path.segments()
    else {
        return Ok(None);
    };
    let Some((ty, schema)) = environment.closed_enum_by_owner(nominal.as_str()) else {
        return Ok(None);
    };
    let Some((ordinal, selected)) = schema
        .variants()
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name() == name.as_str())
    else {
        return Ok(None);
    };
    if !matches!(selected.payload(), EnumVariantPayload::Unit) {
        return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
    }
    let ordinal =
        u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    Ok(Some((
        ty.clone(),
        super::CheckedVariantResolution::try_new(
            checked_builtin_closed_owner(schema, ty, owner)?,
            ordinal,
        )
        .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
    )))
}

enum ResolvedVariantPattern {
    Complete {
        owner: CheckedVariantOwner,
        ordinal: u32,
        payload: Option<TypeKind>,
    },
    Project {
        owner: super::PreparedProjectVariantOwnerSeed,
        ordinal: u32,
        payload: Option<TypeKind>,
    },
}

impl ResolvedVariantPattern {
    const fn payload(&self) -> Option<&TypeKind> {
        match self {
            Self::Complete { payload, .. } | Self::Project { payload, .. } => payload.as_ref(),
        }
    }
}

fn resolve_variant_pattern(
    context: PatternSeedContext<'_>,
    owner: PatternId,
    pattern: &HirVariantPattern,
    ty: &TypeKind,
) -> Result<ResolvedVariantPattern, FinalSemanticAnalysisError> {
    let HirVariantPatternName::Resolved(name) = pattern.name() else {
        return Err(FinalSemanticAnalysisError::RecoveredOwner);
    };
    match ty {
        TypeKind::ProjectNominal(nominal) => {
            resolve_project_variant_pattern(context, owner, pattern, nominal, ty, name)
        }
        TypeKind::Option(item) => {
            validate_builtin_variant_head(pattern.head(), BuiltinTypeConstructor::Option, owner)?;
            let ordinal = match name.as_str() {
                "Some" => 0,
                "None" => 1,
                _ => {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
                }
            };
            let checked_owner = CheckedVariantOwner::option((**item).clone());
            let payload = checked_owner
                .case_payload_type(ordinal)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            Ok(ResolvedVariantPattern::Complete {
                owner: checked_owner,
                ordinal,
                payload,
            })
        }
        TypeKind::Result { ok, error } => {
            validate_builtin_variant_head(pattern.head(), BuiltinTypeConstructor::Result, owner)?;
            let ordinal = match name.as_str() {
                "Ok" => 0,
                "Err" => 1,
                _ => {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
                }
            };
            let checked_owner = CheckedVariantOwner::result((**ok).clone(), (**error).clone());
            let payload = checked_owner
                .case_payload_type(ordinal)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            Ok(ResolvedVariantPattern::Complete {
                owner: checked_owner,
                ordinal,
                payload,
            })
        }
        closed_enum_ty => resolve_closed_variant_pattern(
            context.environment,
            owner,
            pattern,
            closed_enum_ty,
            name,
        ),
    }
}

fn resolve_project_variant_pattern(
    context: PatternSeedContext<'_>,
    owner: PatternId,
    pattern: &HirVariantPattern,
    nominal: &ProjectNominalType,
    ty: &TypeKind,
    name: &super::HirName,
) -> Result<ResolvedVariantPattern, FinalSemanticAnalysisError> {
    let declaration = match pattern.head() {
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(_)) => {
            context.symbols.nominal(nominal.declaration())
        }
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path)) => {
            match context
                .symbols
                .resolve_hir_type_target(
                    context.module.key().path(),
                    path,
                    pattern_span(context.module, owner)?,
                )
                .map_err(|_| FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?
            {
                ProjectTypeTarget::Nominal(declaration) => Some(declaration),
                ProjectTypeTarget::External(_) => None,
            }
        }
        HirVariantPatternHeadValue::Recovered(_) => {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        }
    }
    .filter(|declaration| declaration.id() == nominal.declaration())
    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    let ProjectNominalBody::Enum { variants } = declaration.body() else {
        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
    };
    let (selected_ordinal, variant) = variants
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name().as_str() == name.as_str())
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    let substitutions = nominal_substitutions(declaration, nominal)
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    let payload = variant
        .payload()
        .map(|payload| {
            context
                .types
                .get(&payload)
                .map(|payload| substitutions.apply(payload))
                .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner: payload })
        })
        .transpose()?;
    let selected_ordinal = u32::try_from(selected_ordinal)
        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    let checked_nominal = checked_project_nominal(declaration, ty)?;
    let mut cases = Vec::with_capacity(variants.len());
    for (ordinal, case) in variants.iter().enumerate() {
        let ordinal =
            u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let case_payload = case
            .payload()
            .map(|payload| {
                context
                    .types
                    .get(&payload)
                    .map(|payload| substitutions.apply(payload))
                    .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner: payload })
            })
            .transpose()?;
        cases.push(super::PreparedVariantCaseSeed::new(
            ordinal,
            case_payload,
            Some(case.name().as_str().to_owned()),
        ));
    }
    let owner = super::PreparedProjectVariantOwnerSeed::try_new(checked_nominal, cases)
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
    let payload = match payload {
        None => None,
        Some(payload) => {
            let shape = crate::types::VariantPayloadShape::try_tuple(
                crate::types::VariantPayloadOwnerFamily::Project,
                owner.nominal().identity(),
                selected_ordinal,
                [payload],
            )
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let case = crate::types::AcceptedVariantCaseSemanticId::issue(
                crate::types::VariantPayloadOwnerFamily::Project,
                owner.nominal().identity(),
                selected_ordinal,
                &shape,
            );
            Some(TypeKind::VariantPayload(Box::new(
                crate::types::VariantPayloadType::try_new(
                    crate::types::VariantPayloadOwnerFamily::Project,
                    owner.nominal().identity(),
                    selected_ordinal,
                    case,
                    shape,
                )
                .map_err(|_| FinalSemanticAnalysisError::InvalidNominalOwner)?,
            )))
        }
    };
    Ok(ResolvedVariantPattern::Project {
        owner,
        ordinal: selected_ordinal,
        payload,
    })
}

fn resolve_closed_variant_pattern(
    environment: &TypeCheckEnv,
    owner: PatternId,
    pattern: &HirVariantPattern,
    ty: &TypeKind,
    name: &super::HirName,
) -> Result<ResolvedVariantPattern, FinalSemanticAnalysisError> {
    let schema = environment
        .closed_enum(ty)
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    validate_closed_variant_head(pattern.head(), schema, owner)?;
    let (ordinal, _selected) = schema
        .variants()
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name() == name.as_str())
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    let checked_owner = CheckedVariantOwner::try_environment(schema, ty)
        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
    let ordinal =
        u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    let payload = checked_owner
        .case_payload_type(ordinal)
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
    Ok(ResolvedVariantPattern::Complete {
        owner: checked_owner,
        ordinal,
        payload,
    })
}

fn validate_closed_variant_head(
    head: &HirVariantPatternHeadValue,
    schema: &EnvironmentEnumSchema,
    owner: PatternId,
) -> Result<(), FinalSemanticAnalysisError> {
    match head {
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(_)) => Ok(()),
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path))
            if path.root() == HirPathRoot::ImplicitCrate
                && matches!(
                    path.segments(),
                    [HirPathSegment::Identifier(name)]
                        if name.as_str() == schema.owner().as_str()
                ) =>
        {
            Ok(())
        }
        HirVariantPatternHeadValue::Recovered(_) => Err(FinalSemanticAnalysisError::RecoveredOwner),
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(_)) => {
            Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })
        }
    }
}

fn validate_builtin_variant_head(
    head: &HirVariantPatternHeadValue,
    expected: BuiltinTypeConstructor,
    owner: PatternId,
) -> Result<(), FinalSemanticAnalysisError> {
    match head {
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(_)) => Ok(()),
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path))
            if BuiltinTypeConstructor::from_hir_path(path) == Some(expected) =>
        {
            Ok(())
        }
        HirVariantPatternHeadValue::Recovered(_) => Err(FinalSemanticAnalysisError::RecoveredOwner),
        HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(_)) => {
            Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })
        }
    }
}

fn pattern_local_type(
    module: &HirModule,
    pattern: PatternId,
    locals: &BTreeMap<LocalId, TypeKind>,
) -> Option<TypeKind> {
    module.locals().find_map(|(owner, local)| {
        (local.pattern() == Some(pattern))
            .then(|| locals.get(&owner).cloned())
            .flatten()
    })
}
