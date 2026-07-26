//! Deterministic source-backed publication of typed adapter symbols.

mod input;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use arcweft_adapter_context::manifest::{
    AdapterCallableModelError, AdapterEffectCapability, AdapterEnvironmentOwnerId, AdapterManifest,
    AdapterManifestModelError, AdapterNominalPathError,
};
use arcweft_lang_hir::symbol::{
    CallablePackageIdError, ExternalDeclarationSeed, ExternalDeclarationSeedError,
    ProjectDirectBinding, ProjectDirectBindingError,
};
use arcweft_lang_sema::{
    callable::{
        AdapterPackageId, CallableDocumentationError, CallablePathError, CallableScalarError,
        EnvironmentCallableOwner, RustProvenanceError, StandardEnvironmentId,
    },
    effects::EffectSetParseError,
    env::{
        identity::{EnvironmentBindingId, EnvironmentBindingIdError},
        nominal::RustPackageIdError,
    },
    registration::{
        ExternalRegistrationFact, RegisteredEnvironmentExternalOwner, RegisteredExternalOwner,
        SourceBackedEnvironmentRegistrationInput,
    },
};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{
        ProjectSymbolPath, ProjectSymbolPathError, ProjectSymbolSegment, SymbolPath,
        SymbolPathError,
    },
};
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceName,
    SourceSpanError,
};
use thiserror::Error;

use arcweft_lang_sema::env::TypeCheckEnv;

/// Semantic projection context for one language-free adapter manifest.
#[derive(Clone, Copy, Debug)]
pub struct AdapterSemanticRegistration<'a> {
    manifest: &'a AdapterManifest,
}

/// One adapter's deterministic generated source and typed external contributions.
#[derive(Clone, Debug)]
pub struct SourceBackedAdapterRegistrationFacts {
    document: Arc<SourceDocument>,
    externals: Vec<ExternalRegistrationFact>,
    environment: SourceBackedEnvironmentRegistrationInput,
}

/// Complete source-backed registration contribution from one adapter manifest.
#[derive(Clone, Debug)]
pub struct SourceBackedAdapterRegistrationParts {
    pub document: Arc<SourceDocument>,
    pub externals: Box<[ExternalRegistrationFact]>,
    pub environment: SourceBackedEnvironmentRegistrationInput,
}

