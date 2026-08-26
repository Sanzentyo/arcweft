//! Compact resource codecs for runtime-facing product sections.
//!
//! This module implements the first seq-02.2 section-family migration on top of
//! the seq-02.1 common resource wire envelope. It owns only Sans I/O typed data,
//! deterministic bytes, decode budgets, and patch compatibility fingerprints for
//! runtime types, entrypoints, and adapter requirements.

use crate::container::{BundleDigest, BundleSectionKind};
use crate::patch::PatchCompatibility;
use crate::{ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleManifest};
use arcweft_core::awbc::schema::{
    AwbcAgentTypeShape, AwbcDigest, AwbcEntryKind, AwbcEntryTarget, AwbcFrameLayout,
    AwbcFrameSlotRole, AwbcFunctionKind, AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape,
    AwbcSignature, AwbcSignedIntKind, AwbcStringId, AwbcTypeId, AwbcUnsignedIntKind,
    AwbcVariantIdentity,
};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::budget::{SectionCodecBudget, check_budget};
use super::error::SectionCodecError;
use super::field::{FieldId, FieldRegistry, FieldSpec, ResourceField, ResourceWireType};
use super::kind::ProductSectionCodecKind;
use super::table::{EnumRegistry, EnumSymbol, PublicIdRef, PublicIdTable, StringId, StringTable};
use super::wire::ProductResourceEnvelope;

const NONE_REF: u32 = u32::MAX;

const FIELD_RUNTIME_ABI_VERSION: FieldId = FieldId(1);
const FIELD_RUNTIME_LAYOUT_DIGEST: FieldId = FieldId(2);
const FIELD_RUNTIME_TYPE_DECLARATIONS: FieldId = FieldId(3);
const FIELD_RUNTIME_FUNCTION_INTERFACES: FieldId = FieldId(4);

const FIELD_ENTRYPOINT_RECORDS: FieldId = FieldId(1);
const FIELD_ADAPTER_REQUIREMENT_RECORDS: FieldId = FieldId(1);

/// Runtime-facing section decode limits layered on top of the common seq-02.1
/// resource budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeResourceBudget {
    pub common: SectionCodecBudget,
    pub runtime_types: usize,
    pub function_interfaces: usize,
    pub entrypoints: usize,
    pub adapter_requirements: usize,
    pub adapter_manifests: usize,
    pub host_calls: usize,
}

/// Product runtime type section decoded from `RuntimeTypes`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTypesSection {
    pub abi_version: u32,
    pub runtime_layout_digest: AwbcDigest,
    pub declarations: Vec<RuntimeTypeDeclaration>,
    pub function_interfaces: Vec<FunctionInterfaceFingerprint>,
}

/// One public or anonymous runtime type layout declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTypeDeclaration {
    pub semantic_identity: RuntimeSemanticTypeId,
    pub public_id: Option<String>,
    pub value_kind: RuntimeValueKind,
    pub layout_digest: BundleDigest,
    pub compatibility: TypeCompatibilityLabel,
}

/// Value layout families that runtime/product compatibility can reason about
/// without hard-coding user type names.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeValueKind {
    Unit,
    Bool,
    SignedInteger,
    UnsignedInteger,
    Float,
    Text,
    Duration,
    Progress,
    EntityRef,
    Tuple,
    Sequence,
    Record,
    Variant,
    Choice,
    Nominal,
    Opaque,
    Agent,
    Matrix,
    Tensor,
    Task,
    Need,
    Range,
    Iterator,
    Array,
    Map,
    Stream,
    Shared,
    Reference,
    Function,
    Dynamic,
}

/// Compatibility label emitted by the owning type registry or derived
/// conservatively from AWBC runtime layout facts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeCompatibilityLabel {
    ContentOnly,
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

/// Runtime callable interface fingerprint for one AWBC function binding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionInterfaceFingerprint {
    pub public_id: Option<String>,
    pub awbc_function_index: u32,
    pub kind: RuntimeFunctionKind,
    pub signature_digest: BundleDigest,
    pub frame_layout_digest: BundleDigest,
    pub flags: u32,
    pub compatibility: TypeCompatibilityLabel,
}

/// Product/runtime callable families.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFunctionKind {
    Flow,
    PureHelper,
    TraitMethod,
    StreamTransform,
    LineTask,
    Synthetic,
}

/// Product entrypoint section decoded from `Entrypoints`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntrypointsSection {
    pub entries: Vec<EntrypointDeclaration>,
}

/// One product entrypoint record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntrypointDeclaration {
    pub public_id: String,
    pub exported_name: Option<String>,
    pub awbc_function_index: Option<u32>,
    pub source_anchor: Option<EntrypointSourceAnchor>,
    pub visibility: ProductVisibility,
}

/// Source anchor for human inspection and source-map cross checks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntrypointSourceAnchor {
    pub source_public_id: String,
    pub start_byte: u32,
    pub end_byte: u32,
}

/// Product visibility of an entrypoint.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductVisibility {
    Public,
    Hidden,
    TestOnly,
}

/// Product adapter-requirement section decoded from `AdapterRequirements`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdapterRequirementsSection {
    pub default_adapter: Option<String>,
    pub adapter_manifest_ids: Vec<String>,
    pub required_host_calls: Vec<String>,
    pub adapter_manifests: Vec<BundleAdapterManifest>,
    pub required_capabilities: Vec<CapabilityRequirement>,
    pub optional_capabilities: Vec<CapabilityRequirement>,
    pub feature_flags: Vec<String>,
    pub launch_constraints: Vec<LaunchConstraint>,
    pub platform_refs: Vec<PlatformRequirementRef>,
}

/// Required or optional capability contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRequirement {
    pub public_id: String,
    pub version: VersionRange,
    pub feature_flags: Vec<String>,
}

/// Inclusive version range. `None` is an open bound.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VersionRange {
    pub min: Option<String>,
    pub max: Option<String>,
}

/// Launch-time constraint surfaced to host/player adapters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LaunchConstraint {
    pub public_id: String,
    pub required: bool,
}

/// Platform-specific requirement reference. Runtime semantic validation belongs
/// to the platform adapter, not this Sans I/O codec.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlatformRequirementRef {
    pub platform: String,
    pub requirement: String,
}

/// Cross-section patch compatibility result for migrated runtime resources.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeResourceCompatibility {
    ContentOnly,
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

impl Default for RuntimeResourceBudget {
    fn default() -> Self {
        Self {
            common: SectionCodecBudget {
                records: 262_144,
                items: 262_144,
                public_ids: 262_144,
                strings: 262_144,
                string_bytes: 16 * 1024 * 1024,
                references: 1_000_000,
                ..SectionCodecBudget::default()
            },
            runtime_types: 262_144,
            function_interfaces: 262_144,
            entrypoints: 65_536,
            adapter_requirements: 262_144,
            adapter_manifests: 65_536,
            host_calls: 262_144,
        }
    }
}

impl RuntimeResourceCompatibility {
    pub const fn patch_compatibility(self) -> PatchCompatibility {
        match self {
            Self::ContentOnly => PatchCompatibility::ContentOnly,
            Self::CodeCompatible => PatchCompatibility::CodeCompatible,
            Self::CodeGenerational => PatchCompatibility::CodeGenerational,
            Self::RestartRequired => PatchCompatibility::RestartRequired,
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::ContentOnly => 0,
            Self::CodeCompatible => 1,
            Self::CodeGenerational => 2,
            Self::RestartRequired => 3,
        }
    }

    fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }
}

impl TypeCompatibilityLabel {
    const fn runtime_compatibility(self) -> RuntimeResourceCompatibility {
        match self {
            Self::ContentOnly => RuntimeResourceCompatibility::ContentOnly,
            Self::CodeCompatible => RuntimeResourceCompatibility::CodeCompatible,
            Self::CodeGenerational => RuntimeResourceCompatibility::CodeGenerational,
            Self::RestartRequired => RuntimeResourceCompatibility::RestartRequired,
        }
    }
}

impl VersionRange {
    pub const ANY: Self = Self {
        min: None,
        max: None,
    };
}

impl RuntimeTypesSection {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Result<Self, SectionCodecError> {
        Self::from_awbc_program(bundle.product_awbc_program())
    }

