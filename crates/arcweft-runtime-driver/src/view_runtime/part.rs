//! Accepted product-to-runtime View-part authority.

use arcweft_view::{ViewId, ViewPartLocalName, ViewPartName};
use std::collections::BTreeMap;

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

    pub(crate) fn public_name(
        &self,
        owner: &ViewId,
        local: &ViewPartLocalName,
    ) -> Option<&ViewPartName> {
        self.exports.get(&(owner.clone(), local.clone()))
    }
}
