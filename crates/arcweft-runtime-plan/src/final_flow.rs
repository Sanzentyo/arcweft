//! Runtime-plan lowering from one accepted final-HIR project generation.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::presentation_name::CharacterPresentationCatalogData;
use arcweft_core::effect::{
    RuntimeArtifactFingerprint, RuntimeAssertionProfile, RuntimeEffectExpr,
};
use arcweft_core::entry::{
    RuntimeCallableExecutable, RuntimeCallableExecutableCode, RuntimeCallableId,
    RuntimeCallableRole, RuntimeFlowExecutable,
};
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeFlow, RuntimeMatchArm,
    RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode, RuntimeTraitMethod,
    RuntimeTraitMethodIdentity,
};
use arcweft_core::value::{
    RuntimeExpr, RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue,
};
use arcweft_lang_hir::expr::{HirExprKind, HirThreadBody, HirThreadFlowItem, HirThreadMode};
use arcweft_lang_hir::identity::{HirModuleId, HirSnapshotId, ItemId, StmtId};
use arcweft_lang_hir::item::{
    HirEntryDeclaration, HirEntryId, HirEntryKind, HirFunctionBody, HirFunctionItem,
    HirFunctionParameterGroup, HirImplFunction, HirImplMember, HirItemKind, HirMethodParameter,
    HirMethodParameterGroup, HirMethodReceiverKind, HirParameterKind,
};
use arcweft_lang_hir::leaf::HirIdRef;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::project::HirExecutableProjectView;
use arcweft_lang_hir::source_index::{
    HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtSourceRole,
};
use arcweft_lang_hir::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirStmtKind,
    HirStmtMatchArmBody,
};
use arcweft_lang_hir::symbol::{
    CallableDeclarationId, CallableDeclarationKey, CallableDeclarationOwner, CallablePackageId,
};
use arcweft_source::SourceSpan;
use arcweft_text_model::DialogueContentCatalog;

use crate::assertion_identity::{
    AssertionConditionIndex, AssertionPresentation, RuntimeAssertionInventory,
    RuntimeAssertionMode, RuntimeAssertionSite,
};
use crate::errors::RuntimePlanLowerError;
use crate::final_expr::FinalExprLowerer;
use crate::final_pattern::FinalPatternLowerer;
use crate::semantic_facts::{
    RuntimeAssertionAdmission, RuntimePlanSemanticFacts, RuntimeResolvedValue,
    RuntimeSemanticFactsError, RuntimeTraitIdentity, RuntimeTraitMethodFact, RuntimeTypeShape,
};

/// Final-HIR owner and checked runtime Entry metadata admitted by semantic analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCheckedEntryInput {
    owner: ItemId,
    entry: RuntimeEntrySpec,
}

impl RuntimeCheckedEntryInput {
    pub const fn new(owner: ItemId, entry: RuntimeEntrySpec) -> Self {
        Self { owner, entry }
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn entry(&self) -> &RuntimeEntrySpec {
        &self.entry
    }
}

/// Runtime body family selected for one checked ordinary-function Entry role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeEntryCallableBody {
    PureHelper,
    ControllerFlow(arcweft_core::plan::FlowRuntimeId),
}

/// Exact callable identity, final-HIR owner, and checked role metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEntryCallableInput {
    declaration: CallableDeclarationKey,
    owner: ItemId,
    role: RuntimeCallableRole,
    body: RuntimeEntryCallableBody,
}

impl RuntimeEntryCallableInput {
    pub const fn new(
        declaration: CallableDeclarationKey,
        owner: ItemId,
        role: RuntimeCallableRole,
        body: RuntimeEntryCallableBody,
    ) -> Self {
        Self {
            declaration,
            owner,
            role,
            body,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn role(&self) -> &RuntimeCallableRole {
        &self.role
    }

    pub const fn body(&self) -> &RuntimeEntryCallableBody {
        &self.body
    }
}

/// Exact final-HIR Flow owner and checked executable role metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEntryFlowInput {
    owner: ItemId,
    executable: RuntimeFlowExecutable,
}

impl RuntimeEntryFlowInput {
    pub const fn new(owner: ItemId, executable: RuntimeFlowExecutable) -> Self {
        Self { owner, executable }
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn executable(&self) -> &RuntimeFlowExecutable {
        &self.executable
    }
}

/// Generation-bound checked Entry projection consumed by final runtime lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEntryLoweringInput {
    snapshots: BTreeMap<HirModuleId, HirSnapshotId>,
    entries: Vec<RuntimeCheckedEntryInput>,
    callables: Vec<RuntimeEntryCallableInput>,
    flows: Vec<RuntimeEntryFlowInput>,
}

impl RuntimeEntryLoweringInput {
    pub fn new(
        project: HirExecutableProjectView<'_>,
        entries: Vec<RuntimeCheckedEntryInput>,
        callables: Vec<RuntimeEntryCallableInput>,
        flows: Vec<RuntimeEntryFlowInput>,
    ) -> Self {
        let snapshots = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.snapshot_id()))
            .collect();
        Self {
            snapshots,
            entries,
            callables,
            flows,
        }
    }

    pub fn empty(project: HirExecutableProjectView<'_>) -> Self {
        Self::new(project, Vec::new(), Vec::new(), Vec::new())
    }

    fn validate_generation(&self, project: HirExecutableProjectView<'_>) -> bool {
        self.snapshots
            == project
                .modules()
                .map(|(_, module)| (module.module_id(), module.snapshot_id()))
                .collect()
    }
}

/// Runtime-plan lowering result plus lowering-time counters.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlanLowerReport {
    pub plan: RuntimePlan,
    pub stats: RuntimePlanLowerStats,
    pub dialogue_content_catalog: DialogueContentCatalog,
    pub character_presentation_catalog: Option<Arc<CharacterPresentationCatalogData>>,
    assertion_sites: Box<[RuntimeAssertionSite]>,
}

impl RuntimePlanLowerReport {
    /// Binds the fresh-session assertion sites to the exact completed
    /// runtime-plan artifact identity.
    ///
    /// The fingerprint is copied from the existing runtime-plan `ArtifactKey`
    /// by the compiler/cache owner after plan construction. It never
    /// participates in guard derivation and the returned inventory is never
    /// serialized.
    ///
    /// # Panics
    ///
    /// Panics only if this already-validated report contains duplicate guard
    /// identities, which would violate its construction invariant.
    pub fn bind_assertion_inventory(
        &self,
        artifact: RuntimeArtifactFingerprint,
    ) -> RuntimeAssertionInventory {
        RuntimeAssertionInventory::try_new(artifact, self.assertion_sites.iter().cloned())
            .expect("runtime-plan lowering validated unique assertion guards")
    }

    /// Number of runtime-capable assertion conditions retained for a fresh
    /// compiler session. Debug assertions omitted by the selected profile and
    /// proof-only assertions are absent.
    pub const fn assertion_site_count(&self) -> usize {
        self.assertion_sites.len()
    }
}

/// Runtime-plan counters retained by compiler and profile output.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimePlanLowerStats {
    pub pure_helpers: usize,
    pub pure_candidate_functions_seen: usize,
    pub pure_candidate_lower_attempts: usize,
    pub pure_candidate_lower_failures_inferred: usize,
    pub pure_expr_lowered_nodes: usize,
    pub pure_expr_cloned_nodes: usize,
    pub pure_rewrite_expr_visits: usize,
    pub optimized_flows: usize,
    pub optimized_op_slices: usize,
    pub local_use_tail_scans: usize,
    pub local_use_scan_ops: usize,
    pub sequence_map_sum_fusions: usize,
    pub map_sum_fusions: usize,
    pub sequence_source_inlines: usize,
    pub pure_call_exprs: usize,
}

