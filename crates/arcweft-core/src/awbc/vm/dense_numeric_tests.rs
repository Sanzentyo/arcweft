use super::constant_value;
use crate::awbc::codec::AwbcDecodeBudget;
use crate::awbc::schema::{AwbcConstant, AwbcConstantId, AwbcProgram};
use crate::math::{RuntimeDenseNumericElements, RuntimeDenseNumericPayload};
use crate::math::{RuntimeDenseNumericType as Dense, RuntimeFloatWidth as Width};
use crate::value::RuntimeValue;

#[test]
fn dense_constant_artifact_materialization_keeps_container_width_and_ieee_bits() {
    let bits32 = vec![0x8000_0000, 0x7fc0_0001, 0x7f80_0000];
    let bits64 = vec![
        0x8000_0000_0000_0000,
        0x7ff8_0000_0000_0001,
        0x7ff0_0000_0000_0000,
    ];
    for ty in [
        Dense::Matrix(Width::F32),
        Dense::Matrix(Width::F64),
        Dense::Tensor(Width::F32),
        Dense::Tensor(Width::F64),
    ] {
        let dimensions = match ty {
            Dense::Matrix(_) => vec![1, 3],
            Dense::Tensor(_) => vec![3],
        };
        let elements = match ty.scalar_width() {
            Width::F32 => RuntimeDenseNumericElements::F32(bits32.clone()),
            Width::F64 => RuntimeDenseNumericElements::F64(bits64.clone()),
        };
        let constant = RuntimeDenseNumericPayload::try_new(ty, dimensions, elements).unwrap();
        let program = AwbcProgram {
            constants: vec![AwbcConstant::DenseNumeric(constant)],
            ..AwbcProgram::default()
        };
        let bytes = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
        assert_eq!(decoded.encode_canonical().unwrap(), bytes);
        let value = constant_value(&decoded, AwbcConstantId(0)).unwrap();
        match (ty, value) {
            (Dense::Matrix(Width::F32), RuntimeValue::MatrixF32(matrix)) => {
                assert_eq!((matrix.rows(), matrix.cols()), (1, 3));
                assert_eq!(
                    matrix
                        .values()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    bits32
                );
            }
            (Dense::Matrix(Width::F64), RuntimeValue::MatrixF64(matrix)) => {
                assert_eq!((matrix.rows(), matrix.cols()), (1, 3));
                assert_eq!(
                    matrix
                        .values()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    bits64
                );
            }
            (Dense::Tensor(Width::F32), RuntimeValue::TensorF32(tensor)) => {
                assert_eq!(tensor.shape().dims(), &[3]);
                assert_eq!(
                    tensor
                        .values()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    bits32
                );
            }
            (Dense::Tensor(Width::F64), RuntimeValue::TensorF64(tensor)) => {
                assert_eq!(tensor.shape().dims(), &[3]);
                assert_eq!(
                    tensor
                        .values()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    bits64
                );
            }
            _ => panic!(
                "dense payload materialization changed its checked container or scalar width"
            ),
        }
    }
}
