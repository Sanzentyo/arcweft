//! Iterative canonical value traversal over borrowed logical storage views.

use std::{fmt, rc::Rc};

use crate::value::{
    RuntimeAgentAction, RuntimeAgentActionDispatch, RuntimeAgentCaptureTarget,
    RuntimeAgentCompareOp, RuntimeAgentKind, RuntimeAgentView, RuntimeCommandListView,
    RuntimeDenseNumericView, RuntimeIteratorKind, RuntimeIteratorView, RuntimeOpaquePersistence,
    RuntimeOpaqueValueClass, RuntimePredicateKind, RuntimePredicateListView, RuntimePredicateView,
    RuntimeRecordView, RuntimeSequenceView, RuntimeTupleView, RuntimeValue, RuntimeValueView,
};

use super::{
    CanonicalSink, CanonicalWriter, RuntimeSchemaChoiceMismatch, RuntimeSchemaError,
    value_budget::{ValidationWork, ValidationWorkState, ValueBudget},
};

struct DescriptorAdmission<'a, 'b, S: CanonicalSink + ?Sized, V: ValueValidation> {
    writer: &'a mut CanonicalWriter<'b, S>,
    budget: Option<&'a mut ValueBudget>,
    validation: &'a mut V,
}

impl<S: CanonicalSink + ?Sized, V: ValueValidation> crate::task::descriptor::TaskDescriptorVisitor
    for DescriptorAdmission<'_, '_, S, V>
{
    fn node(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        if let Some(budget) = &mut self.budget {
            budget.node(depth)?;
        }
        self.validation.embedded_node(depth)
    }
    fn fixed(&mut self, bytes: &[u8], depth: usize) -> Result<(), RuntimeSchemaError> {
        self.node(depth)?;
        self.writer.extend(bytes)
    }
    fn string(&mut self, text: &str, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.node(depth)?;
        if let Some(budget) = &self.budget {
            budget.string(text)?;
        }
        self.writer.string(text)
    }
    fn collection(&mut self, count: usize, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.node(depth)?;
        if let Some(budget) = &self.budget {
            budget.collection(count)?;
        }
        self.writer.len(count)
    }
    fn blob(&mut self, bytes: &[u8], depth: usize) -> Result<(), RuntimeSchemaError> {
        self.collection(bytes.len(), depth)?;
        self.writer.extend(bytes)
    }
}

pub(crate) enum ValueAdmission<C, A> {
    Children(C),
    Choice(A),
    Admitted,
}

/// An inert candidate is correlated with the selected original type authority;
/// a live candidate must already lease that exact immutable executable.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ValueAdmissionOrigin {
    Live,
    InertSnapshot,
}

impl ValueAdmissionOrigin {
    pub(crate) fn accepts_program(
        self,
        program: crate::program_types::RuntimeProgramTypes<'_>,
        lease: Option<&crate::task::RuntimeProgramOwner>,
    ) -> bool {
        match lease {
            Some(lease) => program.owns_lease(lease),
            None => matches!(self, Self::InertSnapshot),
        }
    }
}

pub(crate) trait ValueValidation {
    type Expected;
    type Children;
    type Alternatives: ExactSizeIterator<Item = Self::Expected>;

    fn preflight(
        &mut self,
        expected: &Self::Expected,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError>;
    fn alternative(&mut self, depth: usize) -> Result<(), RuntimeSchemaError>;
    /// Logical nodes embedded in predicate or dense storage have no expected
    /// child type, but still consume a snapshot resource allowance.
    fn embedded_node(&mut self, _depth: usize) -> Result<(), RuntimeSchemaError> {
        Ok(())
    }
    fn is_admitted(expected: &Self::Expected) -> bool;
    fn admitted_children() -> Self::Children;

    fn enter(
        &mut self,
        expected: Self::Expected,
        value: RuntimeValueView<'_>,
    ) -> Result<ValueAdmission<Self::Children, Self::Alternatives>, RuntimeSchemaError>;

    fn child(
        &mut self,
        children: &Self::Children,
        index: usize,
    ) -> Result<Self::Expected, RuntimeSchemaError>;

    // Stateful type relations keep speculative Choice obligations isolated.
    fn begin_choice(&mut self) {}
    fn begin_alternative(&mut self) {}
    fn finish_alternative(&mut self, _accepted: bool) {}
    fn finish_choice(&mut self, _admitted: bool) {}
}

/// Validates a persistent value and emits its canonical digest in one traversal.
pub(crate) fn validate_and_hash<V: ValueValidation>(
    value: &RuntimeValue,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<super::RuntimeValueDigest, RuntimeSchemaError> {
    validate_and_hash_view(value.view(), limits, validation, expected)
}

/// The same persistent grammar over an already borrowed live or inert graph.
pub(crate) fn validate_and_hash_view<V: ValueValidation>(
    value: RuntimeValueView<'_>,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<super::RuntimeValueDigest, RuntimeSchemaError> {
    let mut budget = ValueBudget::new(limits);
    let mut sink = super::CanonicalBlake3Sink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: limits.max_encoded_bytes,
        max_string_bytes: Some(limits.max_string_bytes),
    };
    visit(
        value,
        0,
        &mut writer,
        Some(&mut budget),
        validation,
        expected,
    )?;
    Ok(super::RuntimeValueDigest::from_bytes(sink.finish()))
}

/// Checks executable literals through the same type/value visitor. Runtime-only
/// literals (for example ranges) have no persistent digest to publish.
pub(crate) fn validate_literal<V: ValueValidation>(
    value: &RuntimeValue,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    validate_literal_view(value.view(), limits, validation, expected)
}

pub(crate) fn validate_literal_view<V: ValueValidation>(
    value: RuntimeValueView<'_>,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    validate_view_without_digest(value, limits, validation, expected, ValueEncoding::Literal)
}

/// Validates a live value without requiring a canonical persistence encoding.
pub(crate) fn validate_live<V: ValueValidation>(
    value: &RuntimeValue,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    validate_without_digest(value, limits, validation, expected, ValueEncoding::Live)
}

pub(crate) fn validate_live_view<V: ValueValidation>(
    value: RuntimeValueView<'_>,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    validate_view_without_digest(value, limits, validation, expected, ValueEncoding::Live)
}

/// Validates a decoded snapshot candidate before it becomes live.
pub(crate) fn validate_snapshot_view<V: ValueValidation>(
    value: RuntimeValueView<'_>,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    validate_view_without_digest(value, limits, validation, expected, ValueEncoding::Snapshot)
}

fn validate_without_digest<V: ValueValidation>(
    value: &RuntimeValue,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    validate_view_without_digest(value.view(), limits, validation, expected, encoding)
}

fn validate_view_without_digest<V: ValueValidation>(
    value: RuntimeValueView<'_>,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    validate_root_without_digest(
        Work::Value(value, 0, expected, None),
        limits,
        validation,
        encoding,
    )
}

