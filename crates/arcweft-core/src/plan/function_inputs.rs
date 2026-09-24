//! FunctionSite input validation shared by all native activation paths.

use crate::runtime_id::RuntimeFunctionSiteId;
use crate::value::{RuntimeFunctionApplyError, RuntimeValue};

use super::{RuntimeFunctionInputSource, RuntimeFunctionSite, RuntimePlan};

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
        }
        Ok(declaration)
    }
}
