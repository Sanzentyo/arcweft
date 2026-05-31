use crate::pattern::RuntimePattern;
use crate::plan::RuntimePureHelperId;
use crate::time::LogicalDuration;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBinding {
    pub name: String,
    pub value: RuntimeValue,
}

/// Structured payload exchanged at the host/runtime boundary.
///
/// Payloads intentionally retain `RuntimeValue` shape instead of collapsing
/// source and stream items to debug strings. Hosts may still display `label()`
/// for logs, but replay and downstream runtime consumers keep typed data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePayload(pub RuntimeValue);

/// Deterministic value domain used by the Sans I/O flow runtime.
///
/// Floats are preserved as source strings until a later numeric semantic pass
/// chooses exact representation and unit rules. That keeps this runtime model
/// deterministic across native and wasm targets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(String),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    EntityRef(String),
    Tuple(Vec<RuntimeValue>),
    Seq(RuntimeSeq),
    Record(Vec<RuntimeFieldValue>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    },
}

/// Storage strategy for runtime sequence values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeSeq {
    Values(Vec<RuntimeValue>),
    Dense(DenseSeq),
}

impl RuntimeSeq {
    pub fn values(values: Vec<RuntimeValue>) -> Self {
        Self::Values(values)
    }

    pub fn dense_i64(values: Vec<i64>) -> Self {
        Self::Dense(DenseSeq::i64(values))
    }

    pub fn dense_i8(values: Vec<i8>) -> Self {
        Self::Dense(DenseSeq::i8(values))
    }

    pub fn dense_i16(values: Vec<i16>) -> Self {
        Self::Dense(DenseSeq::i16(values))
    }

    pub fn dense_i32(values: Vec<i32>) -> Self {
        Self::Dense(DenseSeq::i32(values))
    }

    pub fn dense_u8(values: Vec<u8>) -> Self {
        Self::Dense(DenseSeq::u8(values))
    }

    pub fn dense_u16(values: Vec<u16>) -> Self {
        Self::Dense(DenseSeq::u16(values))
    }

    pub fn dense_u32(values: Vec<u32>) -> Self {
        Self::Dense(DenseSeq::u32(values))
    }

    pub fn dense_u64(values: Vec<u64>) -> Self {
        Self::Dense(DenseSeq::u64(values))
    }

    pub fn dense_bool(values: Vec<bool>) -> Self {
        Self::Dense(DenseSeq::bool(values))
    }

    pub fn dense_bytes(values: Vec<u8>) -> Self {
        Self::Dense(DenseSeq::bytes(values))
    }

    pub fn dense_chars(values: Vec<char>) -> Self {
        Self::Dense(DenseSeq::chars(values))
    }

