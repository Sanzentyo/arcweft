use arcweft_lang_sema::registration::CharacterInventoryDescriptorV1;

fn require_serialize<T: serde::Serialize>(_: &T) {}

fn bypass(descriptor: &CharacterInventoryDescriptorV1) {
    require_serialize(descriptor);
}

fn main() {}
