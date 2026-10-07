//! Canonical executable tables shared by candidate validation and published plans.
//! Only the aggregate builder owns construction; callers borrow immutable rows.

use super::{
    FlowRuntimeId, LineTaskGroup, RuntimeBuiltinVariantIdentity, RuntimeCallableExecutable,
    RuntimeCallableSpecializationDefinition, RuntimeCallableStateTable,
    RuntimeCheckedRecordTypeError, RuntimeCheckedType, RuntimeCheckedVariantCase,
    RuntimeControlEffectContractTable, RuntimeDialogueContentPlanTable,
    RuntimeDialogueContentTemplateManifestTable, RuntimeEntrySpec, RuntimeFlow,
    RuntimeFlowExecutable, RuntimeFlowSchema, RuntimeFlowTargetError, RuntimeFormatAttempt,
    RuntimeFormatAttemptTable, RuntimeFunctionSiteTable, RuntimeLocalDeclarationTable,
    RuntimeNominalRecordDomainTable, RuntimeNominalTypeId, RuntimeOpaqueTypeOwner,
    RuntimePlanRecordField, RuntimePlanTypeClass, RuntimePlanTypeId, RuntimePlanTypeProjection,
    RuntimePlanTypeResolutionError, RuntimePlanTypeTable, RuntimeProjectCallSiteTable,
    RuntimePureHelper, RuntimePureProgramBinding, RuntimeTraitMethod, RuntimeVariantDomainTable,
    StreamPlan, TypeLayoutHash,
};
use crate::value::RuntimeRecordFieldId;
use std::collections::{BTreeMap, BTreeSet};

struct CheckedTypeTraversal<'a> {
    memo: &'a mut BTreeMap<RuntimePlanTypeId, Option<RuntimeCheckedType>>,
    visiting: &'a mut BTreeSet<RuntimePlanTypeId>,
}

/// Immutable executable table authority shared across preparation and publication.
/// Construction is private to the aggregate builder; borrowing this inventory
/// does not mint a sealed task proof or bind it to an artifact generation.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlanInventory {
    pub(crate) type_table: RuntimePlanTypeTable,
    pub(crate) local_declarations: RuntimeLocalDeclarationTable,
    pub(crate) nominal_record_domains: RuntimeNominalRecordDomainTable,
    pub(crate) variant_domains: RuntimeVariantDomainTable,
    pub(crate) function_sites: RuntimeFunctionSiteTable,
    pub(crate) control_effect_contracts: RuntimeControlEffectContractTable,
    /// Dense registration-site order; each row names one executable function
    /// body with a capture-only, Unit-returning ABI.
    pub(crate) defer_sites: Box<[crate::runtime_id::RuntimeFunctionSiteId]>,
    pub(crate) callable_states: RuntimeCallableStateTable,
    pub(crate) callable_specializations: Box<
        [RuntimeCallableSpecializationDefinition<
            RuntimePlanTypeId,
            crate::runtime_id::RuntimeCallableStateId,
        >],
    >,
    pub(crate) project_call_sites: RuntimeProjectCallSiteTable,
    pub(crate) format_attempts: RuntimeFormatAttemptTable,
    pub(crate) dialogue_content: RuntimeDialogueContentPlanTable,
    pub(crate) entries: Vec<RuntimeEntrySpec>,
    pub(crate) callable_executables: Vec<RuntimeCallableExecutable>,
    pub(crate) flow_executables: Vec<RuntimeFlowExecutable>,
    pub(crate) flows: super::flows::RuntimeFlowTable,
    pub(crate) pure_helpers: Vec<RuntimePureHelper>,
    pub(crate) pure_programs: super::pure_programs::RuntimePureProgramTable,
    pub(crate) trait_methods: Vec<RuntimeTraitMethod>,
    pub(crate) line_task_groups: Vec<LineTaskGroup>,
    pub(crate) stream_plans: Vec<StreamPlan>,
}

