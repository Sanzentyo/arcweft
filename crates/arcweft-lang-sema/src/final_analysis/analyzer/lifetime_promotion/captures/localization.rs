//! Body projections for continuations with several lexical argument sources.

use arcweft_lang_hir::identity::ExprId;

use crate::{
    callable::CheckedCallSite,
    effect_row::EffectRow,
    types::{
        ArrayLength, GenericScope, ScopedArrayLengthView, ScopedEffectRowView, ScopedType,
        ScopedTypeView, TypeKind, TypeProjectionError,
        constraints::{LocalizedTypeTemplate, TypeTemplateLocalizer},
    },
};

use super::{
    Analyzer, FinalSemanticAnalysisError, ReceiverBorrowQuery, storage_projection_error,
    worklist::{BodyProjection, StorageFrame},
};

pub(super) struct RetainedBodyProjection {
    pub(super) site: CheckedCallSite,
    first: LocalizedTypeTemplate,
    suffix: Vec<LocalizedTypeTemplate>,
}

impl RetainedBodyProjection {
    pub(super) fn new(site: CheckedCallSite, prefix: LocalizedTypeTemplate) -> Self {
        Self {
            site,
            first: prefix,
            suffix: Vec::new(),
        }
    }

    pub(super) fn advance(
        &mut self,
        stage: LocalizedTypeTemplate,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let previous = self.suffix.last().unwrap_or(&self.first);
        if previous.result_scope() != stage.source_scope() {
            return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
        }
        self.suffix.push(stage);
        Ok(())
    }

    pub(super) const fn declaration_template(&self) -> &LocalizedTypeTemplate {
        &self.first
    }

    pub(super) fn project_type(
        &self,
        ty: &TypeKind,
        under: &GenericScope,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<ScopedType, TypeProjectionError<FinalSemanticAnalysisError>> {
        let mut projected = self.first.project_type(ty, under, receiver)?;
        for stage in &self.suffix {
            projected = stage.project_type(projected.view().value(), under, receiver)?;
        }
        Ok(projected)
    }

    pub(super) fn project_length(
        &self,
        length: &ArrayLength,
        under: &GenericScope,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<ArrayLength, TypeProjectionError<FinalSemanticAnalysisError>> {
        let mut length = self.first.project_length(length, under, receiver)?;
        for stage in &self.suffix {
            length = stage.project_length(&length, under, receiver)?;
        }
        Ok(length)
    }

    pub(super) fn project_effects(
        &self,
        row: &EffectRow,
        under: &GenericScope,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<EffectRow, TypeProjectionError<FinalSemanticAnalysisError>> {
        let mut row = self.first.project_effects(row, under, receiver)?;
        for stage in &self.suffix {
            row = stage.project_effects(&row, under, receiver)?;
        }
        Ok(row)
    }
}

struct LexicalStorage<'a, 'project, 'catalog, 'control> {
    analyzer: &'a Analyzer<'project, 'catalog, 'control>,
    projections: &'a [BodyProjection],
}

impl<'q> TypeTemplateLocalizer<ReceiverBorrowQuery<'q>> for LexicalStorage<'_, '_, '_, '_> {
    fn type_term(
        &self,
        term: ScopedTypeView<'_>,
        control: &mut ReceiverBorrowQuery<'q>,
    ) -> Result<ScopedType, TypeProjectionError<FinalSemanticAnalysisError>> {
        match self
            .analyzer
            .project_type_chain(term.value(), term.scope(), self.projections, control)
            .map_err(TypeProjectionError::Control)?
        {
            Some(projected) => Ok(projected),
            None => term.clone_with_control(control),
        }
    }

    fn const_term(
        &self,
        term: ScopedArrayLengthView<'_>,
        control: &mut ReceiverBorrowQuery<'q>,
    ) -> Result<ArrayLength, TypeProjectionError<FinalSemanticAnalysisError>> {
        self.analyzer
            .project_length_chain(term.value(), term.scope(), self.projections, control)
            .map_err(TypeProjectionError::Control)
    }

    fn effect_term(
        &self,
        term: ScopedEffectRowView<'_>,
        control: &mut ReceiverBorrowQuery<'q>,
    ) -> Result<EffectRow, TypeProjectionError<FinalSemanticAnalysisError>> {
        self.analyzer
            .project_effect_chain(term.value(), term.scope(), self.projections, control)
            .map_err(TypeProjectionError::Control)
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn localize_application(
        &self,
        site: CheckedCallSite,
        frame: &StorageFrame,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<LocalizedTypeTemplate, FinalSemanticAnalysisError> {
        self.with_selected_application(site, |application| {
            application
                .localize_body_template(
                    &LexicalStorage {
                        analyzer: self,
                        projections: &frame.projections,
                    },
                    receiver,
                )
                .map_err(storage_projection_error)
        })
    }

    pub(super) fn localize_specialization(
        &self,
        owner: ExprId,
        frame: &StorageFrame,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<LocalizedTypeTemplate, FinalSemanticAnalysisError> {
        self.value_specialization(owner)?
            .localize_template(
                &LexicalStorage {
                    analyzer: self,
                    projections: &frame.projections,
                },
                receiver,
            )
            .map_err(storage_projection_error)
    }
}
