//! Revision stamp for one compiler-admitted dialogue presentation profile.

use arcweft_resource_model::registry::ResourceTypeRegistryDigest;
use arcweft_source::{SourceDocumentIdentity, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};
use serde::{Deserialize, Serialize};

/// Exact immutable inputs that were jointly admitted for dialogue presentation.
///
/// The compiler is the admission owner. This lower-layer value exists so the
/// runtime plan, save/reload, and tooling can compare the same six typed facts
/// without depending on the compiler crate or reconstructing a digest from
/// strings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
    use serde_json::{Value, json};

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
            "flow main {}\n",
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

    #[test]
    fn revision_wire_round_trip_preserves_all_six_typed_facts() {
        let manifest = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://game/arcweft.toml").expect("document id"),
            SourceName::Memory,
            "schema = 1\n",
        )
        .expect("document");
        let compiled = SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://game/src/main.arcw").expect("document id"),
            SourceName::Memory,
            "flow main {}\n",
        )
        .expect("document");
        let topology_sources = SourceSetRevision::try_for_identities([manifest.identity()])
            .expect("topology revision");
        let compiled_sources = SourceSetRevision::try_for_identities([compiled.identity()])
            .expect("compiled revision");
        let view_program_id = ViewProgramId::try_new("view_program.dialogue").expect("program id");
        let view_program_revision =
            AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("program revision");
        let resource_types = ResourceTypeRegistry::empty().digest();
        let revision = DialogueProfileRevision::from_admitted_parts(
            manifest.identity().clone(),
            topology_sources,
            compiled_sources,
            view_program_id.clone(),
            view_program_revision,
            resource_types,
        );

        let encoded = serde_json::to_value(&revision).expect("encode profile revision");
        assert_eq!(
            encoded,
            json!({
                "manifest_document": {
                    "id": manifest.identity().id().as_str(),
                    "revision": manifest.identity().revision().to_hex(),
                    "source_len": manifest.identity().source_len(),
                },
                "topology_sources": topology_sources.to_hex(),
                "compiled_sources": compiled_sources.to_hex(),
                "view_program_id": view_program_id.as_str(),
                "view_program_revision": view_program_revision.to_hex(),
                "resource_types": resource_types.to_string(),
            })
        );
        assert_eq!(
            serde_json::from_value::<DialogueProfileRevision>(encoded)
                .expect("decode profile revision"),
            revision
        );
    }

    #[test]
    fn revision_wire_rejects_missing_unknown_and_noncanonical_facts() {
        let manifest = SourceDocument::try_new(
            SourceDocumentId::try_new("manifest").expect("document id"),
            SourceName::Memory,
            "schema = 1\n",
        )
        .expect("document");
        let source_set = SourceSetRevision::try_for_identities([manifest.identity()])
            .expect("source-set revision");
        let revision = DialogueProfileRevision::from_admitted_parts(
            manifest.identity().clone(),
            source_set,
            source_set,
            ViewProgramId::try_new("view_program.dialogue").expect("program id"),
            AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("program revision"),
            ResourceTypeRegistry::empty().digest(),
        );
        let canonical = serde_json::to_value(&revision).expect("encode profile revision");

        let mut missing = canonical.clone();
        missing
            .as_object_mut()
            .expect("wire object")
            .remove("compiled_sources");
        let mut unknown = canonical.clone();
        unknown
            .as_object_mut()
            .expect("wire object")
            .insert("legacy_profile".to_owned(), Value::Bool(true));
        let mut invalid_document = canonical.clone();
        invalid_document["manifest_document"]["id"] = Value::String(String::new());
        let mut uppercase_source_set = canonical.clone();
        uppercase_source_set["topology_sources"] =
            Value::String(source_set.to_hex().to_uppercase());
        let mut invalid_program = canonical.clone();
        invalid_program["view_program_id"] = Value::String("#view_program.dialogue".to_owned());
        let mut uppercase_program_revision = canonical.clone();
        uppercase_program_revision["view_program_revision"] = Value::String("5A".repeat(32));
        let mut malformed_registry_digest = canonical;
        malformed_registry_digest["resource_types"] = Value::String("00".repeat(32));

        for tampered in [
            missing,
            unknown,
            invalid_document,
            uppercase_source_set,
            invalid_program,
            uppercase_program_revision,
            malformed_registry_digest,
        ] {
            assert!(serde_json::from_value::<DialogueProfileRevision>(tampered).is_err());
        }
    }
}