impl RuntimePlanInventory {
    #[must_use]
    pub const fn control_effect_contracts(&self) -> &RuntimeControlEffectContractTable {
        &self.control_effect_contracts
    }

    #[must_use]
    pub const fn callable_states(&self) -> &RuntimeCallableStateTable {
        &self.callable_states
    }

    #[must_use]
    pub const fn callable_specializations(
        &self,
    ) -> &[RuntimeCallableSpecializationDefinition<
        RuntimePlanTypeId,
        crate::runtime_id::RuntimeCallableStateId,
    >] {
        &self.callable_specializations
    }

    #[must_use]
    pub const fn type_table(&self) -> &RuntimePlanTypeTable {
        &self.type_table
    }

    #[must_use]
    pub const fn local_declarations(&self) -> &RuntimeLocalDeclarationTable {
        &self.local_declarations
    }

    #[must_use]
    pub const fn nominal_record_domains(&self) -> &RuntimeNominalRecordDomainTable {
        &self.nominal_record_domains
    }

    #[must_use]
    pub const fn variant_domains(&self) -> &RuntimeVariantDomainTable {
        &self.variant_domains
    }

    #[must_use]
    pub const fn function_sites(&self) -> &RuntimeFunctionSiteTable {
        &self.function_sites
    }

    #[must_use]
    pub const fn format_attempts(&self) -> &RuntimeFormatAttemptTable {
        &self.format_attempts
    }

    #[must_use]
    pub fn format_attempt(
        &self,
        id: crate::runtime_id::RuntimeFormatAttemptId,
    ) -> Option<&RuntimeFormatAttempt> {
        self.format_attempts.get(id)
    }

    #[must_use]
    pub fn defer_function_site(
        &self,
        site: crate::runtime_id::RuntimeDeferSiteId,
    ) -> Option<crate::runtime_id::RuntimeFunctionSiteId> {
        self.defer_sites.get(site.index()).copied()
    }

    #[must_use]
    pub fn defer_sites(&self) -> &[crate::runtime_id::RuntimeFunctionSiteId] {
        &self.defer_sites
    }

    #[must_use]
    pub const fn project_call_sites(&self) -> &RuntimeProjectCallSiteTable {
        &self.project_call_sites
    }

    #[must_use]
    pub const fn dialogue_content(&self) -> &RuntimeDialogueContentPlanTable {
        &self.dialogue_content
    }

    #[must_use]
    pub const fn dialogue_content_templates(&self) -> &RuntimeDialogueContentTemplateManifestTable {
        self.dialogue_content.templates()
    }

    #[must_use]
    pub fn entries(&self) -> &[RuntimeEntrySpec] {
        &self.entries
    }

    #[must_use]
    pub fn callable_executables(&self) -> &[RuntimeCallableExecutable] {
        &self.callable_executables
    }

    #[must_use]
    pub fn flow_executables(&self) -> &[RuntimeFlowExecutable] {
        &self.flow_executables
    }

    #[must_use]
    pub fn flow_schemas(&self) -> &[RuntimeFlowSchema] {
        self.flows.schemas()
    }

    #[must_use]
    pub fn flows(&self) -> &[RuntimeFlow] {
        self.flows.as_slice()
    }

    #[must_use]
    pub fn pure_helpers(&self) -> &[RuntimePureHelper] {
        &self.pure_helpers
    }

    #[must_use]
    pub fn pure_programs(&self) -> &[RuntimePureProgramBinding] {
        self.pure_programs.as_slice()
    }

    pub(crate) fn resolve_pure_program(
        &self,
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    ) -> Result<&RuntimePureProgramBinding, super::pure_programs::RuntimePureProgramLookupError>
    {
        self.pure_programs.resolve(program)
    }

    #[must_use]
    pub fn trait_methods(&self) -> &[RuntimeTraitMethod] {
        &self.trait_methods
    }

    #[must_use]
    pub fn line_task_groups(&self) -> &[LineTaskGroup] {
        &self.line_task_groups
    }

