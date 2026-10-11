//! Checked declaration selection over one compiler-owned analysis lease.
//!
//! Human-readable names are selectors and result metadata. The selected
//! declaration, checked input ABI and compiled plan retain execution authority.

use super::ProjectAnalysisLease;
use crate::lower::{CompiledDeterministicProgram, ProjectInstantiationControl};
use arcweft_lang_hir::project::HirDeclarationBodyRootRole;
use arcweft_lang_sema::{
    callable::{CallableParameterGroup, CheckedCallableLookupError},
    final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource, CheckedItemRole},
};

/// A checked source function and the compiler's original immutable program.
/// Labels are projected from that same checked callable signature.
#[derive(Clone, Debug)]
pub struct CompiledDeterministicFunction {
    name: String,
    input_labels: Box<[String]>,
    program: CompiledDeterministicProgram,
}

impl CompiledDeterministicFunction {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn input_labels(&self) -> &[String] {
        &self.input_labels
    }
    pub const fn program(&self) -> &CompiledDeterministicProgram {
        &self.program
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeterministicFunctionCompileError {
    #[error("no accepted deterministic function matched selector {name:?}")]
    Missing { name: Option<String> },
    #[error("multiple accepted deterministic functions matched selector {name:?}")]
    Ambiguous { name: Option<String> },
    #[error(transparent)]
    Hir(#[from] arcweft_lang_hir::project::HirProjectAnalysisError),
    #[error(transparent)]
    Context(#[from] Box<arcweft_lang_sema::final_analysis::CheckedExecutionContextError>),
    #[error(transparent)]
    Admission(#[from] arcweft_lang_sema::final_analysis::CheckedProgramAdmissionError),
    #[error("selected function differs from its exact checked callable catalog: {cause:?}")]
    Callable { cause: CheckedCallableLookupError },
    #[error(transparent)]
    Compile(#[from] crate::lower::DeterministicProgramCompileError),
}

pub(super) fn compile(
    lease: &ProjectAnalysisLease,
    name: Option<&str>,
    control: &ProjectInstantiationControl,
) -> Result<CompiledDeterministicFunction, DeterministicFunctionCompileError> {
    let analysis = lease.final_analysis();
    let hir = lease.hir_project().analysis_view()?;
    let mut selected = None;
    for body in analysis
        .hir_topology()
        .modules()
        .iter()
        .flat_map(arcweft_lang_hir::project::HirModuleEvaluationTopology::entries)
        .filter_map(|entry| entry.body())
    {
        let declaration = body.declaration();
        if !matches!(
            analysis
                .item(body.source_item())
                .map(arcweft_lang_sema::final_analysis::CheckedItem::role),
            Some(CheckedItemRole::Function { .. })
        ) || !body
            .roots()
            .iter()
            .any(|root| root.role() == HirDeclarationBodyRootRole::FunctionBody)
            || name.is_some_and(|name| name != declaration.name())
        {
            continue;
        }
        let source = CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
            declaration: declaration.clone(),
            role: HirDeclarationBodyRootRole::FunctionBody,
        });
        let context = analysis
            .checked_execution_context(hir, lease.project_symbols(), source.clone(), None)
            .map_err(Box::new)?;
        if let Err(error) = context.checked_deterministic_program(source.clone()) {
            if name.is_some() {
                return Err(error.into());
            }
            continue;
        }
        let callable = analysis
            .checked_callables()
            .project_callable(declaration)
            .map_err(|cause| DeterministicFunctionCompileError::Callable { cause })?;
        let labels = callable
            .signature()
            .groups()
            .iter()
            .flat_map(CallableParameterGroup::parameters)
            .enumerate()
            .map(|(index, parameter)| {
                parameter
                    .name()
                    .map_or_else(|| format!("arg{index}"), |name| name.as_str().to_owned())
            })
            .collect::<Box<[_]>>();
        if selected.is_some() {
            return Err(DeterministicFunctionCompileError::Ambiguous {
                name: name.map(str::to_owned),
            });
        }
        selected = Some((declaration.name().to_owned(), source, labels));
    }
    let (name, source, input_labels) =
        selected.ok_or_else(|| DeterministicFunctionCompileError::Missing {
            name: name.map(str::to_owned),
        })?;
    let program = lease.compile_deterministic_program(source, None, control)?;
    Ok(CompiledDeterministicFunction {
        name,
        input_labels,
        program,
    })
}
