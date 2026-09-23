//! Closed source variants use the same graph and payload projection as ADTs.

use super::{Error, NominalGraphProjection, Schema, VisitState};
use crate::final_analysis::{CheckedVariantOwner, CheckedVariantOwnerKind};
use arcweft_core::entry::{
    RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
    RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
};

impl NominalGraphProjection<'_> {
    pub(super) fn closed_variant(
        &mut self,
        owner: &CheckedVariantOwner,
        depth: u64,
    ) -> Result<Schema, Error> {
        let semantic = owner.semantic_type();
        let nominal = match owner.kind() {
            CheckedVariantOwnerKind::CharacterNominal { .. } => {
                RuntimeNominalTypeId::from_checked_digest(*semantic.as_bytes())
            }
            CheckedVariantOwnerKind::BuiltinClosed { nominal, .. } => {
                RuntimeNominalTypeId::try_new(nominal.as_str().to_owned())?
            }
            _ => {
                return Err(Error::UnsupportedType {
                    semantic_type: semantic,
                });
            }
        };
        self.budget.edge()?;
        if let Some(VisitState::Visiting(identity) | VisitState::Complete(identity)) =
            self.states.get(&semantic)
        {
            return Ok(Schema::NominalRef(identity.clone()));
        }
        self.budget.definition(self.active_depth + 1)?;
        self.budget.members(owner.cases().len())?;
        let identity = RuntimeNominalSchemaIdentity::new(nominal, semantic.into());
        self.states
            .insert(semantic, VisitState::Visiting(identity.clone()));
        self.active_depth += 1;
        let cases = owner
            .cases()
            .iter()
            .map(|case| {
                let invalid = || Error::InvalidVariantPayload {
                    semantic_type: semantic,
                    ordinal: case.ordinal(),
                };
                let name = case.diagnostic_name().ok_or_else(invalid)?;
                self.budget.name(name)?;
                let payload = owner
                    .case_payload_type(case.ordinal())
                    .ok_or_else(invalid)?
                    .map(|ty| self.schema(&ty, depth + 1))
                    .transpose()?;
                Ok(RuntimeNominalSchemaCase::new(
                    case.ordinal(),
                    name.to_owned(),
                    payload,
                ))
            })
            .collect::<Result<_, Error>>()?;
        self.definitions.push(RuntimeNominalSchemaDefinition::new(
            identity.clone(),
            vec![],
            RuntimeNominalSchemaBody::Variant { cases },
        ));
        self.active_depth -= 1;
        self.states
            .insert(semantic, VisitState::Complete(identity.clone()));
        Ok(Schema::NominalRef(identity))
    }
}
