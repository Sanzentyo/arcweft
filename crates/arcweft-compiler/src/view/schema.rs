//! Typed lowering for executable View value programs.
//!
//! View source expressions are compiled once into the shared closed value
//! instruction model. State/local projections become typed input slots; source
//! strings and expression digests are never retained as executable fallbacks.

use std::collections::BTreeMap;

use arcweft_bundle::{
    container::BundleDigest,
    resource_codec::view::{ViewValueInputNamespace, ViewValueInputResource, ViewValueInputSource},
};
use arcweft_lang_syntax::{
    ast::{
        pattern::Pattern,
        view::{ViewForEach, ViewMatchArm},
    },
    expr::{BinaryOp, CallArg, Expr, UnaryOp},
};
use arcweft_presentation::fx::{
    FxRuntimeType, FxRuntimeValue, ValueInstruction, ValueProgramSchema,
    ValueProgramValidationError,
};
use arcweft_view::{ViewValueProgram, ViewValueProgramId};
use thiserror::Error;

const MAX_VIEW_STATE_PROJECTIONS: usize = 256;
const MAX_VIEW_PARAMETER_PROJECTIONS: usize = 256;

mod literal;

use literal::{emit_literal, infer_literal_type};

#[derive(Clone, Debug)]
struct PendingProgram {
    id: ViewValueProgramId,
    return_type: FxRuntimeType,
    instructions: Vec<ValueInstruction>,
}

#[derive(Clone, Copy, Debug)]
struct InputSlot {
    slot: u16,
    value_type: FxRuntimeType,
}

/// Complete typed output moved into one product View program resource.
pub(super) struct CompiledViewValues {
    pub(super) programs: Vec<ViewValueProgram>,
    pub(super) inputs: Vec<ViewValueInputResource>,
}

/// Stateful compiler that gives every program in a View inventory one common
/// typed input schema.
#[derive(Default)]
pub(super) struct ViewValueProgramCompiler {
    pending: Vec<PendingProgram>,
    inputs: Vec<ViewValueInputResource>,
    input_slots: BTreeMap<(ViewValueInputNamespace, ViewValueInputSource), InputSlot>,
    parameter_types: Vec<FxRuntimeType>,
    state_types: Vec<FxRuntimeType>,
    current_view: Option<String>,
    current_parameters: BTreeMap<String, InputSlot>,
    local_types: BTreeMap<String, FxRuntimeType>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum ViewValueCompileError {
    #[error("View value expression `{expression}` requires an expected scalar type")]
    MissingExpectedType { expression: String },
    #[error("View value expression `{expression}` is not supported by the closed value program")]
    UnsupportedExpression { expression: String },
    #[error(
        "View Await source requires a typed state or parameter projection; callable Await sources need a typed request contract"
    )]
    UnsupportedAwaitSource,
    #[error("View value literal `{literal}` is invalid for {expected:?}: {reason}")]
    InvalidLiteral {
        literal: String,
        expected: FxRuntimeType,
        reason: String,
    },
    #[error(
        "View projection {projection:?} was first typed as {existing:?} and cannot also be {requested:?}"
    )]
    ProjectionTypeConflict {
        projection: ViewValueInputSource,
        existing: FxRuntimeType,
        requested: FxRuntimeType,
    },
    #[error("View value program inventory exceeds its {limit} state projections")]
    TooManyStateProjections { limit: usize },
    #[error("View value program inventory exceeds its {limit} parameter projections")]
    TooManyParameterProjections { limit: usize },
    #[error("View value compilation requires an active View definition")]
    MissingDefinitionContext,
    #[error("View value program inventory exceeds u32::MAX programs")]
    TooManyPrograms,
    #[error("View local binding requires a single typed identifier pattern")]
    UnsupportedLocalPattern,
    #[error("View match pattern `{pattern}` cannot be represented by the closed scalar program")]
    UnsupportedMatchPattern { pattern: String },
    #[error(transparent)]
    InvalidProgram(#[from] ValueProgramValidationError),
}

