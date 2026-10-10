use super::{RuntimeEntityReference, runtime_public_id};
use crate::plan::{FlowRuntimeId, RuntimeLineId};
use arcweft_id::{ProjectEntityReferenceFamily, PublicId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A reference issued from one accepted target-project entity generation.
/// The importer retains the original Flow declaration digest separately from
/// its public label. This value confers no callable or `goto` capability.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "ImportedProjectEntityParts")]
pub struct RuntimeImportedProjectEntityReference {
    family: ProjectEntityReferenceFamily,
    #[serde(with = "runtime_public_id")]
    public_id: PublicId,
    target_generation: [u8; 32],
    semantic_identity: [u8; 32],
    value_type: [u8; 32],
    target: ImportedProjectTarget,
}

/// A closed runtime address. Flow keeps the native declaration digest; line
/// uses the same validated RuntimeLineId as the original source line.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum ImportedProjectTarget {
    Public,
    Flow(FlowRuntimeId),
    Line(RuntimeLineId),
}

#[derive(Deserialize)]
struct ImportedProjectEntityParts {
    family: ProjectEntityReferenceFamily,
    #[serde(with = "runtime_public_id")]
    public_id: PublicId,
    target_generation: [u8; 32],
    semantic_identity: [u8; 32],
    value_type: [u8; 32],
    target: ImportedProjectTarget,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeImportedProjectEntityError {
    #[error("imported project reference has an invalid family: {0}")]
    Family(#[from] arcweft_id::PublicIdFamilyError),
    #[error("imported project Flow reference is missing or mismaps its exact checked identity")]
    FlowIdentity,
    #[error("a non-Flow imported project reference carries a Flow identity")]
    UnexpectedFlowIdentity,
    #[error("imported dialogue line cannot project its owning runtime address: {0}")]
    LineIdentity(#[source] crate::runtime_id::RuntimeIdError),
    #[error("imported project reference mismaps its runtime address and public identity")]
    TargetIdentity,
    #[error(
        "imported project reference is missing its admitted generation, semantic identity or value type"
    )]
    MissingIdentity,
}

impl RuntimeImportedProjectEntityReference {
    pub fn try_new(
        family: ProjectEntityReferenceFamily,
        public_id: PublicId,
        target_generation: [u8; 32],
        semantic_identity: [u8; 32],
        value_type: [u8; 32],
        flow: Option<FlowRuntimeId>,
    ) -> Result<Self, RuntimeImportedProjectEntityError> {
        family.validate_public_id(&public_id)?;
        if [target_generation, semantic_identity, value_type].contains(&[0; 32]) {
            return Err(RuntimeImportedProjectEntityError::MissingIdentity);
        }
        let target = match (family, flow) {
            (ProjectEntityReferenceFamily::Flow, Some(flow))
                if flow.public_label().as_str() == public_id.as_str() =>
            {
                ImportedProjectTarget::Flow(flow)
            }
            (ProjectEntityReferenceFamily::Flow, _) => {
                return Err(RuntimeImportedProjectEntityError::FlowIdentity);
            }
            (_, Some(_)) => return Err(RuntimeImportedProjectEntityError::UnexpectedFlowIdentity),
            (ProjectEntityReferenceFamily::DialogueLine, None) => ImportedProjectTarget::Line(
                RuntimeLineId::from_source_entity_body(public_id.as_str())
                    .map_err(RuntimeImportedProjectEntityError::LineIdentity)?,
            ),
            (_, None) => ImportedProjectTarget::Public,
        };
        Ok(Self {
            family,
            public_id,
            target_generation,
            semantic_identity,
            value_type,
            target,
        })
    }

    pub const fn family(&self) -> ProjectEntityReferenceFamily {
        self.family
    }
    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }
    pub const fn target_generation(&self) -> &[u8; 32] {
        &self.target_generation
    }
    pub const fn semantic_identity(&self) -> &[u8; 32] {
        &self.semantic_identity
    }
    pub const fn value_type(&self) -> &[u8; 32] {
        &self.value_type
    }
    pub const fn flow(&self) -> Option<&FlowRuntimeId> {
        match &self.target {
            ImportedProjectTarget::Flow(flow) => Some(flow),
            _ => None,
        }
    }
    pub const fn line(&self) -> Option<&RuntimeLineId> {
        match &self.target {
            ImportedProjectTarget::Line(line) => Some(line),
            _ => None,
        }
    }
}

impl TryFrom<ImportedProjectEntityParts> for RuntimeImportedProjectEntityReference {
    type Error = RuntimeImportedProjectEntityError;
    fn try_from(parts: ImportedProjectEntityParts) -> Result<Self, Self::Error> {
        let flow = match &parts.target {
            ImportedProjectTarget::Flow(flow) => Some(flow.clone()),
            _ => None,
        };
        let value = Self::try_new(
            parts.family,
            parts.public_id,
            parts.target_generation,
            parts.semantic_identity,
            parts.value_type,
            flow,
        )?;
        if value.target != parts.target {
            return Err(RuntimeImportedProjectEntityError::TargetIdentity);
        }
        Ok(value)
    }
}

