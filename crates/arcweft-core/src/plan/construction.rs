//! Sole mutable construction authority for a runtime plan.

use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::sync::Arc;

use thiserror::Error;

mod lower;
mod seed;

use seed::RuntimePlanConstructionIssuer;
pub use seed::{
    RuntimeAgentExprSeed, RuntimeAudioCommandSeed, RuntimeAwaitManyTargetSeed,
    RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed, RuntimeCallArgumentSeed,
    RuntimeCallableExecutableSeed, RuntimeCallableExecutableSeedCode, RuntimeChoiceOptionSeed,
    RuntimeDialogueContentPlanSeed, RuntimeDialogueContentPlanSeedId, RuntimeDialogueMarkSeedId,
    RuntimeDialogueValueSiteSeed, RuntimeEffectFieldSeed, RuntimeEvaluatedEffectSeed,
    RuntimeExprMatchArmSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFieldProjectionSeed,
    RuntimeFlowMatchArmSeed, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId, RuntimeHostArgumentSeed,
    RuntimeHostCallTargetSeed, RuntimeHostTaskRequestTemplateSeed, RuntimeIteratorEvidenceSeed,
    RuntimeIteratorWitnessEvidenceSeed, RuntimeIteratorWitnessExecutableSeed,
    RuntimeLineEffectSeed, RuntimeLineTaskCancelRuleSeed, RuntimeLineTaskGroupSeed,
    RuntimeLineTaskGroupSeedId, RuntimeLineTaskNodeSeed, RuntimeLineTaskTriggerSeed,
    RuntimeLocalDeclarationSeed, RuntimeLocalSeedId, RuntimeNominalRecordFieldSeed,
    RuntimePatternRestSeed, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimePureHelperDeclarationSeed, RuntimePureHelperSeed, RuntimePureHelperSeedId,
    RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed, RuntimeStreamMatchArmSeed,
    RuntimeStreamOpSeed, RuntimeStreamPlanSeed, RuntimeTraitMethodDeclarationSeed,
    RuntimeTraitMethodSeed, RuntimeTraitMethodSeedId,
};

use crate::entry::{
    RuntimeCallableExecutable, RuntimeCallableExecutableCode, RuntimeFlowExecutable,
    RuntimeNominalTypeId,
};
use crate::line_task::{
    LineCancelRule, LineTaskCleanup, LineTaskGroup, LineTaskNode, LineTaskTrigger,
};
use crate::pattern::{RuntimePatternBindingPathError, RuntimeSemanticTypeId};
use crate::runtime_id::{
    RuntimeDialogueContentPlanId, RuntimeDialogueMarkId, RuntimeLineTaskGroupId,
    RuntimeLineTaskNodeId, RuntimeLocalDeclarationId, RuntimePlanTypeId,
};
use crate::stream::StreamPlan;
use crate::value::{RuntimeAgentConstructor, RuntimeRecordFieldIdError};

use super::dialogue_content::RuntimeDialogueContentPlanTableBuilder;
use super::function_sites::{RuntimeFunctionSiteError, RuntimeFunctionSiteTableBuilder};
use super::local_declarations::{
    RuntimeLocalDeclarationTableBuilder, RuntimeLocalDeclarationTableError,
};
use super::nominal_record_domains::{
    RuntimeNominalRecordDomain, RuntimeNominalRecordDomainError, RuntimeNominalRecordDomainSeed,
    RuntimeNominalRecordDomainTableBuilder,
};
use super::type_table::{
    PreparedRuntimePlanTypeBatch, RuntimePlanTypeSeed, RuntimePlanTypeTableBuilder,
    RuntimePlanTypeTableError,
};
use super::variant_domains::{
    RuntimeVariantDomain, RuntimeVariantDomainError, RuntimeVariantDomainSeed,
    RuntimeVariantDomainTableBuilder,
};
use super::{
    RuntimeDialogueContentPlan, RuntimeDialogueMark, RuntimeDialogueValueRole,
    RuntimeDialogueValueSite, RuntimeEntrySpec, RuntimeFlow, RuntimePlan,
    RuntimePlanTypeProjection, RuntimePureHelper, RuntimeTraitMethod,
};

/// Result identities issued by one atomic semantic graph transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePlanSemanticAdmission {
    local_ids: Box<[RuntimeLocalSeedId]>,
}

