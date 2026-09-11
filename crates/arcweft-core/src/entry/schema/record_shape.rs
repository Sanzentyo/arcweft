//! Source shapes shared by nominal schema admission and executable layouts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Source shape of an accepted nominal record, including empty forms.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeNominalRecordShape {
    Unit,
    Tuple,
    Record,
    Newtype,
}

/// A record's field inventory does not describe its declared source shape.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordShapeError {
    #[error("{shape:?} requires {expected} fields, received {actual}")]
    FieldCount {
        shape: RuntimeNominalRecordShape,
        expected: usize,
        actual: usize,
    },
    #[error("record field {ordinal} requires a nonempty name")]
    MissingFieldName { ordinal: usize },
    #[error("{shape:?} field {ordinal} must be unnamed")]
    UnexpectedFieldName {
        shape: RuntimeNominalRecordShape,
        ordinal: usize,
    },
    #[error("record field {ordinal} repeats name `{name}`")]
    DuplicateFieldName { ordinal: usize, name: String },
}

impl RuntimeNominalRecordShape {
    /// Validates the complete ordered name inventory of this source shape.
    /// An empty named record is distinct from an empty tuple and a unit struct.
    pub fn validate_field_names<'a>(
        self,
        names: impl ExactSizeIterator<Item = Option<&'a str>>,
    ) -> Result<(), RuntimeNominalRecordShapeError> {
        let expected = match self {
            Self::Unit => Some(0),
            Self::Newtype => Some(1),
            Self::Tuple | Self::Record => None,
        };
        if let Some(expected) = expected
            && names.len() != expected
        {
            return Err(RuntimeNominalRecordShapeError::FieldCount {
                shape: self,
                expected,
                actual: names.len(),
            });
        }
        let mut seen = BTreeSet::new();
        for (ordinal, name) in names.enumerate() {
            match (self, name) {
                (Self::Record, Some(name)) if !name.is_empty() => {
                    if !seen.insert(name) {
                        return Err(RuntimeNominalRecordShapeError::DuplicateFieldName {
                            ordinal,
                            name: name.to_owned(),
                        });
                    }
                }
                (Self::Record, _) => {
                    return Err(RuntimeNominalRecordShapeError::MissingFieldName { ordinal });
                }
                (Self::Unit | Self::Tuple | Self::Newtype, Some(_)) => {
                    return Err(RuntimeNominalRecordShapeError::UnexpectedFieldName {
                        shape: self,
                        ordinal,
                    });
                }
                (Self::Unit | Self::Tuple | Self::Newtype, None) => {}
            }
        }
        Ok(())
    }

    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Unit => 0,
            Self::Tuple => 1,
            Self::Record => 2,
            Self::Newtype => 3,
        }
    }
}
