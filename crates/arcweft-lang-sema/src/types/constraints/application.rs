//! Application scope membership owned by one constraint frontier path.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use super::context::{
    TypeConstraintConstEligibility, TypeConstraintEffectScope, TypeConstraintParameterEligibility,
    TypeConstraintParameterScope,
};
use super::lexical::ConstraintLexicalScope;
use super::{ConstraintDomain, TypeConstraintInvariant, TypeConstraintParameterScopeInvariant};
use crate::types::generics::GenericApplicationIssuer;
use crate::{
    effect_row::EffectConstraintEligibility,
    types::{GenericConstReference, GenericEffectReference, GenericTypeReference},
};

mod captures;
pub(super) use captures::RetainedRigidGenericScope;

/// Private owner contract retained by an imported scope lease. Only a real
/// admitted application scope implements this trait.
trait ImportedApplicationScopeAuthority {
    fn id(&self) -> ConstraintApplicationId;
    fn as_any(&self) -> &dyn std::any::Any;
    fn same_owner(&self, other: &dyn ImportedApplicationScopeAuthority) -> bool;
    fn introduction_scope(&self) -> &ConstraintLexicalScope;
    fn admits(&self, parameter: &super::ConstraintGenericParameterId) -> bool;
    fn permits_local_inference(&self, parameter: &super::ConstraintGenericParameterId) -> bool;
}

/// One exact application owner and the parameters that owner admitted.
#[derive(Clone)]
struct ImportedApplicationScopeLease {
    owner: Arc<dyn ImportedApplicationScopeAuthority>,
    parameters: Box<[super::ConstraintGenericParameterId]>,
}

impl ImportedApplicationScopeLease {
    fn contains(&self, parameter: &super::ConstraintGenericParameterId) -> bool {
        self.parameters.binary_search(parameter).is_ok() && self.owner.admits(parameter)
    }

    fn id(&self) -> ConstraintApplicationId {
        self.owner.id()
    }

    fn narrowed(&self, parameter: &super::ConstraintGenericParameterId) -> Option<Self> {
        self.contains(parameter).then(|| Self {
            owner: Arc::clone(&self.owner),
            parameters: Box::new([parameter.clone()]),
        })
    }
}

/// Exact parent applications authorized to lend unresolved generic parameters
/// to a nested source callback. Owners are opaque and retain their admitted
/// scopes across child constraint transactions.
pub(crate) struct ImportedGenericParameterScopeLease {
    applications: Box<[ImportedApplicationScopeLease]>,
    parameters: Box<[super::ConstraintGenericParameterId]>,
    captures: RetainedRigidGenericScope,
}

impl Clone for ImportedGenericParameterScopeLease {
    fn clone(&self) -> Self {
        Self {
            applications: self.applications.clone(),
            parameters: self.parameters.clone(),
            captures: self.captures.clone(),
        }
    }
}

impl PartialEq for ImportedGenericParameterScopeLease {
    fn eq(&self, other: &Self) -> bool {
        self.parameters == other.parameters
            && self.captures == other.captures
            && self.applications.len() == other.applications.len()
            && self
                .applications
                .iter()
                .zip(&other.applications)
                .all(|(left, right)| {
                    left.id() == right.id()
                        && left.parameters == right.parameters
                        && left.owner.same_owner(right.owner.as_ref())
                })
    }
}

impl Eq for ImportedGenericParameterScopeLease {}

impl std::fmt::Debug for ImportedGenericParameterScopeLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImportedGenericParameterScopeLease")
            .field(
                "application_ids",
                &self
                    .applications
                    .iter()
                    .map(|owner| owner.id())
                    .collect::<Vec<_>>(),
            )
            .field("parameters", &self.parameters)
            .finish()
    }
}

impl ImportedGenericParameterScopeLease {
    pub(super) fn merge_original_gamma<
        A: super::context::TypeConstraintAccounting,
        D: ConstraintDomain,
    >(
        &self,
        gamma: &mut ConstraintLexicalScope,
        context: &mut super::context::TypeConstraintContext<'_, A, D>,
    ) -> Result<(), super::TypeConstraintError> {
        for application in &self.applications {
            gamma.merge(application.owner.introduction_scope(), context)?;
        }
        Ok(())
    }
    pub(crate) fn parameters(&self) -> &[super::ConstraintGenericParameterId] {
        &self.parameters
    }

