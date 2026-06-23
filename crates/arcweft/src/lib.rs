//! Namespaced facade crate for Arcweft application-facing imports.
//!
//! Runtime and compiler crates should depend on narrower crates directly. This
//! facade keeps crate families discoverable without flattening every public
//! symbol into a single root prelude.

pub mod adt {
    pub use arcweft_adt::*;
}

pub mod character {
    pub use arcweft_character::*;
}

pub mod character_ui {
    pub use arcweft_character_ui::*;
}

pub mod core {
    pub use arcweft_core::*;
}

pub mod dialogue {
    pub use arcweft_dialogue::*;
}

pub mod id {
    pub use arcweft_id::*;
}

pub mod memory {
    pub use arcweft_memory::*;
}

pub mod need {
    pub use arcweft_need::*;
}

pub mod presentation {
    pub use arcweft_presentation::*;
}

pub mod reference {
    pub use arcweft_ref::*;
}

pub mod source {
    pub use arcweft_source::*;
}
