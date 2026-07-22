use crate::types::TypeKind;

pub(super) fn type_kind_label(ty: &TypeKind) -> String {
    ty.source_label()
}
