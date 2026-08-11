# Exact Rust/API shapes

The snippets below are normative for the changed model. Existing unrelated enum
variants and methods remain unchanged.

## 1. Correct the existing owner enum

```rust
/// Closed language-owned type constructor set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinTypeConstructor {
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
    Unit,
    Never,
    Vec,
    Slice,
    Seq,
    Option,
    Probe,
    ThreadHandle,
    Shared,
    Array,
    OrderedMap,
    SortedMap,
    BTreeMap,
    Result,
    Need,
    Stream,
    Source,
    Ref,
    Speaker,
    SpeakerPreset,
}
```

```rust
impl BuiltinTypeConstructor {
    pub const ALL: &'static [Self] = &[
        Self::Bool,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::ISize,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::USize,
        Self::F32,
        Self::F64,
        Self::String,
        Self::Char,
        Self::Bytes,
        Self::Unit,
        Self::Never,
        Self::Vec,
        Self::Slice,
        Self::Seq,
        Self::Option,
        Self::Probe,
        Self::ThreadHandle,
        Self::Shared,
        Self::Array,
        Self::OrderedMap,
        Self::SortedMap,
        Self::BTreeMap,
        Self::Result,
        Self::Need,
        Self::Stream,
        Self::Source,
        Self::Ref,
        Self::Speaker,
        Self::SpeakerPreset,
    ];

    pub const ENTITY_FAMILY_PROJECTIONS: &'static [Self] = &[
        Self::Ref,
        Self::Speaker,
        Self::SpeakerPreset,
    ];

    pub const fn spelling(self) -> &'static str {
        match self {
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
            Self::Unit => "Unit",
            Self::Never => "Never",
            Self::Vec => "Vec",
            Self::Slice => "Slice",
            Self::Seq => "Seq",
            Self::Option => "Option",
            Self::Probe => "Probe",
            Self::ThreadHandle => "ThreadHandle",
            Self::Shared => "Shared",
            Self::Array => "Array",
            Self::OrderedMap => "OrderedMap",
            Self::SortedMap => "SortedMap",
            Self::BTreeMap => "BTreeMap",
            Self::Result => "Result",
            Self::Need => "Need",
            Self::Stream => "Stream",
            Self::Source => "Source",
            Self::Ref => "Ref",
            Self::Speaker => "Speaker",
            Self::SpeakerPreset => "SpeakerPreset",
        }
    }

    pub const fn arity(self) -> u16 {
        match self {
            Self::Bool
            | Self::I8
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
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::Unit
            | Self::Never => 0,
            Self::Vec
            | Self::Slice
            | Self::Seq
            | Self::Option
            | Self::Probe
            | Self::ThreadHandle
            | Self::Shared
            | Self::Ref
            | Self::Speaker
            | Self::SpeakerPreset => 1,
            Self::Array
            | Self::OrderedMap
            | Self::SortedMap
            | Self::BTreeMap
            | Self::Result
            | Self::Need
            | Self::Stream
            | Self::Source => 2,
        }
    }

    pub const fn argument_expectation(
        self,
        argument: u16,
    ) -> Option<TypeArgumentExpectation> {
        if argument >= self.arity() {
            return None;
        }
        match (self, argument) {
            (Self::Array, 1) => Some(TypeArgumentExpectation::ConstInt),
            (
                Self::Ref | Self::Speaker | Self::SpeakerPreset,
                0,
            ) => Some(TypeArgumentExpectation::EntityFamily),
            _ => Some(TypeArgumentExpectation::Type),
        }
    }

    pub fn project_entity_family(self, family: EntityKind) -> Option<TypeKind> {
        Some(match self {
            Self::Ref => TypeKind::entity_ref(family),
            Self::Speaker => TypeKind::Speaker(family),
            Self::SpeakerPreset => TypeKind::SpeakerPreset(family),
            _ => return None,
        })
    }

    pub fn from_type_path(path: &TypePath) -> Option<Self> {
        if path.root() != ModulePathRoot::ImplicitCrate {
            return None;
        }
        let [segment] = path.segments() else {
            return None;
        };
        Self::ALL
            .iter()
            .copied()
            .find(|constructor| constructor.spelling() == segment.as_str())
    }
}
```

The resolver-local free `builtin(path)` function is deleted. Selection,
expectation, and projection are all inherent behavior of the existing owner
enum.

## 2. Make the authored entity inventory bidirectional

```rust
impl EntityKind {
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

    #[must_use]
    pub fn authored_type_name(&self) -> Option<&'static str> {
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

    #[must_use]
    pub fn from_type_name(name: &str) -> Option<Self> {
        Self::AUTHORED_FAMILIES
            .iter()
            .find(|family| family.authored_type_name() == Some(name))
            .cloned()
    }
}
```

