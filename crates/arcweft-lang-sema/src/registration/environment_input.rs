//! Source-backed, world-neutral environment registration input.
//!
//! Adapter and Rust metadata producers stop at this boundary. Final semantic
//! types are projected only after registration has accepted one nominal world.

use arcweft_lang_hir::symbol::ProjectSymbolWorldId;
use arcweft_rust_abi::ArcweftRustTypeParameterIndex;
use arcweft_source::{SourceDocumentIdentity, SourceSpan};

use crate::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableDocumentation, CallableGroupIndex,
        CallableGroupKind, CallableName, CallableOverloadIndex, CallableParameterIndex,
        CallableParameterPassing, CallableParameterPresence, CallableParameterSource,
        CallableSource, CallableValidator, EnvironmentCallableKind, EnvironmentCallableOwner,
        EnvironmentDeclarationOrdinal, ProjectCallablePath, RustCallableProvenance, RustItemPath,
    },
    effect_row::EffectRow,
    env::{
        identity::EnvironmentBindingId,
        nominal::{AcceptedNominalId, AcceptedNominalOrigin, RustPackageId},
        rust_metadata::RustTypeMetadataPublicationInput,
    },
};

/// Digest of canonical typed manifest content, excluding generated source spans.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentManifestDigest([u8; 32]);

/// Digest of one unresolved type input used to distinguish method items.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentTypeInputDigest([u8; 32]);

/// Stable identity of an item contributing environment types or callables.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentPublicationItemId {
    AdapterSymbol {
        owner: EnvironmentCallableOwner,
        path: arcweft_lang_syntax::ast::symbol_path::ProjectSymbolPath,
    },
    AdapterNominal {
        owner: EnvironmentCallableOwner,
        path: arcweft_lang_syntax::types::TypePath,
    },
    AdapterFunction {
        owner: EnvironmentCallableOwner,
        path: ProjectCallablePath,
        overload: CallableOverloadIndex,
    },
    AdapterMethod {
        owner: EnvironmentCallableOwner,
        receiver: EnvironmentTypeInputDigest,
        method: CallableName,
        overload: CallableOverloadIndex,
        declaration_order: EnvironmentDeclarationOrdinal,
    },
    RustType {
        adapter: AdapterPackageId,
        package: RustPackageId,
        rust_item: RustItemPath,
        accepted_path: arcweft_lang_syntax::types::TypePath,
    },
    RustFunction {
        adapter: AdapterPackageId,
        package: RustPackageId,
        rust_item: RustItemPath,
        callable_path: ProjectCallablePath,
        overload: CallableOverloadIndex,
    },
}

/// Root coordinate of a type node inside one publication item.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentTypeSiteRoot {
    SymbolType,
    MethodReceiver,
    Parameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    Result,
    RustStructTupleField {
        field: u16,
    },
    RustStructRecordField {
        field: String,
    },
    RustEnumTupleField {
        variant: String,
        field: u16,
    },
    RustEnumRecordField {
        variant: String,
        field: String,
    },
    RustNewtypeInner,
}

/// Recursive coordinate below an environment type-site root.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentTypeSiteStep {
    VecItem,
    SeqItem,
    OptionItem,
    ResultOk,
    ResultError,
    TupleItem(u16),
    NeedReady,
    NeedError,
    NominalArgument(u16),
}

/// Exact location of a projected type node within one publication item.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentTypeSite {
    root: EnvironmentTypeSiteRoot,
    steps: Box<[EnvironmentTypeSiteStep]>,
}

/// One source-backed type node awaiting accepted-world projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentTypeProjectionNode {
    source: SourceSpan,
    kind: EnvironmentTypeProjectionKind,
}

/// World-neutral recursive type shape used by environment registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentTypeProjectionKind {
    Unit,
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
    Vec(Box<EnvironmentTypeProjectionNode>),
    Seq(Box<EnvironmentTypeProjectionNode>),
    Option(Box<EnvironmentTypeProjectionNode>),
    Result {
        ok: Box<EnvironmentTypeProjectionNode>,
        error: Box<EnvironmentTypeProjectionNode>,
    },
    Tuple(Box<[EnvironmentTypeProjectionNode]>),
    Need {
        ready: Box<EnvironmentTypeProjectionNode>,
        error: Box<EnvironmentTypeProjectionNode>,
    },
    CharacterNominal(crate::types::CharacterNominalType),
    AcceptedNominal {
        id: AcceptedNominalId,
        arguments: Box<[EnvironmentTypeProjectionNode]>,
    },
    TypeParameter {
        index: ArcweftRustTypeParameterIndex,
    },
}

