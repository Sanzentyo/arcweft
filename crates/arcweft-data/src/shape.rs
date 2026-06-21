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
}