impl ViewValueProgramCompiler {
    pub(super) fn begin_definition(
        &mut self,
        view: &str,
        parameters: impl IntoIterator<Item = (String, FxRuntimeType)>,
    ) -> Result<BTreeMap<String, u16>, ViewValueCompileError> {
        self.current_view = Some(view.to_owned());
        self.current_parameters.clear();
        self.local_types.clear();
        let mut slots = BTreeMap::new();
        for (name, value_type) in parameters {
            let source = ViewValueInputSource::DefinitionParameter {
                view: view.to_owned(),
                name: name.clone(),
            };
            let slot =
                self.register_input(ViewValueInputNamespace::Parameter, source, value_type)?;
            self.current_parameters
                .insert(name.clone(), InputSlot { slot, value_type });
            slots.insert(name, slot);
        }
        Ok(slots)
    }

    pub(super) fn is_local(&self, name: &str) -> bool {
        self.local_types.contains_key(name)
    }

    pub(super) fn compile(
        &mut self,
        expression: &Expr,
        expected: Option<FxRuntimeType>,
    ) -> Result<ViewValueProgramId, ViewValueCompileError> {
        self.compile_with_type(expression, expected)
            .map(|(id, _)| id)
    }

    pub(super) fn compile_condition(
        &mut self,
        expression: &Expr,
    ) -> Result<ViewValueProgramId, ViewValueCompileError> {
        self.compile(expression, Some(FxRuntimeType::Bool))
    }

    pub(super) fn compile_local(
        &mut self,
        pattern: &Pattern,
        expression: &Expr,
    ) -> Result<(String, ViewValueProgramId), ViewValueCompileError> {
        let binding = pattern
            .simple_binding_name()
            .ok_or(ViewValueCompileError::UnsupportedLocalPattern)?
            .to_owned();
        let (program, value_type) = self.compile_with_type(expression, None)?;
        self.local_types.insert(binding.clone(), value_type);
        Ok((binding, program))
    }

    pub(super) fn compile_match_condition(
        &mut self,
        scrutinee: &Expr,
        arm: &ViewMatchArm,
    ) -> Result<ViewValueProgramId, ViewValueCompileError> {
        let mut instructions = Vec::new();
        match arm.pattern() {
            Pattern::Discard | Pattern::Ident(_) | Pattern::MutIdent(_) | Pattern::Typed { .. } => {
                instructions.push(ValueInstruction::Constant {
                    value: FxRuntimeValue::Bool(true),
                });
            }
            Pattern::Literal(pattern) => {
                let pattern_type = infer_literal_type(pattern).ok_or_else(|| {
                    ViewValueCompileError::UnsupportedMatchPattern {
                        pattern: format!("{:?}", arm.pattern()),
                    }
                })?;
                self.emit_expression(scrutinee, Some(pattern_type), &mut instructions)?;
                self.emit_expression(pattern, Some(pattern_type), &mut instructions)?;
                instructions.push(ValueInstruction::Equal);
            }
            Pattern::Variant {
                path,
                name,
                payload: None,
            } => {
                self.emit_expression(scrutinee, Some(FxRuntimeType::I32), &mut instructions)?;
                let symbol = path
                    .as_ref()
                    .map_or_else(|| name.clone(), |path| format!("{path}.{name}"));
                instructions.push(ValueInstruction::Constant {
                    value: FxRuntimeValue::I32(symbol_discriminant(&symbol)),
                });
                instructions.push(ValueInstruction::Equal);
            }
            Pattern::Entity(entity) => {
                self.emit_expression(scrutinee, Some(FxRuntimeType::I32), &mut instructions)?;
                instructions.push(ValueInstruction::Constant {
                    value: FxRuntimeValue::I32(symbol_discriminant(entity.body())),
                });
                instructions.push(ValueInstruction::Equal);
            }
            pattern => {
                return Err(ViewValueCompileError::UnsupportedMatchPattern {
                    pattern: format!("{pattern:?}"),
                });
            }
        }
        if let Some(guard) = arm.guard() {
            self.emit_expression(guard, Some(FxRuntimeType::Bool), &mut instructions)?;
            instructions.push(ValueInstruction::And);
        }
        self.finish_pending(FxRuntimeType::Bool, instructions)
    }