/// Whether one exact nominal is visible to authored source in this world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptedNominalInputVisibility {
    Visible,
    Inaccessible,
}

/// One exact nominal declaration contributed by a source-backed environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalInventoryInput {
    id: AcceptedNominalId,
    arity: u16,
    visibility: AcceptedNominalInputVisibility,
    origin: AcceptedNominalOrigin,
    source: SourceSpan,
    item: EnvironmentPublicationItemId,
}

/// One source-visible adapter value whose type awaits accepted-world projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentValueBindingInput {
    item: EnvironmentPublicationItemId,
    id: EnvironmentBindingId,
    ty: EnvironmentTypeProjectionNode,
}

/// One unresolved callable parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentParameterInput {
    index: CallableParameterIndex,
    name: Option<CallableName>,
    ty: EnvironmentParameterTypeInput,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    documentation: Option<std::sync::Arc<str>>,
    source: Option<CallableParameterSource>,
}

/// Optional authoring metadata attached to one callable parameter input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentParameterMetadataInput {
    documentation: Option<std::sync::Arc<str>>,
    source: Option<CallableParameterSource>,
}

/// Exact checked type input or an explicitly unchecked variadic boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentParameterTypeInput {
    Exact(EnvironmentTypeProjectionNode),
    Unchecked { source: SourceSpan },
}

/// One unresolved callable parameter group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentParameterGroupInput {
    index: CallableGroupIndex,
    kind: CallableGroupKind,
    parameters: Box<[EnvironmentParameterInput]>,
}

/// Callable signature whose semantic types have not yet been projected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallableSignatureInput {
    groups: Box<[EnvironmentParameterGroupInput]>,
    result: EnvironmentTypeProjectionNode,
    effects: EffectRow,
    argument_policy: CallableArgumentPolicy,
    validator: CallableValidator,
}

/// Lookup identity awaiting receiver projection where applicable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentCallableLookupInput {
    Free(ProjectCallablePath),
    Method {
        receiver: EnvironmentTypeProjectionNode,
        method: CallableName,
    },
}

/// One callable record awaiting accepted-world type projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublicationRecordInput {
    item: EnvironmentPublicationItemId,
    kind: EnvironmentCallableKind,
    key: EnvironmentCallableLookupInput,
    overload: CallableOverloadIndex,
    schema: EnvironmentCallableSignatureInput,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    declaration_order: EnvironmentDeclarationOrdinal,
}

/// Documentation and implementation provenance carried by one callable publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublicationMetadataInput {
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
}

/// Complete source-backed contribution from one environment owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedEnvironmentRegistrationInput {
    owner: EnvironmentCallableOwner,
    source: SourceDocumentIdentity,
    manifest_digest: EnvironmentManifestDigest,
    nominal_inventory: Box<[AcceptedNominalInventoryInput]>,
    value_bindings: Box<[EnvironmentValueBindingInput]>,
    rust_metadata: Box<[RustTypeMetadataPublicationInput]>,
    callable_records: Box<[EnvironmentCallablePublicationRecordInput]>,
}

impl EnvironmentManifestDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl EnvironmentTypeInputDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl EnvironmentTypeSite {
    pub fn new(
        root: EnvironmentTypeSiteRoot,
        steps: impl Into<Box<[EnvironmentTypeSiteStep]>>,
    ) -> Self {
        Self {
            root,
            steps: steps.into(),
        }
    }

    pub const fn root(&self) -> &EnvironmentTypeSiteRoot {
        &self.root
    }

    pub fn steps(&self) -> &[EnvironmentTypeSiteStep] {
        &self.steps
    }
}

impl EnvironmentTypeProjectionNode {
    pub const fn new(source: SourceSpan, kind: EnvironmentTypeProjectionKind) -> Self {
        Self { source, kind }
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn kind(&self) -> &EnvironmentTypeProjectionKind {
        &self.kind
    }
}

impl AcceptedNominalInventoryInput {
    pub fn new(
        id: AcceptedNominalId,
        arity: u16,
        visibility: AcceptedNominalInputVisibility,
        origin: AcceptedNominalOrigin,
        source: SourceSpan,
        item: EnvironmentPublicationItemId,
    ) -> Self {
        Self {
            id,
            arity,
            visibility,
            origin,
            source,
            item,
        }
    }