    pub fn try_new(
        abi_version: u32,
        runtime_layout_digest: AwbcDigest,
        declarations: impl IntoIterator<Item = RuntimeTypeDeclaration>,
        function_interfaces: impl IntoIterator<Item = FunctionInterfaceFingerprint>,
    ) -> Result<Self, SectionCodecError> {
        let mut declarations = declarations.into_iter().collect::<Vec<_>>();
        declarations.sort_by(runtime_type_order);
        if declarations
            .windows(2)
            .any(|pair| pair[0].semantic_identity == pair[1].semantic_identity)
        {
            return Err(SectionCodecError::NonCanonicalTable(
                "runtime_type_semantic_identity",
            ));
        }
        let mut function_interfaces = function_interfaces.into_iter().collect::<Vec<_>>();
        function_interfaces.sort_by(function_interface_order);
        if function_interfaces
            .windows(2)
            .any(|pair| function_interface_order(&pair[0], &pair[1]) != Ordering::Less)
        {
            return Err(SectionCodecError::NonCanonicalTable(
                "runtime_function_interface",
            ));
        }
        Ok(Self {
            abi_version,
            runtime_layout_digest,
            declarations,
            function_interfaces,
        })
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        self.envelope(RuntimeResourceBudget::default())?
            .encode_canonical()
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, RuntimeResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: RuntimeResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let decoded = ProductResourceEnvelope::decode_with_registry(
            bytes,
            ProductSectionCodecKind::RuntimeTypes,
            &runtime_types_registry()?,
            budget.common,
        )?;
        let envelope = decoded.envelope;
        let abi_version = field_u32(&envelope, FIELD_RUNTIME_ABI_VERSION)?;
        let runtime_layout_digest = AwbcDigest(
            field_bytes(&envelope, FIELD_RUNTIME_LAYOUT_DIGEST)?
                .payload
                .as_slice()
                .try_into()
                .map_err(|_| SectionCodecError::Truncated)?,
        );
        let declarations = decode_type_declarations(
            &field_bytes(&envelope, FIELD_RUNTIME_TYPE_DECLARATIONS)?.payload,
            &envelope.public_ids,
            budget,
        )?;
        let function_interfaces = decode_function_interfaces(
            &field_bytes(&envelope, FIELD_RUNTIME_FUNCTION_INTERFACES)?.payload,
            &envelope.public_ids,
            budget,
        )?;
        Ok(Self {
            abi_version,
            runtime_layout_digest,
            declarations,
            function_interfaces,
        })
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn validate_awbc(&self, program: &AwbcProgram) -> Result<(), SectionCodecError> {
        if self.abi_version != program.header.abi_version
            || self.runtime_layout_digest != program.header.runtime_layout_digest
        {
            return Err(SectionCodecError::RuntimeLayoutMismatch);
        }
        let mut program_identities = BTreeSet::new();
        if program
            .runtime_types
            .iter()
            .any(|ty| !program_identities.insert(ty.semantic_identity()))
        {
            return Err(SectionCodecError::NonCanonicalTable(
                "awbc_runtime_type_semantic_identity",
            ));
        }
        let mut expected_declarations = program
            .runtime_types
            .iter()
            .map(|ty| runtime_type_declaration(program, ty))
            .collect::<Result<Vec<_>, _>>()?;
        expected_declarations.sort_by(runtime_type_order);
        if self.declarations != expected_declarations {
            return Err(SectionCodecError::RuntimeLayoutMismatch);
        }
        check_budget(
            self.function_interfaces.len(),
            program.functions.len(),
            "function_interfaces",
        )?;
        if self
            .function_interfaces
            .iter()
            .any(|fingerprint| fingerprint.awbc_function_index as usize >= program.functions.len())
        {
            return Err(SectionCodecError::BudgetExceeded("function_reference"));
        }
        Ok(())
    }

    pub fn compatibility_with(&self, next: &Self) -> RuntimeResourceCompatibility {
        if self == next {
            return RuntimeResourceCompatibility::ContentOnly;
        }
        if self.abi_version != next.abi_version
            || self.runtime_layout_digest != next.runtime_layout_digest
        {
            return RuntimeResourceCompatibility::RestartRequired;
        }
        type_decl_compatibility(&self.declarations, &next.declarations).max(
            function_interface_compatibility(&self.function_interfaces, &next.function_interfaces),
        )
    }

    fn envelope(
        &self,
        budget: RuntimeResourceBudget,
    ) -> Result<ProductResourceEnvelope, SectionCodecError> {
        check_budget(
            self.declarations.len(),
            budget.runtime_types,
            "runtime_types",
        )?;
        check_budget(
            self.function_interfaces.len(),
            budget.function_interfaces,
            "function_interfaces",
        )?;
        if self
            .declarations
            .windows(2)
            .any(|pair| runtime_type_order(&pair[0], &pair[1]) != Ordering::Less)
        {
            return Err(SectionCodecError::NonCanonicalTable(
                "runtime_type_declaration_order",
            ));
        }
        if self
            .function_interfaces
            .windows(2)
            .any(|pair| function_interface_order(&pair[0], &pair[1]) != Ordering::Less)
        {
            return Err(SectionCodecError::NonCanonicalTable(
                "runtime_function_interface_order",
            ));
        }
        let strings = StringTable::new(enum_symbol_names())?;
        let public_ids = PublicIdTable::new(unique_strings(
            self.declarations
                .iter()
                .filter_map(|declaration| declaration.public_id.clone())
                .chain(
                    self.function_interfaces
                        .iter()
                        .filter_map(|fingerprint| fingerprint.public_id.clone()),
                ),
        ))?;
        let enums = enum_registry(&strings)?;
        let fields = [
            ResourceField::required(
                FIELD_RUNTIME_ABI_VERSION,
                ResourceWireType::U32,
                self.abi_version.to_le_bytes(),
            ),
            ResourceField::required(
                FIELD_RUNTIME_LAYOUT_DIGEST,
                ResourceWireType::Bytes,
                self.runtime_layout_digest.0,
            ),
            ResourceField::new(
                FIELD_RUNTIME_TYPE_DECLARATIONS,
                super::field::FieldRequirement::Required,
                ResourceWireType::Bytes,
                1,
                u16_saturating(self.declarations.len()),
                encode_type_declarations(&self.declarations, &public_ids)?,
            ),
            ResourceField::new(
                FIELD_RUNTIME_FUNCTION_INTERFACES,
                super::field::FieldRequirement::Required,
                ResourceWireType::Bytes,
                1,
                u16_saturating(self.function_interfaces.len()),
                encode_function_interfaces(&self.function_interfaces, &public_ids)?,
            ),
        ];
        ProductResourceEnvelope::with_budget(
            ProductSectionCodecKind::RuntimeTypes,
            strings,
            public_ids,
            enums,
            fields,
            u32_saturating(self.declarations.len() + self.function_interfaces.len()),
            budget.common,
        )
    }

    pub fn from_awbc_program(program: &AwbcProgram) -> Result<Self, SectionCodecError> {
        let declarations = program
            .runtime_types
            .iter()
            .map(|ty| runtime_type_declaration(program, ty))
            .collect::<Result<Vec<_>, _>>()?;
        let function_interfaces = program
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| {
                let public_id = function
                    .public_id
                    .map(|id| canonical_awbc_string(program, id).map(str::to_owned))
                    .transpose()?;
                let signature = program.signatures.get(function.signature.index()).ok_or(
                    SectionCodecError::BudgetExceeded("awbc_signature_reference"),
                )?;
                let signature_digest = runtime_signature_digest(program, signature)?;
                let frame_layout = program
                    .frame_layouts
                    .get(function.frame_layout.index())
                    .ok_or(SectionCodecError::BudgetExceeded(
                        "awbc_frame_layout_reference",
                    ))?;
                let frame_layout_digest = runtime_frame_layout_digest(program, frame_layout)?;
                Ok(FunctionInterfaceFingerprint {
                    public_id,
                    awbc_function_index: u32::try_from(index)
                        .map_err(|_| SectionCodecError::LengthOverflow)?,
                    kind: RuntimeFunctionKind::from(function.kind),
                    signature_digest,
                    frame_layout_digest,
                    flags: function.flags.0,
                    compatibility: if function.flags.0
                        & arcweft_core::awbc::schema::AwbcFunctionFlags::HAS_DYNAMIC_TARGET
                        != 0
                    {
                        TypeCompatibilityLabel::CodeGenerational
                    } else {
                        TypeCompatibilityLabel::CodeCompatible
                    },
                })
            })
            .collect::<Result<Vec<_>, SectionCodecError>>()?;
        Self::try_new(
            program.header.abi_version,
            program.header.runtime_layout_digest,
            declarations,
            function_interfaces,
        )
    }
}

impl EntrypointsSection {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Result<Self, SectionCodecError> {
        let entries = entrypoints_from_awbc(bundle.product_awbc_program(), &bundle.manifest)?;
        Ok(Self::new(entries))
    }

    pub fn new(entries: impl IntoIterator<Item = EntrypointDeclaration>) -> Self {
        let mut entries = entries.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.public_id.cmp(&right.public_id));
        Self { entries }
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        self.envelope(RuntimeResourceBudget::default())?
            .encode_canonical()
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, RuntimeResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: RuntimeResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let decoded = ProductResourceEnvelope::decode_with_registry(
            bytes,
            ProductSectionCodecKind::Entrypoints,
            &entrypoints_registry()?,
            budget.common,
        )?;
        let envelope = decoded.envelope;
        let entries = decode_entrypoints(
            &field_bytes(&envelope, FIELD_ENTRYPOINT_RECORDS)?.payload,
            &envelope.strings,
            &envelope.public_ids,
            budget,
        )?;
        Ok(Self::new(entries))
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn validate_manifest(&self, manifest: &BundleManifest) -> Result<(), SectionCodecError> {
        if let Some(entry) = manifest.entry.as_deref()
            && !self.entries.iter().any(|candidate| {
                candidate.public_id == entry
                    || candidate.exported_name.as_deref() == Some(entry)
                    || candidate.public_id == format!("entry.{entry}")
            })
        {
            return Err(SectionCodecError::BudgetExceeded(
                "entry_manifest_reference",
            ));
        }
        Ok(())
    }

    pub fn validate_awbc(&self, program: &AwbcProgram) -> Result<(), SectionCodecError> {
        if self.entries.iter().any(|entry| {
            entry
                .awbc_function_index
                .is_some_and(|index| index as usize >= program.functions.len())
        }) {
            return Err(SectionCodecError::BudgetExceeded(
                "entrypoint_function_reference",
            ));
        }
        Ok(())
    }

