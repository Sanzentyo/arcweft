//! Full formal layout authenticated against one closed execution context.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::{LocalId, PatternId},
    project::{
        HirBindingSite, HirDeclarationParameterRootChild, HirDeclarationParameterRootRole,
        HirExpressionBindingRole,
    },
};

use super::{
    CheckedExecutionBodyOwner, CheckedExecutionSource, CheckedExpressionResolution,
    FinalSemanticAnalysisError,
};
use crate::{semantic_coordinate::SemanticCoordinateIndex, types::TypeKind};

/// Formal positions retain complete arity, including unused and wildcard
/// parameters. Attached content and synthetic parameters have distinct roles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExecutionParameterOrigin {
    Declaration(crate::callable::CallableParameterCoordinate),
    Closure { parameter: u32 },
    Implicit(super::super::CheckedImplicitCallableIdentity),
    AttachedContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutionParameter {
    origin: CheckedExecutionParameterOrigin,
    ty: TypeKind,
    pattern: Option<PatternId>,
    bindings: Box<[LocalId]>,
}

impl CheckedExecutionParameter {
    pub const fn origin(&self) -> &CheckedExecutionParameterOrigin {
        &self.origin
    }
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
    pub const fn pattern(&self) -> Option<PatternId> {
        self.pattern
    }
    /// These are report-bound lookup handles. Stable input binding origins,
    /// rather than arena IDs, carry their semantic identity.
    pub const fn bindings(&self) -> &[LocalId] {
        &self.bindings
    }
}