    pub(super) fn compile_repeat_source(
        &mut self,
        source: &Expr,
    ) -> Result<ViewValueProgramId, ViewValueCompileError> {
        let count = match source {
            Expr::BracketSeq(items) => Some(items.len()),
            Expr::NumericBracketSeq(items) => Some(items.len()),
            _ => None,
        };
        if let Some(count) = count {
            let count =
                i32::try_from(count).map_err(|_| ViewValueCompileError::InvalidLiteral {
                    literal: count.to_string(),
                    expected: FxRuntimeType::I32,
                    reason: "repeat source length exceeds i32::MAX".to_owned(),
                })?;
            return self.finish_pending(
                FxRuntimeType::I32,
                vec![ValueInstruction::Constant {
                    value: FxRuntimeValue::I32(count),
                }],
            );
        }
        self.compile(source, Some(FxRuntimeType::I32))
    }

    pub(super) fn compile_repeat_key(
        &mut self,
        repeat: &ViewForEach,
    ) -> Result<ViewValueProgramId, ViewValueCompileError> {
        if let Some(key) = repeat.key() {
            return self.compile(key, Some(FxRuntimeType::I32));
        }
        let binding = repeat
            .pattern()
            .simple_binding_name()
            .unwrap_or("_item")
            .to_owned();
        let view = self
            .current_view
            .clone()
            .ok_or(ViewValueCompileError::MissingDefinitionContext)?;
        let mut instructions = Vec::new();
        self.emit_input(
            ViewValueInputNamespace::State,
            ViewValueInputSource::RepeatOrdinal { view, binding },
            FxRuntimeType::I32,
            &mut instructions,
        )?;
        self.finish_pending(FxRuntimeType::I32, instructions)
    }

    pub(super) fn compile_await_source(
        &mut self,
        source: &Expr,
    ) -> Result<ViewValueProgramId, ViewValueCompileError> {
        if matches!(source, Expr::Call(_) | Expr::Await(_)) {
            return Err(ViewValueCompileError::UnsupportedAwaitSource);
        }
        self.compile(source, Some(FxRuntimeType::I32))
    }

