use super::*;
use crate::dialogue_application::{
    HirBuiltinRichTextTag, HirDialogueCoordinateKind, HirDialogueNodeKind,
    HirPostfixBracketCandidates, HirRichTextDirectStyle, HirRichTextIssue, HirRichTextTagIdentity,
};
use crate::source_index::{HirDialogueNodeSourcePart, HirRichTextTagSourcePart, HirSourcePresence};
use crate::type_ref::HirTypeKind;

fn ambiguous_tuple_candidate(element_count: usize) -> String {
    assert!(element_count > 1);
    let mut source = String::with_capacity(
        element_count
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(8))
            .expect("candidate tuple source size"),
    );
    source.push_str("items[(");
    for ordinal in 0..element_count {
        if ordinal > 0 {
            source.push(',');
        }
        source.push('a');
    }
    source.push_str(")]");
    source
}

fn ambiguous_typed_tuple_candidate(plain_element_count: usize) -> String {
    assert!(plain_element_count > 0);
    let mut source = String::from("items[(Vec<I32>::with_capacity(8)");
    for _ in 0..plain_element_count {
        source.push_str(",a");
    }
    source.push_str(")]");
    source
}

#[test]
fn selected_bracket_dialogue_lowers_typed_content_and_exact_sources() {
    let parsed = parsed_source(
        "dialogue-selected-bracket",
        &["alice[前[strong]強調[/strong]後]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    assert!(module.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        crate::diagnostic::HirDiagnostic::LineIdentity(line)
            if line.code()
                == crate::line_identity::DialogueLineDiagnosticCode::MissingLineSourceOwner
    )));

    let owner = owners[0];
    let HirExprKind::DialogueContentApplication(application) = expression(&module, owner).kind()
    else {
        panic!("selected bracket must publish E33");
    };
    assert!(matches!(
        expression(&module, application.target()).kind(),
        HirExprKind::Path(_)
    ));
    assert_eq!(application.content().nodes().len(), 5);
    assert_eq!(application.content().tags().len(), 1);
    assert!(matches!(
        application.content().tags()[0].identity(),
        HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::DirectStyle(
            HirRichTextDirectStyle::Strong
        ))
    ));
    assert!(matches!(
        application.content().nodes()[1].kind(),
        HirDialogueNodeKind::AuthoredStartTag(tag)
            if *tag == application.content().tags()[0].id()
    ));
    assert!(matches!(
        application.content().nodes()[3].kind(),
        HirDialogueNodeKind::AuthoredEndTag(_)
    ));

    for role in [
        HirExprSourceRole::Target,
        HirExprSourceRole::OpenBracket,
        HirExprSourceRole::CloseBracket,
        HirExprSourceRole::Content,
        HirExprSourceRole::ContentBody,
        HirExprSourceRole::DialogueNode {
            ordinal: 0,
            part: HirDialogueNodeSourcePart::Whole,
        },
        HirExprSourceRole::DialogueNode {
            ordinal: 0,
            part: HirDialogueNodeSourcePart::Text,
        },
        HirExprSourceRole::RichTextTag {
            tag: 0,
            part: HirRichTextTagSourcePart::Whole,
        },
        HirExprSourceRole::RichTextTag {
            tag: 0,
            part: HirRichTextTagSourcePart::EndTag,
        },
    ] {
        assert!(matches!(
            module
                .source_site(
                    parsed.document().identity(),
                    HirSourceQuery::Expr { owner, role },
                )
                .expect("E33 source role")
                .presence(),
            HirSourcePresence::Present(_)
        ));
    }
}

#[test]
fn missing_dialogue_content_is_empty_and_poisoned_without_fake_node() {
    let parsed = parsed_source("dialogue-missing-content", &["alice[]".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::DialogueContentApplication(application) = expression(&module, owner).kind()
    else {
        panic!("missing dialogue content keeps E33");
    };
    assert!(application.content().nodes().is_empty());
    assert!(application.content().tags().is_empty());
    assert_eq!(
        expression(&module, owner).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Content,
        })
    );
    assert!(matches!(
        module
            .source_site(
                parsed.document().identity(),
                HirSourceQuery::Expr {
                    owner,
                    role: HirExprSourceRole::Content,
                },
            )
            .expect("missing content query")
            .presence(),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
    ));
}