    pub fn dense_durations(values: Vec<LogicalDuration>) -> Self {
        Self::Dense(DenseSeq::durations(values))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Dense(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_values(&self) -> Option<&[RuntimeValue]> {
        match self {
            Self::Values(values) => Some(values),
            Self::Dense(_) => None,
        }
    }

    pub fn as_i64_slice(&self) -> Option<&[i64]> {
        match self {
            Self::Dense(values) => values.as_i64_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_i32_slice(&self) -> Option<&[i32]> {
        match self {
            Self::Dense(values) => values.as_i32_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_u64_slice(&self) -> Option<&[u64]> {
        match self {
            Self::Dense(values) => values.as_u64_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_bool_slice(&self) -> Option<&[bool]> {
        match self {
            Self::Dense(values) => values.as_bool_slice(),
            Self::Values(_) => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Dense(values) => values.as_bytes(),
            Self::Values(_) => None,
        }
    }

    pub fn as_chars(&self) -> Option<&[char]> {
        match self {
            Self::Dense(values) => values.as_chars(),
            Self::Values(_) => None,
        }
    }

    pub fn as_durations(&self) -> Option<&[LogicalDuration]> {
        match self {
            Self::Dense(values) => values.as_durations(),
            Self::Values(_) => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::Values(values) => values,
            Self::Dense(values) => values.into_values(),
        }
    }

    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        match self {
            Self::Values(values) => Self::Values(values[index..].to_vec()),
            Self::Dense(values) => Self::Dense(values.tail_from(index)),
        }
    }

    pub fn into_i64_vec(self) -> Option<Vec<i64>> {
        match self {
            Self::Dense(values) => values.into_i64_vec(),
            Self::Values(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::Dense(values) => values.sum_as_i64(),
            Self::Values(_) => None,
        }
    }
}

/// Dense sequence storage for homogeneous scalar data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DenseSeq {
    I8(DenseSeqStorage<i8>),
    I16(DenseSeqStorage<i16>),
    I32(DenseSeqStorage<i32>),
    I64(DenseSeqStorage<i64>),
    U8(DenseSeqStorage<u8>),
    U16(DenseSeqStorage<u16>),
    U32(DenseSeqStorage<u32>),
    U64(DenseSeqStorage<u64>),
    Bool(DenseSeqStorage<bool>),
    Bytes(DenseSeqStorage<u8>),
    Chars(DenseSeqStorage<char>),
    Durations(DenseSeqStorage<LogicalDuration>),
}

impl DenseSeq {
    pub fn i8(values: Vec<i8>) -> Self {
        Self::I8(DenseSeqStorage::new(values))
    }

    pub fn i16(values: Vec<i16>) -> Self {
        Self::I16(DenseSeqStorage::new(values))
    }

    pub fn i32(values: Vec<i32>) -> Self {
        Self::I32(DenseSeqStorage::new(values))
    }

    pub fn i64(values: Vec<i64>) -> Self {
        Self::I64(DenseSeqStorage::new(values))
    }

    pub fn u8(values: Vec<u8>) -> Self {
        Self::U8(DenseSeqStorage::new(values))
    }

    pub fn u16(values: Vec<u16>) -> Self {
        Self::U16(DenseSeqStorage::new(values))
    }

    pub fn u32(values: Vec<u32>) -> Self {
        Self::U32(DenseSeqStorage::new(values))
    }

    pub fn u64(values: Vec<u64>) -> Self {
        Self::U64(DenseSeqStorage::new(values))
    }

    pub fn bool(values: Vec<bool>) -> Self {
        Self::Bool(DenseSeqStorage::new(values))
    }

    pub fn bytes(values: Vec<u8>) -> Self {
        Self::Bytes(DenseSeqStorage::new(values))
    }

    pub fn chars(values: Vec<char>) -> Self {
        Self::Chars(DenseSeqStorage::new(values))
    }

    pub fn durations(values: Vec<LogicalDuration>) -> Self {
        Self::Durations(DenseSeqStorage::new(values))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::I8(values) => values.len(),
            Self::I16(values) => values.len(),
            Self::I32(values) => values.len(),
            Self::I64(values) => values.len(),
            Self::U8(values) | Self::Bytes(values) => values.len(),
            Self::U16(values) => values.len(),
            Self::U32(values) => values.len(),
            Self::U64(values) => values.len(),
            Self::Bool(values) => values.len(),
            Self::Chars(values) => values.len(),
            Self::Durations(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_i64_slice(&self) -> Option<&[i64]> {
        match self {
            Self::I64(values) => Some(values.as_slice()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_) => None,
        }
    }

    pub fn as_i32_slice(&self) -> Option<&[i32]> {
        match self {
            Self::I32(values) => Some(values.as_slice()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I64(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_) => None,
        }
    }

    pub fn as_u64_slice(&self) -> Option<&[u64]> {
        match self {
            Self::U64(values) => Some(values.as_slice()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_) => None,
        }
    }

    pub fn as_bool_slice(&self) -> Option<&[bool]> {
        match self {
            Self::Bool(values) => Some(values.as_slice()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_) => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(values) => Some(values.as_slice()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Bool(_)
            | Self::Chars(_)
            | Self::Durations(_) => None,
        }
    }

    pub fn as_chars(&self) -> Option<&[char]> {
        match self {
            Self::Chars(values) => Some(values.as_slice()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Durations(_) => None,
        }
    }

    pub fn as_durations(&self) -> Option<&[LogicalDuration]> {
        match self {
            Self::Durations(values) => Some(values.as_slice()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_) => None,
        }
    }

    pub fn into_values(self) -> Vec<RuntimeValue> {
        match self {
            Self::I8(values) => {
                materialize_i64_sequence(values.into_vec().into_iter().map(i64::from).collect())
            }
            Self::I16(values) => {
                materialize_i64_sequence(values.into_vec().into_iter().map(i64::from).collect())
            }
            Self::I32(values) => {
                materialize_i64_sequence(values.into_vec().into_iter().map(i64::from).collect())
            }
            Self::I64(values) => materialize_i64_sequence(values.into_vec()),
            Self::U8(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::UInt(u64::from(value)))
                .collect(),
            Self::U16(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::UInt(u64::from(value)))
                .collect(),
            Self::U32(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::UInt(u64::from(value)))
                .collect(),
            Self::U64(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::UInt)
                .collect(),
            Self::Bool(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Bool)
                .collect(),
            Self::Bytes(values) => values
                .into_vec()
                .into_iter()
                .map(|value| RuntimeValue::Int(i64::from(value)))
                .collect(),
            Self::Chars(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Char)
                .collect(),
            Self::Durations(values) => values
                .into_vec()
                .into_iter()
                .map(RuntimeValue::Duration)
                .collect(),
        }
    }

    pub fn value_at(&self, index: usize) -> RuntimeValue {
        match self {
            Self::I8(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::I16(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::I32(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::I64(values) => RuntimeValue::Int(values.as_slice()[index]),
            Self::U8(values) => RuntimeValue::UInt(u64::from(values.as_slice()[index])),
            Self::U16(values) => RuntimeValue::UInt(u64::from(values.as_slice()[index])),
            Self::U32(values) => RuntimeValue::UInt(u64::from(values.as_slice()[index])),
            Self::U64(values) => RuntimeValue::UInt(values.as_slice()[index]),
            Self::Bool(values) => RuntimeValue::Bool(values.as_slice()[index]),
            Self::Bytes(values) => RuntimeValue::Int(i64::from(values.as_slice()[index])),
            Self::Chars(values) => RuntimeValue::Char(values.as_slice()[index]),
            Self::Durations(values) => RuntimeValue::Duration(values.as_slice()[index]),
        }
    }

    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        match self {
            Self::I8(values) => Self::I8(values.tail_from(index)),
            Self::I16(values) => Self::I16(values.tail_from(index)),
            Self::I32(values) => Self::I32(values.tail_from(index)),
            Self::I64(values) => Self::I64(values.tail_from(index)),
            Self::U8(values) => Self::U8(values.tail_from(index)),
            Self::U16(values) => Self::U16(values.tail_from(index)),
            Self::U32(values) => Self::U32(values.tail_from(index)),
            Self::U64(values) => Self::U64(values.tail_from(index)),
            Self::Bool(values) => Self::Bool(values.tail_from(index)),
            Self::Bytes(values) => Self::Bytes(values.tail_from(index)),
            Self::Chars(values) => Self::Chars(values.tail_from(index)),
            Self::Durations(values) => Self::Durations(values.tail_from(index)),
        }
    }

    pub fn into_i64_vec(self) -> Option<Vec<i64>> {
        match self {
            Self::I64(values) => Some(values.into_vec()),
            Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::U8(_)
            | Self::U16(_)
            | Self::U32(_)
            | Self::U64(_)
            | Self::Bool(_)
            | Self::Bytes(_)
            | Self::Chars(_)
            | Self::Durations(_) => None,
        }
    }

    pub fn sum_as_i64(&self) -> Option<i64> {
        match self {
            Self::I8(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::I64(values) => Some(values.as_slice().iter().sum()),
            Self::U8(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U16(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U32(values) => Some(values.as_slice().iter().copied().map(i64::from).sum()),
            Self::U64(values) => values.as_slice().iter().try_fold(0_i64, |acc, value| {
                i64::try_from(*value).ok().map(|value| acc + value)
            }),
            Self::Bool(_) | Self::Bytes(_) | Self::Chars(_) | Self::Durations(_) => None,
        }
    }
}

/// Generic backing store for one dense homogeneous sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenseSeqStorage<T> {
    values: Vec<T>,
}

impl<T> DenseSeqStorage<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self { values }
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub fn into_vec(self) -> Vec<T> {
        self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<T: Clone> DenseSeqStorage<T> {
    #[must_use]
    pub fn tail_from(&self, index: usize) -> Self {
        Self::new(self.values[index..].to_vec())
    }
}

/// One field inside a runtime record value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFieldValue {
    pub name: String,
    pub value: RuntimeValue,
}

/// Expression subset executable by the Sans I/O flow runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeExpr {
    Value(RuntimeValue),
    Local(String),
    EntityRef(String),
    Let {
        name: String,
        expr: Box<RuntimeExpr>,
        body: Box<RuntimeExpr>,
    },
    Tuple(Vec<RuntimeExpr>),
    BracketSeq(Vec<RuntimeExpr>),
    RepeatSeq {
        value: Box<RuntimeExpr>,
        len: usize,
    },
    Record(Vec<RuntimeFieldExpr>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeExpr>>,
    },
    Field {
        target: Box<RuntimeExpr>,
        field: String,
    },
    Call {
        callee: String,
        args: Vec<RuntimeExpr>,
    },
    PureCall {
        helper: RuntimePureHelperId,
        args: Vec<RuntimeExpr>,
    },
    SpreadArg(Box<RuntimeExpr>),
    MethodCall {
        receiver: Box<RuntimeExpr>,
        method: String,
        args: Vec<RuntimeExpr>,
    },
    Map {
        source: Box<RuntimeExpr>,
        param: String,
        body: Box<RuntimeExpr>,
    },
    Sum {
        source: Box<RuntimeExpr>,
    },
    Unary {
        op: RuntimeUnaryOp,
        expr: Box<RuntimeExpr>,
    },
    Binary {
        lhs: Box<RuntimeExpr>,
        op: RuntimeBinaryOp,
        rhs: Box<RuntimeExpr>,
    },
    If {
        condition: Box<RuntimeExpr>,
        then_expr: Box<RuntimeExpr>,
        else_expr: Box<RuntimeExpr>,
    },
    IfLet {
        pattern: RuntimePattern,
        expr: Box<RuntimeExpr>,
        guard: Option<Box<RuntimeExpr>>,
        then_expr: Box<RuntimeExpr>,
        else_expr: Box<RuntimeExpr>,
    },
    Match {
        scrutinee: Box<RuntimeExpr>,
        arms: Vec<RuntimeExprMatchArm>,
    },
}

impl RuntimeExpr {
    /// Returns whether this expression can use the allocation-light scalar pure evaluator.
    pub fn supports_scalar_pure_eval(&self) -> bool {
        match self {
            Self::Value(RuntimeValue::Bool(_) | RuntimeValue::Int(_)) | Self::Local(_) => true,
            Self::Let { expr, body, .. } => {
                expr.supports_scalar_pure_eval() && body.supports_scalar_pure_eval()
            }
            Self::Unary { expr, .. } => expr.supports_scalar_pure_eval(),
            Self::Binary { lhs, rhs, .. } => {
                lhs.supports_scalar_pure_eval() && rhs.supports_scalar_pure_eval()
            }
            Self::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.supports_scalar_pure_eval()
                    && then_expr.supports_scalar_pure_eval()
                    && else_expr.supports_scalar_pure_eval()
            }
            Self::Value(_)
            | Self::EntityRef(_)
            | Self::Tuple(_)
            | Self::BracketSeq(_)
            | Self::RepeatSeq { .. }
            | Self::Record(_)
            | Self::Variant { .. }
            | Self::Field { .. }
            | Self::Call { .. }
            | Self::PureCall { .. }
            | Self::SpreadArg(_)
            | Self::MethodCall { .. }
            | Self::Map { .. }
            | Self::Sum { .. }
            | Self::IfLet { .. }
            | Self::Match { .. } => false,
        }
    }
}

/// One value-producing `match` arm in a runtime expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeExprMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeExpr>,
    pub value: RuntimeExpr,
}

/// One field inside a runtime record expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFieldExpr {
    pub name: String,
    pub value: RuntimeExpr,
}

/// Unary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeUnaryOp {
    Not,
    Neg,
}

/// Binary operator supported by the Sans I/O expression evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
}

#[derive(Debug, Eq)]
pub struct RuntimeEnv {
    scopes: Vec<RuntimeScope>,
    spare_scopes: Vec<RuntimeScope>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RuntimeScope {
    bindings: Vec<RuntimeBinding>,
}

/// Pure runtime program consumed by the minimal Sans I/O engine.

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeEvalError {
    #[error("unknown runtime binding `{0}`")]
    UnknownBinding(String),
    #[error("expected bool expression, found {0}")]
    ExpectedBool(String),
    #[error("expected integer expression, found {0}")]
    ExpectedInt(String),
    #[error("expected entity reference expression, found {0}")]
    ExpectedEntityRef(String),
    #[error("expected bracket sequence expression, found {0}")]
    ExpectedBracketSeq(String),
    #[error("field `{field}` does not exist on {value}")]
    MissingField { field: String, value: String },
    #[error("spread argument requires a tuple or bracket sequence, found {0}")]
    InvalidSpread(String),
    #[error("spread argument cannot be evaluated outside a call argument list")]
    SpreadOutsideCall,
    #[error(
        "runtime pure helper `{helper}` expected at most {max} fast-path argument(s), found {found}"
    )]
    TooManyPureArgs {
        helper: String,
        max: usize,
        found: usize,
    },
    #[error("runtime pure helper id {0} does not exist")]
    UnknownPureHelper(usize),
    #[error("operator `{op}` is not supported for {lhs} and {rhs}")]
    UnsupportedBinary {
        op: &'static str,
        lhs: String,
        rhs: String,
    },
    #[error("operator `{op}` is not supported for {value}")]
    UnsupportedUnary { op: &'static str, value: String },
    #[error("pure helper `{name}` cannot evaluate {reason}")]
    UnsupportedPure { name: String, reason: String },
    #[error("pattern did not match {0}")]
    PatternMismatch(String),
    #[error("pattern binds `{0}` more than once")]
    DuplicateBinding(String),
    #[error("break value can only be consumed by a value-producing loop")]
    BreakValueOutsideValueLoop,
    #[error("loop control `{0}` reached a non-loop runtime context")]
    MisplacedLoopControl(&'static str),
}

impl Default for RuntimeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![RuntimeScope::default()],
            spare_scopes: Vec::new(),
        }
    }
}

impl Clone for RuntimeEnv {
    fn clone(&self) -> Self {
        Self {
            scopes: self.scopes.clone(),
            spare_scopes: Vec::new(),
        }
    }
}

impl PartialEq for RuntimeEnv {
    fn eq(&self, other: &Self) -> bool {
        self.scopes == other.scopes
    }
}

impl RuntimeEnv {
    pub fn push_scope(&mut self) {
        self.push_scope_with_capacity(0);
    }

    pub(crate) fn push_scope_with_capacity(&mut self, binding_capacity: usize) {
        let mut scope = self
            .spare_scopes
            .pop()
            .unwrap_or_else(|| RuntimeScope::with_capacity(binding_capacity));
        scope.clear();
        scope.reserve_bindings(binding_capacity);
        self.scopes.push(scope);
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            if let Some(mut scope) = self.scopes.pop() {
                scope.clear();
                self.spare_scopes.push(scope);
            }
        } else if let Some(scope) = self.scopes.last_mut() {
            scope.clear();
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set(name.into(), value);
        }
    }

    pub(crate) fn set_ref(&mut self, name: &str, value: &RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set_ref(name, value);
        }
    }

    pub(crate) fn set_i64(&mut self, name: &str, value: i64) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set_i64(name, value);
        }
    }

    pub fn set_root(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.first_mut() {
            scope.set(name.into(), value);
        }
    }

    pub fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub fn bind_all(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set(binding.name, binding.value);
        }
    }

    pub(crate) fn bind_all_ref(&mut self, bindings: &[RuntimeBinding]) {
        for binding in bindings {
            self.set_ref(&binding.name, &binding.value);
        }
    }

    pub fn bind_all_root(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set_root(binding.name, binding.value);
        }
    }

    pub fn bind_all_root_ref(&mut self, bindings: &[RuntimeBinding]) {
        if self.replace_root_bindings_ref(bindings) {
            return;
        }
        for binding in bindings {
            self.set_root_ref(&binding.name, &binding.value);
        }
    }

    fn replace_root_bindings_ref(&mut self, bindings: &[RuntimeBinding]) -> bool {
        if self.scopes.is_empty() {
            return bindings.is_empty();
        }
        self.scopes
            .first_mut()
            .is_some_and(|scope| scope.replace_binding_values_ref(bindings))
    }

    fn set_root_ref(&mut self, name: &str, value: &RuntimeValue) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        if let Some(scope) = self.scopes.first_mut() {
            scope.set_ref(name, value);
        }
    }

    pub(crate) fn replace_root_i64_bindings(&mut self, input_names: &[String], args: &[i64]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_i64_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_value_bindings_ref(
        &mut self,
        input_names: &[String],
        args: &[RuntimeValue],
    ) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_value_bindings_ref(input_names, args);
        }
    }
}