impl RuntimePlanSemanticAdmission {
    #[must_use]
    pub const fn local_ids(&self) -> &[RuntimeLocalSeedId] {
        &self.local_ids
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePlanTable {
    DialogueContent,
    Entries,
    CallableExecutables,
    FlowExecutables,
    Flows,
    PureHelpers,
    TraitMethods,
    LineTaskGroups,
    StreamPlans,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimePlanBuildError {
    #[error("runtime-plan construction is poisoned by an earlier post-admission failure")]
    Poisoned,
    #[error(transparent)]
    TypeGraph(#[from] RuntimePlanTypeTableError),
    #[error(transparent)]
    LocalDeclarations(#[from] RuntimeLocalDeclarationTableError),
    #[error(transparent)]
    NominalRecordDomain(#[from] RuntimeNominalRecordDomainError),
    #[error(transparent)]
    VariantDomain(#[from] RuntimeVariantDomainError),
    #[error(transparent)]
    FunctionSite(#[from] RuntimeFunctionSiteError),
    #[error(transparent)]
    DialogueContent(#[from] super::RuntimeDialogueContentPlanTableError),
    #[error(transparent)]
    Plan(#[from] super::RuntimePlanError),
    #[error("semantic type {semantic_identity:?} is absent from the transaction type graph")]
    UnknownSemanticType {
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error("type {owner} is not a nominal owner for a record domain")]
    InvalidNominalRecordOwner { owner: RuntimePlanTypeId },
    #[error("type {owner} is not a nominal owner for a variant domain")]
    InvalidVariantOwner { owner: RuntimePlanTypeId },
    #[error("variant domain nominal `{actual:?}` does not match owner type `{expected:?}`")]
    VariantNominalMismatch {
        owner: RuntimePlanTypeId,
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },
    #[error("nominal owner type {owner} cannot have both record and variant domains")]
    ConflictingNominalDomainKinds { owner: RuntimePlanTypeId },
    #[error("function site references local {local} outside the plan")]
    UnknownFunctionLocal { local: RuntimeLocalDeclarationId },
    #[error("function body references local {local} outside its lexical scope")]
    UnreachableFunctionLocal { local: RuntimeLocalDeclarationId },
    #[error("function site declares capture {local} that is not used by its body")]
    UnusedFunctionCapture { local: RuntimeLocalDeclarationId },
    #[error("function body references unknown function site {site}")]
    UnknownFunctionSite {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    #[error("a construction-only local handle belongs to another runtime-plan builder")]
    ForeignLocalSeed,
    #[error("a construction-only function-site handle belongs to another runtime-plan builder")]
    ForeignFunctionSiteSeed,
    #[error("a construction-only dialogue content handle belongs to another runtime-plan builder")]
    ForeignDialogueContentSeed,
    #[error("a construction-only dialogue mark handle belongs to another runtime-plan builder")]
    ForeignDialogueMarkSeed,
    #[error("a construction-only line-task group handle belongs to another runtime-plan builder")]
    ForeignLineTaskGroupSeed,
    #[error("line-task graph has no node at required dense ordinal {ordinal}")]
    InvalidLineTaskNodeOrdinal { ordinal: usize },
    #[error("line-task Detach requires a proved ownership-transfer target and is not admitted")]
    UnsupportedLineTaskDetach,
    #[error("line-task group is attached to a dialogue content plan more than once")]
    DuplicateDialogueLineTaskGroup,
    #[error(
        "line-task mark belongs to dialogue content {actual} rather than attached content {expected}"
    )]
    LineTaskMarkContentMismatch {
        expected: RuntimeDialogueContentPlanId,
        actual: RuntimeDialogueContentPlanId,
    },
    #[error("sealed line-task group {group} is not attached to any dialogue content plan")]
    OrphanLineTaskGroup { group: RuntimeLineTaskGroupId },
    #[error("dialogue content slot {actual} is not the expected canonical slot {expected}")]
    NonCanonicalDialogueValueSlot {
        expected: crate::runtime_id::RuntimeDialogueValueSlotId,
        actual: crate::runtime_id::RuntimeDialogueValueSlotId,
    },
    #[error("dialogue condition slot {slot} has non-Bool plan type {ty}")]
    InvalidDialogueConditionType {
        slot: crate::runtime_id::RuntimeDialogueValueSlotId,
        ty: RuntimePlanTypeId,
    },
    #[error("a construction-only pure-helper handle belongs to another runtime-plan builder")]
    ForeignPureHelperSeed,
    #[error("a construction-only trait-method handle belongs to another runtime-plan builder")]
    ForeignTraitMethodSeed,
    #[error("invalid iterator witness signature at {context}")]
    InvalidIteratorWitness { context: &'static str },
    #[error("AwaitMany concurrency limit must be greater than zero")]
    ZeroAwaitManyLimit,
    #[error("runtime function site {site} was defined more than once")]
    DuplicateFunctionSiteDefinition {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    #[error("runtime pure helper {helper:?} was defined more than once")]
    DuplicatePureHelperDefinition { helper: super::RuntimePureHelperId },
    #[error("runtime trait method {method:?} was defined more than once")]
    DuplicateTraitMethodDefinition { method: super::RuntimeTraitMethodId },
    #[error(
        "runtime-plan construction has incomplete definitions: {function_sites} function site(s), {pure_helpers} pure helper(s), {trait_methods} trait method(s)"
    )]
    IncompleteDefinitions {
        function_sites: usize,
        pure_helpers: usize,
        trait_methods: usize,
    },
    #[error("function site repeats local declaration {local}")]
    DuplicateFunctionLocal { local: RuntimeLocalDeclarationId },
    #[error("function site uses local declaration {local} as both a parameter and capture")]
    FunctionParameterCaptureOverlap { local: RuntimeLocalDeclarationId },
    #[error("{context} has {actual} ABI rows for {expected} input locals")]
    CallableAbiArity {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{context} input {index} plan type {ty} does not match its scalar ABI")]
    CallableInputAbi {
        context: &'static str,
        index: usize,
        ty: RuntimePlanTypeId,
    },
    #[error("{context} result plan type {ty} does not match its scalar ABI")]
    CallableOutputAbi {
        context: &'static str,
        ty: RuntimePlanTypeId,
    },
    #[error("runtime trait method has no receiver input")]
    MissingTraitMethodReceiver,
    #[error(
        "{context} references semantic type {semantic_identity:?} that is absent from the admitted graph"
    )]
    UnknownSeedType {
        context: &'static str,
        semantic_identity: RuntimeSemanticTypeId,
    },
    #[error("{context} expected plan type {expected}, found {actual}")]
    TypeMismatch {
        context: &'static str,
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
    },
    #[error("{context} cannot use plan type {ty} with its admitted projection")]
    InvalidTypeProjection {
        context: &'static str,
        ty: RuntimePlanTypeId,
    },
    #[error("runtime-plan {context} contains a live function value")]
    FunctionValueInPlan { context: &'static str },
    #[error("runtime-plan {context} contains a detached expression carrier")]
    RawExpressionCarrier { context: &'static str },
    #[error("runtime-plan contains non-canonical flow operation {operation}")]
    NonCanonicalFlowOperation { operation: &'static str },
    #[error("runtime-plan {context} value does not satisfy plan type {ty}")]
    InvalidValueType {
        context: &'static str,
        ty: RuntimePlanTypeId,
    },
    #[error("nominal record type {owner} has no admitted field domain")]
    UnknownNominalRecordDomain { owner: RuntimePlanTypeId },
    #[error("record type {owner} has no field at zero-based ordinal {ordinal}")]
    UnknownRecordField {
        owner: RuntimePlanTypeId,
        ordinal: u32,
    },
    #[error("record type {owner} repeats field {field}")]
    DuplicateRecordField {
        owner: RuntimePlanTypeId,
        field: crate::value::RuntimeRecordFieldId,
    },
    #[error("record type {owner} is missing field {field}")]
    MissingRecordField {
        owner: RuntimePlanTypeId,
        field: crate::value::RuntimeRecordFieldId,
    },
    #[error("variant type {owner} has no case at zero-based ordinal {ordinal}")]
    UnknownVariantCase {
        owner: RuntimePlanTypeId,
        ordinal: u32,
    },
    #[error("variant type {owner} case {ordinal} expected payload {expected:?}, found {actual:?}")]
    VariantPayloadMismatch {
        owner: RuntimePlanTypeId,
        ordinal: u32,
        expected: Option<RuntimePlanTypeId>,
        actual: Option<RuntimePlanTypeId>,
    },
    #[error("pattern binds local declaration {local} more than once")]
    DuplicatePatternBinding { local: RuntimeLocalDeclarationId },
    #[error(transparent)]
    PatternBindingPath(#[from] RuntimePatternBindingPathError),
    #[error(transparent)]
    RecordFieldIdentity(#[from] RuntimeRecordFieldIdError),
    #[error("Agent constructor {constructor:?} has an invalid typed expression shape")]
    InvalidAgentExpression {
        constructor: RuntimeAgentConstructor,
    },
    #[error("Agent constructor {constructor:?} operand {operand} has invalid plan type {actual}")]
    InvalidAgentOperandType {
        constructor: RuntimeAgentConstructor,
        operand: usize,
        actual: RuntimePlanTypeId,
    },
    #[error("Agent constructor {constructor:?} result has invalid plan type {actual}")]
    InvalidAgentResultType {
        constructor: RuntimeAgentConstructor,
        actual: RuntimePlanTypeId,
    },
    #[error("spread argument has non-expandable plan type {ty}")]
    IndeterminateSpreadArgument { ty: RuntimePlanTypeId },
    #[error("runtime range expression must retain at least one typed bound")]
    EmptyRangeExpression,
    #[error("runtime sequence expected {expected} item(s), found {actual}")]
    SequenceLengthMismatch { expected: u64, actual: usize },
    #[error(
        "Reduction.unchanged root type {ty} is not an admitted single-argument opaque reduction"
    )]
    InvalidReductionUnchanged { ty: RuntimePlanTypeId },
    #[error("runtime plan table {table:?} exceeds its u32 row limit")]
    TooManyRows { table: RuntimePlanTable },
    #[error("runtime flow `{flow}` contains duplicate parameter local {local}")]
    DuplicateFlowParameter {
        flow: String,
        local: RuntimeLocalDeclarationId,
    },
    #[error("runtime flow `{flow}` executable repeats parameter name `{name}`")]
    DuplicateFlowParameterName { flow: String, name: String },
    #[error("runtime flow `{flow}` executable parameter {index} has an empty name")]
    EmptyFlowParameterName { flow: String, index: usize },
    #[error("runtime flow `{flow}` references unknown parameter local {local}")]
    UnknownFlowParameter {
        flow: String,
        local: RuntimeLocalDeclarationId,
    },
    #[error("runtime flow `{flow}` is defined more than once")]
    DuplicateFlowDefinition { flow: String },
    #[error("runtime flow `{flow}` has more than one executable metadata row")]
    DuplicateFlowExecutable { flow: String },
    #[error("runtime flow executable `{flow}` has no matching plan flow")]
    MissingFlowDefinition { flow: String },
    #[error("runtime flow `{flow}` has {actual} local parameters, expected {expected}")]
    FlowParameterCount {
        flow: String,
        expected: usize,
        actual: usize,
    },
    #[error(
        "runtime flow `{flow}` executable parameter {index} has non-canonical position {actual}"
    )]
    FlowParameterPosition {
        flow: String,
        index: usize,
        actual: u32,
    },
    #[error(
        "runtime flow `{flow}` parameter {index} local {local} has {actual}, expected {expected}"
    )]
    FlowParameterType {
        flow: String,
        index: usize,
        local: RuntimeLocalDeclarationId,
        expected: String,
        actual: String,
    },
    #[error("runtime stream `{stream}` is defined more than once")]
    DuplicateStreamDefinition { stream: String },
}

#[derive(Debug)]
struct ReservedFunctionSite {
    params: Box<[RuntimeLocalDeclarationId]>,
    captures: Box<[RuntimeLocalDeclarationId]>,
    body: Option<crate::value::RuntimeExpr>,
}

#[derive(Debug)]
struct ReservedPureHelper {
    name: String,
    input_locals: Box<[RuntimeLocalDeclarationId]>,
    input_abi: Vec<super::RuntimePureInputType>,
    output_abi: super::RuntimePureOutputType,
    scalar_eval_supported: bool,
    origin: super::RuntimePureHelperOrigin,
    body: Option<crate::value::RuntimeExpr>,
}

#[derive(Debug)]
struct ReservedTraitMethod {
    identity: super::RuntimeTraitMethodIdentity,
    receiver: super::RuntimeReceiverMode,
    input_locals: Box<[RuntimeLocalDeclarationId]>,
    input_abi: Vec<super::RuntimePureInputType>,
    output_abi: super::RuntimePureOutputType,
    body: Option<crate::value::RuntimeExpr>,
}

/// Sole mutable aggregate owner. Its internal issuers are never published or
/// cloned; successful `finish` consumes them into one immutable plan.
#[derive(Debug)]
pub struct RuntimePlanBuilder {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    poisoned: bool,
    types: RuntimePlanTypeTableBuilder,
    locals: RuntimeLocalDeclarationTableBuilder,
    nominal_record_domains: RuntimeNominalRecordDomainTableBuilder,
    variant_domains: RuntimeVariantDomainTableBuilder,
    function_sites: Vec<ReservedFunctionSite>,
    dialogue_content: RuntimeDialogueContentPlanTableBuilder,
    entries: Vec<RuntimeEntrySpec>,
    callable_executables: Vec<RuntimeCallableExecutable>,
    flow_executables: Vec<RuntimeFlowExecutable>,
    flows: Vec<RuntimeFlow>,
    pure_helpers: Vec<ReservedPureHelper>,
    trait_methods: Vec<ReservedTraitMethod>,
    line_task_groups: Vec<LineTaskGroup>,
    line_task_group_mark_owners: Vec<BTreeSet<RuntimeDialogueContentPlanId>>,
    line_task_group_attachments: Vec<bool>,
    stream_plans: Vec<StreamPlan>,
}

impl RuntimePlanBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            issuer: Arc::new(RuntimePlanConstructionIssuer),
            poisoned: false,
            types: RuntimePlanTypeTableBuilder::new(),
            locals: RuntimeLocalDeclarationTableBuilder::new(),
            nominal_record_domains: RuntimeNominalRecordDomainTableBuilder::new(),
            variant_domains: RuntimeVariantDomainTableBuilder::new(),
            function_sites: Vec::new(),
            dialogue_content: RuntimeDialogueContentPlanTableBuilder::new(),
            entries: Vec::new(),
            callable_executables: Vec::new(),
            flow_executables: Vec::new(),
            flows: Vec::new(),
            pure_helpers: Vec::new(),
            trait_methods: Vec::new(),
            line_task_groups: Vec::new(),
            line_task_group_mark_owners: Vec::new(),
            line_task_group_attachments: Vec::new(),
            stream_plans: Vec::new(),
        }
    }

    /// Atomically rewrites semantic type, local, record-domain, and
    /// variant-domain seeds into final plan-local tables.
    ///
    /// Every subtable is prepared before any issuer commits. Consequently a
    /// failure in the last domain leaves type and local row counts unchanged.
    pub fn admit_semantic_batch(
        &mut self,
        types: impl IntoIterator<Item = RuntimePlanTypeSeed>,
        locals: impl IntoIterator<Item = RuntimeLocalDeclarationSeed>,
        nominal_record_domains: impl IntoIterator<Item = RuntimeNominalRecordDomainSeed>,
        variant_domains: impl IntoIterator<Item = RuntimeVariantDomainSeed>,
    ) -> Result<RuntimePlanSemanticAdmission, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let prepared_types = self.types.prepare_batch(types)?;
        let locals = locals.into_iter().collect::<Box<[_]>>();
        let declared_local_types = locals
            .iter()
            .map(|local| resolve_semantic_type(&prepared_types, local.ty()))
            .collect::<Result<Box<[_]>, _>>()?;
        let prepared_locals = self
            .locals
            .prepare_batch(declared_local_types.iter().copied())?;

        let record_domains = nominal_record_domains
            .into_iter()
            .map(|seed| rewrite_record_domain(&prepared_types, &seed))
            .collect::<Result<Vec<_>, _>>()?;
        let variant_domains = variant_domains
            .into_iter()
            .map(|seed| rewrite_variant_domain(&prepared_types, &seed))
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_domain_exclusivity(&record_domains, &variant_domains)?;
        let prepared_records = self.nominal_record_domains.prepare_batch(record_domains)?;
        let prepared_variants = self.variant_domains.prepare_batch(variant_domains)?;

        self.types.commit_batch(prepared_types);
        let admitted_local_ids = self.locals.commit_batch(prepared_locals);
        self.nominal_record_domains.commit_batch(prepared_records);
        self.variant_domains.commit_batch(prepared_variants);
        let local_ids = admitted_local_ids
            .into_vec()
            .into_iter()
            .zip(declared_local_types)
            .map(|(local, ty)| RuntimeLocalSeedId::issued(&self.issuer, local, ty))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(RuntimePlanSemanticAdmission { local_ids })
    }

    pub fn push_function_site_seed(
        &mut self,
        params: impl IntoIterator<Item = RuntimeLocalSeedId>,
        captures: impl IntoIterator<Item = RuntimeLocalSeedId>,
        body: RuntimeExprSeed,
    ) -> Result<RuntimeFunctionSiteSeedId, RuntimePlanBuildError> {
        let result = body.ty();
        let site = self.reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
            params: params.into_iter().collect(),
            captures: captures.into_iter().collect(),
            result,
        })?;
        self.define_function_site_seed(&site, body)?;
        Ok(site)
    }

    pub fn reserve_function_site_seed(
        &mut self,
        seed: RuntimeFunctionSiteDeclarationSeed,
    ) -> Result<RuntimeFunctionSiteSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_function_site_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_function_site_seed(
        &mut self,
        seed: RuntimeFunctionSiteDeclarationSeed,
    ) -> Result<RuntimeFunctionSiteSeedId, RuntimePlanBuildError> {
        let params = self.resolve_function_locals(seed.params)?;
        let captures = self.resolve_function_locals(seed.captures)?;
        let parameter_set = params
            .iter()
            .map(|(local, _)| *local)
            .collect::<BTreeSet<_>>();
        for (local, _) in &captures {
            if parameter_set.contains(local) {
                return Err(RuntimePlanBuildError::FunctionParameterCaptureOverlap {
                    local: *local,
                });
            }
        }
        let parameter_types = params
            .iter()
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let params = params
            .into_iter()
            .map(|(local, _)| local)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let captures = captures
            .into_iter()
            .map(|(local, _)| local)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = self.resolve_seed_type("function result", seed.result)?;
        let ordinal = self
            .function_sites
            .len()
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .ok_or(RuntimeFunctionSiteError::IdentityExhausted)?;
        let site = crate::runtime_id::RuntimeFunctionSiteId::from_accepted_ordinal(ordinal);
        self.function_sites.push(ReservedFunctionSite {
            params,
            captures,
            body: None,
        });
        Ok(RuntimeFunctionSiteSeedId::issued(
            &self.issuer,
            site,
            parameter_types,
            result,
        ))
    }

    pub fn define_function_site_seed(
        &mut self,
        site: &RuntimeFunctionSiteSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_define_function_site_seed(site, body);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_define_function_site_seed(
        &mut self,
        site: &RuntimeFunctionSiteSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        let (site_id, _, result) = site
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        let index = usize::try_from(site_id.get().get() - 1)
            .map_err(|_| RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        let reserved = self
            .function_sites
            .get(index)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        if reserved.body.is_some() {
            return Err(RuntimePlanBuildError::DuplicateFunctionSiteDefinition { site: site_id });
        }
        let params = reserved.params.clone();
        let captures = reserved.captures.clone();
        let body = self.lower_expression(body)?;
        require_reserved_result("function result", result, body.ty())?;
        self.validate_function_body_locals(&body, &params, &captures)?;
        self.function_sites[index].body = Some(body);
        Ok(())
    }

    pub fn push_dialogue_content_seed(
        &mut self,
        seed: RuntimeDialogueContentPlanSeed,
    ) -> Result<RuntimeDialogueContentPlanSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_dialogue_content_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_push_dialogue_content_seed(
        &mut self,
        seed: RuntimeDialogueContentPlanSeed,
    ) -> Result<RuntimeDialogueContentPlanSeedId, RuntimePlanBuildError> {
        let mut values = Vec::with_capacity(seed.values.len());
        for (index, value) in seed.values.into_vec().into_iter().enumerate() {
            let expected = crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index)
                .ok_or(RuntimePlanBuildError::TooManyRows {
                    table: RuntimePlanTable::DialogueContent,
                })?;
            if value.slot != expected {
                return Err(RuntimePlanBuildError::NonCanonicalDialogueValueSlot {
                    expected,
                    actual: value.slot,
                });
            }
            let (function, parameters, result) = value
                .function
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
            if !parameters.is_empty() {
                return Err(RuntimePlanBuildError::CallableAbiArity {
                    context: "dialogue value site",
                    expected: 0,
                    actual: parameters.len(),
                });
            }
            if value.role == RuntimeDialogueValueRole::Condition
                && self.require_bool("dialogue condition", result).is_err()
            {
                return Err(RuntimePlanBuildError::InvalidDialogueConditionType {
                    slot: value.slot,
                    ty: result,
                });
            }
            values.push(RuntimeDialogueValueSite::new(
                value.slot, value.role, function,
            ));
        }
        let marks = seed
            .marks
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let id = RuntimeDialogueMarkId::from_zero_based(index).ok_or(
                    RuntimePlanBuildError::TooManyRows {
                        table: RuntimePlanTable::DialogueContent,
                    },
                )?;
                Ok(RuntimeDialogueMark::new(id, label))
            })
            .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
        let content = self.dialogue_content.push(RuntimeDialogueContentPlan::new(
            seed.line,
            values.into_boxed_slice(),
            marks.into_boxed_slice(),
        ))?;
        Ok(RuntimeDialogueContentPlanSeedId::issued(
            &self.issuer,
            content,
        ))
    }

