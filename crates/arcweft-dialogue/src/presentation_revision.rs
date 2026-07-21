//! Revision stamp for one compiler-admitted dialogue presentation profile.

use arcweft_resource_model::registry::ResourceTypeRegistryDigest;
use arcweft_source::{SourceDocumentIdentity, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

/// Exact immutable inputs that were jointly admitted for dialogue presentation.
///
/// The compiler is the admission owner. This lower-layer value exists so the
/// runtime plan, save/reload, and tooling can compare the same six typed facts
/// without depending on the compiler crate or reconstructing a digest from
/// strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueProfileRevision {
    manifest_document: SourceDocumentIdentity,
    topology_sources: SourceSetRevision,
    compiled_sources: SourceSetRevision,
    view_program_id: ViewProgramId,
    view_program_revision: AcceptedViewProgramRevision,
    resource_types: ResourceTypeRegistryDigest,
}

impl DialogueProfileRevision {
    /// Constructs the stamp after the compiler has checked all cross-product
    /// admission invariants.
    #[allow(clippy::too_many_arguments)]
    pub const fn from_admitted_parts(
        manifest_document: SourceDocumentIdentity,
        topology_sources: SourceSetRevision,
        compiled_sources: SourceSetRevision,
        view_program_id: ViewProgramId,
        view_program_revision: AcceptedViewProgramRevision,
        resource_types: ResourceTypeRegistryDigest,
    ) -> Self {
        Self {
            manifest_document,
            topology_sources,
            compiled_sources,
            view_program_id,
            view_program_revision,
            resource_types,
        }
    }

    pub const fn manifest_document(&self) -> &SourceDocumentIdentity {
        &self.manifest_document
    }

    pub const fn topology_sources(&self) -> SourceSetRevision {
        self.topology_sources
    }

    pub const fn compiled_sources(&self) -> SourceSetRevision {
        self.compiled_sources
    }

    pub const fn view_program_id(&self) -> &ViewProgramId {
        &self.view_program_id
    }

    pub const fn view_program_revision(&self) -> AcceptedViewProgramRevision {
        self.view_program_revision
    }

    pub const fn resource_types(&self) -> ResourceTypeRegistryDigest {
        self.resource_types
    }
}

#[cfg(test)]
mod tests {
    use super::DialogueProfileRevision;
    use arcweft_resource_model::registry::{
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION, ResourceRegistryPublication, ResourceTypeRegistry,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
    use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

    #[test]
    fn stamp_retains_all_six_typed_admission_facts() {
        let manifest = SourceDocument::try_new(
            SourceDocumentId::try_new("manifest").expect("document id"),
            SourceName::Memory,
            "schema = 1\n",
        )
        .expect("document");
        let topology_sources = SourceSetRevision::try_for_identities([manifest.identity()])
            .expect("topology revision");
        let compiled = SourceDocument::try_new(
            SourceDocumentId::try_new("source-main").expect("document id"),
            SourceName::Memory,
            "flow @flow.main {}\n",
        )
        .expect("document");
        let compiled_sources = SourceSetRevision::try_for_identities([compiled.identity()])
            .expect("compiled revision");
        let program_id = ViewProgramId::try_new("view_program.main").expect("program id");
        let program_revision =
            AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("program revision");
        let registry = ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
            RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
            [],
            [],
            [],
        ))
        .expect("registry");

        let revision = DialogueProfileRevision::from_admitted_parts(
            manifest.identity().clone(),
            topology_sources,
            compiled_sources,
            program_id.clone(),
            program_revision,
            registry.digest(),
        );

        assert_eq!(revision.manifest_document(), manifest.identity());
        assert_eq!(revision.topology_sources(), topology_sources);
        assert_eq!(revision.compiled_sources(), compiled_sources);
        assert_eq!(revision.view_program_id(), &program_id);
        assert_eq!(revision.view_program_revision(), program_revision);
        assert_eq!(revision.resource_types(), registry.digest());
    }
}
