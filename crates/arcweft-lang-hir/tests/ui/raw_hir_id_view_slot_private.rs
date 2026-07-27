use arcweft_lang_hir::identity::RawHirIdView;
use core::num::NonZeroU32;

fn main() {}

fn raw_slot(view: RawHirIdView) -> NonZeroU32 {
    view.slot
}