/// Lowers one exact accepted HIR generation and its checked semantic facts.
#[allow(
    clippy::too_many_lines,
    reason = "this function is the single transactional authority switch that validates and publishes one complete runtime plan"
)]
pub fn lower_runtime_plan_with_stats(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    entry_input: &RuntimeEntryLoweringInput,
) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
    facts
        .validate_generation(project)
        .map_err(|error| vec![semantic_fact_error(error)])?;
    if !entry_input.validate_generation(project) {
        return Err(vec![RuntimePlanLowerError::new(
            "checked runtime Entry input belongs to a different accepted HIR generation",
        )]);
    }

    let mut flows = Vec::new();
    let mut assertion_sites = Vec::new();
    let mut errors = Vec::new();
    let mut entries = collect_entry_inputs(entry_input, &mut errors);
    for item in project.items() {
        match item.item().kind() {
            HirItemKind::Flow(flow) => {
                let Some(identity) = facts.flow(item.id()).cloned() else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "checked runtime Flow identity is missing for final-HIR item {:?}",
                        item.id()
                    )));
                    continue;
                };
                let mut lowerer = FinalFlowLowerer::new(
                    item.module(),
                    facts,
                    project.package(),
                    RuntimeAssertionOwner::Flow(identity.clone()),
                );
                match lowerer.lower_body(flow.body()) {
                    Ok(ops) => {
                        assertion_sites.extend(lowerer.into_assertion_sites());
                        flows.push(RuntimeFlow { id: identity, ops });
                    }
                    Err(mut item_errors) => errors.append(&mut item_errors),
                }
            }
            HirItemKind::Entry(entry) => match entries.remove(&item.id()) {
                Some(input) => {
                    if let Err(error) = validate_entry_input(entry, input.entry()) {
                        errors.push(error);
                    }
                }
                None => errors.push(RuntimePlanLowerError::new(format!(
                    "final-HIR Entry item {:?} is absent from the checked runtime Entry input",
                    item.id()
                ))),
            },
            HirItemKind::Source(_) => errors.push(RuntimePlanLowerError::new(format!(
                "final-HIR Source item {:?} requires a checked generation-plan projection fact",
                item.id()
            ))),
            HirItemKind::Error(_) => errors.push(RuntimePlanLowerError::new(format!(
                "recovered final-HIR item {:?} cannot enter runtime-plan lowering",
                item.id()
            ))),
            HirItemKind::Module(_)
            | HirItemKind::Use(_)
            | HirItemKind::Function(_)
            | HirItemKind::Predicate(_)
            | HirItemKind::Proof(_)
            | HirItemKind::Trait(_)
            | HirItemKind::Impl(_)
            | HirItemKind::Enum(_)
            | HirItemKind::Struct(_)
            | HirItemKind::TypeAlias(_)
            | HirItemKind::Resource(_)
            | HirItemKind::Character(_)
            | HirItemKind::View(_)
            | HirItemKind::Action(_)
            | HirItemKind::Activity(_)
            | HirItemKind::Signal(_)
            | HirItemKind::Metric(_)
            | HirItemKind::Layer(_)
            | HirItemKind::ExternCapability(_)
            | HirItemKind::Test(_)
            | HirItemKind::Bench(_)
            | HirItemKind::Style(_) => {}
        }
    }

    for owner in entries.keys() {
        errors.push(RuntimePlanLowerError::new(format!(
            "checked runtime Entry input references non-Entry or stale owner {owner:?}"
        )));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let entries = entry_input
        .entries
        .iter()
        .map(|input| input.entry.clone())
        .collect::<Vec<_>>();
    let (
        pure_helpers,
        callable_executables,
        mut controller_flows,
        controller_executables,
        controller_assertion_sites,
    ) = lower_entry_callables(project, facts, entry_input)?;
    assertion_sites.extend(controller_assertion_sites);
    let flow_executables = lower_entry_flows(project, facts, entry_input, &mut errors)?;
    for controller in controller_flows.drain(..) {
        if flows.iter().any(|flow| flow.id == controller.id) {
            errors.push(RuntimePlanLowerError::new(format!(
                "Entry controller flow `{}` conflicts with a final-HIR Flow identity",
                controller.id
            )));
        } else {
            flows.push(controller);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    validate_unique_assertion_guards(&assertion_sites)?;
    let mut all_flow_executables = flow_executables;
    all_flow_executables.extend(controller_executables);
    let trait_methods = lower_trait_methods(project, facts)?;

    let mut dialogue_records = facts
        .dialogue_applications()
        .map(|(_, application)| application.content().clone())
        .collect::<Vec<_>>();
    dialogue_records.sort_by(|left, right| {
        (left.line(), left.text_key()).cmp(&(right.line(), right.text_key()))
    });
    let dialogue_content_catalog = DialogueContentCatalog::try_from_records(dialogue_records)
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let line_task_groups = if dialogue_content_catalog.records().is_empty() {
        Vec::new()
    } else {
        vec![LineTaskGroup::default()]
    };
    let plan = RuntimePlan::new(flows, line_task_groups)
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?
        .with_entries(entries)
        .with_pure_helpers(pure_helpers.clone())
        .with_trait_methods(trait_methods)
        .with_entry_executables(callable_executables, all_flow_executables);
    Ok(RuntimePlanLowerReport {
        plan,
        stats: RuntimePlanLowerStats {
            pure_helpers: pure_helpers.len(),
            pure_candidate_functions_seen: pure_helpers.len(),
            pure_candidate_lower_attempts: pure_helpers.len(),
            ..RuntimePlanLowerStats::default()
        },
        dialogue_content_catalog,
        character_presentation_catalog: facts.character_presentation_catalog().cloned(),
        assertion_sites: assertion_sites.into_boxed_slice(),
    })
}

fn lower_trait_methods(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
) -> Result<Vec<RuntimeTraitMethod>, Vec<RuntimePlanLowerError>> {
    facts
        .trait_methods()
        .map(|method| lower_trait_method(project, facts, method))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| vec![error])
}

fn lower_trait_method(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    checked: &RuntimeTraitMethodFact,
) -> Result<RuntimeTraitMethod, RuntimePlanLowerError> {
    let module = project
        .modules()
        .find_map(|(_, module)| {
            (module.module_id() == checked.implementation().module()).then_some(module)
        })
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "runtime trait method {:?} belongs to no accepted HIR module",
                checked.id()
            ))
        })?;
    let item = module
        .resolve_item(checked.implementation())
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    let HirItemKind::Impl(implementation) = item.kind() else {
        return Err(RuntimePlanLowerError::new(
            "checked runtime trait method owner is not an Impl item",
        ));
    };
    let Some(HirImplMember::Function(function)) =
        implementation.members().get(usize::from(checked.member()))
    else {
        return Err(RuntimePlanLowerError::new(
            "checked runtime trait method member is not a function",
        ));
    };
    if !function.generic_parameters().is_empty() {
        return Err(RuntimePlanLowerError::new(
            "runtime trait method cannot retain unbound generic parameters",
        ));
    }
    let method_name = function
        .name()
        .resolved()
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method has no resolved name"))?;
    let signature = lower_trait_method_signature(module, facts, function)?;
    let body = function
        .body()
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method has no body"))?;
    let body = FinalExprLowerer::new(module, facts)
        .lower_function_body(body)
        .map_err(RuntimePlanLowerError::new)?;
    let impl_id = project
        .items()
        .position(|item| item.id() == checked.implementation())
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait Impl owner is absent"))?;
    let (trait_id, trait_name) = lower_runtime_trait_identity(project, checked.trait_identity())?;
    Ok(RuntimeTraitMethod {
        id: checked.id(),
        identity: RuntimeTraitMethodIdentity {
            impl_id,
            trait_id,
            witness: Some(checked.id().0),
            trait_name,
            self_type: checked.self_type().to_owned(),
            method_name: method_name.as_str().to_owned(),
            monomorph_label: format!("{}::{}", checked.self_type(), method_name.as_str()),
        },
        receiver: signature.receiver,
        input_names: signature.input_names,
        input_types: signature.input_types,
        output_type: signature.output_type,
        body,
    })
}

