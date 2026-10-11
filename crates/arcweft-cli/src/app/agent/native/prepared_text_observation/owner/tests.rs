use super::*;
use arcweft_presentation::hit::HitRect;
use arcweft_render_wgpu::view_scene::PreparedTextId;

fn dialogue_owner(
    index: u32,
    entry: u64,
    mount: u64,
    role: DialoguePreparedTextRole,
) -> PreparedTextOwner {
    PreparedTextOwner::new(
        PreparedTextId::from_index(index),
        arcweft_id::PublicId::try_new(format!("view.dialogue.text{index}")).unwrap(),
        PreparedTextOwnerKind::DialogueView {
            dialogue: 7,
            entry,
            mount,
            role,
        },
        arcweft_text_model::PreparedTextContext::document(0, 0).unwrap(),
        HitRect::new(0.0, 0.0, 10.0, 10.0),
    )
}

#[test]
fn dialogue_owner_identity_distinguishes_speaker_content_entry_mount_and_text_target() {
    let owners = [
        dialogue_owner(1, 9, 11, DialoguePreparedTextRole::CharacterDisplayName),
        dialogue_owner(2, 9, 11, DialoguePreparedTextRole::Content),
        dialogue_owner(3, 90, 11, DialoguePreparedTextRole::Content),
        dialogue_owner(2, 9, 12, DialoguePreparedTextRole::Content),
        dialogue_owner(4, 9, 11, DialoguePreparedTextRole::Content),
    ];
    let roots = owners
        .iter()
        .map(PreparedTextObservationOwner::new)
        .map(|owner| owner.root_id().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(roots.len(), owners.len());
    for prepared in &owners {
        let owner = PreparedTextObservationOwner::new(prepared);
        assert!(std::ptr::eq(owner.prepared(), prepared));
        assert_eq!(
            owner.root_id(),
            PreparedTextObservationOwner::new(prepared).root_id()
        );
    }
}
