use arcweft_lang_hir::identity::{HirDatabaseId, HirModuleId};
use core::num::NonZeroU32;

fn main() {}

fn forge_module(database: HirDatabaseId) -> HirModuleId {
    HirModuleId {
        database,
        slot: NonZeroU32::MIN,
    }
}