fn validate_root_without_digest<V: ValueValidation>(
    root: Work<'_, V::Expected, V::Children, V::Alternatives>,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    let mut budget = ValueBudget::new(limits);
    let mut sink = super::CanonicalCountingSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: if matches!(encoding, ValueEncoding::Snapshot) {
            limits.max_encoded_bytes
        } else {
            u64::MAX
        },
        max_string_bytes: Some(limits.max_string_bytes),
    };
    visit_work(root, &mut writer, Some(&mut budget), validation, encoding)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ValueEncoding {
    Canonical,
    Literal,
    Live,
    Snapshot,
}

type ValueObserver<'a> =
    dyn for<'value> FnMut(RuntimeValueView<'value>) -> Result<(), RuntimeSchemaError> + 'a;

#[derive(Default)]
pub(crate) struct NoSchemaValidation<'a> {
    resources: Option<ValidationWork>,
    depth: usize,
    observer: Option<&'a mut ValueObserver<'a>>,
}

impl NoSchemaValidation<'_> {
    pub(crate) fn snapshot_resources(limits: super::RuntimeSchemaLimits) -> Self {
        Self {
            resources: Some(ValidationWork::new(limits)),
            depth: 0,
            observer: None,
        }
    }
}

impl ValueValidation for NoSchemaValidation<'_> {
    type Expected = ();
    type Children = ();
    type Alternatives = std::iter::Empty<()>;

    fn preflight(&mut self, (): &(), depth: usize) -> Result<(), RuntimeSchemaError> {
        self.depth = depth;
        if self.resources.is_some() && depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            return Err(crate::value::RuntimeValueNestingError::Exceeded {
                maximum: crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH,
            }
            .into());
        }
        self.alternative(depth)
    }
    fn alternative(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        if let Some(resources) = &mut self.resources {
            resources.charge(depth)?;
        }
        Ok(())
    }
    fn embedded_node(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        self.alternative(depth)
    }
    fn is_admitted((): &()) -> bool {
        true
    }
    fn admitted_children() {}

    fn enter(
        &mut self,
        (): (),
        value: RuntimeValueView<'_>,
    ) -> Result<ValueAdmission<(), Self::Alternatives>, RuntimeSchemaError> {
        if let Some(resources) = &mut self.resources {
            let policy = ValueBudget::new(resources.limits());
            let binding = match value {
                RuntimeValueView::NominalRecord(record) => record.type_instantiation(),
                RuntimeValueView::Variant(variant) => variant.type_instantiation(),
                _ => None,
            };
            if let Some(binding) = binding {
                binding.validate_resource_limits(self.depth + 1, &policy, resources)?;
            }
            if let RuntimeValueView::Sequence(sequence) = value {
                sequence.validate_storage_metadata(self.depth, &policy, resources)?;
            }
        }
        if let Some(observer) = &mut self.observer {
            observer(value)?;
        }
        Ok(ValueAdmission::Admitted)
    }
    fn child(&mut self, (): &(), _: usize) -> Result<(), RuntimeSchemaError> {
        Ok(())
    }
}

/// One resource pass over the complete live or inert logical grammar. The
/// remaining work allowance can be consumed by subsequent type obligations.
pub(crate) fn preflight_snapshot_view<'a>(
    value: RuntimeValueView<'_>,
    limits: super::RuntimeSchemaLimits,
    observer: Option<&'a mut ValueObserver<'a>>,
) -> Result<ValidationWork, RuntimeSchemaError> {
    let mut admission =
        ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::InertSnapshot);
    admission.validate_view(value, 0, observer)?;
    Ok(admission.work())
}

#[cfg(test)]
pub(crate) fn snapshot_view_bytes(
    value: RuntimeValueView<'_>,
) -> Result<Vec<u8>, RuntimeSchemaError> {
    let limits = super::RuntimeSchemaLimits::engine_default();
    let mut sink = super::CanonicalBytesSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: limits.max_encoded_bytes,
        max_string_bytes: Some(limits.max_string_bytes),
    };
    let mut budget = ValueBudget::new(limits);
    let mut validation = NoSchemaValidation {
        resources: None,
        depth: 0,
        observer: None,
    };
    visit_work(
        Work::Value(value, 0, (), None),
        &mut writer,
        Some(&mut budget),
        &mut validation,
        ValueEncoding::Snapshot,
    )?;
    Ok(sink.bytes)
}

pub(crate) fn validate_standalone_task_type(
    ty: &crate::pattern::RuntimeCheckedType,
    limits: super::RuntimeSchemaLimits,
) -> Result<(), RuntimeSchemaError> {
    let mut sink = super::CanonicalCountingSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: limits.max_encoded_bytes,
        max_string_bytes: Some(limits.max_string_bytes),
    };
    let mut budget = ValueBudget::new(limits);
    let mut validation = NoSchemaValidation::snapshot_resources(limits);
    ty.visit_standalone_descriptor(
        &mut DescriptorAdmission {
            writer: &mut writer,
            budget: Some(&mut budget),
            validation: &mut validation,
        },
        0,
    )
}

/// Visits already resource-admitted storage through the same logical grammar.
/// Type/owner obligations share its existing work allowance; no value quota is
/// refreshed and no live value or save DTO is constructed by this pass.
pub(crate) fn visit_admitted_storage<E: From<RuntimeSchemaError>>(
    value: RuntimeValueView<'_>,
    work: &ValidationWork,
    mut observe: impl for<'value> FnMut(RuntimeValueView<'value>) -> Result<(), E>,
) -> Result<(), E> {
    let mut first_error = None;
    let mut observer = |value: RuntimeValueView<'_>| {
        if let Err(error) = observe(value) {
            first_error = Some(error);
            return Err(RuntimeSchemaError::Encoding {
                message: "original storage admission rejected a retained node".into(),
            });
        }
        Ok(())
    };
    let mut sink = super::CanonicalCountingSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: u64::MAX,
        max_string_bytes: None,
    };
    let mut validation = NoSchemaValidation {
        resources: Some(work.clone()),
        depth: 0,
        observer: Some(&mut observer),
    };
    let result = visit_work(
        Work::Value(value, 0, (), None),
        &mut writer,
        None,
        &mut validation,
        ValueEncoding::Snapshot,
    );
    match first_error {
        Some(error) => Err(error),
        None => result.map_err(E::from),
    }
}

/// Applies the existing schema predicate to already admitted storage, without
/// refreshing any value quota. Its predicate owns the enclosing shared work.
pub(crate) fn validate_admitted_view<V: ValueValidation>(
    value: RuntimeValueView<'_>,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    let mut sink = super::CanonicalCountingSink::default();
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: u64::MAX,
        max_string_bytes: None,
    };
    visit_work(
        Work::Value(value, 0, expected, None),
        &mut writer,
        None,
        validation,
        ValueEncoding::Snapshot,
    )
}

/// Resource ownership for an admission containing several complete values and
/// partial places. Logical nodes, encoded complete-value bytes and relation
/// work accumulate across descendants; visiting another value grants no quota.
pub(crate) struct ValueResourceAdmission<Work = ValidationWork> {
    budget: ValueBudget,
    sink: super::CanonicalCountingSink,
    work: Work,
    origin: ValueAdmissionOrigin,
    encoding: ValueEncoding,
    program_storage_obligations: bool,
}

/// The same allowance between validation calls, with no shared local handle.
pub(crate) type ValueResourceAdmissionState = ValueResourceAdmission<ValidationWorkState>;

