use super::{compilation_state, compile_project, removed_role_dialogue_project};
use arcweft_text_model::{
    Milli, RichTextLayoutUpdate, RichTextNode, RichTextRubyPosition, RichTextStyle,
    RichTextWritingMode, presentation_from_styles,
};

#[test]
fn source_layout_preserves_omission_and_explicit_default_contributions() {
    fn visit(nodes: &[RichTextNode], updates: &mut Vec<RichTextLayoutUpdate>) {
        for node in nodes {
            if let RichTextNode::Scope { style, body } = node {
                if let RichTextStyle::Layout { layout } = style.as_ref() {
                    updates.push(layout.clone());
                }
                visit(body, updates);
            }
        }
    }

    for (inner, expected) in [
        (
            ".ruby_under, ruby_size=11px",
            RichTextLayoutUpdate {
                ruby_position: Some(RichTextRubyPosition::Under),
                ruby_font_size: Some(Milli(11_000)),
                ..RichTextLayoutUpdate::default()
            },
        ),
        (
            ".horizontal_tb, column_gap=8px, ruby_gap=0px",
            RichTextLayoutUpdate {
                writing_mode: Some(RichTextWritingMode::HorizontalTb),
                column_gap: Some(Milli(8_000)),
                ruby_gap: Some(Milli::ZERO),
                ..RichTextLayoutUpdate::default()
            },
        ),
    ] {
        let source = format!(
            "pub character alice {{ display = \"Alice\" }}\nflow main() {{ alice[#layout(.vertical_rl, column_gap=15px, ruby_gap=3px)[#layout({inner})[夢]][p]]; }}"
        );
        let (project, context) = removed_role_dialogue_project(&source);
        let (mut session, parsed) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed, &context).unwrap();
        let templates = compiled.runtime_plan().dialogue_content_catalog.templates();
        let mut updates = Vec::new();
        for template in templates {
            visit(&template.content().nodes, &mut updates);
        }
        let [outer, actual] = updates.as_slice() else {
            panic!("exactly two authored layout contributions: {updates:?}");
        };
        assert_eq!(actual, &expected, "{inner}");
        assert_eq!(outer.writing_mode, Some(RichTextWritingMode::VerticalRl));
        assert_eq!(outer.column_gap, Some(Milli(15_000)));
        assert_eq!(outer.ruby_gap, Some(Milli(3_000)));
        let styles = updates
            .into_iter()
            .map(|layout| RichTextStyle::Layout { layout })
            .collect::<Vec<_>>();
        let resolved = presentation_from_styles(&styles).layout.unwrap();
        if expected.writing_mode.is_none() {
            assert_eq!(resolved.writing_mode, RichTextWritingMode::VerticalRl);
            assert_eq!(resolved.column_gap, Milli(15_000));
            assert_eq!(resolved.ruby_gap, Some(Milli(3_000)));
            assert_eq!(resolved.ruby_position, RichTextRubyPosition::Under);
        } else {
            assert_eq!(resolved.writing_mode, RichTextWritingMode::HorizontalTb);
            assert_eq!(resolved.column_gap, Milli(8_000));
            assert_eq!(resolved.ruby_gap, Some(Milli::ZERO));
        }
    }
}
