use arcweft_lang_syntax::expr::LifetimeScopeKind;

/// Entity family used by semantic references and ID checks.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
    Agent,
    Entry,
    Flow,
    Choice,
    ChoiceOption,
    Character,
    View,
    Action,
    Activity,
    Textbox,
    DialogueLine,
    Text,
    Content,
    Input,
    Button,
    Style,
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
    Bytes,
    TextCluster,
    Duration,
    Range(Box<TypeKind>),
    IteratorState {
        family: IteratorStateKind,
        item: Box<TypeKind>,
    },
    DisplayText,
    DebugStatePath,
    ObservationFieldPath,
    Ref(EntityType),
    Probe(Box<TypeKind>),
    Predicate,
    Observation,
    ObservedObject,
    AgentBBox,
    ActionName,
    ActionTarget,
    ActionResult,
    AgentValue,
    DataFormat,
    DataShape,
    AgentEntityMetadata,
    AgentSourceAnchor,
    AgentProjectGraphNeighborhood,
    AgentProjectGraphSymbol,
    AgentProjectGraphEdge,
    CaptureTarget,
    CaptureRef,
    AgentResource,
    AgentResourceBody,
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
        params: Vec<TypeKind>,
        return_type: Box<TypeKind>,
    },
    GenericParam(String),
    Projection {
        subject: Box<TypeKind>,
        trait_name: Option<String>,
        assoc: String,
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
    pub const ACTION_EVENT_TYPE_NAME: &'static str = "ActionEvent";

    /// Returns the canonical Arcweft surface spelling for this semantic type.
    ///
    /// This is intended for diagnostics and tooling displays, not for stable
    /// serialization. Function labels use the documented right-associative
    /// `A -> B` spelling and tuple call groups for multi-parameter functions.
    #[must_use]
    pub fn source_label(&self) -> String {
        if let Some(label) = self.atomic_source_label() {
            return label.to_owned();
        }

        match self {
            Self::Ref(entity) => entity.source_label(),
            Self::Probe(inner) => format!("Probe<{}>", inner.source_label()),
            Self::Range(inner) => format!("Range<{}>", inner.source_label()),
            Self::IteratorState { family, item } => {
                format!("{family:?}IteratorState<{}>", item.source_label())
            }
            Self::Vec(inner) => format!("Vec<{}>", inner.source_label()),
            Self::Array { item, len } => format!("Array<{}, {len}>", item.source_label()),
            Self::Slice(inner) => format!("[{}]", inner.source_label()),
            Self::Seq(inner) => format!("Seq<{}>", inner.source_label()),
            Self::Map { kind, key, value } => {
                format!("{kind:?}<{}, {}>", key.source_label(), value.source_label())
            }
            Self::BorrowRef { lifetime, inner } => lifetime.as_ref().map_or_else(
                || format!("&{}", inner.source_label()),
                |lifetime| format!("&{} {}", lifetime.as_str(), inner.source_label()),
            ),
            Self::Need { ready, error } => {
                format!("Need<{}, {}>", ready.source_label(), error.source_label())
            }
            Self::Stream { item, error } => {
                format!("Stream<{}, {}>", item.source_label(), error.source_label())
            }
            Self::Source { item, error } => {
                format!("Source<{}, {}>", item.source_label(), error.source_label())
            }
            Self::Result { ok, error } => {
                format!("Result<{}, {}>", ok.source_label(), error.source_label())
            }
            Self::Option(inner) => format!("Option<{}>", inner.source_label()),
            Self::Handle {
                name,
                lifetime,
                state,
                must_drop,
            } => format!(
                "Handle<{name}, {}, {state:?}, {must_drop}>",
                lifetime.as_str()
            ),
            Self::ThreadHandle(inner) => format!("ThreadHandle<{}>", inner.source_label()),
            Self::Shared(inner) => format!("Shared<{}>", inner.source_label()),
            Self::Function {
                params,
                return_type,
            } => Self::function_source_label(params, return_type),
            Self::GenericParam(name) | Self::Named(name) => name.clone(),
            Self::Projection {
                subject,
                trait_name,
                assoc,
            } => trait_name.as_ref().map_or_else(
                || format!("{}::{assoc}", subject.source_label()),
                |trait_name| format!("<{} as {trait_name}>::{assoc}", subject.source_label()),
            ),
            Self::Speaker(kind) => format!("Speaker<{kind:?}>"),
            Self::SpeakerPreset(kind) => format!("SpeakerPreset<{kind:?}>"),
            Self::CharacterPatch(kind) => format!("CharacterPatch<{kind:?}>"),
            Self::Tuple(items) => format!(
                "({})",
                items
                    .iter()
                    .map(Self::source_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Choice(alternatives) => alternatives
                .iter()
                .map(Self::source_label)
                .collect::<Vec<_>>()
                .join(" | "),
            _ => unreachable!("atomic type labels are handled before structured labels"),
        }
    }

    fn atomic_source_label(&self) -> Option<&'static str> {
        Some(match self {
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::ISize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::USize => "usize",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::String => "String",
            Self::Char => "char",
            Self::Bytes => "Bytes",
            Self::TextCluster => "TextCluster",
            Self::Duration => "Duration",
            Self::DisplayText => "DisplayText",
            Self::DebugStatePath => "DebugStatePath",
            Self::ObservationFieldPath => "ObservationFieldPath",
            Self::Predicate => "Predicate",
            Self::Observation => "Observation",
            Self::ObservedObject => "ObservedObject",
            Self::AgentBBox => "AgentBBox",
            Self::ActionName => "ActionName",
            Self::ActionTarget => "ActionTarget",
            Self::ActionResult => "ActionResult",
            Self::AgentValue => "AgentValue",
            Self::DataFormat => "DataFormat",
            Self::DataShape => "DataShape",
            Self::AgentEntityMetadata => "AgentEntityMetadata",
            Self::AgentSourceAnchor => "AgentSourceAnchor",
            Self::AgentProjectGraphNeighborhood => "AgentProjectGraphNeighborhood",
            Self::AgentProjectGraphSymbol => "AgentProjectGraphSymbol",
            Self::AgentProjectGraphEdge => "AgentProjectGraphEdge",
            Self::CaptureTarget => "CaptureTarget",
            Self::CaptureRef => "CaptureRef",
            Self::AgentResource => "AgentResource",
            Self::AgentResourceBody => "AgentResourceBody",
            Self::RagContextPack => "RagContextPack",
            Self::FocusPatch => "FocusPatch",
            Self::Unit => "Unit",
            Self::Never => "Never",
            _ => return None,
        })
    }

    fn function_source_label(params: &[Self], return_type: &Self) -> String {
        let params = if params.len() == 1 {
            params[0].source_label()
        } else {
            format!(
                "({})",
                params
                    .iter()
                    .map(Self::source_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        format!("{params} -> {}", return_type.source_label())
    }

    #[must_use]
    pub fn entity_ref(kind: EntityKind) -> Self {
        Self::Ref(EntityType::new(kind, None))
    }

    #[must_use]
    pub fn entity_ref_with_value(kind: EntityKind, value: TypeKind) -> Self {
        Self::Ref(EntityType::new(kind, Some(value)))
    }

    #[must_use]
    pub fn action_event() -> Self {
        Self::Named(Self::ACTION_EVENT_TYPE_NAME.to_owned())
    }

    #[must_use]
    pub fn function_arity(&self) -> Option<usize> {
        match self {
            Self::Function { params, .. } => Some(params.len()),
            _ => None,
        }
    }

    #[must_use]
    pub fn action_event_field(field: &str) -> Option<Self> {
        Some(match field {
            "action" => Self::entity_ref(EntityKind::Action),
            "value" => Self::String,
            _ => return None,
        })
    }

    #[must_use]
    pub fn presentation_handle(name: impl Into<String>) -> Self {
        Self::Handle {
            name: name.into(),
            lifetime: LifetimeScopeKind::Flow,
            state: HandleState::Live,
            must_drop: true,
        }
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

    /// Joins two control-flow branch result types using Arcweft expression
    /// branch rules.
    #[must_use]
    pub fn join_branch(left: Self, right: Self) -> Self {
        match (left, right) {
            (left, right) if left == right => left,
            (Self::Never, right) => right,
            (left, Self::Never) => left,
            (left, right) => Self::normalized_choice([left, right]),
        }
    }

    fn normalized_choice(alternatives: impl IntoIterator<Item = Self>) -> Self {
        let mut flattened = alternatives
            .into_iter()
            .flat_map(|ty| match ty {
                Self::Choice(alternatives) => alternatives,
                ty => vec![ty],
            })
            .collect::<Vec<_>>();
        flattened.sort_by_key(Self::choice_sort_label);
        flattened.dedup();
        match flattened.as_slice() {
            [single] => single.clone(),
            _ => Self::Choice(flattened),
        }
    }

    fn choice_sort_label(ty: &Self) -> String {
        match ty {
            Self::Bool => "bool".to_owned(),
            Self::I8 => "i8".to_owned(),
            Self::I16 => "i16".to_owned(),
            Self::I32 => "i32".to_owned(),
            Self::I64 => "i64".to_owned(),
            Self::I128 => "i128".to_owned(),
            Self::ISize => "isize".to_owned(),
            Self::U8 => "u8".to_owned(),
            Self::U16 => "u16".to_owned(),
            Self::U32 => "u32".to_owned(),
            Self::U64 => "u64".to_owned(),
            Self::U128 => "u128".to_owned(),
            Self::USize => "usize".to_owned(),
            Self::F32 => "f32".to_owned(),
            Self::F64 => "f64".to_owned(),
            Self::String => "String".to_owned(),
            Self::Char => "char".to_owned(),
            Self::Duration => "Duration".to_owned(),
            other => format!("{other:?}"),
        }
    }

    /// Per-character enum type used for manifest-declared look values.
    #[must_use]
    pub fn character_look(character: impl AsRef<str>) -> Self {
        Self::Named(format!("CharacterLook<{}>", character.as_ref()))
    }

    /// Per-character enum type used for manifest-declared part ids.
    #[must_use]
    pub fn character_part(character: impl AsRef<str>) -> Self {
        Self::Named(format!("CharacterPart<{}>", character.as_ref()))
    }

    /// Per-character, per-part enum type used for manifest-declared variants.
    #[must_use]
    pub fn character_variant(character: impl AsRef<str>, part: impl AsRef<str>) -> Self {
        Self::Named(format!(
            "CharacterVariant<{},{}>",
            character.as_ref(),
            part.as_ref()
        ))
    }

    /// Character id encoded by a `CharacterLook<...>` semantic type.
    pub fn character_look_character(&self) -> Option<&str> {
        let Self::Named(name) = self else {
            return None;
        };
        name.strip_prefix("CharacterLook<")?.strip_suffix('>')
    }

    #[must_use]
    pub fn primitive_name(name: &str) -> Option<Self> {
        Some(match name {
            "bool" => Self::Bool,
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
            "char" => Self::Char,
            "Bytes" => Self::Bytes,
            "DataFormat" => Self::DataFormat,
            "DataShape" => Self::DataShape,
            "AgentValue" => Self::AgentValue,
            "TextCluster" => Self::TextCluster,
            "Duration" => Self::Duration,
            "DebugStatePath" => Self::DebugStatePath,
            "ObservationFieldPath" => Self::ObservationFieldPath,
            "Unit" => Self::Unit,
            "Never" => Self::Never,
            _ => return None,
        })
    }
}

impl EntityType {
    fn source_label(&self) -> String {
        self.value().map_or_else(
            || format!("Ref<{:?}>", self.kind()),
            |value| format!("Ref<{:?}, {}>", self.kind(), value.source_label()),
        )
    }
}

/// Standard iterator state family used by semantic trait witnesses.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IteratorStateKind {
    Range,
    Seq,
    Stream,
    Vec,
    Array,
    Slice,
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