struct LoweredTraitMethodSignature {
    receiver: RuntimeReceiverMode,
    input_names: Vec<String>,
    input_types: Vec<RuntimePureInputType>,
    output_type: RuntimePureOutputType,
}

fn lower_trait_method_signature(
    module: &HirModule,
    facts: &RuntimePlanSemanticFacts,
    function: &HirImplFunction,
) -> Result<LoweredTraitMethodSignature, RuntimePlanLowerError> {
    let mut receiver = None;
    let mut input_names = Vec::new();
    let mut input_types = Vec::new();
    for parameter in function
        .parameter_groups()
        .iter()
        .flat_map(HirMethodParameterGroup::parameters)
    {
        match parameter {
            HirMethodParameter::Receiver(parameter) => {
                if receiver.is_some() {
                    return Err(RuntimePlanLowerError::new(
                        "runtime trait method has more than one receiver",
                    ));
                }
                receiver = Some(match parameter.kind() {
                    HirMethodReceiverKind::Owned => RuntimeReceiverMode::Owned,
                    HirMethodReceiverKind::SharedReference => RuntimeReceiverMode::SharedRef,
                    HirMethodReceiverKind::MutableReference => RuntimeReceiverMode::MutRef,
                });
                let local = module
                    .resolve_local(parameter.locals()[0])
                    .map_err(|error| {
                        RuntimePlanLowerError::new(format!(
                            "runtime trait receiver local is stale: {error}"
                        ))
                    })?;
                input_names.push(local.name().as_str().to_owned());
                input_types.push(RuntimePureInputType::Value);
            }
            HirMethodParameter::Typed(parameter) => {
                if parameter.kind() != HirParameterKind::Fixed
                    || parameter.default().is_some()
                    || parameter.locals().len() != 1
                {
                    return Err(RuntimePlanLowerError::new(
                        "runtime trait method requires fixed, non-defaulted, single-binding parameters",
                    ));
                }
                let local = module
                    .resolve_local(parameter.locals()[0])
                    .map_err(|error| {
                        RuntimePlanLowerError::new(format!(
                            "runtime trait parameter local is stale: {error}"
                        ))
                    })?;
                let ty = facts.ty(parameter.ty()).ok_or_else(|| {
                    RuntimePlanLowerError::new(
                        "runtime trait parameter is missing its checked type fact",
                    )
                })?;
                input_names.push(local.name().as_str().to_owned());
                input_types.push(runtime_input_type(ty.shape()));
            }
        }
    }
    let receiver = receiver.ok_or_else(|| {
        RuntimePlanLowerError::new("runtime trait method requires one typed receiver")
    })?;
    let output_type = function
        .return_type()
        .map(|owner| {
            facts
                .ty(owner)
                .map(|ty| runtime_output_type(ty.shape()))
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(
                        "runtime trait method is missing its checked return type fact",
                    )
                })
        })
        .transpose()?
        .unwrap_or(RuntimePureOutputType::Value);
    Ok(LoweredTraitMethodSignature {
        receiver,
        input_names,
        input_types,
        output_type,
    })
}

fn lower_runtime_trait_identity(
    project: HirExecutableProjectView<'_>,
    identity: &RuntimeTraitIdentity,
) -> Result<(Option<usize>, Option<String>), RuntimePlanLowerError> {
    Ok(match identity {
        RuntimeTraitIdentity::Project(owner) => {
            let (position, trait_owner) = project
                .items()
                .enumerate()
                .find(|(_, item)| item.id() == *owner)
                .ok_or_else(|| RuntimePlanLowerError::new("runtime Trait owner is absent"))?;
            let trait_item = trait_owner
                .module()
                .resolve_item(*owner)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let HirItemKind::Trait(trait_item) = trait_item.kind() else {
                return Err(RuntimePlanLowerError::new(
                    "runtime Trait identity does not own a Trait item",
                ));
            };
            let name = trait_item
                .name()
                .resolved()
                .ok_or_else(|| RuntimePlanLowerError::new("runtime Trait has no resolved name"))?;
            (Some(position), Some(name.as_str().to_owned()))
        }
        RuntimeTraitIdentity::StandardIterator => (None, Some("Iterator".to_owned())),
        RuntimeTraitIdentity::StandardIntoIterator => (None, Some("IntoIterator".to_owned())),
    })
}

fn collect_entry_inputs<'input>(
    input: &'input RuntimeEntryLoweringInput,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> BTreeMap<ItemId, &'input RuntimeCheckedEntryInput> {
    let mut entries = BTreeMap::new();
    for entry in &input.entries {
        if entries.insert(entry.owner(), entry).is_some() {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked runtime Entry input repeats final-HIR owner {:?}",
                entry.owner()
            )));
        }
    }
    entries
}

fn validate_entry_input(
    hir: &HirEntryDeclaration,
    runtime: &RuntimeEntrySpec,
) -> Result<(), RuntimePlanLowerError> {
    let expected_kind = match hir.kind() {
        HirEntryKind::Game => RuntimeEntryKind::Game,
        HirEntryKind::Editor => RuntimeEntryKind::Editor,
        HirEntryKind::Cli => RuntimeEntryKind::Cli,
        HirEntryKind::Server => RuntimeEntryKind::Server,
        HirEntryKind::Activity => RuntimeEntryKind::Activity,
        HirEntryKind::Test => RuntimeEntryKind::Test,
        HirEntryKind::Bench => RuntimeEntryKind::Bench,
        HirEntryKind::Agent => RuntimeEntryKind::Agent,
        HirEntryKind::Custom(name) => RuntimeEntryKind::Custom(name.as_str().to_owned()),
        HirEntryKind::Recovered(_) => {
            return Err(RuntimePlanLowerError::new(
                "recovered final-HIR Entry kind cannot enter runtime lowering",
            ));
        }
    };
    if expected_kind != runtime.kind {
        return Err(RuntimePlanLowerError::new(format!(
            "checked runtime Entry `{}` has kind `{}`, but its final-HIR owner has kind `{}`",
            runtime.id,
            runtime.kind.as_str(),
            expected_kind.as_str()
        )));
    }
    let HirEntryId::Authored { value, .. } = hir.id() else {
        return Err(RuntimePlanLowerError::new(
            "final-HIR Entry owner has no authored semantic identity",
        ));
    };
    let Some(HirIdRef::Absolute(source_id)) = value.as_resolved() else {
        return Err(RuntimePlanLowerError::new(
            "final-HIR Entry owner does not retain one absolute semantic identity",
        ));
    };
    if source_id.as_str() != runtime.id.public_label().as_str() {
        return Err(RuntimePlanLowerError::new(format!(
            "checked runtime Entry identity `{}` does not match final-HIR owner `{}`",
            runtime.id,
            source_id.as_str()
        )));
    }
    Ok(())
}

type LoweredEntryCallables = (
    Vec<RuntimePureHelper>,
    Vec<RuntimeCallableExecutable>,
    Vec<RuntimeFlow>,
    Vec<RuntimeFlowExecutable>,
    Vec<RuntimeAssertionSite>,
);

