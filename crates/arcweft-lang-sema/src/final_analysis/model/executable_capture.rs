use arcweft_lang_hir::identity::LocalId;

use crate::semantic_coordinate::StableCheckedBindingCoordinate;

use super::TypeKind;

/// Exact free local retained by a checked executable body.
///
/// The HIR local is generation-bound lowering evidence. Stable transcripts
/// use the binding coordinate together with its accepted semantic type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutableCapture {
    local: LocalId,
    origin: StableCheckedBindingCoordinate,
    ty: TypeKind,
}

impl CheckedExecutableCapture {
    pub(crate) const fn new(
        local: LocalId,
        origin: StableCheckedBindingCoordinate,
        ty: TypeKind,
    ) -> Self {
        Self { local, origin, ty }
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn origin(&self) -> &StableCheckedBindingCoordinate {
        &self.origin
    }

    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}
