//! Borrowed plain construction shapes. Owning payloads allocate only after
//! the enclosing retained admission accepts their complete original type.

use super::{
    LiveRuntimeValueView, RuntimeBorrowedValue, RuntimeScalarView, RuntimeTupleView, RuntimeValue,
    RuntimeValueSource, RuntimeValueView, RuntimeVariantView,
};
use crate::entry::schema::value_encoding::ValueAdmissionOrigin;
use crate::pattern::{RuntimeBuiltinVariantCaseIdentity, RuntimeVariantIdentity};

/// A prospective scalar/tuple/Option tree borrowing all variable-sized data.
/// It carries neither an opaque header nor an executable or affine grant.
#[derive(Clone, Copy, Debug)]
pub struct RuntimePlainValueRecipe<'a> {
    shape: PlainShape<'a>,
}

#[derive(Clone, Copy, Debug)]
enum PlainShape<'a> {
    Scalar(RuntimeScalarView<'a>),
    Tuple(&'a [RuntimePlainValueRecipe<'a>]),
    Option(Option<&'a RuntimePlainValueRecipe<'a>>),
}

impl<'a> RuntimePlainValueRecipe<'a> {
    #[must_use]
    pub const fn tuple(fields: &'a [Self]) -> Self {
        Self {
            shape: PlainShape::Tuple(fields),
        }
    }

    #[must_use]
    pub const fn some(value: &'a Self) -> Self {
        Self {
            shape: PlainShape::Option(Some(value)),
        }
    }

    #[must_use]
    pub const fn none() -> Self {
        Self {
            shape: PlainShape::Option(None),
        }
    }

    /// Inspects the prospective plain shape without constructing owning values.
    #[must_use]
    pub fn borrowed(&self) -> RuntimeBorrowedValue<'_> {
        RuntimeBorrowedValue {
            value: self.view(),
            origin: ValueAdmissionOrigin::Live,
        }
    }

    pub(crate) fn view(&self) -> RuntimeValueView<'_> {
        match self.shape {
            PlainShape::Scalar(value) => RuntimeValueView::Scalar(value),
            PlainShape::Tuple(fields) => {
                RuntimeValueView::Tuple(RuntimeTupleView::PlainRecipes(fields))
            }
            PlainShape::Option(payload) => {
                static OWNER: RuntimeVariantIdentity = RuntimeVariantIdentity::Builtin(
                    RuntimeBuiltinVariantCaseIdentity::OptionNone.owner(),
                );
                let case = if payload.is_some() {
                    RuntimeBuiltinVariantCaseIdentity::OptionSome
                } else {
                    RuntimeBuiltinVariantCaseIdentity::OptionNone
                };
                let (ordinal, schema) = case
                    .owner()
                    .resolve_case(case)
                    .expect("Core's canonical Option case belongs to its owner");
                RuntimeValueView::Variant(RuntimeVariantView::borrowed(
                    &OWNER,
                    ordinal,
                    schema.name(),
                    payload.map(RuntimeValueSource::PlainOptionPayload),
                ))
            }
        }
    }

    /// Called only by retained admission after its full prospective root passes.
    pub(super) fn materialize(&self) -> RuntimeValue {
        match self.shape {
            PlainShape::Scalar(value) => {
                LiveRuntimeValueView::construction(RuntimeValueView::Scalar(value))
                    .copy_unrestricted()
                    .expect("validated scalar atoms carry no affine grant")
            }
            PlainShape::Tuple(fields) => {
                RuntimeValue::Tuple(fields.iter().map(Self::materialize).collect())
            }
            PlainShape::Option(Some(value)) => RuntimeValue::option_some(value.materialize()),
            PlainShape::Option(None) => RuntimeValue::option_none(),
        }
    }
}

impl<'a> From<&'a str> for RuntimePlainValueRecipe<'a> {
    fn from(value: &'a str) -> Self {
        Self {
            shape: PlainShape::Scalar(RuntimeScalarView::String(value)),
        }
    }
}

impl From<super::RuntimeInt> for RuntimePlainValueRecipe<'_> {
    fn from(value: super::RuntimeInt) -> Self {
        Self {
            shape: PlainShape::Scalar(RuntimeScalarView::Int(value)),
        }
    }
}

impl From<super::RuntimeUInt> for RuntimePlainValueRecipe<'_> {
    fn from(value: super::RuntimeUInt) -> Self {
        Self {
            shape: PlainShape::Scalar(RuntimeScalarView::UInt(value)),
        }
    }
}
impl<'a> TryFrom<RuntimeScalarView<'a>> for RuntimePlainValueRecipe<'a> {
    type Error = arcweft_need::ProgressError;

    fn try_from(value: RuntimeScalarView<'a>) -> Result<Self, Self::Error> {
        if let RuntimeScalarView::Progress { ratio, .. } = value {
            super::Progress::new(ratio)?;
        }
        Ok(Self {
            shape: PlainShape::Scalar(value),
        })
    }
}