#[allow(
    clippy::too_many_lines,
    reason = "entry callable admission is one closed transaction across function roles, signatures, helpers, and controller bodies"
)]
fn lower_entry_callables(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    input: &RuntimeEntryLoweringInput,
) -> Result<LoweredEntryCallables, Vec<RuntimePlanLowerError>> {
    let mut by_callable = BTreeMap::<RuntimeCallableId, &RuntimeEntryCallableInput>::new();
    let mut errors = Vec::new();
    for callable in &input.callables {
        let identity = callable.role().callable.clone();
        if let Some(previous) = by_callable.get(&identity) {
            if *previous != callable {
                errors.push(RuntimePlanLowerError::new(format!(
                    "checked Entry callable `{}` has conflicting final-HIR owners or body roles",
                    identity.as_str()
                )));
            }
        } else {
            by_callable.insert(identity, callable);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut pure_helpers = Vec::new();
    let mut executables = Vec::new();
    let mut controller_flows = Vec::new();
    let mut controller_executables = Vec::new();
    let mut assertion_sites = Vec::new();
    for callable in by_callable.into_values() {
        let Some(item) = project.items().find(|item| item.id() == callable.owner()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry callable `{}` references a foreign or stale final-HIR owner {:?}",
                callable.role().callable.as_str(),
                callable.owner()
            )));
            continue;
        };
        let HirItemKind::Function(function) = item.item().kind() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry callable `{}` owner {:?} is not an ordinary function",
                callable.role().callable.as_str(),
                callable.owner()
            )));
            continue;
        };
        if let Err(error) =
            validate_callable_owner(project.package(), item.module_path(), function, callable)
        {
            errors.push(error);
            continue;
        }
        match callable.body() {
            RuntimeEntryCallableBody::PureHelper => {
                let helper = match lower_pure_helper(
                    item.module(),
                    facts,
                    function,
                    RuntimePureHelperId(pure_helpers.len()),
                    callable.role().callable.as_str(),
                ) {
                    Ok(helper) => helper,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                executables.push(RuntimeCallableExecutable {
                    callable: callable.role().callable.clone(),
                    contract: callable.role().contract,
                    code: RuntimeCallableExecutableCode::PureHelper(helper.id),
                });
                pure_helpers.push(helper);
            }
            RuntimeEntryCallableBody::ControllerFlow(flow) => {
                let CallableDeclarationKey::Existing(declaration) = callable.declaration() else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "Entry callable `{}` does not use an ordinary declaration identity",
                        callable.role().callable.as_str()
                    )));
                    continue;
                };
                let lowered = match lower_controller_body(
                    item.module(),
                    facts,
                    function,
                    project.package(),
                    declaration.clone(),
                ) {
                    Ok(lowered) => lowered,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                assertion_sites.extend(lowered.assertion_sites);
                executables.push(RuntimeCallableExecutable {
                    callable: callable.role().callable.clone(),
                    contract: callable.role().contract,
                    code: RuntimeCallableExecutableCode::ControllerFlow(flow.clone()),
                });
                controller_flows.push(RuntimeFlow {
                    id: flow.clone(),
                    ops: lowered.ops,
                });
                controller_executables.push(RuntimeFlowExecutable {
                    flow: flow.clone(),
                    contract: arcweft_core::entry::FlowContractHash::from_bytes(
                        *callable.role().contract.as_bytes(),
                    ),
                    parameters: Vec::new(),
                    controller: Some(callable.role().clone()),
                });
            }
        }
    }
    if errors.is_empty() {
        Ok((
            pure_helpers,
            executables,
            controller_flows,
            controller_executables,
            assertion_sites,
        ))
    } else {
        Err(errors)
    }
}

fn validate_callable_owner(
    package: &CallablePackageId,
    module_path: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
    function: &HirFunctionItem,
    input: &RuntimeEntryCallableInput,
) -> Result<(), RuntimePlanLowerError> {
    let CallableDeclarationKey::Existing(declaration) = input.declaration() else {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry callable `{}` does not use an ordinary declaration identity",
            input.role().callable.as_str()
        )));
    };
    let Some(name) = function.name().resolved() else {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry callable `{}` has a recovered final-HIR function name",
            input.role().callable.as_str()
        )));
    };
    if declaration.owner() != CallableDeclarationOwner::Function
        || declaration.package() != package
        || declaration.module() != module_path
        || declaration.name() != name.as_str()
        || RuntimeCallableId::try_new(declaration.to_string()).as_ref()
            != Ok(&input.role().callable)
    {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry callable `{}` identity does not match its exact final-HIR function owner",
            input.role().callable.as_str()
        )));
    }
    Ok(())
}

fn lower_pure_helper(
    module: &HirModule,
    facts: &RuntimePlanSemanticFacts,
    function: &HirFunctionItem,
    id: RuntimePureHelperId,
    name: &str,
) -> Result<RuntimePureHelper, RuntimePlanLowerError> {
    if !function.generic_parameters().is_empty() {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry pure helper `{name}` cannot retain unbound generic parameters"
        )));
    }
    let mut input_names = Vec::new();
    let mut input_types = Vec::new();
    for parameter in function
        .parameter_groups()
        .iter()
        .flat_map(HirFunctionParameterGroup::parameters)
    {
        if parameter.kind() != HirParameterKind::Fixed
            || parameter.default().is_some()
            || parameter.locals().len() != 1
        {
            return Err(RuntimePlanLowerError::new(format!(
                "Entry pure helper `{name}` requires fixed, non-defaulted, single-binding parameters"
            )));
        }
        let local = module
            .resolve_local(parameter.locals()[0])
            .map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "Entry pure helper `{name}` parameter owner is stale: {error}"
                ))
            })?;
        let ty = facts.ty(parameter.ty()).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Entry pure helper `{name}` is missing a checked parameter type fact"
            ))
        })?;
        input_names.push(local.name().as_str().to_owned());
        input_types.push(runtime_input_type(ty.shape()));
    }
    let output_type = function
        .return_type()
        .map(|ty| {
            facts
                .ty(ty)
                .map(|ty| runtime_output_type(ty.shape()))
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "Entry pure helper `{name}` is missing a checked return type fact"
                    ))
                })
        })
        .transpose()?
        .unwrap_or(RuntimePureOutputType::Value);
    let expression = FinalExprLowerer::new(module, facts)
        .lower_function_body(function.body())
        .map_err(RuntimePlanLowerError::new)?;
    Ok(RuntimePureHelper {
        id,
        name: name.to_owned(),
        input_names,
        input_types,
        output_type,
        expr: expression,
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Inferred,
    })
}

struct LoweredControllerBody {
    ops: Vec<FlowOp>,
    assertion_sites: Vec<RuntimeAssertionSite>,
}

fn lower_controller_body(
    module: &HirModule,
    facts: &RuntimePlanSemanticFacts,
    function: &HirFunctionItem,
    package: &CallablePackageId,
    declaration: CallableDeclarationId,
) -> Result<LoweredControllerBody, RuntimePlanLowerError> {
    if function
        .parameter_groups()
        .iter()
        .any(|group| !group.parameters().is_empty())
    {
        return Err(RuntimePlanLowerError::new(
            "Agent controller ordinary function must not accept parameters",
        ));
    }
    let HirFunctionBody::Block {
        statements, tail, ..
    } = function.body()
    else {
        return Err(RuntimePlanLowerError::new(
            "recovered Agent controller body cannot enter runtime lowering",
        ));
    };
    let mut lowerer = FinalFlowLowerer::new(
        module,
        facts,
        package,
        RuntimeAssertionOwner::Callable(declaration),
    );
    let mut ops = lowerer.lower_statement_ids(statements)?;
    ops.push(FlowOp::ReturnExpr(
        FinalExprLowerer::new(module, facts)
            .lower(*tail)
            .map_err(RuntimePlanLowerError::new)?,
    ));
    Ok(LoweredControllerBody {
        ops,
        assertion_sites: lowerer.into_assertion_sites(),
    })
}

