//! Pattern analysis, binding seeding, and variant ownership.

use super::{
    Analyzer, ArrayLength, BTreeMap, BTreeSet, BuiltinTypeConstructor, CheckedPatternResolution,
    EnumVariantPayload, EnvironmentEnumSchema, ExprId, FinalSemanticAnalysisError,
    FinalSemanticAnalysisInput, HirItem, HirItemKind, HirModule, HirPathRoot, HirPathSegment,
    HirPatternBinding, HirPatternField, HirPatternKind, HirPatternRecordPath,
    HirPatternSequenceRest, HirVariantPattern, HirVariantPatternHead, HirVariantPatternHeadValue,
    HirVariantPatternName, HirVariantPatternPayload, LocalId, PatternId, ProjectNominalBody,
    ProjectNominalType, ProjectSymbolTable, ProjectTypeTarget, TypeCheckEnv, TypeId, TypeKind,
    expression_types::literal_type, statements::pattern_span,
};
use crate::final_analysis::{
    PreparedRecordPattern, PreparedRecordPatternField, PreparedRecordPatternFieldCoordinate,
    PreparedRecordPatternOwner, PreparedRecordPatternRest, PreparedRecordPatternSource,
};
use crate::types::PreparedTypeCompatibility;
use arcweft_lang_hir::item::{
    HirAttachedContentPresence, HirCallableAttachedContentParameter, HirCapabilityMember,
    HirImplMember, HirTraitMember,
};

use super::entities::EntityReferenceResolutionError;

