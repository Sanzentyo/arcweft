//! Borrowed Action role lookup in the selected executable's existing types.

use super::{RuntimeProgramTypeError, RuntimeProgramTypes};
use crate::awbc::schema::AwbcRuntimeTypeShape;
use crate::entry::{RuntimeNominalDeclarationId, RuntimeNominalRecordShape};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::RuntimePlanTypeProjection;

impl<'a> RuntimeProgramTypes<'a> {
    pub(crate) fn action_receive_payload(
        self,
        channel: crate::value::RuntimeValueView<'_>,
        channel_type: RuntimeSemanticTypeId,
        destination: RuntimeSemanticTypeId,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<RuntimeSemanticTypeId, RuntimeProgramTypeError> {
        self.value_type(channel_type)?
            .validate_view(channel, binding, resources)?;
        let crate::value::RuntimeValueView::Scalar(crate::value::RuntimeScalarView::EntityRef(
            reference,
        )) = channel
        else {
            return Err(RuntimeProgramTypeError::InvalidActionChannel);
        };
        let payload = self.action_channel_payload(reference, &mut resources.work())?;
        self.value_type(destination)?
            .require_nominal_type(payload, binding, &resources.work())?;
        Ok(payload)
    }

    /// Resolves an already typed channel against the one original declaration
    /// inventory. The shared allowance bounds the complete table scan.
    pub(crate) fn action_channel_payload(
        self,
        channel: &crate::value::RuntimeEntityReference,
        work: &mut crate::entry::schema::value_budget::ValidationWork,
    ) -> Result<RuntimeSemanticTypeId, RuntimeProgramTypeError> {
        let id = channel
            .project_public_id(arcweft_id::DeclarationIdentityFamily::Action)
            .ok_or(RuntimeProgramTypeError::InvalidActionChannel)?;
        let mut selected = None;
        let mut visit = |semantic, declaration: Option<&RuntimeNominalDeclarationId>| {
            work.charge(0)?;
            if declaration.and_then(RuntimeNominalDeclarationId::action) == Some(id) {
                if selected.replace(semantic).is_some() {
                    return Err(RuntimeProgramTypeError::InvalidActionChannel);
                }
            }
            Ok(())
        };
        match self {
            Self::Plan(program) => {
                for row in program.type_table().declarations() {
                    visit(row.semantic_identity(), row.nominal_declaration())?;
                }
            }
            Self::Awbc(program) => {
                for row in &program.runtime_types {
                    visit(row.semantic_identity(), row.nominal_declaration())?;
                }
            }
        }
        let payload = selected.ok_or(RuntimeProgramTypeError::InvalidActionChannel)?;
        self.action_declaration(payload)?;
        Ok(payload)
    }

    /// Authenticates the original payload role without copying metadata or
    /// expanding its fields. This borrowed row alone grants no execution right.
    pub fn action_declaration(
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
