//! Declaration-default facts derived from the selected execution and parameter inventories.

use super::{
    CheckedExpressionResolution, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError,
};
use arcweft_lang_hir::{
    identity::ExprId, project::HirAnalysisProjectView, symbol::ProjectSymbolTable,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn checked_view_parameter_defaults(
    analysis: &FinalSemanticAnalysis,
    project: HirAnalysisProjectView<'_>,
    symbols: &ProjectSymbolTable,
    facts: &crate::callable::CheckedCallableFacts,
    executable_suspensions: &BTreeMap<
        ExprId,
        super::statement_effects::PreparedExecutableSuspensionRow,
    >,
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
        let execution = executable_suspensions
            .get(&source)
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
            execution.expressions(),
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
    executed_expressions: &[ExprId],
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
    let root_path = coordinates
        .expression_evidence(root)
        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
        .into_coordinate();
    if !executed_expressions.contains(&root)
        || executed_expressions
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
    let mut captured = BTreeSet::new();
    for &owner in executed_expressions {
        let checked = analysis
            .expression(owner)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        let mut local_uses = Vec::new();
        if let Some(local) = checked.execution_local_use() {
            let local_ty = analysis
                .local(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
            if checked.source_value_type() != Some(local_ty.ty()) {
                return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
            }
            local_uses.push(local);
        }
        match checked.resolution() {
            CheckedExpressionResolution::Closure(closure) => {
                for capture in closure.captures() {
                    let binding = analysis
                        .capture(capture.capture())
                        .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                    let local_ty = analysis.local(capture.local()).ok_or(
                        FinalSemanticAnalysisError::LocalTypeUnavailable {
                            owner: capture.local(),
                        },
                    )?;
                    if binding.ty() != local_ty.ty() {
                        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                    }
                    local_uses.push(capture.local());
                }
            }
            CheckedExpressionResolution::ImplicitCallable(callable) => {
                for capture in callable.captures() {
                    let local_ty = analysis.local(capture.lookup_local()).ok_or(
                        FinalSemanticAnalysisError::LocalTypeUnavailable {
                            owner: capture.lookup_local(),
                        },
                    )?;
                    if local_ty.ty().semantic_identity_digest()? != capture.value_type()
                        || coordinates
                            .binding(capture.lookup_local())
                            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                            != *capture.origin()
                    {
                        return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
                    }
                    local_uses.push(capture.lookup_local());
                }
            }
            _ => {}
        }
        for local in local_uses {
            let local_ty = analysis
                .local(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
            let origin = coordinates
                .binding(local)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            if !origin.path().is_at_or_below(&root_path) && captured.insert(local) {
                let (parameter, _) = parameters
                    .get(&local)
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                used.entry(*parameter).or_default().push(
                    crate::callable::CheckedDeclarationDefaultCaptureLocal::new(
                        local,
                        origin,
                        local_ty.ty().clone(),
                    ),
                );
            }
        }
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