impl RuntimeScope {
    fn with_capacity(binding_capacity: usize) -> Self {
        Self {
            bindings: Vec::with_capacity(binding_capacity),
        }
    }

    fn reserve_bindings(&mut self, binding_capacity: usize) {
        let additional = binding_capacity.saturating_sub(self.bindings.capacity());
        self.bindings.reserve(additional);
    }

    fn set(&mut self, name: String, value: RuntimeValue) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = value;
        } else {
            self.bindings.push(RuntimeBinding { name, value });
        }
    }

    fn set_ref(&mut self, name: &str, value: &RuntimeValue) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = value.clone();
        } else {
            self.bindings.push(RuntimeBinding {
                name: name.to_owned(),
                value: value.clone(),
            });
        }
    }

    fn set_i64(&mut self, name: &str, value: i64) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = RuntimeValue::Int(value);
        } else {
            self.bindings.push(RuntimeBinding {
                name: name.to_owned(),
                value: RuntimeValue::Int(value),
            });
        }
    }

    fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.name == name)
            .map(|binding| &binding.value)
    }

    fn clear(&mut self) {
        self.bindings.clear();
    }

    fn replace_i64_bindings(&mut self, input_names: &[String], args: &[i64]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = RuntimeValue::Int(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::Int(value),
                }),
        );
    }

    fn replace_value_bindings_ref(&mut self, input_names: &[String], args: &[RuntimeValue]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args)
                .for_each(|(binding, value)| binding.value = value.clone());
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args)
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.clone(),
                }),
        );
    }

    fn replace_binding_values_ref(&mut self, bindings: &[RuntimeBinding]) -> bool {
        if self.bindings.len() != bindings.len() {
            return false;
        }
        if !self
            .bindings
            .iter()
            .zip(bindings)
            .all(|(current, next)| current.name == next.name)
        {
            return false;
        }
        self.bindings
            .iter_mut()
            .zip(bindings)
            .for_each(|(current, next)| current.value = next.value.clone());
        true
    }
}

