//! Typed test-only mutation of otherwise immutable accepted-project authorities.

use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_source::SourceDocumentIdentity;

use super::{AcceptedProjectSnapshot, AcceptedSourceDocument, AcceptedSourceDocuments};

/// One authority whose impossible intermediate state must remain fail-closed.
pub(crate) enum AcceptedProjectStampMutation {
    ModuleMapping {
        source: SourceDocumentIdentity,
        module: CanonicalModulePath,
    },
}

/// Clones a snapshot while changing exactly one private lookup authority.
pub(crate) fn mutated_project(
    current: &Arc<AcceptedProjectSnapshot>,
    mutation: AcceptedProjectStampMutation,
) -> Arc<AcceptedProjectSnapshot> {
    let sources = AcceptedSourceDocuments {
        world: current.sources.world.clone(),
        symbol_revision: current.sources.symbol_revision,
        character_source_revision: current.sources.character_source_revision,
        by_identity: current
            .sources
            .by_identity
            .iter()
            .map(|(identity, source)| {
                (
                    identity.clone(),
                    AcceptedSourceDocument {
                        document: Arc::clone(&source.document),
                        locator: source.locator.clone(),
                        ownership: source.ownership,
                        access: source.access,
                        line_index: source.line_index.clone(),
                    },
                )
            })
            .collect(),
        by_uri: current.sources.by_uri.clone(),
    };
    let mut module_by_source = current.module_by_source.clone();
    match mutation {
        AcceptedProjectStampMutation::ModuleMapping { source, module } => {
            module_by_source.insert(source, module);
        }
    }
    Arc::new(AcceptedProjectSnapshot {
        tooling: Arc::clone(&current.tooling),
        callable_references: Arc::clone(&current.callable_references),
        entry_references: Arc::clone(&current.entry_references),
        sources,
        module_by_source,
        footprint: current.footprint,
    })
}
