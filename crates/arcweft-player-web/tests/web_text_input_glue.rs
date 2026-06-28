use arcweft_player_web::edit_context::WebEditContextFeatureDetection;
use arcweft_player_web::web_text_input::{PlayerTextInputStatusKind, status_for_detection};

#[test]
fn player_owned_setup_reports_ready_only_for_full_editcontext_support() {
    let ready = status_for_detection(WebEditContextFeatureDetection::new(true, true), true);
    let missing_constructor =
        status_for_detection(WebEditContextFeatureDetection::new(false, true), true);
    let missing_property =
        status_for_detection(WebEditContextFeatureDetection::new(true, false), true);

    assert_eq!(ready.state(), PlayerTextInputStatusKind::Ready);
    assert_eq!(
        missing_constructor.state(),
        PlayerTextInputStatusKind::UnsupportedNoFallback
    );
    assert_eq!(
        missing_property.state(),
        PlayerTextInputStatusKind::UnsupportedNoFallback
    );
    assert!(!ready.fallback_installed());
    assert!(!missing_constructor.fallback_installed());
    assert!(!missing_property.fallback_installed());
}

#[test]
fn player_owned_setup_can_be_explicitly_disabled_without_fallback() {
    let disabled = status_for_detection(WebEditContextFeatureDetection::new(true, true), false);

    assert_eq!(disabled.state(), PlayerTextInputStatusKind::Disabled);
    assert!(!disabled.fallback_installed());
}
