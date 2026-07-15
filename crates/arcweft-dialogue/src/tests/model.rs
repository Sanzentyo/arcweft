use crate::*;
use arcweft_id::{EntityId, PublicId};
use arcweft_ref::{Id, Ref};
use arcweft_source::{SourceAnchor, SourceDocument, SourceDocumentId, SourceName, SourceRange};
use core::time::Duration;

#[test]
fn models_speaker_preset_and_line_plan_out() {
    let alice = SpeakerRef::new(PublicId::try_new("character.alice").expect("valid speaker id"));
    let view = Ref::<View>::new(Id::new(
        EntityId::try_new("view.side").expect("valid view id"),
    ));
    let smile = PublicId::try_new("expression.smile").expect("valid expression id");
    let worried = "worried".to_owned();

    let alice2 = alice.preset(
        SayOptions::empty()
            .with_look(smile.clone())
            .with_voice(VoicePolicy::Auto)
            .with_view(view),
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

    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-generated://dialogue-tests/0")
            .expect("test document id"),
        SourceName::Generated,
        "",
    )
    .expect("test source document");
    let source = SourceAnchor::from_span(
        document
            .span(SourceRange::new(0, 0))
            .expect("empty generated span"),
    );
    let line = DialogueLine::from_preset(&alice2, SayOptions::empty(), content, plan, source);

    assert_eq!(line.speaker().id().as_str(), "character.alice");
    assert_eq!(line.options().look.as_ref(), Some(&smile));
    assert!(matches!(line.options().voice, Some(VoicePolicy::Auto)));
    assert_eq!(line.plan().steps().len(), 4);
    assert!(line.plan().output().is_some());
}
