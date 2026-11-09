//! Metered body type projections used by capture storage observations.

use std::{rc::Rc, sync::Arc};

use arcweft_lang_hir::{
    identity::{ExprId, ItemId},
    symbol::CallableDeclarationKey,
};

use crate::{
    callable::{
        CheckedCallSite, CheckedFunctionSpecialization, FunctionSpecializationSealFailure,
        PreparedCallableApplication, SpecializedDeclarationProjection,
    },
    final_analysis::PreparedExpressionFact,
    types::{
        ArrayLength, ScopedArrayLengthView, ScopedType, ScopedTypeView, TypeKind,
        TypeProjectionError,
    },
};

use super::{
    Analyzer, FinalSemanticAnalysisError, ReceiverBorrowQuery, call_storage_error,
    storage_projection_error,
    worklist::{ApplicationStorage, BodyProjection, StorageFrame, TypeStorage},
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn value_specialization(
        &self,
        owner: ExprId,
    ) -> Result<Arc<CheckedFunctionSpecialization>, FinalSemanticAnalysisError> {
        self.facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::complete)
            .and_then(crate::final_analysis::CheckedExpression::function_specialization_shared)
            .cloned()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
    }

    pub(super) fn invocation_specialization(
        &self,
        site: CheckedCallSite,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<Arc<CheckedFunctionSpecialization>>, FinalSemanticAnalysisError> {
        self.facts
            .prepared_calls()?
            .project_site_payload(
                site,
                |prefix| {
                    let application = prefix.application();
                    let selected = application.selected();
                    if !selected.requires_value_callee()
                        || selected.prepared_continuation().is_some()
                    {
                        return Ok(None);
                    }
                    let origin = prefix
                        .record()
                        .function_value_origin()
                        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                    let callee = origin.callee();
                    let Some(source @ TypeKind::Function { binder, .. }) = self
                        .facts
                        .expressions()
                        .get(&callee)
                        .and_then(PreparedExpressionFact::value_type)
                    else {
                        return Ok(None);
                    };
                    if binder.is_empty() {
                        return Ok(None);
                    }
                    application
                        .function_input_specialization_with_control(callee, source, receiver)
                        .map(Some)
                        .map_err(|error| match error {
                            FunctionSpecializationSealFailure::Invariant(error) => {
                                call_storage_error(site, error)
                            }
                            FunctionSpecializationSealFailure::Projection(error) => {
                                storage_projection_error(error)
                            }
                        })
                },
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
    }

    pub(super) fn specialized_body_projection(
        &self,
        declaration: &CallableDeclarationKey,
        witness: Arc<CheckedFunctionSpecialization>,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BodyProjection, FinalSemanticAnalysisError> {
        let schema = self
            .catalogs
            .world
            .environment()
            .callable_catalog()
            .project_record(declaration)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
            .schema();
        let inventory = schema.generic_inventory();
        for count in [
            inventory.types().len(),
            inventory.consts().len(),
            inventory.effects().len(),
        ] {
            receiver.charge(
                u64::try_from(count).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
            )?;
        }
        let binder = schema.function_value_binder().map_err(|error| {
            storage_projection_error(TypeProjectionError::Instantiation(error.into()))
        })?;
        let body = SpecializedDeclarationProjection::new(binder, witness)
            .map_err(|error| storage_projection_error(TypeProjectionError::Instantiation(error)))?;
        Ok(BodyProjection::SpecializedValue(Rc::new(body)))
    }

    /// Equal shapes agree on the storage observations made by this query.
    /// Value provenance stays symbolic in the result dependencies.
    pub(super) fn application_storage(
        &self,
        site: CheckedCallSite,
        item: ItemId,
        projection: &BodyProjection,
        parent: &StorageFrame,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<ApplicationStorage, FinalSemanticAnalysisError> {
        match projection {
            BodyProjection::SelectedCall(source) => {
                self.with_selected_application(*source, |application| {
                    self.argument_storage(
                        (site, item),
                        application.body_type_arguments(),
                        application.body_const_arguments(),
                        &parent.projections,
                        receiver,
                    )
                })
            }
            BodyProjection::SpecializedValue(body) => self.argument_storage(
                (site, item),
                body.type_arguments(),
                body.const_arguments(),
                &parent.projections,
                receiver,
            ),
        }
    }

    fn argument_storage<'a>(
        &self,
        (site, item): (CheckedCallSite, ItemId),
        types: impl ExactSizeIterator<Item = ScopedTypeView<'a>>,
        lengths: impl ExactSizeIterator<Item = ScopedArrayLengthView<'a>>,
        projections: &[BodyProjection],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<ApplicationStorage, FinalSemanticAnalysisError> {
        let count = types
            .len()
            .checked_add(lengths.len())
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        receiver.charge(
            u64::try_from(count).map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        )?;
        let types = types
            .map(|ty| self.inspect_value_storage(ty.value(), projections, receiver))
            .collect::<Result<_, _>>()?;
        let zero_lengths = lengths
            .map(|length| self.project_zero_length(length.value(), projections, receiver))
            .collect::<Result<_, _>>()?;
        Ok(ApplicationStorage {
            site,
            item,
            types,
            zero_lengths,
        })
    }

    pub(super) fn inspect_value_storage(
        &self,
        ty: &TypeKind,
        projections: &[BodyProjection],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<TypeStorage, FinalSemanticAnalysisError> {
        let projected = self.project_value_storage(ty, projections, receiver)?;
        let ty = projected.as_ref().map_or(ty, |term| term.view().value());
        if receiver.contains_borrow(ty, &self.types)? {
            Ok(TypeStorage::Borrowed)
        } else if matches!(ty, TypeKind::Function { .. }) {
            Ok(TypeStorage::Callable)
        } else {
            Ok(TypeStorage::Ordinary)
        }
    }

    pub(super) fn project_value_storage(
        &self,
        ty: &TypeKind,
        projections: &[BodyProjection],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<Option<ScopedType>, FinalSemanticAnalysisError> {
        let mut projected: Option<ScopedType> = None;
        // Substituting a body's generics can expose free parameters owned
        // by the caller or by the site that specialized the function value.
        for projection in projections.iter().rev() {
            receiver.charge(1)?;
            let input = projected.as_ref().map_or(ty, |term| term.view().value());
            projected = Some(match projection {
                BodyProjection::SelectedCall(site) => {
                    self.with_selected_application(*site, |application| {
                        application
                            .project_body_storage(input, receiver)
                            .map_err(storage_projection_error)
                    })?
                }
                BodyProjection::SpecializedValue(body) => body
                    .project_type(input, receiver)
                    .map_err(storage_projection_error)?,
            });
        }
        Ok(projected)
    }

    fn project_zero_length(
        &self,
        length: &ArrayLength,
        projections: &[BodyProjection],
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        receiver.charge(1)?;
        if let ArrayLength::Const(length) = length {
            return Ok(*length == 0);
        }
        let mut length = length.clone();
        for projection in projections.iter().rev() {
            length = match projection {
                BodyProjection::SelectedCall(site) => {
                    self.with_selected_application(*site, |application| {
                        application
                            .project_body_length(&length, receiver)
                            .map_err(storage_projection_error)
                    })?
                }
                BodyProjection::SpecializedValue(body) => body
                    .project_length(&length, receiver)
                    .map_err(storage_projection_error)?,
            };
        }
        Ok(matches!(length, ArrayLength::Const(0)))
    }

    fn with_selected_application<T>(
        &self,
        site: CheckedCallSite,
        project: impl FnOnce(&PreparedCallableApplication) -> Result<T, FinalSemanticAnalysisError>,
    ) -> Result<T, FinalSemanticAnalysisError> {
        self.facts
            .prepared_calls()?
            .project_site_payload(
                site,
                |prefix| project(prefix.application()),
                |_| Err(FinalSemanticAnalysisError::InvalidOwner),
            )
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
    }
}