    pub const fn id(&self) -> &AcceptedNominalId {
        &self.id
    }

    pub const fn arity(&self) -> u16 {
        self.arity
    }

    pub const fn visibility(&self) -> AcceptedNominalInputVisibility {
        self.visibility
    }

    pub const fn origin(&self) -> AcceptedNominalOrigin {
        self.origin
    }

    pub const fn source(&self) -> &SourceSpan {
        &self.source
    }

    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }
}

impl EnvironmentValueBindingInput {
    pub fn new(
        item: EnvironmentPublicationItemId,
        id: EnvironmentBindingId,
        ty: EnvironmentTypeProjectionNode,
    ) -> Self {
        Self { item, id, ty }
    }

    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }

    pub const fn id(&self) -> &EnvironmentBindingId {
        &self.id
    }

    pub const fn ty(&self) -> &EnvironmentTypeProjectionNode {
        &self.ty
    }
}

impl EnvironmentParameterInput {
    pub fn new(
        index: CallableParameterIndex,
        name: Option<CallableName>,
        ty: EnvironmentParameterTypeInput,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        metadata: EnvironmentParameterMetadataInput,
    ) -> Self {
        Self {
            index,
            name,
            ty,
            passing,
            presence,
            documentation: metadata.documentation,
            source: metadata.source,
        }
    }

    pub const fn index(&self) -> CallableParameterIndex {
        self.index
    }

    pub const fn name(&self) -> Option<&CallableName> {
        self.name.as_ref()
    }

    pub const fn ty(&self) -> &EnvironmentParameterTypeInput {
        &self.ty
    }

    pub const fn passing(&self) -> CallableParameterPassing {
        self.passing
    }

    pub const fn presence(&self) -> CallableParameterPresence {
        self.presence
    }

    pub fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }

    pub const fn source(&self) -> Option<&CallableParameterSource> {
        self.source.as_ref()
    }
}

impl EnvironmentParameterMetadataInput {
    pub fn new(
        documentation: Option<std::sync::Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Self {
        Self {
            documentation,
            source,
        }
    }
}

impl EnvironmentParameterTypeInput {
    pub const fn source(&self) -> &SourceSpan {
        match self {
            Self::Exact(node) => node.source(),
            Self::Unchecked { source } => source,
        }
    }
}

impl EnvironmentParameterGroupInput {
    pub fn new(
        index: CallableGroupIndex,
        kind: CallableGroupKind,
        parameters: impl Into<Box<[EnvironmentParameterInput]>>,
    ) -> Self {
        Self {
            index,
            kind,
            parameters: parameters.into(),
        }
    }

    pub const fn index(&self) -> CallableGroupIndex {
        self.index
    }

    pub const fn kind(&self) -> CallableGroupKind {
        self.kind
    }

    pub fn parameters(&self) -> &[EnvironmentParameterInput] {
        &self.parameters
    }
}

impl EnvironmentCallableSignatureInput {
    pub fn new(
        groups: impl Into<Box<[EnvironmentParameterGroupInput]>>,
        result: EnvironmentTypeProjectionNode,
        effects: EffectRow,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
    ) -> Self {
        Self {
            groups: groups.into(),
            result,
            effects,
            argument_policy,
            validator,
        }
    }

    pub fn groups(&self) -> &[EnvironmentParameterGroupInput] {
        &self.groups
    }

    pub const fn result(&self) -> &EnvironmentTypeProjectionNode {
        &self.result
    }

    pub const fn effects(&self) -> &EffectRow {
        &self.effects
    }

    pub const fn argument_policy(&self) -> CallableArgumentPolicy {
        self.argument_policy
    }

    pub const fn validator(&self) -> &CallableValidator {
        &self.validator
    }
}

impl EnvironmentCallablePublicationRecordInput {
    pub fn new(
        item: EnvironmentPublicationItemId,
        kind: EnvironmentCallableKind,
        key: EnvironmentCallableLookupInput,
        overload: CallableOverloadIndex,
        schema: EnvironmentCallableSignatureInput,
        declaration_order: EnvironmentDeclarationOrdinal,
        metadata: EnvironmentCallablePublicationMetadataInput,
    ) -> Self {
        Self {
            item,
            kind,
            key,
            overload,
            schema,
            documentation: metadata.documentation,
            source: metadata.source,
            rust: metadata.rust,
            declaration_order,
        }
    }