impl Analyzer<'_, '_, '_> {
    /// Loan the original graph only for this pattern operation. Its source
    /// context and work permission borrow disjoint analyzer fields.
    pub(super) fn with_pattern_seed_context<T>(
        &mut self,
        module: &HirModule,
        work: super::preparation_work::AnalyzerPreparationWork<'_>,
        operation: impl FnOnce(
            PatternSeedContext<'_>,
            &mut crate::effect_row::prepared::PreparedEffectGraph,
            &super::state::PreparedTypeObservations<'_>,
        ) -> Result<T, FinalSemanticAnalysisError>,
    ) -> Result<T, FinalSemanticAnalysisError> {
        let context = PatternSeedContext {
            module,
            symbols: self.symbols,
            environment: self.catalogs.world.environment().typecheck_env(),
            source: super::expression_types::SourceTypePreparation::new(
                &self.types,
                &self
                    .staged_callables
                    .as_ref()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
                    .builder,
                &self.topology,
                work,
            ),
        };
        let (declarations, observed) = self.facts.effect_declarations_and_type_observations()?;
        let (graph, _) = declarations.relation();
        operation(context, graph, &observed)
    }

    pub(super) fn analyze_patterns(
        &mut self,
        input: &mut FinalSemanticAnalysisInput,
        selected: &crate::final_analysis::match_edges::CheckedSelectedExpressionGraph,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let context = self.published_expression_context();
        use crate::types::TypeProjectionControl;
        let mut module_work = context.work();
        let mut modules = Vec::new();
        for module in self.modules.values() {
            module_work.visit_binding()?;
            modules.push(*module);
        }
        for module in modules {
            for (owner, pattern) in module.patterns() {
                if !selected
                    .contains_owner(arcweft_lang_hir::identity::SyntheticOwner::Pattern(owner))
                {
                    continue;
                }
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
                        let prepared = self
                            .prepare_record_pattern(&context, module, owner, path, fields, &ty)?;
                        input.push_prepared_pattern(
                            owner,
                            super::PreparedPatternFact::Record(prepared),
                        );
                        continue;
                    }
                    HirPatternKind::Variant(variant) => {
                        let resolved = self.with_pattern_seed_context(
                            module,
                            context.work(),
                            |seed_context, graph, _observed| {
                                resolve_variant_pattern(seed_context, graph, owner, variant, &ty)
                            },
                        )?;
                        let prepared = super::PreparedVariantPattern::try_new(
                            ty.clone(),
                            resolved.owner,
                            resolved.ordinal,
                        )
                        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                        input.push_prepared_pattern(
                            owner,
                            super::PreparedPatternFact::Variant(prepared),
                        );
                        continue;
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
                        let source = self.source_type_preparation(context.work())?;
                        let entity_type = source.retain(owner, &item.ty())?;
                        if source.contextual_type_mismatch(owner, &ty, &entity_type)? {
                            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                                owner,
                            });
                        }
                        item.into_pattern()
                    }
                    HirPatternKind::TypedBinding { ty: annotation, .. } => {
                        self.types.get(annotation).ok_or(
                            FinalSemanticAnalysisError::TypeResolutionFailed { owner: *annotation },
                        )?;
                        input.push_prepared_pattern(
                            owner,
                            super::PreparedPatternFact::TypedBinding(
                                crate::final_analysis::prepared::PreparedTypedBindingPattern::new(
                                    *annotation,
                                    ty,
                                ),
                            ),
                        );
                        continue;
                    }
                    _ => CheckedPatternResolution::Structural,
                };
                input.push_pattern(
                    owner,
                    crate::final_analysis::model::PreparedResolvedPattern::new(ty, resolution),
                );
            }
        }
        Ok(())
    }

    pub(super) fn prepare_record_pattern(
        &mut self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        module: &HirModule,
        owner: PatternId,
        path: &HirPatternRecordPath,
        authored: &[HirPatternField],
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<PreparedRecordPattern, FinalSemanticAnalysisError> {
        self.with_pattern_seed_context(module, context.work(), |seed_context, graph, observed| {
            let schema =
                resolve_record_pattern_schema(seed_context.clone(), graph, owner, path, ty)?;
            for _ in authored {
                seed_context.source.visit_binding()?;
            }
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
                            && observed.local(*binding).is_none()
                        {
                            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                                owner,
                            });
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
                let resolved = schema.field(owner, name, &seed_context, graph.read())?;
                let field_type = resolved.ty;
                let source_type = match source {
                    PreparedRecordPatternSource::Pattern(child) => observed.pattern(child),
                    PreparedRecordPatternSource::Binding(local) => observed.local(local),
                };
                if source_type.is_none() {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
                }
                fields.push(PreparedRecordPatternField::new(
                    source_ordinal,
                    resolved.identity,
                    field_type,
                    source,
                ));
            }
            if matches!(rest, PreparedRecordPatternRest::Absent)
                && seen.len() != schema.field_count()
            {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
            PreparedRecordPattern::try_new(
                seed_context
                    .source
                    .retain_prepared(owner, ty, graph.read())?,
                schema.into_prepared_owner(owner, &seed_context, graph.read())?,
                fields.into_boxed_slice(),
                rest,
            )
            .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)
        })
    }

    pub(super) fn seed_contextual_pattern_locals(
        &mut self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        module: &HirModule,
        pattern: PatternId,
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut locals = BTreeMap::new();
        let mut patterns = BTreeMap::new();
        self.with_pattern_seed_context(
            module,
            context.work(),
            |seed_context, graph, _observed| {
                seed_pattern_locals(seed_context, graph, pattern, ty, &mut locals, &mut patterns)
            },
        )?;
        let mut work = context.work();
        for (&owner, value) in &locals {
            if let Some(existing) = self.facts.locals().get(&owner)
                && existing != value
                && !existing
                    .completes_seeded_binding(value, &mut work)
                    .map_err(|error| {
                        FinalSemanticAnalysisError::source_type_projection(owner.into(), error)
                    })?
            {
                return Err(FinalSemanticAnalysisError::LocalTypeUnavailable { owner });
            }
        }
        for (&owner, value) in &patterns {
            if let Some(existing) = self.facts.patterns().get(&owner)
                && existing != value
                && !existing
                    .completes_seeded_binding(value, &mut work)
                    .map_err(|error| {
                        FinalSemanticAnalysisError::source_type_projection(owner.into(), error)
                    })?
            {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
        }
        for (owner, value) in locals {
            if self.facts.locals().get(&owner) != Some(&value) {
                self.facts
                    .set_local_type(owner, value)
                    .map_err(FinalSemanticAnalysisError::from)?;
            }
        }
        for (owner, value) in patterns {
            if self.facts.patterns().get(&owner) != Some(&value) {
                self.facts
                    .set_pattern_type(owner, value)
                    .map_err(FinalSemanticAnalysisError::from)?;
            }
        }
        Ok(())
    }
}

