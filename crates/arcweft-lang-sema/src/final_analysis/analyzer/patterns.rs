//! Pattern analysis, binding seeding, and variant ownership.

use super::{
    Analyzer, ArrayLength, BTreeMap, BTreeSet, BuiltinTypeConstructor, CheckedBuiltinVariantCase,
    CheckedPattern, CheckedPatternResolution, CheckedVariantOwner, EnumVariantPayload,
    EnvironmentEnumSchema, ExprId, FinalSemanticAnalysisError, FinalSemanticAnalysisInput, HirItem,
    HirItemKind, HirModule, HirPathRoot, HirPathSegment, HirPatternBinding, HirPatternField,
    HirPatternKind, HirPatternRecordPath, HirPatternSequenceRest, HirVariantPattern,
    HirVariantPatternHead, HirVariantPatternHeadValue, HirVariantPatternName,
    HirVariantPatternPayload, LocalId, PatternId, ProjectNominalBody, ProjectNominalDeclaration,
    ProjectNominalType, ProjectSymbolTable, ProjectTypeTarget, TypeCheckEnv, TypeId, TypeKind,
    calls::{checked_project_nominal, nominal_substitutions},
    expression_types::literal_type,
    statements::pattern_span,
};
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
                    HirPatternKind::Record { path, .. } => {
                        let declaration =
                            resolve_project_record_pattern(module, owner, path, &ty, self.symbols)?;
                        CheckedPatternResolution::Nominal(checked_project_nominal(
                            declaration,
                            &ty,
                        )?)
                    }
                    HirPatternKind::Variant(variant) => {
                        let (variant_owner, ordinal, _) = resolve_variant_pattern(
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
                        let HirVariantPatternName::Resolved(name) = variant.name() else {
                            return Err(FinalSemanticAnalysisError::RecoveredOwner);
                        };
                        CheckedPatternResolution::Variant(super::CheckedVariantResolution::new(
                            variant_owner,
                            ordinal,
                            name.clone(),
                        ))
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
                    _ => CheckedPatternResolution::Structural,
                };
                input.push_pattern(owner, CheckedPattern::new(ty, resolution));
            }
        }
        Ok(())
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
                self.facts.set_local_type(owner, value);
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
                self.facts.set_pattern_type(owner, value);
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

struct RecordPatternState<'a> {
    owner: PatternId,
    owner_ty: &'a TypeKind,
    declared: &'a [arcweft_lang_hir::symbol::nominal::ProjectNominalField],
    substitutions: &'a super::TypeParameterSubstitutions,
    seen: BTreeSet<super::HirName>,
    has_rest: bool,
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
        let (_, _, payload) = resolve_variant_pattern(
            PatternSeedContext {
                module: self.module,
                types: self.types,
                symbols: self.symbols,
                environment: self.environment,
            },
            pattern,
            variant,
            ty,
        )?;
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
        let TypeKind::Tuple(element_types) = ty else {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        };
        if elements.len() != element_types.len() {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        for (child, child_ty) in elements.iter().zip(element_types) {
            self.seed(*child, child_ty)?;
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
        let declaration =
            resolve_project_record_pattern(self.module, pattern, path, ty, self.symbols)?;
        let TypeKind::ProjectNominal(nominal) = ty else {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        };
        let substitutions = nominal_substitutions(declaration, nominal)
            .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })?;
        let ProjectNominalBody::Struct { fields: declared } = declaration.body() else {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        };
        let mut state = RecordPatternState {
            owner: pattern,
            owner_ty: ty,
            declared,
            substitutions: &substitutions,
            seen: BTreeSet::new(),
            has_rest: false,
        };
        for field in fields {
            self.seed_record_field(field, &mut state)?;
        }
        if !state.has_rest && state.seen.len() != declared.len() {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        Ok(())
    }

    fn seed_record_field(
        &mut self,
        field: &HirPatternField,
        state: &mut RecordPatternState<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        match field {
            HirPatternField::Explicit {
                name,
                pattern: child,
            } => {
                let field_ty = self.record_field_type(
                    state.owner,
                    name,
                    state.declared,
                    state.substitutions,
                    &mut state.seen,
                )?;
                self.seed(*child, &field_ty)
            }
            HirPatternField::Shorthand { name, local } => {
                let field_ty = self.record_field_type(
                    state.owner,
                    name,
                    state.declared,
                    state.substitutions,
                    &mut state.seen,
                )?;
                insert_local_type(*local, &field_ty, self.locals)
            }
            HirPatternField::Rest { binding } if !state.has_rest => {
                state.has_rest = true;
                if let Some(local) = binding {
                    insert_local_type(*local, state.owner_ty, self.locals)?;
                }
                Ok(())
            }
            HirPatternField::Rest { .. } | HirPatternField::Invalid { .. } => {
                Err(FinalSemanticAnalysisError::RecoveredOwner)
            }
        }
    }

    fn record_field_type(
        &self,
        pattern: PatternId,
        name: &super::HirName,
        declared: &[arcweft_lang_hir::symbol::nominal::ProjectNominalField],
        substitutions: &super::TypeParameterSubstitutions,
        seen: &mut BTreeSet<super::HirName>,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        if !seen.insert(name.clone()) {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern });
        }
        let declared = declared
            .iter()
            .find(|field| field.name().as_str() == name.as_str())
            .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner: pattern })?;
        let field_ty = self.types.get(&declared.ty()).ok_or(
            FinalSemanticAnalysisError::TypeResolutionFailed {
                owner: declared.ty(),
            },
        )?;
        Ok(substitutions.apply(field_ty))
    }

    fn seed_sequence(
        &mut self,
        pattern: PatternId,
        elements: &[PatternId],
        rest: &HirPatternSequenceRest,
        ty: &TypeKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let item = match ty {
            TypeKind::Vec(item) | TypeKind::Array { item, .. } | TypeKind::Slice(item) => {
                item.as_ref()
            }
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

fn resolve_project_record_pattern<'a>(
    module: &HirModule,
    owner: PatternId,
    path: &HirPatternRecordPath,
    ty: &TypeKind,
    symbols: &'a ProjectSymbolTable,
) -> Result<&'a ProjectNominalDeclaration, FinalSemanticAnalysisError> {
    let TypeKind::ProjectNominal(nominal) = ty else {
        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
    };
    let declaration = match path {
        HirPatternRecordPath::Absent => symbols.nominal(nominal.declaration()),
        HirPatternRecordPath::Resolved(path) => match symbols
            .resolve_hir_type_target(module.key().path(), path, pattern_span(module, owner)?)
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
    if declaration.id() != nominal.declaration()
        || !matches!(declaration.body(), ProjectNominalBody::Struct { .. })
    {
        return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
    }
    Ok(declaration)
}

pub(super) fn checked_builtin_closed_owner(
    schema: &EnvironmentEnumSchema,
    ty: &TypeKind,
    owner: ExprId,
) -> Result<CheckedVariantOwner, FinalSemanticAnalysisError> {
    let cases = checked_builtin_closed_cases(schema)
        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
    Ok(CheckedVariantOwner::BuiltinClosed {
        nominal: schema.owner().clone(),
        semantic_identity: ty.semantic_identity_digest(),
        cases,
    })
}

fn checked_builtin_closed_cases(
    schema: &EnvironmentEnumSchema,
) -> Option<Box<[CheckedBuiltinVariantCase]>> {
    schema
        .variants()
        .iter()
        .map(|variant| {
            let payload = match variant.payload() {
                EnumVariantPayload::Unit => None,
                EnumVariantPayload::Tuple(items) => match items.as_slice() {
                    [] => None,
                    [item] => Some(item.clone()),
                    items => Some(TypeKind::Tuple(items.to_vec())),
                },
                EnumVariantPayload::Record(_) => return None,
            };
            Some(CheckedBuiltinVariantCase::new(variant.name(), payload))
        })
        .collect::<Option<Vec<_>>>()
        .map(Vec::into_boxed_slice)
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
        super::CheckedVariantResolution::new(
            checked_builtin_closed_owner(schema, ty, owner)?,
            ordinal,
            name.clone(),
        ),
    )))
}