    pub const fn item(&self) -> &EnvironmentPublicationItemId {
        &self.item
    }

    pub const fn kind(&self) -> EnvironmentCallableKind {
        self.kind
    }

    pub const fn key(&self) -> &EnvironmentCallableLookupInput {
        &self.key
    }

    pub const fn overload(&self) -> CallableOverloadIndex {
        self.overload
    }

    pub const fn schema(&self) -> &EnvironmentCallableSignatureInput {
        &self.schema
    }

    pub const fn documentation(&self) -> &CallableDocumentation {
        &self.documentation
    }

    pub const fn source(&self) -> Option<&CallableSource> {
        self.source.as_ref()
    }

    pub const fn rust(&self) -> Option<&RustCallableProvenance> {
        self.rust.as_ref()
    }

    pub const fn declaration_order(&self) -> EnvironmentDeclarationOrdinal {
        self.declaration_order
    }
}

impl EnvironmentCallablePublicationMetadataInput {
    pub fn new(
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
    ) -> Self {
        Self {
            documentation,
            source,
            rust,
        }
    }
}

impl SourceBackedEnvironmentRegistrationInput {
    pub fn new(
        owner: EnvironmentCallableOwner,
        source: SourceDocumentIdentity,
        manifest_digest: EnvironmentManifestDigest,
        nominal_inventory: impl Into<Box<[AcceptedNominalInventoryInput]>>,
        value_bindings: impl Into<Box<[EnvironmentValueBindingInput]>>,
        rust_metadata: impl Into<Box<[RustTypeMetadataPublicationInput]>>,
        callable_records: impl Into<Box<[EnvironmentCallablePublicationRecordInput]>>,
    ) -> Self {
        Self {
            owner,
            source,
            manifest_digest,
            nominal_inventory: nominal_inventory.into(),
            value_bindings: value_bindings.into(),
            rust_metadata: rust_metadata.into(),
            callable_records: callable_records.into(),
        }
    }

    pub const fn owner(&self) -> &EnvironmentCallableOwner {
        &self.owner
    }

    pub const fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    pub const fn manifest_digest(&self) -> EnvironmentManifestDigest {
        self.manifest_digest
    }

    pub fn nominal_inventory(&self) -> &[AcceptedNominalInventoryInput] {
        &self.nominal_inventory
    }

    pub fn value_bindings(&self) -> &[EnvironmentValueBindingInput] {
        &self.value_bindings
    }

    pub fn rust_metadata(&self) -> &[RustTypeMetadataPublicationInput] {
        &self.rust_metadata
    }

    pub fn callable_records(&self) -> &[EnvironmentCallablePublicationRecordInput] {
        &self.callable_records
    }

    pub(crate) fn source_spans(&self) -> Vec<&SourceSpan> {
        let mut spans = self
            .nominal_inventory
            .iter()
            .map(AcceptedNominalInventoryInput::source)
            .chain(self.value_bindings.iter().flat_map(|binding| {
                let mut spans = Vec::new();
                append_type_spans(binding.ty(), &mut spans);
                spans
            }))
            .chain(
                self.rust_metadata
                    .iter()
                    .map(RustTypeMetadataPublicationInput::source),
            )
            .collect::<Vec<_>>();
        for metadata in &self.rust_metadata {
            spans.extend(
                metadata
                    .parameters()
                    .iter()
                    .map(crate::env::rust_metadata::RustTypeParameterPublicationInput::source),
            );
            append_rust_metadata_spans(metadata.kind(), &mut spans);
        }
        for record in &self.callable_records {
            if let EnvironmentCallableLookupInput::Method { receiver, .. } = record.key() {
                append_type_spans(receiver, &mut spans);
            }
            for group in record.schema().groups() {
                for parameter in group.parameters() {
                    match parameter.ty() {
                        EnvironmentParameterTypeInput::Exact(ty) => {
                            append_type_spans(ty, &mut spans);
                        }
                        EnvironmentParameterTypeInput::Unchecked { source } => spans.push(source),
                    }
                    if let Some(source) = parameter.source() {
                        spans.push(source.whole());
                        spans.extend(source.name());
                        spans.extend(source.ty());
                        spans.extend(source.default());
                    }
                }
            }
            append_type_spans(record.schema().result(), &mut spans);
            if let Some(source) = record.source() {
                spans.extend(source.signature());
                spans.extend(source.name());
                spans.extend(source.result());
                for parameter in source.parameters() {
                    spans.push(parameter.whole());
                    spans.extend(parameter.name());
                    spans.extend(parameter.ty());
                    spans.extend(parameter.default());
                }
            }
        }
        spans
    }