#[test]
fn dialogue_interpolation_and_immediate_coordinates_keep_same_arena_ids() {
    let parsed = parsed_source(
        "dialogue-interpolation-coordinates",
        &["alice(id = @say.entry, text_key = @text.entry)[こんにちは #[actor.name]]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::DialogueContentApplication(application) = expression(&module, owner).kind()
    else {
        panic!("dialogue application");
    };
    assert_eq!(application.coordinates().len(), 2);
    assert_eq!(
        application.coordinates()[0].kind(),
        HirDialogueCoordinateKind::Id
    );
    assert_eq!(
        application.coordinates()[1].kind(),
        HirDialogueCoordinateKind::TextKey
    );
    for coordinate in application.coordinates() {
        assert!(
            module
                .arenas()
                .expressions()
                .resolve(module.slots(), coordinate.value())
                .is_ok()
        );
    }
    let interpolation = application
        .content()
        .nodes()
        .iter()
        .find_map(|node| match node.kind() {
            HirDialogueNodeKind::Interpolation(expression) => Some(*expression),
            _ => None,
        })
        .expect("interpolation expression");
    assert!(matches!(
        expression(&module, interpolation).kind(),
        HirExprKind::Select(_)
    ));
}

#[test]
fn invalid_postfix_publishes_two_failures_without_candidate_ids() {
    let parsed = parsed_source("dialogue-invalid-postfix", &["items[,]".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("invalid generic postfix");
    };
    assert!(matches!(
        postfix.candidates(),
        HirPostfixBracketCandidates::Invalid { .. }
    ));
    assert_eq!(
        module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .expect("expression inventory")
            .count(),
        2,
        "only the source-backed target and E34 owner may exist"
    );
}

#[test]
fn ambiguous_postfix_retains_shared_target_and_distinct_synthetic_candidates() {
    let parsed = parsed_source("dialogue-ambiguous-postfix", &["items[key]".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous generic postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates() else {
        panic!("both viable interpretations must remain typed candidates");
    };

    let HirExprKind::Index(index_payload) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let HirExprKind::DialogueContentApplication(dialogue_payload) =
        expression(&module, *dialogue).kind()
    else {
        panic!("dialogue-content candidate root");
    };
    assert_eq!(postfix.target(), index_payload.target());
    assert_eq!(postfix.target(), dialogue_payload.target());
    assert_ne!(index_payload.index(), postfix.target());

    for (candidate, role) in [
        (*index, SyntheticRole::PostfixIndexCandidateExpression),
        (*dialogue, SyntheticRole::DialogueContentCandidateExpression),
    ] {
        let metadata = module
            .slots()
            .resolve(candidate)
            .expect("candidate root slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == role
                    && key.ordinal() == 0
        ));
        assert!(matches!(
            metadata.source_site(),
            HirSourceSite::Insertion(_)
        ));
    }

    let index_child = index_payload.index();
    let index_child_metadata = module
        .slots()
        .resolve(index_child)
        .expect("candidate index child slot");
    assert!(matches!(
        index_child_metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(owner)
                && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                && key.ordinal() == 1
    ));
    assert!(matches!(
        expression(&module, index_child).kind(),
        HirExprKind::Path(_)
    ));
}

#[test]
fn dialogue_candidate_projects_rich_text_sources_from_exact_outer_owner() {
    let parsed = parsed_source(
        "dialogue-candidate-rich-text-source",
        &["alice[Hello[p]]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let outer = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, outer).kind() else {
        panic!("end-position point tag retains E34 ambiguity");
    };
    let HirPostfixBracketCandidates::Ambiguous { dialogue, .. } = postfix.candidates() else {
        panic!("Dialogue interpretation remains a typed candidate");
    };
    let HirExprKind::DialogueContentApplication(application) =
        expression(&module, *dialogue).kind()
    else {
        panic!("Dialogue candidate publishes final E33 payload");
    };
    assert!(matches!(
        application.content().tags(),
        [tag]
            if matches!(
                tag.identity(),
                HirRichTextTagIdentity::Builtin(HirBuiltinRichTextTag::Page)
            )
    ));

    let lookup = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: *dialogue,
                role: HirExprSourceRole::RichTextTag {
                    tag: 0,
                    part: HirRichTextTagSourcePart::Whole,
                },
            },
        )
        .expect("candidate RichText role is projected through the exact candidate ExprId");
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
        panic!("authored candidate RichText tag retains one span");
    };
    assert_eq!(&parsed.document().text()[span.range().as_range()], "[p]");
}

#[test]
fn ambiguous_index_candidate_uses_preorder_synthetic_expression_ordinals() {
    let parsed = parsed_source(
        "dialogue-ambiguous-composite-index",
        &["items[left + right]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous generic postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary-index candidate must remain typed");
    };
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let HirExprKind::Binary(binary) = expression(&module, index.index()).kind() else {
        panic!("candidate index payload must retain its binary expression");
    };
    assert!(matches!(
        expression(&module, binary.left()).kind(),
        HirExprKind::Path(_)
    ));
    assert!(matches!(
        expression(&module, binary.right()).kind(),
        HirExprKind::Path(_)
    ));

    for (expression, ordinal) in [(index.index(), 1), (binary.left(), 2), (binary.right(), 3)] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("candidate expression slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
        assert!(matches!(metadata.source_site(), HirSourceSite::Span(_)));
    }
}

#[test]
fn ambiguous_associated_call_keeps_candidate_expression_and_type_preorders() {
    let parsed = parsed_source(
        "dialogue-ambiguous-associated-call-index",
        &["items[Vec<I32>::with_capacity(8)]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous generic postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary-index candidate must remain typed");
    };
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let call_id = index.index();
    let HirExprKind::Call(call) = expression(&module, call_id).kind() else {
        panic!("candidate index payload must retain its Call");
    };
    let (receiver, _, member) = call
        .callee()
        .associated_parts()
        .expect("explicit associated Call");
    assert_eq!(
        member.resolved().map(crate::leaf::HirName::as_str),
        Some("with_capacity")
    );
    let receiver_id = receiver.type_id().expect("associated receiver type");
    let receiver_type = module
        .arenas()
        .types()
        .resolve(module.slots(), receiver_id)
        .expect("candidate receiver type");
    let HirTypeKind::Generic(receiver_type) = receiver_type.kind() else {
        panic!("Vec<I32> must remain a typed generic receiver");
    };
    let [argument_type] = receiver_type.arguments() else {
        panic!("Vec receiver has one generic argument");
    };
    assert!(matches!(
        module
            .arenas()
            .types()
            .resolve(module.slots(), *argument_type)
            .expect("candidate generic argument")
            .kind(),
        HirTypeKind::Path(_)
    ));
    let [
        HirCallArgument::Positional {
            value: HirCallValue::Present { value: argument },
        },
    ] = call.arguments()
    else {
        panic!("associated candidate Call keeps its positional argument");
    };

    for (expression, ordinal) in [(call_id, 1), (*argument, 2)] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("candidate Call expression slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
        assert!(matches!(metadata.source_site(), HirSourceSite::Span(_)));
    }
    for (ty, ordinal) in [(receiver_id, 0), (*argument_type, 1)] {
        let metadata = module
            .slots()
            .resolve(ty)
            .expect("candidate Call type slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
        assert!(matches!(metadata.source_site(), HirSourceSite::Span(_)));
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test validates one complete nested associated-call type/value preorder across both ambiguity interpretations"
)]
fn nested_associated_call_keeps_receiver_types_before_argument_types() {
    let parsed = parsed_source(
        "dialogue-nested-associated-call-index",
        &["items[Vec<I32>::with_capacity(Vec<U32>::with_capacity(8))]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous generic postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary-index candidate must remain typed");
    };
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let outer_call_id = index.index();
    let HirExprKind::Call(outer_call) = expression(&module, outer_call_id).kind() else {
        panic!("candidate index payload must retain its outer Call");
    };
    let (outer_receiver, _, _) = outer_call
        .callee()
        .associated_parts()
        .expect("outer associated Call");
    let outer_receiver_id = outer_receiver.type_id().expect("outer receiver type");
    let HirTypeKind::Generic(outer_receiver_type) = module
        .arenas()
        .types()
        .resolve(module.slots(), outer_receiver_id)
        .expect("outer candidate receiver type")
        .kind()
    else {
        panic!("outer Vec<I32> receiver");
    };
    let [outer_type_argument] = outer_receiver_type.arguments() else {
        panic!("outer Vec receiver has one generic argument");
    };
    let [
        HirCallArgument::Positional {
            value: HirCallValue::Present {
                value: nested_call_id,
            },
        },
    ] = outer_call.arguments()
    else {
        panic!("outer Call keeps its nested argument");
    };

    let HirExprKind::Call(nested_call) = expression(&module, *nested_call_id).kind() else {
        panic!("outer argument must retain its nested Call");
    };
    let (nested_receiver, _, _) = nested_call
        .callee()
        .associated_parts()
        .expect("nested associated Call");
    let nested_receiver_id = nested_receiver.type_id().expect("nested receiver type");
    let HirTypeKind::Generic(nested_receiver_type) = module
        .arenas()
        .types()
        .resolve(module.slots(), nested_receiver_id)
        .expect("nested candidate receiver type")
        .kind()
    else {
        panic!("nested Vec<U32> receiver");
    };
    let [nested_type_argument] = nested_receiver_type.arguments() else {
        panic!("nested Vec receiver has one generic argument");
    };
    let [
        HirCallArgument::Positional {
            value: HirCallValue::Present { value: literal },
        },
    ] = nested_call.arguments()
    else {
        panic!("nested Call keeps its literal argument");
    };

    for (expression, ordinal) in [(outer_call_id, 1), (*nested_call_id, 2), (*literal, 3)] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("nested candidate Call expression slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }
    for (ty, ordinal) in [
        (outer_receiver_id, 0),
        (*outer_type_argument, 1),
        (nested_receiver_id, 2),
        (*nested_type_argument, 3),
    ] {
        let metadata = module
            .slots()
            .resolve(ty)
            .expect("nested candidate Call type slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }
}

#[test]
fn nested_e34_associated_call_keeps_one_global_candidate_type_preorder() {
    let parsed = parsed_source(
        "dialogue-nested-e34-associated-call-index",
        &["x[#[Vec<U32>::with_capacity(8)]]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous outer postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary-index candidate");
    };
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("outer ordinary-index root");
    };
    let nested_postfix_id = index.index();
    let HirExprKind::PostfixBracket(nested_postfix) = expression(&module, nested_postfix_id).kind()
    else {
        panic!("nested E34 postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous {
        index: nested_index,
        ..
    } = nested_postfix.candidates()
    else {
        panic!("nested E34 ordinary-index candidate");
    };
    let HirExprKind::Index(nested_index) = expression(&module, *nested_index).kind() else {
        panic!("nested ordinary-index root");
    };
    let HirExprKind::Call(nested_call) = expression(&module, nested_index.index()).kind() else {
        panic!("nested associated Call");
    };
    let (nested_receiver, _, _) = nested_call
        .callee()
        .associated_parts()
        .expect("nested associated receiver");
    let HirTypeKind::Generic(nested_receiver_type) = module
        .arenas()
        .types()
        .resolve(
            module.slots(),
            nested_receiver.type_id().expect("nested receiver type"),
        )
        .expect("nested receiver type")
        .kind()
    else {
        panic!("nested Vec<U32> receiver");
    };
    let [nested_argument_type] = nested_receiver_type.arguments() else {
        panic!("nested receiver type argument");
    };

    for (ty, ordinal) in [
        (nested_receiver.type_id().expect("nested receiver type"), 0),
        (*nested_argument_type, 1),
    ] {
        let metadata = module
            .slots()
            .resolve(ty)
            .expect("nested E34 candidate type slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }
}

#[test]
fn nested_dialogue_application_candidate_keeps_typed_content_and_global_preorder() {
    let parsed = parsed_source(
        "dialogue-nested-application-index-candidate",
        &["items[alice[hello #[actor.name]]]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous outer postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates() else {
        panic!("ordinary-index candidate");
    };
    assert_eq!(expression(&module, *index).state(), &HirPoisonState::Clean);
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let application_id = index.index();
    let HirExprKind::DialogueContentApplication(application) =
        expression(&module, application_id).kind()
    else {
        panic!("nested E33 candidate application");
    };
    assert_eq!(
        expression(&module, application_id).state(),
        &HirPoisonState::Clean
    );
    assert_eq!(
        expression(&module, *dialogue).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidRichText(
            HirRichTextIssue::UnknownRegisteredTag,
        ))
    );
    assert!(matches!(
        expression(&module, application.target()).kind(),
        HirExprKind::Path(_)
    ));
    let interpolation = application
        .content()
        .nodes()
        .iter()
        .find_map(|node| match node.kind() {
            HirDialogueNodeKind::Interpolation(expression) => Some(*expression),
            _ => None,
        })
        .expect("nested application interpolation");
    let HirExprKind::Select(select) = expression(&module, interpolation).kind() else {
        panic!("interpolation retains Select");
    };

    for (expression, ordinal) in [
        (application_id, 1),
        (application.target(), 2),
        (interpolation, 3),
        (select.target(), 4),
    ] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("nested E33 candidate slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }
}

#[test]
fn nested_dialogue_application_candidate_retains_missing_content_recovery() {
    let parsed = parsed_source(
        "dialogue-nested-missing-content-index-candidate",
        &["items[alice[]]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous outer postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates() else {
        panic!("ordinary-index candidate");
    };
    assert_eq!(
        expression(&module, *index).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Index,
            },
        ))
    );
    assert_eq!(
        expression(&module, *dialogue).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::TransactionalChildFailure,),
        ))
    );
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let application_id = index.index();
    let HirExprKind::DialogueContentApplication(application) =
        expression(&module, application_id).kind()
    else {
        panic!("nested E33 candidate application");
    };
    assert!(application.content().nodes().is_empty());
    assert!(application.content().tags().is_empty());
    assert_eq!(
        expression(&module, application_id).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::Content,
        })
    );
}

#[test]
fn record_candidate_keeps_typed_path_field_value_and_global_preorder() {
    let parsed = parsed_source(
        "dialogue-record-index-candidate",
        &["items[Point { x = value }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous outer postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary-index candidate");
    };
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let record_id = index.index();
    let HirExprKind::Record(record) = expression(&module, record_id).kind() else {
        panic!("typed path-qualified Record candidate");
    };
    assert_eq!(record.path().segments().len(), 1);
    let [HirRecordField::Explicit { value, .. }] = record.fields() else {
        panic!("one explicit Record field");
    };
    assert!(matches!(
        expression(&module, *value).kind(),
        HirExprKind::Path(_)
    ));

    for (expression, ordinal) in [(record_id, 1), (*value, 2)] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("Record candidate slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }
}

#[test]
fn record_literal_candidate_retains_missing_field_value_recovery_and_preorder() {
    let parsed = parsed_source(
        "dialogue-record-literal-index-candidate",
        &["items[{ x = value, y: }]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous outer postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("ordinary-index candidate");
    };
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let record_id = index.index();
    let HirExprKind::RecordLiteral(record) = expression(&module, record_id).kind() else {
        panic!("typed RecordLiteral candidate");
    };
    let [HirRecordField::Explicit { value, .. }, invalid] = record.fields() else {
        panic!("authored and missing RecordLiteral fields");
    };
    assert_eq!(invalid.issue(), Some(HirRecordFieldIssue::MissingValue));
    assert_eq!(
        expression(&module, record_id).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::RecordField {
                field: 1,
                part: HirRecordFieldSourcePart::Value,
            },
        })
    );

    for (ordinal, kind) in [(1, "record"), (2, "value"), (3, "missing")] {
        let (_, payload) = module
            .arenas()
            .expressions()
            .try_iter(module.slots())
            .expect("RecordLiteral candidate inventory")
            .find(|(expression, _)| {
                matches!(
                    module
                        .slots()
                        .resolve(*expression)
                        .map(crate::slot::HirSlotMetadata::origin),
                    Ok(HirOrigin::Synthetic(key))
                        if key.owner() == SyntheticOwner::Expr(owner)
                            && key.role()
                                == SyntheticRole::PostfixIndexCandidateExpression
                            && key.ordinal() == ordinal
                )
            })
            .unwrap_or_else(|| panic!("{kind} candidate ordinal {ordinal}"));
        match ordinal {
            1 => assert!(matches!(payload.kind(), HirExprKind::RecordLiteral(_))),
            2 => assert_eq!(payload, expression(&module, *value)),
            3 => assert!(matches!(payload.kind(), HirExprKind::Error(_))),
            _ => unreachable!(),
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this test is one closed nested Dialogue candidate ownership, source-role, and preorder acceptance scenario"
)]
fn nested_dialogue_interpretation_keeps_outer_owner_role_and_global_preorder() {
    let parsed = parsed_source("dialogue-nested-candidate-owner", &["x[#[y]]".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(outer) = expression(&module, owner).kind() else {
        panic!("outer ambiguous postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous {
        index: outer_index,
        dialogue: outer_dialogue,
    } = outer.candidates()
    else {
        panic!("outer interpretations");
    };
    let HirExprKind::Index(outer_index_payload) = expression(&module, *outer_index).kind() else {
        panic!("outer index interpretation");
    };
    let nested_postfix_id = outer_index_payload.index();
    let HirExprKind::PostfixBracket(nested) = expression(&module, nested_postfix_id).kind() else {
        panic!("nested ambiguous postfix");
    };
    let nested_target = nested.target();
    assert!(matches!(
        expression(&module, nested_target).kind(),
        HirExprKind::Error(_)
    ));
    let HirPostfixBracketCandidates::Ambiguous {
        index: nested_index,
        dialogue: nested_dialogue,
    } = nested.candidates()
    else {
        panic!("nested interpretations");
    };
    let HirExprKind::Index(nested_index_payload) = expression(&module, *nested_index).kind() else {
        panic!("nested index interpretation");
    };
    assert!(matches!(
        expression(&module, nested_index_payload.index()).kind(),
        HirExprKind::Path(_)
    ));
    let HirExprKind::DialogueContentApplication(nested_dialogue_payload) =
        expression(&module, *nested_dialogue).kind()
    else {
        panic!("nested Dialogue interpretation");
    };
    assert!(matches!(
        nested_dialogue_payload.content().nodes(),
        [node] if matches!(node.kind(), HirDialogueNodeKind::Text(_))
    ));

    let HirExprKind::DialogueContentApplication(outer_dialogue_payload) =
        expression(&module, *outer_dialogue).kind()
    else {
        panic!("outer Dialogue interpretation");
    };
    let [outer_dialogue_node] = outer_dialogue_payload.content().nodes() else {
        panic!("outer Dialogue has one interpolation");
    };
    let HirDialogueNodeKind::Interpolation(interpolation) = outer_dialogue_node.kind() else {
        panic!("outer Dialogue interpolation");
    };
    assert!(matches!(
        expression(&module, *interpolation).kind(),
        HirExprKind::Path(_)
    ));

    for (expression, ordinal) in [
        (*outer_index, 0),
        (nested_postfix_id, 1),
        (nested_target, 2),
        (*nested_index, 3),
        (nested_index_payload.index(), 4),
        (*nested_dialogue, 5),
    ] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("nested index-role candidate slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }
    for (expression, ordinal) in [(*outer_dialogue, 0), (*interpolation, 1)] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("Dialogue-role candidate slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::DialogueContentCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }

    assert!(expression(&module, *outer_index).is_poisoned());
    assert!(expression(&module, nested_postfix_id).is_poisoned());
    assert!(expression(&module, nested_target).is_poisoned());
    assert!(expression(&module, *nested_index).is_poisoned());
    assert!(expression(&module, *nested_dialogue).is_poisoned());
    assert!(!expression(&module, *outer_dialogue).is_poisoned());
    assert!(!expression(&module, *interpolation).is_poisoned());
}

#[test]
fn dialogue_candidate_condition_tag_keeps_typed_payload_expression() {
    let parsed = parsed_source("dialogue-candidate-condition", &["x[[if y]]".into()]);
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("outer ambiguous postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { dialogue, .. } = postfix.candidates() else {
        panic!("Dialogue interpretation");
    };
    let HirExprKind::DialogueContentApplication(application) =
        expression(&module, *dialogue).kind()
    else {
        panic!("typed Dialogue application");
    };
    let [tag] = application.content().tags() else {
        panic!("one conditional tag");
    };
    let crate::dialogue_application::HirRichTextTagPayload::Condition(condition) = tag.payload()
    else {
        panic!("conditional tag payload");
    };
    assert!(matches!(
        expression(&module, *condition).kind(),
        HirExprKind::Path(_)
    ));
    for (expression, ordinal) in [(*dialogue, 0), (*condition, 1)] {
        let metadata = module
            .slots()
            .resolve(expression)
            .expect("Dialogue candidate slot");
        assert!(matches!(
            metadata.origin(),
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::DialogueContentCandidateExpression
                    && key.ordinal() == ordinal
        ));
    }
    assert!(!expression(&module, *dialogue).is_poisoned());
    assert!(!expression(&module, *condition).is_poisoned());
}

#[test]
fn recovered_ambiguous_index_keeps_candidate_role_for_missing_operand() {
    let parsed = parsed_source(
        "dialogue-ambiguous-recovered-index",
        &["items[left +]".into()],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Recovered);

    let owner = owners[0];
    let HirExprKind::PostfixBracket(postfix) = expression(&module, owner).kind() else {
        panic!("ambiguous generic postfix");
    };
    let HirPostfixBracketCandidates::Ambiguous { index, .. } = postfix.candidates() else {
        panic!("recovered ordinary-index candidate remains viable");
    };
    let HirExprKind::Index(index) = expression(&module, *index).kind() else {
        panic!("ordinary-index candidate root");
    };
    let HirExprKind::Binary(binary) = expression(&module, index.index()).kind() else {
        panic!("recovered binary candidate payload");
    };
    assert!(matches!(
        expression(&module, binary.right()).kind(),
        HirExprKind::Error(_)
    ));
    assert_eq!(
        expression(&module, binary.right()).state(),
        &HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::RightOperand,
        })
    );
    let metadata = module
        .slots()
        .resolve(binary.right())
        .expect("missing candidate operand slot");
    assert!(matches!(
        metadata.origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Expr(owner)
                && key.role() == SyntheticRole::PostfixIndexCandidateExpression
                && key.ordinal() == 3
    ));
    assert!(matches!(
        metadata.source_site(),
        HirSourceSite::Insertion(_)
    ));
}

#[test]
fn ambiguous_candidate_exact_aggregate_descendant_limit_publishes() {
    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let element_count = maximum
        .checked_sub(3)
        .expect("two candidate roots and one tuple root fit the production limit");
    let parsed = parsed_source(
        "dialogue-ambiguous-candidate-descendants-exact",
        &[ambiguous_tuple_candidate(element_count)],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let candidate_descendants = module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .expect("candidate expression inventory")
        .filter(|(expression, _)| {
            matches!(
                module
                    .slots()
                    .resolve(*expression)
                    .map(crate::slot::HirSlotMetadata::origin),
                Ok(HirOrigin::Synthetic(key))
                    if key.owner() == SyntheticOwner::Expr(owner)
                        && matches!(
                            key.role(),
                            SyntheticRole::PostfixIndexCandidateExpression
                                | SyntheticRole::DialogueContentCandidateExpression
                        )
            )
        })
        .count();
    assert_eq!(candidate_descendants, maximum);
}

#[test]
fn ambiguous_candidate_one_over_aggregate_descendant_limit_rolls_back() {
    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let element_count = maximum
        .checked_sub(2)
        .expect("one-over candidate tuple fixture");
    let parsed = parsed_source(
        "dialogue-ambiguous-candidate-descendants-one-over",
        &[ambiguous_tuple_candidate(element_count)],
    );
    let attached = attached_expressions(&parsed)
        .pop()
        .expect("one ambiguous postfix expression");
    let mut database = HirDatabase::try_new().expect("candidate-limit database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::SyntheticDescendantsPerOwner
                && error.observed() == maximum + 1
                && error.maximum() == maximum
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&parsed)).is_none());
}

#[test]
fn ambiguous_typed_candidate_exact_aggregate_descendant_limit_publishes() {
    const CANDIDATE_ROOT_EXPRESSIONS: usize = 2;
    const TUPLE_EXPRESSIONS: usize = 1;
    const ASSOCIATED_CALL_EXPRESSIONS: usize = 2;
    const ASSOCIATED_CALL_TYPES: usize = 2;

    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let fixed_descendants = CANDIDATE_ROOT_EXPRESSIONS
        + TUPLE_EXPRESSIONS
        + ASSOCIATED_CALL_EXPRESSIONS
        + ASSOCIATED_CALL_TYPES;
    let plain_element_count = maximum
        .checked_sub(fixed_descendants)
        .expect("typed candidate fixed descendants fit the production limit");
    assert_eq!(plain_element_count + fixed_descendants, maximum);

    let parsed = parsed_source(
        "dialogue-ambiguous-typed-candidate-descendants-exact",
        &[ambiguous_typed_tuple_candidate(plain_element_count)],
    );
    let (module, owners, _) = lower_and_publish(&parsed);
    assert_eq!(module.status(), HirModuleStatus::Clean);

    let owner = owners[0];
    let candidate_expressions = module
        .arenas()
        .expressions()
        .try_iter(module.slots())
        .expect("typed candidate expression inventory")
        .filter(|(expression, _)| {
            matches!(
                module
                    .slots()
                    .resolve(*expression)
                    .map(crate::slot::HirSlotMetadata::origin),
                Ok(HirOrigin::Synthetic(key))
                    if key.owner() == SyntheticOwner::Expr(owner)
                        && matches!(
                            key.role(),
                            SyntheticRole::PostfixIndexCandidateExpression
                                | SyntheticRole::DialogueContentCandidateExpression
                        )
            )
        })
        .count();
    let mut candidate_type_ordinals = module
        .arenas()
        .types()
        .try_iter(module.slots())
        .expect("typed candidate type inventory")
        .filter_map(|(ty, _)| match module.slots().resolve(ty).ok()?.origin() {
            HirOrigin::Synthetic(key)
                if key.owner() == SyntheticOwner::Expr(owner)
                    && key.role() == SyntheticRole::PostfixIndexCandidateExpression =>
            {
                Some(key.ordinal())
            }
            HirOrigin::Source(_) | HirOrigin::Synthetic(_) => None,
        })
        .collect::<Vec<_>>();
    candidate_type_ordinals.sort_unstable();

    assert_eq!(
        candidate_expressions,
        plain_element_count
            + CANDIDATE_ROOT_EXPRESSIONS
            + TUPLE_EXPRESSIONS
            + ASSOCIATED_CALL_EXPRESSIONS
    );
    assert_eq!(candidate_type_ordinals, [0, 1]);
    assert_eq!(
        candidate_expressions + candidate_type_ordinals.len(),
        maximum
    );
}

#[test]
fn ambiguous_typed_candidate_one_over_aggregate_descendant_limit_rolls_back() {
    const CANDIDATE_ROOT_EXPRESSIONS: usize = 2;
    const TUPLE_EXPRESSIONS: usize = 1;
    const ASSOCIATED_CALL_EXPRESSIONS: usize = 2;
    const ASSOCIATED_CALL_TYPES: usize = 2;

    let maximum = HirLimit::SyntheticDescendantsPerOwner.maximum();
    let fixed_descendants = CANDIDATE_ROOT_EXPRESSIONS
        + TUPLE_EXPRESSIONS
        + ASSOCIATED_CALL_EXPRESSIONS
        + ASSOCIATED_CALL_TYPES;
    let plain_element_count = maximum
        .checked_sub(fixed_descendants)
        .and_then(|count| count.checked_add(1))
        .expect("typed candidate one-over fixture");
    assert_eq!(plain_element_count + fixed_descendants, maximum + 1);

    let parsed = parsed_source(
        "dialogue-ambiguous-typed-candidate-descendants-one-over",
        &[ambiguous_typed_tuple_candidate(plain_element_count)],
    );
    let attached = attached_expressions(&parsed)
        .pop()
        .expect("one typed ambiguous postfix expression");
    let mut database = HirDatabase::try_new().expect("typed candidate-limit database");
    let mut transaction = stage(&database, &parsed);
    let scope = allocate_module_scope(&mut transaction, &parsed);
    assert!(matches!(
        transaction.lower_attached_expression(&attached, scope),
        Err(HirLowerFailure::Limit(error))
            if error.limit() == HirLimit::SyntheticDescendantsPerOwner
                && error.observed() == maximum + 1
                && error.maximum() == maximum
    ));
    assert!(transaction.finish(&mut database).is_err());
    assert!(database.current(&module_key(&parsed)).is_none());
}
