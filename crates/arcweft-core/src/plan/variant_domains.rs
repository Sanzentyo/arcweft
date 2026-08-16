//! Plan-owned nominal variant case domains.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::entry::RuntimeNominalTypeId;
use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::RuntimePlanTypeId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVariantCaseSeed {
    name: String,
    payload: Option<RuntimeSemanticTypeId>,
}

impl RuntimeVariantCaseSeed {
    #[must_use]
    pub fn new(name: impl Into<String>, payload: Option<RuntimeSemanticTypeId>) -> Self {
        Self {
            name: name.into(),
            payload,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn payload(&self) -> Option<RuntimeSemanticTypeId> {
        self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVariantDomainSeed {
    owner: RuntimeSemanticTypeId,
    nominal: RuntimeNominalTypeId,
    cases: Box<[RuntimeVariantCaseSeed]>,
}

impl RuntimeVariantDomainSeed {
    #[must_use]
    pub fn new(
        owner: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: impl IntoIterator<Item = RuntimeVariantCaseSeed>,
    ) -> Self {
        Self {
            owner,
            nominal,
            cases: cases.into_iter().collect::<Vec<_>>().into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn owner(&self) -> RuntimeSemanticTypeId {
        self.owner
    }

    #[must_use]
    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.nominal
    }

    #[must_use]
    pub const fn cases(&self) -> &[RuntimeVariantCaseSeed] {
        &self.cases
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVariantCase {
    name: String,
    payload: Option<RuntimePlanTypeId>,
}

impl RuntimeVariantCase {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn payload(&self) -> Option<RuntimePlanTypeId> {
        self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVariantDomain {
    owner: RuntimePlanTypeId,
    nominal: RuntimeNominalTypeId,
    cases: Box<[RuntimeVariantCase]>,
}

impl RuntimeVariantDomain {
    pub(crate) fn from_admitted_parts(
        owner: RuntimePlanTypeId,
        nominal: RuntimeNominalTypeId,
        cases: impl IntoIterator<Item = (String, Option<RuntimePlanTypeId>)>,
    ) -> Self {
        Self {
            owner,
            nominal,
            cases: cases
                .into_iter()
                .map(|(name, payload)| RuntimeVariantCase { name, payload })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn owner(&self) -> RuntimePlanTypeId {
        self.owner
    }

    #[must_use]
    pub const fn nominal(&self) -> &RuntimeNominalTypeId {
        &self.nominal
    }

    #[must_use]
    pub const fn cases(&self) -> &[RuntimeVariantCase] {
        &self.cases
    }

    #[must_use]
    pub fn case(&self, ordinal: u32) -> Option<&RuntimeVariantCase> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|index| self.cases.get(index))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVariantDomainTable {
    domains: BTreeMap<RuntimePlanTypeId, RuntimeVariantDomain>,
}

impl RuntimeVariantDomainTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.domains.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }

    #[must_use]
    pub fn get(&self, owner: RuntimePlanTypeId) -> Option<&RuntimeVariantDomain> {
        self.domains.get(&owner)
    }

    pub fn domains(&self) -> impl ExactSizeIterator<Item = &RuntimeVariantDomain> {
        self.domains.values()
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeVariantDomainTableBuilder {
    domains: BTreeMap<RuntimePlanTypeId, RuntimeVariantDomain>,
}

pub(crate) struct PreparedRuntimeVariantDomainBatch {
    candidate: BTreeMap<RuntimePlanTypeId, RuntimeVariantDomain>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeVariantDomainError {
    #[error("variant owner type {owner} has conflicting case domains")]
    ConflictingDomain { owner: RuntimePlanTypeId },
    #[error("variant owner type {owner} contains duplicate case `{name}`")]
    DuplicateCaseName {
        owner: RuntimePlanTypeId,
        name: String,
    },
    #[error("variant owner type {owner} contains an empty case name")]
    EmptyCaseName { owner: RuntimePlanTypeId },
    #[error("variant owner type {owner} has no cases")]
    EmptyDomain { owner: RuntimePlanTypeId },
    #[error("variant owner type {owner} has too many cases: {actual}")]
    TooManyCases {
        owner: RuntimePlanTypeId,
        actual: usize,
    },
}

impl RuntimeVariantDomainTableBuilder {
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
    pub(crate) fn get(&self, owner: RuntimePlanTypeId) -> Option<&RuntimeVariantDomain> {
        self.domains.get(&owner)
    }

    pub(crate) fn prepare_batch(
        &self,
        domains: impl IntoIterator<Item = RuntimeVariantDomain>,
    ) -> Result<PreparedRuntimeVariantDomainBatch, RuntimeVariantDomainError> {
        let domains = domains.into_iter().collect::<Box<[_]>>();
        let mut unique = Vec::<RuntimeVariantDomain>::new();
        let mut unique_by_owner = BTreeMap::<RuntimePlanTypeId, usize>::new();
        for domain in &domains {
            validate_domain(domain)?;
            if let Some(index) = unique_by_owner.get(&domain.owner).copied() {
                if unique[index] != *domain {
                    return Err(RuntimeVariantDomainError::ConflictingDomain {
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
                    return Err(RuntimeVariantDomainError::ConflictingDomain {
                        owner: domain.owner,
                    });
                }
                continue;
            }
            candidate.insert(domain.owner, domain);
        }
        Ok(PreparedRuntimeVariantDomainBatch { candidate })
    }

    pub(crate) fn commit_batch(&mut self, prepared: PreparedRuntimeVariantDomainBatch) {
        self.domains = prepared.candidate;
    }

    #[must_use]
    pub(crate) fn finish(self) -> RuntimeVariantDomainTable {
        RuntimeVariantDomainTable {
            domains: self.domains,
        }
    }
}

fn validate_domain(domain: &RuntimeVariantDomain) -> Result<(), RuntimeVariantDomainError> {
    if domain.cases.is_empty() {
        return Err(RuntimeVariantDomainError::EmptyDomain {
            owner: domain.owner,
        });
    }
    if domain.cases.len() > u32::MAX as usize {
        return Err(RuntimeVariantDomainError::TooManyCases {
            owner: domain.owner,
            actual: domain.cases.len(),
        });
    }
    let mut names = std::collections::BTreeSet::new();
    for case in &domain.cases {
        if case.name.is_empty() {
            return Err(RuntimeVariantDomainError::EmptyCaseName {
                owner: domain.owner,
            });
        }
        if !names.insert(case.name.as_str()) {
            return Err(RuntimeVariantDomainError::DuplicateCaseName {
                owner: domain.owner,
                name: case.name.clone(),
            });
        }
    }
    Ok(())
}