fn lower_entry_flows(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    input: &RuntimeEntryLoweringInput,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Result<Vec<RuntimeFlowExecutable>, Vec<RuntimePlanLowerError>> {
    let mut by_runtime = BTreeMap::new();
    let mut owners = BTreeSet::new();
    for flow in &input.flows {
        if !owners.insert(flow.owner()) {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry Flow input repeats final-HIR owner {:?}",
                flow.owner()
            )));
            continue;
        }
        let Some(item) = project.items().find(|item| item.id() == flow.owner()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry Flow input references a foreign or stale owner {:?}",
                flow.owner()
            )));
            continue;
        };
        if !matches!(item.item().kind(), HirItemKind::Flow(_))
            || facts.flow(flow.owner()) != Some(&flow.executable().flow)
        {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry Flow executable `{}` does not match its exact final-HIR Flow owner",
                flow.executable().flow
            )));
            continue;
        }
        match by_runtime.insert(flow.executable().flow.clone(), flow.executable().clone()) {
            Some(previous) if previous != *flow.executable() => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "checked Entry Flow executable `{}` has conflicting role metadata",
                    flow.executable().flow
                )));
            }
            _ => {}
        }
    }
    if errors.is_empty() {
        Ok(by_runtime.into_values().collect())
    } else {
        Err(std::mem::take(errors))
    }
}

const fn runtime_input_type(shape: &RuntimeTypeShape) -> RuntimePureInputType {
    match shape {
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I8) => RuntimePureInputType::I8,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I16) => RuntimePureInputType::I16,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I32) => RuntimePureInputType::I32,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64) => RuntimePureInputType::I64,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I128) => RuntimePureInputType::I128,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::ISize) => RuntimePureInputType::ISize,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U8) => RuntimePureInputType::U8,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U16) => RuntimePureInputType::U16,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U32) => RuntimePureInputType::U32,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U64) => RuntimePureInputType::U64,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U128) => RuntimePureInputType::U128,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::USize) => RuntimePureInputType::USize,
        RuntimeTypeShape::F32 => RuntimePureInputType::F32,
        RuntimeTypeShape::F64 => RuntimePureInputType::F64,
        _ => RuntimePureInputType::Value,
    }
}

const fn runtime_output_type(shape: &RuntimeTypeShape) -> RuntimePureOutputType {
    match shape {
        RuntimeTypeShape::Bool => RuntimePureOutputType::Bool,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I8) => RuntimePureOutputType::I8,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I16) => RuntimePureOutputType::I16,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I32) => RuntimePureOutputType::I32,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64) => RuntimePureOutputType::I64,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I128) => RuntimePureOutputType::I128,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::ISize) => RuntimePureOutputType::ISize,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U8) => RuntimePureOutputType::U8,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U16) => RuntimePureOutputType::U16,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U32) => RuntimePureOutputType::U32,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U64) => RuntimePureOutputType::U64,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U128) => RuntimePureOutputType::U128,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::USize) => RuntimePureOutputType::USize,
        RuntimeTypeShape::F32 => RuntimePureOutputType::F32,
        RuntimeTypeShape::F64 => RuntimePureOutputType::F64,
        _ => RuntimePureOutputType::Value,
    }
}

fn semantic_fact_error(error: RuntimeSemanticFactsError) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(format!(
        "runtime semantic facts do not match the accepted HIR generation: {error}"
    ))
}