    pub fn compatibility_with(&self, next: &Self) -> RuntimeResourceCompatibility {
        if self == next {
            return RuntimeResourceCompatibility::ContentOnly;
        }
        let old = self
            .entries
            .iter()
            .map(|entry| (entry.public_id.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let new = next
            .entries
            .iter()
            .map(|entry| (entry.public_id.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        if old.keys().any(|id| !new.contains_key(id)) {
            return RuntimeResourceCompatibility::RestartRequired;
        }
        let changed_existing = old.iter().any(|(id, left)| {
            new.get(id).is_some_and(|right| {
                left.awbc_function_index != right.awbc_function_index
                    || left.visibility != right.visibility
            })
        });
        if changed_existing {
            RuntimeResourceCompatibility::RestartRequired
        } else if old.len() == new.len() {
            RuntimeResourceCompatibility::ContentOnly
        } else {
            RuntimeResourceCompatibility::CodeCompatible
        }
    }

    fn envelope(
        &self,
        budget: RuntimeResourceBudget,
    ) -> Result<ProductResourceEnvelope, SectionCodecError> {
        check_budget(self.entries.len(), budget.entrypoints, "entrypoints")?;
        let strings = StringTable::new(
            enum_symbol_names()
                .chain(
                    self.entries
                        .iter()
                        .filter_map(|entry| entry.exported_name.clone()),
                )
                .chain(self.entries.iter().filter_map(|entry| {
                    entry
                        .source_anchor
                        .as_ref()
                        .map(|anchor| anchor.source_public_id.clone())
                })),
        )?;
        let public_ids = PublicIdTable::new(unique_strings(
            self.entries.iter().map(|entry| entry.public_id.clone()),
        ))?;
        let enums = enum_registry(&strings)?;
        let fields = [ResourceField::new(
            FIELD_ENTRYPOINT_RECORDS,
            super::field::FieldRequirement::Required,
            ResourceWireType::Bytes,
            1,
            u16_saturating(self.entries.len()),
            encode_entrypoints(&self.entries, &strings, &public_ids)?,
        )];
        ProductResourceEnvelope::with_budget(
            ProductSectionCodecKind::Entrypoints,
            strings,
            public_ids,
            enums,
            fields,
            u32_saturating(self.entries.len()),
            budget.common,
        )
    }
}

impl AdapterRequirementsSection {
    pub fn from_bundle(bundle: &ArcweftBundle) -> Result<Self, SectionCodecError> {
        Ok(Self::new(
            bundle.manifest.adapter.clone(),
            bundle.manifest.adapter_manifest_ids.clone(),
            bundle.manifest.required_host_calls.clone(),
            bundle.adapter_manifests.clone(),
        ))
    }

    pub fn new(
        default_adapter: Option<String>,
        adapter_manifest_ids: impl IntoIterator<Item = String>,
        required_host_calls: impl IntoIterator<Item = String>,
        adapter_manifests: impl IntoIterator<Item = BundleAdapterManifest>,
    ) -> Self {
        let mut adapter_manifest_ids = adapter_manifest_ids.into_iter().collect::<Vec<_>>();
        adapter_manifest_ids.sort();
        adapter_manifest_ids.dedup();
        let mut required_host_calls = required_host_calls.into_iter().collect::<Vec<_>>();
        required_host_calls.sort();
        required_host_calls.dedup();
        let mut adapter_manifests = adapter_manifests.into_iter().collect::<Vec<_>>();
        adapter_manifests.sort_by(|left, right| left.id.cmp(&right.id));
        let required_capabilities = required_host_calls
            .iter()
            .map(|public_id| CapabilityRequirement {
                public_id: public_id.clone(),
                version: VersionRange::ANY,
                feature_flags: Vec::new(),
            })
            .collect::<Vec<_>>();
        let optional_capabilities = optional_capabilities(&adapter_manifests);
        Self {
            default_adapter,
            adapter_manifest_ids,
            required_host_calls,
            adapter_manifests,
            required_capabilities,
            optional_capabilities,
            feature_flags: Vec::new(),
            launch_constraints: Vec::new(),
            platform_refs: Vec::new(),
        }
    }

    pub fn encode_canonical_section(&self) -> Result<Vec<u8>, SectionCodecError> {
        self.envelope(RuntimeResourceBudget::default())?
            .encode_canonical()
    }

    pub fn decode_canonical_section(bytes: &[u8]) -> Result<Self, SectionCodecError> {
        Self::decode_canonical_section_with_budget(bytes, RuntimeResourceBudget::default())
    }

    pub fn decode_canonical_section_with_budget(
        bytes: &[u8],
        budget: RuntimeResourceBudget,
    ) -> Result<Self, SectionCodecError> {
        let decoded = ProductResourceEnvelope::decode_with_registry(
            bytes,
            ProductSectionCodecKind::AdapterRequirements,
            &adapter_requirements_registry()?,
            budget.common,
        )?;
        let envelope = decoded.envelope;
        decode_adapter_requirements(
            &field_bytes(&envelope, FIELD_ADAPTER_REQUIREMENT_RECORDS)?.payload,
            &envelope.strings,
            &envelope.public_ids,
            budget,
        )
    }

    pub fn canonical_digest(&self) -> Result<BundleDigest, SectionCodecError> {
        self.encode_canonical_section()
            .map(|bytes| BundleDigest::of(&bytes))
    }

    pub fn compatibility_with(&self, next: &Self) -> RuntimeResourceCompatibility {
        if self == next {
            return RuntimeResourceCompatibility::ContentOnly;
        }
        if self.default_adapter != next.default_adapter
            || self.adapter_manifest_ids != next.adapter_manifest_ids
            || self.required_host_calls != next.required_host_calls
            || self.required_capabilities != next.required_capabilities
            || self.launch_constraints != next.launch_constraints
            || self.platform_refs != next.platform_refs
        {
            return RuntimeResourceCompatibility::RestartRequired;
        }
        if self.optional_capabilities != next.optional_capabilities
            || self.feature_flags != next.feature_flags
            || self.adapter_manifests != next.adapter_manifests
        {
            RuntimeResourceCompatibility::CodeCompatible
        } else {
            RuntimeResourceCompatibility::ContentOnly
        }
    }

    pub fn apply_to_manifest(&self, manifest: &mut BundleManifest) {
        manifest.adapter.clone_from(&self.default_adapter);
        manifest
            .adapter_manifest_ids
            .clone_from(&self.adapter_manifest_ids);
        manifest
            .required_host_calls
            .clone_from(&self.required_host_calls);
    }

    fn envelope(
        &self,
        budget: RuntimeResourceBudget,
    ) -> Result<ProductResourceEnvelope, SectionCodecError> {
        check_budget(
            self.adapter_manifest_ids.len() + self.required_host_calls.len(),
            budget.adapter_requirements,
            "adapter_requirements",
        )?;
        check_budget(
            self.adapter_manifests.len(),
            budget.adapter_manifests,
            "adapter_manifests",
        )?;
        let strings = StringTable::new(
            enum_symbol_names()
                .chain(self.default_adapter.clone())
                .chain(
                    self.adapter_manifests
                        .iter()
                        .map(|manifest| manifest.display_name.clone()),
                )
                .chain(capability_string_values(&self.required_capabilities))
                .chain(capability_string_values(&self.optional_capabilities))
                .chain(self.feature_flags.iter().cloned())
                .chain(
                    self.platform_refs
                        .iter()
                        .map(|reference| reference.platform.clone()),
                ),
        )?;
        let public_ids = PublicIdTable::new(unique_strings(
            self.adapter_manifest_ids
                .iter()
                .cloned()
                .chain(self.required_host_calls.iter().cloned())
                .chain(
                    self.adapter_manifests
                        .iter()
                        .map(|manifest| manifest.id.clone()),
                )
                .chain(
                    self.adapter_manifests
                        .iter()
                        .flat_map(|manifest| manifest.effects.clone()),
                )
                .chain(self.adapter_manifests.iter().flat_map(|manifest| {
                    manifest.host_calls.iter().flat_map(|host_call| {
                        std::iter::once(host_call.id.clone()).chain(host_call.effects.clone())
                    })
                }))
                .chain(
                    self.required_capabilities
                        .iter()
                        .map(|capability| capability.public_id.clone()),
                )
                .chain(
                    self.optional_capabilities
                        .iter()
                        .map(|capability| capability.public_id.clone()),
                )
                .chain(
                    self.launch_constraints
                        .iter()
                        .map(|constraint| constraint.public_id.clone()),
                )
                .chain(
                    self.platform_refs
                        .iter()
                        .map(|reference| reference.requirement.clone()),
                ),
        ))?;
        let enums = enum_registry(&strings)?;
        let fields = [ResourceField::new(
            FIELD_ADAPTER_REQUIREMENT_RECORDS,
            super::field::FieldRequirement::Required,
            ResourceWireType::Bytes,
            2,
            u16_saturating(
                self.adapter_manifest_ids.len()
                    + self.required_host_calls.len()
                    + self.adapter_manifests.len(),
            ),
            encode_adapter_requirements(self, &strings, &public_ids)?,
        )];
        ProductResourceEnvelope::with_budget(
            ProductSectionCodecKind::AdapterRequirements,
            strings,
            public_ids,
            enums,
            fields,
            u32_saturating(
                self.adapter_manifest_ids.len()
                    + self.required_host_calls.len()
                    + self.adapter_manifests.len(),
            ),
            budget.common,
        )
    }
}

impl RuntimeFunctionKind {
    pub const fn encoded(self) -> u32 {
        match self {
            Self::Flow => 201,
            Self::PureHelper => 202,
            Self::TraitMethod => 203,
            Self::StreamTransform => 204,
            Self::LineTask => 207,
            Self::Synthetic => 208,
        }
    }

    pub const fn from_encoded(value: u32) -> Option<Self> {
        match value {
            201 => Some(Self::Flow),
            202 => Some(Self::PureHelper),
            203 => Some(Self::TraitMethod),
            204 => Some(Self::StreamTransform),
            207 => Some(Self::LineTask),
            208 => Some(Self::Synthetic),
            _ => None,
        }
    }
}

impl From<AwbcFunctionKind> for RuntimeFunctionKind {
    fn from(value: AwbcFunctionKind) -> Self {
        match value {
            AwbcFunctionKind::Flow => Self::Flow,
            AwbcFunctionKind::PureHelper => Self::PureHelper,
            AwbcFunctionKind::TraitMethod => Self::TraitMethod,
            AwbcFunctionKind::StreamTransform => Self::StreamTransform,
            AwbcFunctionKind::LineTask => Self::LineTask,
            AwbcFunctionKind::Synthetic => Self::Synthetic,
        }
    }
}

impl RuntimeValueKind {
    pub const fn encoded(self) -> u32 {
        match self {
            Self::Unit => 101,
            Self::Bool => 102,
            Self::SignedInteger => 103,
            Self::UnsignedInteger => 104,
            Self::Float => 105,
            Self::Text => 106,
            Self::Duration => 107,
            Self::EntityRef => 108,
            Self::Tuple => 109,
            Self::Sequence => 110,
            Self::Record => 111,
            Self::Variant => 112,
            Self::Matrix => 113,
            Self::Tensor => 114,
            Self::Task => 115,
            Self::Need => 116,
            Self::Dynamic => 117,
            Self::Choice => 118,
            Self::Nominal => 119,
            Self::Opaque => 120,
            Self::Agent => 121,
            Self::Progress => 122,
            Self::Range => 123,
            Self::Iterator => 124,
            Self::Array => 125,
            Self::Map => 126,
            Self::Stream => 127,
            Self::Shared => 128,
            Self::Reference => 129,
            Self::Function => 130,
        }
    }

    pub const fn from_encoded(value: u32) -> Option<Self> {
        match value {
            101 => Some(Self::Unit),
            102 => Some(Self::Bool),
            103 => Some(Self::SignedInteger),
            104 => Some(Self::UnsignedInteger),
            105 => Some(Self::Float),
            106 => Some(Self::Text),
            107 => Some(Self::Duration),
            108 => Some(Self::EntityRef),
            109 => Some(Self::Tuple),
            110 => Some(Self::Sequence),
            111 => Some(Self::Record),
            112 => Some(Self::Variant),
            113 => Some(Self::Matrix),
            114 => Some(Self::Tensor),
            115 => Some(Self::Task),
            116 => Some(Self::Need),
            117 => Some(Self::Dynamic),
            118 => Some(Self::Choice),
            119 => Some(Self::Nominal),
            120 => Some(Self::Opaque),
            121 => Some(Self::Agent),
            122 => Some(Self::Progress),
            123 => Some(Self::Range),
            124 => Some(Self::Iterator),
            125 => Some(Self::Array),
            126 => Some(Self::Map),
            127 => Some(Self::Stream),
            128 => Some(Self::Shared),
            129 => Some(Self::Reference),
            130 => Some(Self::Function),
            _ => None,
        }
    }
}

impl TypeCompatibilityLabel {
    pub const fn encoded(self) -> u32 {
        match self {
            Self::ContentOnly => 1,
            Self::CodeCompatible => 2,
            Self::CodeGenerational => 3,
            Self::RestartRequired => 4,
        }
    }

    pub const fn from_encoded(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::ContentOnly),
            2 => Some(Self::CodeCompatible),
            3 => Some(Self::CodeGenerational),
            4 => Some(Self::RestartRequired),
            _ => None,
        }
    }
}

impl ProductVisibility {
    pub const fn encoded(self) -> u32 {
        match self {
            Self::Public => 301,
            Self::Hidden => 302,
            Self::TestOnly => 303,
        }
    }

    pub const fn from_encoded(value: u32) -> Option<Self> {
        match value {
            301 => Some(Self::Public),
            302 => Some(Self::Hidden),
            303 => Some(Self::TestOnly),
            _ => None,
        }
    }
}

fn runtime_types_registry() -> Result<FieldRegistry, SectionCodecError> {
    FieldRegistry::new([
        FieldSpec::required(FIELD_RUNTIME_ABI_VERSION, ResourceWireType::U32),
        FieldSpec::required(FIELD_RUNTIME_LAYOUT_DIGEST, ResourceWireType::Bytes),
        FieldSpec::required(FIELD_RUNTIME_TYPE_DECLARATIONS, ResourceWireType::Bytes),
        FieldSpec::required(FIELD_RUNTIME_FUNCTION_INTERFACES, ResourceWireType::Bytes),
    ])
}

fn entrypoints_registry() -> Result<FieldRegistry, SectionCodecError> {
    FieldRegistry::new([FieldSpec::required(
        FIELD_ENTRYPOINT_RECORDS,
        ResourceWireType::Bytes,
    )])
}

fn adapter_requirements_registry() -> Result<FieldRegistry, SectionCodecError> {
    FieldRegistry::new([FieldSpec::required(
        FIELD_ADAPTER_REQUIREMENT_RECORDS,
        ResourceWireType::Bytes,
    )])
}

fn runtime_type_declaration(
    program: &AwbcProgram,
    ty: &AwbcRuntimeType,
) -> Result<RuntimeTypeDeclaration, SectionCodecError> {
    Ok(RuntimeTypeDeclaration {
        semantic_identity: ty.semantic_identity(),
        public_id: runtime_type_public_id(program, ty)?,
        value_kind: runtime_value_kind(ty),
        layout_digest: runtime_type_layout_digest(program, ty)?,
        compatibility: match ty.shape() {
            AwbcRuntimeTypeShape::Record { .. }
            | AwbcRuntimeTypeShape::Variant { .. }
            | AwbcRuntimeTypeShape::Nominal { .. }
            | AwbcRuntimeTypeShape::NominalRecord { .. }
            | AwbcRuntimeTypeShape::Opaque { .. }
            | AwbcRuntimeTypeShape::Agent(_) => TypeCompatibilityLabel::RestartRequired,
            _ => TypeCompatibilityLabel::CodeCompatible,
        },
    })
}

fn runtime_type_public_id(
    program: &AwbcProgram,
    ty: &AwbcRuntimeType,
) -> Result<Option<String>, SectionCodecError> {
    let id = match ty.shape() {
        AwbcRuntimeTypeShape::Record { public_id, .. } => {
            return public_id
                .map(|id| canonical_awbc_string(program, id).map(str::to_owned))
                .transpose();
        }
        AwbcRuntimeTypeShape::Variant { owner, .. } => match owner {
            AwbcVariantIdentity::Nominal { public_id, .. } => Some(*public_id),
            AwbcVariantIdentity::Option | AwbcVariantIdentity::Result => return Ok(None),
        },
        AwbcRuntimeTypeShape::Nominal { public_id, .. }
        | AwbcRuntimeTypeShape::NominalRecord { public_id, .. } => Some(*public_id),
        _ => return Ok(None),
    };
    id.map(|id| canonical_awbc_string(program, id).map(str::to_owned))
        .transpose()
}

/// Canonical, table-position-independent executable layout identity.
///
/// AWBC table indices are generation-local coordinates. Runtime compatibility
/// therefore commits referenced strings by value and child types by their
/// total semantic identity. This transcript is the sole layout authority used
/// by the compact RuntimeTypes section.
fn runtime_type_layout_digest(
    program: &AwbcProgram,
    ty: &AwbcRuntimeType,
) -> Result<BundleDigest, SectionCodecError> {
    let mut transcript = CanonicalAwbcTranscript::new(b"arcweft.runtime-type-layout.v1\0");
    transcript.write_semantic_type(ty.semantic_identity());
    match ty.shape() {
        AwbcRuntimeTypeShape::Unit => transcript.write_tag(0),
        AwbcRuntimeTypeShape::Bool => transcript.write_tag(1),
        AwbcRuntimeTypeShape::Int(kind) => {
            transcript.write_tag(2);
            transcript.write_u8(signed_int_kind_tag(*kind));
        }
        AwbcRuntimeTypeShape::UInt(kind) => {
            transcript.write_tag(3);
            transcript.write_u8(unsigned_int_kind_tag(*kind));
        }
        AwbcRuntimeTypeShape::F32 => transcript.write_tag(4),
        AwbcRuntimeTypeShape::F64 => transcript.write_tag(5),
        AwbcRuntimeTypeShape::String => transcript.write_tag(6),
        AwbcRuntimeTypeShape::Char => transcript.write_tag(7),
        AwbcRuntimeTypeShape::Duration => transcript.write_tag(8),
        AwbcRuntimeTypeShape::Progress => transcript.write_tag(9),
        AwbcRuntimeTypeShape::EntityRef => transcript.write_tag(10),
        AwbcRuntimeTypeShape::Tuple(items) => {
            transcript.write_tag(11);
            transcript.write_type_list(program, items)?;
        }
        AwbcRuntimeTypeShape::Sequence(item) => {
            transcript.write_tag(12);
            transcript.write_type(program, *item)?;
        }
        AwbcRuntimeTypeShape::Record { public_id, fields } => {
            transcript.write_tag(13);
            transcript.write_optional_string(program, *public_id)?;
            transcript.write_len(fields.len())?;
            for field in fields {
                transcript.write_string(program, field.name)?;
                transcript.write_type(program, field.ty)?;
            }
        }
        AwbcRuntimeTypeShape::Variant {
            owner,
            arguments,
            cases,
        } => {
            transcript.write_tag(14);
            match owner {
                AwbcVariantIdentity::Nominal { public_id } => {
                    transcript.write_u8(0);
                    transcript.write_string(program, *public_id)?;
                }
                AwbcVariantIdentity::Option => transcript.write_u8(1),
                AwbcVariantIdentity::Result => transcript.write_u8(2),
            }
            transcript.write_type_list(program, arguments)?;
            transcript.write_len(cases.len())?;
            for case in cases {
                transcript.write_string(program, case.name)?;
                transcript.write_optional_type(program, case.payload)?;
            }
        }
        AwbcRuntimeTypeShape::Choice(alternatives) => {
            transcript.write_tag(15);
            transcript.write_type_list(program, alternatives)?;
        }
        AwbcRuntimeTypeShape::Nominal {
            public_id,
            layout,
            arguments,
        } => {
            transcript.write_tag(16);
            transcript.write_string(program, *public_id)?;
            transcript.write_bytes(layout);
            transcript.write_type_list(program, arguments)?;
        }
        AwbcRuntimeTypeShape::NominalRecord {
            public_id,
            layout,
            arguments,
            fields,
        } => {
            transcript.write_tag(17);
            transcript.write_string(program, *public_id)?;
            transcript.write_bytes(layout);
            transcript.write_type_list(program, arguments)?;
            transcript.write_len(fields.len())?;
            for field in fields {
                transcript.write_string(program, field.name)?;
                transcript.write_type(program, field.ty)?;
            }
        }
        AwbcRuntimeTypeShape::Opaque {
            producer,
            admission,
            value_class,
            persistence,
            arguments,
        } => {
            transcript.write_tag(18);
            transcript.write_string(program, *producer)?;
            transcript.write_u8(*admission as u8);
            transcript.write_u8(value_class.semantic_tag());
            transcript.write_u8(persistence.semantic_tag());
            transcript.write_type_list(program, arguments)?;
        }
        AwbcRuntimeTypeShape::Bytes => transcript.write_tag(19),
        AwbcRuntimeTypeShape::Never => transcript.write_tag(20),
        AwbcRuntimeTypeShape::Agent(agent) => {
            transcript.write_tag(21);
            match agent {
                AwbcAgentTypeShape::Leaf(kind) => {
                    transcript.write_u8(0);
                    transcript.write_u8(kind.semantic_tag());
                }
                AwbcAgentTypeShape::Probe(item) => {
                    transcript.write_u8(1);
                    transcript.write_type(program, *item)?;
                }
            }
        }
        AwbcRuntimeTypeShape::MatrixF32 => transcript.write_tag(22),
        AwbcRuntimeTypeShape::MatrixF64 => transcript.write_tag(23),
        AwbcRuntimeTypeShape::TensorF32 => transcript.write_tag(24),
        AwbcRuntimeTypeShape::TensorF64 => transcript.write_tag(25),
        AwbcRuntimeTypeShape::Range(item) => {
            transcript.write_tag(26);
            transcript.write_type(program, *item)?;
        }
        AwbcRuntimeTypeShape::Iterator(item) => {
            transcript.write_tag(27);
            transcript.write_type(program, *item)?;
        }
        AwbcRuntimeTypeShape::Array { item, length } => {
            transcript.write_tag(28);
            transcript.write_type(program, *item)?;
            transcript.write_u64(*length);
        }
        AwbcRuntimeTypeShape::Map { key, value } => {
            transcript.write_tag(29);
            transcript.write_type(program, *key)?;
            transcript.write_type(program, *value)?;
        }
        AwbcRuntimeTypeShape::Need(item) => {
            transcript.write_tag(30);
            transcript.write_type(program, *item)?;
        }
        AwbcRuntimeTypeShape::Task(item) => {
            transcript.write_tag(31);
            transcript.write_type(program, *item)?;
        }
        AwbcRuntimeTypeShape::Stream { item, error } => {
            transcript.write_tag(32);
            transcript.write_type(program, *item)?;
            transcript.write_type(program, *error)?;
        }
        AwbcRuntimeTypeShape::Shared(item) => {
            transcript.write_tag(33);
            transcript.write_type(program, *item)?;
        }
        AwbcRuntimeTypeShape::Reference(item) => {
            transcript.write_tag(34);
            transcript.write_type(program, *item)?;
        }
        AwbcRuntimeTypeShape::Function { parameters, result } => {
            transcript.write_tag(35);
            transcript.write_type_list(program, parameters)?;
            transcript.write_type(program, *result)?;
        }
        AwbcRuntimeTypeShape::Dynamic => transcript.write_tag(36),
    }
    Ok(transcript.finish())
}

fn runtime_signature_digest(
    program: &AwbcProgram,
    signature: &AwbcSignature,
) -> Result<BundleDigest, SectionCodecError> {
    let mut transcript = CanonicalAwbcTranscript::new(b"arcweft.runtime-signature.v1\0");
    transcript.write_type_list(program, &signature.params)?;
    transcript.write_optional_type(program, signature.result)?;
    let effects = program.effect_sets.get(signature.effects.index()).ok_or(
        SectionCodecError::BudgetExceeded("awbc_effect_set_reference"),
    )?;
    transcript.write_len(effects.effects.len())?;
    for effect in &effects.effects {
        transcript.write_string(program, *effect)?;
    }
    Ok(transcript.finish())
}

fn runtime_frame_layout_digest(
    program: &AwbcProgram,
    layout: &AwbcFrameLayout,
) -> Result<BundleDigest, SectionCodecError> {
    let mut transcript = CanonicalAwbcTranscript::new(b"arcweft.runtime-frame-layout.v1\0");
    transcript.write_len(layout.slots.len())?;
    for slot in &layout.slots {
        transcript.write_optional_string(program, slot.name)?;
        transcript.write_type(program, slot.ty)?;
        transcript.write_u8(match slot.role {
            AwbcFrameSlotRole::Parameter => 0,
            AwbcFrameSlotRole::Local => 1,
            AwbcFrameSlotRole::Temporary => 2,
            AwbcFrameSlotRole::ReturnValue => 3,
            AwbcFrameSlotRole::RuntimeState => 4,
        });
        transcript.write_u32(slot.scope_depth);
    }
    transcript.write_u32(layout.max_scope_depth);
    Ok(transcript.finish())
}

fn canonical_awbc_string(
    program: &AwbcProgram,
    id: AwbcStringId,
) -> Result<&str, SectionCodecError> {
    program
        .strings
        .get(id.index())
        .map(String::as_str)
        .ok_or(SectionCodecError::BudgetExceeded("awbc_string_reference"))
}

struct CanonicalAwbcTranscript {
    bytes: Vec<u8>,
}

impl CanonicalAwbcTranscript {
    fn new(domain: &'static [u8]) -> Self {
        Self {
            bytes: domain.to_vec(),
        }
    }

    fn write_tag(&mut self, tag: u16) {
        self.bytes.extend_from_slice(&tag.to_le_bytes());
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_len(&mut self, value: usize) -> Result<(), SectionCodecError> {
        self.write_u32(u32::try_from(value).map_err(|_| SectionCodecError::LengthOverflow)?);
        Ok(())
    }

    fn write_bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn write_str(&mut self, value: &str) -> Result<(), SectionCodecError> {
        self.write_len(value.len())?;
        self.write_bytes(value.as_bytes());
        Ok(())
    }

    fn write_string(
        &mut self,
        program: &AwbcProgram,
        id: AwbcStringId,
    ) -> Result<(), SectionCodecError> {
        self.write_str(canonical_awbc_string(program, id)?)
    }

    fn write_optional_string(
        &mut self,
        program: &AwbcProgram,
        id: Option<AwbcStringId>,
    ) -> Result<(), SectionCodecError> {
        match id {
            Some(id) => {
                self.write_u8(1);
                self.write_string(program, id)
            }
            None => {
                self.write_u8(0);
                Ok(())
            }
        }
    }

    fn write_semantic_type(&mut self, identity: RuntimeSemanticTypeId) {
        self.write_bytes(identity.as_bytes());
    }

    fn write_type(
        &mut self,
        program: &AwbcProgram,
        id: AwbcTypeId,
    ) -> Result<(), SectionCodecError> {
        let ty = program
            .runtime_types
            .get(id.index())
            .ok_or(SectionCodecError::BudgetExceeded(
                "awbc_runtime_type_reference",
            ))?;
        self.write_semantic_type(ty.semantic_identity());
        Ok(())
    }

    fn write_optional_type(
        &mut self,
        program: &AwbcProgram,
        id: Option<AwbcTypeId>,
    ) -> Result<(), SectionCodecError> {
        match id {
            Some(id) => {
                self.write_u8(1);
                self.write_type(program, id)
            }
            None => {
                self.write_u8(0);
                Ok(())
            }
        }
    }

    fn write_type_list(
        &mut self,
        program: &AwbcProgram,
        ids: &[AwbcTypeId],
    ) -> Result<(), SectionCodecError> {
        self.write_len(ids.len())?;
        for id in ids {
            self.write_type(program, *id)?;
        }
        Ok(())
    }

    fn finish(self) -> BundleDigest {
        BundleDigest::of(&self.bytes)
    }
}

const fn signed_int_kind_tag(kind: AwbcSignedIntKind) -> u8 {
    match kind {
        AwbcSignedIntKind::I8 => 0,
        AwbcSignedIntKind::I16 => 1,
        AwbcSignedIntKind::I32 => 2,
        AwbcSignedIntKind::I64 => 3,
        AwbcSignedIntKind::I128 => 4,
        AwbcSignedIntKind::ISize => 5,
    }
}

const fn unsigned_int_kind_tag(kind: AwbcUnsignedIntKind) -> u8 {
    match kind {
        AwbcUnsignedIntKind::U8 => 0,
        AwbcUnsignedIntKind::U16 => 1,
        AwbcUnsignedIntKind::U32 => 2,
        AwbcUnsignedIntKind::U64 => 3,
        AwbcUnsignedIntKind::U128 => 4,
        AwbcUnsignedIntKind::USize => 5,
    }
}

fn runtime_value_kind(ty: &AwbcRuntimeType) -> RuntimeValueKind {
    match ty.shape() {
        AwbcRuntimeTypeShape::Unit => RuntimeValueKind::Unit,
        AwbcRuntimeTypeShape::Bool => RuntimeValueKind::Bool,
        AwbcRuntimeTypeShape::Int(_) => RuntimeValueKind::SignedInteger,
        AwbcRuntimeTypeShape::UInt(_) => RuntimeValueKind::UnsignedInteger,
        AwbcRuntimeTypeShape::F32 | AwbcRuntimeTypeShape::F64 => RuntimeValueKind::Float,
        AwbcRuntimeTypeShape::String | AwbcRuntimeTypeShape::Char => RuntimeValueKind::Text,
        AwbcRuntimeTypeShape::Duration => RuntimeValueKind::Duration,
        AwbcRuntimeTypeShape::Progress => RuntimeValueKind::Progress,
        AwbcRuntimeTypeShape::EntityRef => RuntimeValueKind::EntityRef,
        AwbcRuntimeTypeShape::Tuple(_) => RuntimeValueKind::Tuple,
        AwbcRuntimeTypeShape::Sequence(_) | AwbcRuntimeTypeShape::Bytes => {
            RuntimeValueKind::Sequence
        }
        AwbcRuntimeTypeShape::Record { .. } => RuntimeValueKind::Record,
        AwbcRuntimeTypeShape::Variant { .. } => RuntimeValueKind::Variant,
        AwbcRuntimeTypeShape::Choice(_) | AwbcRuntimeTypeShape::Never => RuntimeValueKind::Choice,
        AwbcRuntimeTypeShape::Nominal { .. } | AwbcRuntimeTypeShape::NominalRecord { .. } => {
            RuntimeValueKind::Nominal
        }
        AwbcRuntimeTypeShape::Opaque { .. } => RuntimeValueKind::Opaque,
        AwbcRuntimeTypeShape::Agent(_) => RuntimeValueKind::Agent,
        AwbcRuntimeTypeShape::MatrixF32 | AwbcRuntimeTypeShape::MatrixF64 => {
            RuntimeValueKind::Matrix
        }
        AwbcRuntimeTypeShape::TensorF32 | AwbcRuntimeTypeShape::TensorF64 => {
            RuntimeValueKind::Tensor
        }
        AwbcRuntimeTypeShape::Range(_) => RuntimeValueKind::Range,
        AwbcRuntimeTypeShape::Iterator(_) => RuntimeValueKind::Iterator,
        AwbcRuntimeTypeShape::Array { .. } => RuntimeValueKind::Array,
        AwbcRuntimeTypeShape::Map { .. } => RuntimeValueKind::Map,
        AwbcRuntimeTypeShape::Need(_) => RuntimeValueKind::Need,
        AwbcRuntimeTypeShape::Task(_) => RuntimeValueKind::Task,
        AwbcRuntimeTypeShape::Stream { .. } => RuntimeValueKind::Stream,
        AwbcRuntimeTypeShape::Shared(_) => RuntimeValueKind::Shared,
        AwbcRuntimeTypeShape::Reference(_) => RuntimeValueKind::Reference,
        AwbcRuntimeTypeShape::Function { .. } => RuntimeValueKind::Function,
        AwbcRuntimeTypeShape::Dynamic => RuntimeValueKind::Dynamic,
    }
}

fn entrypoints_from_awbc(
    program: &AwbcProgram,
    manifest: &BundleManifest,
) -> Result<Vec<EntrypointDeclaration>, SectionCodecError> {
    program
        .entries
        .iter()
        .map(|entry| {
            let public_id = program
                .strings
                .get(entry.public_id.index())
                .cloned()
                .ok_or(SectionCodecError::PublicIdOutOfBounds(PublicIdRef(
                    entry.public_id.0,
                )))?;
            Ok(EntrypointDeclaration {
                exported_name: manifest
                    .entry
                    .as_ref()
                    .filter(|name| *name == &public_id || format!("entry.{name}") == public_id)
                    .cloned(),
                awbc_function_index: match &entry.target {
                    AwbcEntryTarget::Function { function, .. } => Some(function.0),
                    AwbcEntryTarget::Routes(routes) => routes.first().map(|route| route.target.0),
                },
                source_anchor: None,
                visibility: match &entry.kind {
                    AwbcEntryKind::Test | AwbcEntryKind::Bench => ProductVisibility::TestOnly,
                    _ => ProductVisibility::Public,
                },
                public_id,
            })
        })
        .collect()
}

fn optional_capabilities(manifests: &[BundleAdapterManifest]) -> Vec<CapabilityRequirement> {
    let mut capabilities = manifests
        .iter()
        .flat_map(|manifest| manifest.effects.iter().cloned())
        .map(|public_id| CapabilityRequirement {
            public_id,
            version: VersionRange::ANY,
            feature_flags: Vec::new(),
        })
        .collect::<Vec<_>>();
    capabilities.sort_by(|left, right| left.public_id.cmp(&right.public_id));
    capabilities.dedup_by(|left, right| left.public_id == right.public_id);
    capabilities
}

fn type_decl_compatibility(
    old: &[RuntimeTypeDeclaration],
    new: &[RuntimeTypeDeclaration],
) -> RuntimeResourceCompatibility {
    let old = type_decl_index(old);
    let new = type_decl_index(new);
    if old.keys().any(|id| !new.contains_key(id)) {
        return RuntimeResourceCompatibility::RestartRequired;
    }
    old.iter().fold(
        if old.len() == new.len() {
            RuntimeResourceCompatibility::ContentOnly
        } else {
            RuntimeResourceCompatibility::CodeCompatible
        },
        |compatibility, (id, left)| {
            let Some(right) = new.get(id) else {
                return RuntimeResourceCompatibility::RestartRequired;
            };
            if left.layout_digest == right.layout_digest && left.value_kind == right.value_kind {
                compatibility
            } else {
                compatibility.max(right.compatibility.runtime_compatibility())
            }
        },
    )
}

fn function_interface_compatibility(
    old: &[FunctionInterfaceFingerprint],
    new: &[FunctionInterfaceFingerprint],
) -> RuntimeResourceCompatibility {
    let old = function_interface_index(old);
    let new = function_interface_index(new);
    if old.keys().any(|id| !new.contains_key(id)) {
        return RuntimeResourceCompatibility::RestartRequired;
    }
    old.iter().fold(
        if old.len() == new.len() {
            RuntimeResourceCompatibility::ContentOnly
        } else {
            RuntimeResourceCompatibility::CodeCompatible
        },
        |compatibility, (id, left)| {
            let Some(right) = new.get(id) else {
                return RuntimeResourceCompatibility::RestartRequired;
            };
            if left.signature_digest == right.signature_digest
                && left.frame_layout_digest == right.frame_layout_digest
                && left.kind == right.kind
                && left.flags == right.flags
            {
                compatibility
            } else {
                compatibility.max(right.compatibility.runtime_compatibility())
            }
        },
    )
}

fn type_decl_index(
    declarations: &[RuntimeTypeDeclaration],
) -> BTreeMap<RuntimeSemanticTypeId, &RuntimeTypeDeclaration> {
    declarations
        .iter()
        .map(|declaration| (declaration.semantic_identity, declaration))
        .collect()
}

fn function_interface_index(
    fingerprints: &[FunctionInterfaceFingerprint],
) -> BTreeMap<String, &FunctionInterfaceFingerprint> {
    fingerprints
        .iter()
        .map(|fingerprint| {
            let key = fingerprint
                .public_id
                .clone()
                .unwrap_or_else(|| format!("__function_{}", fingerprint.awbc_function_index));
            (key, fingerprint)
        })
        .collect()
}

fn runtime_type_order(left: &RuntimeTypeDeclaration, right: &RuntimeTypeDeclaration) -> Ordering {
    left.semantic_identity
        .cmp(&right.semantic_identity)
        .then_with(|| left.public_id.cmp(&right.public_id))
        .then_with(|| left.value_kind.cmp(&right.value_kind))
        .then_with(|| left.layout_digest.cmp(&right.layout_digest))
}

fn function_interface_order(
    left: &FunctionInterfaceFingerprint,
    right: &FunctionInterfaceFingerprint,
) -> Ordering {
    left.public_id
        .cmp(&right.public_id)
        .then_with(|| left.awbc_function_index.cmp(&right.awbc_function_index))
}

fn enum_symbol_specs() -> impl Iterator<Item = (u32, &'static str)> {
    [
        (1, "content_only"),
        (2, "code_compatible"),
        (3, "code_generational"),
        (4, "restart_required"),
        (101, "unit"),
        (102, "bool"),
        (103, "signed_integer"),
        (104, "unsigned_integer"),
        (105, "float"),
        (106, "text"),
        (107, "duration"),
        (108, "entity_ref"),
        (109, "tuple"),
        (110, "sequence"),
        (111, "record"),
        (112, "variant"),
        (113, "matrix"),
        (114, "tensor"),
        (115, "task"),
        (116, "need"),
        (117, "dynamic"),
        (118, "choice"),
        (119, "nominal"),
        (120, "opaque"),
        (121, "agent"),
        (122, "progress"),
        (123, "range"),
        (124, "iterator"),
        (125, "array"),
        (126, "map"),
        (127, "stream"),
        (128, "shared"),
        (129, "reference"),
        (130, "function"),
        (201, "flow"),
        (202, "pure_helper"),
        (203, "trait_method"),
        (204, "stream_transform"),
        (207, "line_task"),
        (208, "synthetic"),
        (301, "public"),
        (302, "hidden"),
        (303, "test_only"),
        (401, "none"),
        (402, "root_bindings"),
        (403, "host_prepared"),
    ]
    .into_iter()
}

fn enum_symbol_names() -> impl Iterator<Item = String> {
    enum_symbol_specs().map(|(_, name)| name.to_owned())
}

fn enum_registry(strings: &StringTable) -> Result<EnumRegistry, SectionCodecError> {
    let symbols = enum_symbol_specs()
        .map(|(code, name)| {
            required_string_ref(strings, name).map(|name| EnumSymbol {
                code,
                name: StringId(name),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    EnumRegistry::with_budget(symbols, strings, SectionCodecBudget::default())
}

fn required_string_ref(strings: &StringTable, value: &str) -> Result<u32, SectionCodecError> {
    strings
        .id_for(value)
        .map(|id| id.0)
        .ok_or(SectionCodecError::NonCanonicalTable("string_ref"))
}

fn optional_string_ref(
    strings: &StringTable,
    value: Option<&str>,
) -> Result<u32, SectionCodecError> {
    value.map_or(Ok(NONE_REF), |value| required_string_ref(strings, value))
}

fn required_public_ref(public_ids: &PublicIdTable, value: &str) -> Result<u32, SectionCodecError> {
    public_ids
        .id_for(value)
        .map(|id| id.0)
        .ok_or(SectionCodecError::NonCanonicalTable("public_id_ref"))
}

fn optional_public_ref(
    public_ids: &PublicIdTable,
    value: Option<&str>,
) -> Result<u32, SectionCodecError> {
    value.map_or(Ok(NONE_REF), |value| required_public_ref(public_ids, value))
}

fn public_ref_value(table: &PublicIdTable, raw: u32) -> Result<Option<String>, SectionCodecError> {
    if raw == NONE_REF {
        Ok(None)
    } else {
        table
            .get(PublicIdRef(raw))
            .map(|value| Some(value.to_owned()))
    }
}

fn string_ref_value(table: &StringTable, raw: u32) -> Result<Option<String>, SectionCodecError> {
    if raw == NONE_REF {
        Ok(None)
    } else {
        table.get(StringId(raw)).map(|value| Some(value.to_owned()))
    }
}

fn field(
    envelope: &ProductResourceEnvelope,
    id: FieldId,
) -> Result<&ResourceField, SectionCodecError> {
    envelope
        .fields
        .iter()
        .find(|field| field.id == id)
        .ok_or(SectionCodecError::MissingRequiredField(id))
}

fn field_bytes(
    envelope: &ProductResourceEnvelope,
    id: FieldId,
) -> Result<&ResourceField, SectionCodecError> {
    let field = field(envelope, id)?;
    if field.wire_type == ResourceWireType::Bytes {
        Ok(field)
    } else {
        Err(SectionCodecError::FieldWireTypeMismatch {
            field: id,
            expected: ResourceWireType::Bytes,
            actual: field.wire_type,
        })
    }
}

fn field_u32(envelope: &ProductResourceEnvelope, id: FieldId) -> Result<u32, SectionCodecError> {
    let field = field(envelope, id)?;
    if field.wire_type != ResourceWireType::U32 || field.payload.len() != 4 {
        return Err(SectionCodecError::FieldWireTypeMismatch {
            field: id,
            expected: ResourceWireType::U32,
            actual: field.wire_type,
        });
    }
    Ok(u32::from_le_bytes(
        field
            .payload
            .as_slice()
            .try_into()
            .map_err(|_| SectionCodecError::Truncated)?,
    ))
}

fn encode_type_declarations(
    declarations: &[RuntimeTypeDeclaration],
    public_ids: &PublicIdTable,
) -> Result<Vec<u8>, SectionCodecError> {
    let mut out = Vec::new();
    write_u32(&mut out, u32_saturating(declarations.len()));
    for declaration in declarations {
        out.extend_from_slice(declaration.semantic_identity.as_bytes());
        write_u32(
            &mut out,
            optional_public_ref(public_ids, declaration.public_id.as_deref())?,
        );
        write_u32(&mut out, declaration.value_kind.encoded());
        write_u32(&mut out, declaration.compatibility.encoded());
        out.extend_from_slice(&declaration.layout_digest.as_bytes());
    }
    Ok(out)
}

fn decode_type_declarations(
    bytes: &[u8],
    public_ids: &PublicIdTable,
    budget: RuntimeResourceBudget,
) -> Result<Vec<RuntimeTypeDeclaration>, SectionCodecError> {
    let mut reader = PayloadReader::new(bytes);
    let count = reader.read_u32()? as usize;
    check_budget(count, budget.runtime_types, "runtime_types")?;
    let mut declarations = Vec::with_capacity(count);
    let mut previous_identity = None;
    for _ in 0..count {
        let semantic_identity = RuntimeSemanticTypeId::from_bytes(reader.read_array()?);
        if previous_identity.is_some_and(|previous| previous >= semantic_identity) {
            return Err(SectionCodecError::NonCanonicalTable(
                "runtime_type_declaration_order",
            ));
        }
        previous_identity = Some(semantic_identity);
        let public_id = public_ref_value(public_ids, reader.read_u32()?)?;
        let value_kind = RuntimeValueKind::from_encoded(reader.read_u32()?)
            .ok_or(SectionCodecError::NonCanonicalTable("runtime_value_kind"))?;
        let compatibility = TypeCompatibilityLabel::from_encoded(reader.read_u32()?)
            .ok_or(SectionCodecError::NonCanonicalTable("type_compatibility"))?;
        let layout_digest = BundleDigest::from_bytes(reader.read_array()?);
        declarations.push(RuntimeTypeDeclaration {
            semantic_identity,
            public_id,
            value_kind,
            layout_digest,
            compatibility,
        });
    }
    if declarations
        .windows(2)
        .any(|pair| runtime_type_order(&pair[0], &pair[1]) != Ordering::Less)
    {
        return Err(SectionCodecError::NonCanonicalTable(
            "runtime_type_declaration_order",
        ));
    }
    reader.finish()?;
    Ok(declarations)
}

fn encode_function_interfaces(
    fingerprints: &[FunctionInterfaceFingerprint],
    public_ids: &PublicIdTable,
) -> Result<Vec<u8>, SectionCodecError> {
    let mut out = Vec::new();
    write_u32(&mut out, u32_saturating(fingerprints.len()));
    for fingerprint in fingerprints {
        write_u32(
            &mut out,
            optional_public_ref(public_ids, fingerprint.public_id.as_deref())?,
        );
        write_u32(&mut out, fingerprint.awbc_function_index);
        write_u32(&mut out, fingerprint.kind.encoded());
        write_u32(&mut out, fingerprint.flags);
        write_u32(&mut out, fingerprint.compatibility.encoded());
        out.extend_from_slice(&fingerprint.signature_digest.as_bytes());
        out.extend_from_slice(&fingerprint.frame_layout_digest.as_bytes());
    }
    Ok(out)
}

fn decode_function_interfaces(
    bytes: &[u8],
    public_ids: &PublicIdTable,
    budget: RuntimeResourceBudget,
) -> Result<Vec<FunctionInterfaceFingerprint>, SectionCodecError> {
    let mut reader = PayloadReader::new(bytes);
    let count = reader.read_u32()? as usize;
    check_budget(count, budget.function_interfaces, "function_interfaces")?;
    let mut fingerprints = Vec::with_capacity(count);
    for _ in 0..count {
        let public_id = public_ref_value(public_ids, reader.read_u32()?)?;
        let awbc_function_index = reader.read_u32()?;
        let kind = RuntimeFunctionKind::from_encoded(reader.read_u32()?).ok_or(
            SectionCodecError::NonCanonicalTable("runtime_function_kind"),
        )?;
        let flags = reader.read_u32()?;
        let compatibility = TypeCompatibilityLabel::from_encoded(reader.read_u32()?).ok_or(
            SectionCodecError::NonCanonicalTable("function_compatibility"),
        )?;
        let signature_digest = BundleDigest::from_bytes(reader.read_array()?);
        let frame_layout_digest = BundleDigest::from_bytes(reader.read_array()?);
        fingerprints.push(FunctionInterfaceFingerprint {
            public_id,
            awbc_function_index,
            kind,
            signature_digest,
            frame_layout_digest,
            flags,
            compatibility,
        });
    }
    if fingerprints
        .windows(2)
        .any(|pair| function_interface_order(&pair[0], &pair[1]) != Ordering::Less)
    {
        return Err(SectionCodecError::NonCanonicalTable(
            "runtime_function_interface_order",
        ));
    }
    reader.finish()?;
    Ok(fingerprints)
}

fn encode_entrypoints(
    entries: &[EntrypointDeclaration],
    strings: &StringTable,
    public_ids: &PublicIdTable,
) -> Result<Vec<u8>, SectionCodecError> {
    let mut out = Vec::new();
    write_u32(&mut out, u32_saturating(entries.len()));
    for entry in entries {
        write_u32(&mut out, required_public_ref(public_ids, &entry.public_id)?);
        write_u32(
            &mut out,
            optional_string_ref(strings, entry.exported_name.as_deref())?,
        );
        write_u32(&mut out, entry.awbc_function_index.unwrap_or(NONE_REF));
        write_u32(&mut out, entry.visibility.encoded());
        if let Some(anchor) = &entry.source_anchor {
            write_u32(
                &mut out,
                optional_string_ref(strings, Some(&anchor.source_public_id))?,
            );
            write_u32(&mut out, anchor.start_byte);
            write_u32(&mut out, anchor.end_byte);
        } else {
            write_u32(&mut out, NONE_REF);
            write_u32(&mut out, 0);
            write_u32(&mut out, 0);
        }
    }
    Ok(out)
}

fn decode_entrypoints(
    bytes: &[u8],
    strings: &StringTable,
    public_ids: &PublicIdTable,
    budget: RuntimeResourceBudget,
) -> Result<Vec<EntrypointDeclaration>, SectionCodecError> {
    let mut reader = PayloadReader::new(bytes);
    let count = reader.read_u32()? as usize;
    check_budget(count, budget.entrypoints, "entrypoints")?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let public_id = public_ids.get(PublicIdRef(reader.read_u32()?))?.to_owned();
        let exported_name = string_ref_value(strings, reader.read_u32()?)?;
        let function = match reader.read_u32()? {
            NONE_REF => None,
            index => Some(index),
        };
        let visibility = ProductVisibility::from_encoded(reader.read_u32()?)
            .ok_or(SectionCodecError::NonCanonicalTable("entry_visibility"))?;
        let source = string_ref_value(strings, reader.read_u32()?)?;
        let start_byte = reader.read_u32()?;
        let end_byte = reader.read_u32()?;
        let source_anchor = source.map(|source_public_id| EntrypointSourceAnchor {
            source_public_id,
            start_byte,
            end_byte,
        });
        entries.push(EntrypointDeclaration {
            public_id,
            exported_name,
            awbc_function_index: function,
            source_anchor,
            visibility,
        });
    }
    reader.finish()?;
    Ok(entries)
}

fn encode_adapter_requirements(
    section: &AdapterRequirementsSection,
    strings: &StringTable,
    public_ids: &PublicIdTable,
) -> Result<Vec<u8>, SectionCodecError> {
    let mut out = Vec::new();
    write_u32(
        &mut out,
        optional_string_ref(strings, section.default_adapter.as_deref())?,
    );
    write_public_id_list(&mut out, public_ids, &section.adapter_manifest_ids)?;
    write_public_id_list(&mut out, public_ids, &section.required_host_calls)?;
    write_u32(&mut out, u32_saturating(section.adapter_manifests.len()));
    for manifest in &section.adapter_manifests {
        write_u32(&mut out, required_public_ref(public_ids, &manifest.id)?);
        write_u32(
            &mut out,
            required_string_ref(strings, &manifest.display_name)?,
        );
        write_public_id_list(&mut out, public_ids, &manifest.effects)?;
        write_u32(&mut out, u32_saturating(manifest.host_calls.len()));
        for host_call in &manifest.host_calls {
            write_u32(&mut out, required_public_ref(public_ids, &host_call.id)?);
            write_public_id_list(&mut out, public_ids, &host_call.effects)?;
        }
    }
    write_capability_list(
        &mut out,
        strings,
        public_ids,
        &section.required_capabilities,
    )?;
    write_capability_list(
        &mut out,
        strings,
        public_ids,
        &section.optional_capabilities,
    )?;
    write_string_list(&mut out, strings, &section.feature_flags)?;
    write_u32(&mut out, u32_saturating(section.launch_constraints.len()));
    for constraint in &section.launch_constraints {
        write_u32(
            &mut out,
            required_public_ref(public_ids, &constraint.public_id)?,
        );
        write_u32(&mut out, u32::from(constraint.required));
    }
    write_u32(&mut out, u32_saturating(section.platform_refs.len()));
    for reference in &section.platform_refs {
        write_u32(&mut out, required_string_ref(strings, &reference.platform)?);
        write_u32(
            &mut out,
            required_public_ref(public_ids, &reference.requirement)?,
        );
    }
    Ok(out)
}

fn decode_adapter_requirements(
    bytes: &[u8],
    strings: &StringTable,
    public_ids: &PublicIdTable,
    budget: RuntimeResourceBudget,
) -> Result<AdapterRequirementsSection, SectionCodecError> {
    let mut reader = PayloadReader::new(bytes);
    let default_adapter = string_ref_value(strings, reader.read_u32()?)?;
    let adapter_manifest_ids =
        read_public_id_list(&mut reader, public_ids, budget.adapter_requirements)?;
    let required_host_calls =
        read_public_id_list(&mut reader, public_ids, budget.adapter_requirements)?;
    let manifest_count = reader.read_u32()? as usize;
    check_budget(
        manifest_count,
        budget.adapter_manifests,
        "adapter_manifests",
    )?;
    let mut adapter_manifests = Vec::with_capacity(manifest_count);
    for _ in 0..manifest_count {
        let id = public_ids.get(PublicIdRef(reader.read_u32()?))?.to_owned();
        let display_name = strings.get(StringId(reader.read_u32()?))?.to_owned();
        let effects = read_public_id_list(&mut reader, public_ids, budget.adapter_requirements)?;
        let host_call_count = reader.read_u32()? as usize;
        check_budget(host_call_count, budget.host_calls, "host_calls")?;
        let mut host_calls = Vec::with_capacity(host_call_count);
        for _ in 0..host_call_count {
            let id = public_ids.get(PublicIdRef(reader.read_u32()?))?.to_owned();
            let effects =
                read_public_id_list(&mut reader, public_ids, budget.adapter_requirements)?;
            host_calls.push(BundleAdapterHostCall { id, effects });
        }
        adapter_manifests.push(BundleAdapterManifest {
            id,
            display_name,
            effects,
            host_calls,
        });
    }
    let required_capabilities = read_capability_list(&mut reader, strings, public_ids, budget)?;
    let optional_capabilities = read_capability_list(&mut reader, strings, public_ids, budget)?;
    let feature_flags = read_string_list(&mut reader, strings, budget.adapter_requirements)?;
    let launch_count = reader.read_u32()? as usize;
    check_budget(
        launch_count,
        budget.adapter_requirements,
        "launch_constraints",
    )?;
    let mut launch_constraints = Vec::with_capacity(launch_count);
    for _ in 0..launch_count {
        let public_id = public_ids.get(PublicIdRef(reader.read_u32()?))?.to_owned();
        let required = match reader.read_u32()? {
            0 => false,
            1 => true,
            _ => return Err(SectionCodecError::NonCanonicalTable("launch_required")),
        };
        launch_constraints.push(LaunchConstraint {
            public_id,
            required,
        });
    }
    let platform_count = reader.read_u32()? as usize;
    check_budget(
        platform_count,
        budget.adapter_requirements,
        "platform_requirements",
    )?;
    let mut platform_refs = Vec::with_capacity(platform_count);
    for _ in 0..platform_count {
        let platform = strings.get(StringId(reader.read_u32()?))?.to_owned();
        let requirement = public_ids.get(PublicIdRef(reader.read_u32()?))?.to_owned();
        platform_refs.push(PlatformRequirementRef {
            platform,
            requirement,
        });
    }
    reader.finish()?;
    let mut section = AdapterRequirementsSection {
        default_adapter,
        adapter_manifest_ids,
        required_host_calls,
        adapter_manifests,
        required_capabilities,
        optional_capabilities,
        feature_flags,
        launch_constraints,
        platform_refs,
    };
    canonicalize_adapter_requirements(&mut section);
    Ok(section)
}

fn capability_string_values(
    capabilities: &[CapabilityRequirement],
) -> impl Iterator<Item = String> + '_ {
    capabilities.iter().flat_map(|capability| {
        capability
            .version
            .min
            .iter()
            .chain(capability.version.max.iter())
            .cloned()
            .chain(capability.feature_flags.iter().cloned())
    })
}

fn write_capability_list(
    out: &mut Vec<u8>,
    strings: &StringTable,
    public_ids: &PublicIdTable,
    capabilities: &[CapabilityRequirement],
) -> Result<(), SectionCodecError> {
    write_u32(out, u32_saturating(capabilities.len()));
    for capability in capabilities {
        write_u32(out, required_public_ref(public_ids, &capability.public_id)?);
        write_u32(
            out,
            optional_string_ref(strings, capability.version.min.as_deref())?,
        );
        write_u32(
            out,
            optional_string_ref(strings, capability.version.max.as_deref())?,
        );
        write_string_list(out, strings, &capability.feature_flags)?;
    }
    Ok(())
}

fn read_capability_list(
    reader: &mut PayloadReader<'_>,
    strings: &StringTable,
    public_ids: &PublicIdTable,
    budget: RuntimeResourceBudget,
) -> Result<Vec<CapabilityRequirement>, SectionCodecError> {
    let count = reader.read_u32()? as usize;
    check_budget(count, budget.adapter_requirements, "capabilities")?;
    (0..count)
        .map(|_| {
            let public_id = public_ids.get(PublicIdRef(reader.read_u32()?))?.to_owned();
            let min = string_ref_value(strings, reader.read_u32()?)?;
            let max = string_ref_value(strings, reader.read_u32()?)?;
            let feature_flags = read_string_list(reader, strings, budget.adapter_requirements)?;
            Ok(CapabilityRequirement {
                public_id,
                version: VersionRange { min, max },
                feature_flags,
            })
        })
        .collect()
}

fn write_string_list(
    out: &mut Vec<u8>,
    strings: &StringTable,
    values: &[String],
) -> Result<(), SectionCodecError> {
    write_u32(out, u32_saturating(values.len()));
    for value in values {
        write_u32(out, required_string_ref(strings, value)?);
    }
    Ok(())
}

fn read_string_list(
    reader: &mut PayloadReader<'_>,
    strings: &StringTable,
    max: usize,
) -> Result<Vec<String>, SectionCodecError> {
    let count = reader.read_u32()? as usize;
    check_budget(count, max, "string_list")?;
    (0..count)
        .map(|_| strings.get(StringId(reader.read_u32()?)).map(str::to_owned))
        .collect()
}

fn canonicalize_adapter_requirements(section: &mut AdapterRequirementsSection) {
    section.adapter_manifest_ids.sort();
    section.adapter_manifest_ids.dedup();
    section.required_host_calls.sort();
    section.required_host_calls.dedup();
    section
        .adapter_manifests
        .sort_by(|left, right| left.id.cmp(&right.id));
    section
        .required_capabilities
        .sort_by(|left, right| left.public_id.cmp(&right.public_id));
    section
        .required_capabilities
        .dedup_by(|left, right| left.public_id == right.public_id);
    section
        .optional_capabilities
        .sort_by(|left, right| left.public_id.cmp(&right.public_id));
    section
        .optional_capabilities
        .dedup_by(|left, right| left.public_id == right.public_id);
    section.feature_flags.sort();
    section.feature_flags.dedup();
    section
        .launch_constraints
        .sort_by(|left, right| left.public_id.cmp(&right.public_id));
    section
        .launch_constraints
        .dedup_by(|left, right| left.public_id == right.public_id);
    section.platform_refs.sort_by(|left, right| {
        left.platform
            .cmp(&right.platform)
            .then_with(|| left.requirement.cmp(&right.requirement))
    });
    section.platform_refs.dedup_by(|left, right| {
        left.platform == right.platform && left.requirement == right.requirement
    });
}

fn write_public_id_list(
    out: &mut Vec<u8>,
    public_ids: &PublicIdTable,
    values: &[String],
) -> Result<(), SectionCodecError> {
    write_u32(out, u32_saturating(values.len()));
    for value in values {
        write_u32(out, required_public_ref(public_ids, value)?);
    }
    Ok(())
}

fn read_public_id_list(
    reader: &mut PayloadReader<'_>,
    public_ids: &PublicIdTable,
    max: usize,
) -> Result<Vec<String>, SectionCodecError> {
    let count = reader.read_u32()? as usize;
    check_budget(count, max, "public_id_list")?;
    (0..count)
        .map(|_| {
            public_ids
                .get(PublicIdRef(reader.read_u32()?))
                .map(str::to_owned)
        })
        .collect()
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn u16_saturating(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

struct PayloadReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], SectionCodecError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(SectionCodecError::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(SectionCodecError::Truncated)?;
        self.offset = end;
        bytes.try_into().map_err(|_| SectionCodecError::Truncated)
    }

    fn read_u32(&mut self) -> Result<u32, SectionCodecError> {
        self.read_array::<4>().map(u32::from_le_bytes)
    }

    fn finish(self) -> Result<(), SectionCodecError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(SectionCodecError::TrailingBytes)
        }
    }
}

/// Maps a migrated runtime resource AWFB section kind to its compact codec.
pub const fn runtime_codec_for_section(kind: BundleSectionKind) -> Option<ProductSectionCodecKind> {
    match kind {
        BundleSectionKind::RuntimeTypes => Some(ProductSectionCodecKind::RuntimeTypes),
        BundleSectionKind::Entrypoints => Some(ProductSectionCodecKind::Entrypoints),
        BundleSectionKind::AdapterRequirements => {
            Some(ProductSectionCodecKind::AdapterRequirements)
        }
        _ => None,
    }
}

/// Computes semantic compatibility for two decoded migrated runtime sections.
pub fn migrated_runtime_section_compatibility(
    kind: BundleSectionKind,
    old: &[u8],
    new: &[u8],
) -> Result<Option<RuntimeResourceCompatibility>, SectionCodecError> {
    match runtime_codec_for_section(kind) {
        Some(ProductSectionCodecKind::RuntimeTypes) => {
            let old = RuntimeTypesSection::decode_canonical_section(old)?;
            let new = RuntimeTypesSection::decode_canonical_section(new)?;
            Ok(Some(old.compatibility_with(&new)))
        }
        Some(ProductSectionCodecKind::Entrypoints) => {
            let old = EntrypointsSection::decode_canonical_section(old)?;
            let new = EntrypointsSection::decode_canonical_section(new)?;
            Ok(Some(old.compatibility_with(&new)))
        }
        Some(ProductSectionCodecKind::AdapterRequirements) => {
            let old = AdapterRequirementsSection::decode_canonical_section(old)?;
            let new = AdapterRequirementsSection::decode_canonical_section(new)?;
            Ok(Some(old.compatibility_with(&new)))
        }
        _ => Ok(None),
    }
}

fn unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod opaque_runtime_type_tests {
    use super::*;
    use arcweft_core::awbc::schema::{
        AwbcAgentTypeShape, AwbcEffectSet, AwbcEffectSetId, AwbcFrameSlot, AwbcStringId, AwbcTypeId,
    };
    use arcweft_core::pattern::RuntimeOpaqueTypeAdmission;
    use arcweft_core::plan::RuntimeAgentOperationalType;
    use arcweft_core::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

    #[test]
    fn opaque_awbc_type_projects_to_final_bundle_value_family() {
        let mut program = AwbcProgram::default();
        program.strings.push("fixture.bundle".to_owned());
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([81; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(0),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: RuntimeOpaqueValueClass::Plain,
                persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: vec![],
            },
        ));
        let declaration =
            runtime_type_declaration(&program, &program.runtime_types[AwbcTypeId(2).index()])
                .expect("opaque runtime type declaration projects");

        assert_eq!(declaration.public_id, None);
        assert_eq!(declaration.value_kind, RuntimeValueKind::Opaque);
        assert_eq!(
            declaration.compatibility,
            TypeCompatibilityLabel::RestartRequired
        );
        assert_eq!(RuntimeValueKind::Opaque.encoded(), 120);
        assert_eq!(
            RuntimeValueKind::from_encoded(120),
            Some(RuntimeValueKind::Opaque)
        );
    }

    #[test]
    fn runtime_layout_and_function_abi_digests_ignore_generation_local_indices() {
        let semantic_identity = RuntimeSemanticTypeId::from_bytes([83; 32]);
        let mut left = AwbcProgram::default();
        left.strings.extend([
            "capture".to_owned(),
            "effect.read".to_owned(),
            "std.fixture".to_owned(),
        ]);
        left.effect_sets[0] = AwbcEffectSet {
            effects: vec![AwbcStringId(1)],
        };
        left.runtime_types.push(AwbcRuntimeType::new(
            semantic_identity,
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(2),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: RuntimeOpaqueValueClass::Plain,
                persistence: RuntimeOpaquePersistence::SnapshotOnly,
                arguments: vec![AwbcTypeId(0)],
            },
        ));

        let mut right = AwbcProgram::default();
        right.runtime_types.swap(0, 1);
        right.strings.extend([
            "effect.read".to_owned(),
            "std.fixture".to_owned(),
            "capture".to_owned(),
            "generation-only".to_owned(),
        ]);
        right.effect_sets[0] = AwbcEffectSet {
            effects: vec![AwbcStringId(0)],
        };
        right.runtime_types.push(AwbcRuntimeType::new(
            semantic_identity,
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(1),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: RuntimeOpaqueValueClass::Plain,
                persistence: RuntimeOpaquePersistence::SnapshotOnly,
                arguments: vec![AwbcTypeId(1)],
            },
        ));

        assert_eq!(
            runtime_type_layout_digest(&left, &left.runtime_types[2]),
            runtime_type_layout_digest(&right, &right.runtime_types[2])
        );

        let left_signature = AwbcSignature {
            params: vec![AwbcTypeId(2)],
            result: Some(AwbcTypeId(0)),
            effects: AwbcEffectSetId(0),
        };
        let right_signature = AwbcSignature {
            params: vec![AwbcTypeId(2)],
            result: Some(AwbcTypeId(1)),
            effects: AwbcEffectSetId(0),
        };
        assert_eq!(
            runtime_signature_digest(&left, &left_signature),
            runtime_signature_digest(&right, &right_signature)
        );

        let left_frame = AwbcFrameLayout {
            slots: vec![AwbcFrameSlot {
                name: Some(AwbcStringId(0)),
                ty: AwbcTypeId(2),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        };
        let right_frame = AwbcFrameLayout {
            slots: vec![AwbcFrameSlot {
                name: Some(AwbcStringId(2)),
                ty: AwbcTypeId(2),
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            }],
            max_scope_depth: 0,
        };
        assert_eq!(
            runtime_frame_layout_digest(&left, &left_frame),
            runtime_frame_layout_digest(&right, &right_frame)
        );
    }

    #[test]
    fn canonical_runtime_digest_rejects_a_missing_table_reference() {
        let mut program = AwbcProgram::default();
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([84; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(u32::MAX),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: RuntimeOpaqueValueClass::Plain,
                persistence: RuntimeOpaquePersistence::SnapshotOnly,
                arguments: Vec::new(),
            },
        ));
        assert_eq!(
            runtime_type_layout_digest(&program, &program.runtime_types[2]),
            Err(SectionCodecError::BudgetExceeded("awbc_string_reference"))
        );
    }

    #[test]
    fn agent_awbc_type_projects_to_non_colliding_bundle_value_family() {
        let mut program = AwbcProgram::default();
        program.runtime_types.push(AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([82; 32]),
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(
                RuntimeAgentOperationalType::Predicate,
            )),
        ));
        let declaration =
            runtime_type_declaration(&program, &program.runtime_types[AwbcTypeId(2).index()])
                .expect("Agent runtime type declaration projects");

