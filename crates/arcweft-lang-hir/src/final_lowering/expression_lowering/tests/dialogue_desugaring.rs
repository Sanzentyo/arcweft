use super::*;
use crate::dialogue_application::ruby::HirRubyDesugaring;
use crate::leaf::{HirPath, HirPathRoot, HirPathValue, HirStringLiteral};

#[test]
fn ruby_desugaring_is_admitted_in_direct_nested_and_candidate_content() {
    for (source, ambiguous, status) in [
        ("alice()[|[夢](ゆめ)]", true, HirModuleStatus::Recovered),
        ("alice()[Before |[夢](ゆめ)]", false, HirModuleStatus::Clean),
        (
            "alice()[Before #strong()[｜夢《ゆめ》]]",
            false,
            HirModuleStatus::Clean,
        ),
        ("items[|value|[夢](ゆめ)]", true, HirModuleStatus::Clean),
        // The alternative Dialogue interpretation recovers an invalid point
        // action; its independent Index block contains a clean Ruby call.
        (
            "items[{ alice()[Before |[夢](ゆめ)] }]",
            true,
            HirModuleStatus::Recovered,
        ),
    ] {
        let parsed = parsed_source("ruby-content-context", &[source.into()]);
        let (module, owners, _) = lower_and_publish(&parsed);
        assert_eq!(module.status(), status, "fixture: {source}");
        if ambiguous {
            assert!(
                matches!(expression(&module, owners[0]).kind(), HirExprKind::PostfixBracket(postfix) if matches!(postfix.candidates(), HirPostfixBracketCandidates::Ambiguous { .. }))
            );
        }
        assert!(module.arenas().expressions().try_iter(module.slots()).unwrap().any(|(id, _)| {
            matches!(module.slots().resolve(id).unwrap().origin(), HirOrigin::Synthetic(key) if key.role() == SyntheticRole::DialogueRubyApplication)
        }));
    }
}

fn generated_node(transaction: &mut StagedHirModuleTransaction<'_>, role: SyntheticRole) -> ExprId {
    let (slots, _) = transaction.storage_mut();
    slots.live_ids::<ExprId>().find(|id| {
        matches!(slots.resolve_staged(*id).unwrap().origin(), HirOrigin::Synthetic(key) if key.role() == role)
    }).expect("fixture owns the generated expression")
}

#[test]
fn altered_desugared_payloads_cannot_publish() {
    for source in [
        "alice()[Before |[夢](ゆめ)]",
        "items[|value|[夢](ゆめ)]",
        "items[{ alice()[Before |[夢](ゆめ)] }]",
    ] {
        for role in HirRubyDesugaring::ROLES {
            assert_expression_freeze_rejects("ruby-payload-tamper", source, |transaction, _| {
                let id = generated_node(transaction, role);
                let ids = HirRubyDesugaring::ROLES.map(|role| generated_node(transaction, role));
                let (slots, arenas) = transaction.storage_mut();
                let scope = arenas
                    .expressions()
                    .resolve_staged(slots, id)
                    .unwrap()
                    .scope();
                if role == SyntheticRole::DialogueRubyApplication {
                    let HirOrigin::Synthetic(key) = slots.resolve_staged(id).unwrap().origin()
                    else {
                        unreachable!()
                    };
                    let SyntheticOwner::Expr(owner) = key.owner() else {
                        unreachable!()
                    };
                    let recipe =
                        HirRubyDesugaring::new(owner, key.ordinal(), scope, "wrong_base", "ゆめ");
                    let [_, _, payload] = recipe.payloads(ids).unwrap();
                    arenas
                        .expressions()
                        .revise_finalized(slots, id, payload)
                        .unwrap();
                    return;
                }
                let kind = if role == SyntheticRole::DialogueRubyTarget {
                    HirExprKind::Path(HirPathValue::Resolved(
                        HirPath::try_new(
                            HirPathRoot::ImplicitCrate,
                            Box::new([HirPathSegment::Identifier(
                                crate::leaf::HirName::try_new("wrong_target".into()).unwrap(),
                            )]),
                        )
                        .unwrap(),
                    ))
                } else {
                    HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value(
                        "wrong_reading".into(),
                    )))
                };
                let payload = HirExpr::try_new(scope, kind, HirPoisonState::Clean).unwrap();
                arenas
                    .expressions()
                    .revise_finalized(slots, id, payload)
                    .unwrap();
            });
        }
    }
}

#[test]
fn unreferenced_desugared_expression_cannot_publish() {
    for source in ["alice()[Before |[夢](ゆめ)]", "items[|value|[夢](ゆめ)]"] {
        assert_expression_freeze_rejects("ruby-orphan-expression", source, |transaction, _| {
            let id = generated_node(transaction, SyntheticRole::DialogueRubyApplication);
            let (slots, arenas) = transaction.storage_mut();
            let metadata = slots.resolve_staged(id).unwrap();
            let HirOrigin::Synthetic(key) = metadata.origin() else {
                unreachable!()
            };
            let SyntheticOwner::Expr(owner) = key.owner() else {
                unreachable!()
            };
            let site = metadata.source_site().clone();
            let scope = arenas
                .expressions()
                .resolve_staged(slots, id)
                .unwrap()
                .scope();
            let recipe = HirRubyDesugaring::new(owner, 100, scope, "夢", "ゆめ");
            let [_, reading, _] = recipe.keys().unwrap();
            let reservation = arenas
                .expressions()
                .reserve_synthetic(slots, reading, site)
                .unwrap();
            let payload = HirExpr::try_new(
                scope,
                HirExprKind::Literal(HirLiteral::String(HirStringLiteral::Value("ゆめ".into()))),
                HirPoisonState::Clean,
            )
            .unwrap();
            arenas
                .expressions()
                .finalize(slots, reservation, payload)
                .unwrap();
        });
    }
}

#[test]
fn desugared_expression_cannot_change_its_parent_scope() {
    assert_expression_freeze_rejects(
        "ruby-scope-tamper",
        "items[{ alice()[Before |[夢](ゆめ)] }]",
        |transaction, root| {
            let id = generated_node(transaction, SyntheticRole::DialogueRubyReading);
            let (slots, arenas) = transaction.storage_mut();
            let root_scope = arenas
                .expressions()
                .resolve_staged(slots, root)
                .unwrap()
                .scope();
            let original = arenas.expressions().resolve_staged(slots, id).unwrap();
            assert_ne!(original.scope(), root_scope);
            let payload =
                HirExpr::try_new(root_scope, original.kind().clone(), HirPoisonState::Clean)
                    .unwrap();
            arenas
                .expressions()
                .revise_finalized(slots, id, payload)
                .unwrap();
        },
    );
}
