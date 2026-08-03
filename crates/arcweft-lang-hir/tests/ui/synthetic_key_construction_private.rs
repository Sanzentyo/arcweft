use arcweft_lang_hir::identity::{SyntheticKey, SyntheticOwner, SyntheticRole};

fn through_constructor(owner: SyntheticOwner) -> SyntheticKey {
    SyntheticKey::try_new(owner, SyntheticRole::ImplicitUnitTail, 0).unwrap()
}

fn through_fields(owner: SyntheticOwner) -> SyntheticKey {
    SyntheticKey {
        owner,
        role: SyntheticRole::ImplicitUnitTail,
        ordinal: 0,
    }
}

fn main() {}