pub(super) fn seed_item_parameter_types(
    owner: arcweft_lang_hir::identity::ItemId,
    item: &HirItem,
    context: PatternSeedContext<'_>,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    catalog: &crate::callable::RegisteredCallableCatalog,
    locals: &mut BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
    patterns: &mut BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>>,
) -> Result<(), FinalSemanticAnalysisError> {
    fn seed_parameter(
        parameter: &arcweft_lang_hir::item::HirParameter,
        declared: &TypeKind,
        context: PatternSeedContext<'_>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        locals: &mut BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
        patterns: &mut BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let declared = context.source.retain(parameter.pattern(), declared)?;
        let binding = match parameter.kind() {
            arcweft_lang_hir::item::HirParameterKind::RestPositional => {
                TypeKind::Vec(Box::new(declared.clone()))
            }
            arcweft_lang_hir::item::HirParameterKind::Fixed
            | arcweft_lang_hir::item::HirParameterKind::ExtensionReceiver => declared.clone(),
        };
        seed_pattern_locals(
            context,
            graph,
            parameter.pattern(),
            &binding,
            locals,
            patterns,
        )
    }
    fn seed_attached_content(
        parameter: Option<HirCallableAttachedContentParameter>,
        context: &PatternSeedContext<'_>,
        locals: &mut BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let Some(parameter) = parameter else {
            return Ok(());
        };
        let content = context
            .environment
            .standard_dialogue_content_type()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let ty = match parameter.presence() {
            HirAttachedContentPresence::Optional => TypeKind::Option(Box::new(content)),
            HirAttachedContentPresence::Required | HirAttachedContentPresence::Defaulted { .. } => {
                content
            }
        };
        let ty = context.source.retain(parameter.binding(), &ty)?;
        if locals.insert(parameter.binding(), ty).is_some() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        Ok(())
    }
    use crate::callable::{
        CallableGroupIndex, CallableParameterCoordinate, CallableParameterIndex,
    };
    use arcweft_lang_hir::source_index::HirCallableSourceOwner;

    // The accepted signature owns omitted input effect parameters. Reading raw
    // annotations here would give the declaration body a different contract.
    let parameter_type = |source, group, parameter| {
        let symbol = context
            .symbols
            .callable_at_source(context.module.snapshot_id(), owner, source)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        let coordinate = CallableParameterCoordinate::new(
            CallableGroupIndex::try_from_usize(group)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?,
            CallableParameterIndex::try_from_usize(parameter)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?,
        );
        catalog
            .project_record(symbol.declaration())
            .and_then(|record| record.schema().parameter_type(coordinate))
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)
    };
    match item.kind() {
        HirItemKind::Action(action) => {
            for parameter in action.parameters() {
                // Action payload signatures own fixed declared types, not a
                // callable body's inferred input-effect parameters.
                let declared = context.source.declared_type(parameter.ty())?;
                let declared = context
                    .environment
                    .canonical_accepted_type(declared.clone());
                seed_parameter(
                    parameter,
                    &declared,
                    context.clone(),
                    graph,
                    locals,
                    patterns,
                )?;
            }
        }
        HirItemKind::Flow(flow) => {
            for (index, parameter) in flow.parameters().iter().enumerate() {
                seed_parameter(
                    parameter,
                    parameter_type(HirCallableSourceOwner::Item, 0, index)?,
                    context.clone(),
                    graph,
                    locals,
                    patterns,
                )?;
            }
        }
        HirItemKind::Function(function) => {
            for (group_index, group) in function.parameter_groups().iter().enumerate() {
                for (index, parameter) in group.parameters().iter().enumerate() {
                    seed_parameter(
                        parameter,
                        parameter_type(HirCallableSourceOwner::Item, group_index, index)?,
                        context.clone(),
                        graph,
                        locals,
                        patterns,
                    )?;
                }
            }
            seed_attached_content(function.attached_content(), &context, locals)?;
        }
        HirItemKind::Predicate(predicate) => {
            for (index, parameter) in predicate.parameters().iter().enumerate() {
                seed_parameter(
                    parameter,
                    parameter_type(HirCallableSourceOwner::Item, 0, index)?,
                    context.clone(),
                    graph,
                    locals,
                    patterns,
                )?;
            }
        }
        HirItemKind::Proof(proof) => {
            for (index, parameter) in proof.parameters().iter().enumerate() {
                seed_parameter(
                    parameter,
                    parameter_type(HirCallableSourceOwner::Item, 0, index)?,
                    context.clone(),
                    graph,
                    locals,
                    patterns,
                )?;
            }
        }
        HirItemKind::View(view) => {
            for (index, parameter) in view.parameters().iter().enumerate() {
                seed_parameter(
                    parameter,
                    parameter_type(HirCallableSourceOwner::ViewItem, 0, index)?,
                    context.clone(),
                    graph,
                    locals,
                    patterns,
                )?;
            }
        }
        HirItemKind::ExternCapability(capability) => {
            for (member, value) in capability.members().iter().enumerate() {
                let HirCapabilityMember::Function(function) = value else {
                    continue;
                };
                let source = HirCallableSourceOwner::ExternCapabilityFunction {
                    member: u16::try_from(member)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?,
                };
                for (group_index, group) in function.parameter_groups().iter().enumerate() {
                    for (index, parameter) in group.parameters().iter().enumerate() {
                        seed_parameter(
                            parameter,
                            parameter_type(source, group_index, index)?,
                            context.clone(),
                            graph,
                            locals,
                            patterns,
                        )?;
                    }
                }
                seed_attached_content(function.attached_content(), &context, locals)?;
            }
        }
        HirItemKind::Impl(implementation) => {
            for (member, value) in implementation.members().iter().enumerate() {
                let HirImplMember::Function(function) = value else {
                    continue;
                };
                let source = HirCallableSourceOwner::ImplFunction {
                    member: u16::try_from(member)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?,
                };
                for (group_index, group) in function.parameter_groups().iter().enumerate() {
                    for (index, parameter) in group.parameters().iter().enumerate() {
                        let declared = parameter_type(source, group_index, index)?;
                        match parameter {
                            arcweft_lang_hir::item::HirMethodParameter::Receiver(receiver) => {
                                seed_pattern_locals(
                                    context.clone(),
                                    graph,
                                    receiver.pattern(),
                                    &context.source.retain(receiver.pattern(), declared)?,
                                    locals,
                                    patterns,
                                )?;
                            }
                            arcweft_lang_hir::item::HirMethodParameter::Typed(parameter) => {
                                seed_parameter(
                                    parameter,
                                    declared,
                                    context.clone(),
                                    graph,
                                    locals,
                                    patterns,
                                )?;
                            }
                        }
                    }
                }
                seed_attached_content(function.attached_content(), &context, locals)?;
            }
        }
        HirItemKind::Trait(trait_item) => {
            for (member, value) in trait_item.members().iter().enumerate() {
                let HirTraitMember::Function(function) = value else {
                    continue;
                };
                let source = HirCallableSourceOwner::TraitFunction {
                    member: u16::try_from(member)
                        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?,
                };
                for (group_index, group) in function.parameter_groups().iter().enumerate() {
                    for (index, parameter) in group.parameters().iter().enumerate() {
                        if let arcweft_lang_hir::item::HirMethodParameter::Typed(parameter) =
                            parameter
                        {
                            seed_parameter(
                                parameter,
                                parameter_type(source, group_index, index)?,
                                context.clone(),
                                graph,
                                locals,
                                patterns,
                            )?;
                        }
                    }
                }
                seed_attached_content(function.attached_content(), &context, locals)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Clone)]
pub(super) struct PatternSeedContext<'a> {
    pub(super) module: &'a HirModule,
    pub(super) symbols: &'a ProjectSymbolTable,
    pub(super) environment: &'a TypeCheckEnv,
    pub(super) source: super::expression_types::SourceTypePreparation<'a>,
}

