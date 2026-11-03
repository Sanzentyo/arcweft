//! Borrowed Action role lookup in the selected executable's existing types.

use super::{RuntimeProgramTypeError, RuntimeProgramTypes};
use crate::awbc::schema::AwbcRuntimeTypeShape;
use crate::entry::{RuntimeNominalDeclarationId, RuntimeNominalRecordShape};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::RuntimePlanTypeProjection;

impl<'a> RuntimeProgramTypes<'a> {
    /// Authenticates the original payload role without copying metadata or
    /// expanding its fields. This borrowed row alone grants no execution right.
    pub(crate) fn action_declaration(
        self,
        semantic_type: RuntimeSemanticTypeId,
    ) -> Result<&'a RuntimeNominalDeclarationId, RuntimeProgramTypeError> {
        let declaration = match self {
            Self::Plan(program) => {
                let ty = program
                    .type_table()
                    .id_for_semantic(semantic_type)
                    .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?;
                let row = program
                    .type_table()
                    .get(ty)
                    .expect("semantic lookup selects its original declaration");
                if !matches!(row.projection(), RuntimePlanTypeProjection::Nominal { arguments, .. }
                    if arguments.is_empty())
                    || !program
                        .nominal_record_domains()
                        .get(ty)
                        .is_some_and(|domain| domain.shape() == RuntimeNominalRecordShape::Record)
                {
                    return Err(RuntimeProgramTypeError::NotActionPayload { semantic_type });
                }
                row.nominal_declaration()
            }
            Self::Awbc(program) => {
                let ty = self.awbc_type(semantic_type)?;
                let row = &program.runtime_types[ty.index()];
                if !matches!(row.shape(), AwbcRuntimeTypeShape::NominalRecord { arguments, shape, .. }
                    if arguments.is_empty() && *shape == RuntimeNominalRecordShape::Record)
                {
                    return Err(RuntimeProgramTypeError::NotActionPayload { semantic_type });
                }
                row.nominal_declaration()
            }
        };
        declaration
            .filter(|declaration| declaration.action().is_some())
            .ok_or(RuntimeProgramTypeError::NotActionPayload { semantic_type })
    }
}