fn validate_unique_assertion_guards(
    sites: &[RuntimeAssertionSite],
) -> Result<(), Vec<RuntimePlanLowerError>> {
    let mut guards = BTreeSet::new();
    let mut errors = Vec::new();
    for site in sites {
        if !guards.insert(site.guard()) {
            errors.push(RuntimePlanLowerError::new(format!(
                "runtime assertion guard collision for {:?}",
                site.guard()
            )));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

enum RuntimeAssertionOwner {
    Callable(CallableDeclarationId),
    Flow(FlowRuntimeId),
}

impl RuntimeAssertionOwner {
    fn label(&self) -> String {
        match self {
            Self::Callable(declaration) => declaration.qualified_name(),
            Self::Flow(flow) => flow.canonical_label(),
        }
    }
}

struct FinalFlowLowerer<'hir> {
    module: &'hir HirModule,
    facts: &'hir RuntimePlanSemanticFacts,
    package: &'hir CallablePackageId,
    assertion_owner: RuntimeAssertionOwner,
    assertion_ordinal: u32,
    assertion_sites: Vec<RuntimeAssertionSite>,
}

impl<'hir> FinalFlowLowerer<'hir> {
    fn new(
        module: &'hir HirModule,
        facts: &'hir RuntimePlanSemanticFacts,
        package: &'hir CallablePackageId,
        assertion_owner: RuntimeAssertionOwner,
    ) -> Self {
        Self {
            module,
            facts,
            package,
            assertion_owner,
            assertion_ordinal: 0,
            assertion_sites: Vec::new(),
        }
    }

    fn into_assertion_sites(self) -> Vec<RuntimeAssertionSite> {
        self.assertion_sites
    }

    fn lower_body(
        &mut self,
        body: &HirThreadBody,
    ) -> Result<Vec<FlowOp>, Vec<RuntimePlanLowerError>> {
        let mut ops = Vec::new();
        let mut errors = Vec::new();
        for item in body.items() {
            match self.lower_thread_item(item) {
                Ok(mut item_ops) => ops.append(&mut item_ops),
                Err(error) => errors.push(error),
            }
        }
        if errors.is_empty() {
            Ok(ops)
        } else {
            Err(errors)
        }
    }

    fn lower_thread_item(
        &mut self,
        item: &HirThreadFlowItem,
    ) -> Result<Vec<FlowOp>, RuntimePlanLowerError> {
        let statement = match item {
            HirThreadFlowItem::DialogueApplication(expression) => {
                let application =
                    self.facts
                        .dialogue_application(*expression)
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "dialogue application {expression:?} has no checked projection fact"
                            ))
                        })?;
                return Ok(vec![FlowOp::Dialogue {
                    line: application.content().line().clone(),
                    task_group: 0,
                }]);
            }
            HirThreadFlowItem::Statement(statement)
            | HirThreadFlowItem::Choice(statement)
            | HirThreadFlowItem::If(statement)
            | HirThreadFlowItem::IfLet(statement)
            | HirThreadFlowItem::Match(statement)
            | HirThreadFlowItem::Loop(statement)
            | HirThreadFlowItem::While(statement)
            | HirThreadFlowItem::WhileLet(statement)
            | HirThreadFlowItem::For(statement)
            | HirThreadFlowItem::Select(statement)
            | HirThreadFlowItem::SourceLocale(statement)
            | HirThreadFlowItem::Scope(statement)
            | HirThreadFlowItem::Include(statement)
            | HirThreadFlowItem::AwaitWith(statement)
            | HirThreadFlowItem::Error(statement) => *statement,
        };
        let kind = self.resolve_statement(statement)?.kind().clone();
        if !thread_item_matches_kind(item, &kind) {
            return Err(RuntimePlanLowerError::new(format!(
                "final-HIR thread item family does not match statement {statement:?} payload {kind:?}"
            )));
        }
        self.lower_statement(statement, &kind)
    }

    fn resolve_statement(
        &self,
        statement: StmtId,
    ) -> Result<&arcweft_lang_hir::stmt::HirStmt, RuntimePlanLowerError> {
        self.module.resolve_stmt(statement).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve final-HIR flow statement {statement:?}: {error}"
            ))
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the final-HIR statement match is intentionally exhaustive so unsupported execution families cannot fall through to a second reader"
    )]
    fn lower_statement(
        &mut self,
        id: StmtId,
        kind: &HirStmtKind,
    ) -> Result<Vec<FlowOp>, RuntimePlanLowerError> {
        let expr = FinalExprLowerer::new(self.module, self.facts);
        let pattern = FinalPatternLowerer::new(self.module, self.facts);
        match kind {
            HirStmtKind::Assertion { mode, conditions } => {
                self.lower_assertion(id, *mode, conditions)
            }
            HirStmtKind::Let {
                pattern: owner,
                initializer,
                ..
            } => Ok(vec![FlowOp::Let {
                pattern: pattern.lower(*owner).map_err(RuntimePlanLowerError::new)?,
                expr: expr
                    .lower(*initializer)
                    .map_err(RuntimePlanLowerError::new)?,
            }]),
            HirStmtKind::Assign { target, value } => Ok(vec![FlowOp::Let {
                pattern: RuntimePattern::Discard,
                expr: expr
                    .lower_assignment(*target, *value, RuntimeExpr::Value(RuntimeValue::Unit))
                    .map_err(RuntimePlanLowerError::new)?,
            }]),
            HirStmtKind::LetElse {
                pattern: owner,
                initializer,
                else_body,
                ..
            } => Ok(vec![FlowOp::LetElse {
                pattern: pattern.lower(*owner).map_err(RuntimePlanLowerError::new)?,
                expr: expr
                    .lower(*initializer)
                    .map_err(RuntimePlanLowerError::new)?,
                else_ops: self.lower_statement_ids(else_body)?,
            }]),
            HirStmtKind::Return { value } => Ok(vec![FlowOp::ReturnExpr(
                expr.lower(*value).map_err(RuntimePlanLowerError::new)?,
            )]),
            HirStmtKind::Goto { target } => {
                if let Some(RuntimeResolvedValue::ProjectItem(item)) = self.facts.value(*target)
                    && let Some(target) = item.flow_runtime_id()
                {
                    Ok(vec![FlowOp::Goto(target.clone())])
                } else {
                    Ok(vec![FlowOp::GotoExpr(
                        expr.lower(*target).map_err(RuntimePlanLowerError::new)?,
                    )])
                }
            }
            HirStmtKind::Expression { expression: thread } => {
                let thread_expr = self.module.resolve_expr(*thread).map_err(|error| {
                    RuntimePlanLowerError::new(format!(
                        "cannot resolve final-HIR Thread expression {thread:?}: {error}"
                    ))
                })?;
                let HirExprKind::Thread(thread_expr) = thread_expr.kind() else {
                    return Err(RuntimePlanLowerError::new(format!(
                        "expression statement {id:?} references non-Thread expression {thread:?} without a checked effect disposition"
                    )));
                };
                if thread_expr.mode() == HirThreadMode::Detached {
                    return Err(RuntimePlanLowerError::new(format!(
                        "detached Thread expression {thread:?} requires typed runtime ownership metadata"
                    )));
                }
                Ok(vec![FlowOp::Thread {
                    name: thread_expr.name().map(|name| name.as_str().to_owned()),
                    body: self.lower_body_as_one_error(thread_expr.body())?,
                }])
            }
            HirStmtKind::If(branch) => Ok(vec![FlowOp::If {
                condition: expr
                    .lower(branch.condition())
                    .map_err(RuntimePlanLowerError::new)?,
                then_ops: self.lower_contextual_body(branch.then_body())?,
                else_ops: branch
                    .else_branch()
                    .map(|branch| self.lower_else_branch(branch))
                    .transpose()?
                    .unwrap_or_default(),
            }]),
            HirStmtKind::IfLet(branch) => Ok(vec![FlowOp::IfLet {
                pattern: pattern
                    .lower(branch.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                expr: expr
                    .lower(branch.scrutinee())
                    .map_err(RuntimePlanLowerError::new)?,
                guard: branch
                    .guard()
                    .map(|guard| expr.lower(guard).map_err(RuntimePlanLowerError::new))
                    .transpose()?,
                then_ops: self.lower_contextual_body(branch.then_body())?,
                else_ops: branch
                    .else_branch()
                    .map(|branch| self.lower_else_branch(branch))
                    .transpose()?
                    .unwrap_or_default(),
            }]),
            HirStmtKind::Match(matched) => {
                let mut arms = Vec::with_capacity(matched.arms().len());
                for arm in matched.arms() {
                    let ops = match arm.body() {
                        HirStmtMatchArmBody::Body(body) => self.lower_contextual_body(body)?,
                        HirStmtMatchArmBody::Expression(expression) => {
                            return Err(RuntimePlanLowerError::new(format!(
                                "flow match expression arm {expression:?} requires an explicit effect/value disposition"
                            )));
                        }
                    };
                    arms.push(RuntimeMatchArm {
                        pattern: pattern
                            .lower(arm.pattern())
                            .map_err(RuntimePlanLowerError::new)?,
                        guard: arm
                            .guard()
                            .map(|guard| expr.lower(guard).map_err(RuntimePlanLowerError::new))
                            .transpose()?,
                        ops,
                    });
                }
                Ok(vec![FlowOp::Match {
                    scrutinee: expr
                        .lower(matched.scrutinee())
                        .map_err(RuntimePlanLowerError::new)?,
                    arms,
                }])
            }
            HirStmtKind::Loop(loop_stmt) => {
                if loop_stmt.label().is_some() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "labeled loop {id:?} requires a typed runtime control-label identity"
                    )));
                }
                Ok(vec![FlowOp::Loop {
                    body: self.lower_contextual_body(loop_stmt.body())?,
                }])
            }
            HirStmtKind::While(while_stmt) => Ok(vec![FlowOp::While {
                condition: expr
                    .lower(while_stmt.condition())
                    .map_err(RuntimePlanLowerError::new)?,
                body: self.lower_contextual_body(while_stmt.body())?,
            }]),
            HirStmtKind::WhileLet(while_stmt) => Ok(vec![FlowOp::WhileLet {
                pattern: pattern
                    .lower(while_stmt.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                expr: expr
                    .lower(while_stmt.scrutinee())
                    .map_err(RuntimePlanLowerError::new)?,
                guard: while_stmt
                    .guard()
                    .map(|guard| expr.lower(guard).map_err(RuntimePlanLowerError::new))
                    .transpose()?,
                body: self.lower_contextual_body(while_stmt.body())?,
            }]),
            HirStmtKind::For(for_stmt) => Ok(vec![FlowOp::For {
                pattern: pattern
                    .lower(for_stmt.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                source: expr
                    .lower(for_stmt.source())
                    .map_err(RuntimePlanLowerError::new)?,
                evidence: self.facts.iteration(id).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "checked iteration evidence is missing for For statement {id:?}"
                    ))
                })?,
                body: self.lower_contextual_body(for_stmt.body())?,
            }]),
            HirStmtKind::Scope(scope) => {
                if scope.name().is_some() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "named Scope {id:?} requires a typed runtime scope identity"
                    )));
                }
                Ok(vec![FlowOp::Scope(
                    self.lower_contextual_body(scope.body())?,
                )])
            }
            HirStmtKind::Break { label, value } if label.is_none() => Ok(vec![FlowOp::Break(
                value
                    .map(|value| expr.lower(value).map_err(RuntimePlanLowerError::new))
                    .transpose()?,
            )]),
            HirStmtKind::Continue { label } if label.is_none() => Ok(vec![FlowOp::Continue]),
            HirStmtKind::Error => Err(RuntimePlanLowerError::new(format!(
                "recovered statement {id:?} cannot enter runtime-plan lowering"
            ))),
            unsupported => Err(RuntimePlanLowerError::new(format!(
                "final-HIR statement {id:?} family {unsupported:?} has no checked core projection"
            ))),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "assertion admission, ordered condition projection, guard derivation, and site publication form one identity-preserving transaction"
    )]
    fn lower_assertion(
        &mut self,
        statement: StmtId,
        hir_mode: HirAssertionMode,
        conditions: &[arcweft_lang_hir::identity::ExprId],
    ) -> Result<Vec<FlowOp>, RuntimePlanLowerError> {
        let ordinal = self.assertion_ordinal;
        self.assertion_ordinal = self.assertion_ordinal.checked_add(1).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "runtime assertion ordinal overflow in declaration {}",
                self.assertion_owner.label()
            ))
        })?;

        let source_mode = hir_mode.resolved().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "recovered assertion mode for statement {statement:?} cannot enter runtime lowering"
            ))
        })?;
        let admission = self.facts.assertion(statement).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "checked assertion admission is missing for statement {statement:?}"
            ))
        })?;

        let runtime_mode = match admission {
            RuntimeAssertionAdmission::Discharged => {
                if source_mode != arcweft_lang_syntax::assertion::AssertionMode::Prove {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime assertion {statement:?} is discharged despite source mode {source_mode:?}"
                    )));
                }
                return Ok(Vec::new());
            }
            RuntimeAssertionAdmission::OmittedDebug => {
                if source_mode != arcweft_lang_syntax::assertion::AssertionMode::Debug {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime assertion {statement:?} is omitted despite source mode {source_mode:?}"
                    )));
                }
                return Ok(Vec::new());
            }
            RuntimeAssertionAdmission::Runtime(mode) => mode,
        };
        let source_runtime_mode = RuntimeAssertionMode::try_from_assertion_mode(source_mode)
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        if source_runtime_mode != runtime_mode {
            return Err(RuntimePlanLowerError::new(format!(
                "checked runtime assertion mode {runtime_mode:?} does not match source mode {source_mode:?} for {statement:?}"
            )));
        }
        if !(1..=64).contains(&conditions.len()) {
            return Err(RuntimePlanLowerError::new(format!(
                "runtime assertion {statement:?} has invalid condition count {}",
                conditions.len()
            )));
        }

        let profile = match runtime_mode {
            RuntimeAssertionMode::Check => RuntimeAssertionProfile::Always,
            RuntimeAssertionMode::Debug => RuntimeAssertionProfile::DebugOnly,
        };
        let statement_span = self.source_span(
            &HirSourceQuery::Stmt {
                owner: statement,
                role: HirStmtSourceRole::Whole,
            },
            "assertion statement",
        )?;
        let mut ops = Vec::with_capacity(conditions.len());
        let mut sites = Vec::with_capacity(conditions.len());
        for (index, condition) in conditions.iter().copied().enumerate() {
            let condition_index = AssertionConditionIndex::try_new(index, conditions.len())
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let condition_span = self.source_span(
                &HirSourceQuery::Expr {
                    owner: condition,
                    role: HirExprSourceRole::Whole,
                },
                "assertion condition",
            )?;
            let range = condition_span.range();
            let condition_label = self
                .module
                .provenance()
                .document()
                .text()
                .get(range.start()..range.end())
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "assertion condition {condition:?} source span is outside its accepted document"
                    ))
                })?;
            let guard = match &self.assertion_owner {
                RuntimeAssertionOwner::Callable(declaration) => {
                    crate::assertion_lower::derive_runtime_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        declaration,
                        ordinal,
                        condition_index,
                        profile,
                    )
                }
                RuntimeAssertionOwner::Flow(flow) => {
                    crate::assertion_lower::derive_runtime_flow_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        flow,
                        ordinal,
                        condition_index,
                        profile,
                    )
                }
            };
            let condition_expr = FinalExprLowerer::new(self.module, self.facts)
                .lower(condition)
                .map_err(RuntimePlanLowerError::new)?;
            let mode_label = match runtime_mode {
                RuntimeAssertionMode::Check => "check",
                RuntimeAssertionMode::Debug => "debug",
            };
            let message = RuntimeExpr::Value(RuntimeValue::String(format!(
                "assert.{mode_label} condition {index} failed"
            )));
            ops.push(FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
                guard,
                condition: condition_expr,
                message,
                profile,
            }));
            sites.push(RuntimeAssertionSite::new(
                guard,
                statement,
                condition_index,
                runtime_mode,
                condition_span,
                AssertionPresentation::new(
                    statement_span.clone(),
                    Arc::<str>::from(condition_label),
                ),
            ));
        }
        self.assertion_sites.extend(sites);
        Ok(ops)
    }

    fn source_span(
        &self,
        query: &HirSourceQuery,
        role: &str,
    ) -> Result<SourceSpan, RuntimePlanLowerError> {
        let lookup = self
            .module
            .source_site(self.module.provenance().source_identity(), query.clone())
            .map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "cannot resolve exact final-HIR {role} source for {query:?}: {error}"
                ))
            })?;
        match lookup.presence() {
            HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
            HirSourcePresence::Present(HirSourceSite::Insertion(_))
            | HirSourcePresence::AbsentOptional => Err(RuntimePlanLowerError::new(format!(
                "executable final-HIR {role} for {query:?} has no authored source span"
            ))),
        }
    }

    fn lower_statement_ids(
        &mut self,
        statements: &[StmtId],
    ) -> Result<Vec<FlowOp>, RuntimePlanLowerError> {
        let mut ops = Vec::new();
        for statement in statements {
            let kind = self.resolve_statement(*statement)?.kind().clone();
            ops.extend(self.lower_statement(*statement, &kind)?);
        }
        Ok(ops)
    }

    fn lower_contextual_body(
        &mut self,
        body: &HirContextualStmtBody,
    ) -> Result<Vec<FlowOp>, RuntimePlanLowerError> {
        match body {
            HirContextualStmtBody::Ordinary { statements, .. } => {
                self.lower_statement_ids(statements)
            }
            HirContextualStmtBody::Thread(body) => self.lower_body_as_one_error(body),
        }
    }

    fn lower_else_branch(
        &mut self,
        branch: &HirConditionalElseBranch,
    ) -> Result<Vec<FlowOp>, RuntimePlanLowerError> {
        match branch {
            HirConditionalElseBranch::Body(body) => self.lower_contextual_body(body),
            HirConditionalElseBranch::ElseIf(statement) => {
                let kind = self.resolve_statement(*statement)?.kind().clone();
                self.lower_statement(*statement, &kind)
            }
        }
    }

    fn lower_body_as_one_error(
        &mut self,
        body: &HirThreadBody,
    ) -> Result<Vec<FlowOp>, RuntimePlanLowerError> {
        self.lower_body(body).map_err(|errors| {
            RuntimePlanLowerError::new(
                errors
                    .into_iter()
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })
    }
}

