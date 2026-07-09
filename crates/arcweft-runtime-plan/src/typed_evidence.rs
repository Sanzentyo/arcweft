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
    /// A call expression's callee type checked as a function value.
    FunctionValueCall {
        callee: Option<String>,
        arg_count: usize,
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

/// Runtime argument order proven for a data-last method fallback call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDataLastMethodFallbackArg {
    /// Source method-call argument at the given index.
    CallArg { index: usize },
    /// The method-call receiver appended as the data-last callable argument.
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
                    } if expected_arg_count == &arg_count
                        && expected_callee.as_deref() == callee
                )
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