impl<Work> ValueResourceAdmission<Work> {
    fn map_work<Next>(self, map: impl FnOnce(Work) -> Next) -> ValueResourceAdmission<Next> {
        ValueResourceAdmission {
            budget: self.budget,
            sink: self.sink,
            work: map(self.work),
            origin: self.origin,
            encoding: self.encoding,
            program_storage_obligations: self.program_storage_obligations,
        }
    }
}

impl ValueResourceAdmissionState {
    pub(crate) fn resume(self) -> ValueResourceAdmission {
        self.map_work(ValidationWork::from_state)
    }
}

impl ValueResourceAdmission {
    pub(crate) fn function_instantiation(
        &mut self,
        binding: &crate::program_types::RuntimeFunctionEffectInstantiation,
    ) -> Result<(), RuntimeSchemaError> {
        binding.validate_resource_limits(0, &self.budget, &mut self.work)
    }
    pub(crate) fn try_into_state(self) -> Result<ValueResourceAdmissionState, Self> {
        let Self {
            budget,
            sink,
            work,
            origin,
            encoding,
            program_storage_obligations,
        } = self;
        match work.try_into_state() {
            Ok(work) => Ok(ValueResourceAdmission {
                budget,
                sink,
                work,
                origin,
                encoding,
                program_storage_obligations,
            }),
            Err(work) => Err(Self {
                budget,
                sink,
                work,
                origin,
                encoding,
                program_storage_obligations,
            }),
        }
    }

    pub(crate) fn live(limits: super::RuntimeSchemaLimits) -> Self {
        Self::new(limits, ValueAdmissionOrigin::Live, ValueEncoding::Live)
    }

    pub(crate) fn snapshot(
        limits: super::RuntimeSchemaLimits,
        origin: ValueAdmissionOrigin,
    ) -> Self {
        Self::new(limits, origin, ValueEncoding::Snapshot)
    }

    fn new(
        limits: super::RuntimeSchemaLimits,
        origin: ValueAdmissionOrigin,
        encoding: ValueEncoding,
    ) -> Self {
        Self {
            budget: ValueBudget::new(limits),
            sink: super::CanonicalCountingSink::default(),
            work: ValidationWork::new(limits),
            origin,
            encoding,
            program_storage_obligations: false,
        }
    }

    pub(crate) fn work(&self) -> ValidationWork {
        self.work.clone()
    }

    pub(crate) fn origin(&self) -> ValueAdmissionOrigin {
        self.origin
    }

    /// Selects the provenance of the next borrowed root in a mixed retained
    /// admission. Counters and sticky work failures remain the same allowance.
    pub(crate) fn set_origin(&mut self, origin: ValueAdmissionOrigin) {
        self.origin = origin;
    }

    pub(crate) fn has_program_storage_obligations(&self) -> bool {
        self.program_storage_obligations
    }

    pub(crate) fn validate_task_descriptor<P>(
        &mut self,
        spec: &crate::task::TaskSpec<P>,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        self.admit_descriptor(|visitor| spec.visit_descriptor(visitor, depth))
    }

    pub(crate) fn validate_standalone_outcome_type(
        &mut self,
        ty: &crate::pattern::RuntimeCheckedType,
    ) -> Result<(), RuntimeSchemaError> {
        self.admit_descriptor(|visitor| ty.visit_standalone_descriptor(visitor, 0))
    }