struct ResolvedRecordPatternField {
    identity: PreparedRecordPatternFieldCoordinate,
    ty: TypeKind<crate::types::PreparedEffects>,
}

enum ResolvedRecordPatternSchema<'a> {
    Project {
        nominal: crate::final_analysis::prepared::PreparedProjectNominal,
        fields: Vec<(
            &'a arcweft_lang_hir::symbol::nominal::ProjectNominalField,
            TypeKind<crate::types::PreparedEffects>,
        )>,
    },
    Environment {
        identity: crate::env::nominal::AcceptedEnvironmentRecord,
        semantics: &'a crate::env::nominal::AcceptedEnvironmentRecordSemantics,
    },
    VariantPayload {
        ty: &'a TypeKind<crate::types::PreparedEffects>,
        fields: &'a [crate::types::VariantPayloadRecordTypeField<crate::types::PreparedEffects>],
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

    fn into_prepared_owner(
        self,
        owner: PatternId,
        context: &PatternSeedContext<'_>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<PreparedRecordPatternOwner, FinalSemanticAnalysisError> {
        Ok(match self {
            Self::Project { nominal, .. } => PreparedRecordPatternOwner::Project(nominal),
            Self::Environment { identity, .. } => {
                PreparedRecordPatternOwner::Environment { record: identity }
            }
            Self::VariantPayload { ty, .. } => {
                let retained = context.source.retain_prepared(owner, ty, graph)?;
                let TypeKind::VariantPayload(payload) = retained else {
                    return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
                };
                PreparedRecordPatternOwner::variant_payload(*payload)
                    .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?
            }
        })
    }

    fn field(
        &self,
        owner: PatternId,
        name: &super::HirName,
        context: &PatternSeedContext<'_>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<ResolvedRecordPatternField, FinalSemanticAnalysisError> {
        match self {
            Self::Project { fields, .. } => {
                let (declaration_ordinal, (_, ty)) = fields
                    .iter()
                    .enumerate()
                    .find(|(_, (field, _))| field.name().as_str() == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
                let declaration_ordinal = u32::try_from(declaration_ordinal)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                Ok(ResolvedRecordPatternField {
                    identity: PreparedRecordPatternFieldCoordinate::new(declaration_ordinal),
                    ty: context.source.retain_prepared(owner, ty, graph)?,
                })
            }
            Self::Environment { semantics, .. } => {
                let field = semantics
                    .field(name.as_str())
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
                Ok(ResolvedRecordPatternField {
                    identity: PreparedRecordPatternFieldCoordinate::new(field.ordinal()),
                    ty: context.source.retain(owner, field.ty())?,
                })
            }
            Self::VariantPayload { fields, .. } => {
                let field = fields
                    .iter()
                    .find(|field| field.diagnostic_name() == name.as_str())
                    .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
                Ok(ResolvedRecordPatternField {
                    identity: PreparedRecordPatternFieldCoordinate::new(field.ordinal()),
                    ty: context.source.retain_prepared(owner, field.ty(), graph)?,
                })
            }
        }
    }
}

pub(super) fn seed_pattern_locals(
    context: PatternSeedContext<'_>,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    pattern: PatternId,
    ty: &TypeKind<crate::types::PreparedEffects>,
    locals: &mut BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
    patterns: &mut BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>>,
) -> Result<(), FinalSemanticAnalysisError> {
    PatternSeeder {
        context,
        graph,
        locals,
        patterns,
    }
    .seed(pattern, ty)
}

struct PatternSeeder<'context, 'facts> {
    context: PatternSeedContext<'context>,
    graph: &'facts mut crate::effect_row::prepared::PreparedEffectGraph,
    locals: &'facts mut BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
    patterns: &'facts mut BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>>,
}

impl PatternSeeder<'_, '_> {
    fn seed(
        &mut self,
        pattern: PatternId,
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let record = self
            .context
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
            HirPatternKind::Literal(literal) => self.validate_literal(pattern, literal, ty)?,
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
        &self,
        pattern: PatternId,
        literal: &arcweft_lang_hir::leaf::HirLiteral,
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let (literal_ty, _) = literal_type(literal, Some(ty))
            .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })?;
        if self
            .context
            .source
            .contextual_type_mismatch(pattern, ty, &literal_ty)?
        {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        Ok(())
    }

    fn seed_variant(
        &mut self,
        pattern: PatternId,
        variant: &HirVariantPattern,
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let payload =
            resolve_variant_pattern(self.context.clone(), self.graph, pattern, variant, ty)?
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
        ty: &TypeKind<crate::types::PreparedEffects>,
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
                    self.seed(*child, field)?;
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
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let schema =
            resolve_record_pattern_schema(self.context.clone(), self.graph, pattern, path, ty)?;
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
                    let field = schema.field(pattern, name, &self.context, self.graph.read())?;
                    self.seed(*child, &field.ty)?;
                }
                HirPatternField::Shorthand { name, local } => {
                    if !seen.insert(name.clone()) {
                        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable {
                            owner: pattern,
                        });
                    }
                    let field = schema.field(pattern, name, &self.context, self.graph.read())?;
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
        ty: &TypeKind<crate::types::PreparedEffects>,
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
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let annotation_ty = self.context.source.annotation(pattern, annotation)?;
        let source = &self.context.source;
        let forward = source.prepared_compatibility(pattern, &annotation_ty, ty)?;
        let relation = match ty {
            TypeKind::Choice(_) => source.prepared_compatibility(pattern, ty, &annotation_ty)?,
            _ => forward,
        };
        if relation == PreparedTypeCompatibility::Rejected {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        let initializer = if forward == PreparedTypeCompatibility::Rejected {
            &annotation_ty
        } else {
            ty
        };
        let completed = source
            .complete_binding_effects(pattern, &annotation_ty, initializer)?
            .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })?;
        seed_pattern_binding(binding, &completed, self.locals)
    }
}