fn resolve_variant_pattern(
    context: PatternSeedContext<'_>,
    owner: PatternId,
    pattern: &HirVariantPattern,
    ty: &TypeKind,
) -> Result<(CheckedVariantOwner, u32, Option<TypeKind>), FinalSemanticAnalysisError> {
    let HirVariantPatternName::Resolved(name) = pattern.name() else {
        return Err(FinalSemanticAnalysisError::RecoveredOwner);
    };
    match ty {
        TypeKind::ProjectNominal(nominal) => {
            resolve_project_variant_pattern(context, owner, pattern, nominal, ty, name)
        }
        TypeKind::Option(item) => {
            validate_builtin_variant_head(pattern.head(), BuiltinTypeConstructor::Option, owner)?;
            let (ordinal, payload) = match name.as_str() {
                "Some" => (0, Some((**item).clone())),
                "None" => (1, None),
                _ => {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
                }
            };
            Ok((
                CheckedVariantOwner::Option {
                    item: (**item).clone(),
                },
                ordinal,
                payload,
            ))
        }
        TypeKind::Result { ok, error } => {
            validate_builtin_variant_head(pattern.head(), BuiltinTypeConstructor::Result, owner)?;
            let (ordinal, payload) = match name.as_str() {
                "Ok" => (0, Some((**ok).clone())),
                "Err" => (1, Some((**error).clone())),
                _ => {
                    return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
                }
            };
            Ok((
                CheckedVariantOwner::Result {
                    ok: (**ok).clone(),
                    error: (**error).clone(),
                },
                ordinal,
                payload,
            ))
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
) -> Result<(CheckedVariantOwner, u32, Option<TypeKind>), FinalSemanticAnalysisError> {
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
    let (ordinal, variant) = variants
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
    Ok((
        CheckedVariantOwner::Project(checked_project_nominal(declaration, ty)?),
        u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        payload,
    ))
}

fn resolve_closed_variant_pattern(
    environment: &TypeCheckEnv,
    owner: PatternId,
    pattern: &HirVariantPattern,
    ty: &TypeKind,
    name: &super::HirName,
) -> Result<(CheckedVariantOwner, u32, Option<TypeKind>), FinalSemanticAnalysisError> {
    let schema = environment
        .closed_enum(ty)
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    validate_closed_variant_head(pattern.head(), schema, owner)?;
    let (ordinal, selected) = schema
        .variants()
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name() == name.as_str())
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    let payload = match selected.payload() {
        EnumVariantPayload::Unit => None,
        EnumVariantPayload::Tuple(items) => match items.as_slice() {
            [] => None,
            [item] => Some(item.clone()),
            items => Some(TypeKind::Tuple(items.to_vec())),
        },
        EnumVariantPayload::Record(_) => {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
        }
    };
    let cases = checked_builtin_closed_cases(schema)
        .ok_or(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })?;
    Ok((
        CheckedVariantOwner::BuiltinClosed {
            nominal: schema.owner().clone(),
            semantic_identity: ty.semantic_identity_digest(),
            cases,
        },
        u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        payload,
    ))
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
