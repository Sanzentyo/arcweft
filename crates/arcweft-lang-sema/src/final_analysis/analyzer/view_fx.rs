//! Final checked authority for View `.fx(value)` applications.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, LocalId},
    item::HirItemKind,
};
use arcweft_presentation::fx::{
    BUILTIN_FX_CALLABLE_CATALOG, BuiltinFxCallableParameter, BuiltinFxParameterBinding,
    FxDefinitionParameterType, FxRuntimeParameterRef, FxRuntimeType, FxRuntimeValue,
    ValueInstruction, ValueProgramSchema,
};
use arcweft_view::ViewParameterCoordinate;

use crate::{
    callable::{
        CallableGroupIndex, CallableParameterCoordinate, CallableResultSchema, CallableValidator,
        CheckedCallArgumentSlotSource, PreparedResolvedCallable,
    },
    final_analysis::{
        CheckedExpressionResolution, CheckedFxApplicationOrdinal, CheckedFxDefinition,
        CheckedViewFxBinding, CheckedViewValueInput, CheckedViewValueProgram,
        FinalSemanticAnalysisError, PreparedExpressionFact,
    },
    semantic_coordinate::{CheckedSemanticPath, SemanticCoordinateIndex},
    types::{CompileTimeFxType, TypeKind},
};

use super::{
    Analyzer,
    checked_value_program::{CheckedValueProgramSealContext, seal_checked_value_expression},
    expression_error::{
        AnalyzerExpressionConsumer, AnalyzerExpressionContext, AnalyzerExpressionError,
    },
    statements::enclosing_item,
};