fn insert_pattern_type(
    owner: PatternId,
    ty: &TypeKind<crate::types::PreparedEffects>,
    patterns: &mut BTreeMap<PatternId, TypeKind<crate::types::PreparedEffects>>,
) -> Result<(), FinalSemanticAnalysisError> {
    if patterns.get(&owner).is_some_and(|existing| existing != ty) {
        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
    }
    patterns.entry(owner).or_insert_with(|| ty.clone());
    Ok(())
}

fn seed_pattern_binding(
    binding: &HirPatternBinding,
    ty: &TypeKind<crate::types::PreparedEffects>,
    locals: &mut BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
) -> Result<(), FinalSemanticAnalysisError> {
    match binding {
        HirPatternBinding::Bound { local, .. } => insert_local_type(*local, ty, locals),
        HirPatternBinding::Recovered { .. } => Err(FinalSemanticAnalysisError::RecoveredOwner),
    }
}

fn insert_local_type(
    owner: LocalId,
    ty: &TypeKind<crate::types::PreparedEffects>,
    locals: &mut BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
) -> Result<(), FinalSemanticAnalysisError> {
    if locals.get(&owner).is_some_and(|existing| existing != ty) {
        return Err(FinalSemanticAnalysisError::LocalTypeUnavailable { owner });
    }
    locals.entry(owner).or_insert_with(|| ty.clone());
    Ok(())
}