impl RuntimePayload {
    pub const fn new(value: RuntimeValue) -> Self {
        Self(value)
    }

    pub const fn value(&self) -> &RuntimeValue {
        &self.0
    }

    pub fn into_value(self) -> RuntimeValue {
        self.0
    }

    pub fn label(&self) -> String {
        runtime_value_label(&self.0)
    }
}

impl From<RuntimeValue> for RuntimePayload {
    fn from(value: RuntimeValue) -> Self {
        Self(value)
    }
}

impl From<String> for RuntimePayload {
    fn from(value: String) -> Self {
        Self(RuntimeValue::String(value))
    }
}

impl From<&str> for RuntimePayload {
    fn from(value: &str) -> Self {
        Self(RuntimeValue::String(value.to_owned()))
    }
}

pub(crate) fn evaluate_unary(
    op: RuntimeUnaryOp,
    value: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match (op, value) {
        (RuntimeUnaryOp::Not, RuntimeValue::Bool(value)) => Ok(RuntimeValue::Bool(!value)),
        (RuntimeUnaryOp::Neg, RuntimeValue::Int(value)) => Ok(RuntimeValue::Int(-value)),
        (op, value) => Err(RuntimeEvalError::UnsupportedUnary {
            op: runtime_unary_op_label(op),
            value: runtime_value_label(&value),
        }),
    }
}

