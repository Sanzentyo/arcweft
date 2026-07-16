//! Fallible one-way normalization from accepted adapter manifests to sema.

use std::sync::Arc;

use crate::{
    callable::{
        AdapterCallableModelError, AdapterCallableName, AdapterCallablePath,
        AdapterFreeCallableKind, AdapterFunctionSignature, AdapterParameterPassing,
        AdapterParameterPresence, AdapterToolingSubject,
    },
    manifest::{AdapterEffectCapability, AdapterManifest, AdapterRustFunction},
    standard::{
        INFERENCE_TENSOR_ADAPTER_ID, MATH_ADAPTER_ID, NATIVE_FILE_ADAPTER_ID,
        NATIVE_HTTP_ADAPTER_ID, SANS_IO_ADAPTER_ID, SYSTEM_INFO_ADAPTER_ID,
    },
};
use arcweft_lang_sema::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableDocumentation, CallableEffectSchema,
        CallableGroupIndex, CallableGroupKind, CallableLimits, CallableLookupKey, CallableName,
        CallableOverloadIndex, CallableParameter, CallableParameterDocumentation,
        CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallableParameterType, CallablePath, CallablePublicationError,
        CallableSignatureSchema, CallableValidator, DocumentationProvenance,
        EnvironmentCallableKind, EnvironmentCallableOwner, EnvironmentCallablePublication,
        EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal, ReceiverMethodKey,
        RustCallableProvenance, RustCallablePurity, RustItemPath, RustPackageProvenance,
        RustProvenanceError, SpreadArgumentPolicy, StandardEnvironmentId,
        UnknownNamedArgumentPolicy,
    },
    effect_row::EffectRow,
    effects::EffectSet,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterManifestSource {
    Standard(StandardEnvironmentId),
    SelectedAdapter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterCallablePublicationError {
    InvalidPackageId(arcweft_lang_sema::callable::CallableScalarError),
    InvalidModel(AdapterCallableModelError),
    StandardIdMismatch {
        source: StandardEnvironmentId,
        actual: AdapterPackageId,
    },
    ReservedStandardIdClaimed {
        actual: AdapterPackageId,
    },
    DuplicateToolingSubject {
        subject: AdapterToolingSubject,
    },
    MissingToolingTarget {
        subject: AdapterToolingSubject,
    },
    InvalidReceiverType,
    InvalidSignature(CallablePublicationError),
    InvalidRustProvenance(RustProvenanceError),
    RustMetadataOwnerMismatch {
        package: AdapterPackageId,
    },
}

impl std::fmt::Display for AdapterCallablePublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "adapter callable publication failed: {self:?}")
    }
}

impl std::error::Error for AdapterCallablePublicationError {}

impl From<AdapterCallableModelError> for AdapterCallablePublicationError {
    fn from(value: AdapterCallableModelError) -> Self {
        Self::InvalidModel(value)
    }
}

impl From<CallablePublicationError> for AdapterCallablePublicationError {
    fn from(value: CallablePublicationError) -> Self {
        Self::InvalidSignature(value)
    }
}

impl From<RustProvenanceError> for AdapterCallablePublicationError {
    fn from(value: RustProvenanceError) -> Self {
        Self::InvalidRustProvenance(value)
    }
}

impl AdapterManifest {
    /// Publishes every callable owned by this accepted manifest.
    pub fn try_callable_publication(
        &self,
        source: AdapterManifestSource,
        limits: &CallableLimits,
    ) -> Result<EnvironmentCallablePublication, AdapterCallablePublicationError> {
        self.try_callable_publication_scope(source, limits, CallablePublicationScope::All)
    }

    /// Publishes only Rust ABI callables augmenting this accepted manifest.
    ///
    /// Standard manifests use this delta beside their fixed bundled
    /// publication so Rust metadata does not duplicate the standard callable
    /// records already accepted under the same typed owner.
    pub fn try_rust_callable_publication(
        &self,
        source: AdapterManifestSource,
        limits: &CallableLimits,
    ) -> Result<EnvironmentCallablePublication, AdapterCallablePublicationError> {
        self.try_callable_publication_scope(source, limits, CallablePublicationScope::RustOnly)
    }

