//! Plan-owned nominal-record construction domains.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::RuntimePlanTypeId;

/// One semantic field supplied by the accepted external lowerer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordDomainFieldSeed {
    name: String,
    ty: RuntimeSemanticTypeId,
}

impl RuntimeNominalRecordDomainFieldSeed {
    #[must_use]
    pub fn new(name: impl Into<String>, ty: RuntimeSemanticTypeId) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimeSemanticTypeId {
        self.ty
    }
}

/// Inert semantic seed for one nominal-record domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordDomainSeed {
    owner: RuntimeSemanticTypeId,
    fields: Box<[RuntimeNominalRecordDomainFieldSeed]>,
}

impl RuntimeNominalRecordDomainSeed {
    #[must_use]
    pub fn new(
        owner: RuntimeSemanticTypeId,
        fields: impl IntoIterator<Item = RuntimeNominalRecordDomainFieldSeed>,
    ) -> Self {
        Self {
            owner,
            fields: fields.into_iter().collect::<Vec<_>>().into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn owner(&self) -> RuntimeSemanticTypeId {
        self.owner
    }

    #[must_use]
    pub const fn fields(&self) -> &[RuntimeNominalRecordDomainFieldSeed] {
        &self.fields
    }
}

/// One defining-order field whose type is owned by the same plan type table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordDomainField {
    name: String,
    ty: RuntimePlanTypeId,
}

impl RuntimeNominalRecordDomainField {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimePlanTypeId {
        self.ty
    }
}

/// Final plan-owned record domain. The owner and every field reference the
/// sole plan-local type graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordDomain {
    owner: RuntimePlanTypeId,
    fields: Box<[RuntimeNominalRecordDomainField]>,
}

impl RuntimeNominalRecordDomain {
    pub(crate) fn from_admitted_parts(
        owner: RuntimePlanTypeId,
        fields: impl IntoIterator<Item = (String, RuntimePlanTypeId)>,
    ) -> Self {
        Self {
            owner,
            fields: fields
                .into_iter()
                .map(|(name, ty)| RuntimeNominalRecordDomainField { name, ty })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn owner(&self) -> RuntimePlanTypeId {
        self.owner
    }

    #[must_use]
    pub const fn fields(&self) -> &[RuntimeNominalRecordDomainField] {
        &self.fields
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordDomainTable {
    domains: BTreeMap<RuntimePlanTypeId, RuntimeNominalRecordDomain>,
}

impl RuntimeNominalRecordDomainTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.domains.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }

    #[must_use]
    pub fn get(&self, owner: RuntimePlanTypeId) -> Option<&RuntimeNominalRecordDomain> {
        self.domains.get(&owner)
    }

    pub fn domains(&self) -> impl ExactSizeIterator<Item = &RuntimeNominalRecordDomain> {
        self.domains.values()
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeNominalRecordDomainTableBuilder {
    domains: BTreeMap<RuntimePlanTypeId, RuntimeNominalRecordDomain>,
}

pub(crate) struct PreparedRuntimeNominalRecordDomainBatch {
    candidate: BTreeMap<RuntimePlanTypeId, RuntimeNominalRecordDomain>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordDomainError {
    #[error("nominal-record owner type {owner} has conflicting field domains")]
    ConflictingDomain { owner: RuntimePlanTypeId },
    #[error("nominal-record owner type {owner} contains duplicate field `{name}`")]
    DuplicateFieldName {
        owner: RuntimePlanTypeId,
        name: String,
    },
    #[error("nominal-record owner type {owner} contains an empty field name")]
    EmptyFieldName { owner: RuntimePlanTypeId },
    #[error("nominal-record owner type {owner} has too many fields: {actual}")]
    TooManyFields {
        owner: RuntimePlanTypeId,
        actual: usize,
    },
}

impl RuntimeNominalRecordDomainTableBuilder {
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            domains: BTreeMap::new(),
        }
    }

    pub(crate) fn contains_owner(&self, owner: RuntimePlanTypeId) -> bool {
        self.domains.contains_key(&owner)
    }

    #[must_use]
    pub(crate) fn get(&self, owner: RuntimePlanTypeId) -> Option<&RuntimeNominalRecordDomain> {
        self.domains.get(&owner)
    }

    pub(crate) fn prepare_batch(
        &self,
        domains: impl IntoIterator<Item = RuntimeNominalRecordDomain>,
    ) -> Result<PreparedRuntimeNominalRecordDomainBatch, RuntimeNominalRecordDomainError> {
        let domains = domains.into_iter().collect::<Box<[_]>>();
        let mut unique = Vec::<RuntimeNominalRecordDomain>::new();
        let mut unique_by_owner = BTreeMap::<RuntimePlanTypeId, usize>::new();
        for domain in &domains {
            validate_domain(domain)?;
            if let Some(index) = unique_by_owner.get(&domain.owner).copied() {
                if unique[index] != *domain {
                    return Err(RuntimeNominalRecordDomainError::ConflictingDomain {
                        owner: domain.owner,
                    });
                }
            } else {
                unique_by_owner.insert(domain.owner, unique.len());
                unique.push(domain.clone());
            }
        }

        let mut candidate = self.domains.clone();
        for domain in unique {
            if let Some(existing) = self.domains.get(&domain.owner) {
                if existing != &domain {
                    return Err(RuntimeNominalRecordDomainError::ConflictingDomain {
                        owner: domain.owner,
                    });
                }
                continue;
            }
            candidate.insert(domain.owner, domain);
        }
        Ok(PreparedRuntimeNominalRecordDomainBatch { candidate })
    }

    pub(crate) fn commit_batch(&mut self, prepared: PreparedRuntimeNominalRecordDomainBatch) {
        self.domains = prepared.candidate;
    }

    #[must_use]
    pub(crate) fn finish(self) -> RuntimeNominalRecordDomainTable {
        RuntimeNominalRecordDomainTable {
            domains: self.domains,
        }
    }
}

fn validate_domain(
    domain: &RuntimeNominalRecordDomain,
) -> Result<(), RuntimeNominalRecordDomainError> {
    if domain.fields.len() > u32::MAX as usize {
        return Err(RuntimeNominalRecordDomainError::TooManyFields {
            owner: domain.owner,
            actual: domain.fields.len(),
        });
    }
    let mut names = std::collections::BTreeSet::new();
    for field in &domain.fields {
        if field.name.is_empty() {
            return Err(RuntimeNominalRecordDomainError::EmptyFieldName {
                owner: domain.owner,
            });
        }
        if !names.insert(field.name.as_str()) {
            return Err(RuntimeNominalRecordDomainError::DuplicateFieldName {
                owner: domain.owner,
                name: field.name.clone(),
            });
        }
    }
    Ok(())
}