        assert_eq!(declaration.value_kind, RuntimeValueKind::Agent);
        assert_eq!(
            declaration.compatibility,
            TypeCompatibilityLabel::RestartRequired
        );
        assert_eq!(RuntimeValueKind::Agent.encoded(), 121);
        assert_eq!(
            RuntimeValueKind::from_encoded(121),
            Some(RuntimeValueKind::Agent)
        );
        assert_eq!(RuntimeValueKind::Progress.encoded(), 122);
        assert_eq!(
            RuntimeValueKind::from_encoded(122),
            Some(RuntimeValueKind::Progress)
        );
        let closed_kinds = [
            (RuntimeValueKind::Range, 123),
            (RuntimeValueKind::Iterator, 124),
            (RuntimeValueKind::Array, 125),
            (RuntimeValueKind::Map, 126),
            (RuntimeValueKind::Stream, 127),
            (RuntimeValueKind::Shared, 128),
            (RuntimeValueKind::Reference, 129),
            (RuntimeValueKind::Function, 130),
        ];
        for (kind, encoded) in closed_kinds {
            assert_eq!(kind.encoded(), encoded);
            assert_eq!(RuntimeValueKind::from_encoded(encoded), Some(kind));
        }
        assert_eq!(RuntimeValueKind::from_encoded(131), None);
    }
}
