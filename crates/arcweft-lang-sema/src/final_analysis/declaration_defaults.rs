//! Declaration-default facts derived from the selected execution and parameter inventories.

use super::{FinalSemanticAnalysis, FinalSemanticAnalysisControl, FinalSemanticAnalysisError};
use arcweft_lang_hir::{
    identity::ExprId, project::HirAnalysisProjectView, symbol::ProjectSymbolTable,
};
use std::collections::BTreeMap;

pub(super) fn checked_view_parameter_defaults(
    analysis: &FinalSemanticAnalysis,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    facts: &crate::callable::CheckedCallableFacts,
    coordinates: &crate::semantic_coordinate::SemanticCoordinateIndex<'_, '_>,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<
    BTreeMap<
        crate::callable::CallableParameterCoordinate,
        crate::callable::CheckedDeclarationDefault,
    >,
    FinalSemanticAnalysisError,
> {
    use crate::callable::{
        CallableCandidateId, CallableGroupIndex, CallableParameterCoordinate,
        CallableParameterIndex, CheckedDeclarationDefault,
    };
    use crate::semantic_coordinate::StableCheckedValueCoordinate;
    let mut defaults = BTreeMap::new();
    let CallableCandidateId::Project(declaration) = facts.record().id() else {
        return Ok(defaults);
    };
    if declaration.owner() != arcweft_lang_hir::symbol::CallableDeclarationOwner::View {
        return Ok(defaults);
    }
    let symbol = symbols
        .callable(declaration)
        .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
    let module = project
        .modules()
        .find_map(|(_, module)| {
            (module.module_id() == symbol.source_item().module()).then_some(module.as_ref())
        })
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
    let arcweft_lang_hir::item::HirItemKind::View(view) = module
        .resolve_item(symbol.source_item())
        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?
        .kind()
    else {
        return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
    };
    for (index, parameter) in view.parameters().iter().enumerate() {
        control.check()?;
        let Some(source) = parameter.default() else {
            continue;
        };
        let position = CallableParameterCoordinate::new(
            CallableGroupIndex::try_from_usize(0)
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
            CallableParameterIndex::try_from_usize(index)
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
        );
        let checked = analysis
            .expression(source)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: source })?;
        let expected = facts
            .signature()
            .parameter_type(position)
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let result = checked
            .value_type()
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: source })?;
        if !expected.accepts(result) {
            return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: source });
        }
        if !checked.effects().is_empty() {
            return Err(FinalSemanticAnalysisError::ViewParameterDefaultEffects { owner: source });
        }
        let execution = analysis
            .expression_execution_region(source)
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        if execution.suspension() != super::CheckedSuspensionRole::NonSuspending {
            return Err(FinalSemanticAnalysisError::ViewParameterDefaultSuspension {
                owner: source,
            });
        }
        let captures = checked_declaration_default_captures(
            analysis,
            module,
            symbol,
            source,
            &execution,
            coordinates,
        )?;
        if let Some(capture) = captures
            .iter()
            .find(|capture| capture.parameter() >= position)
        {
            return Err(
                FinalSemanticAnalysisError::ViewParameterDefaultForwardInput {
                    owner: source,
                    parameter: capture.parameter(),
                },
            );
        }
        let coordinate = StableCheckedValueCoordinate::Expression(
            coordinates
                .expression(source)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?,
        );
        let expression = super::semantic_transcript::checked_declaration_default_expression_digest(
            analysis, project, source, control,
        )
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        defaults.insert(
            position,
            CheckedDeclarationDefault::new(
                source,
                coordinate,
                (
                    expected.semantic_identity_digest()?,
                    result.semantic_identity_digest()?,
                ),
                crate::effect_row::EffectRow::closed(checked.effects().clone()),
                execution.suspension(),
                execution.control(),
                expression,
                captures,
            ),
        );
    }
    Ok(defaults)
}

