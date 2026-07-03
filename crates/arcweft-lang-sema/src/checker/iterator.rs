//! `for` iteration typing through the standard trait substrate.

use super::{
    ForIterationEvidence, ForIterationEvidenceFamily, StandardIteratorFamily, TypeCheckError,
    TypeChecker, TypeKind,
};
use crate::traits::{IntoIteratorResolution, IntoIteratorResolutionError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ForIterationTyping {
    pub item_ty: TypeKind,
}

impl TypeChecker<'_> {
    pub(super) fn check_for_iteration_source(
        &mut self,
        source_ty: Option<&TypeKind>,
    ) -> Option<ForIterationTyping> {
        let Some(source_ty) = source_ty else {
            self.errors.push(TypeCheckError::new(
                "for source type could not be inferred; `for` requires IntoIterator".to_owned(),
            ));
            return None;
        };
        match self
            .trait_catalog
            .resolve_into_iterator(source_ty, &self.active_trait_predicates())
        {
            Ok(resolution) => {
                let item_ty = resolution.item_ty().clone();
                if self.record_runtime_for_iteration_evidence {
                    self.for_iteration_evidence
                        .push(for_iteration_evidence(&resolution));
                }
                Some(ForIterationTyping { item_ty })
            }
            Err(IntoIteratorResolutionError::MissingIntoIterator { source }) => {
                self.errors.push(TypeCheckError::new(format!(
                    "for source of type {source:?} requires IntoIterator"
                )));
                None
            }
            Err(IntoIteratorResolutionError::AmbiguousIntoIterator { source, candidates }) => {
                self.errors.push(TypeCheckError::new(format!(
                    "ambiguous IntoIterator impl for {source:?}: {candidates:?}"
                )));
                None
            }
            Err(IntoIteratorResolutionError::MissingIteratorForIntoIter {
                source,
                into_iter,
                item,
            }) => {
                self.errors.push(TypeCheckError::new(format!(
                    "IntoIterator::IntoIter for {source:?} is {into_iter:?}, which does not implement Iterator<Item = {item:?}>"
                )));
                None
            }
            Err(IntoIteratorResolutionError::AmbiguousIteratorForIntoIter {
                source,
                into_iter,
                candidates,
            }) => {
                self.errors.push(TypeCheckError::new(format!(
                    "ambiguous Iterator impl for IntoIterator::IntoIter {into_iter:?} from {source:?}: {candidates:?}"
                )));
                None
            }
        }
    }
}

fn for_iteration_evidence(resolution: &IntoIteratorResolution) -> ForIterationEvidence {
    ForIterationEvidence {
        family: standard_iterator_family(resolution.source_ty())
            .map_or(ForIterationEvidenceFamily::WitnessUnsupported, |family| {
                ForIterationEvidenceFamily::Builtin(family)
            }),
        item_ty: resolution.item_ty().clone(),
        into_iter_ty: resolution.into_iter_ty().clone(),
    }
}

fn standard_iterator_family(ty: &TypeKind) -> Option<StandardIteratorFamily> {
    match ty {
        TypeKind::Range(_) => Some(StandardIteratorFamily::Range),
        TypeKind::Seq(_) => Some(StandardIteratorFamily::Seq),
        TypeKind::Vec(_) => Some(StandardIteratorFamily::Vec),
        TypeKind::Array { .. } => Some(StandardIteratorFamily::Array),
        TypeKind::Slice(_) => Some(StandardIteratorFamily::Slice),
        _ => None,
    }
}
