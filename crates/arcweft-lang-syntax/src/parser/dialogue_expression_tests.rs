#[cfg(test)]
mod tests {
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use crate::expressions::{
        ExpressionProjection, SyntaxAttachedContentApplicationForm,
        SyntaxDialogueActionArgumentProjection, SyntaxDialogueContentProjection,
        SyntaxDialogueNodeProjection, SyntaxDialoguePointActionPayload, SyntaxExpressionSlot,
    };
    use crate::grammar::build::UnattachedGrammarEntry;
    use crate::grammar::kinds::SyntaxKind;
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
