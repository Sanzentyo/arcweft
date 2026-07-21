use super::{
    BundleViewDiagnosticCode, BundleViewRuntime, BundleViewRuntimeError,
    ViewProgramReplacementError,
};
use crate::dialogue::{
    DialoguePageIndex, DialogueViewInput, DialogueViewOccurrence, DialogueViewPrimaryAction,
    DialogueViewReveal, DialogueViewStage, DialogueViewState,
};
use crate::presentation_handles::PresentationHandleId;
use arcweft_bundle::resource_codec::view::{
    ValidatedViewProduct, ViewDefinitionRef, ViewDefinitionResource, ViewInstructionSpan,
    ViewParameterResource, ViewParameterRole, ViewProductValidationLimits, ViewProgramResource,
};
use arcweft_core::plan::RuntimeLineId;
use arcweft_dialogue::{DialogueProfileRevision, InlineFailurePolicy};
use arcweft_render_text::{
    LineDisplayCatalog, LineDisplayFrame, LineDisplaySpec, RichTextDocument, RuntimeLineContext,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{
    AcceptedViewProgramRevision, DialogueEntryId, DialogueInstanceId, DialoguePresentationId,
    DialogueStageIndex, ViewId, ViewProgramId,
};

fn view_id(value: &str) -> ViewId {
    ViewId::try_new(value).expect("test View ID")
}

fn test_dialogue_revision() -> DialogueProfileRevision {
    let manifest = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-driver-dialogue-acceptance-test").expect("document ID"),
        SourceName::Memory,
        "test manifest",
    )
    .expect("test document");
    let sources =
        SourceSetRevision::try_for_identities([manifest.identity()]).expect("test source revision");
    DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.runtime-driver-dialogue-acceptance-test")
            .expect("View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("View program revision"),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn runtime_with_role(role: ViewParameterRole) -> BundleViewRuntime {
    runtime_with_definitions([("view.Dialogue", role)])
}

fn runtime_with_definitions(
    definitions: impl IntoIterator<Item = (&'static str, ViewParameterRole)>,
) -> BundleViewRuntime {
    let program = ViewProgramResource {
        program_id: ViewProgramId::try_new("view.program.dialogue-acceptance").expect("program ID"),
        definitions: definitions
            .into_iter()
            .map(|(view, role)| ViewDefinitionResource {
                public_id: ViewDefinitionRef::new(view_id(view)),
                body: ViewInstructionSpan::new(0, 0),
                styles: Vec::new(),
                parameters: vec![ViewParameterResource {
                    ordinal: 0,
                    name: "dialogue".to_owned(),
                    role,
                    value_type: None,
                    value_slot: None,
                    default_program: None,
                }],
                state_schema_hash: 1,
            })
            .collect(),
        ..ViewProgramResource::default()
    };
    let product = ValidatedViewProduct::try_new(
        None,
        Some(program),
        None,
        ViewProductValidationLimits::default(),
    )
    .expect("test product validates");
    BundleViewRuntime::try_new(product, None).expect("test runtime accepts")
}

fn display(view: ViewId) -> LineDisplayCatalog {
    LineDisplayCatalog::try_from_lines(
        test_dialogue_revision(),
        vec![LineDisplaySpec {
            line: RuntimeLineId::from_runtime_line_value("say.accepted").expect("line ID"),
            callee: "narrator".to_owned(),
            speaker_label: None,
            text_key: None,
            view,
            profile_style: None,
            dialogue_revision: test_dialogue_revision(),
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            inline_failure: InlineFailurePolicy::FailLine,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(Vec::new()),
        }],
    )
    .expect("test display catalog is revision-consistent")
}

fn display_frame(view: ViewId) -> LineDisplayFrame {
    display(view).lines()[0]
        .resolve_frame(&RuntimeLineContext::default())
        .expect("test display frame resolves")
}

fn dialogue_state() -> DialogueViewState {
    DialogueViewState {
        occurrence: DialogueViewOccurrence {
            presentation: DialoguePresentationId::new(1),
            entry: DialogueEntryId::new(1),
            instance: DialogueInstanceId::new(1),
        },
        stage: DialogueViewStage {
            index: DialogueStageIndex::new(0),
            page: DialoguePageIndex::new(0),
            stage_count: 1,
            page_count: 1,
        },
        reveal: DialogueViewReveal::complete(),
        primary_action: DialogueViewPrimaryAction { target: None },
    }
}

fn rejected_dialogue_frame(
    runtime: &mut BundleViewRuntime,
    view: &ViewId,
    frame: &LineDisplayFrame,
) -> super::BundleViewFrame {
    runtime.evaluate_with_dialogue(
        &[],
        &[DialogueViewInput {
            handle: PresentationHandleId::try_new("dialogue.acceptance").expect("handle ID"),
            view,
            frame,
            state: dialogue_state(),
        }],
        &[],
        false,
    )
}

#[test]
fn accepted_catalog_publishes_only_a_registered_dialogue_owner() {
    let accepted = view_id("view.Dialogue");
    let mut runtime = runtime_with_role(ViewParameterRole::Dialogue);

    runtime
        .accept_dialogue_view_definitions(&display(accepted.clone()))
        .expect("typed dialogue owner accepts");

    assert_eq!(runtime.required_dialogue_views, [accepted].into());
}

#[test]
fn accepted_catalog_rejects_unknown_owner_atomically() {
    let mut runtime = runtime_with_role(ViewParameterRole::Dialogue);
    let declared_before = runtime.declared_dialogue_views.clone();
    let required_before = runtime.required_dialogue_views.clone();
    let unknown = view_id("view.UnknownDialogue");

    assert_eq!(
        runtime.accept_dialogue_view_definitions(&display(unknown.clone())),
        Err(BundleViewRuntimeError::UnknownDialogueViewDefinition {
            definition: unknown,
        })
    );
    assert_eq!(runtime.declared_dialogue_views, declared_before);
    assert_eq!(runtime.required_dialogue_views, required_before);
}

#[test]
fn accepted_catalog_rejects_owner_without_dialogue_role_atomically() {
    let mut runtime = runtime_with_role(ViewParameterRole::Value);
    let declared_before = runtime.declared_dialogue_views.clone();
    let required_before = runtime.required_dialogue_views.clone();
    let owner = view_id("view.Dialogue");

    assert_eq!(
        runtime.accept_dialogue_view_definitions(&display(owner.clone())),
        Err(BundleViewRuntimeError::DialogueViewDefinitionMissingRole { definition: owner })
    );
    assert_eq!(runtime.declared_dialogue_views, declared_before);
    assert_eq!(runtime.required_dialogue_views, required_before);
}

#[test]
fn evaluation_rejects_catalog_valid_but_unauthorized_dialogue_owner_atomically() {
    let owner = view_id("view.Dialogue");
    let frame = display_frame(owner.clone());
    let mut runtime = runtime_with_role(ViewParameterRole::Dialogue);
    let before = runtime.snapshot().expect("runtime snapshots");
    let declared_before = runtime.declared_dialogue_views.clone();
    let required_before = runtime.required_dialogue_views.clone();

    let output = rejected_dialogue_frame(&mut runtime, &owner, &frame);

    assert!(output.mounts.is_empty());
    assert_eq!(
        output.diagnostics[0].code,
        BundleViewDiagnosticCode::InvalidDialogueViewOwner
    );
    assert_eq!(output.diagnostics[0].view.as_deref(), Some("view.Dialogue"));
    assert_eq!(runtime.snapshot().expect("runtime snapshots"), before);
    assert_eq!(runtime.declared_dialogue_views, declared_before);
    assert_eq!(runtime.required_dialogue_views, required_before);
}

#[test]
fn evaluation_rejects_unknown_dialogue_owner_atomically() {
    let unknown = view_id("view.UnknownDialogue");
    let frame = display_frame(unknown.clone());
    let mut runtime = runtime_with_role(ViewParameterRole::Dialogue);
    let before = runtime.snapshot().expect("runtime snapshots");
    let declared_before = runtime.declared_dialogue_views.clone();
    let required_before = runtime.required_dialogue_views.clone();

    let output = rejected_dialogue_frame(&mut runtime, &unknown, &frame);

    assert!(output.mounts.is_empty());
    assert_eq!(
        output.diagnostics[0].code,
        BundleViewDiagnosticCode::InvalidDialogueViewOwner
    );
    assert_eq!(
        output.diagnostics[0].view.as_deref(),
        Some("view.UnknownDialogue")
    );
    assert_eq!(runtime.snapshot().expect("runtime snapshots"), before);
    assert_eq!(runtime.declared_dialogue_views, declared_before);
    assert_eq!(runtime.required_dialogue_views, required_before);
}

#[test]
fn evaluation_rejects_dialogue_owner_without_dialogue_role_atomically() {
    let owner = view_id("view.Dialogue");
    let frame = display_frame(owner.clone());
    let mut runtime = runtime_with_role(ViewParameterRole::Value);
    let before = runtime.snapshot().expect("runtime snapshots");
    let declared_before = runtime.declared_dialogue_views.clone();
    let required_before = runtime.required_dialogue_views.clone();

    let output = rejected_dialogue_frame(&mut runtime, &owner, &frame);

    assert!(output.mounts.is_empty());
    assert_eq!(
        output.diagnostics[0].code,
        BundleViewDiagnosticCode::InvalidDialogueViewOwner
    );
    assert_eq!(output.diagnostics[0].view.as_deref(), Some("view.Dialogue"));
    assert_eq!(runtime.snapshot().expect("runtime snapshots"), before);
    assert_eq!(runtime.declared_dialogue_views, declared_before);
    assert_eq!(runtime.required_dialogue_views, required_before);
}

#[test]
fn live_hot_swap_owner_is_retained_then_pruned_after_its_occurrence_retires() {
    let current = view_id("view.CurrentDialogue");
    let retiring = view_id("view.RetiringDialogue");
    let mut runtime = runtime_with_definitions([
        ("view.CurrentDialogue", ViewParameterRole::Dialogue),
        ("view.RetiringDialogue", ViewParameterRole::Dialogue),
    ]);
    runtime
        .accept_dialogue_view_definitions(&display(current.clone()))
        .expect("current bundle owner accepts");
    let retiring_frame = display_frame(retiring.clone());
    let retiring_input = DialogueViewInput {
        handle: PresentationHandleId::try_new("dialogue.retiring").expect("handle ID"),
        view: &retiring,
        frame: &retiring_frame,
        state: dialogue_state(),
    };

    runtime
        .validate_dialogue_inputs(std::slice::from_ref(&retiring_input))
        .expect("live prior-bundle owner accepts for hot-swap restore");
    assert_eq!(
        runtime.required_dialogue_views,
        [current.clone(), retiring.clone()].into()
    );

    let live = runtime.evaluate_with_dialogue(&[], &[retiring_input], &[], false);
    assert!(live.diagnostics.is_empty());
    assert_eq!(
        runtime.required_dialogue_views,
        [current.clone(), retiring.clone()].into()
    );
    assert_eq!(runtime.transient_dialogue_view_owners(), vec![retiring]);

    let retired = runtime.evaluate_with_dialogue(&[], &[], &[], false);
    assert!(retired.diagnostics.is_empty());
    assert_eq!(runtime.required_dialogue_views, [current.clone()].into());
    assert!(runtime.transient_dialogue_view_owners().is_empty());

    let without_retired =
        runtime_with_definitions([("view.CurrentDialogue", ViewParameterRole::Dialogue)])
            .product()
            .clone();
    let prepared = runtime
        .prepare_view_program_replacement(without_retired)
        .expect("retired dialogue owner can be removed");
    runtime
        .commit_view_program_replacement(prepared)
        .expect("replacement without retired owner commits");

    let without_declared =
        runtime_with_definitions(std::iter::empty::<(&'static str, ViewParameterRole)>())
            .product()
            .clone();
    assert!(matches!(
        runtime.prepare_view_program_replacement(without_declared),
        Err(ViewProgramReplacementError::MissingRequiredDialogueView {
            definition,
        }) if definition == current
    ));
}

#[test]
fn prepared_replacement_is_stale_when_declared_owners_change_independently() {
    let first = view_id("view.FirstDialogue");
    let second = view_id("view.SecondDialogue");
    let mut runtime = runtime_with_definitions([
        ("view.FirstDialogue", ViewParameterRole::Dialogue),
        ("view.SecondDialogue", ViewParameterRole::Dialogue),
    ]);
    runtime
        .accept_dialogue_view_definitions(
            &LineDisplayCatalog::try_from_lines(
                test_dialogue_revision(),
                vec![
                    display(first.clone()).lines()[0].clone(),
                    display(second.clone()).lines()[0].clone(),
                ],
            )
            .expect("test display catalog is revision-consistent"),
        )
        .expect("both declared owners accept");
    let prepared = runtime
        .prepare_view_program_replacement(runtime.product().clone())
        .expect("unchanged candidate prepares against exact state");

    runtime
        .accept_dialogue_view_definitions(&display(first.clone()))
        .expect("first owner becomes the declared baseline");
    let second_frame = display_frame(second.clone());
    runtime
        .validate_dialogue_inputs(&[DialogueViewInput {
            handle: PresentationHandleId::try_new("dialogue.second").expect("handle ID"),
            view: &second,
            frame: &second_frame,
            state: dialogue_state(),
        }])
        .expect("second owner remains transiently active");
    assert_eq!(
        runtime.required_dialogue_views,
        [first.clone(), second].into()
    );
    assert_eq!(runtime.declared_dialogue_views, [first].into());

    assert_eq!(
        runtime.commit_view_program_replacement(prepared),
        Err(ViewProgramReplacementError::StalePreparedState)
    );
}
