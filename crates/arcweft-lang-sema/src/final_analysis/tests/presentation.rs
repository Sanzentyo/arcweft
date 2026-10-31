use super::{analyze, fixture};
use crate::callable::{
    CallableCandidateId, CheckedCallReceiverProjection, PresentationHandleMethodId,
};
use crate::final_analysis::FinalSemanticAnalysisError;
use crate::types::TypeKind;
use arcweft_interaction_model::presentation::PresentationResourceKind;

#[test]
fn generic_handle_methods_preserve_the_checked_receiver_and_selected_identity() {
    for (name, expected) in [
        ("show", PresentationHandleMethodId::Show),
        ("hide", PresentationHandleMethodId::Hide),
        ("unmount", PresentationHandleMethodId::Unmount),
        ("release", PresentationHandleMethodId::Release),
        ("destroy", PresentationHandleMethodId::Destroy),
    ] {
        let source = format!("fn invoke<T>(handle: Handle<T>) {{ handle.{name}() }}");
        let world = fixture(&source, None);
        let report = analyze(&world).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let applications = report
            .calls()
            .filter_map(|(_, call)| call.selected_application())
            .collect::<Vec<_>>();
        let [application] = applications.as_slice() else {
            panic!("{name}: exactly one selected method is required")
        };
        assert_eq!(
            application.core().candidates().selected().id(),
            &CallableCandidateId::PresentationHandleMethod(expected),
            "{name}"
        );
        assert_eq!(application.result().value_type(), Some(&TypeKind::Unit));
        assert!(matches!(
            application.core().execution().receiver(),
            CheckedCallReceiverProjection::Operand {
                ty: TypeKind::Handle { item, .. }, ..
            } if matches!(item.as_ref(), TypeKind::GenericParam(_))
        ));
    }
}

#[test]
fn overlay_pop_requires_the_exact_overlay_handle() {
    let world = fixture("fn invoke(handle: Handle<Overlay>) { handle.pop() }", None);
    let report = analyze(&world).expect("the exact Overlay handle supports pop");
    let selected = report
        .calls()
        .filter_map(|(_, call)| call.selected_application())
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0].core().candidates().selected().id(),
        &CallableCandidateId::PresentationHandleMethod(PresentationHandleMethodId::OverlayPop)
    );

    let mut rejected = vec!["fn invoke<T>(handle: Handle<T>) { handle.pop() }".to_owned()];
    rejected.extend(
        PresentationResourceKind::ALL
            .into_iter()
            .filter(|kind| *kind != PresentationResourceKind::Overlay)
            .map(|kind| {
                format!(
                    "fn invoke(handle: Handle<{}>) {{ handle.pop() }}",
                    kind.type_name()
                )
            }),
    );
    for source in rejected {
        let world = fixture(&source, None);
        assert!(
            matches!(
                analyze(&world),
                Err(FinalSemanticAnalysisError::UnknownCallTarget { name, .. }) if name == "pop"
            ),
            "{source}"
        );
    }
}
