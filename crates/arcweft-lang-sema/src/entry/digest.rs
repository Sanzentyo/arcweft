use arcweft_data::{BytesFormat, EnumRepr, EnumTagStyle, FieldShape, TypeShape, VariantShape};
use arcweft_lang_hir::{
    item::HirRoutePathSegment,
    symbol::{CallableDeclarationId, CallablePackageId},
};

use crate::{
    callable::{CallableGroupKind, CallableParameterPassing, CallableParameterPresence},
    effects::{EffectId, EffectSet},
};

use super::{
    AgentBudget, BoundNominalKind, BoundNominalTypeKey, CallableContractDigest, CheckedAgentPolicy,
    CheckedAgentPolicyDigest, CheckedEntryBindingDigest, CheckedEntryFlowTarget, CheckedEntryId,
    CheckedEntryKind, CheckedEntryRouteBindingSource, CheckedExistingEntryTarget, CheckedFlowId,
    CheckedStatefulEntryKind, FlowContractDigest, NominalSchemaDigest,
};

const VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum CanonicalType {
    Atomic(CanonicalAtomic),
    Named(String),
    Nominal(BoundNominalTypeKey),
    Applied {
        constructor: CanonicalConstructor,
        args: Vec<Self>,
    },
    ConstInt(u64),
    Tuple(Vec<Self>),
    Choice(Vec<Self>),
    Borrow {
        kind: u8,
        lifetime: Option<String>,
        inner: Box<Self>,
    },
    Function {
        params: Vec<Self>,
        result: Box<Self>,
        effects: CanonicalEffectRow,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum CanonicalAtomic {
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
    DataFormat,
    DataShape,
    AgentValue,
    TextCluster,
    Duration,
    DebugStatePath,
    ObservationFieldPath,
    ReducerError,
    AgentError,
    ArcError,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum CanonicalConstructor {
    Vec,
    Array,
    Slice,
    Seq,
    OrderedMap,
    SortedMap,
    BTreeMap,
    Result,
    Option,
    Need,
    Stream,
    Reduction,
    Ref,
    Probe,
    ThreadHandle,
    Shared,
}

impl CanonicalType {
    pub(super) fn source_label(&self) -> String {
        match self {
            Self::Atomic(atomic) => atomic.source_label().to_owned(),
            Self::Named(name) => name.clone(),
            Self::Nominal(key) => format!("{}::{}", key.module(), key.name()),
            Self::Applied { constructor, args } => format!(
                "{}<{}>",
                constructor.source_label(),
                args.iter()
                    .map(Self::source_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::ConstInt(value) => value.to_string(),
            Self::Tuple(items) => format!(
                "({})",
                items
                    .iter()
                    .map(Self::source_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Choice(items) => items
                .iter()
                .map(Self::source_label)
                .collect::<Vec<_>>()
                .join(" | "),
            Self::Borrow {
                kind,
                lifetime,
                inner,
            } => {
                let qualifier = match kind {
                    1 => "",
                    2 => "mut ",
                    _ => "? ",
                };
                let lifetime = lifetime
                    .as_ref()
                    .map(|lifetime| format!("'{lifetime} "))
                    .unwrap_or_default();
                format!("&{lifetime}{qualifier}{}", inner.source_label())
            }
            Self::Function {
                params,
                result,
                effects,
            } => format!(
                "fn({}) -> {} effects {{{}}}",
                params
                    .iter()
                    .map(Self::source_label)
                    .collect::<Vec<_>>()
                    .join(", "),
                result.source_label(),
                effects.effects.join(", ")
            ),
        }
    }
}

impl CanonicalAtomic {
    const fn source_label(self) -> &'static str {
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
            Self::DataFormat => "DataFormat",
            Self::DataShape => "DataShape",
            Self::AgentValue => "AgentValue",
            Self::TextCluster => "TextCluster",
            Self::Duration => "Duration",
            Self::DebugStatePath => "DebugStatePath",
            Self::ObservationFieldPath => "ObservationFieldPath",
            Self::ReducerError => "ReducerError",
            Self::AgentError => "AgentError",
            Self::ArcError => "ArcError",
        }
    }
}

impl CanonicalConstructor {
    const fn source_label(self) -> &'static str {
        match self {
            Self::Vec => "Vec",
            Self::Array => "Array",
            Self::Slice => "Slice",
            Self::Seq => "Seq",
            Self::OrderedMap => "OrderedMap",
            Self::SortedMap => "SortedMap",
            Self::BTreeMap => "BTreeMap",
            Self::Result => "Result",
            Self::Option => "Option",
            Self::Need => "Need",
            Self::Stream => "Stream",
            Self::Reduction => "Reduction",
            Self::Ref => "Ref",
            Self::Probe => "Probe",
            Self::ThreadHandle => "ThreadHandle",
            Self::Shared => "Shared",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CanonicalFlowSuspension {
    Flow,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CanonicalEffectRow {
    pub(super) effects: Vec<String>,
    pub(super) tail: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalParameter {
    pub(super) passing: CallableParameterPassing,
    pub(super) presence: CallableParameterPresence,
    pub(super) receiver: u8,
    pub(super) ty: CanonicalType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalParameterGroup {
    pub(super) kind: CallableGroupKind,
    pub(super) parameters: Vec<CanonicalParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CanonicalGenericParameter {
    Lifetime(String),
    Type {
        name: String,
        bounds: Vec<CanonicalType>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalWherePredicate {
    pub(super) subject: CanonicalType,
    pub(super) bounds: Vec<CanonicalType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalSignature {
    pub(super) generics: Vec<CanonicalGenericParameter>,
    pub(super) groups: Vec<CanonicalParameterGroup>,
    pub(super) result: Option<CanonicalType>,
    pub(super) where_predicates: Vec<CanonicalWherePredicate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalCallableContract {
    pub(super) signature: CanonicalSignature,
    pub(super) contract_effects: EffectSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CanonicalFlowContract {
    pub(super) signature: Option<CanonicalSignature>,
    pub(super) contract_effects: EffectSet,
    pub(super) suspension: CanonicalFlowSuspension,
}

pub(super) fn nominal_schema(shape: &TypeShape) -> NominalSchemaDigest {
    NominalSchemaDigest::from_bytes(blake3::hash(&nominal_schema_bytes(shape)).into())
}

fn nominal_schema_bytes(shape: &TypeShape) -> Vec<u8> {
    let mut bytes = CanonicalBytes::domain(b"arcweft.nominal-schema\0");
    bytes.type_shape(shape);
    bytes.finish()
}

pub(super) fn callable_contract(contract: &CanonicalCallableContract) -> CallableContractDigest {
    CallableContractDigest::from_bytes(blake3::hash(&callable_contract_bytes(contract)).into())
}

fn callable_contract_bytes(contract: &CanonicalCallableContract) -> Vec<u8> {
    let mut bytes = CanonicalBytes::domain(b"arcweft.callable-contract\0");
    bytes.signature(&contract.signature);
    bytes.effect_set(&contract.contract_effects);
    bytes.finish()
}

pub(super) fn flow_contract(
    id: &CheckedFlowId,
    contract: &CanonicalFlowContract,
) -> FlowContractDigest {
    FlowContractDigest::from_bytes(blake3::hash(&flow_contract_bytes(id, contract)).into())
}

fn flow_contract_bytes(id: &CheckedFlowId, contract: &CanonicalFlowContract) -> Vec<u8> {
    let mut bytes = CanonicalBytes::domain(b"arcweft.flow-contract\0");
    bytes.string(id.public_id().as_str());
    bytes.fixed(id.declaration_digest().as_bytes());
    bytes.option(contract.signature.as_ref(), |bytes, signature| {
        bytes.signature(signature);
    });
    bytes.effect_set(&contract.contract_effects);
    bytes.u8(match contract.suspension {
        CanonicalFlowSuspension::Flow => 1,
    });
    bytes.finish()
}

pub(super) fn agent_policy(
    policy: &CheckedAgentPolicy,
    budget: AgentBudget,
) -> CheckedAgentPolicyDigest {
    CheckedAgentPolicyDigest::from_bytes(blake3::hash(&agent_policy_bytes(policy, budget)).into())
}

fn agent_policy_bytes(policy: &CheckedAgentPolicy, budget: AgentBudget) -> Vec<u8> {
    let mut bytes = CanonicalBytes::domain(b"arcweft.agent-policy\0");
    bytes.effect_set(policy.allowed_effects());
    bytes.effect_set(policy.inferred_effects());
    bytes.u64(budget.logical_timeout_millis());
    bytes.u64(budget.max_vm_steps());
    bytes.u32(budget.max_host_calls());
    bytes.u32(budget.max_observations());
    bytes.u32(budget.max_captures());
    bytes.u64(budget.max_capture_bytes());
    bytes.u32(budget.max_rag_queries());
    bytes.u64(budget.max_context_bytes());
    bytes.finish()
}

#[derive(Clone, Copy)]
pub(super) struct StatefulBindingInput<'a> {
    pub(super) package: &'a CallablePackageId,
    pub(super) id: &'a CheckedEntryId,
    pub(super) kind: CheckedStatefulEntryKind,
    pub(super) state: (&'a BoundNominalTypeKey, &'a NominalSchemaDigest),
    pub(super) initializer: (&'a CallableDeclarationId, &'a CallableContractDigest),
    pub(super) event: (&'a BoundNominalTypeKey, &'a NominalSchemaDigest),
    pub(super) reducer: (&'a CallableDeclarationId, &'a CallableContractDigest),
    pub(super) initial_flow: (&'a CheckedFlowId, &'a FlowContractDigest),
}

pub(super) fn stateful_binding(input: StatefulBindingInput<'_>) -> CheckedEntryBindingDigest {
    CheckedEntryBindingDigest::from_bytes(blake3::hash(&stateful_binding_bytes(&input)).into())
}

fn stateful_binding_bytes(input: &StatefulBindingInput<'_>) -> Vec<u8> {
    let mut bytes = CanonicalBytes::domain(b"arcweft.checked-entry-binding\0");
    bytes.u8(1);
    bytes.string(input.package.as_str());
    bytes.string(input.id.public_id().as_str());
    bytes.u8(input.kind.as_checked().canonical_tag());
    bytes.nominal_key(input.state.0);
    bytes.fixed(input.state.1.as_bytes());
    bytes.callable_id(input.initializer.0);
    bytes.fixed(input.initializer.1.as_bytes());
    bytes.nominal_key(input.event.0);
    bytes.fixed(input.event.1.as_bytes());
    bytes.callable_id(input.reducer.0);
    bytes.fixed(input.reducer.1.as_bytes());
    bytes.string(input.initial_flow.0.public_id().as_str());
    bytes.fixed(input.initial_flow.0.declaration_digest().as_bytes());
    bytes.fixed(input.initial_flow.1.as_bytes());
    bytes.finish()
}

pub(super) fn agent_binding(
    package: &CallablePackageId,
    id: &CheckedEntryId,
    controller: (&CallableDeclarationId, &CallableContractDigest),
    policy: &CheckedAgentPolicyDigest,
) -> CheckedEntryBindingDigest {
    CheckedEntryBindingDigest::from_bytes(
        blake3::hash(&agent_binding_bytes(package, id, controller, policy)).into(),
    )
}

fn agent_binding_bytes(
    package: &CallablePackageId,
    id: &CheckedEntryId,
    controller: (&CallableDeclarationId, &CallableContractDigest),
    policy: &CheckedAgentPolicyDigest,
) -> Vec<u8> {
    let mut bytes = CanonicalBytes::domain(b"arcweft.checked-entry-binding\0");
    bytes.u8(2);
    bytes.string(package.as_str());
    bytes.string(id.public_id().as_str());
    bytes.u8(CheckedEntryKind::Agent.canonical_tag());
    bytes.callable_id(controller.0);
    bytes.fixed(controller.1.as_bytes());
    bytes.fixed(policy.as_bytes());
    bytes.finish()
}

pub(super) fn existing_binding(
    package: &CallablePackageId,
    id: &CheckedEntryId,
    kind: &CheckedEntryKind,
    target: &CheckedExistingEntryTarget,
) -> CheckedEntryBindingDigest {
    CheckedEntryBindingDigest::from_bytes(
        blake3::hash(&existing_binding_bytes(package, id, kind, target)).into(),
    )
}

fn existing_binding_bytes(
    package: &CallablePackageId,
    id: &CheckedEntryId,
    kind: &CheckedEntryKind,
    target: &CheckedExistingEntryTarget,
) -> Vec<u8> {
    let mut bytes = CanonicalBytes::domain(b"arcweft.checked-entry-binding\0");
    bytes.u8(3);
    bytes.string(package.as_str());
    bytes.string(id.public_id().as_str());
    bytes.u8(kind.canonical_tag());
    bytes.option(kind.custom_payload(), CanonicalBytes::string);
    match target {
        CheckedExistingEntryTarget::Flow(flow) => {
            bytes.u8(1);
            bytes.entry_flow_target(flow);
        }
        CheckedExistingEntryTarget::Routes(routes) => {
            bytes.u8(2);
            bytes.len(routes.len());
            for route in routes {
                bytes.string(route.method().as_str());
                bytes.len(route.path().segments().len());
                for segment in route.path().segments() {
                    match segment {
                        HirRoutePathSegment::Literal(literal) => {
                            bytes.u8(1);
                            bytes.string(literal);
                        }
                        HirRoutePathSegment::Capture(_) => bytes.u8(2),
                    }
                }
                bytes.entry_flow_target(route.target());
                bytes.len(route.bindings().len());
                for binding in route.bindings() {
                    bytes.u32(binding.parameter().position());
                    match binding.source() {
                        CheckedEntryRouteBindingSource::PathCapture(capture) => {
                            bytes.u8(1);
                            bytes.u32(capture.position());
                        }
                    }
                }
            }
        }
    }
    bytes.finish()
}

struct CanonicalBytes(Vec<u8>);

impl CanonicalBytes {
    fn domain(domain: &[u8]) -> Self {
        let mut this = Self(Vec::with_capacity(256));
        this.0.extend_from_slice(domain);
        this.u32(VERSION);
        this
    }

    fn finish(self) -> Vec<u8> {
        self.0
    }

    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn i128(&mut self, value: i128) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) {
        self.u32(u32::try_from(value).expect("semantic collection length must fit u32"));
    }

    fn string(&mut self, value: &str) {
        self.len(value.len());
        self.0.extend_from_slice(value.as_bytes());
    }

    fn fixed(&mut self, value: &[u8; 32]) {
        self.0.extend_from_slice(value);
    }

    fn option<T: ?Sized>(&mut self, value: Option<&T>, encode: impl FnOnce(&mut Self, &T)) {
        match value {
            Some(value) => {
                self.u8(1);
                encode(self, value);
            }
            None => self.u8(0),
        }
    }

    fn nominal_key(&mut self, key: &BoundNominalTypeKey) {
        self.string(key.package().as_str());
        self.string(&key.module().to_string());
        self.string(key.name());
        self.u8(match key.kind() {
            BoundNominalKind::Struct => 1,
            BoundNominalKind::Enum => 2,
        });
    }

    fn callable_id(&mut self, id: &CallableDeclarationId) {
        self.string(id.package().as_str());
        self.string(&id.module().to_string());
        self.u8(id.owner().digest_tag().saturating_add(1));
        self.len(id.owner_path().len());
        for segment in id.owner_path() {
            self.string(segment.as_str());
        }
        self.string(id.name());
    }

    fn entry_flow_target(&mut self, target: &CheckedEntryFlowTarget) {
        self.fixed(target.id().declaration_digest().as_bytes());
        self.fixed(target.contract_digest().as_bytes());
        self.len(target.parameters().len());
        for parameter in target.parameters() {
            self.u32(parameter.coordinate().position());
            self.fixed(parameter.semantic_type().as_bytes());
        }
    }

    fn type_shape(&mut self, shape: &TypeShape) {
        match shape {
            TypeShape::Unit => self.u8(1),
            TypeShape::Bool => self.u8(2),
            TypeShape::I8 => self.u8(3),
            TypeShape::I16 => self.u8(4),
            TypeShape::I32 => self.u8(5),
            TypeShape::I64 => self.u8(6),
            TypeShape::I128 => self.u8(7),
            TypeShape::Isize => self.u8(8),
            TypeShape::U8 => self.u8(9),
            TypeShape::U16 => self.u8(10),
            TypeShape::U32 => self.u8(11),
            TypeShape::U64 => self.u8(12),
            TypeShape::U128 => self.u8(13),
            TypeShape::Usize => self.u8(14),
            TypeShape::F32 => self.u8(15),
            TypeShape::F64 => self.u8(16),
            TypeShape::String => self.u8(17),
            TypeShape::Char => self.u8(18),
            TypeShape::Bytes { format } => {
                self.u8(19);
                self.u8(bytes_format_tag(*format));
            }
            TypeShape::Option(inner) => {
                self.u8(20);
                self.type_shape(inner);
            }
            TypeShape::Seq(inner) => {
                self.u8(21);
                self.type_shape(inner);
            }
            TypeShape::Map { key, value } => {
                self.u8(22);
                self.type_shape(key);
                self.type_shape(value);
            }
            TypeShape::Record {
                name,
                fields,
                policy,
            } => {
                self.u8(23);
                self.string(name);
                self.bool(policy.deny_unknown_fields);
                self.fields(fields);
            }
            TypeShape::Enum {
                name,
                variants,
                tag,
                repr,
            } => {
                self.u8(24);
                self.string(name);
                self.enum_tag(tag);
                self.option(repr.as_ref(), |bytes, repr| bytes.u8(enum_repr_tag(*repr)));
                self.variants(variants);
            }
            TypeShape::Named(name) => {
                self.u8(25);
                self.string(name);
            }
        }
    }

    fn fields(&mut self, fields: &[FieldShape]) {
        self.len(fields.len());
        for field in fields {
            self.string(&field.rust_name);
            self.string(&field.wire_name);
            self.type_shape(&field.shape);
            self.bool(field.has_default);
            self.bool(field.skip);
            self.option(field.bytes_format.as_ref(), |bytes, format| {
                bytes.u8(bytes_format_tag(*format));
            });
        }
    }

    fn variants(&mut self, variants: &[VariantShape]) {
        self.len(variants.len());
        for variant in variants {
            self.string(&variant.rust_name);
            self.string(&variant.wire_name);
            self.option(variant.payload.as_ref(), Self::type_shape);
            self.option(variant.discriminant.as_ref(), |bytes, value| {
                bytes.i128(*value);
            });
        }
    }

    fn enum_tag(&mut self, tag: &EnumTagStyle) {
        match tag {
            EnumTagStyle::External => self.u8(1),
            EnumTagStyle::Internal { tag } => {
                self.u8(2);
                self.string(tag);
            }
            EnumTagStyle::Adjacent { tag, content } => {
                self.u8(3);
                self.string(tag);
                self.string(content);
            }
        }
    }

    fn signature(&mut self, signature: &CanonicalSignature) {
        self.len(signature.generics.len());
        for generic in &signature.generics {
            match generic {
                CanonicalGenericParameter::Lifetime(name) => {
                    self.u8(1);
                    self.string(name);
                }
                CanonicalGenericParameter::Type { name, bounds } => {
                    self.u8(2);
                    self.string(name);
                    self.canonical_types(bounds);
                }
            }
        }
        self.len(signature.groups.len());
        for group in &signature.groups {
            self.u8(match group.kind {
                CallableGroupKind::Initial => 1,
                CallableGroupKind::Curried => 2,
            });
            self.len(group.parameters.len());
            for parameter in &group.parameters {
                self.u8(match parameter.passing {
                    CallableParameterPassing::PositionalOnly => 1,
                    CallableParameterPassing::PositionalOrNamed => 2,
                    CallableParameterPassing::NamedOnly => 3,
                    CallableParameterPassing::RestPositional => 4,
                    CallableParameterPassing::RestNamed => 5,
                });
                self.u8(match parameter.presence {
                    CallableParameterPresence::Required => 1,
                    CallableParameterPresence::Optional => 2,
                    CallableParameterPresence::Defaulted => 3,
                });
                self.u8(parameter.receiver);
                self.canonical_type(&parameter.ty);
            }
        }
        self.option(signature.result.as_ref(), Self::canonical_type);
        self.len(signature.where_predicates.len());
        for predicate in &signature.where_predicates {
            self.canonical_type(&predicate.subject);
            self.canonical_types(&predicate.bounds);
        }
    }

    fn canonical_type(&mut self, ty: &CanonicalType) {
        match ty {
            CanonicalType::Atomic(atomic) => {
                self.u8(1);
                self.u8(canonical_atomic_tag(*atomic));
            }
            CanonicalType::Named(name) => {
                self.u8(2);
                self.string(name);
            }
            CanonicalType::Nominal(key) => {
                self.u8(3);
                self.nominal_key(key);
            }
            CanonicalType::Applied { constructor, args } => {
                self.u8(4);
                self.u8(canonical_constructor_tag(*constructor));
                self.canonical_types(args);
            }
            CanonicalType::ConstInt(value) => {
                self.u8(9);
                self.u64(*value);
            }
            CanonicalType::Tuple(items) => {
                self.u8(5);
                self.canonical_types(items);
            }
            CanonicalType::Choice(items) => {
                self.u8(6);
                self.canonical_types(items);
            }
            CanonicalType::Borrow {
                kind,
                lifetime,
                inner,
            } => {
                self.u8(7);
                self.u8(*kind);
                self.option(lifetime.as_ref(), |bytes, value| bytes.string(value));
                self.canonical_type(inner);
            }
            CanonicalType::Function {
                params,
                result,
                effects,
            } => {
                self.u8(8);
                self.canonical_types(params);
                self.canonical_type(result);
                self.len(effects.effects.len());
                for effect in &effects.effects {
                    self.string(effect);
                }
                self.u8(effects.tail);
            }
        }
    }

    fn canonical_types(&mut self, types: &[CanonicalType]) {
        self.len(types.len());
        for ty in types {
            self.canonical_type(ty);
        }
    }

    fn effect_set(&mut self, effects: &EffectSet) {
        let mut labels = effects.iter().map(EffectId::as_str).collect::<Vec<_>>();
        labels.sort_unstable();
        labels.dedup();
        self.len(labels.len());
        for effect in labels {
            self.string(effect);
        }
    }
}

const fn canonical_atomic_tag(atomic: CanonicalAtomic) -> u8 {
    match atomic {
        CanonicalAtomic::Bool => 1,
        CanonicalAtomic::I8 => 2,
        CanonicalAtomic::I16 => 3,
        CanonicalAtomic::I32 => 4,
        CanonicalAtomic::I64 => 5,
        CanonicalAtomic::I128 => 6,
        CanonicalAtomic::ISize => 7,
        CanonicalAtomic::U8 => 8,
        CanonicalAtomic::U16 => 9,
        CanonicalAtomic::U32 => 10,
        CanonicalAtomic::U64 => 11,
        CanonicalAtomic::U128 => 12,
        CanonicalAtomic::USize => 13,
        CanonicalAtomic::F32 => 14,
        CanonicalAtomic::F64 => 15,
        CanonicalAtomic::String => 16,
        CanonicalAtomic::Char => 17,
        CanonicalAtomic::Bytes => 18,
        CanonicalAtomic::Unit => 19,
        CanonicalAtomic::Never => 20,
        CanonicalAtomic::DataFormat => 21,
        CanonicalAtomic::DataShape => 22,
        CanonicalAtomic::AgentValue => 23,
        CanonicalAtomic::TextCluster => 24,
        CanonicalAtomic::Duration => 25,
        CanonicalAtomic::DebugStatePath => 26,
        CanonicalAtomic::ObservationFieldPath => 27,
        CanonicalAtomic::ReducerError => 28,
        CanonicalAtomic::AgentError => 29,
        CanonicalAtomic::ArcError => 30,
    }
}

const fn canonical_constructor_tag(constructor: CanonicalConstructor) -> u8 {
    match constructor {
        CanonicalConstructor::Vec => 1,
        CanonicalConstructor::Array => 2,
        CanonicalConstructor::Slice => 3,
        CanonicalConstructor::Seq => 4,
        CanonicalConstructor::OrderedMap => 5,
        CanonicalConstructor::SortedMap => 6,
        CanonicalConstructor::BTreeMap => 7,
        CanonicalConstructor::Result => 8,
        CanonicalConstructor::Option => 9,
        CanonicalConstructor::Need => 10,
        CanonicalConstructor::Stream => 11,
        CanonicalConstructor::Reduction => 13,
        CanonicalConstructor::Ref => 16,
        CanonicalConstructor::Probe => 17,
        CanonicalConstructor::ThreadHandle => 18,
        CanonicalConstructor::Shared => 19,
    }
}

const fn bytes_format_tag(format: BytesFormat) -> u8 {
    match format {
        BytesFormat::Binary => 1,
        BytesFormat::Base64 => 2,
        BytesFormat::Hex => 3,
        BytesFormat::Array => 4,
    }
}

const fn enum_repr_tag(repr: EnumRepr) -> u8 {
    match repr {
        EnumRepr::I8 => 1,
        EnumRepr::I16 => 2,
        EnumRepr::I32 => 3,
        EnumRepr::I64 => 4,
        EnumRepr::I128 => 5,
        EnumRepr::Isize => 6,
        EnumRepr::U8 => 7,
        EnumRepr::U16 => 8,
        EnumRepr::U32 => 9,
        EnumRepr::U64 => 10,
        EnumRepr::U128 => 11,
        EnumRepr::Usize => 12,
    }
}

#[cfg(test)]
mod tests {
    use arcweft_lang_hir::symbol::{
        CallableDeclarationId, CallableDeclarationKey, CallableDeclarationOwner, CallablePackageId,
    };
    use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};

    use super::*;

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        push_u32(bytes, u32::try_from(value.len()).unwrap());
        bytes.extend_from_slice(value.as_bytes());
    }

    fn domain(value: &[u8]) -> Vec<u8> {
        let mut bytes = value.to_vec();
        push_u32(&mut bytes, 1);
        bytes
    }

    fn package() -> CallablePackageId {
        CallablePackageId::try_new("game.pkg").unwrap()
    }

    fn module() -> CanonicalModulePath {
        CanonicalModulePath::crate_root().join(ModuleSegment::new("state").unwrap())
    }

    fn callable(name: &str) -> CallableDeclarationId {
        CallableDeclarationId::try_new(
            package(),
            module(),
            CallableDeclarationOwner::Function,
            name,
        )
        .unwrap()
    }

    fn flow(public_id: &str, declaration_seed: &str) -> CheckedFlowId {
        let declaration_digest =
            CallableDeclarationKey::Existing(callable(declaration_seed)).semantic_digest();
        CheckedFlowId::for_test(
            arcweft_id::PublicId::try_new(public_id).unwrap(),
            declaration_digest,
        )
    }

    fn empty_signature() -> CanonicalSignature {
        CanonicalSignature {
            generics: Vec::new(),
            groups: vec![CanonicalParameterGroup {
                kind: CallableGroupKind::Initial,
                parameters: Vec::new(),
            }],
            result: Some(CanonicalType::Atomic(CanonicalAtomic::Unit)),
            where_predicates: Vec::new(),
        }
    }

    fn push_callable_id(bytes: &mut Vec<u8>, name: &str) {
        push_string(bytes, "game.pkg");
        push_string(bytes, "crate.state");
        bytes.push(1);
        push_u32(bytes, 0);
        push_string(bytes, name);
    }

    fn push_nominal_key(bytes: &mut Vec<u8>, name: &str, kind: u8) {
        push_string(bytes, "game.pkg");
        push_string(bytes, "crate.state");
        push_string(bytes, name);
        bytes.push(kind);
    }

    #[test]
    fn canonical_domain_version_and_contract_bytes_are_exact() {
        let mut expected_nominal = domain(b"arcweft.nominal-schema\0");
        expected_nominal.push(2);
        assert_eq!(nominal_schema_bytes(&TypeShape::Bool), expected_nominal);

        let callable = CanonicalCallableContract {
            signature: empty_signature(),
            contract_effects: EffectSet::new(),
        };
        let mut expected_callable = domain(b"arcweft.callable-contract\0");
        push_u32(&mut expected_callable, 0);
        push_u32(&mut expected_callable, 1);
        expected_callable.push(1);
        push_u32(&mut expected_callable, 0);
        expected_callable.push(1);
        expected_callable.push(1);
        expected_callable.push(19);
        push_u32(&mut expected_callable, 0);
        push_u32(&mut expected_callable, 0);
        assert_eq!(callable_contract_bytes(&callable), expected_callable);

        let flow_id = flow("flow.opening", "opening_flow_identity");
        let flow = CanonicalFlowContract {
            signature: Some(empty_signature()),
            contract_effects: EffectSet::new(),
            suspension: CanonicalFlowSuspension::Flow,
        };
        let mut expected_flow = domain(b"arcweft.flow-contract\0");
        push_string(&mut expected_flow, "flow.opening");
        expected_flow.extend_from_slice(flow_id.declaration_digest().as_bytes());
        expected_flow.push(1);
        push_u32(&mut expected_flow, 0);
        push_u32(&mut expected_flow, 1);
        expected_flow.push(1);
        push_u32(&mut expected_flow, 0);
        expected_flow.push(1);
        expected_flow.push(1);
        expected_flow.push(19);
        push_u32(&mut expected_flow, 0);
        push_u32(&mut expected_flow, 0);
        expected_flow.push(1);
        assert_eq!(flow_contract_bytes(&flow_id, &flow), expected_flow);
    }

    #[test]
    fn same_public_flow_label_with_distinct_declarations_has_distinct_contract_identity() {
        let left = flow("flow.opening", "opening_flow_left");
        let right = flow("flow.opening", "opening_flow_right");
        let contract = CanonicalFlowContract {
            signature: Some(empty_signature()),
            contract_effects: EffectSet::new(),
            suspension: CanonicalFlowSuspension::Flow,
        };

        assert_eq!(left.public_id(), right.public_id());
        assert_ne!(left.declaration_digest(), right.declaration_digest());
        assert_ne!(
            flow_contract(&left, &contract),
            flow_contract(&right, &contract)
        );
    }

    #[test]
    fn canonical_agent_policy_bytes_are_exact_and_sorted() {
        let allowed = EffectSet::from_labels(["signal.write", "agent.observe"]).unwrap();
        let inferred = EffectSet::from_labels(["agent.observe"]).unwrap();
        let policy = CheckedAgentPolicy::new(allowed, inferred);
        let budget = AgentBudget {
            logical_timeout_millis: 1,
            max_vm_steps: 2,
            max_host_calls: 3,
            max_observations: 4,
            max_captures: 5,
            max_capture_bytes: 6,
            max_rag_queries: 7,
            max_context_bytes: 8,
        };
        let mut expected = domain(b"arcweft.agent-policy\0");
        push_u32(&mut expected, 2);
        push_string(&mut expected, "agent.observe");
        push_string(&mut expected, "signal.write");
        push_u32(&mut expected, 1);
        push_string(&mut expected, "agent.observe");
        push_u64(&mut expected, 1);
        push_u64(&mut expected, 2);
        push_u32(&mut expected, 3);
        push_u32(&mut expected, 4);
        push_u32(&mut expected, 5);
        push_u64(&mut expected, 6);
        push_u32(&mut expected, 7);
        push_u64(&mut expected, 8);
        assert_eq!(agent_policy_bytes(&policy, budget), expected);
    }

    #[test]
    fn canonical_stateful_binding_bytes_follow_normative_field_order() {
        let package = package();
        let id = CheckedEntryId::try_new("entry.game.main").unwrap();
        let state = BoundNominalTypeKey::new(
            package.clone(),
            module(),
            "GameState",
            BoundNominalKind::Struct,
        );
        let event = BoundNominalTypeKey::new(
            package.clone(),
            module(),
            "GameEvent",
            BoundNominalKind::Enum,
        );
        let initializer = callable("initial_state");
        let reducer = callable("reduce");
        let flow = flow("flow.opening", "opening_flow_identity");
        let state_digest = NominalSchemaDigest::from_bytes([0x11; 32]);
        let initializer_digest = CallableContractDigest::from_bytes([0x22; 32]);
        let event_digest = NominalSchemaDigest::from_bytes([0x33; 32]);
        let reducer_digest = CallableContractDigest::from_bytes([0x44; 32]);
        let flow_digest = FlowContractDigest::from_bytes([0x55; 32]);

        let actual = stateful_binding_bytes(&StatefulBindingInput {
            package: &package,
            id: &id,
            kind: CheckedStatefulEntryKind::Game,
            state: (&state, &state_digest),
            initializer: (&initializer, &initializer_digest),
            event: (&event, &event_digest),
            reducer: (&reducer, &reducer_digest),
            initial_flow: (&flow, &flow_digest),
        });
        let mut expected = domain(b"arcweft.checked-entry-binding\0");
        expected.push(1);
        push_string(&mut expected, "game.pkg");
        push_string(&mut expected, "entry.game.main");
        expected.push(1);
        push_nominal_key(&mut expected, "GameState", 1);
        expected.extend_from_slice(&[0x11; 32]);
        push_callable_id(&mut expected, "initial_state");
        expected.extend_from_slice(&[0x22; 32]);
        push_nominal_key(&mut expected, "GameEvent", 2);
        expected.extend_from_slice(&[0x33; 32]);
        push_callable_id(&mut expected, "reduce");
        expected.extend_from_slice(&[0x44; 32]);
        push_string(&mut expected, "flow.opening");
        expected.extend_from_slice(flow.declaration_digest().as_bytes());
        expected.extend_from_slice(&[0x55; 32]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn reducer_contract_digest_is_part_of_stateful_binding_identity() {
        let package = package();
        let id = CheckedEntryId::try_new("entry.game.main").unwrap();
        let state = BoundNominalTypeKey::new(
            package.clone(),
            module(),
            "GameState",
            BoundNominalKind::Struct,
        );
        let event = BoundNominalTypeKey::new(
            package.clone(),
            module(),
            "GameEvent",
            BoundNominalKind::Enum,
        );
        let initializer = callable("initial_state");
        let reducer = callable("reduce");
        let flow = flow("flow.opening", "opening_flow_identity");
        let state_digest = NominalSchemaDigest::from_bytes([0x11; 32]);
        let initializer_digest = CallableContractDigest::from_bytes([0x22; 32]);
        let event_digest = NominalSchemaDigest::from_bytes([0x33; 32]);
        let reducer_before = CallableContractDigest::from_bytes([0x44; 32]);
        let reducer_after = CallableContractDigest::from_bytes([0x45; 32]);
        let flow_digest = FlowContractDigest::from_bytes([0x55; 32]);

        let before = stateful_binding(StatefulBindingInput {
            package: &package,
            id: &id,
            kind: CheckedStatefulEntryKind::Game,
            state: (&state, &state_digest),
            initializer: (&initializer, &initializer_digest),
            event: (&event, &event_digest),
            reducer: (&reducer, &reducer_before),
            initial_flow: (&flow, &flow_digest),
        });
        let after = stateful_binding(StatefulBindingInput {
            package: &package,
            id: &id,
            kind: CheckedStatefulEntryKind::Game,
            state: (&state, &state_digest),
            initializer: (&initializer, &initializer_digest),
            event: (&event, &event_digest),
            reducer: (&reducer, &reducer_after),
            initial_flow: (&flow, &flow_digest),
        });

        assert_ne!(before, after);
    }

    #[test]
    fn canonical_agent_binding_bytes_follow_normative_field_order() {
        let package = package();
        let id = CheckedEntryId::try_new("entry.agent.smoke").unwrap();
        let controller = callable("smoke");
        let contract = CallableContractDigest::from_bytes([0x66; 32]);
        let policy = CheckedAgentPolicyDigest::from_bytes([0x77; 32]);
        let actual = agent_binding_bytes(&package, &id, (&controller, &contract), &policy);

        let mut expected = domain(b"arcweft.checked-entry-binding\0");
        expected.push(2);
        push_string(&mut expected, "game.pkg");
        push_string(&mut expected, "entry.agent.smoke");
        expected.push(4);
        push_callable_id(&mut expected, "smoke");
        expected.extend_from_slice(&[0x66; 32]);
        expected.extend_from_slice(&[0x77; 32]);
        assert_eq!(actual, expected);
    }

    #[test]
    fn canonical_existing_binding_bytes_use_a_distinct_variant_prefix() {
        let package = package();
        let id = CheckedEntryId::try_new("entry.server.main").unwrap();
        let target = CheckedExistingEntryTarget::Routes(Box::new([]));
        let actual = existing_binding_bytes(&package, &id, &CheckedEntryKind::Server, &target);

        let mut expected = domain(b"arcweft.checked-entry-binding\0");
        expected.push(3);
        push_string(&mut expected, "game.pkg");
        push_string(&mut expected, "entry.server.main");
        expected.push(6);
        expected.push(0);
        expected.push(2);
        expected.extend_from_slice(&0_u32.to_le_bytes());
        assert_eq!(actual, expected);
    }
}
