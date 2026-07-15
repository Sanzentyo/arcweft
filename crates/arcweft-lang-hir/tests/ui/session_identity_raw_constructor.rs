use arcweft_lang_hir::identity::HirModuleId;
use arcweft_source::identity::SourceGeneration;
use core::num::NonZeroU32;

fn main() {
    let _forged_module = HirModuleId(NonZeroU32::MIN);
    let _forged_generation = SourceGeneration(NonZeroU32::MIN);
}
