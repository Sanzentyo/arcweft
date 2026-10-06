//! FunctionSite input validation shared by all native activation paths.

use crate::runtime_id::RuntimeFunctionSiteId;
use crate::value::{RuntimeFunctionApplyError, RuntimeValue};

use super::{
    RuntimeFunctionInputOwnershipRequirement, RuntimeFunctionInputSource, RuntimeFunctionSite,
    RuntimePlan,
};

pub(crate) struct RuntimeFunctionInputAdmission<'plan> {
    declaration: &'plan RuntimeFunctionSite,
    pub(crate) type_instantiation:
        Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
}

impl std::ops::Deref for RuntimeFunctionInputAdmission<'_> {
    type Target = RuntimeFunctionSite;
    fn deref(&self) -> &Self::Target {
        self.declaration
    }
}

impl RuntimePlan {
    pub(crate) fn validate_function_instantiation(
        &self,
        site: RuntimeFunctionSiteId,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), RuntimeFunctionApplyError> {
        let declaration = self
            .function_sites()
            .get(site)
            .ok_or(RuntimeFunctionApplyError::UnknownStructuredSite { site })?;
        let expected = declaration
            .function_type()
            .and_then(|ty| self.type_table().get(ty))
            .map(|row| row.semantic_identity());
        match (expected, binding) {
            (None, None) => Ok(()),
            (Some(expected), Some(binding))
                if expected == binding.context() && binding.is_valid(self) =>
            {
                Ok(())
            }
            _ => Err(RuntimeFunctionApplyError::InvalidEffectInstantiation { site }),
        }
    }

    /// Validates captures and the complete parameter ABI without mutating a
    /// frame. Defaults, callables and content bodies all enter through this
    /// FunctionSite boundary; validation never constructs a callable value.
    pub(crate) fn validate_function_site_inputs(
        &self,
        site: RuntimeFunctionSiteId,
        captures: &[RuntimeValue],
        arguments: &[RuntimeValue],
    ) -> Result<RuntimeFunctionInputAdmission<'_>, RuntimeFunctionApplyError> {
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
    ) -> Result<RuntimeFunctionInputAdmission<'_>, RuntimeFunctionApplyError> {
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
        let references = declaration
            .inputs()
            .iter()
            .map(|input| {
                let (values, index) = match input.source() {
                    RuntimeFunctionInputSource::Capture { position }
                    | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                        (captures, position as usize)
                    }
                    RuntimeFunctionInputSource::Parameter { position, .. } => {
                        (arguments, position as usize)
                    }
                };
                values
                    .get(index)
                    .copied()
                    .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let type_instantiation = declaration
            .function_type()
            .map(|context| {
                let context = self
                    .type_table()
                    .get(context)
                    .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?
                    .semantic_identity();
                crate::program_types::RuntimeProgramTypes::Plan(self)
                    .instantiate_function_effects(context, &references)
                    .map(std::sync::Arc::new)
                    .ok_or(RuntimeFunctionApplyError::InvalidEffectInstantiation { site })
            })
            .transpose()?;
        for input in declaration.inputs() {
            let local = input.input_local();
            let expected = self
                .local_declarations()
                .get(local)
                .ok_or(RuntimeFunctionApplyError::UnknownStructuredLocal { site, local })?
                .ty();
            let (values, index) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (captures, position as usize)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    (arguments, position as usize)
                }
            };
            let value = values
                .get(index)
                .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?;
            let matches = match &type_instantiation {
                Some(binding) => binding.value_matches(self, expected, value),
                None => self.value_matches_type(expected, value)?,
            };
            if !matches {
                return Err(match input.source() {
                    RuntimeFunctionInputSource::Capture { .. }
                    | RuntimeFunctionInputSource::CapturedParameter { .. } => {
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
        Ok(RuntimeFunctionInputAdmission {
            declaration,
            type_instantiation,
        })
    }
}