impl Analyzer<'_, '_, '_> {
    pub(super) fn view_fx_runtime_parameter_overrides(
        &self,
        context: &AnalyzerExpressionContext<'_>,
        owner: ExprId,
        candidate: &PreparedResolvedCallable,
        group_index: CallableGroupIndex,
    ) -> Result<BTreeMap<CallableParameterCoordinate, TypeKind>, AnalyzerExpressionError> {
        if context.consumer() != AnalyzerExpressionConsumer::ViewFxProducer {
            return Ok(BTreeMap::new());
        }
        let group = candidate
            .schema()
            .group(group_index)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let mut overrides = BTreeMap::new();
        match candidate.schema().validator() {
            CallableValidator::BuiltinFx(row_id) => {
                let row = BUILTIN_FX_CALLABLE_CATALOG
                    .get(*row_id)
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                if row.parameters().len() != group.parameters().len() {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                for (index, (source, selected)) in
                    row.parameters().iter().zip(group.parameters()).enumerate()
                {
                    if source.binding() != BuiltinFxParameterBinding::Abi
                        || selected.index().get() != index
                    {
                        continue;
                    }
                    let Some(FxDefinitionParameterType::Runtime(runtime_type)) =
                        source.parameter_type().direct_definition_parameter_type()
                    else {
                        continue;
                    };
                    let declared = selected
                        .declared_type()
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let (actual_runtime_type, expected) =
                        crate::callable::checked_view_fx_runtime_parameter(declared)
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    if actual_runtime_type != runtime_type {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    }
                    overrides.insert(
                        CallableParameterCoordinate::new(group_index, selected.index()),
                        expected,
                    );
                }
            }
            _ => {
                let CallableResultSchema::Value(TypeKind::CompileTimeFx(
                    CompileTimeFxType::Registered(definition),
                )) = candidate
                    .result_schema_for_group(group_index)
                    .map_err(|_| AnalyzerExpressionError::rejected(owner))?
                else {
                    return Ok(overrides);
                };
                let crate::callable::CallableCandidateId::Project(declaration) = candidate.id()
                else {
                    return Err(AnalyzerExpressionError::rejected(owner));
                };
                let checked = self
                    .fx_definitions
                    .as_ref()
                    .and_then(|catalog| catalog.get(&definition))
                    .and_then(CheckedFxDefinition::project)
                    .filter(|checked| {
                        checked.declaration() == declaration
                            && checked.schema() == candidate.schema().semantic_digest()
                    })
                    .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                if checked.parameter_schema().parameters().len() != group.parameters().len() {
                    return Err(AnalyzerExpressionError::rejected(owner));
                }
                for (selected, definition_parameter) in group
                    .parameters()
                    .iter()
                    .zip(checked.parameter_schema().parameters())
                {
                    let FxDefinitionParameterType::Runtime(runtime_type) =
                        definition_parameter.parameter_type()
                    else {
                        continue;
                    };
                    let declared = selected
                        .declared_type()
                        .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    let (actual_runtime_type, expected) =
                        crate::callable::checked_view_fx_runtime_parameter(declared)
                            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
                    if actual_runtime_type != runtime_type {
                        return Err(AnalyzerExpressionError::rejected(owner));
                    }
                    overrides.insert(
                        CallableParameterCoordinate::new(group_index, selected.index()),
                        expected,
                    );
                }
            }
        }
        Ok(overrides)
    }

    /// Replaces ordinary checked Call shells only after the accepted semantic
    /// coordinate index and final call applications both exist. Sorting those
    /// accepted paths gives each View declaration one stable authored ordinal;
    /// raw HIR IDs and source spellings never enter the retained identity.
    pub(super) fn finalize_view_fx_applications(
        &mut self,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut applications = Vec::<(CheckedSemanticPath, ExprId)>::new();
        for (owner, facts) in self.facts.calls() {
            let Some(application) = facts.selected_application() else {
                continue;
            };
            if application
                .core()
                .candidates()
                .selected()
                .schema()
                .validator()
                != &CallableValidator::ViewModifier(crate::callable::ViewModifierId::Fx)
            {
                continue;
            }
            let module = self.module(owner.module())?;
            let item = enclosing_item(
                module,
                module
                    .resolve_expr(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                    .scope(),
            )?
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            if !matches!(
                module
                    .resolve_item(item)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                    .kind(),
                HirItemKind::View(_)
            ) {
                return Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: *owner });
            }
            applications.push((
                coordinates
                    .expression(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?,
                *owner,
            ));
        }
        applications.sort_by(|left, right| left.0.cmp(&right.0));

        let mut previous_root = None;
        let mut next_ordinal = 0_u32;
        for (path, owner) in applications {
            if previous_root != Some(path.root()) {
                previous_root = Some(path.root());
                next_ordinal = 0;
            }
            let ordinal = CheckedFxApplicationOrdinal::from_checked_order(next_ordinal);
            next_ordinal = next_ordinal
                .checked_add(1)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            let checked = self.checked_view_fx_modifier(owner, ordinal, coordinates)?;
            let previous = self
                .facts
                .expressions()
                .get(&owner)
                .cloned()
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
            let PreparedExpressionFact::Complete(previous) = previous else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            if previous.value_type() != Some(&TypeKind::ViewValue)
                || !matches!(previous.resolution(), CheckedExpressionResolution::Call)
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            self.facts
                .replace_existing_expression(
                    owner,
                    previous.with_resolution(CheckedExpressionResolution::ViewFxApplication(
                        Box::new(checked),
                    )),
                )
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        }
        Ok(())
    }

    fn checked_view_fx_modifier(
        &mut self,
        owner: ExprId,
        ordinal: CheckedFxApplicationOrdinal,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<crate::final_analysis::CheckedViewFxApplication, FinalSemanticAnalysisError> {
        let module = self.module(owner.module())?;
        let expression = module
            .resolve_expr(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::Call(call) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let outer = self
            .facts
            .calls()
            .get(&owner)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .cloned()
            .ok_or(FinalSemanticAnalysisError::CallResolutionFailed { owner })?;
        if outer.core().candidates().selected().schema().validator()
            != &CallableValidator::ViewModifier(crate::callable::ViewModifierId::Fx)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let slots = outer
            .core()
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .collect::<Vec<_>>();
        let [slot] = slots.as_slice() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let CheckedCallArgumentSlotSource::Expression(producer) = slot.source().raw() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if call.arguments().len() != 1 || call.arguments()[0].value() != producer {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let inner = self
            .checked_view_fx_application(module, producer, ordinal, coordinates)
            .map_err(|error| match error {
                AnalyzerExpressionError::Rejected(
                    super::expression_error::AnalyzerExpressionRejection::Unavailable { owner },
                ) => FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner },
                error => error.into_public(producer),
            })?;
        crate::final_analysis::CheckedViewFxApplication::seal_view_fx(
            inner,
            &outer,
            module,
            call,
            coordinates,
        )
        .map_err(|source| FinalSemanticAnalysisError::FxEdgePlan { owner, source })
    }

    fn checked_view_fx_application(
        &mut self,
        module: &arcweft_lang_hir::module::HirModule,
        expression: ExprId,
        ordinal: CheckedFxApplicationOrdinal,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<
        crate::final_analysis::fx_application::CheckedFxApplication<
            crate::final_analysis::CheckedViewFxBinding,
        >,
        AnalyzerExpressionError,
    > {
        let actual = self
            .facts
            .expressions()
            .get(&expression)
            .and_then(PreparedExpressionFact::value_type)
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        let TypeKind::CompileTimeFx(actual) = actual else {
            return Err(AnalyzerExpressionError::rejected(expression));
        };
        let actual = actual.clone();
        let application = self
            .facts
            .calls()
            .get(&expression)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .cloned()
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        let mut definitions = self
            .fx_definitions
            .take()
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        let result = crate::final_analysis::fx_application::seal_shared_fx_application(
            expression,
            &application,
            &actual,
            ordinal,
            &mut definitions,
            |request, selected, source| {
                let declared = selected.declared_type().ok_or(())?;
                match request {
                    crate::final_analysis::fx_application::CheckedFxBindingRequest::Builtin(
                        parameter,
                    ) => self
                        .checked_view_builtin_binding(
                            module,
                            source,
                            parameter,
                            declared,
                            coordinates,
                        )
                        .map_err(|_| ()),
                    crate::final_analysis::fx_application::CheckedFxBindingRequest::Project(
                        expected,
                    ) => {
                        if let Ok(value) = self.checked_compile_time_value(module, source, declared)
                            && let Ok(value) =
                                crate::final_analysis::fx_application::checked_project_fx_argument(
                                    expected, &value,
                                )
                        {
                            return Ok(crate::final_analysis::CheckedViewFxBinding::closed(
                                crate::final_analysis::CheckedContentFxBinding::abi(value),
                            ));
                        }
                        let arcweft_presentation::fx::FxDefinitionParameterType::Runtime(
                            runtime_type,
                        ) = expected
                        else {
                            return Err(());
                        };
                        self.seal_checked_view_value_program(
                            module,
                            source,
                            runtime_type,
                            coordinates,
                        )
                        .map(crate::final_analysis::CheckedViewFxBinding::reactive)
                        .map_err(|_| ())
                    }
                }
            },
        );
        self.fx_definitions = Some(definitions);
        result.map_err(|_| AnalyzerExpressionError::rejected(expression))
    }

    /// View-only binding admission for a value already selected by the shared
    /// producer seal.  Closed values use the same Content binding converter;
    /// the only View-specific extension is a self-validated reactive program
    /// for runtime ABI parameters.
    fn checked_view_builtin_binding(
        &mut self,
        module: &arcweft_lang_hir::module::HirModule,
        expression: ExprId,
        parameter: BuiltinFxCallableParameter,
        declared_source_type: &TypeKind,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<CheckedViewFxBinding, AnalyzerExpressionError> {
        let declared = parameter
            .parameter_type()
            .direct_definition_parameter_type()
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        if let Ok(value) = self.checked_compile_time_value(module, expression, declared_source_type)
            && let Ok(binding) = crate::final_analysis::fx_application::checked_builtin_fx_binding(
                parameter.parameter_type(),
                &value,
            )
        {
            return Ok(CheckedViewFxBinding::closed(binding));
        }
        if parameter.binding() != BuiltinFxParameterBinding::Abi {
            return Err(AnalyzerExpressionError::rejected(expression));
        }
        let FxDefinitionParameterType::Runtime(runtime_type) = declared else {
            return Err(AnalyzerExpressionError::rejected(expression));
        };
        let program =
            self.seal_checked_view_value_program(module, expression, runtime_type, coordinates)?;
        Ok(CheckedViewFxBinding::reactive(program))
    }

    fn seal_checked_view_value_program(
        &mut self,
        module: &arcweft_lang_hir::module::HirModule,
        expression: ExprId,
        expected: FxRuntimeType,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<CheckedViewValueProgram, AnalyzerExpressionError> {
        let parameters = checked_view_parameters(self, module, expression, coordinates)?;
        let mut context = ViewValueProgramSealContext {
            analyzer: self,
            module,
            parameters,
            inputs: Vec::new(),
            slots: BTreeMap::new(),
        };
        let (mut instructions, actual) = seal_checked_value_expression(&mut context, expression)?;
        if actual != expected {
            return Err(AnalyzerExpressionError::rejected(expression));
        }
        instructions.push(ValueInstruction::Return);
        let (inputs, instructions) =
            canonicalize_view_value_inputs(context.inputs, instructions, actual)
                .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        CheckedViewValueProgram::seal(inputs, instructions, actual)
            .map_err(|_| AnalyzerExpressionError::rejected(expression))
    }
}

fn checked_view_parameters(
    analyzer: &Analyzer<'_, '_, '_>,
    module: &arcweft_lang_hir::module::HirModule,
    expression: ExprId,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<BTreeMap<LocalId, CheckedViewValueInput>, AnalyzerExpressionError> {
    let expression_node = module
        .resolve_expr(expression)
        .map_err(|_| AnalyzerExpressionError::rejected(expression))?;
    let item = enclosing_item(module, expression_node.scope())
        .map_err(AnalyzerExpressionError::fatal)?
        .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
    let HirItemKind::View(view) = module
        .resolve_item(item)
        .map_err(|_| AnalyzerExpressionError::rejected(expression))?
        .kind()
    else {
        return Err(AnalyzerExpressionError::rejected(expression));
    };
    let root = coordinates
        .expression(expression)
        .map_err(|_| AnalyzerExpressionError::rejected(expression))?
        .root();
    let mut output = BTreeMap::new();
    for (index, parameter) in view.parameters().iter().enumerate() {
        if parameter.default().is_some() || parameter.locals().len() != 1 {
            return Err(AnalyzerExpressionError::rejected(expression));
        }
        let coordinate = ViewParameterCoordinate::try_from_index(index)
            .ok_or_else(|| AnalyzerExpressionError::rejected(expression))?;
        for local in parameter.locals() {
            if coordinates
                .binding(*local)
                .map_err(|_| AnalyzerExpressionError::rejected(expression))?
                .root()
                != root
            {
                return Err(AnalyzerExpressionError::rejected(expression));
            }
            let Some(value_type) = analyzer
                .facts
                .locals()
                .get(local)
                .and_then(view_runtime_type)
            else {
                continue;
            };
            if output
                .insert(*local, CheckedViewValueInput::new(coordinate, value_type))
                .is_some()
            {
                return Err(AnalyzerExpressionError::rejected(expression));
            }
        }
    }
    Ok(output)
}

/// Canonical program input order is accepted View parameter coordinate order,
/// independent of first use in an expression. Load instructions are remapped
/// to the sorted dense schema before the owner validates the program.
fn canonicalize_view_value_inputs(
    inputs: Vec<CheckedViewValueInput>,
    instructions: Vec<ValueInstruction>,
    return_type: FxRuntimeType,
) -> Option<(Vec<CheckedViewValueInput>, Vec<ValueInstruction>)> {
    let mut indexed = inputs.into_iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by_key(|(_, input)| input.parameter());
    let canonical = indexed.iter().map(|(_, input)| *input).collect::<Vec<_>>();
    let schema = ValueProgramSchema::new(
        canonical.iter().map(|input| input.value_type()).collect(),
        Vec::new(),
        return_type,
    );
    let mut remap = BTreeMap::new();
    for (new, (old, input)) in indexed.into_iter().enumerate() {
        let parameter = schema.parameter_ref(new)?;
        if parameter.runtime_type() != input.value_type() || remap.insert(old, parameter).is_some()
        {
            return None;
        }
    }
    let instructions = instructions
        .into_iter()
        .map(|instruction| match instruction {
            ValueInstruction::LoadParameter { parameter } => remap
                .get(&usize::from(parameter.slot().get()))
                .copied()
                .map(|parameter| ValueInstruction::LoadParameter { parameter }),
            instruction => Some(instruction),
        })
        .collect::<Option<Vec<_>>>()?;
    Some((canonical, instructions))
}

fn view_runtime_type(ty: &TypeKind) -> Option<FxRuntimeType> {
    use crate::types::CompileTimeScalarKind;
    Some(match ty {
        TypeKind::Bool => FxRuntimeType::Bool,
        TypeKind::I32 => FxRuntimeType::I32,
        TypeKind::U32 => FxRuntimeType::U32,
        TypeKind::F32 => FxRuntimeType::F32,
        TypeKind::Duration => FxRuntimeType::Seconds,
        TypeKind::CompileTimeScalar(value) => match value.kind() {
            CompileTimeScalarKind::Milli | CompileTimeScalarKind::Ratio => FxRuntimeType::F32,
            CompileTimeScalarKind::Length => FxRuntimeType::Length,
            CompileTimeScalarKind::Angle => FxRuntimeType::Angle,
            CompileTimeScalarKind::Color => FxRuntimeType::Color,
            CompileTimeScalarKind::PublicId => return None,
        },
        TypeKind::FixedVector(vector)
            if vector.dimensions() == crate::callable::VectorDimensions::Two
                && matches!(vector.component(), TypeKind::F32) =>
        {
            FxRuntimeType::Vec2
        }
        _ => return None,
    })
}

struct ViewValueProgramSealContext<'a, 'project, 'catalog, 'control> {
    analyzer: &'a mut Analyzer<'project, 'catalog, 'control>,
    module: &'a arcweft_lang_hir::module::HirModule,
    parameters: BTreeMap<LocalId, CheckedViewValueInput>,
    inputs: Vec<CheckedViewValueInput>,
    slots: BTreeMap<ViewParameterCoordinate, usize>,
}

impl CheckedValueProgramSealContext for ViewValueProgramSealContext<'_, '_, '_, '_> {
    type Error = AnalyzerExpressionError;

    fn expression(
        &mut self,
        owner: ExprId,
    ) -> Result<arcweft_lang_hir::expr::HirExpr, Self::Error> {
        self.module
            .resolve_expr(owner)
            .cloned()
            .map_err(|_| AnalyzerExpressionError::rejected(owner))
    }

    fn inferred_type(&mut self, owner: ExprId) -> Result<FxRuntimeType, Self::Error> {
        self.analyzer
            .facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::value_type)
            .and_then(view_runtime_type)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))
    }

    fn constant(
        &mut self,
        owner: ExprId,
        expected: FxRuntimeType,
    ) -> Result<Option<FxRuntimeValue>, Self::Error> {
        let ty = self
            .analyzer
            .facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::value_type)
            .cloned()
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let value = match self
            .analyzer
            .checked_compile_time_value(self.module, owner, &ty)
        {
            Ok(value) => value,
            Err(AnalyzerExpressionError::Rejected(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let value = crate::final_analysis::fx_application::checked_project_fx_argument(
            FxDefinitionParameterType::Runtime(expected),
            &value,
        )
        .map_err(|_| AnalyzerExpressionError::rejected(owner))?;
        let arcweft_presentation::fx::FxDefinitionArgumentValue::Runtime(value) = value else {
            return Err(AnalyzerExpressionError::rejected(owner));
        };
        Ok(Some(value))
    }

    fn input(&mut self, owner: ExprId) -> Result<Option<FxRuntimeParameterRef>, Self::Error> {
        let Some(CheckedExpressionResolution::Value(
            crate::final_analysis::CheckedValueResolution::Local(local),
        )) = self
            .analyzer
            .facts
            .expressions()
            .get(&owner)
            .and_then(PreparedExpressionFact::checked_resolution)
        else {
            return Ok(None);
        };
        let input = self
            .parameters
            .get(local)
            .copied()
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let index = if let Some(index) = self.slots.get(&input.parameter()).copied() {
            index
        } else {
            let index = self.inputs.len();
            self.inputs.push(input);
            self.slots.insert(input.parameter(), index);
            index
        };
        let schema = ValueProgramSchema::new(
            self.inputs.iter().map(|input| input.value_type()).collect(),
            Vec::new(),
            input.value_type(),
        );
        Ok(schema.parameter_ref(index))
    }

    fn context_slot(
        &mut self,
        _owner: ExprId,
    ) -> Result<Option<arcweft_presentation::fx::FxContextSlot>, Self::Error> {
        Ok(None)
    }

    fn call(
        &mut self,
        owner: ExprId,
    ) -> Result<(crate::callable::CallableCandidateId, Vec<ExprId>), Self::Error> {
        let application = self
            .analyzer
            .facts
            .calls()
            .get(&owner)
            .and_then(crate::callable::CallTargetFacts::selected_application)
            .ok_or_else(|| AnalyzerExpressionError::rejected(owner))?;
        let sources = application
            .core()
            .execution()
            .arguments()
            .iter()
            .flat_map(|argument| argument.slots())
            .map(|slot| match slot.source().raw() {
                CheckedCallArgumentSlotSource::Expression(expression) => Ok(expression),
                CheckedCallArgumentSlotSource::CompactNumericElement { .. } => {
                    Err(AnalyzerExpressionError::rejected(owner))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            application.core().candidates().selected().id().clone(),
            sources,
        ))
    }

    fn invalid(&self, owner: ExprId) -> Self::Error {
        AnalyzerExpressionError::rejected(owner)
    }
}
