use arcweft_lang_syntax::expr::LifetimeScopeKind;

/// Entity family used by semantic references and ID checks.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
    Agent,
    Entry,
    Flow,
    Fragment,
    Choice,
    ChoiceOption,
    Character,
    Component,
    Activity,
    Textbox,
    DialogueLine,
    Text,
    Asset,
    Image,
    Animation,
    Capture,
    Hook,
    Signal,
    Metric,
    Scene,
    Source,
    Test,
    Bench,
    Layer,
    Voice,
    Se,
    Bgm,
    AudioBus,
    MixerSnapshot,
    Ducking,
    Motion,
    Rig,
    Slot,
    Target,
    Other(String),
}

/// Entity reference type with optional payload type.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EntityType {
    kind: EntityKind,
    value: Option<Box<TypeKind>>,
}

/// Minimal semantic type used by parser/HIR contract tests.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeKind {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    TextCluster,
    Duration,
    Range,
    DisplayText,
    Ref(EntityType),
    Probe(Box<TypeKind>),
    Predicate,
    Observation,
    ActionResult,
    CaptureTarget,
    CaptureRef,
    RagContextPack,
    Vec(Box<TypeKind>),
    Array {
        item: Box<TypeKind>,
        len: String,
    },
    Slice(Box<TypeKind>),
    Seq(Box<TypeKind>),
    Map {
        kind: MapKind,
        key: Box<TypeKind>,
        value: Box<TypeKind>,
    },
    BorrowRef {
        lifetime: Option<LifetimeScopeKind>,
        inner: Box<TypeKind>,
    },
    Need {
        ready: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Stream {
        item: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Source {
        item: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Result {
        ok: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Option(Box<TypeKind>),
    Handle {
        name: String,
        lifetime: LifetimeScopeKind,
        state: HandleState,
        must_drop: bool,
    },
    ThreadHandle(Box<TypeKind>),
    Shared(Box<TypeKind>),
    Function {
        return_type: Box<TypeKind>,
    },
    Speaker(EntityKind),
    SpeakerPreset(EntityKind),
    CharacterPatch(EntityKind),
    FocusPatch,
    Named(String),
    Tuple(Vec<TypeKind>),
    Choice(Vec<TypeKind>),
    Unit,
    Never,
}

impl EntityType {
    pub fn new(kind: EntityKind, value: Option<TypeKind>) -> Self {
        Self {
            kind,
            value: value.map(Box::new),
        }
    }

    pub const fn kind(&self) -> &EntityKind {
        &self.kind
    }

    pub fn value(&self) -> Option<&TypeKind> {
        self.value.as_deref()
    }
}

impl TypeKind {
    #[must_use]
    pub fn entity_ref(kind: EntityKind) -> Self {
        Self::Ref(EntityType::new(kind, None))
    }

    #[must_use]
    pub fn entity_ref_with_value(kind: EntityKind, value: TypeKind) -> Self {
        Self::Ref(EntityType::new(kind, Some(value)))
    }

    #[must_use]
    pub fn is_entity_ref_kind(&self, kind: &EntityKind) -> bool {
        matches!(self, Self::Ref(entity) if entity.kind() == kind)
    }

    #[must_use]
    pub const fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::I128
                | Self::ISize
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::U128
                | Self::USize
        )
    }

    #[must_use]
    pub const fn is_float(&self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    #[must_use]
    pub fn primitive_name(name: &str) -> Option<Self> {
        Some(match name {
            "bool" | "Bool" => Self::Bool,
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "i128" => Self::I128,
            "isize" => Self::ISize,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "u128" => Self::U128,
            "usize" => Self::USize,
            "f32" => Self::F32,
            "f64" => Self::F64,
            "String" => Self::String,
            "char" | "Char" => Self::Char,
            "TextCluster" => Self::TextCluster,
            "Duration" => Self::Duration,
            "()" | "Unit" => Self::Unit,
            _ => return None,
        })
    }
}

/// Deterministic map family preserved by semantic type checking.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MapKind {
    Ordered,
    Sorted,
    BTree,
}

/// Minimal typestate for scoped handles tracked by the syntax checker.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HandleState {
    Live,
    Dropped,
    Detached,
    MovedOut,
}
