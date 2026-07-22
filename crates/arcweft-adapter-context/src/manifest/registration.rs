//! Deterministic source-backed publication of typed adapter symbols.

use std::{fmt::Write as _, sync::Arc};

use arcweft_lang_hir::symbol::{
    ExternalDeclarationSeed, ExternalDeclarationSeedError, ProjectDirectBinding,
    ProjectDirectBindingError,
};
use arcweft_lang_sema::{
    env::identity::{EnvironmentBindingId, EnvironmentBindingIdError},
    registration::{ExternalRegistrationFact, RegisteredExternalOwner},
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
    SourceRange, SourceSpanError,
};
use thiserror::Error;

use super::{AdapterManifest, AdapterSymbolPath};

/// One adapter's deterministic generated source and typed external contributions.
#[derive(Clone, Debug)]
pub struct SourceBackedAdapterRegistrationFacts {
    document: Arc<SourceDocument>,
    externals: Vec<ExternalRegistrationFact>,
}

struct AdapterRegistrationSymbol {
    path: AdapterSymbolPath,
    spelling: String,
    range: SourceRange,
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
}

impl AdapterManifest {
    fn deterministic_registration_source(&self) -> (String, Vec<AdapterRegistrationSymbol>) {
        let mut source = String::new();
        writeln!(&mut source, "adapter-manifest-v1")
            .expect("writing adapter facts to a String cannot fail");
        writeln!(&mut source, "id {:#?}", self.id)
            .expect("writing adapter facts to a String cannot fail");
        writeln!(&mut source, "display-name {:#?}", self.display_name)
            .expect("writing adapter facts to a String cannot fail");
        let mut manifest_facts = Vec::new();
        manifest_facts.extend(
            self.symbols
                .iter()
                .map(|value| format!("symbol-fact {value:#?}")),
        );
        manifest_facts.extend(
            self.methods
                .iter()
                .map(|value| format!("method-fact {value:#?}")),
        );
        manifest_facts.extend(
            self.functions
                .iter()
                .map(|value| format!("function-fact {value:#?}")),
        );
        manifest_facts.extend(
            self.effects
                .iter()
                .map(|value| format!("effect-fact {value:#?}")),
        );
        manifest_facts.extend(
            self.host_calls
                .iter()
                .map(|value| format!("host-call-fact {value:#?}")),
        );
        manifest_facts.extend(
            self.rust_functions
                .iter()
                .map(|value| format!("rust-function-fact {value:#?}")),
        );
        manifest_facts.extend(
            self.rust_types
                .iter()
                .map(|value| format!("rust-type-fact {value:#?}")),
        );
        manifest_facts.extend(
            self.tooling_docs
                .iter()
                .map(|value| format!("tooling-doc-fact {value:#?}")),
        );
        manifest_facts.sort();
        for fact in manifest_facts {
            writeln!(&mut source, "{fact}").expect("writing adapter facts to a String cannot fail");
        }
        let mut symbols = self.symbols.iter().collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            left.path()
                .cmp(right.path())
                .then_with(|| format!("{:?}", left.ty()).cmp(&format!("{:?}", right.ty())))
        });
        let mut ranges = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            source.push_str("symbol ");
            let start = source.len();
            let spelling = symbol.path().to_string();
            source.push_str(&spelling);
            let end = source.len();
            source.push('\n');
            ranges.push(AdapterRegistrationSymbol {
                path: symbol.path().clone(),
                spelling,
                range: SourceRange::new(start, end),
            });
        }
        (source, ranges)
    }

    /// Binds every registration-visible base fact to one deterministic generated document.
    pub fn source_backed_registration_facts(
        &self,
        ordinal: u64,
    ) -> Result<SourceBackedAdapterRegistrationFacts, AdapterRegistrationFactsError> {
        let (source, symbols) = self.deterministic_registration_source();
        let document = Arc::new(SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-generated://adapter-context/{ordinal}"))?,
            SourceName::Generated,
            source,
        )?);
        let mut externals = Vec::with_capacity(symbols.len());
        for symbol in symbols {
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
                RegisteredExternalOwner::Environment(EnvironmentBindingId::try_new(
                    symbol.spelling,
                )?),
                declaration,
            ));
        }
        Ok(SourceBackedAdapterRegistrationFacts {
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

    pub fn into_parts(self) -> (Arc<SourceDocument>, Vec<ExternalRegistrationFact>) {
        (self.document, self.externals)
    }
}
