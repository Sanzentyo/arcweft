//! Original endpoint admission for inert request definitions. The encoder
//! alone produces bytes; only an exact source-derived definition issues Q.

use super::{RuntimeTaskRequestTemplate, TaskRequestTemplateDigest};
use crate::plan::RuntimeTaskPlanSealLimits;
use crate::plan::body_semantic::function::ProducerEndpoint;
use crate::plan::body_semantic::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::task::semantic::TaskSemanticMeter;

impl RuntimeBodySemanticContext<'_> {
    pub(crate) fn request_template_digest(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: ProducerEndpoint<'_>,
        template: &RuntimeTaskRequestTemplate,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<TaskRequestTemplateDigest, RuntimeBodySemanticError> {
        let result = self.admit_request_template(meter, endpoint, template, limits);
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    fn admit_request_template(
        &self,
        meter: &mut TaskSemanticMeter,
        endpoint: ProducerEndpoint<'_>,
        template: &RuntimeTaskRequestTemplate,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<TaskRequestTemplateDigest, RuntimeBodySemanticError> {
        meter.status()?;
        let actual = meter.checked_count_sum(template.arguments.len(), template.fields.len())?;
        if actual > limits.max_request_roles as usize {
            return Err(RuntimeBodySemanticError::RequestRoles {
                actual,
                maximum: limits.max_request_roles,
            });
        }
        if !endpoint.belongs_to(self.plan) {
            return Err(RuntimeBodySemanticError::InvalidHostRequestEndpoint);
        }
        if template.endpoint != endpoint.ordinal() {
            return Err(RuntimeBodySemanticError::InvalidRequestEndpoint {
                expected: endpoint.ordinal(),
                actual: template.endpoint,
            });
        }
        let arguments = endpoint
            .host_arguments()
            .ok_or(RuntimeBodySemanticError::InvalidHostRequestEndpoint)?;
        let mismatch = |component, ordinal| RuntimeBodySemanticError::RequestTemplateMismatch {
            endpoint: endpoint.ordinal(),
            component,
            ordinal,
        };
        if arguments.len() != template.arguments.len() {
            return Err(mismatch("argument count", template.arguments.len()));
        }
        if !template.fields.is_empty() {
            return Err(mismatch("field count", template.fields.len()));
        }
        for argument in &template.arguments {
            meter.charge_work(1)?;
            if self.plan.type_table().get(argument.ty).is_none() {
                return Err(RuntimeBodySemanticError::UnknownType { ty: argument.ty });
            }
        }
        // The same owning extraction supplies role, type, source and complete
        // path meaning. No second resolver or evaluated payload is consulted.
        let expected = self.host_request_template(meter, endpoint, limits)?;
        for (ordinal, (actual, expected)) in template
            .arguments
            .iter()
            .zip(&expected.arguments)
            .enumerate()
        {
            meter.charge_work(1)?;
            if actual != expected {
                return Err(mismatch("argument definition", ordinal));
            }
        }
        self.encode_request_template(meter, endpoint, template, limits)
            .map(|hash| TaskRequestTemplateDigest(*hash.as_bytes()))
    }
}