    /// A child may solve a currently bindable parent variable in its own
    /// path. The lease never lends the parent's bindings or mutable frontier.
    /// The parent still checks the whole source and replays materialization
    /// with its final substitution before publishing the selected call.
    pub(crate) fn permits_local_inference(
        &self,
        parameter: &super::ConstraintGenericParameterId,
    ) -> bool {
        self.applications.iter().any(|owner| {
            owner.contains(parameter) && owner.owner.permits_local_inference(parameter)
        })
    }
}

/// A generation-local opening, never a stable callable or program identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ConstraintApplicationId(GenericApplicationIssuer);

/// A domain source is local to one application. Keeping its opening here
/// distinguishes repeated argument slots and declaration-authored defaults
/// without requiring the domain to invent globally unique source coordinates.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ConstraintSourceId<S> {
    application: ConstraintApplicationId,
    local: S,
}

impl<S: Copy> ConstraintSourceId<S> {
    pub(crate) const fn new(application: ConstraintApplicationId, local: S) -> Self {
        Self { application, local }
    }

    pub(crate) const fn application(self) -> ConstraintApplicationId {
        self.application
    }

    pub(crate) const fn local(self) -> S {
        self.local
    }
}

/// The parameter contracts of one admitted application stay together.
pub(super) struct ConstraintApplicationScope<D: ConstraintDomain> {
    application: D::Application,
    parameters: TypeConstraintParameterScope,
    introduction: ConstraintLexicalScope,
}

impl<D: ConstraintDomain> ConstraintApplicationScope<D> {
    #[cfg(test)]
    pub(super) fn test_root(
        application: D::Application,
        parameters: TypeConstraintParameterScope,
    ) -> Self {
        Self::new(application, parameters, ConstraintLexicalScope::default())
    }

    pub(super) fn new(
        application: D::Application,
        parameters: TypeConstraintParameterScope,
        introduction: ConstraintLexicalScope,
    ) -> Self {
        Self {
            application,
            parameters,
            introduction,
        }
    }

    pub(super) const fn application(&self) -> D::Application {
        self.application
    }

    pub(super) fn id(&self) -> ConstraintApplicationId {
        ConstraintApplicationId(self.parameters.application_issuer())
    }

    pub(super) const fn parameters(&self) -> &TypeConstraintParameterScope {
        &self.parameters
    }

    pub(super) fn introduction_scope(&self) -> &ConstraintLexicalScope {
        &self.introduction
    }

    pub(super) const fn effects(&self) -> &TypeConstraintEffectScope {
        self.parameters.effect_contract()
    }
}

impl<D: ConstraintDomain + 'static> ImportedApplicationScopeAuthority
    for ConstraintApplicationScope<D>
{
    fn id(&self) -> ConstraintApplicationId {
        ConstraintApplicationScope::id(self)
    }

    fn introduction_scope(&self) -> &ConstraintLexicalScope {
        self.introduction_scope()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn same_owner(&self, other: &dyn ImportedApplicationScopeAuthority) -> bool {
        other.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.application() == other.application()
                && self.parameters() == other.parameters()
                && self.introduction_scope() == other.introduction_scope()
        })
    }

    fn admits(&self, parameter: &super::ConstraintGenericParameterId) -> bool {
        match parameter {
            super::ConstraintGenericParameterId::Type(parameter) => {
                self.parameters.eligibility(parameter).is_some()
            }
            super::ConstraintGenericParameterId::Const(parameter) => {
                self.parameters.const_eligibility(parameter).is_some()
            }
            super::ConstraintGenericParameterId::Effect(parameter) => {
                self.parameters.effect_eligibility(parameter).is_some()
            }
        }
    }

    fn permits_local_inference(&self, parameter: &super::ConstraintGenericParameterId) -> bool {
        match parameter {
            super::ConstraintGenericParameterId::Type(parameter) => {
                self.parameters.eligibility(parameter)
                    == Some(TypeConstraintParameterEligibility::Bindable)
            }
            super::ConstraintGenericParameterId::Const(parameter) => {
                self.parameters.const_eligibility(parameter)
                    == Some(TypeConstraintConstEligibility::Bindable)
            }
            super::ConstraintGenericParameterId::Effect(parameter) => {
                self.parameters.effect_eligibility(parameter)
                    == Some(EffectConstraintEligibility::Bindable)
            }
        }
    }
}