This is the one forward/reverse authored inventory. It never constructs
`Other(String)` from source spelling.

## 3. Represent actual argument kinds without pretending

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeArgumentKind {
    Type(TypeKind),
    ConstInt(usize),
    EntityFamily(EntityKind),
}
```

The changed failure and diagnostic fields are exact:

```rust
WrongArgumentKind {
    target: TypeArityTarget,
    argument: u16,
    expected: TypeArgumentExpectation,
    actual: TypeArgumentKind,
}
```

This shape replaces the current `actual: TypeKind` field in both
`TypeResolutionFailure` and `NominalTypeDiagnosticKind`. All other variants are
unchanged. `NominalTypeDiagnosticKind::code()` still maps it to
`NominalTypeDiagnosticCode::WrongKind`.

The existing deterministic `Ord` implementation compares `target`, `argument`,
and `expected`, then compares `TypeArgumentKind` by variant rank
`Type < ConstInt < EntityFamily`; contained `TypeKind` uses its existing
`stable_ordering`, integers use numeric ordering, and entity families use
`authored_type_name` followed by the `Other` payload for the internal-only
case.

## 4. Keep existing outer/child owner facts

No parallel resolution enum or arity target is added:

```rust
TypeNameResolution::Builtin(BuiltinTypeConstructor::Ref)
TypeNameResolution::EntityFamily(EntityKind::Character)
TypeArityTarget::Builtin(BuiltinTypeConstructor::Ref)
```

## 5. Add behavior to the internal node carrier

```rust
impl NodeValue {
    fn argument_kind(&self) -> Option<TypeArgumentKind> {
        if let Some(family) = &self.entity_family {
            return Some(TypeArgumentKind::EntityFamily(family.clone()));
        }
        if let Some(value) = self.const_int {
            return Some(TypeArgumentKind::ConstInt(value));
        }
        match &self.ty {
            Some(TypeKind::Error(_)) | None => None,
            Some(ty) => Some(TypeArgumentKind::Type(ty.clone())),
        }
    }
}
```

A successful `NodeValue` has exactly one of these categories. An error value
propagates its existing poison and does not become a second wrong-kind failure.

## 6. Replace the unary-chain boolean with the typed expectation

```rust
struct SingleArgumentGenericFrame<'a> {
    path: TypeRefNodePath,
    child: TypeRefNodePath,
    base: &'a TypePath,
    depth: u16,
    argument_expectation: Option<TypeArgumentExpectation>,
}
```

```rust
let argument_expectation = BuiltinTypeConstructor::from_type_path(base)
    .and_then(|constructor| constructor.argument_expectation(0));
```

The leaf uses `resolve_entity_family_node` only when the final frame’s
expectation is `Some(TypeArgumentExpectation::EntityFamily)`. The general
generic traversal performs the same per-index lookup. Excess arguments have
`None` and are resolved normally before wrong arity is emitted.

## 7. Apply the closed projection

```rust
fn apply_entity_family_builtin(
    &mut self,
    context: &SourceContext<'_>,
    constructor: BuiltinTypeConstructor,
    (path, value): (TypeRefNodePath, NodeValue),
    child_causes: Vec<TypePoisonId>,
    target: TypeArityTarget,
) -> NodeValue {
    if let Some(family) = value.entity_family.as_ref() {
        let ty = constructor
            .project_entity_family(family.clone())
            .expect("entity-family dispatch is closed by the owner enum");
        return NodeValue::typed(ty, child_causes);
    }

    if let Some(TypeKind::Error(poison)) = value.ty.as_ref() {
        return NodeValue::error(*poison, child_causes);
    }

    if let Some(actual) = value.argument_kind() {
        let failure = TypeResolutionFailure::WrongArgumentKind {
            target,
            argument: 0,
            expected: TypeArgumentExpectation::EntityFamily,
            actual,
        };
        let poison = self.emit_failure(
            &failure,
            context.evidence(&path, true),
            Vec::new(),
        );
        self.replace_node_outcome(&path, TypeNameResolution::Failed(failure));
        return NodeValue::error(poison, child_causes);
    }

    let poison = value
        .causes
        .first()
        .copied()
        .expect("a valueless argument is already poisoned");
    NodeValue::error(poison, child_causes)
}
```

`apply_builtin` dispatches `Ref | Speaker | SpeakerPreset` to this function and
uses `project_entity_family`; it does not compare spellings and does not use an
`if constructor == Speaker { ... } else { ... }` binary assumption.

## 8. Reservation gates

Add `"Ref"` to the existing exhaustive direct-name matches in:

- HIR project symbol reservation;
- accepted exact/open reserved paths.

Do not introduce a sema dependency into HIR solely to share the literal.
Behavior tests keep the layer-local gates aligned.
