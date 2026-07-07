use crate::types::TypeKind;

pub(super) fn type_head(label: &str) -> &str {
    label.split('<').next().unwrap_or(label)
}

pub(super) fn label_has_generic(label: &str) -> bool {
    label
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .any(|part| {
            part.chars().next().is_some_and(char::is_uppercase) && part.chars().count() == 1
        })
}

pub(super) fn type_kind_label(ty: &TypeKind) -> String {
    ty.source_label()
}