pub(crate) fn evaluate_binary(
    lhs: RuntimeValue,
    op: RuntimeBinaryOp,
    rhs: RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    match op {
        RuntimeBinaryOp::Eq => Ok(RuntimeValue::Bool(lhs == rhs)),
        RuntimeBinaryOp::Ne => Ok(RuntimeValue::Bool(lhs != rhs)),
        RuntimeBinaryOp::And => match (lhs, rhs) {
            (RuntimeValue::Bool(lhs), RuntimeValue::Bool(rhs)) => {
                Ok(RuntimeValue::Bool(lhs && rhs))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
        RuntimeBinaryOp::Or => match (lhs, rhs) {
            (RuntimeValue::Bool(lhs), RuntimeValue::Bool(rhs)) => {
                Ok(RuntimeValue::Bool(lhs || rhs))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
        RuntimeBinaryOp::Lt | RuntimeBinaryOp::Le | RuntimeBinaryOp::Gt | RuntimeBinaryOp::Ge => {
            match (lhs, rhs) {
                (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => {
                    Ok(RuntimeValue::Bool(match op {
                        RuntimeBinaryOp::Lt => lhs < rhs,
                        RuntimeBinaryOp::Le => lhs <= rhs,
                        RuntimeBinaryOp::Gt => lhs > rhs,
                        RuntimeBinaryOp::Ge => lhs >= rhs,
                        _ => unreachable!(),
                    }))
                }
                (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs)) => {
                    Ok(RuntimeValue::Bool(match op {
                        RuntimeBinaryOp::Lt => lhs < rhs,
                        RuntimeBinaryOp::Le => lhs <= rhs,
                        RuntimeBinaryOp::Gt => lhs > rhs,
                        RuntimeBinaryOp::Ge => lhs >= rhs,
                        _ => unreachable!(),
                    }))
                }
                (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
            }
        }
        RuntimeBinaryOp::Add
        | RuntimeBinaryOp::Sub
        | RuntimeBinaryOp::Mul
        | RuntimeBinaryOp::Div => match (lhs, rhs) {
            (RuntimeValue::Int(lhs), RuntimeValue::Int(rhs)) => Ok(RuntimeValue::Int(match op {
                RuntimeBinaryOp::Add => lhs + rhs,
                RuntimeBinaryOp::Sub => lhs - rhs,
                RuntimeBinaryOp::Mul => lhs * rhs,
                RuntimeBinaryOp::Div => lhs / rhs,
                _ => unreachable!(),
            })),
            (RuntimeValue::UInt(lhs), RuntimeValue::UInt(rhs)) => {
                Ok(RuntimeValue::UInt(match op {
                    RuntimeBinaryOp::Add => lhs + rhs,
                    RuntimeBinaryOp::Sub => lhs - rhs,
                    RuntimeBinaryOp::Mul => lhs * rhs,
                    RuntimeBinaryOp::Div => lhs / rhs,
                    _ => unreachable!(),
                }))
            }
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
    }
}

pub(crate) fn sum_i64_sequence_ref(items: &[RuntimeValue]) -> Result<i64, RuntimeEvalError> {
    items.iter().try_fold(0_i64, |acc, item| match item {
        RuntimeValue::Int(value) => Ok(acc + value),
        RuntimeValue::UInt(value) => i64::try_from(*value).map(|value| acc + value).map_err(|_| {
            RuntimeEvalError::UnsupportedBinary {
                op: "+",
                lhs: "int".to_owned(),
                rhs: runtime_value_label(item),
            }
        }),
        value => Err(RuntimeEvalError::UnsupportedBinary {
            op: "+",
            lhs: "int".to_owned(),
            rhs: runtime_value_label(value),
        }),
    })
}

pub(crate) fn materialize_i64_sequence(items: Vec<i64>) -> Vec<RuntimeValue> {
    items.into_iter().map(RuntimeValue::Int).collect()
}

pub fn runtime_sequence_values(values: Vec<RuntimeValue>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::values(values))
}

pub fn runtime_sequence_dense_i64(values: Vec<i64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i64(values))
}

pub fn runtime_sequence_dense_i8(values: Vec<i8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i8(values))
}

pub fn runtime_sequence_dense_i16(values: Vec<i16>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i16(values))
}

pub fn runtime_sequence_dense_i32(values: Vec<i32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_i32(values))
}

pub fn runtime_sequence_dense_u8(values: Vec<u8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u8(values))
}

