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
        TypeKind::Bool => "bool".to_owned(),
        TypeKind::I8 => "i8".to_owned(),
        TypeKind::I16 => "i16".to_owned(),
        TypeKind::I32 => "i32".to_owned(),
        TypeKind::I64 => "i64".to_owned(),
        TypeKind::I128 => "i128".to_owned(),
        TypeKind::ISize => "isize".to_owned(),
        TypeKind::U8 => "u8".to_owned(),
        TypeKind::U16 => "u16".to_owned(),
        TypeKind::U32 => "u32".to_owned(),
        TypeKind::U64 => "u64".to_owned(),
        TypeKind::U128 => "u128".to_owned(),
        TypeKind::USize => "usize".to_owned(),
        TypeKind::F32 => "f32".to_owned(),
        TypeKind::F64 => "f64".to_owned(),
        TypeKind::String => "String".to_owned(),
        TypeKind::Char => "char".to_owned(),
        TypeKind::Bytes => "Bytes".to_owned(),
        TypeKind::Duration => "Duration".to_owned(),
        TypeKind::Unit => "()".to_owned(),
        TypeKind::Never => "Never".to_owned(),
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
        TypeKind::IteratorState { family, item } => {
            format!("{family:?}IteratorState<{}>", type_kind_label(item))
        }
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
