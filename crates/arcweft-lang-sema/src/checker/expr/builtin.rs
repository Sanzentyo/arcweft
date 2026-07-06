use crate::types::TypeKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BuiltinCallSpec {
    AssertLike,
    Ensure,
    InlineFailureFallback,
    Math(MathIntrinsic),
    Never,
    StdFloat(StdFloatIntrinsic),
}

impl BuiltinCallSpec {
    pub(super) fn resolve(path: &str) -> Option<Self> {
        let segments = path.split('.').collect::<Vec<_>>();
        StdFloatIntrinsic::resolve(&segments)
            .map(Self::StdFloat)
            .or_else(|| MathIntrinsic::resolve(&segments).map(Self::Math))
            .or(match segments.as_slice() {
                ["fallback"] | ["InlineFailure", "fallback"] => Some(Self::InlineFailureFallback),
                ["panic" | "fail" | "bail"] => Some(Self::Never),
                ["ensure"] => Some(Self::Ensure),
                ["assert" | "debug_assert"] => Some(Self::AssertLike),
                _ => None,
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CapabilityFunctionSpec {
    unchecked_prefix_args: usize,
}

impl CapabilityFunctionSpec {
    pub(super) fn resolve(path: &str) -> Option<Self> {
        let segments = path.split('.').collect::<Vec<_>>();
        match segments.as_slice() {
            ["event", "emit"] => Some(Self {
                unchecked_prefix_args: 1,
            }),
            _ => None,
        }
    }

    pub(super) const fn unchecked_prefix_args(self) -> usize {
        self.unchecked_prefix_args
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MathIntrinsic {
    MatrixF32,
    MatrixF64,
    TensorF32,
    TensorF64,
}

impl MathIntrinsic {
    fn resolve(segments: &[&str]) -> Option<Self> {
        match segments {
            ["math", "matmul_f32" | "matrix_add_f32"] => Some(Self::MatrixF32),
            ["math", "matmul_f64" | "matrix_add_f64"] => Some(Self::MatrixF64),
            ["math", "tensor_add_f32"] => Some(Self::TensorF32),
            ["math", "tensor_add_f64"] => Some(Self::TensorF64),
            _ => None,
        }
    }

    pub(super) const fn operand_type(self) -> &'static str {
        match self {
            Self::MatrixF32 => "MatrixF32",
            Self::MatrixF64 => "MatrixF64",
            Self::TensorF32 => "TensorF32",
            Self::TensorF64 => "TensorF64",
        }
    }

    pub(super) fn return_type(self) -> TypeKind {
        TypeKind::Named(self.operand_type().to_owned())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StdFloatIntrinsic {
    width: FloatWidth,
    kind: StdFloatIntrinsicKind,
}

impl StdFloatIntrinsic {
    fn resolve(segments: &[&str]) -> Option<Self> {
        let ["std", width, name] = segments else {
            return None;
        };
        let width = FloatWidth::resolve(width)?;
        let kind = match *name {
            "abs" | "floor" | "ceil" | "round" | "trunc" | "fract" | "sqrt" | "sin" | "cos"
            | "tan" | "exp" | "exp2" | "ln" | "log2" | "log10" => StdFloatIntrinsicKind::UnarySame,
            "powf" | "atan2" => StdFloatIntrinsicKind::BinarySame,
            "mul_add" => StdFloatIntrinsicKind::TernarySame,
            "is_nan" | "is_infinite" | "is_finite" | "is_sign_positive" | "is_sign_negative" => {
                StdFloatIntrinsicKind::Predicate
            }
            "to_bits" => StdFloatIntrinsicKind::ToBits,
            "from_bits" => StdFloatIntrinsicKind::FromBits,
            "to_f64" if width == FloatWidth::F32 => {
                StdFloatIntrinsicKind::ConvertTo(FloatWidth::F64)
            }
            "to_f32" if width == FloatWidth::F64 => {
                StdFloatIntrinsicKind::ConvertTo(FloatWidth::F32)
            }
            _ => return None,
        };
        Some(Self { width, kind })
    }

    pub(super) fn input_type(self) -> TypeKind {
        match self.kind {
            StdFloatIntrinsicKind::FromBits => self.width.bits_type(),
            StdFloatIntrinsicKind::UnarySame
            | StdFloatIntrinsicKind::BinarySame
            | StdFloatIntrinsicKind::TernarySame
            | StdFloatIntrinsicKind::Predicate
            | StdFloatIntrinsicKind::ToBits
            | StdFloatIntrinsicKind::ConvertTo(_) => self.width.type_kind(),
        }
    }

    pub(super) fn output_type(self) -> TypeKind {
        match self.kind {
            StdFloatIntrinsicKind::Predicate => TypeKind::Bool,
            StdFloatIntrinsicKind::ToBits => self.width.bits_type(),
            StdFloatIntrinsicKind::ConvertTo(width) => width.type_kind(),
            StdFloatIntrinsicKind::UnarySame
            | StdFloatIntrinsicKind::BinarySame
            | StdFloatIntrinsicKind::TernarySame
            | StdFloatIntrinsicKind::FromBits => self.width.type_kind(),
        }
    }

    pub(super) const fn arity(self) -> usize {
        match self.kind {
            StdFloatIntrinsicKind::UnarySame
            | StdFloatIntrinsicKind::Predicate
            | StdFloatIntrinsicKind::ToBits
            | StdFloatIntrinsicKind::FromBits
            | StdFloatIntrinsicKind::ConvertTo(_) => 1,
            StdFloatIntrinsicKind::BinarySame => 2,
            StdFloatIntrinsicKind::TernarySame => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StdFloatIntrinsicKind {
    BinarySame,
    ConvertTo(FloatWidth),
    FromBits,
    Predicate,
    TernarySame,
    ToBits,
    UnarySame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FloatWidth {
    F32,
    F64,
}

impl FloatWidth {
    fn resolve(segment: &str) -> Option<Self> {
        match segment {
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            _ => None,
        }
    }

    fn type_kind(self) -> TypeKind {
        match self {
            Self::F32 => TypeKind::F32,
            Self::F64 => TypeKind::F64,
        }
    }

    fn bits_type(self) -> TypeKind {
        match self {
            Self::F32 => TypeKind::U32,
            Self::F64 => TypeKind::U64,
        }
    }
}