fn thread_item_matches_kind(item: &HirThreadFlowItem, kind: &HirStmtKind) -> bool {
    match item {
        HirThreadFlowItem::DialogueApplication(_) => false,
        HirThreadFlowItem::Statement(_) => !matches!(
            kind,
            HirStmtKind::Choice { .. }
                | HirStmtKind::If(_)
                | HirStmtKind::IfLet(_)
                | HirStmtKind::Match(_)
                | HirStmtKind::Loop(_)
                | HirStmtKind::While(_)
                | HirStmtKind::WhileLet(_)
                | HirStmtKind::For(_)
                | HirStmtKind::Select(_)
                | HirStmtKind::SourceLocale(_)
                | HirStmtKind::Scope(_)
                | HirStmtKind::Include(_)
                | HirStmtKind::AwaitWith(_)
                | HirStmtKind::Error
        ),
        HirThreadFlowItem::Choice(_) => matches!(kind, HirStmtKind::Choice { .. }),
        HirThreadFlowItem::If(_) => matches!(kind, HirStmtKind::If(_)),
        HirThreadFlowItem::IfLet(_) => matches!(kind, HirStmtKind::IfLet(_)),
        HirThreadFlowItem::Match(_) => matches!(kind, HirStmtKind::Match(_)),
        HirThreadFlowItem::Loop(_) => matches!(kind, HirStmtKind::Loop(_)),
        HirThreadFlowItem::While(_) => matches!(kind, HirStmtKind::While(_)),
        HirThreadFlowItem::WhileLet(_) => matches!(kind, HirStmtKind::WhileLet(_)),
        HirThreadFlowItem::For(_) => matches!(kind, HirStmtKind::For(_)),
        HirThreadFlowItem::Select(_) => matches!(kind, HirStmtKind::Select(_)),
        HirThreadFlowItem::SourceLocale(_) => matches!(kind, HirStmtKind::SourceLocale(_)),
        HirThreadFlowItem::Scope(_) => matches!(kind, HirStmtKind::Scope(_)),
        HirThreadFlowItem::Include(_) => matches!(kind, HirStmtKind::Include(_)),
        HirThreadFlowItem::AwaitWith(_) => matches!(kind, HirStmtKind::AwaitWith(_)),
        HirThreadFlowItem::Error(_) => matches!(kind, HirStmtKind::Error),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arcweft_core::{
        entry::{EntryBindingIdentity, RuntimeEntryRoles},
        plan::{
            EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec,
            RuntimeEntryTarget,
        },
    };
    use arcweft_lang_hir::database::HirDatabase;
    use arcweft_lang_hir::item::HirItemKind;
    use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
    use arcweft_lang_hir::project::{HirProject, HirProjectBuilder, HirProjectModule};
    use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
    use arcweft_lang_hir::symbol::{
        CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId,
    };
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
    use arcweft_lang_syntax::incremental::SyntaxDatabase;
    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{
        RuntimeCheckedEntryInput, RuntimeEntryLoweringInput, lower_runtime_plan_with_stats,
    };
    use crate::semantic_facts::{RuntimePlanSemanticFactInput, RuntimePlanSemanticFacts};

    #[test]
    fn empty_flow_lowers_only_with_its_checked_core_identity() {
        let project = project_fixture("empty-flow", "flow opening {}\n");
        let executable = project.executable_view().expect("executable fixture");
        let owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
        let mut input = RuntimePlanSemanticFactInput::new();
        input.push_flow(owner, identity.clone());
        let facts = RuntimePlanSemanticFacts::try_new(executable, input).expect("checked facts");
        let entry_input = RuntimeEntryLoweringInput::empty(executable);
        let report = lower_runtime_plan_with_stats(executable, &facts, &entry_input)
            .expect("empty Flow lowers");
        assert_eq!(report.plan.flows.len(), 1);
        assert_eq!(report.plan.flows[0].id, identity);
        assert!(report.plan.flows[0].ops.is_empty());
    }

    #[test]
    fn thread_expression_statement_lowers_through_the_sole_expression_owner() {
        let project = project_fixture(
            "thread-expression-statement",
            "flow opening {\n    thread {\n    }\n}\n",
        );
        let executable = project.executable_view().expect("executable fixture");
        let owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
        let mut input = RuntimePlanSemanticFactInput::new();
        input.push_flow(owner, identity);
        let facts = RuntimePlanSemanticFacts::try_new(executable, input).expect("checked facts");
        let report = lower_runtime_plan_with_stats(
            executable,
            &facts,
            &RuntimeEntryLoweringInput::empty(executable),
        )
        .expect("Thread expression statement lowers");

        let [FlowOp::Thread { name, body }] = report.plan.flows[0].ops.as_slice() else {
            panic!("ordinary expression statement must project its typed Thread payload")
        };
        assert!(name.is_none());
        assert!(body.is_empty());
    }

    #[test]
    fn final_entry_requires_and_consumes_its_exact_checked_hir_owner() {
        let project = project_fixture(
            "checked-entry-owner",
            "flow @flow.main main {}\nentry cli @entry.cli.main { goto @flow.main }\n",
        );
        let executable = project.executable_view().expect("executable fixture");
        let flow_owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let entry_owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Entry(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Entry item");
        let flow =
            FlowRuntimeId::from_source_entity_body("flow.main").expect("runtime Flow identity");
        let mut fact_input = RuntimePlanSemanticFactInput::new();
        fact_input.push_flow(flow_owner, flow.clone());
        let facts =
            RuntimePlanSemanticFacts::try_new(executable, fact_input).expect("checked facts");

        let missing = RuntimeEntryLoweringInput::empty(executable);
        let errors = lower_runtime_plan_with_stats(executable, &facts, &missing)
            .expect_err("an Entry cannot be silently omitted from the checked input");
        assert!(errors.iter().any(|error| {
            error
                .to_string()
                .contains("absent from the checked runtime Entry input")
        }));

        let runtime_entry = RuntimeEntrySpec {
            id: EntryRuntimeId::from_source_entity_body("entry.cli.main")
                .expect("runtime Entry identity"),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([7; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        };
        let input = RuntimeEntryLoweringInput::new(
            executable,
            vec![RuntimeCheckedEntryInput::new(entry_owner, runtime_entry)],
            Vec::new(),
            Vec::new(),
        );
        let report = lower_runtime_plan_with_stats(executable, &facts, &input)
            .expect("exact checked Entry owner lowers");
        assert_eq!(report.plan.entries.len(), 1);
        assert_eq!(report.plan.flows.len(), 1);
    }

    fn project_fixture(label: &str, source: &str) -> HirProject {
        let package = CallablePackageId::try_new(format!("runtime-plan-final-flow-{label}"))
            .expect("fixture package");
        let path = CanonicalModulePath::crate_root();
        let source_name = SourceName::path(format!("runtime-plan-final-flow-{label}.arcw"));
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("arcweft-test://runtime-plan/flow/{label}"))
                    .expect("fixture document ID"),
                source_name.clone(),
                source,
            )
            .expect("fixture document"),
        );
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(source_name),
                document,
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .expect("attached fixture parse");
        let key = HirModuleKey::new(
            package.clone(),
            path.clone(),
            parsed.document().identity().clone(),
        );
        let mut database = HirDatabase::try_new().expect("HIR database");
        let world = ProjectSymbolWorldId::try_new(
            package.clone(),
            parsed.document().identity().id().clone(),
            "runtime-plan-final-flow-test",
        )
        .expect("fixture symbol world");
        let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
            .expect("fixture symbol revision");
        let transaction = database
            .stage_proof_return_project(
                [LoweringRequest::try_new(key, &parsed).expect("lower request")],
                world,
                revision,
                [parsed.document().identity()],
                arcweft_lang_hir::lowering::HirLoweringControl::new(),
            )
            .expect("final HIR project stages");
        let facts = HirProofReturnSemanticFactSet::try_new(
            Arc::clone(transaction.generation()),
            transaction.headers().cloned(),
            [],
        )
        .expect("runtime-plan fixture has no authored Proof return headers");
        let mut outputs = transaction
            .publish_with_semantic_facts(&mut database, facts)
            .expect("final HIR project publishes");
        let module = outputs
            .pop()
            .expect("one runtime-plan fixture module")
            .into_module();
        assert!(outputs.is_empty());
        let project_module = HirProjectModule::try_new(
            &database,
            &package,
            &path,
            parsed.document().identity(),
            module,
        )
        .expect("accepted module lease");
        let mut builder = HirProjectBuilder::new(&database, package);
        builder
            .insert_module(project_module)
            .expect("module insertion");
        builder.finish().expect("fixture project")
    }
}