    fn try_callable_publication_scope(
        &self,
        source: AdapterManifestSource,
        limits: &CallableLimits,
        scope: CallablePublicationScope,
    ) -> Result<EnvironmentCallablePublication, AdapterCallablePublicationError> {
        let package = AdapterPackageId::try_new(self.id().as_str())
            .map_err(AdapterCallablePublicationError::InvalidPackageId)?;
        let owner = publication_owner(source, &package)?;
        validate_tooling(self)?;

        let mut records = Vec::new();
        if scope == CallablePublicationScope::All {
            for function in self.functions() {
                let subject = AdapterToolingSubject::Free {
                    kind: AdapterFreeCallableKind::Function,
                    path: function.path().clone(),
                    overload: function.overload(),
                };
                records.push(publication_record(
                    EnvironmentCallableKind::Function,
                    CallableLookupKey::Free(callable_path(function.path())?),
                    function.overload().get(),
                    function.signature(),
                    function.effects(),
                    documentation(self, &subject, &package, function.signature())?,
                    None,
                    records.len(),
                    limits,
                )?);
            }
            for method in self.methods() {
                let subject = AdapterToolingSubject::Method {
                    receiver: method.receiver().clone(),
                    name: method.callable_name().clone(),
                    overload: method.overload(),
                };
                let name = callable_name(method.callable_name())?;
                records.push(publication_record(
                    EnvironmentCallableKind::Method,
                    CallableLookupKey::Method(ReceiverMethodKey::new(
                        method.receiver().to_sema_type_kind(),
                        name,
                    )),
                    method.overload().get(),
                    method.signature(),
                    method.effects(),
                    documentation(self, &subject, &package, method.signature())?,
                    None,
                    records.len(),
                    limits,
                )?);
            }
        }
        let rust_declaration_offset = match scope {
            CallablePublicationScope::All => 0,
            CallablePublicationScope::RustOnly => self
                .functions()
                .len()
                .checked_add(self.methods().len())
                .ok_or(CallablePublicationError::InvalidOverload)?,
        };
        for function in self.rust_functions() {
            let subject = AdapterToolingSubject::Free {
                kind: AdapterFreeCallableKind::RustFunction,
                path: function.path().clone(),
                overload: function.overload(),
            };
            let rust = rust_provenance(function, &package)?;
            records.push(publication_record(
                EnvironmentCallableKind::RustFunction,
                CallableLookupKey::Free(callable_path(function.path())?),
                function.overload().get(),
                function.signature(),
                function.effects(),
                documentation(self, &subject, &package, function.signature())?,
                Some(rust),
                rust_declaration_offset
                    .checked_add(records.len())
                    .ok_or(CallablePublicationError::InvalidOverload)?,
                limits,
            )?);
        }
        EnvironmentCallablePublication::try_new(owner, records, limits).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallablePublicationScope {
    All,
    RustOnly,
}

fn publication_owner(
    source: AdapterManifestSource,
    package: &AdapterPackageId,
) -> Result<EnvironmentCallableOwner, AdapterCallablePublicationError> {
    match source {
        AdapterManifestSource::Standard(source) => {
            if standard_adapter_id(source) != Some(package.as_str()) {
                return Err(AdapterCallablePublicationError::StandardIdMismatch {
                    source,
                    actual: package.clone(),
                });
            }
            Ok(EnvironmentCallableOwner::Standard(source))
        }
        AdapterManifestSource::SelectedAdapter => {
            if is_reserved_standard_id(package.as_str()) {
                return Err(AdapterCallablePublicationError::ReservedStandardIdClaimed {
                    actual: package.clone(),
                });
            }
            Ok(EnvironmentCallableOwner::Adapter(package.clone()))
        }
    }
}

const fn standard_adapter_id(id: StandardEnvironmentId) -> Option<&'static str> {
    match id {
        StandardEnvironmentId::Core => None,
        StandardEnvironmentId::SansIo => Some(SANS_IO_ADAPTER_ID),
        StandardEnvironmentId::NativeHttp => Some(NATIVE_HTTP_ADAPTER_ID),
        StandardEnvironmentId::InferenceTensor => Some(INFERENCE_TENSOR_ADAPTER_ID),
        StandardEnvironmentId::SystemInfo => Some(SYSTEM_INFO_ADAPTER_ID),
        StandardEnvironmentId::NativeFile => Some(NATIVE_FILE_ADAPTER_ID),
        StandardEnvironmentId::Math => Some(MATH_ADAPTER_ID),
    }
}

fn is_reserved_standard_id(id: &str) -> bool {
    [
        SANS_IO_ADAPTER_ID,
        NATIVE_HTTP_ADAPTER_ID,
        INFERENCE_TENSOR_ADAPTER_ID,
        SYSTEM_INFO_ADAPTER_ID,
        NATIVE_FILE_ADAPTER_ID,
        MATH_ADAPTER_ID,
    ]
    .contains(&id)
}

#[allow(clippy::too_many_arguments)]
fn publication_record(
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    overload: usize,
    signature: &AdapterFunctionSignature,
    effects: &[AdapterEffectCapability],
    documentation: CallableDocumentation,
    rust: Option<RustCallableProvenance>,
    declaration_order: usize,
    limits: &CallableLimits,
) -> Result<EnvironmentCallablePublicationRecord, AdapterCallablePublicationError> {
    let schema = callable_schema(signature, effects, limits)?;
    EnvironmentCallablePublicationRecord::try_new(
        kind,
        key,
        CallableOverloadIndex::try_from_usize(overload)
            .map_err(|_| CallablePublicationError::InvalidOverload)?,
        schema,
        documentation,
        None,
        rust,
        EnvironmentDeclarationOrdinal::try_from_usize(declaration_order)
            .map_err(|_| CallablePublicationError::InvalidOverload)?,
    )
    .map_err(Into::into)
}

fn callable_schema(
    signature: &AdapterFunctionSignature,
    effects: &[AdapterEffectCapability],
    limits: &CallableLimits,
) -> Result<CallableSignatureSchema, AdapterCallablePublicationError> {
    let mut has_rest = false;
    let groups = signature
        .groups()
        .iter()
        .map(|group| {
            let group_index = CallableGroupIndex::try_from_usize(group.index().get())
                .map_err(|_| CallablePublicationError::InvalidOverload)?;
            let parameters = group
                .parameters()
                .iter()
                .map(|parameter| {
                    let passing = match parameter.passing() {
                        AdapterParameterPassing::PositionalOrNamed => {
                            CallableParameterPassing::PositionalOrNamed
                        }
                        AdapterParameterPassing::PositionalOnly => {
                            CallableParameterPassing::PositionalOnly
                        }
                        AdapterParameterPassing::NamedOnly => CallableParameterPassing::NamedOnly,
                        AdapterParameterPassing::RestPositional => {
                            has_rest = true;
                            CallableParameterPassing::RestPositional
                        }
                        AdapterParameterPassing::RestNamed => {
                            has_rest = true;
                            CallableParameterPassing::RestNamed
                        }
                    };
                    CallableParameter::try_new(
                        CallableParameterIndex::try_from_usize(parameter.index().get())
                            .map_err(|_| CallablePublicationError::InvalidOverload)?,
                        parameter
                            .name()
                            .map(|name| {
                                CallableName::try_new(name.as_str())
                                    .map_err(|_| CallablePublicationError::InvalidOverload)
                            })
                            .transpose()?,
                        CallableParameterType::Exact(parameter.ty().to_sema_type_kind()),
                        passing,
                        match parameter.presence() {
                            AdapterParameterPresence::Required => {
                                CallableParameterPresence::Required
                            }
                            AdapterParameterPresence::Defaulted => {
                                CallableParameterPresence::Defaulted
                            }
                        },
                        None,
                        None,
                    )
                    .map_err(CallablePublicationError::from)
                })
                .collect::<Result<Vec<_>, _>>()?;
            CallableParameterGroup::try_new(
                group_index,
                if group.index().get() == 0 {
                    CallableGroupKind::Initial
                } else {
                    CallableGroupKind::Curried
                },
                parameters,
                limits,
            )
            .map_err(CallablePublicationError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let effects = EffectSet::from_labels(effects.iter().map(AdapterEffectCapability::as_str))
        .map_err(|_| CallablePublicationError::InvalidOverload)?;
    CallableSignatureSchema::try_new(
        groups,
        signature.return_type().to_sema_type_kind(),
        CallableEffectSchema::fixed(EffectRow::closed(effects)),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            if has_rest {
                SpreadArgumentPolicy::TypedRest
            } else {
                SpreadArgumentPolicy::Reject
            },
        ),
        CallableValidator::Ordinary,
        limits,
    )
    .map_err(|error| CallablePublicationError::from(error).into())
}

fn callable_name(
    name: &AdapterCallableName,
) -> Result<CallableName, AdapterCallablePublicationError> {
    CallableName::try_new(name.as_str())
        .map_err(|_| CallablePublicationError::InvalidOverload.into())
}

fn callable_path(
    path: &AdapterCallablePath,
) -> Result<CallablePath, AdapterCallablePublicationError> {
    let segments = path
        .segments()
        .iter()
        .map(callable_name)
        .collect::<Result<Vec<_>, _>>()?;
    CallablePath::try_new(segments).map_err(|_| CallablePublicationError::InvalidOverload.into())
}

fn documentation(
    manifest: &AdapterManifest,
    subject: &AdapterToolingSubject,
    package: &AdapterPackageId,
    _signature: &AdapterFunctionSignature,
) -> Result<CallableDocumentation, AdapterCallablePublicationError> {
    let Some(doc) = manifest
        .tooling_docs()
        .iter()
        .find(|doc| doc.subject() == subject)
    else {
        return Ok(CallableDocumentation::missing());
    };
    let parameters = doc
        .parameters()
        .iter()
        .map(|parameter| {
            CallableParameterDocumentation::try_new(
                CallableGroupIndex::try_from_usize(parameter.group().get())
                    .map_err(|_| CallablePublicationError::InvalidOverload)?,
                CallableParameterIndex::try_from_usize(parameter.parameter().get())
                    .map_err(|_| CallablePublicationError::InvalidOverload)?,
                Arc::<str>::from(parameter.text()),
            )
            .map_err(|_| CallablePublicationError::InvalidOverload)
        })
        .collect::<Result<Vec<_>, _>>()?;
    CallableDocumentation::try_new(
        doc.summary().map(Arc::<str>::from),
        doc.details().map(Arc::<str>::from),
        parameters,
        DocumentationProvenance::AdapterTooling {
            package: package.clone(),
        },
    )
    .map_err(|_| CallablePublicationError::InvalidOverload.into())
}

fn validate_tooling(manifest: &AdapterManifest) -> Result<(), AdapterCallablePublicationError> {
    let mut seen = Vec::<&AdapterToolingSubject>::new();
    for doc in manifest.tooling_docs() {
        if seen.iter().any(|subject| *subject == doc.subject()) {
            return Err(AdapterCallablePublicationError::DuplicateToolingSubject {
                subject: doc.subject().clone(),
            });
        }
        seen.push(doc.subject());
        let Some(signature) = subject_signature(manifest, doc.subject()) else {
            return Err(AdapterCallablePublicationError::MissingToolingTarget {
                subject: doc.subject().clone(),
            });
        };
        for parameter in doc.parameters() {
            let Some(group) = signature.groups().get(parameter.group().get()) else {
                return Err(AdapterCallablePublicationError::InvalidModel(
                    AdapterCallableModelError::ToolingParameterOutOfBounds {
                        subject: doc.subject().clone(),
                        group: parameter.group().get(),
                        parameter: parameter.parameter().get(),
                    },
                ));
            };
            if group
                .parameters()
                .get(parameter.parameter().get())
                .is_none()
            {
                return Err(AdapterCallablePublicationError::InvalidModel(
                    AdapterCallableModelError::ToolingParameterOutOfBounds {
                        subject: doc.subject().clone(),
                        group: parameter.group().get(),
                        parameter: parameter.parameter().get(),
                    },
                ));
            }
        }
    }
    Ok(())
}

fn subject_signature<'a>(
    manifest: &'a AdapterManifest,
    subject: &AdapterToolingSubject,
) -> Option<&'a AdapterFunctionSignature> {
    match subject {
        AdapterToolingSubject::Free {
            kind: AdapterFreeCallableKind::Function,
            path,
            overload,
        } => manifest
            .functions()
            .iter()
            .find(|function| function.path() == path && function.overload() == *overload)
            .map(super::manifest::AdapterFunction::signature),
        AdapterToolingSubject::Free {
            kind: AdapterFreeCallableKind::RustFunction,
            path,
            overload,
        } => manifest
            .rust_functions()
            .iter()
            .find(|function| function.path() == path && function.overload() == *overload)
            .map(AdapterRustFunction::signature),
        AdapterToolingSubject::Method {
            receiver,
            name,
            overload,
        } => manifest
            .methods()
            .iter()
            .find(|method| {
                method.receiver() == receiver
                    && method.callable_name() == name
                    && method.overload() == *overload
            })
            .map(super::manifest::AdapterMethod::signature),
    }
}

fn rust_provenance(
    function: &AdapterRustFunction,
    adapter: &AdapterPackageId,
) -> Result<RustCallableProvenance, AdapterCallablePublicationError> {
    let package = function.package();
    let package = RustPackageProvenance::try_new(
        Arc::<str>::from(package.name.as_str()),
        Arc::<str>::from(package.version.as_str()),
        package.metadata_hash.as_deref().map(Arc::<str>::from),
    )?;
    let path = RustItemPath::try_new(function.rust_path())
        .map_err(|_| CallablePublicationError::InvalidOverload)?;
    RustCallableProvenance::try_new(
        adapter.clone(),
        package,
        path,
        match function.purity() {
            arcweft_rust_abi::ArcweftRustPurity::External => RustCallablePurity::External,
            arcweft_rust_abi::ArcweftRustPurity::Pure => RustCallablePurity::Pure,
            arcweft_rust_abi::ArcweftRustPurity::Task => RustCallablePurity::Task,
        },
    )
    .map_err(Into::into)
}
