use crate::effect_row::{EffectRow, EffectRowTail};
use arcweft_character::id::{CharacterId, CharacterPartId};
use arcweft_lang_syntax::{
    ast::module_path::ModulePathRoot,
    expr::{IntSuffix, LifetimeScopeKind},
    reference::BorrowKind,
    types::TypePath,
};
use core::fmt;

mod character_nominal;
mod compatibility;
mod mismatch;
mod nominal;
mod openness;
mod order;
mod substitution;

pub use character_nominal::{CharacterNominalFamily, CharacterNominalType};
pub use mismatch::{TypeMismatch, TypeMismatchPathSegment, TypeMismatchReason};
pub use nominal::{
    AcceptedNominalType, DetachedTypeOwnerId, GenericTypeOwnerId, GenericTypeParameterId,
    OpenNominalType, ProjectNominalType, TypePoisonId,
};
pub(crate) use substitution::TypeParameterSubstitutions;

/// Statically known or deliberately unresolved length of an array type.
///
/// Array lengths are semantic values, not source spellings. In particular,
/// generic array lengths retain the declaration-owned parameter identity that
/// introduced them; they are never represented by a convention such as
/// `"N"`. `Inferred` is reserved for checker-local inference where no authored
/// length was supplied, while authored resolution failures retain their
/// diagnostic identity through `Error`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArrayLength {
    /// A concrete compile-time length.
    Const(usize),
    /// A declaration-owned generic constant length.
    Generic(GenericTypeParameterId),
    /// A resolver-owned error already reported for this length.
    Error(TypePoisonId),
    /// A checker-local length that remains to be inferred.
    Inferred,
}

impl ArrayLength {
    /// Returns whether this length leaves overload applicability open.
    #[must_use]
    pub(crate) const fn has_open_components(&self) -> bool {
        matches!(self, Self::Generic(_) | Self::Error(_) | Self::Inferred)
    }

    /// Returns whether this length is known to equal `actual`.
    #[must_use]
    pub(crate) const fn matches_const(&self, actual: usize) -> bool {
        match self {
            Self::Const(expected) => *expected == actual,
            Self::Generic(_) | Self::Error(_) | Self::Inferred => true,
        }
    }

    /// Returns whether an expected array length can accept an actual one.
    ///
    /// A concrete length is exact. Generic, recovery, and inference lengths
    /// deliberately remain open here; the owner-specific generic substitution
    /// machinery retains the identity when an operation needs to bind it.
    #[must_use]
    pub(crate) fn accepts(&self, actual: &Self) -> bool {
        match self {
            Self::Const(expected) => {
                matches!(actual, Self::Const(found) if expected == found)
                    || matches!(actual, Self::Error(_))
            }
            Self::Generic(_) | Self::Error(_) | Self::Inferred => true,
        }
    }

    /// Returns the diagnostic and tooling spelling for this semantic length.
    #[must_use]
    pub(crate) fn source_label(&self) -> String {
        match self {
            Self::Const(value) => value.to_string(),
            Self::Generic(parameter) => parameter.source_label(),
            Self::Error(poison) => format!("<array-length-error:{}>", poison.index()),
            Self::Inferred => "_".to_owned(),
        }
    }
}