pub fn runtime_sequence_dense_u16(values: Vec<u16>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u16(values))
}

pub fn runtime_sequence_dense_u32(values: Vec<u32>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u32(values))
}

pub fn runtime_sequence_dense_u64(values: Vec<u64>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_u64(values))
}

pub fn runtime_sequence_dense_bool(values: Vec<bool>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_bool(values))
}

pub fn runtime_sequence_dense_bytes(values: Vec<u8>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_bytes(values))
}

pub fn runtime_sequence_dense_chars(values: Vec<char>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_chars(values))
}

pub fn runtime_sequence_dense_durations(values: Vec<LogicalDuration>) -> RuntimeValue {
    RuntimeValue::Seq(RuntimeSeq::dense_durations(values))
}

pub(crate) fn runtime_value_into_sequence_values(
    value: RuntimeValue,
) -> Result<Vec<RuntimeValue>, RuntimeValue> {
    match value {
        RuntimeValue::Seq(seq) => Ok(seq.into_values()),
        RuntimeValue::Tuple(values) => Ok(values),
        value => Err(value),
    }
}

fn unsupported_binary(
    op: RuntimeBinaryOp,
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<RuntimeValue, RuntimeEvalError> {
    Err(RuntimeEvalError::UnsupportedBinary {
        op: runtime_binary_op_label(op),
        lhs: runtime_value_label(lhs),
        rhs: runtime_value_label(rhs),
    })
}

pub(crate) fn runtime_unary_op_label(op: RuntimeUnaryOp) -> &'static str {
    match op {
        RuntimeUnaryOp::Not => "!",
        RuntimeUnaryOp::Neg => "-",
    }
}

