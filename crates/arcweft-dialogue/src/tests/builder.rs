use crate::*;
use arcweft_id::PublicId;
use core::time::Duration;

#[test]
fn builder_api_builds_dialogue_line_from_concise_call_shape()
-> Result<(), Box<dyn std::error::Error>> {
    let alice = SpeakerPreset::new(character("alice"))
        .voice(VoicePolicy::Auto)
        .look("smile")
        .window(textbox("side"));

    let line = alice
        .say()
        .id(line_id("say.opening.dream_hint"))
        .content(DialogueContent::parse_lossy(
            "今日は少しだけ、｜変な夢《へんなゆめ》を見たんだ。[p]",
        ))
        .at(
            Duration::from_millis(420),
            Cue::Face {
                target: "alice",
                face: "worried",
            },
        )
        .cancel_on(InputEventKind::SkipLine, CancelAction::Continue)
        .build()?;

    assert_eq!(line.speaker().id().as_str(), "character.alice");
    assert_eq!(
        line.options().id.as_ref().map(PublicId::as_str),
        Some("say.opening.dream_hint")
    );
    assert_eq!(
        line.options()
            .text_box
            .as_ref()
            .map(|text_box| text_box.id().as_str()),
        Some("textbox.side")
    );
    assert!(matches!(line.options().voice, Some(VoicePolicy::Auto)));
    assert_eq!(line.content().parts().len(), 4);
    assert!(matches!(
        line.content().parts().get(1),
        Some(DialogueContentPart::Ruby { base, ruby })
            if base == "変な夢" && ruby == "へんなゆめ"
    ));
    assert!(matches!(
        line.content().parts().last(),
        Some(DialogueContentPart::Tag(DialogueTag::Page))
    ));
    assert_eq!(line.plan().cues().len(), 1);
    assert_eq!(
        line.plan().cues()[0].anchor,
        TimelineAnchor::FromStart(Duration::from_millis(420))
    );
    assert!(matches!(
        &line.plan().cues()[0].cue,
        CueAction::Face { target, face } if target == "alice" && face == "worried"
    ));
    assert_eq!(line.plan().cancel_rules().len(), 1);
    assert_eq!(line.plan().cancel_rules()[0].action, CancelAction::Continue);

    Ok(())
}
