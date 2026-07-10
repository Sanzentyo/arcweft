//! Type-checker evidence consumed by runtime-plan lowering.

/// Runtime-plan-local expression identifier aligned with type-check evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypedExpressionId(usize);

impl RuntimeTypedExpressionId {
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// One lowering-sensitive expression fact exported from type checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTypedLoweringEvidence {
    pub expression_id: RuntimeTypedExpressionId,
    pub kind: RuntimeTypedLoweringEvidenceKind,
}

/// Runtime-plan decisions proven by type checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTypedLoweringEvidenceKind {
    /// Concrete primitive representation selected for a numeric literal or
    /// compact integer sequence by semantic analysis.
    ResolvedNumericType { target: RuntimeNumericType },
    /// A call expression's callee type checked as a function value.
    FunctionValueCall {
        callee: Option<String>,
        arg_count: usize,
        partial: bool,
    },
    /// An expression was checked in a function-typed expected context.
    ExpectedFunctionValue { arity: usize },
    /// A top-level function path was referenced as a runtime function value.
    FunctionValueReference { callee: String },
    /// A direct named function signature call returned a partial function.
    SignaturePartialCall { callee: String, arg_count: usize },
    /// A function-valued expression owns a callable used by effect-row reports.
    FunctionEffectCallable { callable: String },
    /// A method-call expression resolved as data-last callable fallback.
    DataLastMethodFallback {
        method: String,
        arg_count: usize,
        arg_order: Vec<RuntimeDataLastMethodFallbackArg>,
    },
}

/// Runtime-plan-local numeric primitive selected by the type checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeNumericType {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
}

impl RuntimeNumericType {
    /// Canonical Arcweft source spelling of this numeric primitive.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::ISize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::USize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

impl From<arcweft_lang_hir::syntax::expr::IntSuffix> for RuntimeNumericType {
    fn from(suffix: arcweft_lang_hir::syntax::expr::IntSuffix) -> Self {
        use arcweft_lang_hir::syntax::expr::IntSuffix;
        match suffix {
            IntSuffix::I8 => Self::I8,
            IntSuffix::I16 => Self::I16,
            IntSuffix::I32 => Self::I32,
            IntSuffix::I64 => Self::I64,
            IntSuffix::I128 => Self::I128,
            IntSuffix::ISize => Self::ISize,
            IntSuffix::U8 => Self::U8,
            IntSuffix::U16 => Self::U16,
            IntSuffix::U32 => Self::U32,
            IntSuffix::U64 => Self::U64,
            IntSuffix::U128 => Self::U128,
            IntSuffix::USize => Self::USize,
        }
    }
}

/// Runtime argument order proven for a data-last method fallback call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDataLastMethodFallbackArg {
    /// Source method-call argument in the fallback callable's first stage.
    CallArg { index: usize },
    /// The method receiver applied as one separate final call group.
    Receiver,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeTypedLoweringEvidenceLookup<'a> {
    evidence: &'a [RuntimeTypedLoweringEvidence],
}

impl<'a> RuntimeTypedLoweringEvidenceLookup<'a> {
    pub(crate) const fn new(evidence: &'a [RuntimeTypedLoweringEvidence]) -> Self {
        Self { evidence }
    }

    pub(crate) fn has_function_value_call(
        self,
        expression_id: RuntimeTypedExpressionId,
        callee: Option<&str>,
        arg_count: usize,
    ) -> bool {
        self.evidence.iter().any(|evidence| {
            evidence.expression_id == expression_id
                && matches!(
                    &evidence.kind,
                    RuntimeTypedLoweringEvidenceKind::FunctionValueCall {
                        callee: expected_callee,
                        arg_count: expected_arg_count,
                        ..
                    } if expected_arg_count == &arg_count
                        && expected_callee.as_deref() == callee
                )
        })
    }

    pub(crate) fn has_partial_function_value_call(
        self,
        expression_id: RuntimeTypedExpressionId,
        callee: Option<&str>,
        arg_count: usize,
    ) -> bool {
        self.evidence.iter().any(|evidence| {
            evidence.expression_id == expression_id
                && matches!(
                    &evidence.kind,
                    RuntimeTypedLoweringEvidenceKind::FunctionValueCall {
                        callee: expected_callee,
                        arg_count: expected_arg_count,
                        partial: true,
                    } if expected_arg_count == &arg_count
                        && expected_callee.as_deref() == callee
                )
        })
    }

    pub(crate) fn resolved_numeric_type(
        self,
        expression_id: RuntimeTypedExpressionId,
    ) -> Option<RuntimeNumericType> {
        self.evidence.iter().find_map(|evidence| {
            if evidence.expression_id != expression_id {
                return None;
            }
            let RuntimeTypedLoweringEvidenceKind::ResolvedNumericType { target } = evidence.kind
            else {
                return None;
            };
            Some(target)
        })
    }

    pub(crate) fn has_expected_function_value(
        self,
        expression_id: RuntimeTypedExpressionId,
    ) -> bool {
        self.evidence.iter().any(|evidence| {
            evidence.expression_id == expression_id
                && matches!(
                    evidence.kind,
                    RuntimeTypedLoweringEvidenceKind::ExpectedFunctionValue { .. }
                )
        })
    }

    pub(crate) fn has_function_value_reference(
        self,
        expression_id: RuntimeTypedExpressionId,
        callee: &str,
    ) -> bool {
        self.evidence.iter().any(|evidence| {
            evidence.expression_id == expression_id
                && matches!(
                    &evidence.kind,
                    RuntimeTypedLoweringEvidenceKind::FunctionValueReference {
                        callee: expected_callee,
                    } if expected_callee == callee
                )
        })
    }

    pub(crate) fn has_signature_partial_call(
        self,
        expression_id: RuntimeTypedExpressionId,
        callee: &str,
        arg_count: usize,
    ) -> bool {
        self.evidence.iter().any(|evidence| {
            evidence.expression_id == expression_id
                && matches!(
                    &evidence.kind,
                    RuntimeTypedLoweringEvidenceKind::SignaturePartialCall {
                        callee: expected_callee,
                        arg_count: expected_arg_count,
                    } if expected_callee == callee && expected_arg_count == &arg_count
                )
        })
    }

    pub(crate) fn data_last_method_fallback_arg_order(
        self,
        expression_id: RuntimeTypedExpressionId,
        method: &str,
        arg_count: usize,
    ) -> Option<&'a [RuntimeDataLastMethodFallbackArg]> {
        self.evidence.iter().find_map(|evidence| {
            if evidence.expression_id != expression_id {
                return None;
            }
            let RuntimeTypedLoweringEvidenceKind::DataLastMethodFallback {
                method: expected_method,
                arg_count: expected_arg_count,
                arg_order,
            } = &evidence.kind
            else {
                return None;
            };
            (expected_method == method && expected_arg_count == &arg_count)
                .then_some(arg_order.as_slice())
        })
    }
}
