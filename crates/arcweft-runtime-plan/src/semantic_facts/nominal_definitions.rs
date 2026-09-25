//! Complete source type definitions, independent of constructor expression use.

use super::*;

/// The existing normalized record or variant owner for one reachable nominal.
/// Only this inventory emits plan domain rows; expression facts select its
/// coordinates and do not determine whether the definition exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeNominalDefinition {
    Record(RuntimeResolvedNominalRecord),
    Variant(RuntimeVariantOwner),
}

impl RuntimeNominalDefinition {
    pub fn nominal_variant(
        ty: &RuntimeNormalizedType,
        cases: Box<[RuntimeNormalizedVariantCase]>,
    ) -> Result<Self, RuntimeSemanticFactsError> {
        let RuntimeTypeShape::Nominal { nominal, arguments } = ty.shape() else {
            return Err(RuntimeSemanticFactsError::NominalDefinitionMismatch {
                identity: ty.identity(),
            });
        };
        let cases = RuntimeNormalizedVariantCases::try_new(cases).map_err(|error| {
            RuntimeSemanticFactsError::NominalVariantDefinition {
                identity: ty.identity(),
                source: Box::new(error),
            }
        })?;
        Ok(Self::Variant(RuntimeVariantOwner::Nominal {
            nominal: nominal.clone(),
            arguments: arguments.clone(),
            cases,
        }))
    }

    fn identity(&self) -> RuntimeSemanticTypeId {
        match self {
            Self::Record(record) => record.nominal().identity(),
            Self::Variant(owner) => owner.semantic_identity(),
        }
    }

    pub(super) fn append_types<'a>(&'a self, types: &mut Vec<&'a RuntimeNormalizedType>) {
        match self {
            Self::Record(record) => types.extend(
                record
                    .fields()
                    .iter()
                    .map(RuntimeResolvedNominalRecordField::ty),
            ),
            Self::Variant(owner) => owner.append_normalized_types(types),
        }
    }

    /// Closed source enums can be reached only through a selected case. In
    /// that position no expression type row retains the owner, but the checked
    /// variant owner still carries its exact nominal identity and layout.
    pub(super) fn closed_owner_type_seed(&self) -> Option<RuntimePlanTypeSeed> {
        let Self::Variant(
            RuntimeVariantOwner::CharacterNominal {
                identity,
                nominal,
                layout,
                ..
            }
            | RuntimeVariantOwner::BuiltinClosed {
                identity,
                nominal,
                layout,
                ..
            },
        ) = self
        else {
            return None;
        };
        Some(RuntimePlanTypeSeed::new(
            *identity,
            RuntimePlanTypeProjection::Nominal {
                nominal: nominal.clone(),
                layout: *layout,
                arguments: Box::new([]),
            },
        ))
    }

    pub(super) fn record_seed(&self) -> Option<RuntimeNominalRecordDomainSeed> {
        let Self::Record(record) = self else {
            return None;
        };
        Some(RuntimeNominalRecordDomainSeed::new(
            record.nominal().identity(),
            record.layout().shape(),
            record
                .fields()
                .iter()
                .zip(record.layout().fields())
                .map(|(field, accepted)| {
                    RuntimeNominalRecordDomainFieldSeed::new(
                        accepted.field(),
                        accepted.name().map(str::to_owned),
                        field.ty().identity(),
                    )
                }),
        ))
    }

    pub(super) fn variant_seed(&self) -> Option<RuntimeVariantDomainSeed> {
        let Self::Variant(owner) = self else {
            return None;
        };
        owner.runtime_plan_domain_seed()
    }
}

impl RuntimePlanSemanticFacts {
    pub(crate) fn rust_field_default_programs(
        &self,
    ) -> impl Iterator<Item = &arcweft_lang_sema::callable::CheckedRustFieldDefaultProgram> {
        self.nominal_definitions
            .values()
            .filter_map(|definition| {
                let nominal = match definition {
                    RuntimeNominalDefinition::Record(record) => record.nominal(),
                    RuntimeNominalDefinition::Variant(RuntimeVariantOwner::Nominal {
                        nominal,
                        ..
                    }) => nominal,
                    _ => return None,
                };
                match nominal.source() {
                    RuntimeResolvedNominalSource::AcceptedRust(projection) => {
                        Some(projection.default_programs())
                    }
                    _ => None,
                }
            })
            .flatten()
    }

    /// Completes the owned source facts before compiler publication. Every
    /// reachable source nominal is projected once, including roots used only
    /// by Entry roles and types reached solely through fields or case payloads.
    /// Failure returns no partially completed inventory.
    pub fn try_with_nominal_definitions<E>(
        mut self,
        mut project: impl FnMut(&RuntimeNormalizedType) -> Result<RuntimeNominalDefinition, E>,
    ) -> Result<Self, E>
    where
        E: From<RuntimeSemanticFactsError>,
    {
        let mut pending = self
            .all_normalized_type_roots()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        let mut definitions = BTreeMap::new();
        let mut work = 0_u64;
        while let Some(ty) = pending.pop() {
            work = work
                .checked_add(1)
                .ok_or(RuntimeSemanticFactsError::NominalDefinitionBudget)?;
            if work > u64::from(RuntimeSchemaLimits::engine_default().max_nodes) {
                return Err(RuntimeSemanticFactsError::NominalDefinitionBudget.into());
            }
            pending.extend(ty.children().into_iter().cloned());
            if !matches!(ty.shape(), RuntimeTypeShape::Nominal { .. })
                || !seen.insert(ty.identity())
            {
                continue;
            }
            let definition = project(&ty)?;
            if definition.identity() != ty.identity() {
                return Err(RuntimeSemanticFactsError::NominalDefinitionMismatch {
                    identity: ty.identity(),
                }
                .into());
            }
            let mut children = Vec::new();
            definition.append_types(&mut children);
            pending.extend(children.into_iter().cloned());
            definitions.insert(ty.identity(), definition);
        }
        let mut conflict = None;
        self.visit_variant_owners(&mut |owner| {
            if matches!(
                owner,
                RuntimeVariantOwner::CharacterNominal { .. }
                    | RuntimeVariantOwner::BuiltinClosed { .. }
            ) {
                let identity = owner.semantic_identity();
                let definition = RuntimeNominalDefinition::Variant(owner.clone());
                match definitions.entry(identity) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(definition);
                    }
                    std::collections::btree_map::Entry::Occupied(entry) => {
                        let previous = entry.get();
                        if previous != &definition
                            && previous.variant_seed() != definition.variant_seed()
                        {
                            conflict = Some(RuntimeSemanticFactsError::NominalDefinitionMismatch {
                                identity,
                            });
                        }
                    }
                }
            }
        });
        if let Some(error) = conflict {
            return Err(error.into());
        }
        self.nominal_definitions = definitions;
        Ok(self)
    }
}