/// Admitted application membership, independent of source-path lineage.
/// A path or a joint original-constraint owner supplies its own roots; this
/// inventory cannot manufacture a parent by selecting one admitted member.
pub(super) struct ConstraintApplicationInventory<D: ConstraintDomain> {
    applications: BTreeMap<ConstraintApplicationId, Arc<ConstraintApplicationScope<D>>>,
    imported: Box<[ImportedApplicationScopeLease]>,
    captures: RetainedRigidGenericScope,
}

impl<D: ConstraintDomain> Clone for ConstraintApplicationInventory<D> {
    fn clone(&self) -> Self {
        Self {
            applications: self.applications.clone(),
            imported: self.imported.clone(),
            captures: self.captures.clone(),
        }
    }
}

impl<D: ConstraintDomain> ConstraintApplicationInventory<D> {
    pub(super) fn empty() -> Self {
        Self {
            applications: BTreeMap::new(),
            imported: Box::new([]),
            captures: RetainedRigidGenericScope::default(),
        }
    }

    /// A product retains independently admitted roots, including legitimate
    /// shared openings. Identity conflicts fail; overlap is never freshened.
    pub(super) fn merge_original<A: super::context::TypeConstraintAccounting>(
        &mut self,
        original: &Self,
        context: &mut super::context::TypeConstraintContext<'_, A, D>,
    ) -> Result<(), super::TypeConstraintError> {
        let conflict = || {
            super::TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                TypeConstraintParameterScopeInvariant::ApplicationContractConflict,
            ))
        };
        for (id, application) in &original.applications {
            context.enter_node()?;
            if let Some(existing) = self.applications.get(id) {
                if !existing.same_owner(application.as_ref()) {
                    return Err(conflict());
                }
            } else {
                for owner in &self.imported {
                    context.enter_node()?;
                    if owner.id() == *id && !owner.owner.same_owner(application.as_ref()) {
                        return Err(conflict());
                    }
                }
                self.applications.insert(*id, Arc::clone(application));
            }
        }
        let mut imported =
            BTreeMap::<ConstraintApplicationId, ImportedApplicationScopeLease>::new();
        for owner in self.imported.iter().chain(original.imported.iter()) {
            context.enter_node()?;
            if let Some(application) = self.applications.get(&owner.id())
                && !owner.owner.same_owner(application.as_ref())
            {
                return Err(conflict());
            }
            if let Some(existing) = imported.get_mut(&owner.id()) {
                if !existing.owner.same_owner(owner.owner.as_ref()) {
                    return Err(conflict());
                }
                let mut parameters = BTreeSet::new();
                for parameter in existing.parameters.iter().chain(owner.parameters.iter()) {
                    context.enter_node()?;
                    parameters.insert(parameter.clone());
                }
                existing.parameters = parameters.into_iter().collect();
            } else {
                for _ in &owner.parameters {
                    context.enter_node()?;
                }
                imported.insert(owner.id(), owner.clone());
            }
        }
        self.imported = imported.into_values().collect();
        self.captures = self.captures.union(&original.captures, context)?;
        Ok(())
    }

    pub(super) fn single(
        scope: ConstraintApplicationScope<D>,
        imported: Option<ImportedGenericParameterScopeLease>,
    ) -> Self {
        let application = scope.id();
        let (imported, captures) = imported.map_or_else(
            || (Vec::new(), RetainedRigidGenericScope::default()),
            |lease| (lease.applications.into_vec(), lease.captures),
        );
        Self {
            applications: BTreeMap::from([(application, Arc::new(scope))]),
            imported: imported.into_boxed_slice(),
            captures,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.applications
            .len()
            .saturating_add(self.imported.len())
            .saturating_add(self.captures.len())
    }

    pub(super) fn validate_admission(
        &self,
        application: &ConstraintApplicationScope<D>,
    ) -> Result<(), TypeConstraintParameterScopeInvariant> {
        if self.applications.contains_key(&application.id())
            || self
                .imported
                .iter()
                .any(|owner| owner.id() == application.id())
            || self
                .applications()
                .any(|owned| owned.application() == application.application())
        {
            return Err(TypeConstraintParameterScopeInvariant::ApplicationAlreadyAdmitted);
        }
        Ok(())
    }

    pub(super) fn admit(
        &mut self,
        application: ConstraintApplicationScope<D>,
    ) -> Result<(), TypeConstraintParameterScopeInvariant> {
        self.validate_admission(&application)?;
        self.applications
            .insert(application.id(), Arc::new(application));
        Ok(())
    }

    pub(super) fn application(
        &self,
        id: ConstraintApplicationId,
    ) -> Option<&ConstraintApplicationScope<D>> {
        self.applications.get(&id).map(Arc::as_ref)
    }

    pub(super) fn require_application(
        &self,
        id: ConstraintApplicationId,
    ) -> Result<&ConstraintApplicationScope<D>, TypeConstraintInvariant> {
        self.application(id)
            .ok_or(TypeConstraintInvariant::ParameterScope(
                TypeConstraintParameterScopeInvariant::ApplicationOutOfScope,
            ))
    }

    pub(super) fn parameter_eligibility(
        &self,
        reference: &GenericTypeReference,
    ) -> Option<TypeConstraintParameterEligibility> {
        match reference {
            GenericTypeReference::Inference(parameter) => self
                .application(ConstraintApplicationId(parameter.issuer()))
                .and_then(|application| application.parameters().eligibility(reference))
                .or_else(|| {
                    self.imported_parameter_binding(&super::ConstraintGenericParameterId::Type(
                        reference.clone(),
                    ))
                    .map(|bindable| {
                        if bindable {
                            TypeConstraintParameterEligibility::Bindable
                        } else {
                            TypeConstraintParameterEligibility::Rigid
                        }
                    })
                }),
            // Free references keep their declaration identity across captures.
            // Inference variables resolve through their exact application or
            // an imported owner lease retained by this path.
            GenericTypeReference::Free(_) => {
                (self
                    .captures
                    .contains(&super::ConstraintGenericParameterId::Type(
                        reference.clone(),
                    ))
                    || self.applications().any(|application| {
                        application.parameters().eligibility(reference)
                            == Some(TypeConstraintParameterEligibility::Rigid)
                    })
                    || self.imported.iter().any(|owner| {
                        owner.contains(&super::ConstraintGenericParameterId::Type(
                            reference.clone(),
                        ))
                    }))
                .then_some(TypeConstraintParameterEligibility::Rigid)
            }
            GenericTypeReference::Bound(_) => None,
        }
    }

    fn imported_parameter_binding(
        &self,
        parameter: &super::ConstraintGenericParameterId,
    ) -> Option<bool> {
        self.imported
            .iter()
            .find(|owner| owner.contains(parameter))
            .map(|owner| owner.owner.permits_local_inference(parameter))
    }

    pub(super) fn import_parameters(
        &self,
        parameters: &[super::ConstraintGenericParameterId],
    ) -> Result<ImportedGenericParameterScopeLease, TypeConstraintInvariant>
    where
        D: 'static,
    {
        if parameters.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err(TypeConstraintInvariant::ParameterScope(
                TypeConstraintParameterScopeInvariant::ParameterUnordered,
            ));
        }
        let mut imported = BTreeMap::<
            ConstraintApplicationId,
            (
                Arc<dyn ImportedApplicationScopeAuthority>,
                BTreeSet<super::ConstraintGenericParameterId>,
            ),
        >::new();
        let mut captures = BTreeSet::new();
        for parameter in parameters {
            if self.captures.contains(parameter) {
                captures.insert(parameter.clone());
                continue;
            }
            let Some(owner) = self.owner_for_parameter(parameter) else {
                return Err(match parameter {
                    super::ConstraintGenericParameterId::Type(reference) => {
                        TypeConstraintInvariant::ParameterScope(
                            TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                                parameter: reference.clone(),
                            },
                        )
                    }
                    super::ConstraintGenericParameterId::Const(reference) => {
                        TypeConstraintInvariant::ParameterScope(
                            TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                                parameter: reference.clone(),
                            },
                        )
                    }
                    super::ConstraintGenericParameterId::Effect(reference) => {
                        TypeConstraintInvariant::Effect(super::TypeConstraintEffectInvariant {
                            kind: super::TypeConstraintEffectInvariantKind::ForeignVariable,
                            variable: Some(reference.clone()),
                        })
                    }
                });
            };
            imported
                .entry(owner.id())
                .or_insert_with(|| (Arc::clone(&owner.owner), BTreeSet::new()))
                .1
                .insert(parameter.clone());
        }
        Ok(ImportedGenericParameterScopeLease {
            applications: imported
                .into_values()
                .map(|(owner, parameters)| ImportedApplicationScopeLease {
                    owner,
                    parameters: parameters.into_iter().collect(),
                })
                .collect(),
            parameters: parameters.to_vec().into_boxed_slice(),
            captures: RetainedRigidGenericScope::from_admitted(captures),
        })
    }

    fn owner_for_parameter(
        &self,
        parameter: &super::ConstraintGenericParameterId,
    ) -> Option<ImportedApplicationScopeLease> {
        self.imported
            .iter()
            .find_map(|owner| owner.narrowed(parameter))
            .or_else(|| match parameter {
                super::ConstraintGenericParameterId::Type(reference) => {
                    self.scope_for_type(reference)
                }
                super::ConstraintGenericParameterId::Const(reference) => {
                    self.scope_for_const(reference)
                }
                super::ConstraintGenericParameterId::Effect(reference) => {
                    self.scope_for_effect(reference)
                }
            })
    }

    /// The exact owner capability survives imports. A receiving path's ambient
    /// scope neither extends nor replaces this application's dependencies.
    pub(super) fn binding_scope(
        &self,
        parameter: &super::ConstraintGenericParameterId,
    ) -> Option<&ConstraintLexicalScope> {
        use super::ConstraintGenericParameterId;
        let issuer = match parameter {
            ConstraintGenericParameterId::Type(GenericTypeReference::Inference(reference)) => {
                Some(reference.issuer())
            }
            ConstraintGenericParameterId::Const(GenericConstReference::Inference(reference)) => {
                Some(reference.issuer())
            }
            ConstraintGenericParameterId::Effect(GenericEffectReference::Inference(reference)) => {
                Some(reference.issuer())
            }
            _ => None,
        };
        let owned = match issuer {
            Some(issuer) => self
                .applications
                .get(&ConstraintApplicationId(issuer))
                .map(Arc::as_ref)
                .filter(|application| application.admits(parameter)),
            None => self
                .applications()
                .find(|application| application.admits(parameter)),
        };
        owned
            .map(ConstraintApplicationScope::introduction_scope)
            .or_else(|| {
                self.imported
                    .iter()
                    .find(|lease| lease.contains(parameter))
                    .map(|lease| lease.owner.introduction_scope())
            })
    }

    pub(super) fn merge_lexical_scopes<A: super::context::TypeConstraintAccounting>(
        &self,
        scope: &mut ConstraintLexicalScope,
        context: &mut super::context::TypeConstraintContext<'_, A, D>,
    ) -> Result<(), super::TypeConstraintError> {
        for application in self.applications() {
            scope.merge(application.introduction_scope(), context)?;
        }
        for imported in &self.imported {
            scope.merge(imported.owner.introduction_scope(), context)?;
        }
        Ok(())
    }

    fn scope_for_type(
        &self,
        reference: &GenericTypeReference,
    ) -> Option<ImportedApplicationScopeLease>
    where
        D: 'static,
    {
        let parameter = super::ConstraintGenericParameterId::Type(reference.clone());
        let scope = match reference {
            GenericTypeReference::Inference(parameter) => self
                .applications
                .get(&ConstraintApplicationId(parameter.issuer()))
                .filter(|application| application.parameters().eligibility(reference).is_some())
                .cloned(),
            GenericTypeReference::Free(_) => self
                .applications()
                .find(|application| {
                    application.parameters().eligibility(reference)
                        == Some(TypeConstraintParameterEligibility::Rigid)
                })
                .map(|application| {
                    Arc::clone(
                        self.applications
                            .get(&application.id())
                            .expect("iterated application remains in its owner table"),
                    )
                }),
            GenericTypeReference::Bound(_) => None,
        }?;
        let owner: Arc<dyn ImportedApplicationScopeAuthority> = scope;
        Some(ImportedApplicationScopeLease {
            owner,
            parameters: Box::new([parameter]),
        })
    }

    fn scope_for_const(
        &self,
        reference: &GenericConstReference,
    ) -> Option<ImportedApplicationScopeLease>
    where
        D: 'static,
    {
        let parameter = super::ConstraintGenericParameterId::Const(reference.clone());
        let scope = match reference {
            GenericConstReference::Inference(parameter) => self
                .applications
                .get(&ConstraintApplicationId(parameter.issuer()))
                .filter(|application| {
                    application
                        .parameters()
                        .const_eligibility(reference)
                        .is_some()
                })
                .cloned(),
            GenericConstReference::Free(_) => self
                .applications()
                .find(|application| {
                    application.parameters().const_eligibility(reference)
                        == Some(TypeConstraintConstEligibility::Rigid)
                })
                .map(|application| {
                    Arc::clone(
                        self.applications
                            .get(&application.id())
                            .expect("iterated application remains in its owner table"),
                    )
                }),
            GenericConstReference::Bound(_) => None,
        }?;
        let owner: Arc<dyn ImportedApplicationScopeAuthority> = scope;
        Some(ImportedApplicationScopeLease {
            owner,
            parameters: Box::new([parameter]),
        })
    }

    fn scope_for_effect(
        &self,
        reference: &GenericEffectReference,
    ) -> Option<ImportedApplicationScopeLease>
    where
        D: 'static,
    {
        let parameter = super::ConstraintGenericParameterId::Effect(reference.clone());
        let scope = match reference {
            GenericEffectReference::Inference(parameter) => self
                .applications
                .get(&ConstraintApplicationId(parameter.issuer()))
                .filter(|application| {
                    application
                        .parameters()
                        .effect_eligibility(reference)
                        .is_some()
                })
                .cloned(),
            GenericEffectReference::Free(_) => self
                .applications()
                .find(|application| {
                    application.parameters().effect_eligibility(reference)
                        == Some(EffectConstraintEligibility::Rigid)
                })
                .map(|application| {
                    Arc::clone(
                        self.applications
                            .get(&application.id())
                            .expect("iterated application remains in its owner table"),
                    )
                }),
            GenericEffectReference::Bound(_) => None,
        }?;
        let owner: Arc<dyn ImportedApplicationScopeAuthority> = scope;
        Some(ImportedApplicationScopeLease {
            owner,
            parameters: Box::new([parameter]),
        })
    }

    pub(super) fn const_parameter_eligibility(
        &self,
        reference: &GenericConstReference,
    ) -> Option<TypeConstraintConstEligibility> {
        match reference {
            GenericConstReference::Inference(parameter) => self
                .application(ConstraintApplicationId(parameter.issuer()))
                .and_then(|application| application.parameters().const_eligibility(reference))
                .or_else(|| {
                    self.imported_parameter_binding(&super::ConstraintGenericParameterId::Const(
                        reference.clone(),
                    ))
                    .map(|bindable| {
                        if bindable {
                            TypeConstraintConstEligibility::Bindable
                        } else {
                            TypeConstraintConstEligibility::Rigid
                        }
                    })
                }),
            GenericConstReference::Free(_) => {
                (self
                    .captures
                    .contains(&super::ConstraintGenericParameterId::Const(
                        reference.clone(),
                    ))
                    || self.applications().any(|application| {
                        application.parameters().const_eligibility(reference)
                            == Some(TypeConstraintConstEligibility::Rigid)
                    })
                    || self.imported.iter().any(|owner| {
                        owner.contains(&super::ConstraintGenericParameterId::Const(
                            reference.clone(),
                        ))
                    }))
                .then_some(TypeConstraintConstEligibility::Rigid)
            }
            GenericConstReference::Bound(_) => None,
        }
    }

    pub(super) fn effect_eligibility(
        &self,
        reference: &GenericEffectReference,
    ) -> Option<EffectConstraintEligibility> {
        match reference {
            GenericEffectReference::Inference(parameter) => self
                .application(ConstraintApplicationId(parameter.issuer()))
                .and_then(|application| application.parameters().effect_eligibility(reference))
                .or_else(|| {
                    self.imported_parameter_binding(&super::ConstraintGenericParameterId::Effect(
                        reference.clone(),
                    ))
                    .map(|bindable| {
                        if bindable {
                            EffectConstraintEligibility::Bindable
                        } else {
                            EffectConstraintEligibility::Rigid
                        }
                    })
                }),
            GenericEffectReference::Free(_) => {
                (self
                    .captures
                    .contains(&super::ConstraintGenericParameterId::Effect(
                        reference.clone(),
                    ))
                    || self.applications().any(|application| {
                        application.parameters().effect_eligibility(reference)
                            == Some(EffectConstraintEligibility::Rigid)
                    })
                    || self.imported.iter().any(|owner| {
                        owner.contains(&super::ConstraintGenericParameterId::Effect(
                            reference.clone(),
                        ))
                    }))
                .then_some(EffectConstraintEligibility::Rigid)
            }
            GenericEffectReference::Bound(_) => None,
        }
    }

    pub(super) fn applications(&self) -> impl Iterator<Item = &ConstraintApplicationScope<D>> {
        self.applications.values().map(Arc::as_ref)
    }
}

#[cfg(test)]
mod tests;