    pub(super) fn finish(self) -> Result<CompiledViewValues, ViewValueCompileError> {
        let schema = |return_type| {
            ValueProgramSchema::new(
                self.parameter_types.clone(),
                self.state_types.clone(),
                return_type,
            )
        };
        let programs = self
            .pending
            .into_iter()
            .map(|pending| {
                ViewValueProgram::validate(
                    pending.id,
                    schema(pending.return_type),
                    pending.instructions,
                )
                .map_err(ViewValueCompileError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CompiledViewValues {
            programs,
            inputs: self.inputs,
        })
    }

    fn compile_with_type(
        &mut self,
        expression: &Expr,
        expected: Option<FxRuntimeType>,
    ) -> Result<(ViewValueProgramId, FxRuntimeType), ViewValueCompileError> {
        let mut instructions = Vec::new();
        let return_type = self.emit_expression(expression, expected, &mut instructions)?;
        let id = self.finish_pending(return_type, instructions)?;
        Ok((id, return_type))
    }

    fn finish_pending(
        &mut self,
        return_type: FxRuntimeType,
        mut instructions: Vec<ValueInstruction>,
    ) -> Result<ViewValueProgramId, ViewValueCompileError> {
        let id = ViewValueProgramId(
            u32::try_from(self.pending.len())
                .map_err(|_| ViewValueCompileError::TooManyPrograms)?,
        );
        instructions.push(ValueInstruction::Return);
        self.pending.push(PendingProgram {
            id,
            return_type,
            instructions,
        });
        Ok(id)
    }

    fn emit_expression(
        &mut self,
        expression: &Expr,
        expected: Option<FxRuntimeType>,
        instructions: &mut Vec<ValueInstruction>,
    ) -> Result<FxRuntimeType, ViewValueCompileError> {
        match expression {
            Expr::Literal(literal) => emit_literal(literal, expected, instructions),
            Expr::Path(path) => self.emit_path(path.as_label(), expected, instructions),
            Expr::Select(select) => {
                let path = Expr::Select(select.clone())
                    .dotted_selector_label()
                    .ok_or_else(|| unsupported(expression))?;
                self.emit_path(&path, expected, instructions)
            }
            Expr::LifetimePath { key, .. } => {
                let value_type = expected.ok_or_else(|| missing_expected(expression))?;
                self.emit_input(
                    ViewValueInputNamespace::State,
                    ViewValueInputSource::LifetimeProjection {
                        scope: key.scope().as_str().to_owned(),
                        path: key.path().to_vec(),
                    },
                    value_type,
                    instructions,
                )?;
                Ok(value_type)
            }
            Expr::ShortVariant(name) => {
                require_expected(expression, expected, FxRuntimeType::I32)?;
                instructions.push(ValueInstruction::Constant {
                    value: FxRuntimeValue::I32(symbol_discriminant(name.as_str())),
                });
                Ok(FxRuntimeType::I32)
            }
            Expr::EntityRef(reference) => {
                require_expected(expression, expected, FxRuntimeType::I32)?;
                instructions.push(ValueInstruction::Constant {
                    value: FxRuntimeValue::I32(symbol_discriminant(&reference.canonical_body())),
                });
                Ok(FxRuntimeType::I32)
            }
            Expr::Unary { op, expr } => {
                let operand_type = self.emit_expression(expr, expected, instructions)?;
                instructions.push(match op {
                    UnaryOp::Not => ValueInstruction::Not,
                    UnaryOp::Neg => ValueInstruction::Neg,
                });
                Ok(operand_type)
            }
            Expr::Binary { lhs, op, rhs } => {
                self.emit_binary(lhs, *op, rhs, expected, instructions)
            }
            Expr::If {
                condition,
                then_branch,
                else_branch: Some(else_branch),
            } => {
                self.emit_expression(condition, Some(FxRuntimeType::Bool), instructions)?;
                let value_type = self.emit_expression(then_branch, expected, instructions)?;
                self.emit_expression(else_branch, Some(value_type), instructions)?;
                instructions.push(ValueInstruction::Select);
                Ok(value_type)
            }
            Expr::Call(call) => {
                self.emit_intrinsic(call.callee(), call.args(), expected, instructions)
            }
            _ => Err(unsupported(expression)),
        }
    }

    fn emit_path(
        &mut self,
        path: &str,
        expected: Option<FxRuntimeType>,
        instructions: &mut Vec<ValueInstruction>,
    ) -> Result<FxRuntimeType, ViewValueCompileError> {
        let value_type = expected.ok_or_else(|| ViewValueCompileError::MissingExpectedType {
            expression: path.to_owned(),
        })?;
        if let Some(parameter) = self.current_parameters.get(path).copied() {
            if parameter.value_type != value_type {
                let view = self
                    .current_view
                    .clone()
                    .ok_or(ViewValueCompileError::MissingDefinitionContext)?;
                return Err(ViewValueCompileError::ProjectionTypeConflict {
                    projection: ViewValueInputSource::DefinitionParameter {
                        view,
                        name: path.to_owned(),
                    },
                    existing: parameter.value_type,
                    requested: value_type,
                });
            }
            instructions.push(ValueInstruction::LoadParameter {
                slot: parameter.slot,
                ty: value_type,
            });
            return Ok(value_type);
        }
        let source = if self.local_types.contains_key(path) {
            let view = self
                .current_view
                .clone()
                .ok_or(ViewValueCompileError::MissingDefinitionContext)?;
            ViewValueInputSource::Local {
                view,
                name: path.to_owned(),
            }
        } else {
            ViewValueInputSource::Projection {
                path: path.split('.').map(str::to_owned).collect(),
            }
        };
        self.emit_input(
            ViewValueInputNamespace::State,
            source,
            value_type,
            instructions,
        )?;
        Ok(value_type)
    }

    fn emit_input(
        &mut self,
        namespace: ViewValueInputNamespace,
        source: ViewValueInputSource,
        value_type: FxRuntimeType,
        instructions: &mut Vec<ValueInstruction>,
    ) -> Result<(), ViewValueCompileError> {
        let slot = self.register_input(namespace, source, value_type)?;
        instructions.push(match namespace {
            ViewValueInputNamespace::Parameter => ValueInstruction::LoadParameter {
                slot,
                ty: value_type,
            },
            ViewValueInputNamespace::State => ValueInstruction::LoadState {
                slot,
                ty: value_type,
            },
        });
        Ok(())
    }

    fn register_input(
        &mut self,
        namespace: ViewValueInputNamespace,
        source: ViewValueInputSource,
        value_type: FxRuntimeType,
    ) -> Result<u16, ViewValueCompileError> {
        let key = (namespace, source.clone());
        let slot = if let Some(existing) = self.input_slots.get(&key) {
            if existing.value_type != value_type {
                return Err(ViewValueCompileError::ProjectionTypeConflict {
                    projection: source,
                    existing: existing.value_type,
                    requested: value_type,
                });
            }
            existing.slot
        } else {
            let types = match namespace {
                ViewValueInputNamespace::Parameter => {
                    if self.parameter_types.len() >= MAX_VIEW_PARAMETER_PROJECTIONS {
                        return Err(ViewValueCompileError::TooManyParameterProjections {
                            limit: MAX_VIEW_PARAMETER_PROJECTIONS,
                        });
                    }
                    &mut self.parameter_types
                }
                ViewValueInputNamespace::State => {
                    if self.state_types.len() >= MAX_VIEW_STATE_PROJECTIONS {
                        return Err(ViewValueCompileError::TooManyStateProjections {
                            limit: MAX_VIEW_STATE_PROJECTIONS,
                        });
                    }
                    &mut self.state_types
                }
            };
            let slot = u16::try_from(types.len()).map_err(|_| match namespace {
                ViewValueInputNamespace::Parameter => {
                    ViewValueCompileError::TooManyParameterProjections {
                        limit: MAX_VIEW_PARAMETER_PROJECTIONS,
                    }
                }
                ViewValueInputNamespace::State => ViewValueCompileError::TooManyStateProjections {
                    limit: MAX_VIEW_STATE_PROJECTIONS,
                },
            })?;
            types.push(value_type);
            self.inputs.push(ViewValueInputResource {
                namespace,
                slot,
                value_type,
                source: source.clone(),
            });
            self.input_slots.insert(key, InputSlot { slot, value_type });
            slot
        };
        Ok(slot)
    }

    fn emit_binary(
        &mut self,
        lhs: &Expr,
        operator: BinaryOp,
        rhs: &Expr,
        expected: Option<FxRuntimeType>,
        instructions: &mut Vec<ValueInstruction>,
    ) -> Result<FxRuntimeType, ViewValueCompileError> {
        match operator {
            BinaryOp::And | BinaryOp::Or | BinaryOp::Implies => {
                self.emit_expression(lhs, Some(FxRuntimeType::Bool), instructions)?;
                if operator == BinaryOp::Implies {
                    instructions.push(ValueInstruction::Not);
                }
                self.emit_expression(rhs, Some(FxRuntimeType::Bool), instructions)?;
                instructions.push(if operator == BinaryOp::And {
                    ValueInstruction::And
                } else {
                    ValueInstruction::Or
                });
                Ok(FxRuntimeType::Bool)
            }
            BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Gte
            | BinaryOp::Lte
            | BinaryOp::Gt
            | BinaryOp::Lt => {
                let operand_type = infer_expression_type(lhs)
                    .or_else(|| infer_expression_type(rhs))
                    .ok_or_else(|| missing_expected(lhs))?;
                self.emit_expression(lhs, Some(operand_type), instructions)?;
                self.emit_expression(rhs, Some(operand_type), instructions)?;
                instructions.push(match operator {
                    BinaryOp::Eq | BinaryOp::NotEq => ValueInstruction::Equal,
                    BinaryOp::Gte => ValueInstruction::GreaterEqual,
                    BinaryOp::Lte => ValueInstruction::LessEqual,
                    BinaryOp::Gt => ValueInstruction::Greater,
                    BinaryOp::Lt => ValueInstruction::Less,
                    _ => unreachable!(),
                });
                if operator == BinaryOp::NotEq {
                    instructions.push(ValueInstruction::Not);
                }
                Ok(FxRuntimeType::Bool)
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                let operand_type = expected
                    .or_else(|| infer_expression_type(lhs))
                    .or_else(|| infer_expression_type(rhs))
                    .ok_or_else(|| missing_expected(lhs))?;
                self.emit_expression(lhs, Some(operand_type), instructions)?;
                self.emit_expression(rhs, Some(operand_type), instructions)?;
                instructions.push(match operator {
                    BinaryOp::Add => ValueInstruction::Add,
                    BinaryOp::Sub => ValueInstruction::Sub,
                    BinaryOp::Mul => ValueInstruction::Mul,
                    BinaryOp::Div => ValueInstruction::Div,
                    _ => unreachable!(),
                });
                Ok(operand_type)
            }
            BinaryOp::In | BinaryOp::Merge | BinaryOp::Rem => {
                Err(ViewValueCompileError::UnsupportedExpression {
                    expression: format!("{lhs:?} {operator:?} {rhs:?}"),
                })
            }
        }
    }

    fn emit_intrinsic(
        &mut self,
        callee: &Expr,
        args: &[CallArg],
        expected: Option<FxRuntimeType>,
        instructions: &mut Vec<ValueInstruction>,
    ) -> Result<FxRuntimeType, ViewValueCompileError> {
        let name = callee
            .dotted_selector_label()
            .ok_or_else(|| unsupported(callee))?;
        let args = args
            .iter()
            .map(|argument| match argument {
                CallArg::Positional(value) => Ok(value),
                CallArg::Named { value, .. } => Ok(value.as_ref()),
                CallArg::Spread { value } => Err(unsupported(value)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let value_type = expected
            .or_else(|| {
                args.first()
                    .and_then(|argument| infer_expression_type(argument))
            })
            .ok_or_else(|| missing_expected(callee))?;
        let (arity, instruction) = match name.rsplit('.').next().unwrap_or(&name) {
            "abs" => (1, ValueInstruction::Abs),
            "min" => (2, ValueInstruction::Min),
            "max" => (2, ValueInstruction::Max),
            "clamp" => (3, ValueInstruction::Clamp),
            "sin" => (1, ValueInstruction::Sin),
            "cos" => (1, ValueInstruction::Cos),
            "floor" => (1, ValueInstruction::Floor),
            "fract" => (1, ValueInstruction::Fract),
            _ => return Err(unsupported(callee)),
        };
        if args.len() != arity {
            return Err(ViewValueCompileError::UnsupportedExpression {
                expression: format!("{name} expects {arity} arguments, got {}", args.len()),
            });
        }
        for argument in args {
            self.emit_expression(argument, Some(value_type), instructions)?;
        }
        instructions.push(instruction);
        Ok(match name.rsplit('.').next().unwrap_or(&name) {
            "sin" | "cos" | "floor" | "fract" => FxRuntimeType::F32,
            _ => value_type,
        })
    }
}

fn infer_expression_type(expression: &Expr) -> Option<FxRuntimeType> {
    infer_literal_type(expression).or(match expression {
        Expr::ShortVariant(_) | Expr::EntityRef(_) => Some(FxRuntimeType::I32),
        Expr::Unary {
            op: UnaryOp::Not, ..
        }
        | Expr::Binary {
            op:
                BinaryOp::Implies
                | BinaryOp::Or
                | BinaryOp::And
                | BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Gte
                | BinaryOp::Lte
                | BinaryOp::Gt
                | BinaryOp::Lt,
            ..
        } => Some(FxRuntimeType::Bool),
        _ => None,
    })
}

fn require_expected(
    expression: &Expr,
    expected: Option<FxRuntimeType>,
    actual: FxRuntimeType,
) -> Result<(), ViewValueCompileError> {
    if expected.is_none_or(|expected| expected == actual) {
        Ok(())
    } else {
        Err(ViewValueCompileError::UnsupportedExpression {
            expression: format!("{expression:?} cannot produce {:?}", expected.unwrap()),
        })
    }
}

fn missing_expected(expression: &Expr) -> ViewValueCompileError {
    ViewValueCompileError::MissingExpectedType {
        expression: format!("{expression:?}"),
    }
}

fn unsupported(expression: &Expr) -> ViewValueCompileError {
    ViewValueCompileError::UnsupportedExpression {
        expression: format!("{expression:?}"),
    }
}

fn symbol_discriminant(source: &str) -> i32 {
    let digest = BundleDigest::of(source.as_bytes());
    let mut bytes = [0_u8; 4];
    bytes.copy_from_slice(&digest.as_bytes()[..4]);
    i32::from_le_bytes(bytes)
}