    fn admit_descriptor(
        &mut self,
        visit: impl FnOnce(
            &mut DescriptorAdmission<'_, '_, super::CanonicalCountingSink, NoSchemaValidation<'_>>,
        ) -> Result<(), RuntimeSchemaError>,
    ) -> Result<(), RuntimeSchemaError> {
        let limits = self.work.limits();
        let mut writer = CanonicalWriter {
            sink: &mut self.sink,
            max_encoded_bytes: limits.max_encoded_bytes,
            max_string_bytes: Some(limits.max_string_bytes),
        };
        let mut validation = NoSchemaValidation {
            resources: Some(self.work.clone()),
            depth: 0,
            observer: None,
        };
        let result = visit(&mut DescriptorAdmission {
            writer: &mut writer,
            budget: Some(&mut self.budget),
            validation: &mut validation,
        });
        if let Err(error) = &result {
            self.work.retain_failure(error.clone());
        }
        result
    }
    pub(crate) fn validate_view<'a>(
        &mut self,
        value: RuntimeValueView<'_>,
        depth: usize,
        mut observer: Option<&'a mut ValueObserver<'a>>,
    ) -> Result<(), RuntimeSchemaError> {
        let limits = self.work.limits();
        let encoding = self.encoding;
        let mut writer = CanonicalWriter {
            sink: &mut self.sink,
            max_encoded_bytes: if matches!(encoding, ValueEncoding::Snapshot) {
                limits.max_encoded_bytes
            } else {
                u64::MAX
            },
            max_string_bytes: Some(limits.max_string_bytes),
        };
        let mut program_storage = false;
        let mut inspect = |value: RuntimeValueView<'_>| {
            program_storage |= value.requires_program_storage_admission();
            match &mut observer {
                Some(observer) => observer(value),
                None => Ok(()),
            }
        };
        let mut validation = NoSchemaValidation {
            resources: Some(self.work.clone()),
            depth: 0,
            observer: Some(&mut inspect),
        };
        let result = visit_work(
            Work::Value(value, depth, (), None),
            &mut writer,
            Some(&mut self.budget),
            &mut validation,
            encoding,
        );
        self.program_storage_obligations |= program_storage;
        if let Err(error) = &result {
            self.work.retain_failure(error.clone());
        }
        result
    }

    pub(crate) fn vacant(&mut self, depth: usize) -> Result<(), RuntimeSchemaError> {
        if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
            return Err(crate::value::RuntimeValueNestingError::Exceeded {
                maximum: crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH,
            }
            .into());
        }
        self.work.charge(depth)
    }

    /// Bounds retained metadata before it is cloned without inventing a value
    /// node or value-grammar byte. The enclosing frame codec owns its wire bytes.
    pub(crate) fn metadata_string(&mut self, text: &str) -> Result<(), RuntimeSchemaError> {
        let result = self.work.charge(0).and_then(|()| self.budget.string(text));
        if let Err(error) = &result {
            self.work.retain_failure(error.clone());
        }
        result
    }

    /// Counts an enclosing owner's actual canonical metadata header in the
    /// same byte bank as its value payloads, without fabricating value nodes.
    pub(crate) fn encoded_metadata_string(&mut self, text: &str) -> Result<(), RuntimeSchemaError> {
        self.metadata_string(text)?;
        self.encode_metadata(|writer| writer.string(text))
    }

    pub(crate) fn encoded_metadata_count(
        &mut self,
        count: usize,
    ) -> Result<(), RuntimeSchemaError> {
        self.work.collection(count)?;
        self.work.charge(0)?;
        self.encode_metadata(|writer| writer.len(count))
    }

    pub(crate) fn encoded_metadata_fixed(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), RuntimeSchemaError> {
        self.work.charge(0)?;
        self.encode_metadata(|writer| writer.extend(bytes))
    }

    fn encode_metadata(
        &mut self,
        encode: impl FnOnce(
            &mut CanonicalWriter<'_, super::CanonicalCountingSink>,
        ) -> Result<(), RuntimeSchemaError>,
    ) -> Result<(), RuntimeSchemaError> {
        let limits = self.work.limits();
        let result = encode(&mut CanonicalWriter {
            sink: &mut self.sink,
            max_encoded_bytes: limits.max_encoded_bytes,
            max_string_bytes: Some(limits.max_string_bytes),
        });
        if let Err(error) = &result {
            self.work.retain_failure(error.clone());
        }
        result
    }

    pub(crate) fn partial_record(
        &mut self,
        header: &crate::value::RuntimePlaceRecordHeader,
        count: usize,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        self.vacant(depth)?;
        self.budget.node(depth)?;
        self.budget.collection(count)?;
        if let crate::value::RuntimePlaceRecordHeader::Structural { names } = header {
            self.budget.collection(names.len())?;
            for _ in names {
                self.work.charge(depth + 1)?;
            }
        }
        self.partial_record_header_fields(header, depth)
    }

    /// Place headers are metadata, so their traversal uses the enclosing
    /// schema depth and work limits rather than complete-value nesting or
    /// sequence limits. Retained record nodes still share the value allowance.
    pub(crate) fn partial_record_header(
        &mut self,
        header: &crate::value::RuntimePlaceRecordHeader,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        self.budget.node(depth)?;
        self.partial_record_header_fields(header, depth)
    }

    fn partial_record_header_fields(
        &mut self,
        header: &crate::value::RuntimePlaceRecordHeader,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        match header {
            crate::value::RuntimePlaceRecordHeader::Nominal {
                nominal,
                type_instantiation,
                ..
            } => {
                self.budget.string(nominal.as_str())?;
                if let Some(binding) = type_instantiation {
                    binding.validate_resource_limits(depth + 1, &self.budget, &mut self.work)?;
                }
            }
            crate::value::RuntimePlaceRecordHeader::Structural { names } => {
                for name in names {
                    self.budget.string(name)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum PathStep<'a> {
    Index(usize),
    Name(&'a str),
}

#[derive(Default)]
struct ValuePath<'a>(Vec<PathStep<'a>>);

impl fmt::Display for ValuePath<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("$")?;
        for step in &self.0 {
            match step {
                PathStep::Index(index) => write!(formatter, "[{index}]")?,
                PathStep::Name(name) => write!(formatter, ".{name}")?,
            }
        }
        Ok(())
    }
}

impl RuntimeSchemaError {
    fn at_value_path(mut self, current: &ValuePath<'_>) -> Self {
        if let Some(path) = self.value_path_mut() {
            *path = current.to_string();
        }
        self
    }

    pub(crate) fn within_value_path(mut self, parent: &str) -> Self {
        if let Some(path) = self.value_path_mut() {
            *path = format!("{parent}{}", path.strip_prefix('$').unwrap_or(path));
        }
        self
    }

    fn value_path_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Arity { path, .. }
            | Self::ArrayLength { path, .. }
            | Self::RecordField { path, .. }
            | Self::OpaqueOwner { path }
            | Self::Type { path, .. }
            | Self::NonFinite { path, .. }
            | Self::MissingField { path, .. }
            | Self::UnknownField { path, .. }
            | Self::UnknownVariant { path, .. }
            | Self::VariantPayload { path }
            | Self::UnresolvedNamed { path, .. }
            | Self::NominalIdentity { path, .. }
            | Self::NominalSemanticIdentity { path, .. }
            | Self::NominalLayout { path }
            | Self::BuiltinVariantOwner { path, .. }
            | Self::ChoiceNoMatch { path, .. }
            | Self::ChoiceAmbiguous { path, .. }
            | Self::ValidationWork { path, .. }
            | Self::ValidationDepth { path, .. } => Some(path),
            Self::ValueNesting(_)
            | Self::BudgetExceeded { .. }
            | Self::Encoding { .. }
            | Self::TaskSemantic { .. }
            | Self::SchemaEncodingOverflow
            | Self::NominalGraph { .. }
            | Self::NominalGraphRequired { .. }
            | Self::UnresolvedNominal { .. } => None,
        }
    }

    pub(crate) fn is_choice_mismatch(&self) -> bool {
        matches!(
            self,
            Self::Arity { .. }
                | Self::ArrayLength { .. }
                | Self::RecordField { .. }
                | Self::OpaqueOwner { .. }
                | Self::Type { .. }
                | Self::MissingField { .. }
                | Self::UnknownField { .. }
                | Self::UnknownVariant { .. }
                | Self::VariantPayload { .. }
                | Self::NominalIdentity { .. }
                | Self::NominalSemanticIdentity { .. }
                | Self::NominalLayout { .. }
                | Self::BuiltinVariantOwner { .. }
                | Self::ChoiceNoMatch { .. }
                | Self::ChoiceAmbiguous { .. }
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitMode {
    Output,
    Probe,
}

/// Candidate visits reuse the traversal and scalar rules without publishing
/// bytes. The chosen value is emitted once after every candidate has finished.
enum VisitOutput<'a, S: CanonicalSink + ?Sized> {
    Output(&'a mut S),
    Probe,
}

impl<S: CanonicalSink + ?Sized> CanonicalSink for VisitOutput<'_, S> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RuntimeSchemaError> {
        match self {
            Self::Output(sink) => sink.write(bytes),
            Self::Probe => Ok(()),
        }
    }
    fn bytes_written(&self) -> u64 {
        match self {
            Self::Output(sink) => sink.bytes_written(),
            Self::Probe => 0,
        }
    }
}

struct ChoiceVisit<'a, A: Iterator> {
    value: RuntimeValueView<'a>,
    depth: usize,
    alternatives: std::iter::Enumerate<A>,
    mismatches: Vec<RuntimeSchemaChoiceMismatch>,
    first: Option<u32>,
    second: Option<u32>,
    parent_mode: VisitMode,
    path_length: usize,
}

