//! Consuming completion of one original runtime-plan construction.

use super::{
    RuntimeAssertionSite, RuntimePlanLowerError, RuntimePlanLowerReport, RuntimePlanLowerStats,
    RuntimePlanSemanticFacts,
};
use arcweft_core::plan::{RuntimePlanBuilder, RuntimeTaskPlanSealLimits};
use arcweft_core::task::ViewTaskPlanAuthority;
use arcweft_text_model::{DialogueContentCatalog, DialogueContentFragmentTemplate};
use std::sync::Arc;

/// A genuine original View selector and the coordinate issued for its real function.
pub struct RuntimeViewMatchTaskPlanSource {
    program: arcweft_id::runtime_program::RuntimePureProgramId,
    coordinate: arcweft_core::plan::RuntimeTaskPlanBuildCoordinate,
    catalog: Arc<arcweft_lang_sema::final_analysis::CompilerLocalViewMatchCatalogRow>,
}

impl RuntimeViewMatchTaskPlanSource {
    pub const fn program(&self) -> arcweft_id::runtime_program::RuntimePureProgramId {
        self.program
    }
    pub const fn coordinate(&self) -> &arcweft_core::plan::RuntimeTaskPlanBuildCoordinate {
        &self.coordinate
    }
    pub fn catalog(&self) -> &arcweft_lang_sema::final_analysis::CompilerLocalViewMatchCatalogRow {
        &self.catalog
    }
}

pub(super) fn reserve_view_task_plan(
    builder: &mut RuntimePlanBuilder,
    program: &crate::semantic_facts::RuntimePureProgramFact,
    function: arcweft_core::plan::RuntimeFunctionSiteSeedId,
) -> Result<Option<RuntimeViewMatchTaskPlanSource>, arcweft_core::plan::RuntimePlanBuildError> {
    use arcweft_core::plan::{
        RuntimeControlEffectCancellation, RuntimeControlEffectCardinality,
        RuntimeControlEffectContractSeed, RuntimeControlEffectIdentity, RuntimeControlEffectKind,
        RuntimeControlEffectOrdering, RuntimeControlEffectRow,
        RuntimeControlEffectTerminalBehavior, RuntimeTaskControlMode,
        RuntimeTaskPlanFunctionRootSeed, RuntimeTaskSemanticBinding,
    };
    let Some(catalog) = program.view_match_catalog() else {
        return Ok(None);
    };
    // One subscription invocation can return immediately or wait for its local
    // inputs. It produces one selected value; cancellation has no value payload.
    let control = builder.push_control_effect_contract(RuntimeControlEffectContractSeed {
        mode: RuntimeTaskControlMode::MaySuspend,
        effects: Box::new([RuntimeControlEffectRow {
            kind: RuntimeControlEffectKind::ViewSubscription,
            identity: Some(RuntimeControlEffectIdentity::from_checked_digest(
                *catalog.admission().as_bytes(),
            )),
            inputs: program
                .input_types()
                .iter()
                .map(crate::semantic_facts::RuntimeNormalizedType::identity)
                .collect(),
            output: Some(program.result().identity()),
            cardinality: RuntimeControlEffectCardinality::ExactlyOnce,
            ordering: RuntimeControlEffectOrdering::SingleTerminal,
            cancellation: RuntimeControlEffectCancellation::ObservedNoPayload,
            terminal: RuntimeControlEffectTerminalBehavior::Value,
        }]),
        children: Box::new([]),
    })?;
    let coordinate = builder.push_task_plan_function_root(RuntimeTaskPlanFunctionRootSeed::new(
        function,
        arcweft_core::task::NeedProducerFamily::ViewMatchSubscription,
        arcweft_core::task::TaskClass::LocalView,
        control,
        RuntimeTaskSemanticBinding::View,
    ))?;
    Ok(Some(RuntimeViewMatchTaskPlanSource {
        program: program.program(),
        coordinate,
        catalog: Arc::clone(catalog),
    }))
}

/// The original construction remains unsealed until its upper authority is ready.
#[must_use = "finish the original construction to produce a RuntimePlan"]
pub struct PreparedRuntimePlanLowering<'facts> {
    builder: RuntimePlanBuilder,
    facts: &'facts RuntimePlanSemanticFacts,
    plain_text_context_template: Option<DialogueContentFragmentTemplate>,
    assertion_sites: Vec<RuntimeAssertionSite>,
    view_task_plans: Box<[RuntimeViewMatchTaskPlanSource]>,
}

