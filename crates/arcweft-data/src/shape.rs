/// Byte representation policy. This is the Arcweft-native counterpart of
/// common `serde_bytes` use-cases, without depending on serde in the builtin layer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BytesFormat {
    #[default]
    Binary,
    Base64,
    Hex,
    Array,
}

/// Numeric representation for C-like enums, corresponding to `serde_repr` use-cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumRepr {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

impl EnumRepr {
    #[must_use]
    pub const fn type_shape(self) -> TypeShape {
        match self {
            Self::I8 => TypeShape::I8,
            Self::I16 => TypeShape::I16,
            Self::I32 => TypeShape::I32,
            Self::I64 => TypeShape::I64,
            Self::I128 => TypeShape::I128,
            Self::Isize => TypeShape::Isize,
            Self::U8 => TypeShape::U8,
            Self::U16 => TypeShape::U16,
            Self::U32 => TypeShape::U32,
            Self::U64 => TypeShape::U64,
            Self::U128 => TypeShape::U128,
            Self::Usize => TypeShape::Usize,
        }
    }

    #[must_use]
    pub const fn is_unsigned(self) -> bool {
        matches!(
            self,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::U128 | Self::Usize
        )
    }
}

/// External tag strategy for enum payloads.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum EnumTagStyle {
    #[default]
    External,
    Internal {
        tag: String,
    },
    Adjacent {
        tag: String,
        content: String,
    },
}

/// Record decoding policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordPolicy {
    pub deny_unknown_fields: bool,
}

impl Default for RecordPolicy {
    fn default() -> Self {
        Self {
            deny_unknown_fields: true,
        }
    }
}

/// Common rename rules for field and variant names.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenameRule {
    #[default]
    None,
    SnakeCase,
    KebabCase,
    CamelCase,
    PascalCase,
}

impl RenameRule {
    #[must_use]
    pub fn apply(self, input: &str) -> String {
        match self {
            Self::None => input.to_owned(),
            Self::SnakeCase => to_words(input).join("_"),
            Self::KebabCase => to_words(input).join("-"),
            Self::CamelCase => {
                let mut words = to_words(input).into_iter();
                match words.next() {
                    Some(first) => words.fold(first, |mut out, word| {
                        push_pascal(&mut out, &word);
                        out
                    }),
                    None => String::new(),
                }
            }
            Self::PascalCase => to_words(input)
                .into_iter()
                .fold(String::new(), |mut out, word| {
                    push_pascal(&mut out, &word);
                    out
                }),
        }
    }
}

fn push_pascal(out: &mut String, word: &str) {
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
}

fn to_words(input: &str) -> Vec<String> {
    let (mut words, current) = input.chars().fold(
        (Vec::<String>::new(), String::new()),
        |(mut words, mut current), ch| {
            if ch == '_' || ch == '-' {
                if !current.is_empty() {
                    words.push(current.clone());
                    current.clear();
                }
                return (words, current);
            }
            if ch.is_uppercase() && !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            current.extend(ch.to_lowercase());
            (words, current)
        },
    );
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Reflected field metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldShape {
    pub rust_name: String,
    pub wire_name: String,
    pub shape: TypeShape,
    pub has_default: bool,
    pub skip: bool,
    pub bytes_format: Option<BytesFormat>,
}

impl FieldShape {
    #[must_use]
    pub fn new(
        rust_name: impl Into<String>,
        wire_name: impl Into<String>,
        shape: TypeShape,
    ) -> Self {
        Self {
            rust_name: rust_name.into(),
            wire_name: wire_name.into(),
            shape,
            has_default: false,
            skip: false,
            bytes_format: None,
        }
    }

    #[must_use]
    pub const fn with_default(mut self) -> Self {
        self.has_default = true;
        self
    }

    #[must_use]
    pub const fn skipped(mut self) -> Self {
        self.skip = true;
        self
    }

    #[must_use]
    pub const fn with_bytes_format(mut self, bytes_format: BytesFormat) -> Self {
        self.bytes_format = Some(bytes_format);
        self
    }

    #[must_use]
    pub fn value_shape(&self) -> TypeShape {
        match (self.bytes_format, &self.shape) {
            (Some(format), TypeShape::Bytes { .. }) => TypeShape::Bytes { format },
            _ => self.shape.clone(),
        }
    }
}

/// Reflected enum variant metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantShape {
    pub rust_name: String,
    pub wire_name: String,
    pub payload: Option<TypeShape>,
    pub discriminant: Option<i128>,
}

