use std::fmt::{self, Display, Formatter};

/// Segment inside a structured data path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PathSegment {
    Field(String),
    Index(usize),
    Variant(String),
}

impl Display for PathSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(name) => write!(f, ".{name}"),
            Self::Index(index) => write!(f, "[{index}]"),
            Self::Variant(name) => write!(f, "::{name}"),
        }
    }
}

/// Path to a failing field, sequence element, or enum payload.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DataPath(Vec<PathSegment>);

impl DataPath {
    #[must_use]
    pub const fn root() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn field(mut self, name: impl Into<String>) -> Self {
        self.0.push(PathSegment::Field(name.into()));
        self
    }

    #[must_use]
    pub fn index(mut self, index: usize) -> Self {
        self.0.push(PathSegment::Index(index));
        self
    }

    #[must_use]
    pub fn variant(mut self, name: impl Into<String>) -> Self {
        self.0.push(PathSegment::Variant(name.into()));
        self
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn segments(&self) -> &[PathSegment] {
        &self.0
    }

    pub fn push_front(&mut self, segment: PathSegment) {
        self.0.insert(0, segment);
    }
}

impl Display for DataPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("$");
        }
        f.write_str("$")?;
        self.0.iter().try_for_each(|segment| write!(f, "{segment}"))
    }
}

/// Stable machine-readable data error class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataErrorKind {
    MissingField,
    UnknownField,
    DuplicateField,
    InvalidType,
    InvalidEnumTag,
    NumberOutOfRange,
    InvalidEncoding,
    TrailingData,
    LimitExceeded,
    UnsupportedFormat,
    Io,
    Custom,
}

impl Display for DataErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::MissingField => "missing_field",
            Self::UnknownField => "unknown_field",
            Self::DuplicateField => "duplicate_field",
            Self::InvalidType => "invalid_type",
            Self::InvalidEnumTag => "invalid_enum_tag",
            Self::NumberOutOfRange => "number_out_of_range",
            Self::InvalidEncoding => "invalid_encoding",
            Self::TrailingData => "trailing_data",
            Self::LimitExceeded => "limit_exceeded",
            Self::UnsupportedFormat => "unsupported_format",
            Self::Io => "io",
            Self::Custom => "custom",
        };
        f.write_str(label)
    }
}

/// Error carrying kind, path, and readable message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataError {
    kind: DataErrorKind,
    path: DataPath,
    message: String,
}

impl DataError {
    #[must_use]
    pub fn new(kind: DataErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            path: DataPath::root(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn invalid_type(expected: impl Into<String>, found: impl Into<String>) -> Self {
        Self::new(
            DataErrorKind::InvalidType,
            format!("expected {}, found {}", expected.into(), found.into()),
        )
    }

    #[must_use]
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::UnsupportedFormat, message)
    }

    #[must_use]
    pub fn limit(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::LimitExceeded, message)
    }

    #[must_use]
    pub fn with_path(mut self, path: DataPath) -> Self {
        self.path = path;
        self
    }

    #[must_use]
    pub fn at_field(mut self, field: impl Into<String>) -> Self {
        self.path.push_front(PathSegment::Field(field.into()));
        self
    }

    #[must_use]
    pub fn at_index(mut self, index: usize) -> Self {
        self.path.push_front(PathSegment::Index(index));
        self
    }

    #[must_use]
    pub fn at_variant(mut self, variant: impl Into<String>) -> Self {
        self.path.push_front(PathSegment::Variant(variant.into()));
        self
    }

    #[must_use]
    pub const fn kind(&self) -> &DataErrorKind {
        &self.kind
    }

    #[must_use]
    pub const fn path(&self) -> &DataPath {
        &self.path
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for DataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}: {}", self.kind, self.path, self.message)
    }
}

impl std::error::Error for DataError {}

pub type Result<T> = std::result::Result<T, DataError>;
