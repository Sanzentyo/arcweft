use super::*;
use arcweft_text_model::{ResolvedRichTextNode, RichTextColor, RichTextStyleOrigin};

#[test]
fn authored_style_scopes_keep_original_sources_and_cascade_in_native_awbc_and_saved_frames() {
    let source = r##"
pub character alice { display = "Alice" }
flow main() -> Unit {
    alice()[#color(rgb("#202122"))[先#color(rgb("#404142"))[猫]後]外[p]]
}
entry cli @entry.main { goto @flow.main }
"##;
    // This existing integration owner executes both backends, materializes the
    // Content, validates both frames and round-trips each public JSON snapshot
    // against its original program and admitted CharacterDialogue schema.
    let frame = frames_for_authored_dialogue_spacing(source);
    assert_eq!(frame.text, "先猫後外");
    let ResolvedRichTextNode::Scope {
        style,
        origin,
        body,
    } = &frame.nodes[0]
    else {
        panic!("outer source-owned style scope")
    };
    let outer_style = RichTextStyle::Color {
        value: RichTextColor::Rgba8 {
            value: [32, 33, 34, 255],
        },
    };
    assert_eq!(style.as_ref(), &outer_style);
    let RichTextStyleOrigin::Authored { source: outer } = origin else {
        panic!("outer scope retains its original checked source")
    };
    let ResolvedRichTextNode::Scope { style, origin, .. } = &body[1] else {
        panic!("nested override remains structurally nested")
    };
    let inner_style = RichTextStyle::Color {
        value: RichTextColor::Rgba8 {
            value: [64, 65, 66, 255],
        },
    };
    assert_eq!(style.as_ref(), &inner_style);
    let RichTextStyleOrigin::Authored { source: inner } = origin else {
        panic!("inner scope retains its original checked source")
    };
    assert_eq!(outer.source(), inner.source());
    assert_eq!(
        outer.source().source_len(),
        u64::try_from(source.len()).unwrap()
    );
    let original = |origin: &arcweft_text_model::RichTextStyleSource| {
        source
            .get(usize::try_from(origin.start()).unwrap()..usize::try_from(origin.end()).unwrap())
            .unwrap()
    };
    assert!(original(outer).contains("#202122"));
    assert!(original(inner).contains("#404142"));
    assert!(outer.start() < inner.start());
    let runs = &frame.display_map.text_runs;
    assert_eq!(runs.len(), 4);
    for (run, expected) in runs.iter().zip([
        vec![outer_style.clone()],
        vec![outer_style.clone(), inner_style],
        vec![outer_style],
        Vec::new(),
    ]) {
        assert_eq!(run.styles, expected);
    }
}
