//! Ordered name admission for every Rust metadata member namespace.

use std::collections::BTreeMap;

use super::{
    AcceptedRustStructShape, AcceptedRustTypeMetadataCatalogError, AcceptedRustTypeMetadataKind,
};
use crate::env::{EnumVariantPayload, nominal::AcceptedNominalId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustMetadataNameScope {
    StructFields,
    Variants,
    VariantFields { variant: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustMetadataNameProblem {
    Empty,
    Duplicate { first: usize },
}

impl RustMetadataNameScope {
    fn validate<'a>(
        self,
        id: &AcceptedNominalId,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), AcceptedRustTypeMetadataCatalogError> {
        let mut first_occurrences = BTreeMap::new();
        for (ordinal, name) in names.into_iter().enumerate() {
            let problem = if name.is_empty() {
                Some(RustMetadataNameProblem::Empty)
            } else {
                first_occurrences
                    .insert(name, ordinal)
                    .map(|first| RustMetadataNameProblem::Duplicate { first })
            };
            if let Some(problem) = problem {
                return Err(AcceptedRustTypeMetadataCatalogError::InvalidName {
                    id: id.clone(),
                    scope: self,
                    ordinal,
                    name: name.to_owned(),
                    problem,
                });
            }
        }
        Ok(())
    }
}

impl AcceptedRustTypeMetadataKind {
    pub(super) fn validate_names(
        &self,
        id: &AcceptedNominalId,
    ) -> Result<(), AcceptedRustTypeMetadataCatalogError> {
        match self {
            Self::Struct {
                shape: AcceptedRustStructShape::Record(fields),
            } => RustMetadataNameScope::StructFields
                .validate(id, fields.iter().map(|field| field.name())),
            Self::Enum { variants } => {
                RustMetadataNameScope::Variants
                    .validate(id, variants.iter().map(|variant| variant.name()))?;
                for (variant, metadata) in variants.iter().enumerate() {
                    if let EnumVariantPayload::Record(fields) = metadata.payload() {
                        RustMetadataNameScope::VariantFields { variant }
                            .validate(id, fields.iter().map(|field| field.name()))?;
                    }
                }
                Ok(())
            }
            Self::Struct {
                shape: AcceptedRustStructShape::Unit | AcceptedRustStructShape::Tuple(_),
            }
            | Self::Newtype { .. } => Ok(()),
        }
    }
}
