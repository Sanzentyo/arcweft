use crate::{
    ArcweftRustField, ArcweftRustPackageId, ArcweftRustStructShape, ArcweftRustTypeDecl,
    ArcweftRustTypeKind, ArcweftRustTypePath, ArcweftRustTypeRef, ArcweftRustVariantPayload,
};
use std::fmt;

impl fmt::Display for ArcweftRustPackageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for ArcweftRustTypePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments().iter().enumerate() {
            if index > 0 {
                formatter.write_str("::")?;
            }
            formatter.write_str(segment.as_str())?;
        }
        Ok(())
    }
}

impl fmt::Display for ArcweftRustTypeDecl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ArcweftRustTypeKind::Struct { shape } => {
                write!(formatter, "struct {}", self.path)?;
                match shape {
                    ArcweftRustStructShape::Unit => Ok(()),
                    ArcweftRustStructShape::Tuple { fields } => {
                        formatter.write_str("(")?;
                        write_types(formatter, fields)?;
                        formatter.write_str(")")
                    }
                    ArcweftRustStructShape::Record { fields } => {
                        formatter.write_str(" { ")?;
                        write_fields(formatter, fields)?;
                        formatter.write_str(" }")
                    }
                }
            }
            ArcweftRustTypeKind::Enum { variants } => {
                write!(formatter, "enum {}", self.path)?;
                if variants.is_empty() {
                    return Ok(());
                }
                formatter.write_str(" { ")?;
                for (index, variant) in variants.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    formatter.write_str(&variant.name)?;
                    match &variant.payload {
                        ArcweftRustVariantPayload::Unit => {}
                        ArcweftRustVariantPayload::Tuple { fields } => {
                            formatter.write_str("(")?;
                            write_types(formatter, fields)?;
                            formatter.write_str(")")?;
                        }
                        ArcweftRustVariantPayload::Record { fields } => {
                            formatter.write_str(" { ")?;
                            write_fields(formatter, fields)?;
                            formatter.write_str(" }")?;
                        }
                    }
                }
                formatter.write_str(" }")
            }
            ArcweftRustTypeKind::Newtype { inner } => {
                write!(formatter, "newtype {}({inner})", self.path)
            }
        }
    }
}

impl fmt::Display for ArcweftRustTypeRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => formatter.write_str("()"),
            Self::Bool => formatter.write_str("Bool"),
            Self::I8 => formatter.write_str("i8"),
            Self::I16 => formatter.write_str("i16"),
            Self::I32 => formatter.write_str("i32"),
            Self::I64 => formatter.write_str("i64"),
            Self::I128 => formatter.write_str("i128"),
            Self::ISize => formatter.write_str("isize"),
            Self::U8 => formatter.write_str("u8"),
            Self::U16 => formatter.write_str("u16"),
            Self::U32 => formatter.write_str("u32"),
            Self::U64 => formatter.write_str("u64"),
            Self::U128 => formatter.write_str("u128"),
            Self::USize => formatter.write_str("usize"),
            Self::F32 => formatter.write_str("f32"),
            Self::F64 => formatter.write_str("f64"),
            Self::String => formatter.write_str("String"),
            Self::Char => formatter.write_str("Char"),
            Self::Vec { item } => write!(formatter, "Vec<{item}>"),
            Self::Seq { item } => write!(formatter, "Seq<{item}>"),
            Self::Option { item } => write!(formatter, "Option<{item}>"),
            Self::Result { ok, error } => write!(formatter, "Result<{ok}, {error}>"),
            Self::Tuple { items } => {
                formatter.write_str("(")?;
                write_types(formatter, items)?;
                formatter.write_str(")")
            }
            Self::Nominal {
                package,
                path,
                arguments,
            } => {
                write!(formatter, "{package}::{path}")?;
                if !arguments.is_empty() {
                    formatter.write_str("<")?;
                    write_types(formatter, arguments)?;
                    formatter.write_str(">")?;
                }
                Ok(())
            }
            Self::TypeParameter { index } => write!(formatter, "T{}", index.get()),
        }
    }
}

fn write_fields(formatter: &mut fmt::Formatter<'_>, fields: &[ArcweftRustField]) -> fmt::Result {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        write!(formatter, "{}: {}", field.name, field.ty)?;
    }
    Ok(())
}

fn write_types(formatter: &mut fmt::Formatter<'_>, types: &[ArcweftRustTypeRef]) -> fmt::Result {
    for (index, ty) in types.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        write!(formatter, "{ty}")?;
    }
    Ok(())
}
