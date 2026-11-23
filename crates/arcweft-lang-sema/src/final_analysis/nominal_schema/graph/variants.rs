//! Closed source variants use the same walk and payload projection as ADTs.

use arcweft_core::entry::{
    RuntimeNominalDeclarationId, RuntimeNominalSchemaIdentity, RuntimeNominalTypeId,
};

use super::{
    Error, NominalGraphProjection, Schema, VisitState,
    walk::{Body, Definition, Task, Walk},
};
use crate::final_analysis::{CheckedVariantCase, CheckedVariantOwner, CheckedVariantOwnerKind};

impl NominalGraphProjection<'_> {
    pub(super) fn closed_variant<'walk>(
        &mut self,
        walk: &mut Walk<'walk>,
        owner: &'walk CheckedVariantOwner,
        depth: u64,
    ) -> Result<(), Error> {
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
            walk.emit(Schema::NominalRef(identity.clone()));
            return Ok(());
        }
        self.budget.definition(self.active_depth + 1)?;
        self.budget.members(owner.cases().len())?;
        let identity = RuntimeNominalSchemaIdentity::new(nominal, semantic.into());
        self.states
            .insert(semantic, VisitState::Visiting(identity.clone()));
        self.active_depth += 1;
        walk.push(Task::FinishDefinition(
            Definition {
                semantic,
                identity,
                declaration: RuntimeNominalDeclarationId::from_bytes(*semantic.as_bytes()),
                arguments: 0,
                rust: None,
            },
            Body::Variant(owner.cases().len()),
        ));
        for case in owner.cases().iter().rev() {
            walk.push(Task::ClosedCase(ClosedCase {
                owner,
                case,
                depth: depth + 1,
            }));
        }
        Ok(())
    }
}

pub(super) struct ClosedCase<'walk> {
    owner: &'walk CheckedVariantOwner,
    case: &'walk CheckedVariantCase,
    depth: u64,
}

impl<'walk> ClosedCase<'walk> {
    pub(super) fn visit<'source: 'walk>(
        self,
        projection: &mut NominalGraphProjection<'source>,
        walk: &mut Walk<'walk>,
    ) -> Result<(), Error> {
        let ordinal = self.case.ordinal();
        let invalid = || Error::InvalidVariantPayload {
            semantic_type: self.owner.semantic_type(),
            ordinal,
        };
        let name = self.case.diagnostic_name().ok_or_else(invalid)?;
        projection.budget.name(name)?;
        let payload = self.owner.case_payload_type(ordinal).ok_or_else(invalid)?;
        walk.push(Task::Case {
            ordinal,
            name,
            has_payload: payload.is_some(),
        });
        if let Some(ty) = payload {
            walk.visit_instantiated(projection, ty, self.depth, None)?;
        }
        Ok(())
    }
}