impl From<RuntimeImportedProjectEntityReference> for RuntimeEntityReference {
    fn from(value: RuntimeImportedProjectEntityReference) -> Self {
        Self::ImportedProject(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    fn imported_flow(digest: [u8; 32]) -> RuntimeImportedProjectEntityReference {
        let flow = FlowRuntimeId::from_checked_declaration_digest(digest, "flow.opening")
            .expect("checked Flow identity");
        RuntimeImportedProjectEntityReference::try_new(
            ProjectEntityReferenceFamily::Flow,
            PublicId::try_new("flow.opening").expect("Flow label"),
            [1; 32],
            [2; 32],
            [3; 32],
            Some(flow),
        )
        .expect("imported Flow")
    }

    #[test]
    fn imported_entity_identity_uses_exact_flow_and_logical_declaration_family() {
        let first = imported_flow([0x11; 32]);
        let foreign = imported_flow([0x22; 32]);
        let local = RuntimeEntityReference::StructuralFlow(first.flow().expect("Flow").clone());
        let imported = RuntimeEntityReference::ImportedProject(first);
        assert_eq!(local, imported);
        assert_ne!(imported, RuntimeEntityReference::ImportedProject(foreign));
        assert_eq!(HashSet::from([local.clone(), imported.clone()]).len(), 1);
        assert_eq!(BTreeSet::from([local, imported]).len(), 1);
        let id = PublicId::try_new("signal.level").expect("Signal ID");
        let imported = RuntimeEntityReference::ImportedProject(
            RuntimeImportedProjectEntityReference::try_new(
                ProjectEntityReferenceFamily::Signal,
                id.clone(),
                [1; 32],
                [2; 32],
                [3; 32],
                None,
            )
            .expect("Signal owner"),
        );
        let local =
            RuntimeEntityReference::try_project(arcweft_id::DeclarationIdentityFamily::Signal, id)
                .expect("Signal ref");
        assert_eq!(local, imported);
        assert_eq!(
            imported.project_label(arcweft_id::DeclarationIdentityFamily::Signal),
            Some("signal.level")
        );
        assert_eq!(
            imported.project_label(arcweft_id::DeclarationIdentityFamily::Flow),
            None
        );
    }

    #[test]
    fn imported_entity_serde_rejects_forged_family_flow_and_missing_digest() {
        let reference = imported_flow([0x11; 32]);
        let encoded = serde_json::to_value(&reference).expect("imported entity JSON");
        let decoded: RuntimeImportedProjectEntityReference =
            serde_json::from_value(encoded.clone()).expect("same closed entity");
        assert_eq!(decoded, reference);
        for field in ["target_generation", "semantic_identity", "value_type"] {
            let mut forged = encoded.clone();
            forged[field] = serde_json::to_value([0u8; 32]).expect("zero digest");
            assert!(
                serde_json::from_value::<RuntimeImportedProjectEntityReference>(forged).is_err()
            );
        }
        let mut forged = encoded.clone();
        forged["target"] = serde_json::json!("public");
        assert!(serde_json::from_value::<RuntimeImportedProjectEntityReference>(forged).is_err());
        let mut forged = encoded;
        forged["family"] = serde_json::json!("signal");
        assert!(serde_json::from_value::<RuntimeImportedProjectEntityReference>(forged).is_err());
    }
}

#[cfg(test)]
mod imported_line_tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    #[test]
    fn imported_line_uses_original_runtime_line_for_equality_fields_and_error_provenance() {
        let imported = RuntimeEntityReference::ImportedProject(
            RuntimeImportedProjectEntityReference::try_new(
                ProjectEntityReferenceFamily::DialogueLine,
                PublicId::try_new("say.opening").expect("source line"),
                [1; 32],
                [2; 32],
                [3; 32],
                None,
            )
            .expect("admitted line address"),
        );
        let native = RuntimeEntityReference::DialogueLine(
            RuntimeLineId::from_source_entity_body("say.opening").expect("native line"),
        );
        assert_eq!(imported, native);
        assert_eq!(imported.line_identity(), native.line_identity());
        assert_eq!(imported.runtime_label(), native.runtime_label());
        assert_eq!(HashSet::from([imported.clone(), native.clone()]).len(), 1);
        assert_eq!(BTreeSet::from([imported.clone(), native]).len(), 1);
        assert!(
            crate::value::RuntimeArcErrorFrame::empty()
                .with_line(imported.clone())
                .is_ok()
        );
        let encoded = serde_json::to_value(&imported).expect("line with imported provenance");
        let decoded: RuntimeEntityReference =
            serde_json::from_value(encoded).expect("same validated line");
        assert_eq!(decoded, imported);
    }
}
