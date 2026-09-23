//! Validate the resolved source policy before adapter publication. These checks
//! use declaration slots, not a reconstructed reflection graph.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    ArcweftRustEnumRepr as Repr, ArcweftRustEnumTagStyle as Tag, ArcweftRustField,
    ArcweftRustStructShape, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustVariantPayload,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ArcweftRustCodecPolicyError {
    #[error("a non-enum declaration cannot carry enum tag or repr policy")]
    NonEnumPolicy,
    #[error("adjacent enum tag and content must use distinct keys")]
    AmbiguousAdjacentKeys,
    #[error("duplicate variant wire name `{name}` at slots {first} and {duplicate}")]
    DuplicateVariant {
        name: String,
        first: usize,
        duplicate: usize,
    },
    #[error(
        "duplicate field wire name `{name}` at slots {first} and {duplicate} in variant {variant:?}"
    )]
    DuplicateField {
        variant: Option<usize>,
        name: String,
        first: usize,
        duplicate: usize,
    },
    #[error("internally tagged variant {variant} must have unit or record payload")]
    InternalTuple { variant: usize },
    #[error("field {field} of variant {variant} uses the enum tag key")]
    TagFieldCollision { variant: usize, field: usize },
    #[error("repr variant {variant} must have no payload")]
    ReprPayload { variant: usize },
    #[error("variant {variant} has a missing, unexpected, or out-of-range repr discriminant")]
    Discriminant { variant: usize },
    #[error("variant {variant} repeats discriminant {value}")]
    DuplicateDiscriminant { variant: usize, value: i128 },
}

impl ArcweftRustTypeDecl {
    pub(crate) fn validate_data_policy(&self) -> Result<(), ArcweftRustCodecPolicyError> {
        use ArcweftRustCodecPolicyError as Error;
        let tag = self
            .data_policy
            .as_ref()
            .map_or(&Tag::External, |policy| &policy.tag);
        let repr = self.data_policy.as_ref().and_then(|policy| policy.repr);
        let ArcweftRustTypeKind::Enum { variants } = &self.kind else {
            if !matches!(tag, Tag::External) || repr.is_some() {
                return Err(Error::NonEnumPolicy);
            }
            if let ArcweftRustTypeKind::Struct {
                shape: ArcweftRustStructShape::Record { fields },
            } = &self.kind
            {
                validate_fields(fields, None, None)?;
            }
            return Ok(());
        };
        if matches!(tag, Tag::Adjacent { tag, content } if tag == content) {
            return Err(Error::AmbiguousAdjacentKeys);
        }
        let mut names = BTreeMap::new();
        let mut discriminants = BTreeSet::new();
        for (ordinal, variant) in variants.iter().enumerate() {
            let name = variant.wire_name.as_deref().unwrap_or(&variant.name);
            if let Some(first) = names.insert(name, ordinal) {
                return Err(Error::DuplicateVariant {
                    name: name.to_owned(),
                    first,
                    duplicate: ordinal,
                });
            }
            match (repr, variant.discriminant) {
                (Some(repr), Some(value)) if repr.accepts(value) => {
                    if !matches!(variant.payload, ArcweftRustVariantPayload::Unit) {
                        return Err(Error::ReprPayload { variant: ordinal });
                    }
                    if !discriminants.insert(value) {
                        return Err(Error::DuplicateDiscriminant {
                            variant: ordinal,
                            value,
                        });
                    }
                }
                (None, None) => {}
                _ => return Err(Error::Discriminant { variant: ordinal }),
            }
            match &variant.payload {
                ArcweftRustVariantPayload::Tuple { .. } if matches!(tag, Tag::Internal { .. }) => {
                    return Err(Error::InternalTuple { variant: ordinal });
                }
                ArcweftRustVariantPayload::Record { fields } => {
                    let key = if let Tag::Internal { tag } = tag {
                        Some(tag.as_str())
                    } else {
                        None
                    };
                    validate_fields(fields, Some(ordinal), key)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn validate_fields(
    fields: &[ArcweftRustField],
    variant: Option<usize>,
    tag: Option<&str>,
) -> Result<(), ArcweftRustCodecPolicyError> {
    let mut names = BTreeMap::new();
    for (ordinal, field) in fields.iter().enumerate().filter(|(_, field)| !field.skip) {
        let name = field.wire_name.as_deref().unwrap_or(&field.name);
        if let Some(first) = names.insert(name, ordinal) {
            return Err(ArcweftRustCodecPolicyError::DuplicateField {
                variant,
                name: name.to_owned(),
                first,
                duplicate: ordinal,
            });
        }
        if tag == Some(name) {
            return Err(ArcweftRustCodecPolicyError::TagFieldCollision {
                variant: variant.expect("only enum variants have a tag"),
                field: ordinal,
            });
        }
    }
    Ok(())
}

impl Repr {
    fn accepts(self, value: i128) -> bool {
        let (min, max) = match self {
            Self::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
            Self::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
            Self::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
            Self::I64 | Self::Isize => (i128::from(i64::MIN), i128::from(i64::MAX)),
            Self::I128 => (i128::MIN, i128::MAX),
            Self::U8 => (0, i128::from(u8::MAX)),
            Self::U16 => (0, i128::from(u16::MAX)),
            Self::U32 => (0, i128::from(u32::MAX)),
            Self::U64 | Self::Usize => (0, i128::from(u64::MAX)),
            Self::U128 => (0, i128::MAX),
        };
        (min..=max).contains(&value)
    }
}
