#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

pub(crate) const fn default_true() -> bool {
    true
}
