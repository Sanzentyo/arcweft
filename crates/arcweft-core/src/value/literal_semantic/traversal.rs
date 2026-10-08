//! One borrowed static literal graph for encoding and count-only preflight.
//! Cursors enter the next source child without materializing columnar rows.

use super::{
    RuntimeAgentPredicate, RuntimeAgentValue, RuntimeCommand, RuntimeIterator, RuntimeRecordView,
    RuntimeSeq, RuntimeTupleView, RuntimeValue, RuntimeValueView,
};
use crate::value::RuntimeRecordFieldId;

pub(super) enum LiteralEvent<'a> {
    Value(RuntimeValueView<'a>),
    Agent(&'a RuntimeAgentValue),
    Predicate(&'a RuntimeAgentPredicate),
    Child(Option<RuntimeRecordFieldId>),
    Command(&'a RuntimeCommand),
}

enum Work<'a> {
    Value(RuntimeValueView<'a>),
    Tuple(RuntimeTupleView<'a>, usize),
    Sequence(&'a RuntimeSeq, usize),
    Record(RuntimeRecordView<'a>, usize),
    Values(std::slice::Iter<'a, RuntimeValue>),
    Commands(std::slice::Iter<'a, RuntimeCommand>),
    Deque(std::collections::vec_deque::Iter<'a, RuntimeValue>),
    Agent(&'a RuntimeAgentValue),
    Predicate(&'a RuntimeAgentPredicate),
    Predicates(std::slice::Iter<'a, RuntimeAgentPredicate>),
}

pub(super) struct LiteralEvents<'a> {
    work: Vec<Work<'a>>,
}
impl<'a> LiteralEvents<'a> {
    pub(super) fn new(value: &'a RuntimeValue) -> Self {
        Self {
            work: vec![Work::Value(value.view())],
        }
    }
}
impl<'a> Iterator for LiteralEvents<'a> {
    type Item = LiteralEvent<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(work) = self.work.pop() {
            let event = match work {
                Work::Tuple(tuple, index) => {
                    let Some(child) = tuple.get(index) else {
                        continue;
                    };
                    self.work.push(Work::Tuple(tuple, index + 1));
                    self.work.push(Work::Value(child));
                    LiteralEvent::Child(None)
                }
                Work::Sequence(sequence, index) => {
                    let Some(child) = sequence.value_view(index) else {
                        continue;
                    };
                    self.work.push(Work::Sequence(sequence, index + 1));
                    self.work.push(Work::Value(child));
                    LiteralEvent::Child(None)
                }
                Work::Record(record, index) => {
                    let Some((field, _, child)) = record.get(index) else {
                        continue;
                    };
                    self.work.push(Work::Record(record, index + 1));
                    self.work.push(Work::Value(child));
                    LiteralEvent::Child(Some(field))
                }
                Work::Values(mut values) => {
                    let Some(value) = values.next() else { continue };
                    self.work.push(Work::Values(values));
                    self.work.push(Work::Value(value.view()));
                    LiteralEvent::Child(None)
                }
                Work::Deque(mut values) => {
                    let Some(value) = values.next() else { continue };
                    self.work.push(Work::Deque(values));
                    self.work.push(Work::Value(value.view()));
                    LiteralEvent::Child(None)
                }
                Work::Commands(mut commands) => {
                    let Some(command) = commands.next() else {
                        continue;
                    };
                    self.work.push(Work::Commands(commands));
                    self.work
                        .push(Work::Value(command.payload().value().view()));
                    LiteralEvent::Command(command)
                }
                Work::Agent(agent) => {
                    match agent {
                        RuntimeAgentValue::Predicate(predicate) => {
                            self.work.push(Work::Predicate(predicate));
                        }
                        RuntimeAgentValue::ActionTarget(_)
                        | RuntimeAgentValue::CaptureTarget(_)
                        | RuntimeAgentValue::DebugStatePath(_)
                        | RuntimeAgentValue::ObservationFieldPath(_)
                        | RuntimeAgentValue::Probe(_)
                        | RuntimeAgentValue::Diagnostics
                        | RuntimeAgentValue::ViewportPoint { .. }
                        | RuntimeAgentValue::BinaryData(_)
                        | RuntimeAgentValue::DataShape(_) => {}
                    }
                    LiteralEvent::Agent(agent)
                }
                Work::Predicate(predicate) => {
                    match predicate {
                        RuntimeAgentPredicate::Compare { value, .. } => {
                            self.work.push(Work::Value(value.view()));
                        }
                        RuntimeAgentPredicate::All { predicates }
                        | RuntimeAgentPredicate::Any { predicates } => {
                            self.work.push(Work::Predicates(predicates.iter()));
                        }
                        RuntimeAgentPredicate::Not { predicate } => {
                            self.work.push(Work::Predicate(predicate));
                        }
                        RuntimeAgentPredicate::Exists { .. }
                        | RuntimeAgentPredicate::ActionEnabled { .. }
                        | RuntimeAgentPredicate::DiagnosticsHasError => {}
                    }
                    LiteralEvent::Predicate(predicate)
                }
                Work::Predicates(mut predicates) => {
                    let Some(predicate) = predicates.next() else {
                        continue;
                    };
                    self.work.push(Work::Predicates(predicates));
                    self.work.push(Work::Predicate(predicate));
                    LiteralEvent::Child(None)
                }
                Work::Value(value) => {
                    self.enqueue_value_children(value);
                    LiteralEvent::Value(value)
                }
            };
            return Some(event);
        }
        None
    }
}

impl<'a> LiteralEvents<'a> {
    fn enqueue_value_children(&mut self, value: RuntimeValueView<'a>) {
        match value {
            RuntimeValueView::Tuple(tuple) => self.work.push(Work::Tuple(tuple, 0)),
            RuntimeValueView::Sequence(sequence) => {
                self.work.push(Work::Sequence(sequence, 0));
            }
            RuntimeValueView::Record(record) => self.work.push(Work::Record(record, 0)),
            RuntimeValueView::NominalRecord(record) => {
                self.work.push(Work::Values(record.fields().iter()));
            }
            RuntimeValueView::Opaque(value) => {
                self.work.push(Work::Value(value.payload().view()));
            }
            RuntimeValueView::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    self.work.push(Work::Value(payload.view()));
                }
            }
            RuntimeValueView::Reduction(value) => {
                self.work.push(Work::Commands(value.commands().iter()));
                self.work.push(Work::Value(value.state().view()));
            }
            RuntimeValueView::Agent(agent) => self.work.push(Work::Agent(agent)),
            RuntimeValueView::RuntimeOnly(RuntimeValue::Iterator(RuntimeIterator::Values {
                items,
            })) => self.work.push(Work::Deque(items.iter())),
            RuntimeValueView::Scalar(_)
            | RuntimeValueView::RuntimeOnly(
                RuntimeValue::Range(_)
                | RuntimeValue::Iterator(RuntimeIterator::Range(_) | RuntimeIterator::Witness { .. })
                | RuntimeValue::NeedHandle(_)
                | RuntimeValue::Callable(_)
                | RuntimeValue::Unit
                | RuntimeValue::Bool(_)
                | RuntimeValue::Int(_)
                | RuntimeValue::UInt(_)
                | RuntimeValue::F32(_)
                | RuntimeValue::F64(_)
                | RuntimeValue::String(_)
                | RuntimeValue::Color(_)
                | RuntimeValue::Char(_)
                | RuntimeValue::Duration(_)
                | RuntimeValue::Progress(_)
                | RuntimeValue::EntityRef(_)
                | RuntimeValue::Tuple(_)
                | RuntimeValue::Seq(_)
                | RuntimeValue::Record(_)
                | RuntimeValue::NominalRecord(_)
                | RuntimeValue::Opaque(_)
                | RuntimeValue::Reduction(_)
                | RuntimeValue::Agent(_)
                | RuntimeValue::Variant { .. }
                | RuntimeValue::MatrixF32(_)
                | RuntimeValue::MatrixF64(_)
                | RuntimeValue::TensorF32(_)
                | RuntimeValue::TensorF64(_),
            ) => {}
        }
    }
}
