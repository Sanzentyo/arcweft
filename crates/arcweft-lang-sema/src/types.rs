use crate::{
    effect_row::{EffectRow, EffectRowTail},
    effects::EffectSet,
};
use arcweft_character::id::{CharacterId, CharacterPartId};
use arcweft_lang_syntax::{
    ast::module_path::ModulePathRoot,
    expr::{IntSuffix, LifetimeScopeKind},
    reference::BorrowKind,
    types::{TypePath, TypeRef},
};
use core::fmt::{self, Write as _};

mod character_nominal;
mod compatibility;
mod mismatch;
mod openness;
mod order;

pub use character_nominal::{CharacterNominalFamily, CharacterNominalType};
pub use mismatch::{TypeMismatch, TypeMismatchPathSegment, TypeMismatchReason};

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

impl EntityKind {
    /// Resolves the canonical Arcweft type name for an entity family.
    #[must_use]
    pub fn from_type_name(name: &str) -> Option<Self> {
        Some(match name {
            "Agent" => Self::Agent,
            "Entry" => Self::Entry,
            "Flow" => Self::Flow,
            "Choice" => Self::Choice,
            "ChoiceOption" => Self::ChoiceOption,
            "Character" => Self::Character,
            "View" => Self::View,
            "Action" => Self::Action,
            "Activity" => Self::Activity,
            "DialogueLine" => Self::DialogueLine,
            "Text" => Self::Text,
            "Content" => Self::Content,
            "Input" => Self::Input,
            "Button" => Self::Button,
            "Style" => Self::Style,
            "Asset" => Self::Asset,
            "Image" => Self::Image,
            "Animation" => Self::Animation,
            "Capture" => Self::Capture,
            "Hook" => Self::Hook,
            "Signal" => Self::Signal,
            "Metric" => Self::Metric,
            "Scene" => Self::Scene,
            "Source" => Self::Source,
            "Test" => Self::Test,
            "Bench" => Self::Bench,
            "Layer" => Self::Layer,
            "Voice" => Self::Voice,
            "Se" => Self::Se,
            "Bgm" => Self::Bgm,
            "AudioBus" => Self::AudioBus,
            "MixerSnapshot" => Self::MixerSnapshot,
            "Ducking" => Self::Ducking,
            "Motion" => Self::Motion,
            "Rig" => Self::Rig,
            "Slot" => Self::Slot,
            "Target" => Self::Target,
            _ => return None,
        })
    }
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
        kind: BorrowKind,
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
        effects: EffectRow,
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
    /// Manifest-backed character enum with structural nominal identity.
    CharacterNominal(CharacterNominalType),
    Named(String),
    Tuple(Vec<TypeKind>),
    Choice(Vec<TypeKind>),
    Unit,
    Never,
}

/// Sema-owned classification of a value accepted as authored speaker-line sugar.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SpeakerLineType {
    Preset(EntityKind),
    Speaker(EntityKind),
}

impl From<IntSuffix> for TypeKind {
    fn from(suffix: IntSuffix) -> Self {
        match suffix {
            IntSuffix::I8 => Self::I8,
            IntSuffix::I16 => Self::I16,
            IntSuffix::I32 => Self::I32,
            IntSuffix::I64 => Self::I64,
            IntSuffix::I128 => Self::I128,
            IntSuffix::ISize => Self::ISize,
            IntSuffix::U8 => Self::U8,
            IntSuffix::U16 => Self::U16,
            IntSuffix::U32 => Self::U32,
            IntSuffix::U64 => Self::U64,
            IntSuffix::U128 => Self::U128,
            IntSuffix::USize => Self::USize,
        }
    }
}

