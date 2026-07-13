use super::*;
use arcweft_agent_protocol::action::{AgentActionDispatch, AgentActionKind};
use arcweft_bundle::resource_codec::view::{
    CompositionOnBlurPolicy, EnterKeyHint, TextAssistPolicy, TextCapitalization, ViewInputKind,
    ViewInputPurpose, ViewSecureInputPolicy, ViewTextSelectionPolicy, ViewTextShortcutPolicy,
    ViewTextTabPolicy, ViewTextVerticalNavigationPolicy,
};
use arcweft_bundle::resource_codec::{
    ViewRuntimeActionButton, ViewRuntimeActionButtonAction, ViewRuntimeButtonBounds,
    ViewRuntimeControlVisualStyle, ViewRuntimeTextControl, ViewRuntimeTextControlBounds,
    ViewRuntimeTextControlHandlers, ViewRuntimeTextControlOptions, ViewRuntimeTextSelection,
};
use arcweft_bundle::{
    BundleImageObjectBounds, BundleImageObjectFit, BundleImageObjectPlayback,
    BundleImageObjectTransform,
};
use arcweft_id::PublicId;
use arcweft_presentation::image::{ImageObjectAlignment, ImageObjectTransform};
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticTree};
use arcweft_render_wgpu::geometry::RenderImageFrame;

#[test]
fn player_semantic_objects_preserve_runtime_view_parent() {
    let presentation = BundlePresentationSnapshot {
        text_inputs: vec![runtime_text_control("input.visitor_name")],
        action_buttons: vec![runtime_action_button("button.continue")],
        ..BundlePresentationSnapshot::default()
    };
    let view_by_target = player_runtime_view_by_target(&presentation);
    let input_target = interaction_target("input.visitor_name");
    let input_node = SemanticNode::new(
        layer_id("view.text_input"),
        input_target.clone(),
        SemanticRole::TextField,
        HitRect::new(48.0, 48.0, 420.0, 48.0),
    );
    let render_control = RenderTextInputControl::new(
        input_target,
        arcweft_presentation::text_input::TextInputSessionId(41),
        "Ada",
        arcweft_presentation::text_input::TextRange::new(
            arcweft_presentation::text_input::TextByteOffset(3),
            arcweft_presentation::text_input::TextByteOffset(3),
        ),
        arcweft_presentation::text_input::TextInputOptions::default(),
        SemanticRole::TextField,
        HitRect::new(48.0, 48.0, 420.0, 48.0),
    );
    let button_node = SemanticNode::new(
        layer_id("view.button"),
        interaction_target("button.continue"),
        SemanticRole::Button,
        HitRect::new(484.0, 48.0, 180.0, 48.0),
    );

    let input = player_semantic_object(7, &input_node, Some(&render_control), &view_by_target)
        .expect("input object");
    let button =
        player_semantic_object(7, &button_node, None, &view_by_target).expect("button object");

    assert_eq!(input.parent_id.as_deref(), Some("view.ModernFeedbackPanel"));
    assert_eq!(
        button.parent_id.as_deref(),
        Some("view.ModernFeedbackPanel")
    );
}

#[test]
fn player_semantic_actions_become_agent_action_targets() {
    let mut semantics = SemanticTree::default();
    semantics.push(
        SemanticNode::new(
            layer_id("view.button"),
            interaction_target("button.continue"),
            SemanticRole::Button,
            HitRect::new(484.0, 48.0, 180.0, 48.0),
        )
        .with_action(PublicId::try_new("action.feedback.submit_name").unwrap()),
    );

    let actions = agent_action_targets_for_semantics(&semantics);

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id, "action.feedback.submit_name");
    assert_eq!(actions[0].target, "button.continue");
    assert_eq!(actions[0].action, AgentActionKind::Invoke);
    assert_eq!(actions[0].kind, AgentActionDispatch::Semantic);
    assert!(actions[0].enabled);
}

#[test]
fn missing_requested_capture_scopes_report_structured_diagnostics() {
    let mut diagnostics = Vec::new();

    push_missing_capture_scope_diagnostics(
        &mut diagnostics,
        9,
        [
            Some(RequestedCaptureScope {
                kind: RequestedCaptureScopeKind::View,
                id: "view.HiddenPanel",
            }),
            Some(RequestedCaptureScope {
                kind: RequestedCaptureScopeKind::Object,
                id: "button.hidden",
            }),
            None,
        ],
        &[],
        &[],
        &[],
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.step == 9
            && diagnostic.severity == AgentDiagnosticSeverity::Error
            && diagnostic.source.as_deref() == Some("agent.observe")
            && diagnostic.code.as_deref() == Some("AGENT_CAPTURE_MISSING_SCOPE")
    }));
    assert!(diagnostics[0].message.contains("view.HiddenPanel"));
    assert!(diagnostics[1].message.contains("button.hidden"));
}

#[test]
fn player_image_object_observation_skips_hidden_source_and_frame() {
    let viewport = AgentViewport {
        width: 1280,
        height: 720,
        scale: 1.0,
    };
    let render_image = render_image("image.glass_bg");
    let visible_source = bundle_image_object("image.glass_bg", true);
    let hidden_source = bundle_image_object("image.glass_bg", false);
    let visible =
        player_observed_image_object(3, &viewport, &render_image, Some(&visible_source), 125)
            .expect("visible image object");
    let hidden =
        player_observed_image_object(3, &viewport, &render_image, Some(&hidden_source), 125);

    assert_eq!(visible.id, "object.image.image.glass_bg");
    assert!(hidden.is_none());
}