impl super::super::CheckedClosedExecutionContext<'_> {
    pub(super) fn execution_parameters(
        &self,
        source: &CheckedExecutionSource,
    ) -> Result<Box<[CheckedExecutionParameter]>, super::super::CheckedExecutionContextError> {
        let analysis = self.analysis();
        let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis);
        let mut parameters = Vec::new();
        match source {
            CheckedExecutionSource::EvaluateValue(_) => {}
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                ..
            }) => {
                let declaration_view = analysis
                    .hir_topology()
                    .declaration(declaration)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let checked = analysis
                    .checked_callables()
                    .project_callable(declaration)
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let mut patterns = BTreeMap::new();
                for root in declaration_view.body().parameter_roots() {
                    if let HirDeclarationParameterRootRole::Pattern { group, parameter } =
                        root.role()
                    {
                        let HirDeclarationParameterRootChild::Pattern(pattern) = root.child()
                        else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        };
                        let key = (
                            usize::try_from(group)
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                            usize::try_from(parameter)
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                        );
                        if patterns.insert(key, pattern).is_some() {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        }
                    }
                }
                let mut formal_bindings = BTreeMap::<(usize, usize), Vec<LocalId>>::new();
                for binding in declaration_view.module().local_origins().rows() {
                    if let HirBindingSite::DeclarationParameter {
                        item,
                        owner,
                        group,
                        parameter,
                    } = binding.site()
                        && item == declaration_view.body().source_item()
                        && owner == declaration_view.body().source_owner()
                    {
                        let key = (
                            usize::try_from(group)
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                            usize::try_from(parameter)
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                        );
                        formal_bindings
                            .entry(key)
                            .or_default()
                            .push(binding.local());
                    }
                }
                for group in checked.signature().groups() {
                    for parameter in group.parameters() {
                        let position = crate::callable::CallableParameterCoordinate::new(
                            group.index(),
                            parameter.index(),
                        );
                        let key = (position.group().get(), position.parameter().get());
                        let pattern = patterns
                            .remove(&key)
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        let ty = parameter
                            .declared_type()
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        let pattern_ty = analysis
                            .pattern(pattern)
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                            .ty();
                        let ty = self.instantiate_type(ty)?;
                        if !ty.accepts(&self.instantiate_type(pattern_ty)?) {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        }
                        let bindings = self.ordered_parameter_bindings(
                            formal_bindings.remove(&key).unwrap_or_default(),
                            &coordinates,
                        )?;
                        parameters.push(CheckedExecutionParameter {
                            origin: CheckedExecutionParameterOrigin::Declaration(position),
                            ty,
                            pattern: Some(pattern),
                            bindings,
                        });
                    }
                }
                if !patterns.is_empty() || !formal_bindings.is_empty() {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                }
                if let Some(attached) = checked.attached_content() {
                    parameters.push(CheckedExecutionParameter {
                        origin: CheckedExecutionParameterOrigin::AttachedContent,
                        ty: self.instantiate_type(attached.abi_type())?,
                        pattern: None,
                        bindings: Box::new([attached.binding()]),
                    });
                }
            }
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner)) => {
                let expression = analysis
                    .expression(*owner)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                match expression.resolution() {
                    CheckedExpressionResolution::ImplicitCallable(callable) => {
                        parameters.push(CheckedExecutionParameter {
                            origin: CheckedExecutionParameterOrigin::Implicit(callable.identity()),
                            ty: self.instantiate_type(callable.parameter())?,
                            pattern: None,
                            bindings: Box::new([]),
                        });
                    }
                    CheckedExpressionResolution::Closure(_) => {
                        let module = self
                            .project()
                            .modules()
                            .find_map(|(_, module)| {
                                (module.module_id() == owner.module()).then_some(module)
                            })
                            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                        let arcweft_lang_hir::expr::HirExprKind::Closure(closure) = module
                            .resolve_expr(*owner)
                            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                            .kind()
                        else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        };
                        let TypeKind::Function { params, .. } = expression
                            .value_type()
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                        else {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        };
                        if params.len() != closure.parameters().len() {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        }
                        let origins = analysis
                            .hir_topology()
                            .module(owner.module())
                            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
                            .local_origins();
                        let mut formal_bindings = BTreeMap::<u32, Vec<LocalId>>::new();
                        for binding in origins.rows() {
                            if let HirBindingSite::Expression {
                                expression,
                                role: HirExpressionBindingRole::ClosureParameter { parameter },
                            } = binding.site()
                                && expression == *owner
                            {
                                formal_bindings
                                    .entry(parameter)
                                    .or_default()
                                    .push(binding.local());
                            }
                        }
                        for (ordinal, (parameter, ty)) in
                            closure.parameters().iter().zip(params).enumerate()
                        {
                            let parameter_index = u32::try_from(ordinal)
                                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                            let pattern = parameter.pattern();
                            let ty = self.instantiate_type(ty)?;
                            let pattern_ty = analysis
                                .pattern(pattern)
                                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                                .ty();
                            if !ty.accepts(&self.instantiate_type(pattern_ty)?) {
                                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                            }
                            let bindings = self.ordered_parameter_bindings(
                                formal_bindings.remove(&parameter_index).unwrap_or_default(),
                                &coordinates,
                            )?;
                            parameters.push(CheckedExecutionParameter {
                                origin: CheckedExecutionParameterOrigin::Closure {
                                    parameter: parameter_index,
                                },
                                ty,
                                pattern: Some(pattern),
                                bindings,
                            });
                        }
                        if !formal_bindings.is_empty() {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        }
                    }
                    _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into()),
                }
            }
        }
        Ok(parameters.into_boxed_slice())
    }

    fn ordered_parameter_bindings(
        &self,
        locals: Vec<LocalId>,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<Box<[LocalId]>, FinalSemanticAnalysisError> {
        let mut bindings = locals
            .into_iter()
            .map(|local| {
                self.analysis()
                    .local(local)
                    .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
                coordinates
                    .binding(local)
                    .map(|origin| (origin, local))
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)
            })
            .collect::<Result<Vec<_>, _>>()?;
        bindings.sort_by(|left, right| left.0.cmp(&right.0));
        if bindings.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(bindings.into_iter().map(|(_, local)| local).collect())
    }
}
