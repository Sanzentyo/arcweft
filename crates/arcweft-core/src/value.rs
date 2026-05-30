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
    Float(String),
    String(String),
    Char(char),
    Duration(LogicalDuration),
    EntityRef(String),
    Tuple(Vec<RuntimeValue>),
    BracketSeq(Vec<RuntimeValue>),
    Record(Vec<RuntimeFieldValue>),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    },
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEnv {
    scopes: Vec<RuntimeScope>,
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
        }
    }
}

impl RuntimeEnv {
    pub fn push_scope(&mut self) {
        self.scopes.push(RuntimeScope::default());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        } else if let Some(scope) = self.scopes.last_mut() {
            scope.clear();
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set(name.into(), value);
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

    pub fn bind_all_root(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set_root(binding.name, binding.value);
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
}

impl RuntimeScope {
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
            (lhs, rhs) => unsupported_binary(op, &lhs, &rhs),
        },
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

fn runtime_unary_op_label(op: RuntimeUnaryOp) -> &'static str {
    match op {
        RuntimeUnaryOp::Not => "!",
        RuntimeUnaryOp::Neg => "-",
    }
}

fn runtime_binary_op_label(op: RuntimeBinaryOp) -> &'static str {
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
        RuntimeValue::Float(value)
        | RuntimeValue::String(value)
        | RuntimeValue::EntityRef(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::BracketSeq(values) => format!("bracket_seq/{}", values.len()),
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
