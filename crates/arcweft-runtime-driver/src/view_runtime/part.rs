//! Accepted product-to-runtime View-part authority.

use arcweft_bundle::resource_codec::{
    SectionCodecError,
    view::{ViewDefinitionRef, ViewProgramResource},
};
use arcweft_view::{ViewPartLocalName, ViewPartName, ViewProgramId};
use std::collections::BTreeMap;

/// One validated immutable View program accepted by the runtime.
#[derive(Clone, Debug)]
pub(crate) struct AcceptedViewProgram {
    resource: ViewProgramResource,
    program_id: ViewProgramId,
    parts: ViewPartRuntimeCatalog,
}

/// Deterministic owner-local IDs and public capabilities derived at acceptance.
#[derive(Clone, Debug, Default)]
pub(crate) struct ViewPartRuntimeCatalog {
    exports: BTreeMap<(ViewDefinitionRef, ViewPartLocalName), ViewPartName>,
}

impl AcceptedViewProgram {
    pub(crate) fn try_new(resource: ViewProgramResource) -> Result<Self, SectionCodecError> {
        let _ = resource.encode_canonical_section()?;
        let program_id = ViewProgramId::try_new(resource.program_id.clone())
            .map_err(|_| SectionCodecError::NonCanonicalTable("view_program_identities"))?;
        let parts = ViewPartRuntimeCatalog::from_resource(&resource);
        Ok(Self {
            resource,
            program_id,
            parts,
        })
    }

    pub(crate) const fn resource(&self) -> &ViewProgramResource {
        &self.resource
    }

    pub(crate) const fn parts(&self) -> &ViewPartRuntimeCatalog {
        &self.parts
    }

    pub(crate) const fn program_id(&self) -> &ViewProgramId {
        &self.program_id
    }
}

impl ViewPartRuntimeCatalog {
    fn from_resource(resource: &ViewProgramResource) -> Self {
        let exports = resource
            .exported_parts
            .iter()
            .map(|export| {
                (
                    (export.target.view.clone(), export.target.part.clone()),
                    export.public_name.clone(),
                )
            })
            .collect();
        Self { exports }
    }

    pub(crate) fn public_name(
        &self,
        owner: &str,
        local: &ViewPartLocalName,
    ) -> Option<&ViewPartName> {
        let owner = ViewDefinitionRef::try_new(owner.to_owned()).ok()?;
        self.exports.get(&(owner, local.clone()))
    }
}
