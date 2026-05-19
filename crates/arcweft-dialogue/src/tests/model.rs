use crate::*;
use arcweft_id::PublicId;
use arcweft_presentation::{
    BackgroundSurface, PresentationSlot, PresentationTarget, SlotValue, asset, bg, clear_bg,
};
use arcweft_source::SourceAnchor;
use core::time::Duration;

#[test]
fn models_speaker_preset_and_line_plan_out() {
    let alice = SpeakerRef::new(PublicId::try_new("character.alice").expect("valid speaker id"));
    let textbox = TextBoxRef::new(PublicId::try_new("textbox.side").expect("valid textbox id"));
    let smile = PublicId::try_new("expression.smile").expect("valid expression id");
    let worried = "worried".to_owned();

    let alice2 = alice.preset(
        SayOptions::empty()
            .with_look(smile.clone())
            .with_voice(VoicePolicy::Auto)
            .with_text_box(textbox),
    );

    let content = DialogueContent::new([
        DialogueContentPart::Text("今日は少しだけ、".to_owned()),
        DialogueContentPart::Ruby {
            base: "変な夢".to_owned(),
            ruby: "へんなゆめ".to_owned(),
        },
        DialogueContentPart::Text("を見たんだ。".to_owned()),
        DialogueContentPart::Tag(DialogueTag::Page),
    ]);

    let actor = LinePlanStep::Let {
        name: "actor".to_owned(),
        expr: PlanExpr::Call(PlanCall {
            receiver: "alice2".to_owned(),
            method: "stage_handle".to_owned(),
            args: Vec::new(),
        }),
    };
    let face0 = LinePlanStep::Let {
        name: "face0".to_owned(),
        expr: PlanExpr::Call(PlanCall {
            receiver: "actor".to_owned(),
            method: "face".to_owned(),
            args: Vec::new(),
        }),
    };
    let face1 = LinePlanStep::Let {
        name: "face1".to_owned(),
        expr: PlanExpr::Name(worried),
    };
    let timed_face = LinePlanStep::At {
        anchor: TimelineAnchor::FromStart(Duration::from_millis(420)),
        step: Box::new(face1),
    };
    let voice = LinePlanStep::Let {
        name: "voice".to_owned(),
        expr: PlanExpr::Call(PlanCall {
            receiver: "line".to_owned(),
            method: "voice_handle".to_owned(),
            args: Vec::new(),
        }),
    };

    let plan = LinePlan::new([actor, face0, timed_face, voice]).with_out(PlanExpr::Tuple(vec![
        PlanExpr::Name("actor".to_owned()),
        PlanExpr::Tuple(vec![
            PlanExpr::Name("face0".to_owned()),
            PlanExpr::Name("face1".to_owned()),
            PlanExpr::Name("voice".to_owned()),
        ]),
    ]));

    let line = DialogueLine::from_preset(
        &alice2,
        SayOptions::empty(),
        content,
        plan,
        SourceAnchor::generated(),
    );

    assert_eq!(line.speaker().id().as_str(), "character.alice");
    assert_eq!(line.options().look.as_ref(), Some(&smile));
    assert!(matches!(line.options().voice, Some(VoicePolicy::Auto)));
    assert_eq!(line.plan().steps().len(), 4);
    assert!(line.plan().output().is_some());
}

#[test]
fn presentation_handles_share_scope_lifetime_and_slots() {
    let line_scope = PresentationScope::line();
    let alice = character("alice");
    let background = bg(asset("bg.room"), line_scope.clone());
    let shown_alice = show(&alice, "smile", line_scope.clone());

    assert_eq!(background.scope(), &line_scope);
    assert_eq!(shown_alice.scope(), &line_scope);
    assert_eq!(background.target().id().as_str(), "target.scene");
    assert_eq!(background.slot().id().as_str(), "slot.background.default");
    assert_eq!(
        shown_alice.slot().id().as_str(),
        "slot.character.alice.default"
    );
    assert_eq!(shown_alice.value().character().as_str(), "character.alice");
    assert_eq!(
        shown_alice.value().expression().map(PublicId::as_str),
        Some("expression.smile")
    );
}

#[test]
fn presentation_slot_value_behaves_like_static_option() {
    let mut slot = SlotValue::empty(
        PresentationTarget::scene(),
        PresentationSlot::default_background(),
    );
    assert!(slot.get().is_none());

    let first = bg(asset("bg.room"), PresentationScope::flow());
    assert!(slot.set(first).is_none());
    assert_eq!(
        slot.get()
            .map(BackgroundSurface::asset)
            .map(PublicId::as_str),
        Some("asset.bg.room")
    );

    let second = bg(asset("bg.evening"), PresentationScope::line());
    let previous = slot.set(second).expect("previous background is returned");
    assert_eq!(previous.asset().as_str(), "asset.bg.room");
    assert_eq!(
        slot.get()
            .map(BackgroundSurface::asset)
            .map(PublicId::as_str),
        Some("asset.bg.evening")
    );

    let cleared = slot.clear().expect("background clears");
    assert_eq!(cleared.asset().as_str(), "asset.bg.evening");
    assert!(slot.get().is_none());
    assert_eq!(
        clear_bg(PresentationScope::line()).slot().id().as_str(),
        "slot.background.default"
    );
    assert_eq!(
        hide(&character("alice"), PresentationScope::line())
            .slot()
            .id()
            .as_str(),
        "slot.character.alice.default"
    );
}
