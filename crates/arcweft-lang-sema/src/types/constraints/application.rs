//! Application scope membership owned by one constraint frontier path.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use super::context::{
    TypeConstraintConstEligibility, TypeConstraintEffectScope, TypeConstraintParameterEligibility,
    TypeConstraintParameterScope,
};
use super::{ConstraintDomain, TypeConstraintInvariant, TypeConstraintParameterScopeInvariant};
use crate::types::generics::GenericApplicationIssuer;
use crate::{
    effect_row::EffectConstraintEligibility,
    types::{GenericConstReference, GenericEffectReference, GenericTypeReference},
};

/// Private owner contract retained by an imported scope lease. Only a real
/// admitted application scope implements this trait.
trait ImportedApplicationScopeAuthority {
    fn id(&self) -> ConstraintApplicationId;
    fn admits(&self, parameter: &super::ConstraintGenericParameterId) -> bool;
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
}

impl Clone for ImportedGenericParameterScopeLease {
    fn clone(&self) -> Self {
        Self {
            applications: self.applications.clone(),
            parameters: self.parameters.clone(),
        }
    }
}

impl PartialEq for ImportedGenericParameterScopeLease {
    fn eq(&self, other: &Self) -> bool {
        self.parameters == other.parameters
            && self.applications.len() == other.applications.len()
            && self
                .applications
                .iter()
                .zip(&other.applications)
                .all(|(left, right)| left.id() == right.id() && left.parameters == right.parameters)
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
    pub(crate) fn parameters(&self) -> &[super::ConstraintGenericParameterId] {
        &self.parameters
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
}

impl<D: ConstraintDomain> ConstraintApplicationScope<D> {
    pub(super) fn new(
        application: D::Application,
        parameters: TypeConstraintParameterScope,
    ) -> Self {
        Self {
            application,
            parameters,
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
}

/// Scope membership follows a path when alternatives fork. Work accounting
/// is shared by the run and deliberately does not live in this inventory.
pub(super) struct ConstraintApplicationScopes<D: ConstraintDomain> {
    root: ConstraintApplicationId,
    applications: BTreeMap<ConstraintApplicationId, Arc<ConstraintApplicationScope<D>>>,
    imported: Box<[ImportedApplicationScopeLease]>,
}

impl<D: ConstraintDomain> Clone for ConstraintApplicationScopes<D> {
    fn clone(&self) -> Self {
        Self {
            root: self.root,
            applications: self.applications.clone(),
            imported: self.imported.clone(),
        }
    }
}

impl<D: ConstraintDomain> ConstraintApplicationScopes<D> {
    pub(super) fn root(
        scope: ConstraintApplicationScope<D>,
        imported: Option<ImportedGenericParameterScopeLease>,
    ) -> Self {
        let root = scope.id();
        let imported = imported.map_or_else(Vec::new, |lease| lease.applications.into_vec());
        Self {
            root,
            applications: BTreeMap::from([(root, Arc::new(scope))]),
            imported: imported.into_boxed_slice(),
        }
    }

    pub(super) const fn root_id(&self) -> ConstraintApplicationId {
        self.root
    }

    pub(super) fn len(&self) -> usize {
        self.applications.len().saturating_add(self.imported.len())
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

    pub(super) fn root_scope(&self) -> &ConstraintApplicationScope<D> {
        self.application(self.root)
            .expect("the application inventory owns its root")
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
                    self.imported
                        .iter()
                        .any(|owner| {
                            owner.contains(&super::ConstraintGenericParameterId::Type(
                                reference.clone(),
                            ))
                        })
                        .then_some(TypeConstraintParameterEligibility::Rigid)
                }),
            // Free references keep their declaration identity across captures.
            // Inference variables resolve through their exact application or
            // an imported owner lease retained by this path.
            GenericTypeReference::Free(_) => (self.applications().any(|application| {
                application.parameters().eligibility(reference)
                    == Some(TypeConstraintParameterEligibility::Rigid)
            }) || self.imported.iter().any(|owner| {
                owner.contains(&super::ConstraintGenericParameterId::Type(
                    reference.clone(),
                ))
            }))
            .then_some(TypeConstraintParameterEligibility::Rigid),
            GenericTypeReference::Bound(_) => None,
        }
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
        for parameter in parameters {
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
                    self.imported
                        .iter()
                        .any(|owner| {
                            owner.contains(&super::ConstraintGenericParameterId::Const(
                                reference.clone(),
                            ))
                        })
                        .then_some(TypeConstraintConstEligibility::Rigid)
                }),
            GenericConstReference::Free(_) => (self.applications().any(|application| {
                application.parameters().const_eligibility(reference)
                    == Some(TypeConstraintConstEligibility::Rigid)
            }) || self.imported.iter().any(|owner| {
                owner.contains(&super::ConstraintGenericParameterId::Const(
                    reference.clone(),
                ))
            }))
            .then_some(TypeConstraintConstEligibility::Rigid),
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
                    self.imported
                        .iter()
                        .any(|owner| {
                            owner.contains(&super::ConstraintGenericParameterId::Effect(
                                reference.clone(),
                            ))
                        })
                        .then_some(EffectConstraintEligibility::Rigid)
                }),
            GenericEffectReference::Free(_) => (self.applications().any(|application| {
                application.parameters().effect_eligibility(reference)
                    == Some(EffectConstraintEligibility::Rigid)
            }) || self.imported.iter().any(|owner| {
                owner.contains(&super::ConstraintGenericParameterId::Effect(
                    reference.clone(),
                ))
            }))
            .then_some(EffectConstraintEligibility::Rigid),
            GenericEffectReference::Bound(_) => None,
        }
    }

    pub(super) fn applications(&self) -> impl Iterator<Item = &ConstraintApplicationScope<D>> {
        self.applications.values().map(Arc::as_ref)
    }
}

#[cfg(test)]
mod tests;
