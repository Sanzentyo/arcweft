use crate::{DialogueContent, DialogueContentPart, DialogueTag};

#[test]
fn parse_lossy_accepts_dialogue_authoring_ruby_forms() {
    let content = DialogueContent::parse_lossy(
        "｜自然《しぜん》 |[明示](めいじ) |短縮{たんしゅく} [rb rt=タグ]形式[/rb][page]",
    );

    assert!(content.parts().iter().any(
        |part| matches!(part, DialogueContentPart::Ruby { base, ruby } if base == "自然" && ruby == "しぜん")
    ));
    assert!(content.parts().iter().any(
        |part| matches!(part, DialogueContentPart::Ruby { base, ruby } if base == "明示" && ruby == "めいじ")
    ));
    assert!(content.parts().iter().any(
        |part| matches!(part, DialogueContentPart::Ruby { base, ruby } if base == "短縮" && ruby == "たんしゅく")
    ));
    assert!(content.parts().iter().any(
        |part| matches!(part, DialogueContentPart::Ruby { base, ruby } if base == "形式" && ruby == "タグ")
    ));
    assert!(matches!(
        content.parts().last(),
        Some(DialogueContentPart::Tag(DialogueTag::Page))
    ));
}
