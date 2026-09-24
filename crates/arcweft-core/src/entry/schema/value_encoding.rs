//! Iterative canonical value traversal over borrowed logical storage views.

use std::{fmt, rc::Rc, slice};

use crate::value::{
    RuntimeAgentAction, RuntimeAgentActionDispatch, RuntimeAgentCaptureTarget,
    RuntimeAgentCompareOp, RuntimeAgentPredicate, RuntimeAgentValue, RuntimeCommand,
    RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeRecordView, RuntimeSeq,
    RuntimeTupleView, RuntimeValue, RuntimeValueView,
};

use super::{
    CanonicalSink, CanonicalWriter, RuntimeSchemaChoiceMismatch, RuntimeSchemaError,
    value_budget::ValueBudget,
};

pub(crate) enum ValueAdmission<C, A> {
    Children(C),
    Choice(A),
    Admitted,
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
}

/// Validates a persistent value and emits its canonical digest in one traversal.
pub(crate) fn validate_and_hash<V: ValueValidation>(
    value: &RuntimeValue,
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
        value.view(),
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
    validate_without_digest(value, limits, validation, expected, ValueEncoding::Literal)
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

/// Validates a decoded snapshot candidate before it becomes live.
pub(crate) fn validate_snapshot<V: ValueValidation>(
    value: &RuntimeValue,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
) -> Result<(), RuntimeSchemaError> {
    validate_without_digest(value, limits, validation, expected, ValueEncoding::Snapshot)
}

fn validate_without_digest<V: ValueValidation>(
    value: &RuntimeValue,
    limits: super::RuntimeSchemaLimits,
    validation: &mut V,
    expected: V::Expected,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    let mut budget = ValueBudget::new(limits);
    let mut sink = VisitOutput::<super::CanonicalBlake3Sink>::Probe;
    let mut writer = CanonicalWriter {
        sink: &mut sink,
        max_encoded_bytes: u64::MAX,
        max_string_bytes: Some(limits.max_string_bytes),
    };
    visit_with_encoding(
        value.view(),
        0,
        &mut writer,
        Some(&mut budget),
        validation,
        expected,
        encoding,
    )
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ValueEncoding {
    Canonical,
    Literal,
    Live,
    Snapshot,
}

pub(crate) struct NoSchemaValidation;

impl ValueValidation for NoSchemaValidation {
    type Expected = ();
    type Children = ();
    type Alternatives = std::iter::Empty<()>;

    fn preflight(&mut self, (): &(), _: usize) -> Result<(), RuntimeSchemaError> {
        Ok(())
    }
    fn alternative(&mut self, _: usize) -> Result<(), RuntimeSchemaError> {
        Ok(())
    }
    fn is_admitted((): &()) -> bool {
        true
    }
    fn admitted_children() {}

    fn enter(
        &mut self,
        (): (),
        _: RuntimeValueView<'_>,
    ) -> Result<ValueAdmission<(), Self::Alternatives>, RuntimeSchemaError> {
        Ok(ValueAdmission::Admitted)
    }
    fn child(&mut self, (): &(), _: usize) -> Result<(), RuntimeSchemaError> {
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
        let path = match &mut self {
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
            Self::BudgetExceeded { .. }
            | Self::Encoding { .. }
            | Self::SchemaEncodingOverflow
            | Self::NominalGraph { .. }
            | Self::NominalGraphRequired { .. }
            | Self::UnresolvedNominal { .. } => None,
        };
        if let Some(path) = path {
            *path = current.to_string();
        }
        self
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
    Sequence(&'a RuntimeSeq, usize, usize, Rc<C>),
    Values(slice::Iter<'a, RuntimeValue>, usize, usize, Rc<C>),
    CommandList(&'a [RuntimeCommand], usize, Rc<C>),
    Commands(slice::Iter<'a, RuntimeCommand>, usize, usize, Rc<C>),
    Predicate(&'a RuntimeAgentPredicate, usize, Rc<C>, PathStep<'a>),
    Predicates(slice::Iter<'a, RuntimeAgentPredicate>, usize, usize, Rc<C>),
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
    mut budget: Option<&mut ValueBudget>,
    validation: &mut V,
    expected: V::Expected,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    let mut path = ValuePath::default();
    let mut work = vec![Work::Value(root, depth, expected, None)];
    let mut mode = VisitMode::Output;
    let mut active_choices = Vec::new();
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
                        let value = choice.value;
                        let depth = choice.depth + 1;
                        active_choices.push(work.len());
                        work.push(Work::ChoiceComplete(choice, ordinal));
                        mode = VisitMode::Probe;
                        work.push(Work::Value(value, depth, expected, None));
                    } else {
                        mode = choice.parent_mode;
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
                        budget.as_deref(),
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
                    }
                }
                Work::Values(mut values, index, depth, children) => {
                    if let Some(value) = values.next() {
                        let expected = validation.child(&children, index)?;
                        work.push(Work::Values(values, index + 1, depth, children));
                        work.push(Work::Value(
                            value.view(),
                            depth,
                            expected,
                            Some(PathStep::Index(index)),
                        ));
                    }
                }
                Work::CommandList(commands, depth, children) => {
                    writer.len(commands.len())?;
                    work.push(Work::Commands(commands.iter(), 1, depth, children));
                }
                Work::Commands(mut commands, index, depth, children) => {
                    if let Some(command) = commands.next() {
                        if let Some(budget) = budget.as_deref_mut() {
                            budget.node(depth + 1)?;
                        }
                        writer.string(command.constructor().as_str())?;
                        writer.string(command.target().as_str())?;
                        let expected = validation.child(&children, index)?;
                        work.push(Work::Commands(commands, index + 1, depth, children));
                        work.push(Work::Value(
                            command.payload().0.view(),
                            depth + 2,
                            expected,
                            Some(PathStep::Index(index - 1)),
                        ));
                    }
                }
                Work::Predicate(predicate, depth, children, step) => {
                    path.0.push(step);
                    work.push(Work::PopPath);
                    if let Some(budget) = budget.as_deref_mut() {
                        budget.node(depth)?;
                    }
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
                Work::Predicates(mut predicates, index, depth, children) => {
                    if let Some(predicate) = predicates.next() {
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
                            budget.as_deref(),
                            &mut work,
                            validation,
                            Rc::new(children),
                            encoding,
                        )?,
                        ValueAdmission::Admitted => value_prefix(
                            value,
                            depth,
                            writer,
                            budget.as_deref(),
                            &mut work,
                            validation,
                            Rc::new(V::admitted_children()),
                            encoding,
                        )?,
                        ValueAdmission::Choice(alternatives) => {
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
    budget: Option<&ValueBudget>,
    work: &mut Vec<Work<'a, V::Expected, V::Children, V::Alternatives>>,
    validation: &mut V,
    children: Rc<V::Children>,
    encoding: ValueEncoding,
) -> Result<(), RuntimeSchemaError> {
    match value {
        RuntimeValueView::Scalar(value) => {
            // A probe sink discards bytes, but scalar validation (notably
            // finite floats) is identical for live, snapshot and digest use.
            writer.scalar(value)?;
        }
        RuntimeValueView::Tuple(tuple) => {
            writer.u8(11)?;
            writer.len(tuple.len())?;
            work.push(Work::Tuple(tuple, 0, depth + 1, children));
        }
        RuntimeValueView::Sequence(sequence) => {
            writer.u8(12)?;
            writer.len(sequence.len())?;
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
            work.push(Work::Values(record.fields().iter(), 0, depth + 1, children));
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
                ValueEncoding::Snapshot => affine,
                ValueEncoding::Live => false,
            };
            if rejected {
                return Err(RuntimeSchemaError::Encoding {
                    message: match encoding {
                        ValueEncoding::Canonical | ValueEncoding::Literal => {
                            "opaque value class/persistence is not constant-admissible"
                        }
                        ValueEncoding::Snapshot => {
                            "opaque affine handle is not snapshot-admissible"
                        }
                        ValueEncoding::Live => "opaque value class/persistence is not admissible",
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
                value.payload().view(),
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
                value.state().view(),
                depth + 1,
                expected,
                Some(PathStep::Name("state")),
            ));
        }
        RuntimeValueView::Agent(value) => {
            if matches!(value, RuntimeAgentValue::DataShape(_))
                && matches!(encoding, ValueEncoding::Canonical | ValueEncoding::Literal)
            {
                return Err(RuntimeSchemaError::Encoding {
                    message:
                        "DataShape witnesses require a selected program and program-bound snapshot"
                            .to_owned(),
                });
            }
            writer.u8(17)?;
            agent_prefix(value, depth, writer, budget, work, validation, children)?;
        }
        RuntimeValueView::Variant {
            owner,
            ordinal,
            name,
            payload,
        } => {
            writer.u8(14)?;
            writer.variant_identity(owner)?;
            writer.var_u32(ordinal)?;
            writer.string(name)?;
            writer.u8(u8::from(payload.is_some()))?;
            if let Some(payload) = payload {
                work.push(Work::Value(
                    payload.view(),
                    depth + 1,
                    validation.child(&children, 0)?,
                    Some(PathStep::Name(name)),
                ));
            }
        }
        RuntimeValueView::RuntimeOnly(RuntimeValue::Callable(callable))
            if matches!(encoding, ValueEncoding::Live | ValueEncoding::Snapshot) =>
        {
            if let Some(budget) = budget {
                budget.collection(callable.retained().len())?;
            }
            work.push(Work::Values(
                callable.retained().iter(),
                0,
                depth + 1,
                children,
            ));
        }
        RuntimeValueView::RuntimeOnly(value)
            if matches!(encoding, ValueEncoding::Literal | ValueEncoding::Live) =>
        {
            if let RuntimeValue::Iterator(crate::value::RuntimeIterator::Values { items, .. }) =
                value
            {
                if let Some(budget) = budget {
                    budget.collection(items.len())?;
                }
                work.push(Work::Values(items.iter(), 0, depth + 1, children));
            }
        }
        RuntimeValueView::RuntimeOnly(_) => {
            return Err(RuntimeSchemaError::Encoding {
                message: "runtime-only value has no replay/save encoding".to_owned(),
            });
        }
    }
    Ok(())
}

fn agent_prefix<'a, S: CanonicalSink + ?Sized, V: ValueValidation>(
    value: &'a RuntimeAgentValue,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    budget: Option<&ValueBudget>,
    work: &mut Vec<Work<'a, V::Expected, V::Children, V::Alternatives>>,
    validation: &mut V,
    children: Rc<V::Children>,
) -> Result<(), RuntimeSchemaError> {
    match value {
        RuntimeAgentValue::ActionTarget(target) => {
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
        RuntimeAgentValue::CaptureTarget(target) => {
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
        RuntimeAgentValue::DebugStatePath(path) => {
            writer.u8(2)?;
            writer.string(path.as_str())?;
        }
        RuntimeAgentValue::ObservationFieldPath(path) => {
            writer.u8(3)?;
            writer.string(path.as_str())?;
        }
        RuntimeAgentValue::Probe(probe) => {
            writer.u8(4)?;
            writer.agent_probe(probe)?;
        }
        RuntimeAgentValue::Diagnostics => writer.u8(5)?,
        RuntimeAgentValue::Predicate(predicate) => {
            writer.u8(6)?;
            // The root predicate is the Agent value's already charged node.
            predicate_prefix(predicate, depth, writer, budget, work, validation, children)?;
        }
        RuntimeAgentValue::ViewportPoint { x, y } => {
            writer.u8(7)?;
            writer.fixed_u32(*x)?;
            writer.fixed_u32(*y)?;
        }
        RuntimeAgentValue::BinaryData(data) => {
            writer.u8(8)?;
            writer.string(data)?;
        }
        RuntimeAgentValue::DataShape(shape) => {
            writer.u8(9)?;
            writer.extend(shape.shape_type().as_bytes())?;
            writer.extend(shape.value_type().as_bytes())?;
        }
    }
    Ok(())
}

fn predicate_prefix<'a, S: CanonicalSink + ?Sized, V: ValueValidation>(
    predicate: &'a RuntimeAgentPredicate,
    depth: usize,
    writer: &mut CanonicalWriter<'_, S>,
    budget: Option<&ValueBudget>,
    work: &mut Vec<Work<'a, V::Expected, V::Children, V::Alternatives>>,
    validation: &mut V,
    children: Rc<V::Children>,
) -> Result<(), RuntimeSchemaError> {
    match predicate {
        RuntimeAgentPredicate::Compare { probe, op, value } => {
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
        RuntimeAgentPredicate::Exists { probe } => {
            writer.u8(1)?;
            writer.agent_probe(probe)?;
        }
        RuntimeAgentPredicate::ActionEnabled { target } => {
            writer.u8(2)?;
            writer.string(target.as_str())?;
        }
        RuntimeAgentPredicate::DiagnosticsHasError => writer.u8(3)?,
        RuntimeAgentPredicate::All { predicates } | RuntimeAgentPredicate::Any { predicates } => {
            if let Some(budget) = budget {
                budget.collection(predicates.len())?;
            }
            writer.u8(if matches!(predicate, RuntimeAgentPredicate::All { .. }) {
                4
            } else {
                5
            })?;
            writer.len(predicates.len())?;
            work.push(Work::Predicates(predicates.iter(), 0, depth + 1, children));
        }
        RuntimeAgentPredicate::Not { predicate } => {
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
