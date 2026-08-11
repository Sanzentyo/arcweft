use crate::Signature;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwapCompatibility {
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

#[must_use]
pub fn classify_swap(
    old: &Signature,
    new: &Signature,
    provider_abi_equal: bool,
    runtime_abi_equal: bool,
) -> SwapCompatibility {
    if !provider_abi_equal || !runtime_abi_equal {
        return SwapCompatibility::RestartRequired;
    }
    if old == new {
        SwapCompatibility::CodeCompatible
    } else {
        SwapCompatibility::CodeGenerational
    }
}
