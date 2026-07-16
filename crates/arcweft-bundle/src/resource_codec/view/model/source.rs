//! Canonical product-source tables owned by View resources.

use std::collections::BTreeMap;

use crate::resource_codec::{
    ProductSourceRef, ProductSourceRefIndex, SourceRangeRef, ViewProductBuildError,
};

use super::{ViewProgramResource, ViewStyleResource};

impl ViewProgramResource {
    pub(crate) fn canonicalize_source_table(&mut self) {
        let Some(rebase) = canonical_rebase(&mut self.source_refs) else {
            return;
        };
        self.for_each_source_range_mut(|range| apply_rebase(range, &rebase));
    }

    pub(crate) fn validate_source_table(&self) -> Result<(), ViewProductBuildError> {
        validate_table(&self.source_refs)?;
        self.source_ranges().try_for_each(|range| {
            self.source_refs.get(range.source().index()).map_or_else(
                || {
                    Err(ViewProductBuildError::InvalidSourceIndex {
                        index: range.source().value(),
                        count: self.source_refs.len(),
                    })
                },
                |_| Ok(()),
            )
        })
    }

    pub(crate) fn offset_source_indexes(
        &mut self,
        offset: usize,
    ) -> Result<(), ViewProductBuildError> {
        let mut result = Ok(());
        self.for_each_source_range_mut(|range| {
            if result.is_err() {
                return;
            }
            result = range
                .source()
                .index()
                .checked_add(offset)
                .ok_or(ViewProductBuildError::TooManySourceRefs)
                .and_then(ProductSourceRefIndex::try_from_index)
                .map(|source| range.set_source(source));
        });
        result
    }

    pub(crate) fn source_ranges(&self) -> impl Iterator<Item = &SourceRangeRef> {
        self.instructions
            .iter()
            .filter_map(super::ViewProgramInstruction::source)
            .chain(
                self.semantic_targets
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.layout_bounds
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.scroll_regions
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(self.surfaces.iter().filter_map(|item| item.source.as_ref()))
            .chain(
                self.text_blocks
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.action_buttons
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(
                self.focus_groups
                    .iter()
                    .filter_map(|item| item.source.as_ref()),
            )
            .chain(self.focus_navigation.iter().flat_map(|item| {
                item.source
                    .iter()
                    .chain(item.edges.iter().filter_map(|edge| edge.source.as_ref()))
            }))
            .chain(
                self.exported_parts
                    .iter()
                    .flat_map(|part| part.source.ranges()),
            )
    }

    pub(crate) fn for_each_source_range_mut(&mut self, mut apply: impl FnMut(&mut SourceRangeRef)) {
        self.instructions
            .iter_mut()
            .filter_map(super::ViewProgramInstruction::source_mut)
            .for_each(&mut apply);
        self.semantic_targets
            .iter_mut()
            .filter_map(|item| item.source.as_mut())
            .for_each(&mut apply);
        self.layout_bounds
            .iter_mut()
            .filter_map(|item| item.source.as_mut())
            .for_each(&mut apply);
        self.scroll_regions
            .iter_mut()
            .filter_map(|item| item.source.as_mut())
            .for_each(&mut apply);
        self.surfaces
            .iter_mut()
            .filter_map(|item| item.source.as_mut())
            .for_each(&mut apply);
        self.text_blocks
            .iter_mut()
            .filter_map(|item| item.source.as_mut())
            .for_each(&mut apply);
        self.action_buttons
            .iter_mut()
            .filter_map(|item| item.source.as_mut())
            .for_each(&mut apply);
        self.focus_groups
            .iter_mut()
            .filter_map(|item| item.source.as_mut())
            .for_each(&mut apply);
        for navigation in &mut self.focus_navigation {
            navigation.source.iter_mut().for_each(&mut apply);
            navigation
                .edges
                .iter_mut()
                .filter_map(|edge| edge.source.as_mut())
                .for_each(&mut apply);
        }
        self.exported_parts
            .iter_mut()
            .flat_map(|part| part.source.ranges_mut())
            .for_each(apply);
    }
}

impl ViewStyleResource {
    pub(crate) fn canonicalize_source_table(&mut self) {
        let Some(rebase) = canonical_rebase(&mut self.source_refs) else {
            return;
        };
        self.source_map_refs
            .iter_mut()
            .for_each(|range| apply_rebase(range, &rebase));
    }

    pub(crate) fn validate_source_table(&self) -> Result<(), ViewProductBuildError> {
        validate_table(&self.source_refs)?;
        self.source_map_refs.iter().try_for_each(|range| {
            self.source_refs.get(range.source().index()).map_or_else(
                || {
                    Err(ViewProductBuildError::InvalidSourceIndex {
                        index: range.source().value(),
                        count: self.source_refs.len(),
                    })
                },
                |_| Ok(()),
            )
        })
    }

    pub(crate) fn offset_source_indexes(
        &mut self,
        offset: usize,
    ) -> Result<(), ViewProductBuildError> {
        for range in &mut self.source_map_refs {
            let source = range
                .source()
                .index()
                .checked_add(offset)
                .ok_or(ViewProductBuildError::TooManySourceRefs)
                .and_then(ProductSourceRefIndex::try_from_index)?;
            range.set_source(source);
        }
        Ok(())
    }
}

fn canonical_rebase(source_refs: &mut Vec<ProductSourceRef>) -> Option<Vec<ProductSourceRefIndex>> {
    let original = source_refs.clone();
    let mut canonical = original.clone();
    canonical.sort();
    canonical.dedup();
    let lookup = canonical
        .iter()
        .enumerate()
        .map(|(index, source)| {
            ProductSourceRefIndex::try_from_index(index).map(|index| (source.clone(), index))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()
        .ok()?;
    let rebase = original
        .iter()
        .map(|source| lookup.get(source).copied())
        .collect::<Option<Vec<_>>>()?;
    *source_refs = canonical;
    Some(rebase)
}

fn apply_rebase(range: &mut SourceRangeRef, rebase: &[ProductSourceRefIndex]) {
    if let Some(source) = rebase.get(range.source().index()).copied() {
        range.set_source(source);
    }
}

fn validate_table(source_refs: &[ProductSourceRef]) -> Result<(), ViewProductBuildError> {
    ProductSourceRefIndex::try_from_index(source_refs.len())?;
    if source_refs.windows(2).all(|pair| pair[0] < pair[1]) || source_refs.len() < 2 {
        Ok(())
    } else {
        Err(ViewProductBuildError::UnknownSource)
    }
}
