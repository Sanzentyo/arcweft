//! One non-owning runtime label projection for live and inert storage.

use super::{RuntimeScalarView as Scalar, RuntimeValueView as View};
use crate::{
    entry::RuntimeSchemaError,
    math::{RuntimeDenseNumericType, RuntimeFloatWidth},
    program_types::RuntimeProgramTypes,
};
use std::borrow::Cow;

impl<'a> View<'a> {
    pub(crate) fn label(
        self,
        original: Option<RuntimeProgramTypes<'_>>,
    ) -> Result<Cow<'a, str>, RuntimeSchemaError> {
        let owned = Cow::Owned;
        Ok(match self {
            Self::Scalar(value) => match value {
                Scalar::Unit => Cow::Borrowed("()"),
                Scalar::Bool(value) => owned(value.to_string()),
                Scalar::Int(value) => owned(value.label()),
                Scalar::UInt(value) => owned(value.label()),
                Scalar::F32(value) => owned(value.to_string()),
                Scalar::F64(value) => owned(value.to_string()),
                Scalar::String(value) => Cow::Borrowed(value),
                Scalar::Color(value) => {
                    let [r, g, b, a] = value.rgba8();
                    owned(format!("#{r:02x}{g:02x}{b:02x}{a:02x}"))
                }
                Scalar::Char(value) => owned(value.to_string()),
                Scalar::Duration(value) => owned(format!("{}ns", value.as_nanos())),
                Scalar::Progress { ratio, label } => {
                    label.map_or_else(|| owned(format!("progress/{ratio}")), Cow::Borrowed)
                }
                Scalar::EntityRef(value) => owned(value.runtime_label()),
            },
            Self::DenseNumeric(value) => match value.ty() {
                RuntimeDenseNumericType::Matrix(width) => {
                    let name = match width {
                        RuntimeFloatWidth::F32 => "f32",
                        RuntimeFloatWidth::F64 => "f64",
                    };
                    owned(format!(
                        "matrix/{name}/{}x{}",
                        value.dimension(0)?,
                        value.dimension(1)?
                    ))
                }
                RuntimeDenseNumericType::Tensor(width) => {
                    let name = match width {
                        RuntimeFloatWidth::F32 => "f32",
                        RuntimeFloatWidth::F64 => "f64",
                    };
                    let dims = (0..value.rank())
                        .map(|index| value.dimension(index))
                        .collect::<Result<Vec<_>, _>>()?;
                    owned(format!("tensor/{name}/{dims:?}"))
                }
            },
            Self::Tuple(value) => owned(format!("tuple/{}", value.len())),
            Self::Record(value) => owned(format!("record/{}", value.len())),
            Self::Sequence(value) => {
                owned(format!("seq/{}/{}", value.storage_label(), value.len()?))
            }
            Self::NominalRecord(value) => owned(format!(
                "nominal-record/{}/{}",
                value.type_id().as_str(),
                value.fields().len()
            )),
            Self::Opaque(value) => owned(format!("opaque/{}", value.producer().as_str())),
            Self::Reduction(value) => owned(format!("reduction/{}", value.commands().len())),
            Self::Agent(value) => Cow::Borrowed(value.label()),
            Self::Variant(value) => owned(if value.payload().is_some() {
                format!(".{}(...)", value.name())
            } else {
                format!(".{}", value.name())
            }),
            Self::Range(value) => owned(value.label()),
            Self::Iterator(_) => Cow::Borrowed("iterator"),
            Self::Need(value) => owned(format!("need/{}", value.need_id())),
            Self::Callable(value) => value.remaining_arity(original).map_or_else(
                |_| Cow::Borrowed("function/invalid"),
                |arity| owned(format!("function/{arity}")),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64},
        task::RuntimeProgramOwner,
        value::{AwbcRuntimeValueSnapshot, RecordSeq, RuntimeSeq, RuntimeValue, TupleSeq},
    };

    #[test]
    fn retained_labels_preserve_numeric_shape_and_physical_sequence_storage() {
        let owner = RuntimeProgramOwner::Awbc(std::sync::Arc::new(
            crate::awbc::schema::AwbcProgram::default(),
        ));
        let cases = [
            (
                RuntimeValue::MatrixF32(DenseMatrixF32::new(2, 3, vec![0.0; 6]).unwrap()),
                "matrix/f32/2x3",
            ),
            (
                RuntimeValue::MatrixF64(DenseMatrixF64::new(1, 2, vec![0.0; 2]).unwrap()),
                "matrix/f64/1x2",
            ),
            (
                RuntimeValue::TensorF32(DenseTensorF32::new(vec![2, 1, 3], vec![0.0; 6]).unwrap()),
                "tensor/f32/[2, 1, 3]",
            ),
            (
                RuntimeValue::TensorF64(DenseTensorF64::new(vec![2, 3], vec![0.0; 6]).unwrap()),
                "tensor/f64/[2, 3]",
            ),
            (
                RuntimeValue::Seq(RuntimeSeq::Values(vec![RuntimeValue::Unit])),
                "seq/values/1",
            ),
            (
                RuntimeValue::Seq(RuntimeSeq::dense_u16(vec![1, 2])),
                "seq/u16/2",
            ),
            (
                RuntimeValue::Seq(RuntimeSeq::dense_bytes(vec![1, 2])),
                "seq/bytes/2",
            ),
            (
                RuntimeValue::Seq(RuntimeSeq::TupleColumns(
                    TupleSeq::new(2, vec![RuntimeSeq::dense_bool(vec![true, false])]).unwrap(),
                )),
                "seq/tuple_columns/2",
            ),
            (
                RuntimeValue::Seq(RuntimeSeq::RecordColumns(
                    RecordSeq::try_from_fields(2, vec![]).unwrap(),
                )),
                "seq/record_columns/2",
            ),
        ];
        for (value, expected) in cases {
            let image =
                AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &owner).unwrap();
            assert_eq!(value.view().label(None).unwrap(), expected);
            assert_eq!(image.view().label(Some(owner.types())).unwrap(), expected);
        }
    }

    #[test]
    fn string_labels_borrow_the_original_live_or_inert_storage() {
        let value = RuntimeValue::String("retained".into());
        let image = AwbcRuntimeValueSnapshot::String("retained".into());
        for (view, source) in [
            (
                value.view(),
                match &value {
                    RuntimeValue::String(s) => s.as_str(),
                    _ => unreachable!(),
                },
            ),
            (
                image.view(),
                match &image {
                    AwbcRuntimeValueSnapshot::String(s) => s.as_str(),
                    _ => unreachable!(),
                },
            ),
        ] {
            let Cow::Borrowed(label) = view.label(None).unwrap() else {
                panic!("string label was copied")
            };
            assert_eq!(label.as_ptr(), source.as_ptr());
        }
    }
}