impl fmt::Display for ArrayLength {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source_label())
    }
}

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
    /// Fixed entity families that may appear as contextual authored type atoms.
    pub const AUTHORED_FAMILIES: &'static [Self] = &[
        Self::Agent,
        Self::Entry,
        Self::Flow,
        Self::Choice,
        Self::ChoiceOption,
        Self::Character,
        Self::View,
        Self::Action,
        Self::Activity,
        Self::DialogueLine,
        Self::Text,
        Self::Content,
        Self::Input,
        Self::Button,
        Self::Style,
        Self::Asset,
        Self::Image,
        Self::Animation,
        Self::Capture,
        Self::Hook,
        Self::Signal,
        Self::Metric,
        Self::Scene,
        Self::Source,
        Self::Test,
        Self::Bench,
        Self::Layer,
        Self::Voice,
        Self::Se,
        Self::Bgm,
        Self::AudioBus,
        Self::MixerSnapshot,
        Self::Ducking,
        Self::Motion,
        Self::Rig,
        Self::Slot,
        Self::Target,
    ];

    /// Canonical source spelling for a fixed authored entity family.
    #[must_use]
    pub const fn authored_type_name(&self) -> Option<&'static str> {
        Some(match self {
            Self::Agent => "Agent",
            Self::Entry => "Entry",
            Self::Flow => "Flow",
            Self::Choice => "Choice",
            Self::ChoiceOption => "ChoiceOption",
            Self::Character => "Character",
            Self::View => "View",
            Self::Action => "Action",
            Self::Activity => "Activity",
            Self::DialogueLine => "DialogueLine",
            Self::Text => "Text",
            Self::Content => "Content",
            Self::Input => "Input",
            Self::Button => "Button",
            Self::Style => "Style",
            Self::Asset => "Asset",
            Self::Image => "Image",
            Self::Animation => "Animation",
            Self::Capture => "Capture",
            Self::Hook => "Hook",
            Self::Signal => "Signal",
            Self::Metric => "Metric",
            Self::Scene => "Scene",
            Self::Source => "Source",
            Self::Test => "Test",
            Self::Bench => "Bench",
            Self::Layer => "Layer",
            Self::Voice => "Voice",
            Self::Se => "Se",
            Self::Bgm => "Bgm",
            Self::AudioBus => "AudioBus",
            Self::MixerSnapshot => "MixerSnapshot",
            Self::Ducking => "Ducking",
            Self::Motion => "Motion",
            Self::Rig => "Rig",
            Self::Slot => "Slot",
            Self::Target => "Target",
            Self::Other(_) => return None,
        })
    }

    /// Resolves the canonical Arcweft type name for an entity family.
    #[must_use]
    pub fn from_type_name(name: &str) -> Option<Self> {
        Self::AUTHORED_FAMILIES
            .iter()
            .find(|family| family.authored_type_name() == Some(name))
            .cloned()
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
        len: ArrayLength,
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
    /// Generic parameter selected by a declaration-owned typed identity.
    GenericParam(GenericTypeParameterId),
    /// Source-backed project struct or enum selected through the project table.
    ProjectNominal(ProjectNominalType),
    /// Exact opaque type selected through the accepted environment catalog.
    AcceptedNominal(AcceptedNominalType),
    /// Type admitted by one explicit open-nominal environment rule.
    OpenNominal(OpenNominalType),
    /// Recovery carrier for a previously recorded authoritative type failure.
    Error(TypePoisonId),
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
    /// Internal or host-produced semantic value without an authored `TypeRef` origin.
    ///
    /// Authored type resolution must use `crate::nominal::resolve_type_ref` and
    /// never constructs this variant as a path fallback.
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

pub(crate) fn direct_type_name(path: &TypePath) -> Option<&str> {
    (path.root() == ModulePathRoot::ImplicitCrate && path.segments().len() == 1)
        .then(|| path.path().last_segment().as_str())
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
        if let Some(resolved) = self.resolve_nominal_effect_rows_with(resolve) {
            return resolved;
        }
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

    fn resolve_nominal_effect_rows_with<E>(
        &self,
        resolve: &mut impl FnMut(&EffectRow) -> Result<EffectRow, E>,
    ) -> Option<Result<Self, E>> {
        let mut resolve_arguments = |arguments: &[Self]| {
            arguments
                .iter()
                .map(|argument| argument.resolve_effect_rows_with(resolve))
                .collect::<Result<Vec<_>, E>>()
        };
        Some(match self {
            Self::ProjectNominal(nominal) => {
                resolve_arguments(nominal.arguments()).map(|arguments| {
                    Self::ProjectNominal(ProjectNominalType::new(
                        nominal.declaration().clone(),
                        arguments,
                    ))
                })
            }
            Self::AcceptedNominal(nominal) => {
                resolve_arguments(nominal.arguments()).map(|arguments| {
                    Self::AcceptedNominal(AcceptedNominalType::new(
                        nominal.declaration().clone(),
                        arguments,
                    ))
                })
            }
            Self::OpenNominal(nominal) => resolve_arguments(nominal.arguments()).map(|arguments| {
                Self::OpenNominal(OpenNominalType::new(
                    nominal.rule().clone(),
                    nominal.path().clone(),
                    arguments,
                ))
            }),
            _ => return None,
        })
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
            Self::GenericParam(parameter) => parameter.source_label(),
            Self::ProjectNominal(nominal) => nominal.source_label(),
            Self::AcceptedNominal(nominal) => nominal.source_label(),
            Self::OpenNominal(nominal) => nominal.source_label(),
            Self::Error(poison) => format!("<type-error:{}>", poison.index()),
            Self::Named(name) => name.clone(),
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
        flattened.sort_by(Self::stable_ordering);
        flattened.dedup();
        match flattened.as_slice() {
            [single] => single.clone(),
            _ => Self::Choice(flattened),
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
    fn authored_entity_families_round_trip_without_other() {
        assert!(!EntityKind::AUTHORED_FAMILIES.is_empty());
        for family in EntityKind::AUTHORED_FAMILIES {
            let name = family
                .authored_type_name()
                .expect("the authored inventory contains only fixed families");
            assert_eq!(EntityKind::from_type_name(name).as_ref(), Some(family));
        }
        assert_eq!(
            EntityKind::Other("Plugin".to_owned()).authored_type_name(),
            None
        );
        assert_eq!(EntityKind::from_type_name("Plugin"), None);
    }

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
