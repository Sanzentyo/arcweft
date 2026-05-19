use arcweft_lang_syntax::LifetimeScopeKind;

/// Entity family used by semantic references and ID checks.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
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

/// Minimal semantic type used by parser/HIR contract tests.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeKind {
    Bool,
    Int,
    Float,
    String,
    Char,
    TextCluster,
    Duration,
    Range,
    DisplayText,
    Ref(EntityKind),
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
    Unit,
    Never,
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
