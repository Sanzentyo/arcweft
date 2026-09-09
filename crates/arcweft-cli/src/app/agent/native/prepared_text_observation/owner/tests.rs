use super::*;
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::PreparedTextId;

fn dialogue_owner(index: u32, entry: u64, role: DialoguePreparedTextRole) -> PreparedTextOwner {
    PreparedTextOwner::new(
        PreparedTextId::from_index(index),
        arcweft_id::PublicId::try_new(format!("view.dialogue.text{index}")).unwrap(),
        PreparedTextOwnerKind::DialogueView {
            dialogue: 7,
            entry,
            mount: 11,
            role,
        },
        0,
        HitRect::new(0.0, 0.0, 10.0, 10.0),
    )
}

#[test]
fn dialogue_children_resolve_to_content_independently_of_prepared_order() {
    let speaker = dialogue_owner(1, 9, DialoguePreparedTextRole::CharacterDisplayName);
    let content = dialogue_owner(2, 9, DialoguePreparedTextRole::Content);
    let other_entry = dialogue_owner(3, 90, DialoguePreparedTextRole::Content);
    for owners in [
        [speaker.clone(), content.clone(), other_entry.clone()],
        [other_entry.clone(), content.clone(), speaker.clone()],
    ] {
        let selected = PreparedTextObservationOwner::select(&owners, |owner| {
            owner.contains_object("object.dialogue.7.9.ruby.0")
        })
        .unwrap();
        assert_eq!(selected.prepared().text, content.text);
        assert_eq!(selected.root_id(), "object.dialogue.7.9");
    }
    let owners = [speaker, other_entry];
    assert!(matches!(
        PreparedTextObservationOwner::select(&owners, |owner| owner
            .contains_object("object.dialogue.7.9.ruby.0")),
        Err(PreparedTextOwnerSelectionError::Missing)
    ));
}

#[test]
fn duplicate_content_is_rejected_for_observation_and_capture_queries() {
    let owners = [
        dialogue_owner(1, 9, DialoguePreparedTextRole::CharacterDisplayName),
        dialogue_owner(2, 9, DialoguePreparedTextRole::Content),
        dialogue_owner(3, 9, DialoguePreparedTextRole::Content),
    ];
    assert!(matches!(
        PreparedTextObservationOwner::select(&owners, |owner| matches!(
            owner.prepared().kind,
            PreparedTextOwnerKind::DialogueView {
                dialogue: 7,
                entry: 9,
                ..
            }
        )),
        Err(PreparedTextOwnerSelectionError::Ambiguous { count: 2 })
    ));
    assert!(matches!(
        PreparedTextObservationOwner::select(&owners, |owner| owner
            .contains_object("object.dialogue.7.9.ruby.0")),
        Err(PreparedTextOwnerSelectionError::Ambiguous { count: 2 })
    ));
}
