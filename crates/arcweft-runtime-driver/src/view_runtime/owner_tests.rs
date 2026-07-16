use arcweft_view::{RustViewId, ViewDescriptor, ViewId, ViewRegistry, ViewSchemaId};

use super::owner::{
    AcceptedViewProgramGeneration, ResolvedMountedViewOwner, SavedViewOwner, ViewOwnerEvidence,
    ViewSaveError,
};

fn view_id(value: &str) -> ViewId {
    ViewId::try_new(value).unwrap()
}

#[test]
fn anonymous_and_public_rust_owners_project_only_stable_evidence() {
    let mut registry = ViewRegistry::default();
    let anonymous = registry
        .register(ViewDescriptor::anonymous_rust(
            ViewSchemaId(11),
            RustViewId(21),
        ))
        .unwrap();
    let public_view = view_id("view.host.public");
    let public = registry
        .register(ViewDescriptor::public_rust(
            public_view.clone(),
            ViewSchemaId(12),
            RustViewId(22),
        ))
        .unwrap();

    let anonymous_owner = ResolvedMountedViewOwner::resolve_registry(
        anonymous,
        &registry,
        None,
        AcceptedViewProgramGeneration::INITIAL,
    )
    .unwrap();
    let public_owner = ResolvedMountedViewOwner::resolve_registry(
        public,
        &registry,
        None,
        AcceptedViewProgramGeneration::INITIAL,
    )
    .unwrap();

    assert_eq!(anonymous_owner.evidence(), ViewOwnerEvidence::AnonymousHost);
    assert_eq!(
        anonymous_owner.saved(&registry),
        Err(ViewSaveError::AnonymousRustViewNotPersistable)
    );
    assert_eq!(
        public_owner.evidence(),
        ViewOwnerEvidence::Public {
            view: public_view.clone()
        }
    );
    assert_eq!(
        public_owner.saved(&registry),
        Ok(SavedViewOwner::Rust {
            view: public_view,
            schema: ViewSchemaId(12),
        })
    );

    assert_eq!(
        serde_json::to_string(&anonymous_owner.evidence()).unwrap(),
        r#"{"kind":"anonymous_host"}"#
    );
    assert_eq!(
        serde_json::to_string(&public_owner.evidence()).unwrap(),
        r#"{"kind":"public","view":"view.host.public"}"#
    );
}

#[test]
fn public_rust_owner_restores_through_stable_view_and_schema_in_a_fresh_registry() {
    let public_view = view_id("view.host.persisted");
    let mut original_registry = ViewRegistry::default();
    let _anonymous = original_registry
        .register(ViewDescriptor::anonymous_rust(
            ViewSchemaId(31),
            RustViewId(41),
        ))
        .unwrap();
    let original_slot = original_registry
        .register(ViewDescriptor::public_rust(
            public_view.clone(),
            ViewSchemaId(32),
            RustViewId(42),
        ))
        .unwrap();
    let original = ResolvedMountedViewOwner::resolve_registry(
        original_slot,
        &original_registry,
        None,
        AcceptedViewProgramGeneration::INITIAL,
    )
    .unwrap();
    let saved = original.saved(&original_registry).unwrap();

    let mut restored_registry = ViewRegistry::default();
    let restored_slot = restored_registry
        .register(ViewDescriptor::public_rust(
            public_view,
            ViewSchemaId(32),
            RustViewId(52),
        ))
        .unwrap();
    assert_ne!(original_slot, restored_slot);
    let restored = ResolvedMountedViewOwner::resolve_saved(
        &saved,
        &restored_registry,
        None,
        AcceptedViewProgramGeneration::INITIAL,
    )
    .unwrap();

    assert_eq!(restored.saved(&restored_registry), Ok(saved.clone()));

    let SavedViewOwner::Rust { view, .. } = saved else {
        unreachable!("public Rust owner saves as the Rust owner form")
    };
    let forged = SavedViewOwner::Rust {
        view,
        schema: ViewSchemaId(99),
    };
    assert_eq!(
        ResolvedMountedViewOwner::resolve_saved(
            &forged,
            &restored_registry,
            None,
            AcceptedViewProgramGeneration::INITIAL,
        ),
        Err(ViewSaveError::ImplementationKindMismatch)
    );
}