fn resolve_record_pattern_schema<'a>(
    context: PatternSeedContext<'a>,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    owner: PatternId,
    path: &HirPatternRecordPath,
    ty: &'a TypeKind<crate::types::PreparedEffects>,
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
            context.source.with_nominal_type_arguments(
                owner,
                declaration,
                nominal,
                graph,
                |nominal, projection| {
                    for _ in fields {
                        projection.visit_binding()?;
                    }
                    let mut prepared_fields = Vec::with_capacity(fields.len());
                    for field in fields {
                        prepared_fields.push((field, projection.annotation(field.ty())?));
                    }
                    Ok(ResolvedRecordPatternSchema::Project {
                        nominal: crate::final_analysis::prepared::PreparedProjectNominal::new(
                            nominal,
                        ),
                        fields: prepared_fields,
                    })
                },
            )
        }
        TypeKind::VariantPayload(payload) => {
            if !matches!(path, HirPatternRecordPath::Absent) {
                return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
            }
            let fields = payload
                .shape()
                .record_fields()
                .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            Ok(ResolvedRecordPatternSchema::VariantPayload { ty, fields })
        }
        _ => {
            // The registered catalog is keyed by canonical type. Retain the
            // pattern's original type for the final catalog and field check.
            let observed = context.source.observe_canonical(owner, ty, graph.read())?;
            let semantic_type = observed
                .semantic_identity_digest()
                .map_err(|_| FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            let accepted = context
                .environment
                .nominal_catalog()
                .environment_record_for_semantic_type(semantic_type)
                .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
            let semantics = accepted
                .environment_record()
                .filter(|record| {
                    record.ty() == &observed && record.semantic_type() == semantic_type
                })
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
            context.source.visit_binding()?;
            let identity = accepted
                .owned_environment_record()
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            Ok(ResolvedRecordPatternSchema::Environment {
                identity,
                semantics,
            })
        }
    }
}