pub(crate) fn runtime_binary_op_label(op: RuntimeBinaryOp) -> &'static str {
    match op {
        RuntimeBinaryOp::Eq => "==",
        RuntimeBinaryOp::Ne => "!=",
        RuntimeBinaryOp::Lt => "<",
        RuntimeBinaryOp::Le => "<=",
        RuntimeBinaryOp::Gt => ">",
        RuntimeBinaryOp::Ge => ">=",
        RuntimeBinaryOp::Add => "+",
        RuntimeBinaryOp::Sub => "-",
        RuntimeBinaryOp::Mul => "*",
        RuntimeBinaryOp::Div => "/",
        RuntimeBinaryOp::And => "&&",
        RuntimeBinaryOp::Or => "||",
    }
}

pub(crate) fn expr_runtime_label(expr: &RuntimeExpr) -> String {
    match expr {
        RuntimeExpr::Value(value) => runtime_value_label(value),
        RuntimeExpr::Local(name) => name.clone(),
        RuntimeExpr::EntityRef(target) => format!("@{target}"),
        RuntimeExpr::Let { name, .. } => format!("let {name}"),
        RuntimeExpr::Tuple(items) => format!("tuple/{}", items.len()),
        RuntimeExpr::BracketSeq(items) => format!("bracket_seq/{}", items.len()),
        RuntimeExpr::RepeatSeq { len, .. } => format!("repeat_seq/{len}"),
        RuntimeExpr::Record(fields) => format!("record/{}", fields.len()),
        RuntimeExpr::Variant { name, .. } => format!(".{name}"),
        RuntimeExpr::Field { field, .. } => format!(".{field}"),
        RuntimeExpr::Call { callee, .. } => format!("{callee}()"),
        RuntimeExpr::PureCall { helper, .. } => format!("pure#{}()", helper.0),
        RuntimeExpr::SpreadArg(expr) => format!("{}...", expr_runtime_label(expr)),
        RuntimeExpr::MethodCall { method, .. } => format!(".{method}()"),
        RuntimeExpr::Map { .. } => "map".to_owned(),
        RuntimeExpr::Sum { .. } => "sum".to_owned(),
        RuntimeExpr::Unary { op, .. } => runtime_unary_op_label(*op).to_owned(),
        RuntimeExpr::Binary { op, .. } => runtime_binary_op_label(*op).to_owned(),
        RuntimeExpr::If { .. } => "if".to_owned(),
        RuntimeExpr::IfLet { .. } => "if let".to_owned(),
        RuntimeExpr::Match { .. } => "match".to_owned(),
    }
}

pub(crate) fn runtime_value_label(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::UInt(value) => value.to_string(),
        RuntimeValue::Float(value)
        | RuntimeValue::String(value)
        | RuntimeValue::EntityRef(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::Seq(seq) => match seq {
            RuntimeSeq::Values(values) => format!("seq/values/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I8(values)) => format!("seq/i8/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I16(values)) => format!("seq/i16/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I32(values)) => format!("seq/i32/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::I64(values)) => format!("seq/i64/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U8(values)) => format!("seq/u8/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U16(values)) => format!("seq/u16/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U32(values)) => format!("seq/u32/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::U64(values)) => format!("seq/u64/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Bool(values)) => format!("seq/bool/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Bytes(values)) => format!("seq/bytes/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Chars(values)) => format!("seq/chars/{}", values.len()),
            RuntimeSeq::Dense(DenseSeq::Durations(values)) => {
                format!("seq/durations/{}", values.len())
            }
        },
        RuntimeValue::Record(fields) => format!("record/{}", fields.len()),
        RuntimeValue::Variant { name, payload, .. } => {
            if payload.is_some() {
                format!(".{name}(...)")
            } else {
                format!(".{name}")
            }
        }
    }
}
