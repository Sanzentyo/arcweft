use super::{compilation_state, compile_project, removed_role_dialogue_project};
use arcweft_text_model::{
    CheckedLength, RichTextLayoutUpdate, RichTextNode, RichTextRubyPosition, RichTextStyle,
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
                ruby_font_size: Some(CheckedLength::px(11_000)),
                ..RichTextLayoutUpdate::default()
            },
        ),
        (
            ".horizontal_tb, column_gap=8px, ruby_gap=0px",
            RichTextLayoutUpdate {
                writing_mode: Some(RichTextWritingMode::HorizontalTb),
                column_gap: Some(CheckedLength::px(8_000)),
                ruby_gap: Some(CheckedLength::px(0)),
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
        assert_eq!(outer.column_gap, Some(CheckedLength::px(15_000)));
        assert_eq!(outer.ruby_gap, Some(CheckedLength::px(3_000)));
        let styles = updates
            .into_iter()
            .map(|layout| RichTextStyle::Layout { layout })
            .collect::<Vec<_>>();
        let resolved = presentation_from_styles(&styles).layout.unwrap();
        if expected.writing_mode.is_none() {
            assert_eq!(resolved.writing_mode, RichTextWritingMode::VerticalRl);
            assert_eq!(resolved.column_gap, CheckedLength::px(15_000));
            assert_eq!(resolved.ruby_gap, Some(CheckedLength::px(3_000)));
            assert_eq!(resolved.ruby_position, RichTextRubyPosition::Under);
        } else {
            assert_eq!(resolved.writing_mode, RichTextWritingMode::HorizontalTb);
            assert_eq!(resolved.column_gap, CheckedLength::px(8_000));
            assert_eq!(resolved.ruby_gap, Some(CheckedLength::px(0)));
        }
    }
}

#[test]
fn source_font_and_ruby_lengths_retain_units_in_materialized_templates() {
    fn visit(nodes: &[RichTextNode], lengths: &mut Vec<CheckedLength>) {
        for node in nodes {
            if let RichTextNode::Scope { style, body } = node {
                match style.as_ref() {
                    RichTextStyle::Size { length } => lengths.push(*length),
                    RichTextStyle::Layout { layout } => lengths.extend(layout.ruby_font_size),
                    _ => {}
                }
                visit(body, lengths);
            }
        }
    }
    for (font, ruby, expected) in [
        (
            "18pt",
            "9pt",
            [CheckedLength::pt(18_000), CheckedLength::pt(9_000)],
        ),
        (
            "24px",
            "12px",
            [CheckedLength::px(24_000), CheckedLength::px(12_000)],
        ),
    ] {
        let source = format!(
            "pub character alice {{ display = \"Alice\" }}\nflow main() {{ alice[#size({font})[#layout(.ruby_under, ruby_size={ruby})[夢]][p]]; }}"
        );
        let (project, context) = removed_role_dialogue_project(&source);
        let (mut session, parsed) = compilation_state(&project);
        let compiled = compile_project(&mut session, &project, &parsed, &context).unwrap();
        let mut lengths = Vec::new();
        for template in compiled.runtime_plan().dialogue_content_catalog.templates() {
            visit(&template.content().nodes, &mut lengths);
        }
        assert_eq!(lengths, expected, "{source}");
    }
}
