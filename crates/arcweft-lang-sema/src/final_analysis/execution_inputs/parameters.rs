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

/// Stable identity of the whole accepted formal role, including wildcard and
/// destructured parameters. Leaf binding identities remain separate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedExecutionParameterIdentity([u8; 32]);

impl CheckedExecutionParameterIdentity {
    fn from_coordinate(
        coordinate: &super::CheckedExecutionCoordinate,
        origin: &CheckedExecutionParameterOrigin,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.lang.checked-execution-parameter.v1\0");
        match coordinate {
            super::CheckedExecutionCoordinate::DeclarationBody(body)
            | super::CheckedExecutionCoordinate::DeclarationMutationBody(body) => {
                let root = body.path().root();
                hash.update(&[0, root.tag()]);
                hash.update(root.as_bytes());
            }
            super::CheckedExecutionCoordinate::CallableBody(path)
            | super::CheckedExecutionCoordinate::MutationBody(path) => {
                hash.update(&[1]);
                hash.update(
                    &path
                        .canonical_bytes()
                        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
                );
            }
            _ => return Err(FinalSemanticAnalysisError::WrongPayloadFamily),
        }
        match origin {
            CheckedExecutionParameterOrigin::Declaration(position) => {
                hash.update(&[0]);
                hash.update(
                    &u32::try_from(position.group().get())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                        .to_le_bytes(),
                );
                hash.update(
                    &u32::try_from(position.parameter().get())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                        .to_le_bytes(),
                );
            }
            CheckedExecutionParameterOrigin::Closure { parameter } => {
                hash.update(&[1]);
                hash.update(&parameter.to_le_bytes());
            }
            CheckedExecutionParameterOrigin::Implicit(_) => {
                hash.update(&[2]);
            }
            CheckedExecutionParameterOrigin::AttachedContent => {
                hash.update(&[3]);
            }
        }
        Ok(Self(*hash.finalize().as_bytes()))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExecutionParameter {
    origin: CheckedExecutionParameterOrigin,
    identity: CheckedExecutionParameterIdentity,
    ty: TypeKind,
    passing: arcweft_core::plan::RuntimeFunctionParameterPassing,
    pattern: Option<PatternId>,
    bindings: Box<[LocalId]>,
}

impl CheckedExecutionParameter {
    fn new(
        origin: CheckedExecutionParameterOrigin,
        ty: TypeKind,
        passing: arcweft_core::plan::RuntimeFunctionParameterPassing,
        pattern: Option<PatternId>,
        bindings: Box<[LocalId]>,
        coordinate: &super::CheckedExecutionCoordinate,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let identity = CheckedExecutionParameterIdentity::from_coordinate(coordinate, &origin)?;
        Ok(Self {
            origin,
            identity,
            ty,
            passing,
            pattern,
            bindings,
        })
    }

    pub const fn identity(&self) -> CheckedExecutionParameterIdentity {
        self.identity
    }
    pub const fn origin(&self) -> &CheckedExecutionParameterOrigin {
        &self.origin
    }
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
    pub const fn passing(&self) -> arcweft_core::plan::RuntimeFunctionParameterPassing {
        self.passing
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
        coordinate: &super::CheckedExecutionCoordinate,
    ) -> Result<Box<[CheckedExecutionParameter]>, super::super::CheckedExecutionContextError> {
        let analysis = self.analysis();
        let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis);
        let mut parameters = Vec::new();
        match source {
            CheckedExecutionSource::EvaluateValue(_)
            | CheckedExecutionSource::SelectMatch(_)
            | CheckedExecutionSource::ExportIteration(_)
            | CheckedExecutionSource::ExportBinding(_) => {}
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                ..
            })
            | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::Declaration {
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
                        let ty = parameter
                            .passing()
                            .value_binding_type(self.instantiate_type(ty)?)
                            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                        if !ty.accepts(&self.instantiate_type(pattern_ty)?) {
                            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                        }
                        let bindings = self.ordered_parameter_bindings(
                            formal_bindings.remove(&key).unwrap_or_default(),
                            &coordinates,
                        )?;
                        parameters.push(self.execution_parameter(
                            CheckedExecutionParameterOrigin::Declaration(position),
                            ty,
                            Some(pattern),
                            bindings,
                            coordinate,
                        )?);
                    }
                }
                if !patterns.is_empty() || !formal_bindings.is_empty() {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
                }
                if let Some(attached) = checked.attached_content() {
                    parameters.push(self.execution_parameter(
                        CheckedExecutionParameterOrigin::AttachedContent,
                        self.instantiate_type(attached.abi_type())?,
                        None,
                        Box::new([attached.binding()]),
                        coordinate,
                    )?);
                }
            }
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(owner))
            | CheckedExecutionSource::ExportMutation(CheckedExecutionBodyOwner::CallableValue(
                owner,
            )) => {
                let expression = analysis
                    .expression(*owner)
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                match expression.resolution() {
                    CheckedExpressionResolution::ImplicitCallable(callable) => {
                        parameters.push(self.execution_parameter(
                            CheckedExecutionParameterOrigin::Implicit(callable.identity()),
                            self.instantiate_type(callable.parameter())?,
                            None,
                            Box::new([]),
                            coordinate,
                        )?);
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
                            parameters.push(self.execution_parameter(
                                CheckedExecutionParameterOrigin::Closure {
                                    parameter: parameter_index,
                                },
                                ty,
                                Some(pattern),
                                bindings,
                                coordinate,
                            )?);
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

    fn execution_parameter(
        &self,
        origin: CheckedExecutionParameterOrigin,
        ty: TypeKind,
        pattern: Option<PatternId>,
        bindings: Box<[LocalId]>,
        coordinate: &super::CheckedExecutionCoordinate,
    ) -> Result<CheckedExecutionParameter, super::super::CheckedExecutionContextError> {
        use super::super::CheckedTypeCopyCapability;
        use arcweft_core::plan::RuntimeFunctionParameterPassing;
        let passing = match &ty {
            TypeKind::BorrowRef {
                kind: arcweft_lang_syntax::reference::BorrowKind::Shared,
                ..
            }
            | TypeKind::Shared(_) => RuntimeFunctionParameterPassing::Shared,
            _ => match self
                .analysis()
                .type_copy_capability(&ty, &self.environment().type_scope())?
            {
                CheckedTypeCopyCapability::Unrestricted => RuntimeFunctionParameterPassing::Value,
                CheckedTypeCopyCapability::ValueDependent
                | CheckedTypeCopyCapability::Unavailable => RuntimeFunctionParameterPassing::Affine,
            },
        };
        Ok(CheckedExecutionParameter::new(
            origin, ty, passing, pattern, bindings, coordinate,
        )?)
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
