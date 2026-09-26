//! Typed runtime payload for the standard `ArcError` opaque owner.

use super::{RuntimeDialogueContentValue, RuntimeEntityReference, RuntimeUInt, RuntimeValue};
use crate::entry::{RuntimeSchemaError, RuntimeSchemaLimits};
use crate::pattern::{
    RuntimeBuiltinVariantCaseIdentity, RuntimeOpaqueTypeOwner, runtime_standard_opaque_type,
};
use crate::runtime_id::RuntimeFunctionSiteId;
use crate::task::TaskId;
use crate::time::TickId;
use crate::value::{MAX_RUNTIME_VALUE_NESTING_DEPTH, RuntimeSeq, RuntimeValueNestingError};
use arcweft_id::{DeclarationIdentityFamily, TextKey};
use arcweft_source::{
    SourceCoordinate, SourceCoordinateError, SourceDocumentId, SourceRange, SourceRevision,
};
use std::collections::BTreeSet;
use thiserror::Error;

/// Version of the typed runtime payload carried by `std.arc_error`.
pub const RUNTIME_ARC_ERROR_VALUE_VERSION: u8 = 1;

/// Stable error kind code retained by a runtime ArcError.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeArcErrorKind(Box<str>);

impl RuntimeArcErrorKind {
    /// Constructs a canonical ASCII error-kind code.
    pub fn try_new(value: impl Into<String>) -> Result<Self, RuntimeArcErrorValueError> {
        let value = value.into();
        if value.is_empty()
            || !value.as_bytes()[0].is_ascii_alphabetic()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(RuntimeArcErrorValueError::InvalidKind);
        }
        Ok(Self(value.into_boxed_str()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A typed exact cause retained by ArcError context propagation.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeArcErrorSource {
    /// Existing ArcError keeps its complete trace and structured fields.
    ArcError(Box<RuntimeArcError>),
    /// A domain-specific Result error keeps its exact runtime owner and value.
    TypedValue(RuntimeValue),
}

impl RuntimeArcErrorSource {
    #[must_use]
    pub fn as_typed_value(&self) -> Option<&RuntimeValue> {
        match self {
            Self::ArcError(_) => None,
            Self::TypedValue(value) => Some(value),
        }
    }

    #[must_use]
    pub fn as_arc_error(&self) -> Option<&RuntimeArcError> {
        match self {
            Self::ArcError(value) => Some(value),
            Self::TypedValue(_) => None,
        }
    }
}

/// Fixed-width state digest used by an ArcError trace.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeArcErrorStateHash([u8; 32]);

impl RuntimeArcErrorStateHash {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Optional native backtrace text retained by an ArcError trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArcErrorNativeBacktrace(Box<str>);

impl RuntimeArcErrorNativeBacktrace {
    pub fn try_new(
        value: impl Into<String>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        let value = value.into();
        ensure_string_limit(&value, limits, "native backtrace")?;
        Ok(Self(value.into_boxed_str()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One source-aware runtime trace frame.
///
/// `voice_key` is reserved in the version-1 wire shape until a lower-layer
/// voice-key owner exists; producers leave that slot absent. Source positions
/// use [`SourceCoordinate`], which retains revision and bounds without
/// pretending that a decoded range has been checked against UTF-8 source text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeArcErrorFrame {
    source: Option<SourceCoordinate>,
    flow: Option<RuntimeEntityReference>,
    line: Option<RuntimeEntityReference>,
    text_key: Option<TextKey>,
    entity: Option<RuntimeEntityReference>,
    function: Option<RuntimeFunctionSiteId>,
    dispatch_owner: Option<RuntimeEntityReference>,
    dispatch_event: Option<RuntimeEntityReference>,
    task: Option<TaskId>,
    await_target: Option<RuntimeEntityReference>,
    message: Option<RuntimeDialogueContentValue>,
}

impl RuntimeArcErrorFrame {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_source(mut self, source: SourceCoordinate) -> Self {
        self.source = Some(source);
        self
    }

    /// Adds a checked Flow project reference.
    pub fn with_flow(
        mut self,
        flow: RuntimeEntityReference,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        if !matches!(
            flow,
            RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Flow,
                ..
            }
        ) {
            return Err(RuntimeArcErrorValueError::InvalidFlowReference);
        }
        self.flow = Some(flow);
        Ok(self)
    }

    /// Adds a checked DialogueLine reference.
    pub fn with_line(
        mut self,
        line: RuntimeEntityReference,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        if !matches!(line, RuntimeEntityReference::DialogueLine(_)) {
            return Err(RuntimeArcErrorValueError::InvalidLineReference);
        }
        self.line = Some(line);
        Ok(self)
    }

    #[must_use]
    pub fn with_text_key(mut self, text_key: TextKey) -> Self {
        self.text_key = Some(text_key);
        self
    }

    #[must_use]
    pub fn with_entity(mut self, entity: RuntimeEntityReference) -> Self {
        self.entity = Some(entity);
        self
    }

    #[must_use]
    pub fn with_function(mut self, function: RuntimeFunctionSiteId) -> Self {
        self.function = Some(function);
        self
    }

    #[must_use]
    pub fn with_dispatch_owner(mut self, owner: RuntimeEntityReference) -> Self {
        self.dispatch_owner = Some(owner);
        self
    }

    #[must_use]
    pub fn with_dispatch_event(mut self, event: RuntimeEntityReference) -> Self {
        self.dispatch_event = Some(event);
        self
    }

    #[must_use]
    pub fn with_task(mut self, task: TaskId) -> Self {
        self.task = Some(task);
        self
    }

    #[must_use]
    pub fn with_await_target(mut self, target: RuntimeEntityReference) -> Self {
        self.await_target = Some(target);
        self
    }

    #[must_use]
    pub fn with_message(mut self, message: RuntimeDialogueContentValue) -> Self {
        self.message = Some(message);
        self
    }

    #[must_use]
    pub fn source(&self) -> Option<&SourceCoordinate> {
        self.source.as_ref()
    }

    #[must_use]
    pub fn flow(&self) -> Option<&RuntimeEntityReference> {
        self.flow.as_ref()
    }

    #[must_use]
    pub fn line(&self) -> Option<&RuntimeEntityReference> {
        self.line.as_ref()
    }

    #[must_use]
    pub fn text_key(&self) -> Option<&TextKey> {
        self.text_key.as_ref()
    }

    #[must_use]
    pub fn entity(&self) -> Option<&RuntimeEntityReference> {
        self.entity.as_ref()
    }

    #[must_use]
    pub const fn function(&self) -> Option<RuntimeFunctionSiteId> {
        self.function
    }

    #[must_use]
    pub fn dispatch_owner(&self) -> Option<&RuntimeEntityReference> {
        self.dispatch_owner.as_ref()
    }

    #[must_use]
    pub fn dispatch_event(&self) -> Option<&RuntimeEntityReference> {
        self.dispatch_event.as_ref()
    }

    #[must_use]
    pub fn task(&self) -> Option<&TaskId> {
        self.task.as_ref()
    }

    #[must_use]
    pub fn await_target(&self) -> Option<&RuntimeEntityReference> {
        self.await_target.as_ref()
    }

    #[must_use]
    pub fn message(&self) -> Option<&RuntimeDialogueContentValue> {
        self.message.as_ref()
    }
}

/// Source trace and optional runtime metadata for one ArcError.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeArcErrorTrace {
    origin: RuntimeArcErrorFrame,
    frames: Vec<RuntimeArcErrorFrame>,
    state_hash: Option<RuntimeArcErrorStateHash>,
    tick: Option<TickId>,
    native_backtrace: Option<RuntimeArcErrorNativeBacktrace>,
}

impl RuntimeArcErrorTrace {
    #[must_use]
    pub fn new(origin: RuntimeArcErrorFrame) -> Self {
        Self {
            origin,
            frames: Vec::new(),
            state_hash: None,
            tick: None,
            native_backtrace: None,
        }
    }

    #[must_use]
    pub fn with_frame(mut self, frame: RuntimeArcErrorFrame) -> Self {
        self.frames.push(frame);
        self
    }

    #[must_use]
    pub fn with_state_hash(mut self, state_hash: RuntimeArcErrorStateHash) -> Self {
        self.state_hash = Some(state_hash);
        self
    }

    #[must_use]
    pub fn with_tick(mut self, tick: TickId) -> Self {
        self.tick = Some(tick);
        self
    }

    #[must_use]
    pub fn with_native_backtrace(mut self, backtrace: RuntimeArcErrorNativeBacktrace) -> Self {
        self.native_backtrace = Some(backtrace);
        self
    }

    #[must_use]
    pub fn origin(&self) -> &RuntimeArcErrorFrame {
        &self.origin
    }

    #[must_use]
    pub fn frames(&self) -> &[RuntimeArcErrorFrame] {
        &self.frames
    }

    #[must_use]
    pub const fn state_hash(&self) -> Option<RuntimeArcErrorStateHash> {
        self.state_hash
    }

    #[must_use]
    pub const fn tick(&self) -> Option<TickId> {
        self.tick
    }

    #[must_use]
    pub fn native_backtrace(&self) -> Option<&RuntimeArcErrorNativeBacktrace> {
        self.native_backtrace.as_ref()
    }

    fn append_context(
        &mut self,
        mut frame: RuntimeArcErrorFrame,
        message: RuntimeDialogueContentValue,
    ) {
        frame.message = Some(message);
        self.frames.push(frame);
    }
}

/// One ordered, typed key/value pair in ArcError data.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeArcErrorDataEntry {
    key: String,
    value: RuntimeValue,
}

impl RuntimeArcErrorDataEntry {
    pub fn try_new(
        key: impl Into<String>,
        value: RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        let key = key.into();
        if key.is_empty() {
            return Err(RuntimeArcErrorValueError::EmptyDataKey);
        }
        ensure_string_limit(&key, limits, "data key")?;
        Ok(Self { key, value })
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn value(&self) -> &RuntimeValue {
        &self.value
    }
}

/// One validated standard runtime ArcError payload.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeArcError {
    kind: RuntimeArcErrorKind,
    message: RuntimeDialogueContentValue,
    source: Option<RuntimeArcErrorSource>,
    trace: RuntimeArcErrorTrace,
    data: Vec<RuntimeArcErrorDataEntry>,
}

/// Invalid or over-limit ArcError runtime payload.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeArcErrorValueError {
    #[error("runtime value is not owned by the exact std.arc_error owner")]
    InvalidOwner,
    #[error("context operation receiver is not the expected Result or Option carrier")]
    InvalidContextReceiver,
    #[error("ArcError payload is not the canonical version-1 tuple")]
    InvalidPayload,
    #[error("ArcError payload version is {actual}, expected {expected}")]
    UnsupportedVersion { actual: u8, expected: u8 },
    #[error("ArcError kind must be a nonempty ASCII identifier")]
    InvalidKind,
    #[error("ArcError Flow frame reference does not belong to the Flow family")]
    InvalidFlowReference,
    #[error("ArcError dialogue-line frame reference is not a DialogueLine")]
    InvalidLineReference,
    #[error("ArcError data key must not be empty")]
    EmptyDataKey,
    #[error("ArcError data key `{key}` occurs more than once")]
    DuplicateDataKey { key: String },
    #[error("ArcError {field} has {actual} entries, above the limit {maximum}")]
    SequenceLimit {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("ArcError {field} contains {actual} bytes, above the limit {maximum}")]
    StringLimit {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("ArcError runtime value is invalid: {message}")]
    InvalidRuntimeValue { message: String },
    #[error("ArcError Content message is invalid: {message}")]
    InvalidContent { message: String },
    #[error("ArcError source coordinate is invalid: {message}")]
    InvalidSourceCoordinate { message: String },
    #[error("ArcError {field} field is invalid")]
    InvalidField { field: &'static str },
}

impl RuntimeArcError {
    /// Constructs an ArcError payload under the named engine limits.
    pub fn try_new(
        kind: RuntimeArcErrorKind,
        message: RuntimeDialogueContentValue,
        source: Option<RuntimeArcErrorSource>,
        trace: RuntimeArcErrorTrace,
        data: impl IntoIterator<Item = RuntimeArcErrorDataEntry>,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        Self::try_new_with_limits(
            kind,
            message,
            source,
            trace,
            data,
            RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Constructs an ArcError payload after validating the complete recursive
    /// runtime value against one shared limits policy.
    pub fn try_new_with_limits(
        kind: RuntimeArcErrorKind,
        message: RuntimeDialogueContentValue,
        source: Option<RuntimeArcErrorSource>,
        trace: RuntimeArcErrorTrace,
        data: impl IntoIterator<Item = RuntimeArcErrorDataEntry>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        let value = Self {
            kind,
            message,
            source,
            trace,
            data: data.into_iter().collect(),
        };
        value.validate(limits)?;
        Ok(value)
    }

    /// Wraps an exact Result error value with context, retaining an existing
    /// ArcError as its typed source when the value has that exact owner.
    pub fn context_from_result(
        error: RuntimeValue,
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        Self::context_from_result_with_limits(
            error,
            message,
            frame,
            RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Limits-aware form of [`Self::context_from_result`].
    pub fn context_from_result_with_limits(
        error: RuntimeValue,
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        let source = if arc_error_owner().accepts_opaque_value(match &error {
            RuntimeValue::Opaque(value) => value,
            _ => return Self::context_from_typed_value(error, message, frame, limits),
        }) {
            RuntimeArcErrorSource::ArcError(Box::new(Self::try_from_runtime_value_with_limits(
                &error, limits,
            )?))
        } else {
            RuntimeArcErrorSource::TypedValue(error)
        };
        Self::context_from_source(source, message, frame, limits)
    }

    /// Applies eager Result context to one canonical runtime carrier. `Ok`
    /// passes through unchanged; `Err` becomes a canonical `ArcError` while
    /// preserving its exact typed cause and appending the supplied frame.
    pub fn context_result_value(
        result: RuntimeValue,
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
    ) -> Result<RuntimeValue, RuntimeArcErrorValueError> {
        Self::context_result_value_with(
            result,
            || Ok(message),
            frame,
            RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Applies lazy Result context. The message producer is called exactly
    /// once on `Err` and never on `Ok`.
    pub fn context_result_value_with(
        result: RuntimeValue,
        message: impl FnOnce() -> Result<RuntimeDialogueContentValue, RuntimeArcErrorValueError>,
        frame: RuntimeArcErrorFrame,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, RuntimeArcErrorValueError> {
        let (case, payload) = result
            .try_into_builtin_variant_case()
            .map_err(|_| RuntimeArcErrorValueError::InvalidContextReceiver)?;
        match case {
            RuntimeBuiltinVariantCaseIdentity::ResultOk => payload
                .map(RuntimeValue::result_ok)
                .ok_or(RuntimeArcErrorValueError::InvalidContextReceiver),
            RuntimeBuiltinVariantCaseIdentity::ResultErr => {
                let cause = payload.ok_or(RuntimeArcErrorValueError::InvalidContextReceiver)?;
                let message = message()?;
                let error = Self::context_from_result_with_limits(cause, message, frame, limits)?;
                Ok(RuntimeValue::result_err(error.into_runtime_value()))
            }
            _ => Err(RuntimeArcErrorValueError::InvalidContextReceiver),
        }
    }

    /// Applies eager Option context. `Some` becomes `Ok`; `None` becomes a
    /// canonical `MissingValue` ArcError carrying the supplied Content.
    pub fn context_option_value(
        option: RuntimeValue,
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
    ) -> Result<RuntimeValue, RuntimeArcErrorValueError> {
        Self::context_option_value_with(
            option,
            || Ok(message),
            frame,
            RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Applies lazy Option context. The message producer is called exactly
    /// once on `None` and never on `Some`.
    pub fn context_option_value_with(
        option: RuntimeValue,
        message: impl FnOnce() -> Result<RuntimeDialogueContentValue, RuntimeArcErrorValueError>,
        frame: RuntimeArcErrorFrame,
        limits: RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, RuntimeArcErrorValueError> {
        let (case, payload) = option
            .try_into_builtin_variant_case()
            .map_err(|_| RuntimeArcErrorValueError::InvalidContextReceiver)?;
        match case {
            RuntimeBuiltinVariantCaseIdentity::OptionSome => payload
                .map(RuntimeValue::result_ok)
                .ok_or(RuntimeArcErrorValueError::InvalidContextReceiver),
            RuntimeBuiltinVariantCaseIdentity::OptionNone => {
                if payload.is_some() {
                    return Err(RuntimeArcErrorValueError::InvalidContextReceiver);
                }
                let message = message()?;
                let error = Self::missing_value_with_limits(message, frame, limits)?;
                Ok(RuntimeValue::result_err(error.into_runtime_value()))
            }
            _ => Err(RuntimeArcErrorValueError::InvalidContextReceiver),
        }
    }

    fn context_from_typed_value(
        error: RuntimeValue,
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        Self::context_from_source(
            RuntimeArcErrorSource::TypedValue(error),
            message,
            frame,
            limits,
        )
    }

    fn context_from_source(
        source: RuntimeArcErrorSource,
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        let (kind, mut trace, data) = match &source {
            RuntimeArcErrorSource::ArcError(error) => {
                (error.kind.clone(), error.trace.clone(), error.data.clone())
            }
            RuntimeArcErrorSource::TypedValue(_) => (
                RuntimeArcErrorKind::try_new("Context")?,
                RuntimeArcErrorTrace::new(RuntimeArcErrorFrame::empty()),
                Vec::new(),
            ),
        };
        trace.append_context(frame, message.clone());
        Self::try_new_with_limits(kind, message, Some(source), trace, data, limits)
    }

    /// Materializes `Option::None` as a MissingValue ArcError and attaches the
    /// supplied Content context frame.
    pub fn missing_value(
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        Self::missing_value_with_limits(message, frame, RuntimeSchemaLimits::engine_default())
    }

    /// Limits-aware form of [`Self::missing_value`].
    pub fn missing_value_with_limits(
        message: RuntimeDialogueContentValue,
        frame: RuntimeArcErrorFrame,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        let mut trace = RuntimeArcErrorTrace::new(RuntimeArcErrorFrame::empty());
        trace.append_context(frame, message.clone());
        Self::try_new_with_limits(
            RuntimeArcErrorKind::try_new("MissingValue")?,
            message,
            None,
            trace,
            [],
            limits,
        )
    }

    /// Decodes the exact `std.arc_error` payload using engine limits.
    pub fn try_from_runtime_value(value: &RuntimeValue) -> Result<Self, RuntimeArcErrorValueError> {
        Self::try_from_runtime_value_with_limits(value, RuntimeSchemaLimits::engine_default())
    }

    /// Decodes the exact `std.arc_error` payload under shared runtime limits.
    pub fn try_from_runtime_value_with_limits(
        value: &RuntimeValue,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, RuntimeArcErrorValueError> {
        validate_runtime_value(value, limits)?;
        let opaque = match value {
            RuntimeValue::Opaque(value) if arc_error_owner().accepts_opaque_value(value) => value,
            _ => return Err(RuntimeArcErrorValueError::InvalidOwner),
        };
        let RuntimeValue::Tuple(fields) = opaque.payload() else {
            return Err(RuntimeArcErrorValueError::InvalidPayload);
        };
        let [version, kind, message, source, trace, data] = fields.as_slice() else {
            return Err(RuntimeArcErrorValueError::InvalidPayload);
        };
        let actual_version = decode_u8(version, "version")?;
        if actual_version != RUNTIME_ARC_ERROR_VALUE_VERSION {
            return Err(RuntimeArcErrorValueError::UnsupportedVersion {
                actual: actual_version,
                expected: RUNTIME_ARC_ERROR_VALUE_VERSION,
            });
        }
        let kind = RuntimeArcErrorKind::try_new(decode_string(kind, "kind")?.to_owned())?;
        let message =
            RuntimeDialogueContentValue::try_from_runtime_value_with_limits(message, limits)
                .map_err(|error| RuntimeArcErrorValueError::InvalidContent {
                    message: error.to_string(),
                })?;
        let source = decode_option(source, "source")?
            .map(|source| decode_source(source, limits))
            .transpose()?;
        let trace = decode_trace(trace, limits)?;
        let data = decode_data(data, limits)?;
        Self::try_new_with_limits(kind, message, source, trace, data, limits)
    }

    /// Encodes this validated ArcError through the exact standard opaque owner.
    #[must_use]
    pub fn into_runtime_value(self) -> RuntimeValue {
        arc_error_owner()
            .try_wrap(self.encode_payload())
            .expect("the standard ArcError owner is exact")
    }

    #[must_use]
    pub fn kind(&self) -> &RuntimeArcErrorKind {
        &self.kind
    }

    #[must_use]
    pub fn message(&self) -> &RuntimeDialogueContentValue {
        &self.message
    }

    #[must_use]
    pub fn source(&self) -> Option<&RuntimeArcErrorSource> {
        self.source.as_ref()
    }

    #[must_use]
    pub fn trace(&self) -> &RuntimeArcErrorTrace {
        &self.trace
    }

    #[must_use]
    pub fn data(&self) -> &[RuntimeArcErrorDataEntry] {
        &self.data
    }

    fn validate(&self, limits: RuntimeSchemaLimits) -> Result<(), RuntimeArcErrorValueError> {
        ensure_string_limit(self.kind.as_str(), limits, "kind")?;
        if self.data.len() > usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX) {
            return Err(RuntimeArcErrorValueError::SequenceLimit {
                field: "data",
                actual: self.data.len(),
                maximum: usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        if self.trace.frames.len()
            > usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX)
        {
            return Err(RuntimeArcErrorValueError::SequenceLimit {
                field: "trace frames",
                actual: self.trace.frames.len(),
                maximum: usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX),
            });
        }
        let mut keys = BTreeSet::new();
        for entry in &self.data {
            ensure_string_limit(&entry.key, limits, "data key")?;
            if !keys.insert(entry.key.as_str()) {
                return Err(RuntimeArcErrorValueError::DuplicateDataKey {
                    key: entry.key.clone(),
                });
            }
        }
        validate_frame(&self.trace.origin)?;
        for frame in &self.trace.frames {
            validate_frame(frame)?;
        }
        if let Some(backtrace) = &self.trace.native_backtrace {
            ensure_string_limit(backtrace.as_str(), limits, "native backtrace")?;
        }
        validate_runtime_value(&self.encode_payload(), limits)
    }

    fn encode_payload(&self) -> RuntimeValue {
        let source = self
            .source
            .as_ref()
            .map_or_else(RuntimeValue::option_none, |source| {
                RuntimeValue::option_some(encode_source(source))
            });
        let data = RuntimeValue::Seq(RuntimeSeq::Values(
            self.data
                .iter()
                .map(|entry| {
                    RuntimeValue::Tuple(vec![
                        RuntimeValue::String(entry.key.clone()),
                        entry.value.clone(),
                    ])
                })
                .collect(),
        ));
        RuntimeValue::Tuple(vec![
            RuntimeValue::u8(RUNTIME_ARC_ERROR_VALUE_VERSION),
            RuntimeValue::String(self.kind.as_str().to_owned()),
            self.message.clone().into_runtime_value(),
            source,
            encode_trace(&self.trace),
            data,
        ])
    }
}

fn arc_error_owner() -> RuntimeOpaqueTypeOwner {
    runtime_standard_opaque_type(&["ArcError"])
        .and_then(|spec| spec.monomorphic_owner())
        .expect("the standard ArcError opaque type has a monomorphic owner")
}

fn encode_source(source: &RuntimeArcErrorSource) -> RuntimeValue {
    match source {
        RuntimeArcErrorSource::ArcError(error) => error.clone().into_runtime_value(),
        RuntimeArcErrorSource::TypedValue(value) => value.clone(),
    }
}

fn decode_source(
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
) -> Result<RuntimeArcErrorSource, RuntimeArcErrorValueError> {
    if matches!(value, RuntimeValue::Opaque(opaque) if arc_error_owner().accepts_opaque_value(opaque))
    {
        Ok(RuntimeArcErrorSource::ArcError(Box::new(
            RuntimeArcError::try_from_runtime_value_with_limits(value, limits)?,
        )))
    } else {
        Ok(RuntimeArcErrorSource::TypedValue(value.clone()))
    }
}

fn encode_trace(trace: &RuntimeArcErrorTrace) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        encode_frame(&trace.origin),
        RuntimeValue::Seq(RuntimeSeq::Values(
            trace.frames.iter().map(encode_frame).collect(),
        )),
        trace
            .state_hash
            .map_or_else(RuntimeValue::option_none, |hash| {
                RuntimeValue::option_some(RuntimeValue::Seq(RuntimeSeq::dense_bytes(
                    hash.as_bytes().to_vec(),
                )))
            }),
        trace.tick.map_or_else(RuntimeValue::option_none, |tick| {
            RuntimeValue::option_some(RuntimeValue::u64(tick.0))
        }),
        // Replay cursor remains absent until its Core owner is defined.
        RuntimeValue::option_none(),
        trace
            .native_backtrace
            .as_ref()
            .map_or_else(RuntimeValue::option_none, |value| {
                RuntimeValue::option_some(RuntimeValue::String(value.as_str().to_owned()))
            }),
    ])
}

fn decode_trace(
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
) -> Result<RuntimeArcErrorTrace, RuntimeArcErrorValueError> {
    let RuntimeValue::Tuple(fields) = value else {
        return Err(RuntimeArcErrorValueError::InvalidField { field: "trace" });
    };
    let [
        origin,
        frames,
        state_hash,
        tick,
        replay_cursor,
        native_backtrace,
    ] = fields.as_slice()
    else {
        return Err(RuntimeArcErrorValueError::InvalidField { field: "trace" });
    };
    let origin = decode_frame(origin, limits)?;
    let frames = decode_sequence(frames, "trace frames")?
        .iter()
        .map(|frame| decode_frame(frame, limits))
        .collect::<Result<Vec<_>, _>>()?;
    if frames.len() > usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX) {
        return Err(RuntimeArcErrorValueError::SequenceLimit {
            field: "trace frames",
            actual: frames.len(),
            maximum: usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX),
        });
    }
    let state_hash = decode_option(state_hash, "state hash")?
        .map(|hash| decode_digest(hash, "state hash").map(RuntimeArcErrorStateHash::from_bytes))
        .transpose()?;
    let tick = decode_option(tick, "tick")?
        .map(|tick| decode_u64(tick, "tick").map(TickId))
        .transpose()?;
    if decode_option(replay_cursor, "replay cursor")?.is_some() {
        return Err(RuntimeArcErrorValueError::InvalidField {
            field: "unowned replay cursor",
        });
    }
    let native_backtrace = decode_option(native_backtrace, "native backtrace")?
        .map(|value| {
            RuntimeArcErrorNativeBacktrace::try_new(
                decode_string(value, "native backtrace")?.to_owned(),
                limits,
            )
        })
        .transpose()?;
    Ok(RuntimeArcErrorTrace {
        origin,
        frames,
        state_hash,
        tick,
        native_backtrace,
    })
}

fn encode_frame(frame: &RuntimeArcErrorFrame) -> RuntimeValue {
    RuntimeValue::Tuple(vec![
        encode_option(frame.source.as_ref().map(encode_source_coordinate)),
        encode_option(frame.flow.clone().map(RuntimeValue::EntityRef)),
        encode_option(frame.line.clone().map(RuntimeValue::EntityRef)),
        encode_option(
            frame
                .text_key
                .as_ref()
                .map(|value| RuntimeValue::String(value.as_str().to_owned())),
        ),
        // VoiceKey remains absent until its lower-layer owner exists.
        RuntimeValue::option_none(),
        encode_option(frame.entity.clone().map(RuntimeValue::EntityRef)),
        encode_option(
            frame
                .function
                .map(|value| RuntimeValue::u32(value.get().get())),
        ),
        encode_option(frame.dispatch_owner.clone().map(RuntimeValue::EntityRef)),
        encode_option(frame.dispatch_event.clone().map(RuntimeValue::EntityRef)),
        encode_option(
            frame
                .task
                .as_ref()
                .map(|value| RuntimeValue::String(value.0.clone())),
        ),
        encode_option(frame.await_target.clone().map(RuntimeValue::EntityRef)),
        encode_option(
            frame
                .message
                .as_ref()
                .map(|value| value.clone().into_runtime_value()),
        ),
    ])
}

fn decode_frame(
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
) -> Result<RuntimeArcErrorFrame, RuntimeArcErrorValueError> {
    let RuntimeValue::Tuple(fields) = value else {
        return Err(RuntimeArcErrorValueError::InvalidField {
            field: "trace frame",
        });
    };
    let [
        source,
        flow,
        line,
        text_key,
        voice_key,
        entity,
        function,
        dispatch_owner,
        dispatch_event,
        task,
        await_target,
        message,
    ] = fields.as_slice()
    else {
        return Err(RuntimeArcErrorValueError::InvalidField {
            field: "trace frame",
        });
    };
    if decode_option(voice_key, "voice key")?.is_some() {
        return Err(RuntimeArcErrorValueError::InvalidField {
            field: "unowned voice key",
        });
    }
    let flow = decode_option(flow, "flow")?
        .map(|value| decode_entity_ref(value, "flow"))
        .transpose()?;
    let line = decode_option(line, "line")?
        .map(|value| decode_entity_ref(value, "line"))
        .transpose()?;
    let text_key = decode_option(text_key, "text key")?
        .map(|value| {
            TextKey::try_new(decode_string(value, "text key")?.to_owned())
                .map_err(|_| RuntimeArcErrorValueError::InvalidField { field: "text key" })
        })
        .transpose()?;
    let entity = decode_option(entity, "entity")?
        .map(|value| decode_entity_ref(value, "entity"))
        .transpose()?;
    let function = decode_option(function, "function")?
        .map(|value| {
            let value = decode_u32(value, "function")?;
            let value = std::num::NonZeroU32::new(value)
                .ok_or(RuntimeArcErrorValueError::InvalidField { field: "function" })?;
            Ok(RuntimeFunctionSiteId::from_accepted_ordinal(value))
        })
        .transpose()?;
    let dispatch_owner = decode_option(dispatch_owner, "dispatch owner")?
        .map(|value| decode_entity_ref(value, "dispatch owner"))
        .transpose()?;
    let dispatch_event = decode_option(dispatch_event, "dispatch event")?
        .map(|value| decode_entity_ref(value, "dispatch event"))
        .transpose()?;
    let task = decode_option(task, "task")?
        .map(|value| Ok(TaskId(decode_string(value, "task")?.to_owned())))
        .transpose()?;
    let await_target = decode_option(await_target, "await target")?
        .map(|value| decode_entity_ref(value, "await target"))
        .transpose()?;
    let message = decode_option(message, "frame message")?
        .map(|value| {
            RuntimeDialogueContentValue::try_from_runtime_value_with_limits(value, limits).map_err(
                |error| RuntimeArcErrorValueError::InvalidContent {
                    message: error.to_string(),
                },
            )
        })
        .transpose()?;
    Ok(RuntimeArcErrorFrame {
        source: decode_option(source, "source coordinate")?
            .map(decode_source_coordinate)
            .transpose()?,
        flow,
        line,
        text_key,
        entity,
        function,
        dispatch_owner,
        dispatch_event,
        task,
        await_target,
        message,
    })
}

fn validate_frame(frame: &RuntimeArcErrorFrame) -> Result<(), RuntimeArcErrorValueError> {
    if frame.flow.as_ref().is_some_and(|flow| {
        !matches!(
            flow,
            RuntimeEntityReference::Project {
                family: DeclarationIdentityFamily::Flow,
                ..
            }
        )
    }) {
        return Err(RuntimeArcErrorValueError::InvalidFlowReference);
    }
    if frame
        .line
        .as_ref()
        .is_some_and(|line| !matches!(line, RuntimeEntityReference::DialogueLine(_)))
    {
        return Err(RuntimeArcErrorValueError::InvalidLineReference);
    }
    Ok(())
}

fn encode_source_coordinate(coordinate: &SourceCoordinate) -> RuntimeValue {
    let revision = coordinate.source().revision();
    RuntimeValue::Tuple(vec![
        RuntimeValue::String(coordinate.source().id().as_str().to_owned()),
        RuntimeValue::Seq(RuntimeSeq::dense_bytes(revision.as_bytes().to_vec())),
        RuntimeValue::u64(coordinate.source().source_len()),
        RuntimeValue::u64(coordinate.range().start() as u64),
        RuntimeValue::u64(coordinate.range().end() as u64),
    ])
}

fn decode_source_coordinate(
    value: &RuntimeValue,
) -> Result<SourceCoordinate, RuntimeArcErrorValueError> {
    let RuntimeValue::Tuple(fields) = value else {
        return Err(RuntimeArcErrorValueError::InvalidField {
            field: "source coordinate",
        });
    };
    let [id, revision, source_len, start, end] = fields.as_slice() else {
        return Err(RuntimeArcErrorValueError::InvalidField {
            field: "source coordinate",
        });
    };
    let id =
        SourceDocumentId::try_new(decode_string(id, "source id")?.to_owned()).map_err(|error| {
            RuntimeArcErrorValueError::InvalidSourceCoordinate {
                message: error.to_string(),
            }
        })?;
    let revision = decode_digest(revision, "source revision")?;
    let source_len = decode_u64(source_len, "source length")?;
    let start = usize::try_from(decode_u64(start, "source range start")?).map_err(|_| {
        RuntimeArcErrorValueError::InvalidField {
            field: "source range start",
        }
    })?;
    let end = usize::try_from(decode_u64(end, "source range end")?).map_err(|_| {
        RuntimeArcErrorValueError::InvalidField {
            field: "source range end",
        }
    })?;
    SourceCoordinate::try_from_parts(
        id,
        SourceRevision::from_bytes(revision),
        source_len,
        SourceRange::new(start, end),
    )
    .map_err(source_coordinate_error)
}

fn source_coordinate_error(error: SourceCoordinateError) -> RuntimeArcErrorValueError {
    RuntimeArcErrorValueError::InvalidSourceCoordinate {
        message: error.to_string(),
    }
}

fn decode_data(
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
) -> Result<Vec<RuntimeArcErrorDataEntry>, RuntimeArcErrorValueError> {
    let entries = decode_sequence(value, "data")?;
    if entries.len() > usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX) {
        return Err(RuntimeArcErrorValueError::SequenceLimit {
            field: "data",
            actual: entries.len(),
            maximum: usize::try_from(limits.max_sequence_items).unwrap_or(usize::MAX),
        });
    }
    entries
        .iter()
        .map(|entry| {
            let RuntimeValue::Tuple(fields) = entry else {
                return Err(RuntimeArcErrorValueError::InvalidField {
                    field: "data entry",
                });
            };
            let [key, value] = fields.as_slice() else {
                return Err(RuntimeArcErrorValueError::InvalidField {
                    field: "data entry",
                });
            };
            RuntimeArcErrorDataEntry::try_new(
                decode_string(key, "data key")?.to_owned(),
                value.clone(),
                limits,
            )
        })
        .collect()
}

fn encode_option(value: Option<RuntimeValue>) -> RuntimeValue {
    value.map_or_else(RuntimeValue::option_none, RuntimeValue::option_some)
}

fn decode_option<'a>(
    value: &'a RuntimeValue,
    field: &'static str,
) -> Result<Option<&'a RuntimeValue>, RuntimeArcErrorValueError> {
    match value.builtin_variant_case() {
        Some((RuntimeBuiltinVariantCaseIdentity::OptionNone, None)) => Ok(None),
        Some((RuntimeBuiltinVariantCaseIdentity::OptionSome, Some(value))) => Ok(Some(value)),
        _ => Err(RuntimeArcErrorValueError::InvalidField { field }),
    }
}

fn decode_sequence<'a>(
    value: &'a RuntimeValue,
    field: &'static str,
) -> Result<&'a [RuntimeValue], RuntimeArcErrorValueError> {
    match value {
        RuntimeValue::Seq(RuntimeSeq::Values(values)) => Ok(values),
        _ => Err(RuntimeArcErrorValueError::InvalidField { field }),
    }
}

fn decode_entity_ref(
    value: &RuntimeValue,
    field: &'static str,
) -> Result<RuntimeEntityReference, RuntimeArcErrorValueError> {
    match value {
        RuntimeValue::EntityRef(value) => Ok(value.clone()),
        _ => Err(RuntimeArcErrorValueError::InvalidField { field }),
    }
}

fn decode_string<'a>(
    value: &'a RuntimeValue,
    field: &'static str,
) -> Result<&'a str, RuntimeArcErrorValueError> {
    match value {
        RuntimeValue::String(value) => Ok(value),
        _ => Err(RuntimeArcErrorValueError::InvalidField { field }),
    }
}

fn decode_u8(value: &RuntimeValue, field: &'static str) -> Result<u8, RuntimeArcErrorValueError> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U8(value)) => Ok(*value),
        _ => Err(RuntimeArcErrorValueError::InvalidField { field }),
    }
}

fn decode_u32(value: &RuntimeValue, field: &'static str) -> Result<u32, RuntimeArcErrorValueError> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U32(value)) => Ok(*value),
        _ => Err(RuntimeArcErrorValueError::InvalidField { field }),
    }
}

fn decode_u64(value: &RuntimeValue, field: &'static str) -> Result<u64, RuntimeArcErrorValueError> {
    match value {
        RuntimeValue::UInt(RuntimeUInt::U64(value)) => Ok(*value),
        _ => Err(RuntimeArcErrorValueError::InvalidField { field }),
    }
}

fn decode_digest(
    value: &RuntimeValue,
    field: &'static str,
) -> Result<[u8; 32], RuntimeArcErrorValueError> {
    let RuntimeValue::Seq(sequence) = value else {
        return Err(RuntimeArcErrorValueError::InvalidField { field });
    };
    if sequence.dense_kind() != Some(crate::value::DenseSeqKind::Bytes) {
        return Err(RuntimeArcErrorValueError::InvalidField { field });
    }
    sequence
        .as_bytes()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .ok_or(RuntimeArcErrorValueError::InvalidField { field })
}

fn validate_runtime_value(
    value: &RuntimeValue,
    limits: RuntimeSchemaLimits,
) -> Result<(), RuntimeArcErrorValueError> {
    let depth_limit = usize::try_from(limits.max_depth)
        .unwrap_or(usize::MAX)
        .min(MAX_RUNTIME_VALUE_NESTING_DEPTH);
    value
        .validate_nesting_depth(depth_limit)
        .map_err(|error: RuntimeValueNestingError| {
            RuntimeArcErrorValueError::InvalidRuntimeValue {
                message: error.to_string(),
            }
        })?;
    crate::entry::schema::canonical_runtime_snapshot_value_bytes(value, limits).map_err(
        |error: RuntimeSchemaError| RuntimeArcErrorValueError::InvalidRuntimeValue {
            message: error.to_string(),
        },
    )?;
    Ok(())
}

fn ensure_string_limit(
    value: &str,
    limits: RuntimeSchemaLimits,
    field: &'static str,
) -> Result<(), RuntimeArcErrorValueError> {
    if limits.permits_string_bytes(value.len()) {
        Ok(())
    } else {
        Err(RuntimeArcErrorValueError::StringLimit {
            field,
            actual: value.len(),
            maximum: usize::try_from(limits.max_string_bytes).unwrap_or(usize::MAX),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::RuntimeArtifactFingerprint;
    use crate::entry::RuntimeDialogueContentTemplateDigest;
    use crate::runtime_id::RuntimeDialogueContentTemplateId;
    use crate::value::RuntimeDialogueContentValue;

    fn content_message() -> RuntimeDialogueContentValue {
        RuntimeDialogueContentValue::try_new(
            RuntimeArtifactFingerprint::try_from_bytes([1; 32]).expect("artifact fingerprint"),
            RuntimeDialogueContentTemplateId::from_accepted_ordinal(
                std::num::NonZeroU32::new(1).expect("template id"),
            ),
            RuntimeDialogueContentTemplateDigest::from_bytes([2; 32]),
            [],
        )
        .expect("typed Content message")
    }

    #[test]
    fn result_error_context_round_trip_preserves_exact_typed_cause() {
        let cause = RuntimeValue::Opaque(crate::value::RuntimeOpaqueValue::new_exact(
            &runtime_standard_opaque_type(&["AssetError"])
                .and_then(|spec| spec.monomorphic_owner())
                .expect("AssetError owner"),
            RuntimeValue::String("missing".to_owned()),
        ));
        let error = RuntimeArcError::context_from_result(
            cause.clone(),
            content_message(),
            RuntimeArcErrorFrame::empty(),
        )
        .expect("context ArcError");
        let encoded = error.clone().into_runtime_value();
        let decoded = RuntimeArcError::try_from_runtime_value(&encoded).expect("decode ArcError");

        assert_eq!(decoded, error);
        assert!(matches!(
            decoded.source(),
            Some(RuntimeArcErrorSource::TypedValue(value)) if value == &cause
        ));
        assert_eq!(
            decoded.trace().frames().len(),
            1,
            "context is retained as a trace frame"
        );
        assert!(matches!(
            &encoded,
            RuntimeValue::Opaque(value)
                if arc_error_owner().accepts_opaque_value(value)
        ));
    }

    #[test]
    fn option_none_context_is_a_missing_value_with_typed_content_and_no_cause() {
        let error =
            RuntimeArcError::missing_value(content_message(), RuntimeArcErrorFrame::empty())
                .expect("missing-value ArcError");
        let decoded = RuntimeArcError::try_from_runtime_value(&error.into_runtime_value())
            .expect("decode missing-value ArcError");

        assert_eq!(decoded.kind().as_str(), "MissingValue");
        assert!(decoded.source().is_none());
        assert_eq!(decoded.trace().frames().len(), 1);
        assert!(decoded.trace().frames()[0].message().is_some());
    }

    #[test]
    fn result_context_value_preserves_success_and_wraps_failure() {
        let success = RuntimeValue::Unit;
        let output = RuntimeArcError::context_result_value(
            RuntimeValue::result_ok(success.clone()),
            content_message(),
            RuntimeArcErrorFrame::empty(),
        )
        .expect("Result::Ok context pass-through");
        assert_eq!(
            output.try_into_builtin_variant_case(),
            Ok((RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(success)))
        );

        let cause = RuntimeValue::Opaque(crate::value::RuntimeOpaqueValue::new_exact(
            &runtime_standard_opaque_type(&["AssetError"])
                .and_then(|spec| spec.monomorphic_owner())
                .expect("AssetError owner"),
            RuntimeValue::String("missing".to_owned()),
        ));
        let message = content_message();
        let output = RuntimeArcError::context_result_value(
            RuntimeValue::result_err(cause.clone()),
            message.clone(),
            RuntimeArcErrorFrame::empty(),
        )
        .expect("Result::Err context conversion");
        let (case, Some(error)) = output
            .try_into_builtin_variant_case()
            .expect("Result output carrier")
        else {
            panic!("context failure returns Result::Err")
        };
        assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultErr);
        let error = RuntimeArcError::try_from_runtime_value(&error).expect("canonical ArcError");
        assert_eq!(error.message(), &message);
        assert!(matches!(
            error.source(),
            Some(RuntimeArcErrorSource::TypedValue(value)) if value == &cause
        ));
        assert_eq!(
            error.trace().frames()[0].message(),
            Some(&message),
            "the typed Content message appears on the appended context frame"
        );
    }

    #[test]
    fn option_context_value_preserves_some_and_turns_none_into_missing_value() {
        let success = RuntimeValue::String("route".to_owned());
        let output = RuntimeArcError::context_option_value(
            RuntimeValue::option_some(success.clone()),
            content_message(),
            RuntimeArcErrorFrame::empty(),
        )
        .expect("Option::Some context conversion");
        assert_eq!(
            output.try_into_builtin_variant_case(),
            Ok((RuntimeBuiltinVariantCaseIdentity::ResultOk, Some(success)))
        );

        let message = content_message();
        let output = RuntimeArcError::context_option_value(
            RuntimeValue::option_none(),
            message.clone(),
            RuntimeArcErrorFrame::empty(),
        )
        .expect("Option::None context conversion");
        let (case, Some(error)) = output
            .try_into_builtin_variant_case()
            .expect("Result output carrier")
        else {
            panic!("missing Option value returns Result::Err")
        };
        assert_eq!(case, RuntimeBuiltinVariantCaseIdentity::ResultErr);
        let error = RuntimeArcError::try_from_runtime_value(&error).expect("canonical ArcError");
        assert_eq!(error.kind().as_str(), "MissingValue");
        assert!(error.source().is_none());
        assert_eq!(error.message(), &message);
        assert_eq!(error.trace().frames()[0].message(), Some(&message));
    }

    #[test]
    fn lazy_context_message_factory_runs_only_for_error_or_absence() {
        let calls = std::cell::Cell::new(0);
        let limits = RuntimeSchemaLimits::engine_default();
        let frame = RuntimeArcErrorFrame::empty();
        let output = RuntimeArcError::context_result_value_with(
            RuntimeValue::result_ok(RuntimeValue::Unit),
            || {
                calls.set(calls.get() + 1);
                Ok(content_message())
            },
            frame.clone(),
            limits,
        )
        .expect("lazy Result::Ok context pass-through");
        assert_eq!(calls.get(), 0);
        assert_eq!(
            output.builtin_variant_case(),
            Some((
                RuntimeBuiltinVariantCaseIdentity::ResultOk,
                Some(&RuntimeValue::Unit)
            ))
        );

        let output = RuntimeArcError::context_option_value_with(
            RuntimeValue::option_some(RuntimeValue::Bool(true)),
            || {
                calls.set(calls.get() + 1);
                Ok(content_message())
            },
            frame.clone(),
            limits,
        )
        .expect("lazy Option::Some context pass-through");
        assert_eq!(calls.get(), 0);
        assert_eq!(
            output.builtin_variant_case(),
            Some((
                RuntimeBuiltinVariantCaseIdentity::ResultOk,
                Some(&RuntimeValue::Bool(true))
            ))
        );

        RuntimeArcError::context_result_value_with(
            RuntimeValue::result_err(RuntimeValue::String("failure".to_owned())),
            || {
                calls.set(calls.get() + 1);
                Ok(content_message())
            },
            frame.clone(),
            limits,
        )
        .expect("lazy Result::Err context");
        RuntimeArcError::context_option_value_with(
            RuntimeValue::option_none(),
            || {
                calls.set(calls.get() + 1);
                Ok(content_message())
            },
            frame,
            limits,
        )
        .expect("lazy Option::None context");
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn decoder_rejects_wrong_owner_and_enforces_shared_limits() {
        let foreign_owner = runtime_standard_opaque_type(&["AssetError"])
            .and_then(|spec| spec.monomorphic_owner())
            .expect("AssetError owner");
        let wrong_owner = foreign_owner
            .try_wrap(RuntimeValue::Unit)
            .expect("opaque value");
        assert_eq!(
            RuntimeArcError::try_from_runtime_value(&wrong_owner),
            Err(RuntimeArcErrorValueError::InvalidOwner)
        );

        let error = RuntimeArcError::context_from_result(
            RuntimeValue::String("cause".to_owned()),
            content_message(),
            RuntimeArcErrorFrame::empty(),
        )
        .expect("context ArcError");
        let mut limits = RuntimeSchemaLimits::engine_default();
        limits.max_encoded_bytes = 1;
        assert!(matches!(
            RuntimeArcError::try_from_runtime_value_with_limits(
                &error.into_runtime_value(),
                limits
            ),
            Err(RuntimeArcErrorValueError::InvalidRuntimeValue { .. })
        ));
    }
}