    /// Lowers a recursive, construction-only line-task seed into the one
    /// dense preorder graph admitted by this plan builder.
    pub fn push_line_task_group_seed(
        &mut self,
        seed: RuntimeLineTaskGroupSeed,
    ) -> Result<RuntimeLineTaskGroupSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_line_task_group_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_push_line_task_group_seed(
        &mut self,
        seed: RuntimeLineTaskGroupSeed,
    ) -> Result<RuntimeLineTaskGroupSeedId, RuntimePlanBuildError> {
        if line_task_seed_requests_detach(&seed) {
            return Err(RuntimePlanBuildError::UnsupportedLineTaskDetach);
        }
        let mark_owners = self.line_task_mark_owners(&seed)?;
        let captures = seed
            .free_locals()
            .into_vec()
            .into_iter()
            .map(|capture| {
                capture
                    .resolve(&self.issuer)
                    .map(|(local, _)| local)
                    .ok_or(RuntimePlanBuildError::ForeignLocalSeed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let capture_scope = captures.iter().copied().collect::<BTreeSet<_>>();
        let mut nodes = Vec::new();
        let root = self.lower_line_task_node_seed(seed.root, &mut nodes)?;
        let nodes = nodes
            .into_iter()
            .enumerate()
            .map(|(ordinal, node)| {
                node.ok_or(RuntimePlanBuildError::InvalidLineTaskNodeOrdinal { ordinal })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let cancel_rules = seed
            .cancel_rules
            .into_vec()
            .into_iter()
            .map(|rule| self.lower_line_task_cancel_rule_seed(rule))
            .collect::<Result<Vec<_>, _>>()?;
        let cleanup = LineTaskCleanup::new(
            self.lower_flow_ops(seed.cleanup_completed)?
                .into_boxed_slice(),
            self.lower_flow_ops(seed.cleanup_cancelled)?
                .into_boxed_slice(),
            self.lower_flow_ops(seed.cleanup_failed)?.into_boxed_slice(),
            seed.cleanup_policy,
        );
        let mut action_sets = nodes
            .iter()
            .filter_map(|node| match node {
                LineTaskNode::Action(actions) => Some(actions.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        action_sets.extend(cancel_rules.iter().map(LineCancelRule::action));
        action_sets.push(cleanup.actions(crate::line_task::ScopeExit::Completed));
        action_sets.push(cleanup.actions(crate::line_task::ScopeExit::Cancelled));
        action_sets.push(cleanup.actions(crate::line_task::ScopeExit::Failed));
        self.validate_line_task_actions_locals(&action_sets, &capture_scope)?;
        let group = RuntimeLineTaskGroupId::from_zero_based(self.line_task_groups.len()).ok_or(
            RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::LineTaskGroups,
            },
        )?;
        self.line_task_groups.push(LineTaskGroup::new(
            captures.into_boxed_slice(),
            root,
            nodes.into_boxed_slice(),
            cancel_rules.into_boxed_slice(),
            cleanup,
        ));
        self.line_task_group_mark_owners.push(mark_owners);
        self.line_task_group_attachments.push(false);
        Ok(RuntimeLineTaskGroupSeedId::issued(&self.issuer, group))
    }

    fn lower_line_task_node_seed(
        &self,
        seed: RuntimeLineTaskNodeSeed,
        nodes: &mut Vec<Option<LineTaskNode>>,
    ) -> Result<RuntimeLineTaskNodeId, RuntimePlanBuildError> {
        let ordinal = nodes.len();
        let id = RuntimeLineTaskNodeId::from_zero_based(ordinal).ok_or(
            RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::LineTaskGroups,
            },
        )?;
        nodes.push(None);
        let node = match seed {
            RuntimeLineTaskNodeSeed::Sequence(children) => LineTaskNode::Sequence(
                children
                    .into_iter()
                    .map(|child| self.lower_line_task_node_seed(child, nodes))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            RuntimeLineTaskNodeSeed::Start(children) => LineTaskNode::Start(
                children
                    .into_iter()
                    .map(|child| self.lower_line_task_node_seed(child, nodes))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            ),
            RuntimeLineTaskNodeSeed::Parallel { policy, children } => LineTaskNode::Parallel {
                policy,
                children: children
                    .into_iter()
                    .map(|child| self.lower_line_task_node_seed(child, nodes))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            },
            RuntimeLineTaskNodeSeed::Child {
                id,
                key,
                name,
                trigger,
                priority,
                join_policy,
                cancel_policy,
                scope,
            } => LineTaskNode::Child {
                id,
                key,
                name,
                trigger: self.lower_line_task_trigger_seed(trigger)?,
                priority,
                join_policy,
                cancel_policy,
                scope: self.lower_line_task_node_seed(*scope, nodes)?,
            },
            RuntimeLineTaskNodeSeed::Action(actions) => {
                LineTaskNode::Action(self.lower_flow_ops(actions)?.into_boxed_slice())
            }
        };
        nodes[ordinal] = Some(node);
        Ok(id)
    }

    fn lower_line_task_trigger_seed(
        &self,
        seed: RuntimeLineTaskTriggerSeed,
    ) -> Result<LineTaskTrigger, RuntimePlanBuildError> {
        match seed {
            RuntimeLineTaskTriggerSeed::Immediate => Ok(LineTaskTrigger::Immediate),
            RuntimeLineTaskTriggerSeed::Delay(delay) => Ok(LineTaskTrigger::Delay(delay)),
            RuntimeLineTaskTriggerSeed::Mark(mark) => {
                let (content, mark) = mark
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignDialogueMarkSeed)?;
                if self
                    .dialogue_content
                    .get(content)
                    .and_then(|content| content.marks().get(mark.index()))
                    .is_none()
                {
                    return Err(RuntimePlanBuildError::ForeignDialogueMarkSeed);
                }
                Ok(LineTaskTrigger::Mark(mark))
            }
        }
    }

    fn lower_line_task_cancel_rule_seed(
        &self,
        seed: RuntimeLineTaskCancelRuleSeed,
    ) -> Result<LineCancelRule, RuntimePlanBuildError> {
        let (content, trigger) = seed
            .trigger
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignDialogueMarkSeed)?;
        if self
            .dialogue_content
            .get(content)
            .and_then(|content| content.marks().get(trigger.index()))
            .is_none()
        {
            return Err(RuntimePlanBuildError::ForeignDialogueMarkSeed);
        }
        Ok(LineCancelRule::new(
            trigger,
            self.lower_flow_ops(seed.action)?.into_boxed_slice(),
        ))
    }

    /// Establishes the only link between one dialogue content plan and its
    /// optional line-task group. A finished group cannot be shared.
    pub fn attach_line_task_group_seed(
        &mut self,
        content: &RuntimeDialogueContentPlanSeedId,
        group: &RuntimeLineTaskGroupSeedId,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_attach_line_task_group_seed(content, group);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_attach_line_task_group_seed(
        &mut self,
        content: &RuntimeDialogueContentPlanSeedId,
        group: &RuntimeLineTaskGroupSeedId,
    ) -> Result<(), RuntimePlanBuildError> {
        let content = content
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignDialogueContentSeed)?;
        let group = group
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignLineTaskGroupSeed)?;
        if self.line_task_groups.get(group.index()).is_none() {
            return Err(RuntimePlanBuildError::ForeignLineTaskGroupSeed);
        }
        if self
            .line_task_group_attachments
            .get(group.index())
            .copied()
            .unwrap_or(false)
        {
            return Err(RuntimePlanBuildError::DuplicateDialogueLineTaskGroup);
        }
        let owners = self
            .line_task_group_mark_owners
            .get(group.index())
            .ok_or(RuntimePlanBuildError::ForeignLineTaskGroupSeed)?;
        if let Some(actual) = owners.iter().copied().find(|owner| *owner != content) {
            return Err(RuntimePlanBuildError::LineTaskMarkContentMismatch {
                expected: content,
                actual,
            });
        }
        let content = self
            .dialogue_content
            .get_mut(content)
            .ok_or(RuntimePlanBuildError::ForeignDialogueContentSeed)?;
        if !content.attach_line_task_group(group) {
            return Err(RuntimePlanBuildError::DuplicateDialogueLineTaskGroup);
        }
        self.line_task_group_attachments[group.index()] = true;
        Ok(())
    }

    fn line_task_mark_owners(
        &self,
        seed: &RuntimeLineTaskGroupSeed,
    ) -> Result<BTreeSet<RuntimeDialogueContentPlanId>, RuntimePlanBuildError> {
        let mut owners = BTreeSet::new();
        self.collect_line_task_node_mark_owners(&seed.root, &mut owners)?;
        for rule in &seed.cancel_rules {
            let (content, _) = rule
                .trigger
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignDialogueMarkSeed)?;
            owners.insert(content);
        }
        Ok(owners)
    }

    fn collect_line_task_node_mark_owners(
        &self,
        node: &RuntimeLineTaskNodeSeed,
        owners: &mut BTreeSet<RuntimeDialogueContentPlanId>,
    ) -> Result<(), RuntimePlanBuildError> {
        match node {
            RuntimeLineTaskNodeSeed::Sequence(children)
            | RuntimeLineTaskNodeSeed::Start(children)
            | RuntimeLineTaskNodeSeed::Parallel { children, .. } => {
                for child in children {
                    self.collect_line_task_node_mark_owners(child, owners)?;
                }
            }
            RuntimeLineTaskNodeSeed::Child { trigger, scope, .. } => {
                if let RuntimeLineTaskTriggerSeed::Mark(mark) = trigger {
                    let (content, _) = mark
                        .resolve(&self.issuer)
                        .ok_or(RuntimePlanBuildError::ForeignDialogueMarkSeed)?;
                    owners.insert(content);
                }
                self.collect_line_task_node_mark_owners(scope, owners)?;
            }
            RuntimeLineTaskNodeSeed::Action(_) => {}
        }
        Ok(())
    }

    pub fn push_pure_helper_seed(
        &mut self,
        seed: RuntimePureHelperSeed,
    ) -> Result<RuntimePureHelperSeedId, RuntimePlanBuildError> {
        let result = seed.body.ty();
        let body = seed.body;
        let helper = self.reserve_pure_helper_seed(RuntimePureHelperDeclarationSeed {
            name: seed.name,
            inputs: seed.inputs,
            input_abi: seed.input_abi,
            result,
            output_abi: seed.output_abi,
            scalar_eval_supported: seed.scalar_eval_supported,
            origin: seed.origin,
        })?;
        self.define_pure_helper_seed(&helper, body)?;
        Ok(helper)
    }

    pub fn reserve_pure_helper_seed(
        &mut self,
        seed: RuntimePureHelperDeclarationSeed,
    ) -> Result<RuntimePureHelperSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_pure_helper_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_pure_helper_seed(
        &mut self,
        seed: RuntimePureHelperDeclarationSeed,
    ) -> Result<RuntimePureHelperSeedId, RuntimePlanBuildError> {
        let inputs = self.resolve_function_locals(seed.inputs)?;
        self.validate_callable_input_abi("pure helper", &inputs, &seed.input_abi)?;
        let input_types = inputs
            .iter()
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let input_locals = inputs
            .into_iter()
            .map(|(local, _)| local)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = self.resolve_seed_type("pure helper result", seed.result)?;
        self.validate_callable_output_abi("pure helper", result, seed.output_abi)?;
        let helper = super::RuntimePureHelperId(self.pure_helpers.len());
        if u32::try_from(self.pure_helpers.len()).is_err() {
            return Err(RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::PureHelpers,
            });
        }
        self.pure_helpers.push(ReservedPureHelper {
            name: seed.name,
            input_locals,
            input_abi: seed.input_abi,
            output_abi: seed.output_abi,
            scalar_eval_supported: seed.scalar_eval_supported,
            origin: seed.origin,
            body: None,
        });
        Ok(RuntimePureHelperSeedId::issued(
            &self.issuer,
            helper,
            input_types,
            result,
        ))
    }

    pub fn define_pure_helper_seed(
        &mut self,
        helper: &RuntimePureHelperSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_define_pure_helper_seed(helper, body);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_define_pure_helper_seed(
        &mut self,
        helper: &RuntimePureHelperSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        let (helper_id, _, result) = helper
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
        let reserved = self
            .pure_helpers
            .get(helper_id.0)
            .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
        if reserved.body.is_some() {
            return Err(RuntimePlanBuildError::DuplicatePureHelperDefinition { helper: helper_id });
        }
        let inputs = reserved.input_locals.clone();
        let body = self.lower_expression(body)?;
        require_reserved_result("pure helper result", result, body.ty())?;
        self.validate_function_body_locals(&body, &inputs, &[])?;
        self.pure_helpers[helper_id.0].body = Some(body);
        Ok(())
    }

    pub fn push_trait_method_seed(
        &mut self,
        seed: RuntimeTraitMethodSeed,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanBuildError> {
        let result = seed.body.ty();
        let body = seed.body;
        let method = self.reserve_trait_method_seed(RuntimeTraitMethodDeclarationSeed {
            identity: seed.identity,
            receiver: seed.receiver,
            inputs: seed.inputs,
            input_abi: seed.input_abi,
            result,
            output_abi: seed.output_abi,
        })?;
        self.define_trait_method_seed(&method, body)?;
        Ok(method)
    }

    pub fn reserve_trait_method_seed(
        &mut self,
        seed: RuntimeTraitMethodDeclarationSeed,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_reserve_trait_method_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_reserve_trait_method_seed(
        &mut self,
        seed: RuntimeTraitMethodDeclarationSeed,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanBuildError> {
        let inputs = self.resolve_function_locals(seed.inputs)?;
        if inputs.is_empty() {
            return Err(RuntimePlanBuildError::MissingTraitMethodReceiver);
        }
        self.validate_callable_input_abi("trait method", &inputs, &seed.input_abi)?;
        let input_types = inputs
            .iter()
            .map(|(_, ty)| *ty)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let input_locals = inputs
            .into_iter()
            .map(|(local, _)| local)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = self.resolve_seed_type("trait method result", seed.result)?;
        self.validate_callable_output_abi("trait method", result, seed.output_abi)?;
        let method = super::RuntimeTraitMethodId(self.trait_methods.len());
        if u32::try_from(self.trait_methods.len()).is_err() {
            return Err(RuntimePlanBuildError::TooManyRows {
                table: RuntimePlanTable::TraitMethods,
            });
        }
        self.trait_methods.push(ReservedTraitMethod {
            identity: seed.identity,
            receiver: seed.receiver,
            input_locals,
            input_abi: seed.input_abi,
            output_abi: seed.output_abi,
            body: None,
        });
        Ok(RuntimeTraitMethodSeedId::issued(
            &self.issuer,
            method,
            seed.receiver,
            input_types,
            result,
        ))
    }

    pub fn define_trait_method_seed(
        &mut self,
        method: &RuntimeTraitMethodSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_define_trait_method_seed(method, body);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_define_trait_method_seed(
        &mut self,
        method: &RuntimeTraitMethodSeedId,
        body: RuntimeExprSeed,
    ) -> Result<(), RuntimePlanBuildError> {
        let (method_id, _, _, result) = method
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        let reserved = self
            .trait_methods
            .get(method_id.0)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        if reserved.body.is_some() {
            return Err(RuntimePlanBuildError::DuplicateTraitMethodDefinition {
                method: method_id,
            });
        }
        let inputs = reserved.input_locals.clone();
        let body = self.lower_expression(body)?;
        require_reserved_result("trait method result", result, body.ty())?;
        self.validate_function_body_locals(&body, &inputs, &[])?;
        self.trait_methods[method_id.0].body = Some(body);
        Ok(())
    }

    pub fn push_entry(&mut self, value: RuntimeEntrySpec) -> Result<u32, RuntimePlanBuildError> {
        push_row(&mut self.entries, value, RuntimePlanTable::Entries)
    }

    pub fn push_callable_executable_seed(
        &mut self,
        seed: RuntimeCallableExecutableSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_callable_executable_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_push_callable_executable_seed(
        &mut self,
        seed: RuntimeCallableExecutableSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        let code = match seed.code {
            RuntimeCallableExecutableSeedCode::PureHelper(helper) => {
                let (helper, _, _) = helper
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
                if self.pure_helpers.get(helper.0).is_none() {
                    return Err(RuntimePlanBuildError::ForeignPureHelperSeed);
                }
                RuntimeCallableExecutableCode::PureHelper(helper)
            }
            RuntimeCallableExecutableSeedCode::ControllerFlow(flow) => {
                RuntimeCallableExecutableCode::ControllerFlow(flow)
            }
        };
        push_row(
            &mut self.callable_executables,
            RuntimeCallableExecutable {
                callable: seed.callable,
                contract: seed.contract,
                code,
            },
            RuntimePlanTable::CallableExecutables,
        )
    }

    pub fn push_flow_executable(
        &mut self,
        value: RuntimeFlowExecutable,
    ) -> Result<u32, RuntimePlanBuildError> {
        push_row(
            &mut self.flow_executables,
            value,
            RuntimePlanTable::FlowExecutables,
        )
    }

    pub fn push_flow_seed(&mut self, seed: RuntimeFlowSeed) -> Result<u32, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_flow_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    /// Admits one complete typed stream transform into the immutable plan.
    pub fn push_stream_plan_seed(
        &mut self,
        seed: RuntimeStreamPlanSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        self.ensure_usable()?;
        let result = self.try_push_stream_plan_seed(seed);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn try_push_stream_plan_seed(
        &mut self,
        seed: RuntimeStreamPlanSeed,
    ) -> Result<u32, RuntimePlanBuildError> {
        if self.stream_plans.iter().any(|plan| plan.id() == &seed.id) {
            return Err(RuntimePlanBuildError::DuplicateStreamDefinition {
                stream: seed.id.canonical_label(),
            });
        }
        let plan = self.lower_stream_plan_seed(seed)?;
        push_row(&mut self.stream_plans, plan, RuntimePlanTable::StreamPlans)
    }

    pub fn finish(self) -> Result<RuntimePlan, RuntimePlanBuildError> {
        self.validate_finish_preconditions()?;
        let mut function_site_builder = RuntimeFunctionSiteTableBuilder::new();
        for site in self.function_sites {
            let Some(body) = site.body else {
                unreachable!("incomplete function sites returned before materialization")
            };
            function_site_builder.push(site.params, site.captures, body)?;
        }
        let pure_helpers = self
            .pure_helpers
            .into_iter()
            .enumerate()
            .map(|(index, helper)| {
                let Some(expr) = helper.body else {
                    unreachable!("incomplete pure helpers returned before materialization")
                };
                RuntimePureHelper {
                    id: super::RuntimePureHelperId(index),
                    name: helper.name,
                    input_locals: helper.input_locals,
                    input_types: helper.input_abi,
                    output_type: helper.output_abi,
                    expr,
                    scalar_eval_supported: helper.scalar_eval_supported,
                    origin: helper.origin,
                }
            })
            .collect();
        let trait_methods = self
            .trait_methods
            .into_iter()
            .enumerate()
            .map(|(index, method)| {
                let Some(body) = method.body else {
                    unreachable!("incomplete trait methods returned before materialization")
                };
                RuntimeTraitMethod {
                    id: super::RuntimeTraitMethodId(index),
                    identity: method.identity,
                    receiver: method.receiver,
                    input_locals: method.input_locals,
                    input_types: method.input_abi,
                    output_type: method.output_abi,
                    body,
                }
            })
            .collect();
        let type_table = self.types.finish();
        let local_declarations = self.locals.finish();
        validate_flow_parameters(
            &self.flows,
            &self.flow_executables,
            &local_declarations,
            &type_table,
        )?;
        let plan = RuntimePlan {
            type_table,
            local_declarations,
            nominal_record_domains: self.nominal_record_domains.finish(),
            variant_domains: self.variant_domains.finish(),
            function_sites: function_site_builder.finish(),
            dialogue_content: self.dialogue_content.finish(),
            entries: self.entries,
            callable_executables: self.callable_executables,
            flow_executables: self.flow_executables,
            flows: self.flows,
            pure_helpers,
            trait_methods,
            line_task_groups: self.line_task_groups,
            stream_plans: self.stream_plans,
        };
        plan.verify()?;
        Ok(plan)
    }

    fn validate_finish_preconditions(&self) -> Result<(), RuntimePlanBuildError> {
        if self.poisoned {
            return Err(RuntimePlanBuildError::Poisoned);
        }
        let incomplete_function_sites = self
            .function_sites
            .iter()
            .filter(|site| site.body.is_none())
            .count();
        let incomplete_pure_helpers = self
            .pure_helpers
            .iter()
            .filter(|helper| helper.body.is_none())
            .count();
        let incomplete_trait_methods = self
            .trait_methods
            .iter()
            .filter(|method| method.body.is_none())
            .count();
        if incomplete_function_sites != 0
            || incomplete_pure_helpers != 0
            || incomplete_trait_methods != 0
        {
            return Err(RuntimePlanBuildError::IncompleteDefinitions {
                function_sites: incomplete_function_sites,
                pure_helpers: incomplete_pure_helpers,
                trait_methods: incomplete_trait_methods,
            });
        }
        if let Some((index, _)) = self
            .line_task_group_attachments
            .iter()
            .enumerate()
            .find(|(_, attached)| !**attached)
        {
            let group = RuntimeLineTaskGroupId::from_zero_based(index).ok_or(
                RuntimePlanBuildError::TooManyRows {
                    table: RuntimePlanTable::LineTaskGroups,
                },
            )?;
            return Err(RuntimePlanBuildError::OrphanLineTaskGroup { group });
        }
        Ok(())
    }

    fn validate_domain_exclusivity(
        &self,
        records: &[RuntimeNominalRecordDomain],
        variants: &[RuntimeVariantDomain],
    ) -> Result<(), RuntimePlanBuildError> {
        let record_owners = records
            .iter()
            .map(RuntimeNominalRecordDomain::owner)
            .collect::<BTreeSet<_>>();
        let variant_owners = variants
            .iter()
            .map(RuntimeVariantDomain::owner)
            .collect::<BTreeSet<_>>();
        if let Some(owner) = record_owners.intersection(&variant_owners).next() {
            return Err(RuntimePlanBuildError::ConflictingNominalDomainKinds { owner: *owner });
        }
        for owner in record_owners {
            if self.variant_domains.contains_owner(owner) {
                return Err(RuntimePlanBuildError::ConflictingNominalDomainKinds { owner });
            }
        }
        for owner in variant_owners {
            if self.nominal_record_domains.contains_owner(owner) {
                return Err(RuntimePlanBuildError::ConflictingNominalDomainKinds { owner });
            }
        }
        Ok(())
    }

    fn try_push_flow_seed(&mut self, seed: RuntimeFlowSeed) -> Result<u32, RuntimePlanBuildError> {
        let (id, params, ops) = seed.into_parts();
        let label = id.canonical_label();
        let mut unique = BTreeSet::new();
        let mut resolved = Vec::with_capacity(params.len());
        for param in params {
            let (local, _) = param
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
            if !self.locals.contains(local) {
                return Err(RuntimePlanBuildError::UnknownFlowParameter { flow: label, local });
            }
            if !unique.insert(local) {
                return Err(RuntimePlanBuildError::DuplicateFlowParameter { flow: label, local });
            }
            resolved.push(local);
        }
        let ops = self.lower_flow_ops(ops)?;
        let mut scope = resolved.iter().copied().collect::<BTreeSet<_>>();
        self.validate_flow_operation_locals(&ops, &mut scope)?;
        push_row(
            &mut self.flows,
            RuntimeFlow {
                id,
                params: resolved.into_boxed_slice(),
                ops,
            },
            RuntimePlanTable::Flows,
        )
    }

    fn resolve_function_locals(
        &self,
        locals: impl IntoIterator<Item = RuntimeLocalSeedId>,
    ) -> Result<Vec<(RuntimeLocalDeclarationId, RuntimePlanTypeId)>, RuntimePlanBuildError> {
        let mut resolved = Vec::new();
        let mut unique = BTreeSet::new();
        for local in locals {
            let (local, ty) = local
                .resolve(&self.issuer)
                .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
            if !self.locals.contains(local) {
                return Err(RuntimePlanBuildError::UnknownFunctionLocal { local });
            }
            if !unique.insert(local) {
                return Err(RuntimePlanBuildError::DuplicateFunctionLocal { local });
            }
            resolved.push((local, ty));
        }
        Ok(resolved)
    }

    fn ensure_usable(&self) -> Result<(), RuntimePlanBuildError> {
        if self.poisoned {
            Err(RuntimePlanBuildError::Poisoned)
        } else {
            Ok(())
        }
    }
}

fn line_task_seed_requests_detach(seed: &RuntimeLineTaskGroupSeed) -> bool {
    matches!(
        seed.cleanup_policy.child_tasks,
        crate::line_task::ChildTaskCleanup::Detach
    ) || line_task_node_requests_detach(&seed.root)
}

fn line_task_node_requests_detach(node: &RuntimeLineTaskNodeSeed) -> bool {
    match node {
        RuntimeLineTaskNodeSeed::Sequence(children)
        | RuntimeLineTaskNodeSeed::Start(children)
        | RuntimeLineTaskNodeSeed::Parallel { children, .. } => {
            children.iter().any(line_task_node_requests_detach)
        }
        RuntimeLineTaskNodeSeed::Child {
            cancel_policy,
            scope,
            ..
        } => {
            *cancel_policy == crate::line_task::ChildCancelPolicy::Detach
                || line_task_node_requests_detach(scope)
        }
        RuntimeLineTaskNodeSeed::Action(_) => false,
    }
}

fn validate_flow_parameters(
    flows: &[RuntimeFlow],
    executables: &[RuntimeFlowExecutable],
    locals: &super::RuntimeLocalDeclarationTable,
    types: &super::RuntimePlanTypeTable,
) -> Result<(), RuntimePlanBuildError> {
    let mut flow_ids = BTreeSet::new();
    for flow in flows {
        let label = flow.id.canonical_label();
        if !flow_ids.insert(flow.id.clone()) {
            return Err(RuntimePlanBuildError::DuplicateFlowDefinition { flow: label });
        }
        let mut unique = BTreeSet::new();
        for &local in &flow.params {
            if !unique.insert(local) {
                return Err(RuntimePlanBuildError::DuplicateFlowParameter { flow: label, local });
            }
            if !locals.contains(local) {
                return Err(RuntimePlanBuildError::UnknownFlowParameter { flow: label, local });
            }
        }
        let mut matching = executables.iter().filter(|row| row.flow == flow.id);
        let Some(executable) = matching.next() else {
            continue;
        };
        if matching.next().is_some() {
            return Err(RuntimePlanBuildError::DuplicateFlowExecutable { flow: label });
        }
        if executable.parameters.len() != flow.params.len() {
            return Err(RuntimePlanBuildError::FlowParameterCount {
                flow: label,
                expected: executable.parameters.len(),
                actual: flow.params.len(),
            });
        }
        let mut parameter_names = BTreeSet::new();
        for (index, (parameter, &local)) in
            executable.parameters.iter().zip(&flow.params).enumerate()
        {
            if parameter.name.is_empty() {
                return Err(RuntimePlanBuildError::EmptyFlowParameterName { flow: label, index });
            }
            if !parameter_names.insert(parameter.name.as_str()) {
                return Err(RuntimePlanBuildError::DuplicateFlowParameterName {
                    flow: label,
                    name: parameter.name.clone(),
                });
            }
            if usize::try_from(parameter.position).ok() != Some(index) {
                return Err(RuntimePlanBuildError::FlowParameterPosition {
                    flow: label,
                    index,
                    actual: parameter.position,
                });
            }
            let local_ty = locals
                .get(local)
                .ok_or(RuntimePlanBuildError::UnknownFlowParameter {
                    flow: label.clone(),
                    local,
                })?
                .ty();
            let matches = types.get(local_ty).is_some_and(|declaration| {
                matches!(
                    declaration.projection(),
                    RuntimePlanTypeProjection::ProjectNominal { nominal, layout, .. }
                        if nominal == &parameter.nominal && layout == &parameter.layout
                )
            });
            if !matches {
                let expected = format!(
                    "nominal {:?} layout {:?}",
                    parameter.nominal, parameter.layout
                );
                let actual = types.get(local_ty).map_or_else(
                    || "missing type declaration".to_owned(),
                    |declaration| format!("{:?}", declaration.projection()),
                );
                return Err(RuntimePlanBuildError::FlowParameterType {
                    flow: label,
                    index,
                    local,
                    expected,
                    actual,
                });
            }
        }
    }
    for executable in executables {
        if !flow_ids.contains(&executable.flow) {
            return Err(RuntimePlanBuildError::MissingFlowDefinition {
                flow: executable.flow.canonical_label(),
            });
        }
    }
    Ok(())
}

impl Default for RuntimePlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_semantic_type(
    types: &PreparedRuntimePlanTypeBatch,
    semantic_identity: RuntimeSemanticTypeId,
) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
    types
        .id_for_semantic(semantic_identity)
        .ok_or(RuntimePlanBuildError::UnknownSemanticType { semantic_identity })
}

fn rewrite_record_domain(
    types: &PreparedRuntimePlanTypeBatch,
    seed: &RuntimeNominalRecordDomainSeed,
) -> Result<RuntimeNominalRecordDomain, RuntimePlanBuildError> {
    let owner = resolve_semantic_type(types, seed.owner())?;
    let owner_declaration = types
        .get(owner)
        .ok_or(RuntimePlanBuildError::UnknownSemanticType {
            semantic_identity: seed.owner(),
        })?;
    if !matches!(
        owner_declaration.projection(),
        RuntimePlanTypeProjection::ProjectNominal { .. }
    ) {
        return Err(RuntimePlanBuildError::InvalidNominalRecordOwner { owner });
    }
    let fields = seed
        .fields()
        .iter()
        .map(|field| {
            Ok((
                field.name().to_owned(),
                resolve_semantic_type(types, field.ty())?,
            ))
        })
        .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
    Ok(RuntimeNominalRecordDomain::from_admitted_parts(
        owner, fields,
    ))
}

fn rewrite_variant_domain(
    types: &PreparedRuntimePlanTypeBatch,
    seed: &RuntimeVariantDomainSeed,
) -> Result<RuntimeVariantDomain, RuntimePlanBuildError> {
    let owner = resolve_semantic_type(types, seed.owner())?;
    let owner_declaration = types
        .get(owner)
        .ok_or(RuntimePlanBuildError::UnknownSemanticType {
            semantic_identity: seed.owner(),
        })?;
    match owner_declaration.projection() {
        RuntimePlanTypeProjection::ProjectNominal { nominal, .. } => {
            if nominal != seed.nominal() {
                return Err(RuntimePlanBuildError::VariantNominalMismatch {
                    owner,
                    expected: nominal.clone(),
                    actual: seed.nominal().clone(),
                });
            }
        }
        RuntimePlanTypeProjection::Opaque { .. } => {}
        _ => return Err(RuntimePlanBuildError::InvalidVariantOwner { owner }),
    }
    let cases = seed
        .cases()
        .iter()
        .map(|case| {
            let payload = case
                .payload()
                .map(|semantic_identity| resolve_semantic_type(types, semantic_identity))
                .transpose()?;
            Ok((case.name().to_owned(), payload))
        })
        .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
    Ok(RuntimeVariantDomain::from_admitted_parts(
        owner,
        seed.nominal().clone(),
        cases,
    ))
}

fn push_row<T>(
    rows: &mut Vec<T>,
    value: T,
    table: RuntimePlanTable,
) -> Result<u32, RuntimePlanBuildError> {
    let index =
        u32::try_from(rows.len()).map_err(|_| RuntimePlanBuildError::TooManyRows { table })?;
    rows.len()
        .checked_add(1)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or(RuntimePlanBuildError::TooManyRows { table })?;
    rows.push(value);
    Ok(index)
}

fn require_reserved_result(
    context: &'static str,
    expected: RuntimePlanTypeId,
    actual: RuntimePlanTypeId,
) -> Result<(), RuntimePlanBuildError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::TypeMismatch {
            context,
            expected,
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::TypeLayoutHash;
    use crate::pattern::RuntimeCheckedType;
    use crate::plan::{
        RuntimeNominalRecordDomainFieldSeed, RuntimePlanTypeProjection,
        RuntimePlanTypeResolutionError, RuntimeVariantCaseSeed,
    };
    use crate::value::RuntimeValue;

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn nominal() -> RuntimeNominalTypeId {
        RuntimeNominalTypeId::try_new("game.State").expect("nominal identity")
    }

    fn type_seeds() -> Vec<RuntimePlanTypeSeed> {
        vec![
            RuntimePlanTypeSeed::new(
                identity(1),
                RuntimePlanTypeProjection::ProjectNominal {
                    nominal: nominal(),
                    layout: TypeLayoutHash::from_bytes([3; 32]),
                    arguments: Box::new([]),
                },
            ),
            RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool),
        ]
    }

    #[test]
    fn record_domain_failure_rolls_back_types_and_locals() {
        let mut builder = RuntimePlanBuilder::new();
        let invalid = RuntimeNominalRecordDomainSeed::new(
            identity(1),
            [
                RuntimeNominalRecordDomainFieldSeed::new("value", identity(2)),
                RuntimeNominalRecordDomainFieldSeed::new("value", identity(2)),
            ],
        );
        assert!(matches!(
            builder.admit_semantic_batch(
                type_seeds(),
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [invalid],
                []
            ),
            Err(RuntimePlanBuildError::NominalRecordDomain(
                RuntimeNominalRecordDomainError::DuplicateFieldName { .. }
            ))
        ));

        let valid = RuntimeNominalRecordDomainSeed::new(
            identity(1),
            [RuntimeNominalRecordDomainFieldSeed::new(
                "value",
                identity(2),
            )],
        );
        let admitted = builder
            .admit_semantic_batch(
                type_seeds(),
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [valid],
                [],
            )
            .expect("failed transaction committed nothing");
        assert_eq!(admitted.local_ids().len(), 1);
        let plan = builder.finish().expect("sealed plan");
        let record_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("record type");
        assert_eq!(plan.type_table().len(), 2);
        assert_eq!(plan.local_declarations().len(), 1);
        assert!(plan.nominal_record_domains().get(record_ty).is_some());
    }

    #[test]
    fn variant_domain_failure_rolls_back_the_type_batch() {
        let mut builder = RuntimePlanBuilder::new();
        let empty = RuntimeVariantDomainSeed::new(identity(1), nominal(), []);
        assert!(matches!(
            builder.admit_semantic_batch(type_seeds(), [], [], [empty]),
            Err(RuntimePlanBuildError::VariantDomain(
                RuntimeVariantDomainError::EmptyDomain { .. }
            ))
        ));

        let valid = RuntimeVariantDomainSeed::new(
            identity(1),
            nominal(),
            [RuntimeVariantCaseSeed::new("Ready", Some(identity(2)))],
        );
        builder
            .admit_semantic_batch(type_seeds(), [], [], [valid])
            .expect("failed transaction committed nothing");
        let plan = builder.finish().expect("sealed plan");
        let variant_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("variant type");
        assert!(plan.variant_domains().get(variant_ty).is_some());
        assert!(matches!(
            plan.checked_type(variant_ty),
            Ok(Some(RuntimeCheckedType::Variant { .. }))
        ));
    }

    #[test]
    fn recursive_variant_domain_is_checked_without_materializing_its_predicate() {
        let mut builder = RuntimePlanBuilder::new();
        let recursive = RuntimeVariantDomainSeed::new(
            identity(1),
            nominal(),
            [RuntimeVariantCaseSeed::new("Next", Some(identity(1)))],
        );
        builder
            .admit_semantic_batch(type_seeds(), [], [], [recursive])
            .expect("recursive nominal domain is structurally valid");
        let plan = builder.finish().expect("sealed plan");
        let recursive_ty = plan
            .type_table()
            .id_for_semantic(identity(1))
            .expect("recursive type");

        assert_eq!(
            plan.type_class(recursive_ty),
            Ok(crate::plan::RuntimePlanTypeClass::Checked)
        );
        assert_eq!(
            plan.checked_type(recursive_ty),
            Err(RuntimePlanTypeResolutionError::CheckedProjectionCycle { ty: recursive_ty })
        );
    }

    #[test]
    fn conflicting_batch_does_not_commit_local_rows() {
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::Bool,
                )],
                [],
                [],
                [],
            )
            .expect("initial bool type");
        assert!(matches!(
            builder.admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::String,
                )],
                [RuntimeLocalDeclarationSeed::new(identity(1))],
                [],
                [],
            ),
            Err(RuntimePlanBuildError::TypeGraph(
                RuntimePlanTypeTableError::ConflictingProjection { .. }
            ))
        ));

        builder
            .admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    identity(2),
                    RuntimePlanTypeProjection::String,
                )],
                [RuntimeLocalDeclarationSeed::new(identity(2))],
                [],
                [],
            )
            .expect("failed batch left no local row");
        let plan = builder.finish().expect("unpoisoned preflight failure");
        assert_eq!(plan.type_table().len(), 2);
        assert_eq!(plan.local_declarations().len(), 1);
    }

    #[test]
    fn cross_builder_local_injection_poisoned_the_target_builder() {
        let mut first = RuntimePlanBuilder::new();
        let foreign = first
            .admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::Bool,
                )],
                [RuntimeLocalDeclarationSeed::new(identity(1))],
                [],
                [],
            )
            .expect("first admission")
            .local_ids()[0]
            .clone();
        let mut second = RuntimePlanBuilder::new();
        second
            .admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::Bool,
                )],
                [RuntimeLocalDeclarationSeed::new(identity(1))],
                [],
                [],
            )
            .expect("second admission");

        assert_eq!(
            second.push_function_site_seed(
                [foreign],
                [],
                RuntimeExprSeed::new(
                    identity(1),
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                ),
            ),
            Err(RuntimePlanBuildError::ForeignLocalSeed)
        );
        assert_eq!(second.finish(), Err(RuntimePlanBuildError::Poisoned));
    }
}
