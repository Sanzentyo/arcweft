use arcweft_lang_sema::types::{EntityKind, TypeKind};

fn removed_speaker_preset_helpers(ty: &TypeKind) {
    let _ = ty.speaker_preset_entity_kind();
    let _ = ty.is_speaker_preset_for(&EntityKind::Character);
}

fn removed_unsigned_integer_helper(ty: &TypeKind) {
    let _ = ty.is_unsigned_integer();
}

fn main() {}
