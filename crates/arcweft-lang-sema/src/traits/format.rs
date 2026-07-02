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
    match ty {
        TypeKind::Projection {
            subject,
            trait_name,
            assoc,
        } => trait_name.as_ref().map_or_else(
            || format!("{}::{assoc}", type_kind_label(subject)),
            |trait_name| format!("<{} as {trait_name}>::{assoc}", type_kind_label(subject)),
        ),
        TypeKind::GenericParam(name) | TypeKind::Named(name) => name.clone(),
        TypeKind::Vec(inner) => format!("Vec<{}>", type_kind_label(inner)),
        TypeKind::Seq(inner) => format!("Seq<{}>", type_kind_label(inner)),
        TypeKind::Range(inner) => format!("Range<{}>", type_kind_label(inner)),
        TypeKind::Slice(inner) => format!("[{}]", type_kind_label(inner)),
        TypeKind::Option(inner) => format!("Option<{}>", type_kind_label(inner)),
        TypeKind::Result { ok, error } => {
            format!(
                "Result<{}, {}>",
                type_kind_label(ok),
                type_kind_label(error)
            )
        }
        other => format!("{other:?}"),
    }
}
