//! Bijection between one accepted world and its Rust metadata publications.

mod admission;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use arcweft_source::SourceSpan;
use thiserror::Error;

use super::RustTypeMetadataPublicationInput;
use crate::env::nominal::{AcceptedNominalId, AcceptedNominalSemantics};
use crate::nominal::{
    NominalAggregationLimitKind, NominalAggregationLimits, NominalResolutionLimitKind,
    NominalResolutionLimits,
};
use crate::registration::{AcceptedNominalWorld, EnvironmentPublicationItemId};

#[derive(Clone, Debug, Eq, Error, Ord, PartialEq, PartialOrd)]
#[error("Rust metadata publication for {declaration:?}: {kind}")]
pub struct RustMetadataJoinError {
    declaration: AcceptedNominalId,
    source_span: SourceSpan,
    kind: RustMetadataJoinErrorKind,
}

#[derive(Clone, Debug, Eq, Error, Ord, PartialEq, PartialOrd)]
pub enum RustMetadataJoinErrorKind {
    #[error("Rust metadata exceeded {kind:?}: observed {observed}, maximum {maximum}")]
    ReferenceLimit {
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("Rust metadata exceeded {kind:?}: observed {observed}, maximum {maximum}")]
    AggregateLimit {
        kind: NominalAggregationLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("the exact declaration is absent")]
    MissingDeclaration,
    #[error("the declaration is not a structural Rust ADT")]
    NonStructuralDeclaration,
    #[error("the declaration has no metadata publication")]
    MissingMetadata,
    #[error("the declaration has duplicate metadata publications")]
    DuplicateMetadata { first: Box<SourceSpan> },
    #[error("the declaration has no source publication")]
    MissingPublication,
    #[error("the declaration has both visible and inaccessible publications")]
    AmbiguousPublication,
    #[error("the metadata publication item differs from its declaration")]
    PublicationMismatch {
        expected: Box<EnvironmentPublicationItemId>,
        actual: Box<EnvironmentPublicationItemId>,
    },
    #[error("the metadata source differs from its declaration")]
    SourceMismatch { expected: Box<SourceSpan> },
    #[error("the Rust package, item and exact declaration identity do not agree")]
    OwnerMismatch,
    #[error("the declaration has arity {expected}, but metadata has {actual} parameters")]
    ArityMismatch { expected: u16, actual: usize },
}

impl RustMetadataJoinError {
    pub const fn declaration(&self) -> &AcceptedNominalId {
        &self.declaration
    }
    pub const fn source_span(&self) -> &SourceSpan {
        &self.source_span
    }
    pub const fn kind(&self) -> &RustMetadataJoinErrorKind {
        &self.kind
    }
}

/// A transient proof tied to both borrowed inputs. Only a successful complete
/// join can construct it; projected metadata retains the publication identity.
#[derive(Debug)]
pub(crate) struct JoinedRustMetadata<'world, 'input> {
    world: &'world AcceptedNominalWorld,
    inputs: &'input [RustTypeMetadataPublicationInput],
    nominal_limits: NominalResolutionLimits,
}

impl<'world, 'input> JoinedRustMetadata<'world, 'input> {
    pub(crate) const fn world(&self) -> &'world AcceptedNominalWorld {
        self.world
    }
    pub(crate) const fn inputs(&self) -> &'input [RustTypeMetadataPublicationInput] {
        self.inputs
    }
    pub(crate) const fn nominal_limits(&self) -> NominalResolutionLimits {
        self.nominal_limits
    }
}

impl AcceptedNominalWorld {
    pub(crate) fn join_rust_metadata<'world, 'input>(
        &'world self,
        inputs: &'input [RustTypeMetadataPublicationInput],
        nominal_limits: NominalResolutionLimits,
        aggregation_limits: NominalAggregationLimits,
    ) -> Result<JoinedRustMetadata<'world, 'input>, RustMetadataJoinError> {
        use RustMetadataJoinErrorKind as Kind;

        admission::validate(inputs, nominal_limits, aggregation_limits)?;

        let mut by_id = BTreeMap::new();
        for input in inputs {
            let fail = |kind| RustMetadataJoinError {
                declaration: input.id().clone(),
                source_span: input.source().clone(),
                kind,
            };
            if let Some(first) = by_id.insert(input.id(), input) {
                return Err(fail(Kind::DuplicateMetadata {
                    first: Box::new(first.source().clone()),
                }));
            }
            let record = self
                .nominal_catalog()
                .exact(input.id().canonical_path())
                .filter(|record| record.id() == input.id())
                .ok_or_else(|| fail(Kind::MissingDeclaration))?;
            if !matches!(record.semantics(), AcceptedNominalSemantics::RustAdt) {
                return Err(fail(Kind::NonStructuralDeclaration));
            }
            if usize::from(record.arity()) != input.parameters().len() {
                return Err(fail(Kind::ArityMismatch {
                    expected: record.arity(),
                    actual: input.parameters().len(),
                }));
            }
            let publication = match (
                self.visibility().visible(input.id()),
                self.visibility().inaccessible(input.id()),
            ) {
                (Some(publication), None) | (None, Some(publication)) => publication,
                (None, None) => return Err(fail(Kind::MissingPublication)),
                (Some(_), Some(_)) => return Err(fail(Kind::AmbiguousPublication)),
            };
            if publication.item() != input.item() {
                return Err(fail(Kind::PublicationMismatch {
                    expected: Box::new(publication.item().clone()),
                    actual: Box::new(input.item().clone()),
                }));
            }
            for expected in [
                record
                    .source()
                    .expect("Rust ADT declarations retain their source"),
                publication.declaration(),
            ] {
                if expected != input.source() {
                    return Err(fail(Kind::SourceMismatch {
                        expected: Box::new(expected.clone()),
                    }));
                }
            }
            if !matches!(input.item(), EnvironmentPublicationItemId::RustType { package, rust_item, accepted_path, .. }
                if package == input.package() && rust_item == input.rust_item()
                    && accepted_path == input.id().canonical_path()
                    && input.package_provenance().name() == package.as_str()
                    && matches!(input.id().owner(), crate::env::nominal::AcceptedNominalOwnerId::RustPackage(owner) if owner == package))
            {
                return Err(fail(Kind::OwnerMismatch));
            }
        }
        for record in self.nominal_catalog().exact_records() {
            if matches!(record.semantics(), AcceptedNominalSemantics::RustAdt)
                && !by_id.contains_key(record.id())
            {
                return Err(RustMetadataJoinError {
                    declaration: record.id().clone(),
                    source_span: record
                        .source()
                        .expect("Rust ADT declarations retain their source")
                        .clone(),
                    kind: Kind::MissingMetadata,
                });
            }
        }
        Ok(JoinedRustMetadata {
            world: self,
            inputs,
            nominal_limits,
        })
    }
}
