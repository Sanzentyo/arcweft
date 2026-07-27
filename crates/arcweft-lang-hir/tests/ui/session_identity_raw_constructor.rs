use arcweft_lang_hir::identity::HirDatabaseId;
use arcweft_source::identity::SourceGeneration;
use core::num::{NonZeroU32, NonZeroU64};

fn main() {
    let _forged_database = HirDatabaseId(NonZeroU64::MIN);
    let _forged_generation = SourceGeneration(NonZeroU32::MIN);
}