enum Work<'a, E, C, A: Iterator<Item = E>> {
    PopPath,
    Value(RuntimeValueView<'a>, usize, E, Option<PathStep<'a>>),
    AdmittedValue(RuntimeValueView<'a>, usize),
    ChoiceNext(ChoiceVisit<'a, A>),
    ChoiceComplete(ChoiceVisit<'a, A>, u32),
    Tuple(RuntimeTupleView<'a>, usize, usize, Rc<C>),
    Record(RuntimeRecordView<'a>, usize, usize, Rc<C>),
    Sequence(RuntimeSequenceView<'a>, usize, usize, Rc<C>),
    BorrowedValues(
        Box<dyn Iterator<Item = RuntimeValueView<'a>> + 'a>,
        usize,
        usize,
        Rc<C>,
    ),
    CommandList(RuntimeCommandListView<'a>, usize, Rc<C>),
    Commands(RuntimeCommandListView<'a>, usize, usize, Rc<C>),
    Predicate(RuntimePredicateView<'a>, usize, Rc<C>, PathStep<'a>),
    Predicates(RuntimePredicateListView<'a>, usize, usize, Rc<C>),
}

pub(super) fn visit<S: CanonicalSink + ?Sized, V: ValueValidation>(
    root: RuntimeValueView<'_>,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    budget: Option<&mut ValueBudget>,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    visit_with_encoding(
        root,
        depth,
        writer,
        budget,
        validation,
        expected,
        ValueEncoding::Canonical,
    )
}

fn visit_with_encoding<S: CanonicalSink + ?Sized, V: ValueValidation>(
    root: RuntimeValueView<'_>,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    budget: Option<&mut ValueBudget>,
    validation: &mut V,
    expected: V::Expected,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    visit_work(
        Work::Value(root, depth, expected, None),
        writer,
        budget,
        validation,
        encoding,
    )
}

fn visit_work<S: CanonicalSink + ?Sized, V: ValueValidation>(
    root: Work<'_, V::Expected, V::Children, V::Alternatives>,
    writer: &mut CanonicalWriter<'_, S>,
    mut budget: Option<&mut ValueBudget>,
    validation: &mut V,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    let mut path = ValuePath::default();
    let mut work = vec![root];
    let mut mode = VisitMode::Output;
    let mut active_choices = Vec::new();
    let mut choice_active_alternatives = Vec::new();
    while let Some(next) = work.pop() {
        let mut target = match mode {
            VisitMode::Output => VisitOutput::Output(&mut *writer.sink),
            VisitMode::Probe => VisitOutput::Probe,
        };
        let mut active_writer = CanonicalWriter {
            sink: &mut target,
            max_encoded_bytes: if mode == VisitMode::Output {
                writer.max_encoded_bytes
            } else {
                u64::MAX
            },
            max_string_bytes: writer.max_string_bytes,
        };
        let writer = &mut active_writer;
        let result = (|| {
            match next {
                Work::PopPath => {
                    path.0.pop();
                }
                Work::ChoiceNext(mut choice) => {
                    if let Some((index, expected)) = choice.alternatives.next() {
                        let ordinal = u32::try_from(index).map_err(|_| {
                            RuntimeSchemaError::BudgetExceeded {
                                budget: "sequence_items",
                            }
                        })?;
                        validation.alternative(choice.depth + 1)?;
                        validation.begin_alternative();
                        *choice_active_alternatives
                            .last_mut()
                            .expect("active Choice owns its checkpoint") = true;
                        let value = choice.value;
                        let depth = choice.depth + 1;
                        active_choices.push(work.len());
                        work.push(Work::ChoiceComplete(choice, ordinal));
                        mode = VisitMode::Probe;
                        work.push(Work::Value(value, depth, expected, None));
                    } else {
                        mode = choice.parent_mode;
                        validation.finish_choice(choice.first.is_some() && choice.second.is_none());
                        let finished = choice_active_alternatives.pop();
                        debug_assert_eq!(finished, Some(false));
                        match (choice.first, choice.second) {
                            (None, _) => {
                                return Err(RuntimeSchemaError::ChoiceNoMatch {
                                    path: path.to_string(),
                                    branches: choice.mismatches.into_boxed_slice(),
                                });
                            }
                            (Some(first), Some(second)) => {
                                return Err(RuntimeSchemaError::ChoiceAmbiguous {
                                    path: path.to_string(),
                                    first,
                                    second,
                                });
                            }
                            (Some(_), None) => {
                                if mode == VisitMode::Output {
                                    work.push(Work::AdmittedValue(choice.value, choice.depth));
                                }
                            }
                        }
                    }
                }
                Work::ChoiceComplete(mut choice, ordinal) => {
                    validation.finish_alternative(true);
                    *choice_active_alternatives
                        .last_mut()
                        .expect("active Choice owns its checkpoint") = false;
                    let completed_boundary = active_choices.pop();
                    debug_assert_eq!(completed_boundary, Some(work.len()));
                    if choice.first.is_none() {
                        choice.first = Some(ordinal);
                    } else if choice.second.is_none() {
                        choice.second = Some(ordinal);
                    }
                    work.push(Work::ChoiceNext(choice));
                }
                Work::AdmittedValue(value, depth) => {
                    value_prefix(
                        value,
                        depth,
                        writer,
                        budget.as_deref_mut(),
                        &mut work,
                        validation,
                        Rc::new(V::admitted_children()),
                        encoding,
                    )?;
                }
                Work::Tuple(tuple, index, depth, children) => {
                    if let Some(value) = tuple.get(index) {
                        let expected = validation.child(&children, index)?;
                        work.push(Work::Tuple(tuple, index + 1, depth, children));
                        work.push(Work::Value(
                            value,
                            depth,
                            expected,
                            Some(PathStep::Index(index)),
                        ));
                    } else if index < tuple.len() {
                        return Err(RuntimeSchemaError::Encoding {
                            message: "logical tuple is missing an indexed child".to_owned(),
                        });
                    }
                }
                Work::Record(record, index, depth, children) => {
                    if let Some((field, name, value)) = record.get(index) {
                        writer.var_u32(field.get().get())?;
                        writer.string(name)?;
                        let expected = validation.child(&children, index)?;
                        work.push(Work::Record(record, index + 1, depth, children));
                        work.push(Work::Value(
                            value,
                            depth,
                            expected,
                            Some(PathStep::Name(name)),
                        ));
                    } else if index < record.len() {
                        return Err(RuntimeSchemaError::Encoding {
                            message: "logical record is missing an indexed child".to_owned(),
                        });
                    }
                }
                Work::Sequence(sequence, index, depth, children) => {
                    if let Some(value) = sequence.value_view(index) {
                        let expected = validation.child(&children, index)?;
                        work.push(Work::Sequence(sequence, index + 1, depth, children));
                        work.push(Work::Value(
                            value,
                            depth,
                            expected,
                            Some(PathStep::Index(index)),
                        ));
                    } else if index < sequence.len()? {
                        return Err(RuntimeSchemaError::Encoding {
                            message: "logical sequence is missing an indexed row".to_owned(),
                        });
                    }
                }
                Work::BorrowedValues(mut values, index, depth, children) => {
                    if let Some(value) = values.next() {
                        let expected = validation.child(&children, index)?;
                        work.push(Work::BorrowedValues(values, index + 1, depth, children));
                        work.push(Work::Value(
                            value,
                            depth,
                            expected,
                            Some(PathStep::Index(index)),
                        ));
                    }
                }
                Work::CommandList(commands, depth, children) => {
                    writer.len(commands.len())?;
                    work.push(Work::Commands(commands, 1, depth, children));
                }
                Work::Commands(commands, index, depth, children) => {
                    if let Some(command) = commands.get(index - 1) {
                        if mode == VisitMode::Output
                            && let Some(budget) = budget.as_deref_mut()
                        {
                            budget.node(depth + 1)?;
                        }
                        writer.string(command.constructor().as_str())?;
                        writer.string(command.target().as_str())?;
                        let expected = validation.child(&children, index)?;
                        work.push(Work::Commands(commands, index + 1, depth, children));
                        work.push(Work::Value(
                            command.payload(),
                            depth + 2,
                            expected,
                            Some(PathStep::Index(index - 1)),
                        ));
                    }
                }
                Work::Predicate(predicate, depth, children, step) => {
                    if encoding == ValueEncoding::Snapshot
                        && depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH
                    {
                        return Err(crate::value::RuntimeValueNestingError::Exceeded {
                            maximum: crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH,
                        }
                        .into());
                    }
                    path.0.push(step);
                    work.push(Work::PopPath);
                    if mode == VisitMode::Output
                        && let Some(budget) = budget.as_deref_mut()
                    {
                        budget.node(depth)?;
                    }
                    validation.embedded_node(depth)?;
                    predicate_prefix(
                        predicate,
                        depth,
                        writer,
                        budget.as_deref(),
                        &mut work,
                        validation,
                        children,
                    )?;
                }
                Work::Predicates(predicates, index, depth, children) => {
                    if let Some(predicate) = predicates.get(index) {
                        work.push(Work::Predicates(
                            predicates,
                            index + 1,
                            depth,
                            Rc::clone(&children),
                        ));
                        work.push(Work::Predicate(
                            predicate,
                            depth,
                            children,
                            PathStep::Index(index),
                        ));
                    }
                }
                Work::Value(value, depth, expected, step) => {
                    if let Some(step) = step {
                        path.0.push(step);
                        work.push(Work::PopPath);
                    }
                    if mode == VisitMode::Probe && V::is_admitted(&expected) {
                        return Ok(());
                    }
                    if mode == VisitMode::Output
                        && let Some(budget) = budget.as_deref_mut()
                    {
                        budget.value(value, depth)?;
                    }
                    validation.preflight(&expected, depth)?;
                    if mode == VisitMode::Probe
                        && let Some(budget) = budget.as_deref()
                    {
                        budget.shape(value)?;
                    }
                    match validation.enter(expected, value)? {
                        ValueAdmission::Children(children) => value_prefix(
                            value,
                            depth,
                            writer,
                            if mode == VisitMode::Output {
                                budget.as_deref_mut()
                            } else {
                                None
                            },
                            &mut work,
                            validation,
                            Rc::new(children),
                            encoding,
                        )?,
                        ValueAdmission::Admitted => value_prefix(
                            value,
                            depth,
                            writer,
                            if mode == VisitMode::Output {
                                budget.as_deref_mut()
                            } else {
                                None
                            },
                            &mut work,
                            validation,
                            Rc::new(V::admitted_children()),
                            encoding,
                        )?,
                        ValueAdmission::Choice(alternatives) => {
                            validation.begin_choice();
                            choice_active_alternatives.push(false);
                            work.push(Work::ChoiceNext(ChoiceVisit {
                                value,
                                depth,
                                alternatives: alternatives.enumerate(),
                                mismatches: Vec::new(),
                                first: None,
                                second: None,
                                parent_mode: mode,
                                path_length: path.0.len(),
                            }));
                        }
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            let error = error.at_value_path(&path);
            if !error.is_choice_mismatch() {
                while let Some(active) = choice_active_alternatives.pop() {
                    if active {
                        validation.finish_alternative(false);
                    }
                    validation.finish_choice(false);
                }
                return Err(error);
            }
            let Some(boundary) = active_choices.pop() else {
                return Err(error);
            };
            work.truncate(boundary + 1);
            let Some(Work::ChoiceComplete(mut choice, alternative)) = work.pop() else {
                unreachable!("active Choice owns its continuation");
            };
            path.0.truncate(choice.path_length);
            validation.finish_alternative(false);
            *choice_active_alternatives
                .last_mut()
                .expect("active Choice owns its checkpoint") = false;
            choice
                .mismatches
                .push(RuntimeSchemaChoiceMismatch::new(alternative, error));
            work.push(Work::ChoiceNext(choice));
        }
    }
    Ok(())
}

fn value_prefix<'a, S: CanonicalSink + ?Sized, V: ValueValidation>(
    value: RuntimeValueView<'a>,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    mut budget: Option<&mut ValueBudget>,
    work: &mut Vec<Work<'a, V::Expected, V::Children, V::Alternatives>>,
    validation: &mut V,
    children: Rc<V::Children>,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    match value {
        RuntimeValueView::Scalar(value) => {
            writer.scalar_for_encoding(value, encoding)?;
        }
        RuntimeValueView::Tuple(tuple) => {
            writer.u8(11)?;
            writer.len(tuple.len())?;
            work.push(Work::Tuple(tuple, 0, depth + 1, children));
        }
        RuntimeValueView::Sequence(sequence) => {
            writer.u8(12)?;
            writer.len(sequence.len()?)?;
            work.push(Work::Sequence(sequence, 0, depth + 1, children));
        }
        RuntimeValueView::Record(record) => {
            writer.u8(13)?;
            writer.len(record.len())?;
            work.push(Work::Record(record, 0, depth + 1, children));
        }
        RuntimeValueView::NominalRecord(record) => {
            writer.u8(15)?;
            writer.string(record.type_id().as_str())?;
            writer.extend(record.semantic_identity().as_bytes())?;
            writer.extend(record.layout().as_bytes())?;
            writer.len(record.fields().len())?;
            work.push(Work::Tuple(record.fields(), 0, depth + 1, children));
        }
        RuntimeValueView::Opaque(value) => {
            let affine = matches!(
                value.value_class(),
                RuntimeOpaqueValueClass::AffineHandle(_)
            );
            let rejected = match encoding {
                ValueEncoding::Canonical | ValueEncoding::Literal => {
                    value.persistence() == RuntimeOpaquePersistence::SnapshotOnly || affine
                }
                ValueEncoding::Snapshot | ValueEncoding::Live => false,
            };
            if rejected {
                return Err(RuntimeSchemaError::Encoding {
                    message: match encoding {
                        ValueEncoding::Canonical | ValueEncoding::Literal => {
                            "opaque value class/persistence is not constant-admissible"
                        }
                        ValueEncoding::Snapshot | ValueEncoding::Live => {
                            "opaque value class/persistence is not admissible"
                        }
                    }
                    .to_owned(),
                });
            }
            writer.u8(16)?;
            writer.string(value.producer().as_str())?;
            writer.extend(value.semantic_identity().as_bytes())?;
            writer.u8(value.value_class().semantic_tag())?;
            writer.u8(value.persistence().semantic_tag())?;
            work.push(Work::Value(
                value.payload(),
                depth + 1,
                validation.child(&children, 0)?,
                Some(PathStep::Name("payload")),
            ));
        }
        RuntimeValueView::Reduction(value) => {
            writer.u8(18)?;
            writer.string(value.owner().producer().as_str())?;
            writer.extend(value.owner().semantic_identity().as_bytes())?;
            let expected = validation.child(&children, 0)?;
            work.push(Work::CommandList(value.commands(), depth, children));
            work.push(Work::Value(
                value.state(),
                depth + 1,
                expected,
                Some(PathStep::Name("state")),
            ));
        }
        RuntimeValueView::Agent(value) => {
            if matches!(value.kind(), RuntimeAgentKind::DataShape(_))
                && matches!(encoding, ValueEncoding::Canonical | ValueEncoding::Literal)
            {
                return Err(RuntimeSchemaError::Encoding {
                    message:
                        "DataShape witnesses require a selected program and program-bound snapshot"
                            .to_owned(),
                });
            }
            writer.u8(17)?;
            agent_prefix(
                value,
                depth,
                writer,
                budget.as_deref(),
                work,
                validation,
                children,
            )?;
        }
        RuntimeValueView::Variant(variant) => {
            writer.u8(14)?;
            writer.variant_identity(variant.owner())?;
            writer.var_u32(variant.ordinal())?;
            writer.string(variant.name())?;
            writer.u8(u8::from(variant.payload().is_some()))?;
            if let Some(payload) = variant.payload() {
                work.push(Work::Value(
                    payload,
                    depth + 1,
                    validation.child(&children, 0)?,
                    Some(PathStep::Name(variant.name())),
                ));
            }
        }
        RuntimeValueView::DenseNumeric(value) => {
            writer.dense_numeric(value, budget, depth, encoding, validation)?;
        }
        RuntimeValueView::Need(handle) if !matches!(encoding, ValueEncoding::Literal) => {
            writer.u8(20)?;
            if matches!(encoding, ValueEncoding::Live | ValueEncoding::Snapshot) {
                handle.visit_descriptor(
                    &mut DescriptorAdmission {
                        writer,
                        budget: budget.as_deref_mut(),
                        validation,
                    },
                    depth + 1,
                )?;
                if let Some(budget) = budget {
                    budget.collection(handle.request_values().count())?;
                }
                // The producer contract owns request types; this traversal
                // charges their complete resource graph without reinterpreting
                // request arguments as children of the Need outcome type.
                work.push(Work::BorrowedValues(
                    handle.request_values(),
                    0,
                    depth + 1,
                    Rc::new(V::admitted_children()),
                ));
            } else {
                writer.extend(handle.need_id().as_bytes())?;
            }
        }
        RuntimeValueView::Callable(callable)
            if matches!(encoding, ValueEncoding::Live | ValueEncoding::Snapshot) =>
        {
            writer.u8(26)?;
            writer.var_u32(callable.state().get().get())?;
            writer.len(callable.retained().len())?;
            if let Some(budget) = budget {
                budget.collection(callable.retained().len())?;
            }
            work.push(Work::Tuple(callable.retained(), 0, depth + 1, children));
        }
        RuntimeValueView::Range(range)
            if matches!(encoding, ValueEncoding::Live | ValueEncoding::Snapshot) =>
        {
            writer.runtime_range(range)?;
        }
        RuntimeValueView::Iterator(iterator)
            if matches!(encoding, ValueEncoding::Live | ValueEncoding::Snapshot) =>
        {
            iterator_prefix(
                iterator,
                depth,
                writer,
                budget.as_deref(),
                work,
                validation,
                children,
            )?;
        }
        RuntimeValueView::Iterator(iterator) if matches!(encoding, ValueEncoding::Literal) => {
            match iterator.kind() {
                RuntimeIteratorKind::Values(items) => {
                    if let Some(budget) = budget {
                        budget.collection(items.len())?;
                    }
                    work.push(Work::Tuple(items, 0, depth + 1, children));
                }
                RuntimeIteratorKind::Range(_) => {}
                RuntimeIteratorKind::Witness(_) => {
                    return Err(RuntimeSchemaError::Encoding {
                        message: "iterator witnesses are not static literals".to_owned(),
                    });
                }
            }
        }
        RuntimeValueView::Need(_) | RuntimeValueView::Callable(_)
            if matches!(encoding, ValueEncoding::Literal) =>
        {
            return Err(RuntimeSchemaError::Encoding {
                message: "program-owned runtime values are not static literals".to_owned(),
            });
        }
        RuntimeValueView::Range(_)
        | RuntimeValueView::Iterator(_)
        | RuntimeValueView::Need(_)
        | RuntimeValueView::Callable(_)
            if matches!(encoding, ValueEncoding::Literal | ValueEncoding::Live) => {}
        RuntimeValueView::Range(_)
        | RuntimeValueView::Iterator(_)
        | RuntimeValueView::Need(_)
        | RuntimeValueView::Callable(_) => {
            return Err(RuntimeSchemaError::Encoding {
                message: "runtime-only value has no replay/save encoding".to_owned(),
            });
        }
    }
    Ok(())
}

fn iterator_prefix<'a, S: CanonicalSink + ?Sized, V: ValueValidation>(
    iterator: RuntimeIteratorView<'a>,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    budget: Option<&ValueBudget>,
    work: &mut Vec<Work<'a, V::Expected, V::Children, V::Alternatives>>,
    validation: &mut V,
    children: Rc<V::Children>,
) -> Result<(), RuntimeSchemaError> {
    match iterator.kind() {
        RuntimeIteratorKind::Range(range) => writer.runtime_range_iterator(range)?,
        RuntimeIteratorKind::Values(items) => {
            writer.u8(24)?;
            writer.len(items.len())?;
            if let Some(budget) = budget {
                budget.collection(items.len())?;
            }
            work.push(Work::Tuple(items, 0, depth + 1, children));
        }
        RuntimeIteratorKind::Witness(witness) => {
            writer.u8(25)?;
            writer.len(witness.next_method().0)?;
            work.push(Work::Value(
                witness.state(),
                depth + 1,
                validation.child(&children, 0)?,
                None,
            ));
        }
    }
    Ok(())
}

fn agent_prefix<'a, S: CanonicalSink + ?Sized, V: ValueValidation>(
    value: RuntimeAgentView<'a>,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    budget: Option<&ValueBudget>,
    work: &mut Vec<Work<'a, V::Expected, V::Children, V::Alternatives>>,
    validation: &mut V,
    children: Rc<V::Children>,
) -> Result<(), RuntimeSchemaError> {
    match value.kind() {
        RuntimeAgentKind::ActionTarget(target) => {
            writer.u8(0)?;
            writer.string(target.id().as_str())?;
            writer.string(target.target().as_str())?;
            writer.u8(match target.action() {
                RuntimeAgentAction::AdvanceText => 0,
                RuntimeAgentAction::SelectChoice => 1,
                RuntimeAgentAction::Invoke => 2,
                RuntimeAgentAction::Scroll => 3,
                RuntimeAgentAction::PointerClick => 4,
            })?;
            writer.u8(match target.dispatch() {
                RuntimeAgentActionDispatch::Semantic => 0,
                RuntimeAgentActionDispatch::Physical => 1,
            })?;
            writer.u8(u8::from(target.enabled()))?;
        }
        RuntimeAgentKind::CaptureTarget(target) => {
            writer.u8(1)?;
            match target {
                RuntimeAgentCaptureTarget::Viewport => writer.u8(0)?,
                RuntimeAgentCaptureTarget::Layer { target } => {
                    writer.u8(1)?;
                    writer.string(target.as_str())?;
                }
                RuntimeAgentCaptureTarget::Object { target } => {
                    writer.u8(2)?;
                    writer.string(target.as_str())?;
                }
            }
        }
        RuntimeAgentKind::DebugStatePath(path) => {
            writer.u8(2)?;
            writer.string(path.as_str())?;
        }
        RuntimeAgentKind::ObservationFieldPath(path) => {
            writer.u8(3)?;
            writer.string(path.as_str())?;
        }
        RuntimeAgentKind::Probe(probe) => {
            writer.u8(4)?;
            writer.agent_probe(probe)?;
        }
        RuntimeAgentKind::Diagnostics => writer.u8(5)?,
        RuntimeAgentKind::Predicate(predicate) => {
            writer.u8(6)?;
            // The root predicate is the Agent value's already charged node.
            validation.embedded_node(depth)?;
            predicate_prefix(predicate, depth, writer, budget, work, validation, children)?;
        }
        RuntimeAgentKind::ViewportPoint { x, y } => {
            writer.u8(7)?;
            writer.fixed_u32(x)?;
            writer.fixed_u32(y)?;
        }
        RuntimeAgentKind::BinaryData(data) => {
            writer.u8(8)?;
            writer.string(data)?;
        }
        RuntimeAgentKind::DataShape(shape) => {
            writer.u8(9)?;
            writer.extend(shape.shape_type().as_bytes())?;
            writer.extend(shape.value_type().as_bytes())?;
        }
    }
    Ok(())
}

fn predicate_prefix<'a, S: CanonicalSink + ?Sized, V: ValueValidation>(
    predicate: RuntimePredicateView<'a>,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    budget: Option<&ValueBudget>,
    work: &mut Vec<Work<'a, V::Expected, V::Children, V::Alternatives>>,
    validation: &mut V,
    children: Rc<V::Children>,
) -> Result<(), RuntimeSchemaError> {
    match predicate.kind() {
        RuntimePredicateKind::Compare { probe, op, value } => {
            writer.u8(0)?;
            writer.agent_probe(probe)?;
            writer.u8(match op {
                RuntimeAgentCompareOp::Eq => 0,
                RuntimeAgentCompareOp::NotEq => 1,
                RuntimeAgentCompareOp::Greater => 2,
                RuntimeAgentCompareOp::GreaterOrEqual => 3,
                RuntimeAgentCompareOp::Less => 4,
                RuntimeAgentCompareOp::LessOrEqual => 5,
            })?;
            work.push(Work::Value(
                value.view(),
                depth + 1,
                validation.child(&children, 0)?,
                Some(PathStep::Name("value")),
            ));
        }
        RuntimePredicateKind::Exists { probe } => {
            writer.u8(1)?;
            writer.agent_probe(probe)?;
        }
        RuntimePredicateKind::ActionEnabled { target } => {
            writer.u8(2)?;
            writer.string(target.as_str())?;
        }
        RuntimePredicateKind::DiagnosticsHasError => writer.u8(3)?,
        RuntimePredicateKind::All(predicates) | RuntimePredicateKind::Any(predicates) => {
            if let Some(budget) = budget {
                budget.collection(predicates.len())?;
            }
            writer.u8(
                if matches!(predicate.kind(), RuntimePredicateKind::All(_)) {
                    4
                } else {
                    5
                },
            )?;
            writer.len(predicates.len())?;
            work.push(Work::Predicates(predicates, 0, depth + 1, children));
        }
        RuntimePredicateKind::Not(predicate) => {
            writer.u8(6)?;
            work.push(Work::Predicate(
                predicate,
                depth + 1,
                children,
                PathStep::Name("not"),
            ));
        }
    }
    Ok(())
}

impl<S: CanonicalSink + ?Sized> CanonicalWriter<'_, S> {
    fn runtime_range_iterator(
        &mut self,
        range: &crate::value::RuntimeRangeIterator,
    ) -> Result<(), RuntimeSchemaError> {
        let end = range
            .validated_end()
            .ok_or_else(|| RuntimeSchemaError::Encoding {
                message: "range iterator has an invalid retained cursor".to_owned(),
            })?;
        self.u8(23)?;
        self.scalar(end)?;
        let (inclusive, done) = match range {
            crate::value::RuntimeRangeIterator::Int {
                current,
                inclusive,
                done,
                ..
            } => {
                self.i128(*current)?;
                (*inclusive, *done)
            }
            crate::value::RuntimeRangeIterator::UInt {
                current,
                inclusive,
                done,
                ..
            } => {
                self.u128(*current)?;
                (*inclusive, *done)
            }
        };
        self.u8(u8::from(inclusive))?;
        self.u8(u8::from(done))
    }

    fn runtime_range(
        &mut self,
        range: &crate::value::RuntimeRange,
    ) -> Result<(), RuntimeSchemaError> {
        use crate::value::{RuntimeRange, RuntimeScalarView as Scalar};
        self.u8(22)?;
        match range {
            RuntimeRange::Int {
                start,
                end,
                inclusive,
            } => {
                self.u8(u8::from(*inclusive))?;
                self.option(start.as_ref(), |writer, value| {
                    writer.scalar(Scalar::Int(*value))
                })?;
                self.option(end.as_ref(), |writer, value| {
                    writer.scalar(Scalar::Int(*value))
                })?;
            }
            RuntimeRange::UInt {
                start,
                end,
                inclusive,
            } => {
                self.u8(u8::from(*inclusive))?;
                self.option(start.as_ref(), |writer, value| {
                    writer.scalar(Scalar::UInt(*value))
                })?;
                self.option(end.as_ref(), |writer, value| {
                    writer.scalar(Scalar::UInt(*value))
                })?;
            }
        }
        Ok(())
    }

    fn scalar_for_encoding(
        &mut self,
        value: crate::value::RuntimeScalarView<'_>,
        encoding: ValueEncoding,
    ) -> Result<(), RuntimeSchemaError> {
        use crate::value::RuntimeScalarView as Scalar;
        // Canonical ingress/digests retain finite floats and normalized zero.
        // Executable literals, live values and snapshots retain exact IEEE bits.
        match (encoding, value) {
            (
                ValueEncoding::Literal | ValueEncoding::Live | ValueEncoding::Snapshot,
                Scalar::F32(value),
            ) => {
                self.u8(5)?;
                self.fixed_u32(value.to_bits())
            }
            (
                ValueEncoding::Literal | ValueEncoding::Live | ValueEncoding::Snapshot,
                Scalar::F64(value),
            ) => {
                self.u8(6)?;
                self.u64(value.to_bits())
            }
            (_, value) => self.scalar(value),
        }
    }

    fn dense_numeric<V: ValueValidation>(
        &mut self,
        value: RuntimeDenseNumericView<'_>,
        mut budget: Option<&mut ValueBudget>,
        depth: usize,
        encoding: ValueEncoding,
        validation: &mut V,
    ) -> Result<(), RuntimeSchemaError> {
        let values = value.values();
        if let Some(budget) = budget.as_deref() {
            budget.collection(value.rank())?;
            budget.collection(values.len())?;
        }
        self.u8(21)?;
        self.extend(&value.ty().semantic_parts())?;
        self.len(value.rank())?;
        for index in 0..value.rank() {
            self.len(value.dimension(index)?)?;
        }
        self.len(values.len())?;
        for value in values {
            if let Some(budget) = budget.as_deref_mut() {
                budget.node(depth + 1)?;
            }
            validation.embedded_node(depth + 1)?;
            self.scalar_for_encoding(value, encoding)?;
        }
        Ok(())
    }
}
