//! Storage headers use a depth-bounded cursor, including zero-row storage.

use super::{RuntimeSchemaError, RuntimeSequenceView, ValidationWork, ValueBudget};
use crate::value::{
    RecordSeqField, RuntimeRecordFieldId, RuntimeSeq,
    awbc_save::{AwbcRuntimeRecordSeqFieldSnapshot, AwbcRuntimeSeqSnapshot},
    record::RecordFieldAdmission,
};

#[derive(Clone, Copy)]
enum Columns<'a> {
    Tuples(&'a [RuntimeSeq]),
    SnapshotTuples(&'a [AwbcRuntimeSeqSnapshot]),
    Records(&'a [RecordSeqField]),
    SnapshotRecords(&'a [AwbcRuntimeRecordSeqFieldSnapshot]),
}

impl<'a> Columns<'a> {
    fn len(self) -> usize {
        match self {
            Self::Tuples(v) => v.len(),
            Self::SnapshotTuples(v) => v.len(),
            Self::Records(v) => v.len(),
            Self::SnapshotRecords(v) => v.len(),
        }
    }
    fn is_record(self) -> bool {
        matches!(self, Self::Records(_) | Self::SnapshotRecords(_))
    }
    fn get(
        self,
        index: usize,
    ) -> Option<(
        Option<(RuntimeRecordFieldId, &'a str)>,
        RuntimeSequenceView<'a>,
    )> {
        Some(match self {
            Self::Tuples(v) => (None, RuntimeSequenceView::Live(v.get(index)?)),
            Self::SnapshotTuples(v) => (None, RuntimeSequenceView::Snapshot(v.get(index)?)),
            Self::Records(v) => {
                let field = v.get(index)?;
                (
                    Some((field.field(), field.name())),
                    RuntimeSequenceView::Live(field.values()),
                )
            }
            Self::SnapshotRecords(v) => {
                let field = v.get(index)?;
                (
                    Some((field.field, &field.name)),
                    RuntimeSequenceView::Snapshot(&field.values),
                )
            }
        })
    }
}

enum Work<'a> {
    Header(RuntimeSequenceView<'a>, usize, Option<usize>),
    Columns {
        columns: Columns<'a>,
        index: usize,
        length: usize,
        depth: usize,
        fields: Option<RecordFieldAdmission<'a>>,
    },
}

impl RuntimeSequenceView<'_> {
    pub(crate) fn validate_storage_metadata(
        self,
        depth: usize,
        budget: &ValueBudget,
        work: &mut ValidationWork,
    ) -> Result<(), RuntimeSchemaError> {
        let mut pending = vec![Work::Header(self, depth, None)];
        while let Some(next) = pending.pop() {
            match next {
                Work::Header(sequence, depth, expected) => {
                    if depth > crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH {
                        return Err(crate::value::RuntimeValueNestingError::Exceeded {
                            maximum: crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH,
                        }
                        .into());
                    }
                    work.charge(depth)?;
                    let length = sequence.len()?;
                    budget.collection(length)?;
                    if expected.is_some_and(|expected| expected != length) {
                        return Err(Self::column_length_error());
                    }
                    let columns = match sequence {
                        Self::Live(RuntimeSeq::TupleColumns(v)) => {
                            Some(Columns::Tuples(v.columns()))
                        }
                        Self::Live(RuntimeSeq::RecordColumns(v)) => {
                            Some(Columns::Records(v.fields()))
                        }
                        Self::Snapshot(AwbcRuntimeSeqSnapshot::TupleColumns {
                            columns, ..
                        }) => Some(Columns::SnapshotTuples(columns)),
                        Self::Snapshot(AwbcRuntimeSeqSnapshot::RecordColumns {
                            fields, ..
                        }) => Some(Columns::SnapshotRecords(fields)),
                        Self::Bytes(_)
                        | Self::DialogueBindings(_)
                        | Self::DialogueEffects(_)
                        | Self::Live(RuntimeSeq::Values(_) | RuntimeSeq::Dense(_))
                        | Self::Snapshot(
                            AwbcRuntimeSeqSnapshot::Values(_) | AwbcRuntimeSeqSnapshot::Dense(_),
                        ) => None,
                    };
                    if let Some(columns) = columns {
                        budget.collection(columns.len())?;
                        let fields = columns
                            .is_record()
                            .then(|| RecordFieldAdmission::new(columns.len()))
                            .transpose()
                            .map_err(Self::record_error)?;
                        pending.push(Work::Columns {
                            columns,
                            index: 0,
                            length,
                            depth: depth + 1,
                            fields,
                        });
                    }
                }
                Work::Columns {
                    columns,
                    index,
                    length,
                    depth,
                    mut fields,
                } => {
                    if let Some((field, values)) = columns.get(index) {
                        if let Some((identity, name)) = field {
                            budget.string(name)?;
                            fields
                                .as_mut()
                                .expect("record columns retain ordered field admission")
                                .admit(identity, name)
                                .map_err(Self::record_error)?;
                        }
                        pending.push(Work::Columns {
                            columns,
                            index: index + 1,
                            length,
                            depth,
                            fields,
                        });
                        pending.push(Work::Header(values, depth, Some(length)));
                    }
                }
            }
        }
        Ok(())
    }
}