impl VariantShape {
    #[must_use]
    pub fn unit(rust_name: impl Into<String>, wire_name: impl Into<String>) -> Self {
        Self {
            rust_name: rust_name.into(),
            wire_name: wire_name.into(),
            payload: None,
            discriminant: None,
        }
    }

    #[must_use]
    pub fn with_payload(mut self, payload: TypeShape) -> Self {
        self.payload = Some(payload);
        self
    }

    #[must_use]
    pub const fn with_discriminant(mut self, discriminant: i128) -> Self {
        self.discriminant = Some(discriminant);
        self
    }
}

/// Format-independent structural shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeShape {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    F32,
    F64,
    String,
    Char,
    Bytes {
        format: BytesFormat,
    },
    Option(Box<TypeShape>),
    Seq(Box<TypeShape>),
    Map {
        key: Box<TypeShape>,
        value: Box<TypeShape>,
    },
    Record {
        name: String,
        fields: Vec<FieldShape>,
        policy: RecordPolicy,
    },
    Enum {
        name: String,
        variants: Vec<VariantShape>,
        tag: EnumTagStyle,
        repr: Option<EnumRepr>,
    },
    Named(String),
}

impl TypeShape {
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::Isize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::Usize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::String => "string",
            Self::Char => "char",
            Self::Bytes { .. } => "bytes",
            Self::Option(_) => "option",
            Self::Seq(_) => "sequence",
            Self::Map { .. } => "map",
            Self::Record { .. } => "record",
            Self::Enum { .. } => "enum",
            Self::Named(_) => "named shape",
        }
    }

    #[must_use]
    pub fn record(name: impl Into<String>, fields: impl IntoIterator<Item = FieldShape>) -> Self {
        Self::Record {
            name: name.into(),
            fields: fields.into_iter().collect(),
            policy: RecordPolicy::default(),
        }
    }

    #[must_use]
    pub fn enumeration(
        name: impl Into<String>,
        variants: impl IntoIterator<Item = VariantShape>,
    ) -> Self {
        Self::Enum {
            name: name.into(),
            variants: variants.into_iter().collect(),
            tag: EnumTagStyle::default(),
            repr: None,
        }
    }

    #[must_use]
    pub fn option(inner: Self) -> Self {
        Self::Option(Box::new(inner))
    }

    #[must_use]
    pub fn seq(inner: Self) -> Self {
        Self::Seq(Box::new(inner))
    }

    #[must_use]
    pub fn map(key: Self, value: Self) -> Self {
        Self::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    #[must_use]
    pub fn signed_bounds(&self) -> Option<(i128, i128)> {
        match self {
            Self::I8 => Some((i128::from(i8::MIN), i128::from(i8::MAX))),
            Self::I16 => Some((i128::from(i16::MIN), i128::from(i16::MAX))),
            Self::I32 => Some((i128::from(i32::MIN), i128::from(i32::MAX))),
            Self::I64 => Some((i128::from(i64::MIN), i128::from(i64::MAX))),
            Self::I128 => Some((i128::MIN, i128::MAX)),
            Self::Isize => Some((isize::MIN as i128, isize::MAX as i128)),
            _ => None,
        }
    }

    #[must_use]
    pub fn unsigned_max(&self) -> Option<u128> {
        match self {
            Self::U8 => Some(u128::from(u8::MAX)),
            Self::U16 => Some(u128::from(u16::MAX)),
            Self::U32 => Some(u128::from(u32::MAX)),
            Self::U64 => Some(u128::from(u64::MAX)),
            Self::U128 => Some(u128::MAX),
            Self::Usize => Some(usize::MAX as u128),
            _ => None,
        }
    }
}
