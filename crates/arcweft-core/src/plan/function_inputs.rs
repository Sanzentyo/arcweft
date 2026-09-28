//! FunctionSite input validation shared by all native activation paths.

use crate::runtime_id::RuntimeFunctionSiteId;
use crate::value::{RuntimeFunctionApplyError, RuntimeValue};

use super::{
    RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputSource, RuntimeFunctionSite,
    RuntimePlan,
};

impl RuntimePlan {
    /// Validates captures and the complete parameter ABI without mutating a
    /// frame. Defaults, callables and content bodies all enter through this
    /// FunctionSite boundary; validation never constructs a callable value.
    pub(crate) fn validate_function_site_inputs(
        &self,
        site: RuntimeFunctionSiteId,
        captures: &[RuntimeValue],
        arguments: &[RuntimeValue],
    ) -> Result<&RuntimeFunctionSite, RuntimeFunctionApplyError> {
        let captures = captures.iter().collect::<Vec<_>>();
        let arguments = arguments.iter().collect::<Vec<_>>();
        self.validate_function_site_input_refs(site, &captures, &arguments)
    }

    /// Checks a projected invocation before any affine callable input is
    /// transferred from its retained packet into a child frame.
    pub(crate) fn validate_function_site_input_refs(
        &self,
        site: RuntimeFunctionSiteId,
        captures: &[&RuntimeValue],
        arguments: &[&RuntimeValue],
    ) -> Result<&RuntimeFunctionSite, RuntimeFunctionApplyError> {
        let declaration = self
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        let capture_count = declaration.capture_inputs().count();
        if capture_count != captures.len() {
            return Err(RuntimeFunctionApplyError::CaptureCountMismatch {
                site,
                expected: capture_count,
                actual: captures.len(),
            });
        }
        let parameter_count = declaration.parameter_inputs().count();
        if parameter_count != arguments.len() {
            return Err(RuntimeFunctionApplyError::TooManyArguments {
                remaining: parameter_count,
                provided: arguments.len(),
            });
        }
        for input in declaration.inputs() {
            let local = input.input_local();
            let expected = self
                .local_declarations()
                .get(local)
                .ok_or(RuntimeFunctionApplyError::UnknownStructuredLocal { site, local })?
                .ty();
            let (values, index) = match input.source() {
                RuntimeFunctionInputSource::Capture { position } => (captures, position as usize),
                RuntimeFunctionInputSource::Parameter { position } => {
                    (arguments, position as usize)
                }
            };
            let value = values
                .get(index)
                .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?;
            if !self.value_matches_type(expected, value)? {
                return Err(match input.source() {
                    RuntimeFunctionInputSource::Capture { .. } => {
                        RuntimeFunctionApplyError::CaptureTypeMismatch {
                            site,
                            index,
                            local,
                            expected,
                        }
                    }
                    RuntimeFunctionInputSource::Parameter { .. } => {
                        RuntimeFunctionApplyError::ArgumentTypeMismatch {
                            site,
                            index,
                            local,
                            expected,
                        }
                    }
                });
            }
            if input.ownership() == RuntimeFunctionInputOwnershipRequirement::Unrestricted
                && !value.ownership().permits_copy()
            {
                return Err(RuntimeFunctionApplyError::UnrestrictedInputRequired {
                    site,
                    input: input.source(),
                });
            }
            for &local in input.unrestricted_bindings() {
                let ownership = crate::pattern::runtime_pattern_binding_ownership(
                    input.pattern(),
                    value,
                    local,
                )
                .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?;
                if !ownership.permits_copy() {
                    return Err(RuntimeFunctionApplyError::UnrestrictedBindingRequired {
                        site,
                        local,
                    });
                }
            }
        }
        Ok(declaration)
    }
}
