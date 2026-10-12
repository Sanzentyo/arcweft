//! One borrowed static literal graph for encoding and count-only preflight.
//! Cursors enter the next source child without materializing columnar rows.

use crate::value::{
    RuntimeCommandListView, RuntimeCommandView, RuntimeIteratorKind, RuntimeSequenceView,
};

use super::{
    RuntimeAgentPredicate, RuntimeAgentValue, RuntimeRecordView, RuntimeTupleView, RuntimeValue,
    RuntimeValueView,
};
use crate::value::RuntimeRecordFieldId;

pub(super) enum LiteralEvent<'a> {
    Value(RuntimeValueView<'a>),
    Agent(&'a RuntimeAgentValue),
    Predicate(&'a RuntimeAgentPredicate),
    Child(Option<RuntimeRecordFieldId>),
    Command(RuntimeCommandView<'a>),
}

enum Work<'a> {
    Value(RuntimeValueView<'a>),
    Tuple(RuntimeTupleView<'a>, usize),
    Sequence(RuntimeSequenceView<'a>, usize),
    Record(RuntimeRecordView<'a>, usize),
    Commands(RuntimeCommandListView<'a>, usize),
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
                Work::Commands(commands, index) => {
                    let Some(command) = commands.get(index) else {
                        continue;
                    };
                    self.work.push(Work::Commands(commands, index + 1));
                    self.work.push(Work::Value(command.payload()));
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
                self.work.push(Work::Tuple(record.fields(), 0));
            }
            RuntimeValueView::Opaque(value) => {
                self.work.push(Work::Value(value.payload()));
            }
            RuntimeValueView::Variant(variant) => {
                if let Some(payload) = variant.payload() {
                    self.work.push(Work::Value(payload));
                }
            }
            RuntimeValueView::Reduction(value) => {
                self.work.push(Work::Commands(value.commands(), 0));
                self.work.push(Work::Value(value.state()));
            }
            RuntimeValueView::Agent(agent) => self.work.push(Work::Agent(
                agent
                    .live_value()
                    .expect("literal traversal is live-issued"),
            )),
            RuntimeValueView::Iterator(iterator) => {
                if let RuntimeIteratorKind::Values(items) = iterator.kind() {
                    self.work.push(Work::Tuple(items, 0));
                }
            }
            RuntimeValueView::Scalar(_)
            | RuntimeValueView::DenseNumeric(_)
            | RuntimeValueView::Range(_)
            | RuntimeValueView::Need(_)
            | RuntimeValueView::Callable(_) => {}
        }
    }
}