#[test]
fn player_image_object_observation_uses_scroll_clipped_visible_quad() {
    let viewport = AgentViewport {
        width: 1280,
        height: 720,
        scale: 1.0,
    };
    let mut render_image = render_image("image.glass_bg");
    render_image.fit = ImageObjectFit::Stretch;
    render_image.bounds = HitRect::new(100.0, 170.0, 200.0, 80.0);
    render_image.viewport_clip = Some(HitRect::new(100.0, 100.0, 160.0, 80.0));
    let source = bundle_image_object("image.glass_bg", true);
    let object = player_observed_image_object(3, &viewport, &render_image, Some(&source), 125)
        .expect("visible image object");

    assert_eq!(object.bbox.x, 100);
    assert_eq!(object.bbox.y, 170);
    assert_eq!(object.bbox.width, 160);
    assert_eq!(object.bbox.height, 10);
}

#[test]
fn hidden_image_object_capture_scope_reports_missing_scope_diagnostic() {
    let viewport = AgentViewport {
        width: 1280,
        height: 720,
        scale: 1.0,
    };
    let render_image = render_image("image.glass_bg");
    let hidden_source = bundle_image_object("image.glass_bg", false);
    let objects =
        player_observed_image_object(4, &viewport, &render_image, Some(&hidden_source), 125)
            .into_iter()
            .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    push_missing_capture_scope_diagnostics(
        &mut diagnostics,
        4,
        [
            None,
            Some(RequestedCaptureScope {
                kind: RequestedCaptureScopeKind::Object,
                id: "object.image.image.glass_bg",
            }),
            None,
        ],
        &[],
        &[],
        &objects,
    );

    assert!(objects.is_empty());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("AGENT_CAPTURE_MISSING_SCOPE")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("object.image.image.glass_bg")
    );
}

#[test]
fn released_image_object_capture_scope_reports_missing_scope_diagnostic() {
    let objects = Vec::new();
    let mut diagnostics = Vec::new();

    push_missing_capture_scope_diagnostics(
        &mut diagnostics,
        5,
        [
            None,
            Some(RequestedCaptureScope {
                kind: RequestedCaptureScopeKind::Object,
                id: "object.image.image.glass_bg",
            }),
            None,
        ],
        &[],
        &[],
        &objects,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].step, 5);
    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("AGENT_CAPTURE_MISSING_SCOPE")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("object.image.image.glass_bg")
    );
}

fn runtime_text_control(public_id: &str) -> ViewRuntimeTextControl {
    ViewRuntimeTextControl {
        public_id: public_id.to_owned(),
        target: public_id.to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        containing_scroll_region: None,
        session: 41,
        value: String::new(),
        selection: ViewRuntimeTextSelection::new(0, 0),
        options: ViewRuntimeTextControlOptions {
            purpose: ViewInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: false,
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
        },
        kind: ViewInputKind::TextField,
        bounds: ViewRuntimeTextControlBounds::from_px(48, 48, 420, 48),
        label: None,
        handlers: ViewRuntimeTextControlHandlers::default(),
        style: ViewRuntimeControlVisualStyle::default(),
    }
}

fn runtime_action_button(public_id: &str) -> ViewRuntimeActionButton {
    ViewRuntimeActionButton {
        public_id: public_id.to_owned(),
        target: public_id.to_owned(),
        view: Some("view.ModernFeedbackPanel".to_owned()),
        containing_scroll_region: None,
        label: "Continue".to_owned(),
        enabled: true,
        bounds: ViewRuntimeButtonBounds::new(484_000, 48_000, 180_000, 48_000),
        action: ViewRuntimeActionButtonAction::Noop,
        style: ViewRuntimeControlVisualStyle::default(),
    }
}

fn interaction_target(public_id: &str) -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new(public_id).expect("valid test target id"))
}

fn layer_id(public_id: &str) -> LayerId {
    LayerId::new(PublicId::try_new(public_id).expect("valid test layer id"))
}

fn render_image(id: &str) -> RenderImage {
    RenderImage {
        id: id.to_owned(),
        frame: RenderImageFrame {
            index: None,
            width: 2,
            height: 1,
            rgba: vec![10, 20, 30, 255, 40, 50, 60, 255],
        },
        bounds: HitRect::new(0.0, 0.0, 1280.0, 720.0),
        containing_scroll_region: None,
        viewport_clip: None,
        placement: None,
        fit: ImageObjectFit::Cover,
        alignment: ImageObjectAlignment::top_left(),
        transform: ImageObjectTransform::identity(),
        opacity_milli: 1_000,
    }
}

fn bundle_image_object(id: &str, visible: bool) -> BundleImageObject {
    BundleImageObject {
        id: id.to_owned(),
        asset: "asset.glass_bg".to_owned(),
        target: Some("target.glass_bg".to_owned()),
        layer: Some("layer.background".to_owned()),
        view: None,
        containing_scroll_region: None,
        bounds: BundleImageObjectBounds::from_px(0, 0, 1280, 720),
        placement: None,
        fit: BundleImageObjectFit::Cover,
        alignment: arcweft_bundle::BundleImageObjectAlignment::default(),
        playback: BundleImageObjectPlayback::default(),
        transform: BundleImageObjectTransform::default(),
        depth_milli: -10_000,
        opacity_milli: 1_000,
        actions: Vec::new(),
        params: BTreeMap::default(),
        proxies: Vec::new(),
        visible,
    }
}
