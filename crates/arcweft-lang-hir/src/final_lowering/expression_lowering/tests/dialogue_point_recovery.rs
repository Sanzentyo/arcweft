use super::*;
use crate::dialogue_application::{
    HirDialogueNodeKind, HirDialoguePointActionIdentity, HirDialoguePointActionPayload,
};
use crate::source_index::HirDialoguePointActionSourcePart;

#[test]
fn recovered_point_actions_keep_their_owned_source_and_never_issue_marks() {
    for markup in [
        "[mark .release]",
        "[mark @.]",
        "[mark @character.release]",
        "[mark @.one.two]",
        "[mark \"@.release\"]",
        "[call]",
        "[at]",
    ] {
        let parsed = parsed_source(
            "point-action-recovery",
            &[format!(
                "@<character.alice>()[本文。|[夢](ゆめ){markup}[p]]"
            )],
        );
        let (module, owners, _) = lower_and_publish(&parsed);
        let HirExprKind::AttachedContentApplication(application) =
            expression(&module, owners[0]).kind()
        else {
            panic!("fixture retains attached content: {markup}")
        };
        assert!(!module.is_analysis_ready(), "fixture: {markup}");
        assert!(
            application.content().marks().is_empty(),
            "fixture: {markup}"
        );
        let recovered = application
            .content()
            .nodes()
            .iter()
            .find_map(|node| match node.kind() {
                HirDialogueNodeKind::PointAction(action) if action.has_recovery() => Some(action),
                _ => None,
            })
            .unwrap_or_else(|| panic!("typed point-action recovery: {markup}"));
        assert!(matches!(
            (recovered.identity(), recovered.payload()),
            (HirDialoguePointActionIdentity::RecoveredMark(_), _)
                | (HirDialoguePointActionIdentity::Invalid(_), _)
                | (_, HirDialoguePointActionPayload::MissingCall(_))
                | (_, HirDialoguePointActionPayload::MissingTimedCue(_))
        ));
        let source = module
            .source_anchor(HirSourceQuery::Expr {
                owner: owners[0],
                role: HirExprSourceRole::DialoguePointAction {
                    ordinal: recovered.id().ordinal(),
                    part: HirDialoguePointActionSourcePart::Whole,
                },
            })
            .expect("recovered action owns its source role")
            .expect("recovered action has authored source");
        assert_eq!(&parsed.source()[source.range().as_range()], markup);
    }
}

#[test]
fn accepted_mark_keeps_its_local_identity_without_recovery() {
    let parsed = parsed_source(
        "accepted-point-action",
        &["@<character.alice>()[本文。[mark @.release][p]]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    let HirExprKind::AttachedContentApplication(application) =
        expression(&module, owners[0]).kind()
    else {
        panic!("attached content")
    };
    assert!(module.is_analysis_ready());
    assert_eq!(application.content().marks().len(), 1);
    assert_eq!(application.content().marks()[0].name().as_str(), "release");
    assert!(application.content().nodes().iter().all(|node| {
        !matches!(node.kind(), HirDialogueNodeKind::PointAction(action) if action.has_recovery())
    }));
}

#[test]
fn a_missing_call_cannot_be_published_as_an_authored_payload() {
    use crate::dialogue_application::{
        HirAttachedContentApplication, HirDialogueContent, HirDialogueNode, HirDialoguePointAction,
    };
    use crate::expr::HirExpr;

    assert_expression_freeze_rejects(
        "point-action-payload-substitution",
        "@<character.alice>()[本文。[call][p]]",
        |transaction, owner| {
            let (slots, arenas) = transaction.storage_mut();
            let expression = arenas.expressions().resolve_staged(slots, owner).unwrap();
            let HirExprKind::AttachedContentApplication(application) = expression.kind() else {
                panic!("attached content")
            };
            let mut nodes = application.content().nodes().to_vec();
            let mut replaced = false;
            for node in &mut nodes {
                let HirDialogueNodeKind::PointAction(action) = node.kind() else {
                    continue;
                };
                let HirDialoguePointActionPayload::MissingCall(missing) = action.payload() else {
                    continue;
                };
                let substituted = HirDialoguePointAction::try_new(
                    action.id(),
                    action.identity().clone(),
                    action.arguments().to_vec().into_boxed_slice(),
                    HirDialoguePointActionPayload::Call(missing),
                )
                .unwrap();
                *node =
                    HirDialogueNode::new(node.id(), HirDialogueNodeKind::PointAction(substituted));
                replaced = true;
            }
            assert!(replaced);
            let content = HirDialogueContent::try_new(
                application.content().id(),
                nodes.into_boxed_slice(),
                Box::new([]),
            )
            .unwrap();
            let substituted = HirAttachedContentApplication::try_new_with_body_presence(
                owner,
                content,
                application.family().clone(),
                application.body_presence(),
            )
            .unwrap();
            let payload = HirExpr::try_new(
                expression.scope(),
                HirExprKind::AttachedContentApplication(substituted),
                expression.state().clone(),
            )
            .unwrap();
            arenas
                .expressions()
                .revise_finalized(slots, owner, payload)
                .unwrap();
        },
    );
}
