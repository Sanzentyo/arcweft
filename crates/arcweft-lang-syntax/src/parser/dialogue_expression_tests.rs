#[cfg(test)]
mod tests {
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use crate::expressions::{
        ExpressionComponentRole, ExpressionProjection, PendingExpressionProjection,
        SyntaxAttachedContentApplicationForm, SyntaxCallProjection, SyntaxClosureProjection,
        SyntaxClosureSyntax, SyntaxClosureTerminator, SyntaxDialogueActionArgumentProjection,
        SyntaxDialogueContentProjection, SyntaxDialogueNodeProjection,
        SyntaxDialoguePointActionPayload, SyntaxExpressionSlot,
    };
    use crate::grammar::build::UnattachedGrammarEntry;
    use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
    use crate::parser::parse_document;

    fn document(source: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:dialogue-expression-final").unwrap(),
            SourceName::Memory,
            source,
        )
        .unwrap()
    }

    fn applications<'a>(
        built: &'a crate::grammar::build::GrammarBuild,
    ) -> Vec<&'a crate::expressions::SyntaxAttachedContentApplicationProjection> {
        built
            .index()
            .entries()
            .iter()
            .filter_map(UnattachedGrammarEntry::expression_projection)
            .filter_map(|projection| match projection.projection() {
                ExpressionProjection::AttachedContentApplication(application) => Some(application),
                _ => None,
            })
            .collect()
    }

    fn callback_call_projections(
        built: &crate::grammar::build::GrammarBuild,
    ) -> Vec<&PendingExpressionProjection> {
        built
            .index()
            .entries()
            .iter()
            .filter_map(UnattachedGrammarEntry::expression_projection)
            .filter(|projection| {
                matches!(
                    projection.projection(),
                    ExpressionProjection::Call(SyntaxCallProjection::CallbackBlock(_))
                )
            })
            .collect()
    }

    fn indented_callback_closures(
        built: &crate::grammar::build::GrammarBuild,
    ) -> Vec<&SyntaxClosureProjection> {
        built
            .index()
            .entries()
            .iter()
            .filter(|entry| entry.role() == SyntaxRole::Argument(0))
            .filter_map(UnattachedGrammarEntry::expression_projection)
            .filter_map(|projection| match projection.projection() {
                ExpressionProjection::Closure(closure)
                    if matches!(
                        closure.syntax(),
                        SyntaxClosureSyntax::IndentedCallback { .. }
                    ) =>
                {
                    Some(closure)
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn ordinary_dialogue_forms_keep_typed_body_applications() {
        let bracket_source = "flow opening {\n    let handles = alice()[本文です。[p]]\n}\n";
        let built = parse_document(
            &document(bracket_source),
            crate::parser::ParseOptions::default(),
        )
        .expect("ordinary bracket dialogue parses");
        let bracket = applications(&built);
        assert_eq!(bracket.len(), 1);
        assert!(matches!(
            bracket[0].form(),
            SyntaxAttachedContentApplicationForm::Bracket { .. }
        ));
        assert_eq!(built.green().to_string(), bracket_source);

        let colon_source = "flow opening {\n    alice: inline\n}\n";
        let built = parse_document(
            &document(colon_source),
            crate::parser::ParseOptions::default(),
        )
        .expect("ordinary colon dialogue parses");
        let colon = applications(&built);
        assert_eq!(colon.len(), 1);
        assert!(matches!(
            colon[0].form(),
            SyntaxAttachedContentApplicationForm::Colon
        ));
        assert_eq!(built.green().to_string(), colon_source);
    }

    #[test]
    fn bracket_dialogue_with_indented_plan_keeps_its_head_distinct_from_with_colon() {
        let source = "flow opening {\n    alice()[本文。[mark @.release][p]]\n    with:\n        on mark(@.release) => log.info(\"released\")\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("bracket dialogue with an indented line plan parses");
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        let applications = applications(&built);
        assert_eq!(applications.len(), 1);
        assert!(matches!(
            applications[0].form(),
            SyntaxAttachedContentApplicationForm::Bracket { .. }
        ));
        assert!(applications[0].has_plan());
        assert_eq!(built.green().to_string(), source);
    }

    #[test]
    fn bracket_dialogue_followed_by_bare_scope_has_two_flow_items() {
        let source = "flow opening {\n    alice()[本文。[p]] { log.info(\"after\") }\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("bracket dialogue and bare scope parse");
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(applications(&built).len(), 1);
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::ScopeStatement)
                .count(),
            1
        );
        assert_eq!(built.green().to_string(), source);
    }

    #[test]
    fn for_over_bracket_expression_keeps_its_braced_body() {
        let source = "flow opening {\n    for value in [true, false] { log.info(value) }\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("for loop with a bracket iterable parses");
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::ForStatement)
                .count(),
            1
        );
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::ScopeStatement)
                .count(),
            0
        );
        assert_eq!(built.green().to_string(), source);
    }

    #[test]
    fn explicit_with_braces_remains_a_dialogue_plan() {
        let source = "flow opening {\n    alice()[本文。[p]] with { out () }\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("braced line plan parses");
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        let applications = applications(&built);
        assert_eq!(applications.len(), 1);
        assert!(applications[0].has_plan());
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::ScopeStatement)
                .count(),
            0
        );
    }

    #[test]
    fn inline_timed_cue_uses_the_indented_callback_graph_and_keeps_its_sibling() {
        let source = concat!(
            "flow opening {\n",
            "    alice()[本文。[p]]\n",
            "    with:\n",
            "        at(0.42s): alice.stage.look(smile)\n",
            "        let following = true\n",
            "}\n",
        );
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("inline timed cue parses");
        assert_eq!(built.green().to_string(), source);

        let applications = applications(&built);
        assert_eq!(applications.len(), 1);
        assert!(applications[0].has_plan());

        let calls = callback_call_projections(&built);
        let [call] = calls.as_slice() else {
            panic!("inline timed cue owns one callback-block Call projection");
        };
        let ExpressionProjection::Call(SyntaxCallProjection::CallbackBlock(call)) =
            call.projection()
        else {
            panic!("inline timed cue retains the callback-block Call family");
        };
        assert_eq!(call.callback(), SyntaxExpressionSlot::Authored);
        assert_eq!(
            call.terminator(),
            crate::expressions::SyntaxCallArgumentListTerminator::Closed
        );

        let closures = indented_callback_closures(&built);
        let [closure] = closures.as_slice() else {
            panic!("inline timed cue owns one indented callback Closure projection");
        };
        assert_eq!(
            closure.syntax(),
            SyntaxClosureSyntax::IndentedCallback {
                terminator: SyntaxClosureTerminator::Closed,
            }
        );
        assert_eq!(closure.body(), SyntaxExpressionSlot::Authored);

        let entries = built.index().entries();
        assert!(entries.iter().any(|entry| {
            entry.kind() == SyntaxKind::ExpressionStatement
                && entry.role() == SyntaxRole::DialogueLinePlanItem(0)
        }));
        assert!(entries.iter().any(|entry| {
            entry.kind() == SyntaxKind::LetStatement
                && entry.role() == SyntaxRole::DialogueLinePlanItem(1)
        }));
    }

    #[test]
    fn inline_timed_cue_empty_body_and_malformed_tail_recover_before_the_next_item() {
        let source = concat!(
            "flow opening {\n",
            "    alice()[本文。[p]]\n",
            "    with:\n",
            "        at(0.42s): alice.stage.look(smile) trailing\n",
            "        at(0.84s):\n",
            "        let following = true\n",
            "}\n",
        );
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("malformed and empty timed cues retain typed syntax");
        assert_eq!(built.green().to_string(), source);

        let calls = callback_call_projections(&built);
        assert_eq!(calls.len(), 2);
        let closures = indented_callback_closures(&built);
        assert_eq!(closures.len(), 2);
        assert_eq!(closures[0].body(), SyntaxExpressionSlot::Authored);
        assert_eq!(closures[1].body(), SyntaxExpressionSlot::Missing);
        assert!(built.index().entries().iter().any(|entry| {
            entry.kind() == SyntaxKind::MissingExpression && entry.role() == SyntaxRole::Body
        }));

        let recovery = built
            .index()
            .entries()
            .iter()
            .filter_map(UnattachedGrammarEntry::expression_projection)
            .flat_map(|projection| projection.components())
            .find(|component| component.role() == ExpressionComponentRole::Recovery)
            .expect("malformed timed-cue body retains an exact recovery component")
            .range();
        assert_eq!(&source[recovery.start()..recovery.end()], " trailing");

        assert!(built.index().entries().iter().any(|entry| {
            entry.kind() == SyntaxKind::LetStatement
                && entry.role() == SyntaxRole::DialogueLinePlanItem(2)
        }));
    }

    #[test]
    fn retained_ruby_is_a_typed_dialogue_node_surface() {
        let source = "flow opening {\n    alice[｜漢字《かんじ》|[base](reading)]\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("ruby surface parses");
        let application = applications(&built)
            .into_iter()
            .find(|application| {
                matches!(
                    application.content(),
                    SyntaxDialogueContentProjection::Present(content)
                        if content.nodes().iter().any(|node| matches!(node, SyntaxDialogueNodeProjection::Ruby { .. }))
                )
            })
            .expect("ruby node remains in typed content");
        assert!(matches!(
            application.content(),
            SyntaxDialogueContentProjection::Present(_)
        ));
        assert_eq!(built.green().to_string(), source);
    }

    #[test]
    fn raw_call_body_is_opaque_in_the_syntax_projection() {
        let source = "flow opening {\n    alice[#raw()[a[b][p]]]\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("raw call parses");
        let application = applications(&built)
            .into_iter()
            .find(|application| {
                matches!(
                    application.content(),
                    SyntaxDialogueContentProjection::RawLiteral(literal)
                        if literal.value() == "a[b][p]"
                )
            })
            .expect("raw call owns opaque literal body");
        assert!(matches!(
            application.content(),
            SyntaxDialogueContentProjection::RawLiteral(_)
        ));
        assert_eq!(built.green().to_string(), source);
    }

    #[test]
    fn raw_call_spacing_keeps_the_typed_body_opaque() {
        let source = "flow opening {\n    alice[#raw ()[a[b][p]]]\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("spaced raw call parses");
        let application = applications(&built)
            .into_iter()
            .find(|application| {
                matches!(
                    application.content(),
                    SyntaxDialogueContentProjection::RawLiteral(literal)
                        if literal.value() == "a[b][p]"
                )
            })
            .expect("spaced raw call owns opaque literal body");
        assert!(matches!(
            application.content(),
            SyntaxDialogueContentProjection::RawLiteral(_)
        ));
        assert_eq!(built.green().to_string(), source);
    }

    #[test]
    fn non_raw_content_calls_still_parse_nested_dialogue_content() {
        let source = "flow opening {\n    alice[#obj()[#[value]]]\n}\n";
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("ordinary content call parses");
        let application = applications(&built)
            .into_iter()
            .find(|application| {
                matches!(
                    application.content(),
                    SyntaxDialogueContentProjection::Present(content)
                        if content.nodes().iter().any(|node| {
                            matches!(node, SyntaxDialogueNodeProjection::Interpolation(_))
                        })
                )
            })
            .expect("ordinary content call reparses its nested body");
        assert!(matches!(
            application.content(),
            SyntaxDialogueContentProjection::Present(_)
        ));
        assert_eq!(built.green().to_string(), source);
    }

    #[test]
    fn point_actions_keep_one_typed_call_payload_and_timed_cue_duration() {
        let source = concat!(
            "flow opening {\n",
            "    let line = alice[本文。[call log.info(\"content\")] [at 120ms call=log.info(\"delay\")]]\n",
            "}\n",
        );
        let built = parse_document(&document(source), crate::parser::ParseOptions::default())
            .expect("point-action payloads parse");
        let application = applications(&built)
            .into_iter()
            .next()
            .expect("one dialogue application");
        let SyntaxDialogueContentProjection::Present(content) = application.content() else {
            panic!("point actions retain dialogue content");
        };
        let actions = content
            .nodes()
            .iter()
            .filter_map(|node| match node {
                SyntaxDialogueNodeProjection::PointAction(action) => Some(action),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [call, timed] = actions.as_slice() else {
            panic!("call and timed cue remain two point actions");
        };
        assert!(matches!(
            call.payload(),
            SyntaxDialoguePointActionPayload::Call(SyntaxExpressionSlot::Authored)
        ));
        assert!(call.arguments().is_empty());
        assert!(matches!(
            timed.payload(),
            SyntaxDialoguePointActionPayload::TimedCue(SyntaxExpressionSlot::Authored)
        ));
        assert!(matches!(
            timed.arguments(),
            [SyntaxDialogueActionArgumentProjection::Positional { value }]
                if value.decoded() == "120ms"
        ));
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::DialogueActionTimedCuePayload)
                .count(),
            1
        );
        assert_eq!(
            built
                .index()
                .entries()
                .iter()
                .filter(|entry| entry.kind() == SyntaxKind::DialogueActionDialogueCallPayload)
                .count(),
            2
        );
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(built.green().to_string(), source);
    }
}
