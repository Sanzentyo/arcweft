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

    #[test]
    fn nested_content_apostrophes_preserve_typed_bodies_and_lossless_source() {
        for content in [
            "#layout(.vertical_rl)[天O'K人][p]",
            "#outer()[\"]\"][p]",
            "#outer()[#inner()[O'K]][p]",
            "#outer()[#[\"a]b\"]][p]",
            "#outer()[[signal .warning note='a]b']][p]",
            "#raw()[O'K #unfinished(][p]",
            "#outer()[#raw()[O'K #unfinished(]][p]",
            "#outer()[#inner()[O'K] #[\"a]b\"] [signal .warning note='a]b']][p]",
        ] {
            let source = format!("flow opening {{\n    alice: {content}\n}}\n");
            let built =
                parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();
            assert!(
                built.diagnostics().is_empty(),
                "{content}: {:?}",
                built.diagnostics()
            );
            assert_eq!(built.green().to_string(), source);
            let nested = applications(&built);
            assert!(nested.len() >= 2);
            assert!(nested.iter().all(|application| !matches!(
                application.content(),
                SyntaxDialogueContentProjection::Missing { .. }
            )));
            if content == "#raw()[O'K #unfinished(][p]"
                || content == "#outer()[#raw()[O'K #unfinished(]][p]"
            {
                assert!(nested.iter().any(|application| matches!(
                    application.content(), SyntaxDialogueContentProjection::RawLiteral(literal)
                        if literal.value() == "O'K #unfinished("
                )));
            }
        }
    }

    #[test]
    fn explicit_attached_bodies_admit_numerals_and_ascii_punctuation() {
        for text in ["2026", "123", "...", "!?", "2026/10/09"] {
            let source =
                format!("flow opening {{\n    alice: #layout(.vertical_rl)[{text}][p]\n}}\n");
            let built =
                parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();
            assert!(
                built.diagnostics().is_empty(),
                "{text}: {:?}",
                built.diagnostics()
            );
            assert_eq!(built.green().to_string(), source);
            assert!(applications(&built).iter().any(|application| matches!(application.content(),
                SyntaxDialogueContentProjection::Present(content)
                    if matches!(content.nodes(), [SyntaxDialogueNodeProjection::Text(actual)] if actual.as_ref() == text)
            )), "explicit body must retain its exact text: {text}");
        }
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
    fn bare_dialogue_plans_classify_only_the_dialogue_head() {
        for (document_id, dialogue) in [
            (
                "memory:bare-colon-dialogue-plan-classification",
                "    alice(voice=auto):\n        聞いて。[p]\n",
            ),
            (
                "memory:bare-bracket-dialogue-plan-classification",
                "    alice(voice=auto)[聞いて。[p]]\n",
            ),
        ] {
            let source = format!(
                "flow opening() -> String {{\n{dialogue}    with:\n        let actor = alice.stage.acquire(scope=line)\n        out \"Done\"\n    return \"done\"\n}}\n"
            );
            let document = SourceDocument::try_new(
                SourceDocumentId::try_new(document_id).unwrap(),
                SourceName::Memory,
                source.as_str(),
            )
            .unwrap();
            let built = parse_document(&document, crate::parser::ParseOptions::default())
                .expect("bare Dialogue with an aligned line plan parses");
            assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
            assert_eq!(built.green().to_string(), source);

            let dialogue_applications = applications(&built);
            let [application] = dialogue_applications.as_slice() else {
                panic!("bare Dialogue must remain one attached content application");
            };
            assert!(application.has_plan());
            assert!(!built.index().entries().iter().any(|entry| {
                entry.kind() == SyntaxKind::AssignmentStatement
                    && entry.role() == SyntaxRole::ThreadFlowItem(0)
            }));
            assert!(built.index().entries().iter().any(|entry| {
                entry.kind() == SyntaxKind::LetStatement
                    && entry.role() == SyntaxRole::DialogueLinePlanItem(0)
            }));
            assert!(built.index().entries().iter().any(|entry| {
                entry.kind() == SyntaxKind::OutStatement
                    && entry.role() == SyntaxRole::DialogueLinePlanItem(1)
            }));
            assert!(built.index().entries().iter().any(|entry| {
                entry.kind() == SyntaxKind::ReturnStatement
                    && entry.role() == SyntaxRole::ThreadFlowItem(1)
            }));
        }
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
    fn explicit_line_plan_owns_an_otherwise_ambiguous_bracket_payload() {
        for (content, ambiguous) in [("Ready[p]", true), ("value", true), ("2026", false)] {
            let bare = format!("flow opening {{ alice()[{content}] }}\n");
            let bare_build =
                parse_document(&document(&bare), crate::parser::ParseOptions::default()).unwrap();
            assert!(bare_build.diagnostics().is_empty());
            assert!(
                bare_build.index().entries().iter().any(|entry| {
                    match entry
                        .expression_projection()
                        .map(PendingExpressionProjection::projection)
                    {
                        Some(ExpressionProjection::PostfixBracket(
                            crate::expressions::SyntaxPostfixBracketProjection::Ambiguous {
                                ..
                            },
                        )) => ambiguous,
                        Some(ExpressionProjection::Index(_)) => !ambiguous,
                        _ => false,
                    }
                }),
                "{content}: {:?}",
                bare_build
                    .index()
                    .entries()
                    .iter()
                    .filter_map(|entry| entry.expression_projection())
                    .collect::<Vec<_>>()
            );

            let source = format!(
                "flow opening {{\n    alice()[{content}] with {{ init {{ log.info(\"ready\") }} }}\n    let following = true\n}}\n"
            );
            let built =
                parse_document(&document(&source), crate::parser::ParseOptions::default()).unwrap();
            assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
            assert_eq!(built.green().to_string(), source);
            let attached = applications(&built);
            assert_eq!(attached.len(), 1);
            assert!(attached[0].has_plan());
            assert!(!attached[0].has_recovery());
            assert!(
                built
                    .index()
                    .entries()
                    .iter()
                    .any(|entry| { entry.kind() == SyntaxKind::LetStatement })
            );
            assert!(
                !built
                    .index()
                    .entries()
                    .iter()
                    .any(|entry| { entry.kind() == SyntaxKind::ErrorExpression })
            );
        }
    }

    #[test]
    fn explicit_line_plan_missing_body_keeps_typed_recovery_and_following_declaration() {
        let source = "flow opening {\n    alice()[Ready[p]] with\n}\nfn following() { () }\n";
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert_eq!(built.green().to_string(), source);
        let attached = applications(&built);
        assert_eq!(attached.len(), 1);
        assert!(attached[0].has_plan());
        assert!(
            built
                .index()
                .entries()
                .iter()
                .any(|entry| entry.kind() == SyntaxKind::MissingBody)
        );
        assert!(
            built
                .index()
                .entries()
                .iter()
                .any(|entry| entry.kind() == SyntaxKind::FunctionItem)
        );
        assert!(
            built
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.range().is_empty())
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

    fn authored_space_owner(
        built: &crate::grammar::build::GrammarBuild,
    ) -> &PendingExpressionProjection {
        built
            .index()
            .entries()
            .iter()
            .filter_map(UnattachedGrammarEntry::expression_projection)
            .find(|projection| {
                matches!(
                    projection.projection(),
                    ExpressionProjection::AttachedContentApplication(application)
                        if matches!(application.content(), SyntaxDialogueContentProjection::Present(content)
                            if content.nodes().iter().any(|node| matches!(node,
                                SyntaxDialogueNodeProjection::Text(text) if text.as_ref() == "Head ")))
                )
            })
            .expect("the authored Head text selects its exact content owner")
    }

    fn assert_authored_text_spans(
        source: &str,
        owner: &PendingExpressionProjection,
        expected: &[&str],
    ) {
        use crate::expressions::SyntaxDialogueNodeSourcePart;

        let ExpressionProjection::AttachedContentApplication(application) = owner.projection()
        else {
            panic!("the selected projection owns attached content")
        };
        let SyntaxDialogueContentProjection::Present(content) = application.content() else {
            panic!("the selected body is present")
        };
        let texts: Vec<_> = content
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, node)| match node {
                SyntaxDialogueNodeProjection::Text(text) => Some((ordinal, text.as_ref())),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts.iter().map(|(_, text)| *text).collect::<Vec<_>>(),
            expected
        );
        for (ordinal, text) in texts {
            let ordinal = u32::try_from(ordinal).expect("the fixture node ordinal fits u32");
            for part in [
                SyntaxDialogueNodeSourcePart::Whole,
                SyntaxDialogueNodeSourcePart::Text,
            ] {
                let range = owner
                    .components()
                    .iter()
                    .find(|component| {
                        component.role() == ExpressionComponentRole::DialogueNode { ordinal, part }
                    })
                    .expect("each Text retains both owning source roles")
                    .range();
                assert_eq!(&source[range.as_range()], text);
                assert_eq!(range.end() - range.start(), text.len());
            }
        }
    }

    #[test]
    fn dialogue_inline_spaces_after_atoms_keep_exact_text_spans_and_lossless_source() {
        let source = "flow opening {\n    alice: Head #[name] \t|[夢](ゆめ) #strong()[body][r] 2026 ABC。\n}\n";
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(built.green().to_string(), source);
        let owner = authored_space_owner(&built);
        assert_authored_text_spans(source, owner, &["Head ", " \t", " ", " 2026 ABC。"]);

        let replayed = parse_document(
            &document(&built.green().to_string()),
            crate::parser::ParseOptions::default(),
        )
        .unwrap();
        assert!(
            replayed.diagnostics().is_empty(),
            "{:?}",
            replayed.diagnostics()
        );
        assert_eq!(
            authored_space_owner(&replayed).projection(),
            owner.projection()
        );
        assert_eq!(
            authored_space_owner(&replayed).components(),
            owner.components()
        );
    }

    #[test]
    fn dialogue_physical_newlines_own_indentation_and_original_line_ending_spans() {
        use crate::expressions::{SyntaxDialogueNodeSourcePart, SyntaxLineBreakKind};

        let source = "flow opening {\n    alice: #strong()[ \tHead #[name]  \r\n\t  Next[r] \t2026 \r  Third\n    Last]\n}\n";
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(built.green().to_string(), source);
        let owner = authored_space_owner(&built);
        assert_authored_text_spans(
            source,
            owner,
            &["Head ", "  ", "Next", " \t2026 ", "Third", "Last"],
        );
        let ExpressionProjection::AttachedContentApplication(application) = owner.projection()
        else {
            panic!("the selected projection owns attached content")
        };
        let SyntaxDialogueContentProjection::Present(content) = application.content() else {
            panic!("the selected body is present")
        };
        let endings: Vec<_> = content
            .nodes()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, node)| match node {
                SyntaxDialogueNodeProjection::LineBreak(kind) => {
                    assert_eq!(*kind, SyntaxLineBreakKind::Line);
                    let ordinal = u32::try_from(ordinal).unwrap();
                    let range = owner
                        .components()
                        .iter()
                        .find(|component| {
                            component.role()
                                == ExpressionComponentRole::DialogueNode {
                                    ordinal,
                                    part: SyntaxDialogueNodeSourcePart::LineBreak,
                                }
                        })
                        .unwrap()
                        .range();
                    Some(&source[range.as_range()])
                }
                _ => None,
            })
            .collect();
        assert_eq!(endings, ["\r\n", "\r", "\n"]);
    }

    #[test]
    fn inline_space_preservation_keeps_atom_boundary_comments_as_lossless_trivia() {
        let source = "flow opening {\n    alice: Head #[name] \t/* hidden */ tail[r] \t/** documentation */done\n}\n";
        let built =
            parse_document(&document(source), crate::parser::ParseOptions::default()).unwrap();
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(built.green().to_string(), source);
        assert_authored_text_spans(
            source,
            authored_space_owner(&built),
            &["Head ", " \t", " tail", " \t", "done"],
        );
    }

    #[test]
    fn dialogue_margin_only_body_stays_missing_and_raw_whitespace_stays_opaque() {
        use crate::expressions::SyntaxDialogueContentRecoveryBoundary;

        let missing = "flow opening {\n    alice: #strong()[ \t]\n    let following = true\n}\n";
        let built =
            parse_document(&document(missing), crate::parser::ParseOptions::default()).unwrap();
        assert_eq!(built.green().to_string(), missing);
        assert!(applications(&built).iter().any(|application| {
            matches!(application.content(), SyntaxDialogueContentProjection::Missing {
                boundary: SyntaxDialogueContentRecoveryBoundary::CloseBracket { range }
            } if &missing[range.as_range()] == "]")
        }));
        assert!(
            built
                .index()
                .entries()
                .iter()
                .any(|entry| entry.kind() == SyntaxKind::LetStatement)
        );

        let raw = "flow opening {\n    alice: #raw()[ \t#[value][r]\r\n  literal]\n}\n";
        let built = parse_document(&document(raw), crate::parser::ParseOptions::default()).unwrap();
        assert!(built.diagnostics().is_empty(), "{:?}", built.diagnostics());
        assert_eq!(built.green().to_string(), raw);
        assert!(applications(&built).iter().any(|application| {
            matches!(application.content(), SyntaxDialogueContentProjection::RawLiteral(literal)
                if literal.value() == " \t#[value][r]\r\n  literal"
                    && &raw[literal.range().as_range()] == literal.value())
        }));
    }
}
