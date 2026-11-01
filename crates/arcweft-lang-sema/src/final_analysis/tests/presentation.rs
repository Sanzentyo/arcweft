use super::{analyze, fixture};
use crate::callable::{
    CallableCandidateId, CallableDiagnosticCode, CallableGroupIndex, CallableParameterConsumer,
    CheckedCallReceiverProjection, PresentationArgumentRole, PresentationCallableId,
    PresentationHandleMethodId,
};
use crate::final_analysis::FinalSemanticAnalysisError;
use crate::types::TypeKind;
use arcweft_interaction_model::presentation::PresentationResourceKind;

#[test]
fn inert_slot_reference_calls_reject_scope_and_occupant_configuration() {
    for call in ["ref.bg", "ref.show"] {
        for name in ["scope", "fade", "look", "param.extra"] {
            let character = if call == "ref.show" {
                "@character.alice, "
            } else {
                ""
            };
            let expression = format!("{call}({character}{name} = 42i32)");
            let source = format!("pub character alice {{}}\nfn address() {{ {expression} }}");
            let world = fixture(&source, None);
            let report = analyze(&world).unwrap_or_else(|error| panic!("{source}: {error:?}"));
            let calls = report.calls().collect::<Vec<_>>();
            let [(_, call)] = calls.as_slice() else {
                panic!("one rejected address call: {source}")
            };
            assert!(call.selected_application().is_none(), "{source}");
            let [diagnostic] = call.diagnostics() else {
                panic!("one address argument diagnostic: {source}")
            };
            assert_eq!(
                diagnostic.code(),
                CallableDiagnosticCode::NoViableSignature,
                "{source}"
            );
            assert_eq!(
                &source[diagnostic
                    .span()
                    .expect("address call span")
                    .range()
                    .as_range()],
                expression
            );
        }
    }
}

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

#[test]
fn mount_binds_the_retained_view_value_and_publishes_a_view_handle() {
    for value in [
        r#"Card("hello")"#,
        r#"{ let selected = Card; selected(caption = "hello") }"#,
    ] {
        let source = format!(
            "view Card(caption: String) {{ Text(caption) }}\nfn install() -> Handle<View> {{ mount({value}) }}\n"
        );
        let world = fixture(&source, None);
        let report = analyze(&world).unwrap_or_else(|error| panic!("{value}: {error:?}"));
        let calls = report
            .calls()
            .filter_map(|(_, call)| call.selected_application())
            .collect::<Vec<_>>();
        let mount = calls
            .iter()
            .find(|call| {
                call.core().candidates().selected().id()
                    == &CallableCandidateId::Presentation(PresentationCallableId::Mount)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{value}: mount must be selected: {:?}",
                    report
                        .calls()
                        .map(|(_, call)| call.diagnostics())
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(
            mount.result().value_type(),
            Some(&TypeKind::presentation_handle(
                TypeKind::PresentationResource(PresentationResourceKind::View)
            ))
        );
        let parameter = &mount
            .core()
            .candidates()
            .selected()
            .schema()
            .group(CallableGroupIndex::ZERO)
            .expect("mount argument group")
            .parameters()[0];
        assert_eq!(
            parameter.consumer(),
            &CallableParameterConsumer::Presentation(PresentationArgumentRole::ViewValue)
        );
        assert!(calls.iter().any(|call| {
            matches!(
                call.core().candidates().selected().id(),
                CallableCandidateId::Project(_)
            ) && call.result().value_type() == Some(&TypeKind::ViewValue)
        }));
    }
}

#[test]
fn mount_rejects_an_ordinary_value_instead_of_a_retained_view() {
    let source = "fn install() -> Handle<View> { mount(42i64) }";
    let world = fixture(source, None);
    let report = analyze(&world).expect("the rejected call retains its diagnostic");
    let calls = report.calls().collect::<Vec<_>>();
    let [(_, call)] = calls.as_slice() else {
        panic!("one rejected mount call")
    };
    assert!(call.selected_application().is_none());
    let [diagnostic] = call.diagnostics() else {
        panic!("one rejected mount diagnostic is required")
    };
    assert_eq!(diagnostic.code(), CallableDiagnosticCode::NoViableSignature);
    assert_eq!(
        &source[diagnostic
            .span()
            .expect("mount source span")
            .range()
            .as_range()],
        "mount(42i64)"
    );
}