    #[must_use]
    pub fn stream_plans(&self) -> &[StreamPlan] {
        &self.stream_plans
    }

    /// Derives the complete checked predicate in plan context. Nominal enum
    /// cases come from the owner-keyed variant domain rather than a copied
    /// type-row sidecar.
    pub fn checked_type(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        self.checked_type_inner(ty, &mut BTreeMap::new(), &mut BTreeSet::new())
    }

    /// Derives the final execution class from the complete plan-owned graph.
    pub fn type_class(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeClass, RuntimePlanTypeResolutionError> {
        self.type_table.class(ty)
    }

    fn checked_type_inner(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        if let Some(checked) = memo.get(&ty) {
            return Ok(checked.clone());
        }
        if !visiting.insert(ty) {
            return Err(RuntimePlanTypeResolutionError::CheckedProjectionCycle { ty });
        }
        let declaration = self
            .type_table
            .get(ty)
            .ok_or(RuntimePlanTypeResolutionError::UnknownType { ty })?;
        let checked = match declaration.projection() {
            RuntimePlanTypeProjection::Never => Some(RuntimeCheckedType::Never),
            RuntimePlanTypeProjection::Unit => Some(RuntimeCheckedType::Unit),
            RuntimePlanTypeProjection::Bool => Some(RuntimeCheckedType::Bool),
            RuntimePlanTypeProjection::Signed(width) => Some(RuntimeCheckedType::Signed(*width)),
            RuntimePlanTypeProjection::Unsigned(width) => {
                Some(RuntimeCheckedType::Unsigned(*width))
            }
            RuntimePlanTypeProjection::F32 => Some(RuntimeCheckedType::F32),
            RuntimePlanTypeProjection::F64 => Some(RuntimeCheckedType::F64),
            RuntimePlanTypeProjection::String => Some(RuntimeCheckedType::String),
            RuntimePlanTypeProjection::Color => Some(RuntimeCheckedType::Color),
            RuntimePlanTypeProjection::Char => Some(RuntimeCheckedType::Char),
            RuntimePlanTypeProjection::Bytes => Some(RuntimeCheckedType::Bytes),
            RuntimePlanTypeProjection::Duration => Some(RuntimeCheckedType::Duration),
            RuntimePlanTypeProjection::Progress => Some(RuntimeCheckedType::Progress),
            RuntimePlanTypeProjection::EntityReference => Some(RuntimeCheckedType::EntityReference),
            RuntimePlanTypeProjection::AgentValue => Some(RuntimeCheckedType::AgentValue),
            RuntimePlanTypeProjection::Sequence { item, .. } => self
                .checked_type_inner(*item, memo, visiting)?
                .map(|item| RuntimeCheckedType::Sequence(Box::new(item))),
            RuntimePlanTypeProjection::Map { kind, key, value } => {
                let Some(key) = self.checked_type_inner(*key, memo, visiting)? else {
                    visiting.remove(&ty);
                    memo.insert(ty, None);
                    return Ok(None);
                };
                let Some(value) = self.checked_type_inner(*value, memo, visiting)? else {
                    visiting.remove(&ty);
                    memo.insert(ty, None);
                    return Ok(None);
                };
                Some(RuntimeCheckedType::Map {
                    kind: *kind,
                    key: Box::new(key),
                    value: Box::new(value),
                })
            }
            RuntimePlanTypeProjection::Array { item, length } => self
                .checked_type_inner(*item, memo, visiting)?
                .zip(length.constant())
                .map(|(item, length)| RuntimeCheckedType::Array {
                    item: Box::new(item),
                    length,
                }),
            RuntimePlanTypeProjection::Nominal {
                nominal,
                layout,
                arguments,
            } => self.checked_nominal_or_variant(
                ty,
                nominal,
                declaration.semantic_identity(),
                *layout,
                arguments,
                CheckedTypeTraversal { memo, visiting },
            )?,
            RuntimePlanTypeProjection::Tuple(items) => self
                .checked_children(items, memo, visiting)?
                .map(RuntimeCheckedType::Tuple),
            RuntimePlanTypeProjection::Record(fields) => {
                self.checked_record_type(ty, fields, memo, visiting)?
            }
            RuntimePlanTypeProjection::Choice(items) => self
                .checked_children(items, memo, visiting)?
                .map(RuntimeCheckedType::Choice),
            RuntimePlanTypeProjection::Result { value, error, .. } => {
                self.checked_result_type(*value, *error, memo, visiting)?
            }
            RuntimePlanTypeProjection::Option { item, .. } => self
                .checked_type_inner(*item, memo, visiting)?
                .map(|item| RuntimeCheckedType::Option(Box::new(item))),
            RuntimePlanTypeProjection::BuiltinVariant { owner, cases } => {
                self.checked_builtin_variant_type(*owner, cases, memo, visiting)?
            }
            RuntimePlanTypeProjection::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                arguments,
            } => {
                if self.variant_domains.get(ty).is_some() {
                    self.checked_variant(
                        ty,
                        declaration.semantic_identity(),
                        arguments,
                        memo,
                        visiting,
                    )?
                } else {
                    Some(RuntimeCheckedType::Opaque {
                        owner: RuntimeOpaqueTypeOwner::with_admission(
                            producer.clone(),
                            declaration.semantic_identity(),
                            *admission,
                            *value_class,
                            *persistence,
                        ),
                    })
                }
            }
            RuntimePlanTypeProjection::Agent(agent) => {
                let checked = agent
                    .clone()
                    .try_map(|child| self.checked_type_inner(child, memo, visiting))?;
                checked
                    .try_map(|child| child.map(Box::new).ok_or(()))
                    .ok()
                    .map(RuntimeCheckedType::Agent)
            }
            RuntimePlanTypeProjection::Range(_)
            | RuntimePlanTypeProjection::Iterator(_)
            | RuntimePlanTypeProjection::Need(_)
            | RuntimePlanTypeProjection::Stream { .. }
            | RuntimePlanTypeProjection::ThreadHandle(_)
            | RuntimePlanTypeProjection::Shared(_)
            | RuntimePlanTypeProjection::Reference(_)
            | RuntimePlanTypeProjection::Function { .. }
            | RuntimePlanTypeProjection::BoundType(_) => None,
        };
        visiting.remove(&ty);
        memo.insert(ty, checked.clone());
        Ok(checked)
    }