pub(super) fn resolve_closed_variant_path(
    source: &super::expression_types::SourceTypePreparation<'_>,
    environment: &TypeCheckEnv,
    path: &arcweft_lang_hir::leaf::HirPath,
    owner: ExprId,
) -> Result<
    Option<(
        TypeKind<crate::types::PreparedEffects>,
        super::PreparedVariantOwnerSeed<crate::types::PreparedEffects>,
        u32,
    )>,
    FinalSemanticAnalysisError,
> {
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
    let (ty, seed) = source.environment_variant(owner, schema, ty)?;
    Ok(Some((ty, seed, ordinal)))
}

struct ResolvedVariantPattern {
    owner: super::PreparedVariantOwnerSeed<crate::types::PreparedEffects>,
    ordinal: u32,
    payload: Option<TypeKind<crate::types::PreparedEffects>>,
}

impl ResolvedVariantPattern {
    const fn payload(&self) -> Option<&TypeKind<crate::types::PreparedEffects>> {
        self.payload.as_ref()
    }
}

fn resolve_variant_pattern(
    context: PatternSeedContext<'_>,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    owner: PatternId,
    pattern: &HirVariantPattern,
    ty: &TypeKind<crate::types::PreparedEffects>,
) -> Result<ResolvedVariantPattern, FinalSemanticAnalysisError> {
    let HirVariantPatternName::Resolved(name) = pattern.name() else {
        return Err(FinalSemanticAnalysisError::RecoveredOwner);
    };
    match ty {
        TypeKind::ProjectNominal(nominal) => {
            resolve_project_variant_pattern(context, graph, owner, pattern, nominal, name)
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
            let checked_owner = super::PreparedVariantOwnerSeed::option((**item).clone());
            let payload = checked_owner
                .case_payload_type(ordinal)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            Ok(ResolvedVariantPattern {
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
            let checked_owner =
                super::PreparedVariantOwnerSeed::result((**ok).clone(), (**error).clone());
            let payload = checked_owner
                .case_payload_type(ordinal)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
            Ok(ResolvedVariantPattern {
                owner: checked_owner,
                ordinal,
                payload,
            })
        }
        closed_enum_ty => resolve_closed_variant_pattern(
            context,
            graph.read(),
            owner,
            pattern,
            closed_enum_ty,
            name,
        ),
    }
}

fn resolve_project_variant_pattern(
    context: PatternSeedContext<'_>,
    graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    owner: PatternId,
    pattern: &HirVariantPattern,
    nominal: &ProjectNominalType<crate::types::PreparedEffects>,
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
    let (selected_ordinal, _) = variants
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name().as_str() == name.as_str())
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    let selected_ordinal = u32::try_from(selected_ordinal)
        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    let owner = context
        .source
        .project_variant_owner(owner, declaration, nominal, graph)?;
    let payload = owner
        .case_payload_type(selected_ordinal)
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
    Ok(ResolvedVariantPattern {
        owner,
        ordinal: selected_ordinal,
        payload,
    })
}

fn resolve_closed_variant_pattern(
    context: PatternSeedContext<'_>,
    graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    owner: PatternId,
    pattern: &HirVariantPattern,
    ty: &TypeKind<crate::types::PreparedEffects>,
    name: &super::HirName,
) -> Result<ResolvedVariantPattern, FinalSemanticAnalysisError> {
    let observed = context.source.observe_canonical(owner, ty, graph)?;
    let schema = context
        .environment
        .closed_enum(&observed)
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    validate_closed_variant_head(pattern.head(), schema, owner)?;
    let (ordinal, _selected) = schema
        .variants()
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name() == name.as_str())
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    let original = context.source.retain_prepared(owner, ty, graph)?;
    let checked_owner = context
        .source
        .environment_variant_owner(owner, schema, original)?;
    let ordinal =
        u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
    let payload = checked_owner
        .case_payload_type(ordinal)
        .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
    Ok(ResolvedVariantPattern {
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
    locals: &BTreeMap<LocalId, TypeKind<crate::types::PreparedEffects>>,
) -> Option<TypeKind<crate::types::PreparedEffects>> {
    module.locals().find_map(|(owner, local)| {
        (local.pattern() == Some(pattern))
            .then(|| locals.get(&owner).cloned())
            .flatten()
    })
}