impl<'facts> PreparedRuntimePlanLowering<'facts> {
    pub(super) fn new(
        builder: RuntimePlanBuilder,
        facts: &'facts RuntimePlanSemanticFacts,
        plain_text_context_template: Option<DialogueContentFragmentTemplate>,
        assertion_sites: Vec<RuntimeAssertionSite>,
        view_task_plans: Box<[RuntimeViewMatchTaskPlanSource]>,
    ) -> Self {
        Self {
            builder,
            facts,
            plain_text_context_template,
            assertion_sites,
            view_task_plans,
        }
    }

    pub fn view_task_plans(&self) -> &[RuntimeViewMatchTaskPlanSource] {
        &self.view_task_plans
    }

    /// Completes the sole Core seal, then the existing report-owned catalogs.
    pub fn finish(
        self,
        limits: RuntimeTaskPlanSealLimits,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
        let Self {
            builder,
            facts,
            plain_text_context_template,
            assertion_sites,
            view_task_plans: _,
        } = self;
        let mut plan = builder
            .finish_with_task_authority(limits, authority)
            .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
        let mut dialogue_records = Vec::new();
        facts.visit_dialogue_applications(&mut |_, _, application| {
            dialogue_records.push(application.content().clone());
        });
        dialogue_records.sort_by(|left, right| {
            (left.key(), left.text_key()).cmp(&(right.key(), right.text_key()))
        });
        let mut dialogue_templates = Vec::new();
        facts.visit_dialogue_content_fragments(&mut |_, fragment| {
            dialogue_templates.push(fragment.template().clone());
        });
        dialogue_templates.extend(facts.format_templates().map(|fact| fact.template().clone()));
        if let Some(template) = plain_text_context_template {
            dialogue_templates.push(template);
        }
        dialogue_templates.sort_by_key(DialogueContentFragmentTemplate::id);
        let dialogue_content_catalog = DialogueContentCatalog::try_from_records_and_templates(
            dialogue_records,
            dialogue_templates,
        )
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
        if let Some(identity) = plan.dialogue_content().plain_text_context_template() {
            let path = format!("dialogue.template.{}", identity.id());
            let canonical = DialogueContentFragmentTemplate::plain_text_context(identity.id())
                .map_err(|error| {
                    vec![RuntimePlanLowerError::new(format!(
                        "plain-text context template factory failed: {error}"
                    ))]
                })?;
            let Some(manifest) = plan.dialogue_content().template(identity.id()) else {
                return Err(vec![RuntimePlanLowerError::new(format!(
                    "{path} context pointer has no RuntimePlan manifest"
                ))]);
            };
            let Some(template) = dialogue_content_catalog.find_template(identity.id()) else {
                return Err(vec![RuntimePlanLowerError::new(format!(
                    "{path} context pointer has no text-model template"
                ))]);
            };
            if manifest.digest() != identity.digest()
                || template.digest() != identity.digest()
                || template != &canonical
            {
                return Err(vec![RuntimePlanLowerError::new(format!(
                    "{path} context template disagrees with its canonical text-model identity or body"
                ))]);
            }
            let proof = arcweft_core::value::RuntimeDialoguePlainTextContextTemplateProof::
            try_from_validated_ref(identity, canonical.digest())
            .map_err(|error| {
                vec![RuntimePlanLowerError::new(format!(
                    "{path} context proof is invalid: {error}"
                ))]
            })?;
            plan.accept_plain_text_context_template_proof(proof)
                .map_err(|error| {
                    vec![RuntimePlanLowerError::new(format!(
                        "{path} context proof cannot be attached to the RuntimePlan: {error}"
                    ))]
                })?;
        }
        let stats = RuntimePlanLowerStats::from_plan(&plan);
        let character_dialogue_generation = facts
            .character_dialogue_generation()
            .map(|declaration| {
                declaration
                    .try_map_type_refs(|ty| Ok::<_, std::convert::Infallible>(ty.identity()))
                    .map(Arc::new)
                    .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
            })
            .transpose()?;
        Ok(RuntimePlanLowerReport {
            plan,
            stats,
            dialogue_content_catalog,
            character_presentation_catalog: facts.character_presentation_catalog().cloned(),
            character_dialogue_generation,
            assertion_sites: assertion_sites.into_boxed_slice(),
        })
    }
}