    pub(crate) fn bind_world(
        self,
        world: ProjectSymbolWorldId,
    ) -> BoundEnvironmentRegistrationInput {
        BoundEnvironmentRegistrationInput { world, input: self }
    }
}

fn append_type_spans<'a>(root: &'a EnvironmentTypeProjectionNode, spans: &mut Vec<&'a SourceSpan>) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        spans.push(node.source());
        match node.kind() {
            EnvironmentTypeProjectionKind::Vec(item)
            | EnvironmentTypeProjectionKind::Seq(item)
            | EnvironmentTypeProjectionKind::Option(item) => pending.push(item),
            EnvironmentTypeProjectionKind::Result { ok, error } => {
                pending.push(error);
                pending.push(ok);
            }
            EnvironmentTypeProjectionKind::Tuple(items)
            | EnvironmentTypeProjectionKind::AcceptedNominal {
                arguments: items, ..
            } => pending.extend(items.iter().rev()),
            EnvironmentTypeProjectionKind::Need { ready, error } => {
                pending.push(error);
                pending.push(ready);
            }
            EnvironmentTypeProjectionKind::Unit
            | EnvironmentTypeProjectionKind::Bool
            | EnvironmentTypeProjectionKind::I8
            | EnvironmentTypeProjectionKind::I16
            | EnvironmentTypeProjectionKind::I32
            | EnvironmentTypeProjectionKind::I64
            | EnvironmentTypeProjectionKind::I128
            | EnvironmentTypeProjectionKind::ISize
            | EnvironmentTypeProjectionKind::U8
            | EnvironmentTypeProjectionKind::U16
            | EnvironmentTypeProjectionKind::U32
            | EnvironmentTypeProjectionKind::U64
            | EnvironmentTypeProjectionKind::U128
            | EnvironmentTypeProjectionKind::USize
            | EnvironmentTypeProjectionKind::F32
            | EnvironmentTypeProjectionKind::F64
            | EnvironmentTypeProjectionKind::String
            | EnvironmentTypeProjectionKind::Char
            | EnvironmentTypeProjectionKind::CharacterNominal(_)
            | EnvironmentTypeProjectionKind::TypeParameter { .. } => {}
        }
    }
}

fn append_rust_metadata_spans<'a>(
    kind: &'a crate::env::rust_metadata::RustTypeMetadataPublicationKind,
    spans: &mut Vec<&'a SourceSpan>,
) {
    use crate::env::rust_metadata::{
        RustStructMetadataInput, RustTypeMetadataPublicationKind, RustVariantPayloadInput,
    };

    match kind {
        RustTypeMetadataPublicationKind::Struct { shape } => match shape {
            RustStructMetadataInput::Unit => {}
            RustStructMetadataInput::Tuple(fields) => {
                fields
                    .iter()
                    .for_each(|field| append_type_spans(field, spans));
            }
            RustStructMetadataInput::Record(fields) => {
                fields
                    .iter()
                    .for_each(|(_, field)| append_type_spans(field, spans));
            }
        },
        RustTypeMetadataPublicationKind::Enum { variants } => {
            for variant in variants {
                spans.push(variant.source());
                match variant.payload() {
                    RustVariantPayloadInput::Unit => {}
                    RustVariantPayloadInput::Tuple(fields) => {
                        fields
                            .iter()
                            .for_each(|field| append_type_spans(field, spans));
                    }
                    RustVariantPayloadInput::Record(fields) => {
                        fields
                            .iter()
                            .for_each(|(_, field)| append_type_spans(field, spans));
                    }
                }
            }
        }
        RustTypeMetadataPublicationKind::Newtype { inner } => append_type_spans(inner, spans),
    }
}

/// Registration input bound to the exact project world that accepted its source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundEnvironmentRegistrationInput {
    world: ProjectSymbolWorldId,
    input: SourceBackedEnvironmentRegistrationInput,
}

impl BoundEnvironmentRegistrationInput {
    pub(crate) const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub(crate) const fn input(&self) -> &SourceBackedEnvironmentRegistrationInput {
        &self.input
    }
}