    fn checked_record_type(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        fields: &[RuntimePlanRecordField<crate::runtime_id::RuntimePlanTypeId>],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let mut checked = Vec::with_capacity(fields.len());
        for (ordinal, field) in fields.iter().enumerate() {
            let Some(field_ty) = self.checked_type_inner(*field.ty(), memo, visiting)? else {
                return Ok(None);
            };
            let field_id =
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).map_err(|_| {
                    RuntimePlanTypeResolutionError::InvalidCheckedRecord {
                        ty,
                        source: RuntimeCheckedRecordTypeError::FieldOrdinalOverflow,
                    }
                })?;
            checked.push((field_id, field.diagnostic_name().to_owned(), field_ty));
        }
        RuntimeCheckedType::try_record(checked)
            .map(Some)
            .map_err(|source| RuntimePlanTypeResolutionError::InvalidCheckedRecord { ty, source })
    }

    fn checked_result_type(
        &self,
        value: crate::runtime_id::RuntimePlanTypeId,
        error: crate::runtime_id::RuntimePlanTypeId,
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let Some(value) = self.checked_type_inner(value, memo, visiting)? else {
            return Ok(None);
        };
        let Some(error) = self.checked_type_inner(error, memo, visiting)? else {
            return Ok(None);
        };
        Ok(Some(RuntimeCheckedType::Result {
            ok: Box::new(value),
            error: Box::new(error),
        }))
    }

    fn checked_builtin_variant_type(
        &self,
        owner: RuntimeBuiltinVariantIdentity,
        cases: &[Option<crate::runtime_id::RuntimePlanTypeId>],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let mut checked_cases = Vec::with_capacity(cases.len());
        for (schema, payload) in owner.cases().iter().zip(cases) {
            let payload = match payload {
                Some(payload) => {
                    let Some(payload) = self.checked_type_inner(*payload, memo, visiting)? else {
                        return Ok(None);
                    };
                    Some(Box::new(payload))
                }
                None => None,
            };
            checked_cases.push(RuntimeCheckedVariantCase {
                name: schema.name().to_owned(),
                payload,
            });
        }
        Ok(Some(RuntimeCheckedType::Variant {
            owner: crate::pattern::RuntimeVariantIdentity::Builtin(owner),
            arguments: Vec::new(),
            cases: checked_cases,
        }))
    }

    fn checked_nominal_or_variant(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        nominal: &RuntimeNominalTypeId,
        semantic_identity: crate::pattern::RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        arguments: &[crate::runtime_id::RuntimePlanTypeId],
        traversal: CheckedTypeTraversal<'_>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let CheckedTypeTraversal { memo, visiting } = traversal;
        if self.variant_domains.get(ty).is_some() {
            return self.checked_variant(ty, semantic_identity, arguments, memo, visiting);
        }
        let Some(arguments) = self.checked_children(arguments, memo, visiting)? else {
            return Ok(None);
        };
        Ok(Some(RuntimeCheckedType::Nominal {
            nominal: nominal.clone(),
            semantic_identity,
            layout,
            arguments,
        }))
    }

    fn checked_variant(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        semantic_identity: crate::pattern::RuntimeSemanticTypeId,
        arguments: &[crate::runtime_id::RuntimePlanTypeId],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let Some(domain) = self.variant_domains.get(ty) else {
            return Ok(None);
        };
        let Some(arguments) = self.checked_children(arguments, memo, visiting)? else {
            return Ok(None);
        };
        let mut cases = Vec::with_capacity(domain.cases().len());
        for case in domain.cases() {
            let payload = match case.payload() {
                Some(payload) => {
                    let Some(payload) = self.checked_type_inner(payload, memo, visiting)? else {
                        return Ok(None);
                    };
                    Some(Box::new(payload))
                }
                None => None,
            };
            cases.push(RuntimeCheckedVariantCase {
                name: case.name().to_owned(),
                payload,
            });
        }
        Ok(Some(RuntimeCheckedType::Variant {
            owner: crate::pattern::RuntimeVariantIdentity::Nominal {
                nominal: domain.nominal().clone(),
                semantic_identity,
                layout: domain.layout(),
            },
            arguments,
            cases,
        }))
    }

    fn checked_children(
        &self,
        children: &[crate::runtime_id::RuntimePlanTypeId],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<Vec<RuntimeCheckedType>>, RuntimePlanTypeResolutionError> {
        let mut checked = Vec::with_capacity(children.len());
        for child in children {
            let Some(child) = self.checked_type_inner(*child, memo, visiting)? else {
                return Ok(None);
            };
            checked.push(child);
        }
        Ok(Some(checked))
    }
}

impl RuntimePlanInventory {
    pub fn is_empty(&self) -> bool {
        self.flows.is_empty() && self.line_task_groups.is_empty() && self.stream_plans.is_empty()
    }

    /// Resolves one dynamic target against the exact accepted Flow inventory.
    ///
    /// A legacy canonical runtime ID still selects itself exactly. Otherwise
    /// the validated public label must identify one and only one accepted Flow;
    /// duplicate module-local labels are a terminal ambiguity.
    pub fn resolve_flow_target_value(
        &self,
        value: &str,
    ) -> Result<FlowRuntimeId, RuntimeFlowTargetError> {
        self.flows.resolve_target(value)
    }
}
