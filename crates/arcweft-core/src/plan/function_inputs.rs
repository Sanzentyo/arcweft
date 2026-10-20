//! FunctionSite input validation shared by all native activation paths.

use crate::runtime_id::RuntimeFunctionSiteId;
use crate::value::{RuntimeFunctionApplyError, RuntimeValue};

use super::{
    RuntimeFunctionInputSource, RuntimeFunctionSite, RuntimePlan, RuntimeValueOwnershipRequirement,
};

pub(crate) struct RuntimeFunctionInputAdmission<'plan> {
    plan: &'plan RuntimePlan,
    site: RuntimeFunctionSiteId,
    declaration: &'plan RuntimeFunctionSite,
    pub(crate) type_instantiation:
        Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
}

impl RuntimeFunctionInputAdmission<'_> {
    pub(crate) const fn site(&self) -> RuntimeFunctionSiteId {
        self.site
    }

    /// Admits all formal patterns before copying or transferring any input.
    pub(crate) fn validate_owned_patterns(
        &self,
        captures: &[&RuntimeValue],
        arguments: &[&RuntimeValue],
    ) -> Result<(), crate::value::RuntimeEvalError> {
        for input in self.declaration.inputs() {
            let (values, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (captures, position)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => (arguments, position),
            };
            let value = values
                .get(position as usize)
                .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site: self.site })?;
            if !crate::pattern::inspect_runtime_pattern_owned(
                self.plan,
                input.pattern(),
                value,
                self.type_instantiation.as_deref(),
            )? {
                return Err(crate::value::RuntimeEvalError::PatternMismatch(format!(
                    "function site {} input {:?}",
                    self.site,
                    input.source()
                )));
            }
        }
        Ok(())
    }

    /// Transfers the admitted input owners through their actual formal patterns.
    /// Flow roots and structured calls share this binding rule.
    pub(crate) fn stage_owned_inputs(
        &self,
        captures: impl IntoIterator<Item = RuntimeValue>,
        arguments: impl IntoIterator<Item = RuntimeValue>,
    ) -> Result<Vec<crate::value::RuntimeLocalBinding>, crate::value::RuntimeEvalError> {
        let site = self.site;
        let mut captures = captures.into_iter().map(Some).collect::<Vec<_>>();
        let mut arguments = arguments.into_iter().map(Some).collect::<Vec<_>>();
        self.validate_owned_patterns(
            &captures.iter().flatten().collect::<Vec<_>>(),
            &arguments.iter().flatten().collect::<Vec<_>>(),
        )?;
        let mut staged = Vec::new();
        for input in self.declaration.inputs() {
            let (values, position) = match input.source() {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => {
                    (&mut captures, position)
                }
                RuntimeFunctionInputSource::Parameter { position, .. } => {
                    (&mut arguments, position)
                }
            };
            let value =
                values
                    .get_mut(usize::try_from(position).map_err(|_| {
                        RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site }
                    })?)
                    .and_then(Option::take)
                    .ok_or(RuntimeFunctionApplyError::InvalidBoundArgumentPrefix { site })?;
            let bindings = crate::pattern::match_runtime_pattern_owned(
                self.plan,
                input.pattern(),
                value,
                self.type_instantiation.as_deref(),
            )?
            .ok_or_else(|| {
                crate::value::RuntimeEvalError::PatternMismatch(format!(
                    "function site {site} input {:?}",
                    input.source()
                ))
            })?;
            staged.extend(bindings);
        }
        Ok(staged)
    }
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
        work: &crate::entry::schema::value_budget::ValidationWork,
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
                if expected == binding.context() && binding.is_valid_with_work(self, work) =>
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
        self.validate_function_site_input_refs_with_work(
            site,
            captures,
            arguments,
            &crate::entry::schema::value_budget::ValidationWork::new(
                crate::entry::RuntimeSchemaLimits::engine_default(),
            ),
        )
    }

    /// Continues an enclosing borrowed admission without resetting its work
    /// allowance for effect inference or individual input relations.
    pub(crate) fn validate_function_site_input_refs_with_work(
        &self,
        site: RuntimeFunctionSiteId,
        captures: &[&RuntimeValue],
        arguments: &[&RuntimeValue],
        work: &crate::entry::schema::value_budget::ValidationWork,
    ) -> Result<RuntimeFunctionInputAdmission<'_>, RuntimeFunctionApplyError> {
        let resource_failure = |source| RuntimeFunctionApplyError::InputInstantiation {
            site,
            source: crate::program_types::RuntimeFunctionEffectInstantiationError::Resource(
                Box::new(source),
            ),
        };
        if let Some(source) = work.failure() {
            return Err(resource_failure(source));
        }
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
                    .instantiate_function_effects(context, &references, work)
                    .map(std::sync::Arc::new)
                    .map_err(|source| RuntimeFunctionApplyError::InputInstantiation {
                        site,
                        source,
                    })
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
                Some(binding) => binding.value_matches_with_work(self, expected, value, work),
                None => self.value_matches_type_with_work(expected, value, work)?,
            };
            if let Some(source) = work.failure() {
                return Err(resource_failure(source));
            }
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
            if input.ownership() == RuntimeValueOwnershipRequirement::Unrestricted
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
            plan: self,
            site,
            declaration,
            type_instantiation,
        })
    }
}