impl From<&TypeRef> for TypeKind {
    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive surface-TypeRef projection is one closed conversion boundary"
    )]
    fn from(ty: &TypeRef) -> Self {
        match ty {
            TypeRef::Never => Self::Never,
            TypeRef::ConstInt(value) => Self::Named(value.to_string()),
            TypeRef::Path(path) => direct_type_name(path)
                .and_then(Self::primitive_name)
                .unwrap_or_else(|| Self::Named(path.canonical_string())),
            TypeRef::Tuple(items) => Self::Tuple(items.iter().map(Self::from).collect()),
            TypeRef::Function {
                params,
                return_type,
                effects,
            } => Self::function_with_effects(
                params.iter().map(Self::from),
                Self::from(return_type.as_ref()),
                effects.as_ref().map_or_else(EffectRow::unknown, |effects| {
                    EffectSet::from_labels(effects.effects())
                        .map_or_else(|_| EffectRow::unknown(), EffectRow::closed)
                }),
            ),
            TypeRef::Choice(alternatives) => {
                let mut flattened = alternatives
                    .iter()
                    .map(Self::from)
                    .flat_map(|ty| match ty {
                        Self::Choice(alternatives) => alternatives,
                        ty => vec![ty],
                    })
                    .collect::<Vec<_>>();
                flattened.sort_by_key(Self::source_label);
                flattened.dedup();
                match flattened.as_slice() {
                    [single] => single.clone(),
                    _ => Self::Choice(flattened),
                }
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Vec") && args.len() == 1 =>
            {
                Self::Vec(Box::new(Self::from(&args[0])))
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Array") && args.len() == 2 =>
            {
                Self::Array {
                    item: Box::new(Self::from(&args[0])),
                    len: canonical_type_ref_label(&args[1]),
                }
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Seq") && args.len() == 1 =>
            {
                Self::Seq(Box::new(Self::from(&args[0])))
            }
            TypeRef::Generic { base, args }
                if matches!(
                    direct_type_name(base),
                    Some("OrderedMap" | "SortedMap" | "BTreeMap")
                ) && args.len() == 2 =>
            {
                Self::Map {
                    kind: match direct_type_name(base) {
                        Some("OrderedMap") => MapKind::Ordered,
                        Some("SortedMap") => MapKind::Sorted,
                        Some("BTreeMap") => MapKind::BTree,
                        _ => unreachable!("map names are filtered by the match guard"),
                    },
                    key: Box::new(Self::from(&args[0])),
                    value: Box::new(Self::from(&args[1])),
                }
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Result") && args.len() == 2 =>
            {
                Self::Result {
                    ok: Box::new(Self::from(&args[0])),
                    error: Box::new(Self::from(&args[1])),
                }
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("ArcResult") && args.len() == 1 =>
            {
                Self::Result {
                    ok: Box::new(Self::from(&args[0])),
                    error: Box::new(Self::Named("ArcError".to_owned())),
                }
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Option") && args.len() == 1 =>
            {
                Self::Option(Box::new(Self::from(&args[0])))
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Speaker") && args.len() == 1 =>
            {
                entity_kind_from_type_ref(&args[0])
                    .map_or_else(|| Self::Named(canonical_type_ref_label(ty)), Self::Speaker)
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("SpeakerPreset") && args.len() == 1 =>
            {
                entity_kind_from_type_ref(&args[0]).map_or_else(
                    || Self::Named(canonical_type_ref_label(ty)),
                    Self::SpeakerPreset,
                )
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Need") && args.len() == 2 =>
            {
                Self::Need {
                    ready: Box::new(Self::from(&args[0])),
                    error: Box::new(Self::from(&args[1])),
                }
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Stream") && args.len() == 2 =>
            {
                Self::Stream {
                    item: Box::new(Self::from(&args[0])),
                    error: Box::new(Self::from(&args[1])),
                }
            }
            TypeRef::Generic { base, args }
                if direct_type_name(base) == Some("Source") && args.len() == 2 =>
            {
                Self::Source {
                    item: Box::new(Self::from(&args[0])),
                    error: Box::new(Self::from(&args[1])),
                }
            }
            TypeRef::Projection { subject, assoc } => Self::Projection {
                subject: Box::new(Self::from(subject.as_ref())),
                trait_name: None,
                assoc: assoc.as_str().to_owned(),
            },
            TypeRef::TraitBound(bound) => direct_type_name(bound.path())
                .and_then(Self::primitive_name)
                .unwrap_or_else(|| Self::Named(bound.path().canonical_string())),
            TypeRef::Reference(reference) => Self::BorrowRef {
                kind: reference.kind(),
                lifetime: reference
                    .region()
                    .name()
                    .map(|lifetime| LifetimeScopeKind::parse(lifetime.name())),
                inner: Box::new(Self::from(reference.referent())),
            },
            TypeRef::Slice(inner) => Self::Slice(Box::new(Self::from(inner.as_ref()))),
            TypeRef::Recovery(id) => Self::Named(format!("<recovered-type:{}>", id.index())),
            TypeRef::Generic { .. } => Self::Named(canonical_type_ref_label(ty)),
        }
    }
}

fn entity_kind_from_type_ref(ty: &TypeRef) -> Option<EntityKind> {
    let TypeRef::Path(path) = ty else {
        return None;
    };
    direct_type_name(path).and_then(EntityKind::from_type_name)
}

pub(crate) fn direct_type_name(path: &TypePath) -> Option<&str> {
    (path.root() == ModulePathRoot::ImplicitCrate && path.segments().len() == 1)
        .then(|| path.path().last_segment().as_str())
}

fn canonical_type_ref_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.canonical_string(),
        TypeRef::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(canonical_type_ref_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Function {
            params,
            return_type,
            effects,
        } => {
            let params = if params.len() == 1 {
                canonical_type_ref_label(&params[0])
            } else {
                format!(
                    "({})",
                    params
                        .iter()
                        .map(canonical_type_ref_label)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let mut label = format!("{params} -> {}", canonical_type_ref_label(return_type));
            if let Some(effects) = effects {
                let row = if effects.effects().is_empty() {
                    "{ }".to_owned()
                } else {
                    format!("{{ {} }}", effects.effects().join(", "))
                };
                write!(&mut label, " effects {row}")
                    .expect("writing canonical type text to String cannot fail");
            }
            label
        }
        TypeRef::Choice(alternatives) => alternatives
            .iter()
            .map(canonical_type_ref_label)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Generic { base, args } => format!(
            "{}<{}>",
            base.canonical_string(),
            args.iter()
                .map(canonical_type_ref_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::TraitBound(bound) => {
            let mut args = bound
                .args()
                .iter()
                .map(canonical_type_ref_label)
                .collect::<Vec<_>>();
            args.extend(bound.associated().iter().map(|binding| {
                format!(
                    "{} = {}",
                    binding.name().as_str(),
                    canonical_type_ref_label(binding.value())
                )
            }));
            format!("{}<{}>", bound.path(), args.join(", "))
        }
        TypeRef::Projection { subject, assoc } => {
            format!("{}::{}", canonical_type_ref_label(subject), assoc.as_str())
        }
        TypeRef::Reference(reference) => {
            let lifetime = reference
                .region()
                .name()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!(
                "&{lifetime}{}{}",
                reference.kind().source_qualifier(),
                canonical_type_ref_label(reference.referent())
            )
        }
        TypeRef::Slice(inner) => format!("[{}]", canonical_type_ref_label(inner)),
        TypeRef::Recovery(id) => format!("<recovered-type:{}>", id.index()),
    }
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
    /// Returns the entity family carried by an actual semantic preset type.
    #[must_use]
    pub const fn speaker_preset_entity_kind(&self) -> Option<&EntityKind> {
        match self {
            Self::SpeakerPreset(kind) => Some(kind),
            _ => None,
        }
    }

    /// Reports whether this semantic preset targets the expected entity family.
    #[must_use]
    pub fn is_speaker_preset_for(&self, expected: &EntityKind) -> bool {
        self.speaker_preset_entity_kind() == Some(expected)
    }

    /// Classifies only the semantic types accepted by speaker-line sugar.
    #[must_use]
    pub fn speaker_line_classification(&self) -> Option<SpeakerLineType> {
        match self {
            Self::SpeakerPreset(kind) => Some(SpeakerLineType::Preset(kind.clone())),
            Self::Speaker(kind) => Some(SpeakerLineType::Speaker(kind.clone())),
            Self::Ref(entity) if entity.kind() == &EntityKind::Character => {
                Some(SpeakerLineType::Speaker(EntityKind::Character))
            }
            _ => None,
        }
    }

    pub const ACTION_EVENT_TYPE_NAME: &'static str = "ActionEvent";

    pub(crate) fn resolve_effect_rows_with<E>(
        &self,
        resolve: &mut impl FnMut(&EffectRow) -> Result<EffectRow, E>,
    ) -> Result<Self, E> {
        let resolved = match self {
            Self::Range(inner) => Self::Range(Box::new(inner.resolve_effect_rows_with(resolve)?)),
            Self::IteratorState { family, item } => Self::IteratorState {
                family: *family,
                item: Box::new(item.resolve_effect_rows_with(resolve)?),
            },
            Self::Ref(entity) => Self::Ref(EntityType::new(
                entity.kind().clone(),
                entity
                    .value()
                    .map(|value| value.resolve_effect_rows_with(resolve))
                    .transpose()?,
            )),
            Self::Probe(inner) => Self::Probe(Box::new(inner.resolve_effect_rows_with(resolve)?)),
            Self::Vec(inner) => Self::Vec(Box::new(inner.resolve_effect_rows_with(resolve)?)),
            Self::Array { item, len } => Self::Array {
                item: Box::new(item.resolve_effect_rows_with(resolve)?),
                len: len.clone(),
            },
            Self::Slice(inner) => Self::Slice(Box::new(inner.resolve_effect_rows_with(resolve)?)),
            Self::Seq(inner) => Self::Seq(Box::new(inner.resolve_effect_rows_with(resolve)?)),
            Self::Map { kind, key, value } => Self::Map {
                kind: *kind,
                key: Box::new(key.resolve_effect_rows_with(resolve)?),
                value: Box::new(value.resolve_effect_rows_with(resolve)?),
            },
            Self::BorrowRef {
                kind,
                lifetime,
                inner,
            } => Self::BorrowRef {
                kind: *kind,
                lifetime: lifetime.clone(),
                inner: Box::new(inner.resolve_effect_rows_with(resolve)?),
            },
            Self::Need { ready, error } => Self::Need {
                ready: Box::new(ready.resolve_effect_rows_with(resolve)?),
                error: Box::new(error.resolve_effect_rows_with(resolve)?),
            },
            Self::Stream { item, error } => Self::Stream {
                item: Box::new(item.resolve_effect_rows_with(resolve)?),
                error: Box::new(error.resolve_effect_rows_with(resolve)?),
            },
            Self::Source { item, error } => Self::Source {
                item: Box::new(item.resolve_effect_rows_with(resolve)?),
                error: Box::new(error.resolve_effect_rows_with(resolve)?),
            },
            Self::Result { ok, error } => Self::Result {
                ok: Box::new(ok.resolve_effect_rows_with(resolve)?),
                error: Box::new(error.resolve_effect_rows_with(resolve)?),
            },
            Self::Option(inner) => Self::Option(Box::new(inner.resolve_effect_rows_with(resolve)?)),
            Self::ThreadHandle(inner) => {
                Self::ThreadHandle(Box::new(inner.resolve_effect_rows_with(resolve)?))
            }
            Self::Shared(inner) => Self::Shared(Box::new(inner.resolve_effect_rows_with(resolve)?)),
            Self::Function {
                params,
                return_type,
                effects,
            } => Self::Function {
                params: params
                    .iter()
                    .map(|param| param.resolve_effect_rows_with(resolve))
                    .collect::<Result<_, _>>()?,
                return_type: Box::new(return_type.resolve_effect_rows_with(resolve)?),
                effects: resolve(effects)?,
            },
            Self::Projection {
                subject,
                trait_name,
                assoc,
            } => Self::Projection {
                subject: Box::new(subject.resolve_effect_rows_with(resolve)?),
                trait_name: trait_name.clone(),
                assoc: assoc.clone(),
            },
            Self::Tuple(items) => Self::Tuple(
                items
                    .iter()
                    .map(|item| item.resolve_effect_rows_with(resolve))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Choice(alternatives) => Self::Choice(
                alternatives
                    .iter()
                    .map(|alternative| alternative.resolve_effect_rows_with(resolve))
                    .collect::<Result<_, _>>()?,
            ),
            // Every remaining variant is a leaf: it contains neither a nested
            // `TypeKind` nor an `EffectRow`, so resolution is identity.
            _ => self.clone(),
        };
        Ok(resolved)
    }

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
            Self::BorrowRef {
                kind,
                lifetime,
                inner,
            } => {
                let lifetime = lifetime
                    .as_ref()
                    .map(|lifetime| format!("'{} ", lifetime.as_str()))
                    .unwrap_or_default();
                format!(
                    "&{lifetime}{}{}",
                    kind.source_qualifier(),
                    inner.source_label()
                )
            }
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
                effects,
            } => Self::function_source_label(params, return_type, effects),
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
            Self::CharacterNominal(nominal) => nominal.source_label(),
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

    fn function_source_label(params: &[Self], return_type: &Self, effects: &EffectRow) -> String {
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
        let label = format!("{params} -> {}", return_type.source_label());
        function_effect_row_label(effects).map_or(label.clone(), |effects| {
            format!("{label} effects {effects}")
        })
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
    pub fn function(params: impl IntoIterator<Item = TypeKind>, return_type: TypeKind) -> Self {
        Self::function_with_effects(params, return_type, EffectRow::unknown())
    }

    #[must_use]
    pub fn function_with_effects(
        params: impl IntoIterator<Item = TypeKind>,
        return_type: TypeKind,
        effects: EffectRow,
    ) -> Self {
        Self::Function {
            params: params.into_iter().collect(),
            return_type: Box::new(return_type),
            effects,
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
    pub const fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 | Self::I128 | Self::ISize
        )
    }

    #[must_use]
    pub const fn is_unsigned_integer(&self) -> bool {
        matches!(
            self,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::U128 | Self::USize
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
    pub fn character_look(character: CharacterId) -> Self {
        Self::CharacterNominal(CharacterNominalType::Look { character })
    }

    /// Per-character enum type used for manifest-declared part ids.
    #[must_use]
    pub fn character_part(character: CharacterId) -> Self {
        Self::CharacterNominal(CharacterNominalType::Part { character })
    }

    /// Per-character, per-part enum type used for manifest-declared variants.
    #[must_use]
    pub fn character_variant(character: CharacterId, part: CharacterPartId) -> Self {
        Self::CharacterNominal(CharacterNominalType::Variant { character, part })
    }

    /// Manifest-backed character nominal identity, when this is one.
    #[must_use]
    pub const fn character_nominal(&self) -> Option<&CharacterNominalType> {
        match self {
            Self::CharacterNominal(nominal) => Some(nominal),
            _ => None,
        }
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

impl fmt::Display for TypeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source_label())
    }
}

#[cfg(test)]
mod speaker_line_tests {
    use super::{EntityKind, SpeakerLineType, TypeKind};

    #[test]
    fn semantic_types_are_the_only_speaker_line_classifier() {
        let character = EntityKind::Character;
        assert_eq!(
            TypeKind::SpeakerPreset(character.clone()).speaker_line_classification(),
            Some(SpeakerLineType::Preset(character.clone()))
        );
        assert_eq!(
            TypeKind::Speaker(character.clone()).speaker_line_classification(),
            Some(SpeakerLineType::Speaker(character.clone()))
        );
        assert_eq!(
            TypeKind::entity_ref(character.clone()).speaker_line_classification(),
            Some(SpeakerLineType::Speaker(character))
        );
        assert!(
            TypeKind::Named("SpeakerPreset".to_owned())
                .speaker_line_classification()
                .is_none()
        );
    }
}

fn function_effect_row_label(effects: &EffectRow) -> Option<String> {
    match effects.tail() {
        EffectRowTail::Unknown => None,
        EffectRowTail::Closed if effects.concrete().is_empty() => None,
        EffectRowTail::Closed | EffectRowTail::Variable(_) => Some(effects.display_label()),
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