pub(super) fn checked_declaration_default_captures(
    analysis: &FinalSemanticAnalysis,
    module: &arcweft_lang_hir::module::HirModule,
    symbol: &arcweft_lang_hir::symbol::CallableSymbol,
    root: ExprId,
    execution: &super::execution_regions::CheckedExecutionRegion,
    coordinates: &crate::semantic_coordinate::SemanticCoordinateIndex<'_, '_>,
) -> Result<Box<[crate::callable::CheckedDeclarationDefaultCapture]>, FinalSemanticAnalysisError> {
    use arcweft_lang_hir::item::HirItemKind;

    let item = module
        .resolve_item(symbol.source_item())
        .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
    let groups = match item.kind() {
        HirItemKind::Function(function) => function
            .parameter_groups()
            .iter()
            .map(|group| group.parameters())
            .collect::<Vec<_>>(),
        HirItemKind::View(view) => vec![view.parameters()],
        _ => return Err(FinalSemanticAnalysisError::CheckedCallableCatalog),
    };
    let executed_expressions = execution.expressions();
    if !executed_expressions.contains(&root)
        || executed_expressions
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || execution
            .statements()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
    }
    let mut parameters = BTreeMap::new();
    for (group_index, group) in groups.iter().enumerate() {
        let group_coordinate = crate::callable::CallableGroupIndex::try_from_usize(group_index)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        for (parameter_index, parameter) in group.iter().enumerate() {
            let parameter_coordinate =
                crate::callable::CallableParameterIndex::try_from_usize(parameter_index)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let coordinate = crate::callable::CallableParameterCoordinate::new(
                group_coordinate,
                parameter_coordinate,
            );
            for local in parameter.locals() {
                if parameters.insert(*local, (coordinate, parameter)).is_some() {
                    return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                }
            }
        }
    }

    let mut used = BTreeMap::<
        crate::callable::CallableParameterCoordinate,
        Vec<crate::callable::CheckedDeclarationDefaultCaptureLocal>,
    >::new();
    let mut collector =
        super::free_capture::CheckedFreeLocalCollector::new(root, coordinates, |local| {
            analysis.local(local).map(|binding| binding.ty().clone())
        })?;
    for &owner in executed_expressions {
        collector.include(analysis.checked_capture_inputs(owner)?)?;
    }
    for &owner in execution.statements() {
        let statement = analysis
            .statement(owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        collector.include(
            super::free_capture::CheckedCaptureExpression::from_statement(owner, statement)?,
        )?;
    }
    for &owner in execution.places() {
        let place = analysis
            .expression(owner)
            .and_then(super::CheckedExpression::mutable_place)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let ty = analysis
            .local(place.local_id())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
            .ty();
        collector.include(super::free_capture::CheckedCaptureExpression::from_place(
            owner, &place, ty,
        )?)?;
    }
    for capture in collector.finish() {
        let (parameter, _) = parameters
            .get(&capture.local())
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        used.entry(*parameter).or_default().push(
            crate::callable::CheckedDeclarationDefaultCaptureLocal::new(
                capture.local(),
                capture.origin().clone(),
                capture.ty().clone(),
            ),
        );
    }
    let mut captures = Vec::new();
    for (group_index, group) in groups.iter().enumerate() {
        let group_coordinate = crate::callable::CallableGroupIndex::try_from_usize(group_index)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        for (parameter_index, parameter) in group.iter().enumerate() {
            let parameter_coordinate =
                crate::callable::CallableParameterIndex::try_from_usize(parameter_index)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let coordinate = crate::callable::CallableParameterCoordinate::new(
                group_coordinate,
                parameter_coordinate,
            );
            let Some(mut used_locals) = used.remove(&coordinate) else {
                continue;
            };
            used_locals.sort_by(|left, right| left.origin().cmp(right.origin()));
            let pattern = analysis
                .pattern(parameter.pattern())
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let pattern_digest =
                super::semantic_transcript::checked_declaration_default_pattern_digest(
                    analysis,
                    module,
                    parameter.pattern(),
                )
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let binding_evidence = parameter
                .locals()
                .iter()
                .map(|local| {
                    let checked = analysis.local(*local).ok_or(
                        FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *local },
                    )?;
                    let origin = coordinates
                        .binding(*local)
                        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                    Ok(crate::callable::CheckedDeclarationDefaultCaptureLocal::new(
                        *local,
                        origin,
                        checked.ty().clone(),
                    ))
                })
                .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?
                .into_boxed_slice();
            captures.push(crate::callable::CheckedDeclarationDefaultCapture::new(
                coordinate,
                parameter.pattern(),
                pattern_digest,
                parameter.locals().to_vec().into_boxed_slice(),
                binding_evidence,
                used_locals.into_boxed_slice(),
                pattern.ty().clone(),
            ));
        }
    }
    if !used.is_empty() {
        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
    }
    Ok(captures.into_boxed_slice())
}
