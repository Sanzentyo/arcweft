//! Accepted product-to-runtime View-part authority.

use arcweft_view::{ViewId, ViewPartLocalName, ViewPartName};
use std::collections::BTreeMap;

use super::owner::ResolvedMountedViewOwner;

/// Deterministic owner-local IDs and public capabilities derived at acceptance.
#[derive(Clone, Debug, Default)]
pub(crate) struct ViewPartRuntimeCatalog {
    exports: BTreeMap<(ViewId, ViewPartLocalName), ViewPartName>,
}

impl ViewPartRuntimeCatalog {
    pub(super) fn from_programs<'a>(
        definitions: impl IntoIterator<Item = &'a super::catalog::RuntimeViewDefinition>,
    ) -> Self {
        let mut exports = BTreeMap::new();
        for definition in definitions {
            for export in definition.semantic.exported_parts() {
                let local = &definition.semantic.static_parts()[export.part().index()];
                exports.insert(
                    (definition.view.clone(), local.local_name().clone()),
                    export.public_name().clone(),
                );
            }
        }
        Self { exports }
    }

    pub(super) fn public_name(
        &self,
        owner: &ResolvedMountedViewOwner,
        local: &ViewPartLocalName,
    ) -> Option<&ViewPartName> {
        let ResolvedMountedViewOwner::Arcweft { view, .. } = owner else {
            return None;
        };
        self.exports.get(&(view.clone(), local.clone()))
    }
}

#[cfg(test)]
mod tests {
    use arcweft_view::{
        RustViewId, ViewDescriptor, ViewId, ViewPartLocalName, ViewPartName, ViewRegistry,
        ViewSchemaId,
    };

    use super::{ResolvedMountedViewOwner, ViewPartRuntimeCatalog};
    use crate::view_runtime::AcceptedViewProgramGeneration;

    #[test]
    fn public_rust_owner_cannot_mint_an_arcweft_exported_part_capability() {
        let view = ViewId::try_new("view.shared").unwrap();
        let local = ViewPartLocalName::try_new("panel.title").unwrap();
        let public = ViewPartName::try_new("panel.title").unwrap();
        let catalog = ViewPartRuntimeCatalog {
            exports: [((view.clone(), local.clone()), public)]
                .into_iter()
                .collect(),
        };
        let mut registry = ViewRegistry::default();
        let slot = registry
            .register(ViewDescriptor::public_rust(
                view,
                ViewSchemaId(1),
                RustViewId(1),
            ))
            .unwrap();
        let owner = ResolvedMountedViewOwner::resolve_registry(
            slot,
            &registry,
            None,
            AcceptedViewProgramGeneration::INITIAL,
        )
        .unwrap();

        assert_eq!(catalog.public_name(&owner, &local), None);
    }
}