/// Failure while binding adapter facts to one generated source revision.
#[derive(Debug, Error)]
pub enum AdapterRegistrationFactsError {
    #[error(transparent)]
    DocumentId(#[from] SourceDocumentIdError),
    #[error(transparent)]
    Document(#[from] SourceDocumentError),
    #[error(transparent)]
    Span(#[from] SourceSpanError),
    #[error(transparent)]
    SymbolPath(#[from] SymbolPathError),
    #[error(transparent)]
    ProjectSymbolPath(#[from] ProjectSymbolPathError),
    #[error(transparent)]
    ProjectDirectBinding(#[from] ProjectDirectBindingError),
    #[error(transparent)]
    ExternalDeclaration(#[from] ExternalDeclarationSeedError),
    #[error(transparent)]
    EnvironmentBinding(#[from] EnvironmentBindingIdError),
    #[error(transparent)]
    CallableIdentity(#[from] CallableScalarError),
    #[error(transparent)]
    CallablePackage(#[from] CallablePackageIdError),
    #[error(transparent)]
    CallablePath(#[from] CallablePathError),
    #[error(transparent)]
    CallableModel(#[from] AdapterCallableModelError),
    #[error(transparent)]
    CallableDocumentation(#[from] CallableDocumentationError),
    #[error(transparent)]
    Effect(#[from] EffectSetParseError),
    #[error(transparent)]
    RustPackage(#[from] RustPackageIdError),
    #[error(transparent)]
    RustProvenance(#[from] RustProvenanceError),
    #[error(transparent)]
    ManifestModel(#[from] AdapterManifestModelError),
    #[error(transparent)]
    NominalPath(#[from] AdapterNominalPathError),
    #[error(
        "adapter type reference claims environment owner `{actual:?}` instead of `{expected:?}`"
    )]
    EnvironmentOwnerMismatch {
        expected: AdapterEnvironmentOwnerId,
        actual: AdapterEnvironmentOwnerId,
    },
    #[error("Rust metadata field index {value} exceeds u16")]
    RustFieldIndexOverflow { value: usize },
    #[error("type source-site index {value} exceeds u16")]
    TypeSiteIndexOverflow { value: usize },
    #[error("duplicate generated source site for item {item:?} at {site:?}")]
    DuplicateTypeSourceSite {
        item: Box<arcweft_lang_sema::registration::EnvironmentPublicationItemId>,
        site: Box<arcweft_lang_sema::registration::EnvironmentTypeSite>,
    },
    #[error("missing generated source site for item {item:?} at {site:?}")]
    MissingTypeSourceSite {
        item: Box<arcweft_lang_sema::registration::EnvironmentPublicationItemId>,
        site: Box<arcweft_lang_sema::registration::EnvironmentTypeSite>,
    },
    #[error("duplicate generated item source for {item:?}")]
    DuplicateItemSource {
        item: Box<arcweft_lang_sema::registration::EnvironmentPublicationItemId>,
    },
    #[error("missing generated item source for {item:?}")]
    MissingItemSource {
        item: Box<arcweft_lang_sema::registration::EnvironmentPublicationItemId>,
    },
}

impl<'a> AdapterSemanticRegistration<'a> {
    /// Selects one manifest as the source of semantic environment facts.
    pub const fn new(manifest: &'a AdapterManifest) -> Self {
        Self { manifest }
    }

    /// Declares this manifest's effect capabilities without target availability.
    pub fn declare_effects(self, env: TypeCheckEnv) -> TypeCheckEnv {
        self.manifest.effects().iter().fold(env, |env, effect| {
            env.with_capability(effect_capability(effect))
        })
    }

    fn grant_effect_availability(self, env: TypeCheckEnv) -> TypeCheckEnv {
        self.manifest.effects().iter().fold(env, |env, effect| {
            env.with_available_effect(effect_capability(effect))
        })
    }

    /// Declares this manifest's effects and marks them as target-provided.
    pub fn declare_target_effects(self, env: TypeCheckEnv) -> TypeCheckEnv {
        self.grant_effect_availability(self.declare_effects(env))
    }

    /// Binds every registration-visible base fact to one deterministic generated document.
    pub fn source_backed_facts(
        self,
        ordinal: u64,
    ) -> Result<SourceBackedAdapterRegistrationFacts, AdapterRegistrationFactsError> {
        let manifest = self.manifest;
        let owner = environment_callable_owner(manifest)?;
        let nominal_owner = EnvironmentBindingId::try_new(
            AdapterEnvironmentOwnerId::for_adapter(manifest.id()).as_str(),
        )?;
        let rendering = input::source::render(manifest, &owner)?;
        let document = Arc::new(SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-generated://adapter-sema/{ordinal}"))?,
            SourceName::Generated,
            rendering.text,
        )?);
        let mut externals = Vec::with_capacity(rendering.symbols.len());
        for symbol in rendering.symbols {
            let declaration = document.span(symbol.range)?;
            let project_path = ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                symbol
                    .path
                    .segments()
                    .iter()
                    .map(|segment| ProjectSymbolSegment::try_new(segment.as_str().to_owned()))
                    .collect::<Result<Vec<_>, ProjectSymbolPathError>>()?,
            )?;
            let canonical_path = SymbolPath::try_new(
                ModulePathRoot::ImplicitCrate,
                Vec::new(),
                symbol.spelling.clone(),
            )?;
            let direct_binding = ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                project_path,
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )?;
            let seed = ExternalDeclarationSeed::try_new(
                canonical_path,
                Some(Visibility::Public),
                declaration.clone(),
                vec![direct_binding],
            )?;
            externals.push(ExternalRegistrationFact::new(
                seed,
                RegisteredExternalOwner::Environment(RegisteredEnvironmentExternalOwner::new(
                    nominal_owner.clone(),
                    EnvironmentBindingId::try_new(symbol.spelling)?,
                )),
                declaration,
            ));
        }
        Ok(SourceBackedAdapterRegistrationFacts {
            environment: input::environment_input(manifest, owner, &document, &rendering.map)?,
            document,
            externals,
        })
    }
}

impl SourceBackedAdapterRegistrationFacts {
    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub fn externals(&self) -> &[ExternalRegistrationFact] {
        &self.externals
    }

    pub const fn environment(&self) -> &SourceBackedEnvironmentRegistrationInput {
        &self.environment
    }

    pub fn into_parts(self) -> SourceBackedAdapterRegistrationParts {
        SourceBackedAdapterRegistrationParts {
            document: self.document,
            externals: self.externals.into_boxed_slice(),
            environment: self.environment,
        }
    }
}

fn environment_callable_owner(
    manifest: &AdapterManifest,
) -> Result<EnvironmentCallableOwner, CallableScalarError> {
    let standard = match manifest.id().as_str() {
        arcweft_adapter_context::standard::SANS_IO_ADAPTER_ID => {
            Some(StandardEnvironmentId::SansIo)
        }
        arcweft_adapter_context::standard::NATIVE_HTTP_ADAPTER_ID => {
            Some(StandardEnvironmentId::NativeHttp)
        }
        arcweft_adapter_context::standard::INFERENCE_TENSOR_ADAPTER_ID => {
            Some(StandardEnvironmentId::InferenceTensor)
        }
        arcweft_adapter_context::standard::SYSTEM_INFO_ADAPTER_ID => {
            Some(StandardEnvironmentId::SystemInfo)
        }
        arcweft_adapter_context::standard::NATIVE_FILE_ADAPTER_ID => {
            Some(StandardEnvironmentId::NativeFile)
        }
        arcweft_adapter_context::standard::MATH_ADAPTER_ID => Some(StandardEnvironmentId::Math),
        _ => None,
    };
    standard.map_or_else(
        || AdapterPackageId::try_new(manifest.id().as_str()).map(EnvironmentCallableOwner::Adapter),
        |owner| Ok(EnvironmentCallableOwner::Standard(owner)),
    )
}

fn effect_capability(effect: &AdapterEffectCapability) -> arcweft_lang_sema::env::EffectCapability {
    arcweft_lang_sema::env::EffectCapability::new(effect.as_str())
}
