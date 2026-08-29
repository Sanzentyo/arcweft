//! Exact owner joins, open domains, and sequence partition planning.

use std::collections::BTreeSet;

use arcweft_lang_hir::pattern::{HirPatternKind, HirPatternSequenceRest};

use super::super::match_transaction::CoverageTranscriptHasher;
use super::super::transcript_writer::TranscriptHasher;
use super::super::{CheckedCoverageDomainDigest, CheckedPatternResolution, CheckedVariantOwner};
use super::{
    CheckedMatchBuildError, CheckedMatchLimitKind, CoverageConstructor, CoverageConstructorId,
    CoverageTypeDomain, MatchCoverageAnalyzer, SequencePartition, checked_len,
};
use crate::{
    env::nominal::AcceptedEnvironmentRecordSemantics,
    semantic_coordinate::StableSemanticCoordinate,
    types::{
        AcceptedVariantCaseSemanticId, MatchDomainFamily, MatchDomainInvalidity,
        SemanticTypeDigest, TypeKind, VariantPayloadShape, VariantPayloadType,
    },
};

struct SequencePartitionPlan {
    exact: BTreeSet<u64>,
    rest_prefixes: Vec<u64>,
    cut_points: BTreeSet<u64>,
}

impl SequencePartitionPlan {
    fn new(analyzer: &mut MatchCoverageAnalyzer<'_, '_>) -> Result<Self, CheckedMatchBuildError> {
        let mut plan = Self {
            exact: BTreeSet::new(),
            rest_prefixes: Vec::new(),
            cut_points: BTreeSet::new(),
        };
        plan.insert_cut_point(analyzer, 0)?;
        Ok(plan)
    }

    fn insert_point(
        analyzer: &mut MatchCoverageAnalyzer<'_, '_>,
        points: &mut BTreeSet<u64>,
        point: u64,
    ) -> Result<(), CheckedMatchBuildError> {
        if points.contains(&point) {
            return Ok(());
        }
        analyzer
            .budget
            .charge(CheckedMatchLimitKind::SequencePartitions, 1)?;
        points.insert(point);
        Ok(())
    }

    fn insert_cut_point(
        &mut self,
        analyzer: &mut MatchCoverageAnalyzer<'_, '_>,
        point: u64,
    ) -> Result<(), CheckedMatchBuildError> {
        let mut points = std::mem::take(&mut self.cut_points);
        let result = Self::insert_point(analyzer, &mut points, point);
        self.cut_points = points;
        result
    }

    fn insert_exact(
        &mut self,
        analyzer: &mut MatchCoverageAnalyzer<'_, '_>,
        point: u64,
    ) -> Result<(), CheckedMatchBuildError> {
        let mut points = std::mem::take(&mut self.exact);
        let result = Self::insert_point(analyzer, &mut points, point);
        self.exact = points;
        result
    }

    fn observe_rest(&mut self, minimum: u64) -> Result<(), CheckedMatchBuildError> {
        self.rest_prefixes.try_reserve_exact(1).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::SequencePartitions,
            }
        })?;
        self.rest_prefixes.push(minimum);
        Ok(())
    }

    fn push_partition(
        analyzer: &mut MatchCoverageAnalyzer<'_, '_>,
        partitions: &mut Vec<SequencePartition>,
        partition: SequencePartition,
    ) -> Result<(), CheckedMatchBuildError> {
        analyzer
            .budget
            .charge(CheckedMatchLimitKind::SequencePartitions, 1)?;
        partitions.try_reserve_exact(1).map_err(|_| {
            CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::SequencePartitions,
            }
        })?;
        partitions.push(partition);
        Ok(())
    }

    fn materialize(
        self,
        analyzer: &mut MatchCoverageAnalyzer<'_, '_>,
        owner: SemanticTypeDigest,
        item: &TypeKind,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        let points = self.cut_points.iter().copied().collect::<Vec<_>>();
        let mut partitions = Vec::new();
        for window in points.windows(2) {
            let lower = window[0];
            let upper = window[1];
            if self.exact.contains(&lower) {
                Self::push_partition(analyzer, &mut partitions, SequencePartition::Exact(lower))?;
            } else if lower < upper {
                Self::push_partition(
                    analyzer,
                    &mut partitions,
                    SequencePartition::Interval {
                        lower,
                        upper_exclusive: Some(upper),
                    },
                )?;
            }
        }
        let last = points.last().copied().unwrap_or(0);
        if self.exact.contains(&last) {
            Self::push_partition(analyzer, &mut partitions, SequencePartition::Exact(last))?;
            let lower = last
                .checked_add(1)
                .ok_or(CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::SequencePartitions,
                })?;
            Self::push_partition(
                analyzer,
                &mut partitions,
                SequencePartition::Interval {
                    lower,
                    upper_exclusive: None,
                },
            )?;
        } else {
            Self::push_partition(
                analyzer,
                &mut partitions,
                SequencePartition::Interval {
                    lower: last,
                    upper_exclusive: None,
                },
            )?;
        }
        partitions
            .into_iter()
            .map(|partition| {
                let arity = match partition {
                    SequencePartition::Exact(length) => length,
                    SequencePartition::Interval { lower, .. } => self
                        .rest_prefixes
                        .iter()
                        .copied()
                        .filter(|minimum| *minimum <= lower)
                        .max()
                        .unwrap_or(0),
                };
                analyzer
                    .budget
                    .charge(CheckedMatchLimitKind::PatternNodes, arity)?;
                let arity = usize::try_from(arity).map_err(|_| {
                    CheckedMatchBuildError::ArithmeticOverflow {
                        kind: CheckedMatchLimitKind::PatternNodes,
                    }
                })?;
                Ok(CoverageConstructor {
                    identity: CoverageConstructorId::Sequence { owner, partition },
                    field_types: vec![item.clone(); arity].into_boxed_slice(),
                    variant_payload: None,
                })
            })
            .collect()
    }
}

impl MatchCoverageAnalyzer<'_, '_> {
    pub(super) fn domain(
        &mut self,
        ty: &TypeKind,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<CoverageTypeDomain, CheckedMatchBuildError> {
        self.poll()?;
        let owner = ty.semantic_identity_digest();
        #[cfg(test)]
        if let Some(domain) = self.domain_overrides.get(&owner) {
            return Ok(domain.clone());
        }
        let family = ty
            .match_domain_family()
            .map_err(|invalidity| match invalidity {
                MatchDomainInvalidity::Poison => CheckedMatchBuildError::PoisonedSemanticNode {
                    coordinate: coordinate.clone(),
                },
                MatchDomainInvalidity::Unsupported => {
                    CheckedMatchBuildError::UnsupportedDomain { type_digest: owner }
                }
            })?;
        if let Some(variant) = self.analysis.semantic_shapes().closed_variant(owner) {
            return Self::closed_variant_domain(variant, coordinate);
        }
        if let Some(record) = self.analysis.semantic_shapes().environment_record(owner) {
            return Self::environment_record_domain(owner, record, coordinate);
        }
        self.domain_for_family(ty, owner, family, coordinate)
    }

    fn closed_variant_domain(
        owner: &CheckedVariantOwner,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<CoverageTypeDomain, CheckedMatchBuildError> {
        Ok(CoverageTypeDomain::Constructors(
            Self::variant_constructors(owner, coordinate)?.into_boxed_slice(),
        ))
    }

    fn environment_record_domain(
        owner: SemanticTypeDigest,
        record: &AcceptedEnvironmentRecordSemantics,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<CoverageTypeDomain, CheckedMatchBuildError> {
        let mut fields = Vec::new();
        for (index, field) in record.fields().iter().enumerate() {
            let ordinal =
                u32::try_from(index).map_err(|_| CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::PatternNodes,
                })?;
            if field.ordinal() != ordinal
                || field.type_digest() != field.ty().semantic_identity_digest()
                || field.semantic_id()
                    != crate::env::nominal::AcceptedEnvironmentFieldSemanticId::issue(
                        owner,
                        ordinal,
                        field.type_digest(),
                    )
            {
                return Err(CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: coordinate.clone(),
                });
            }
            fields.push(field.ty().clone());
        }
        Ok(CoverageTypeDomain::Constructors(
            vec![CoverageConstructor {
                identity: CoverageConstructorId::Record { owner },
                field_types: fields.into_boxed_slice(),
                variant_payload: None,
            }]
            .into_boxed_slice(),
        ))
    }

    fn domain_for_family(
        &mut self,
        ty: &TypeKind,
        owner: SemanticTypeDigest,
        family: MatchDomainFamily,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<CoverageTypeDomain, CheckedMatchBuildError> {
        let constructors = match family {
            MatchDomainFamily::Empty => return Ok(CoverageTypeDomain::Empty),
            MatchDomainFamily::Unit => {
                vec![CoverageConstructor::nullary(CoverageConstructorId::Unit)]
            }
            MatchDomainFamily::Bool => vec![
                CoverageConstructor::nullary(CoverageConstructorId::Bool(false)),
                CoverageConstructor::nullary(CoverageConstructorId::Bool(true)),
            ],
            MatchDomainFamily::Option(item) => {
                let checked = CheckedVariantOwner::option(item.clone());
                Self::variant_constructors(&checked, coordinate)?
            }
            MatchDomainFamily::Result { ok, error } => {
                let checked = CheckedVariantOwner::result(ok.clone(), error.clone());
                Self::variant_constructors(&checked, coordinate)?
            }
            MatchDomainFamily::ProjectNominal => {
                self.project_nominal_domain(ty, owner, coordinate)?
            }
            MatchDomainFamily::Tuple(fields) => vec![CoverageConstructor {
                identity: CoverageConstructorId::Tuple { owner },
                field_types: fields.to_vec().into_boxed_slice(),
                variant_payload: None,
            }],
            MatchDomainFamily::VariantPayload(payload) => {
                Self::variant_payload_domain(owner, payload, coordinate)?
            }
            MatchDomainFamily::Array { item, length } => self.array_domain(owner, item, length)?,
            MatchDomainFamily::SymbolicSequence(item) => {
                return self.symbolic_sequence_domain(owner, item, coordinate);
            }
            MatchDomainFamily::Choice(alternatives) => {
                let constructors = self.choice_domain(owner, alternatives, coordinate)?;
                if constructors.is_empty() {
                    return Ok(CoverageTypeDomain::Empty);
                }
                constructors
            }
            MatchDomainFamily::RequiresClosedVariant => {
                return Err(CheckedMatchBuildError::MissingExactOwner {
                    coordinate: coordinate.clone(),
                });
            }
            MatchDomainFamily::ClosedOpaqueAtomic => Self::closed_opaque_atomic_constructors(ty),
            MatchDomainFamily::OpenOrOpaque => self.open_constructors(ty)?,
        };
        Ok(CoverageTypeDomain::Constructors(
            constructors.into_boxed_slice(),
        ))
    }

    fn project_nominal_domain(
        &self,
        ty: &TypeKind,
        owner: SemanticTypeDigest,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        let definition = self
            .analysis
            .project_nominal_semantic(owner)
            .ok_or_else(|| CheckedMatchBuildError::MissingExactOwner {
                coordinate: coordinate.clone(),
            })?;
        if definition.nominal().ty() != *ty {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: coordinate.clone(),
            });
        }
        if let Some(cases) = definition.cases() {
            cases
                .iter()
                .map(|case| {
                    let payload = case.payload().clone();
                    if case.semantic_id()
                        != AcceptedVariantCaseSemanticId::issue(
                            crate::types::VariantPayloadOwnerFamily::Project,
                            owner,
                            case.ordinal(),
                            &payload,
                        )
                    {
                        return Err(CheckedMatchBuildError::InvalidCheckedRow {
                            coordinate: coordinate.clone(),
                        });
                    }
                    Ok(CoverageConstructor {
                        identity: CoverageConstructorId::Variant {
                            owner,
                            case: case.semantic_id(),
                            ordinal: case.ordinal(),
                        },
                        field_types: variant_payload_field_types(&payload),
                        variant_payload: Some(payload),
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        } else if let Some(fields) = definition.fields() {
            for field in fields {
                if field.semantic_id()
                    != crate::record_field::AcceptedRecordFieldSemanticId::issue(
                        owner,
                        field.declaration_ordinal(),
                        field.ty().semantic_identity_digest(),
                    )
                {
                    return Err(CheckedMatchBuildError::InvalidCheckedRow {
                        coordinate: coordinate.clone(),
                    });
                }
            }
            Ok(vec![CoverageConstructor {
                identity: CoverageConstructorId::Record { owner },
                field_types: fields
                    .iter()
                    .map(|field| field.ty().clone())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                variant_payload: None,
            }])
        } else {
            Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: coordinate.clone(),
            })
        }
    }

    fn variant_payload_domain(
        owner: SemanticTypeDigest,
        payload: &crate::types::VariantPayloadType,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        if VariantPayloadType::try_new(
            payload.owner_family(),
            payload.owner_type(),
            payload.case_ordinal(),
            payload.case(),
            payload.shape().clone(),
        )
        .is_err()
        {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: coordinate.clone(),
            });
        }
        let identity = match payload.shape() {
            VariantPayloadShape::Tuple(_) => CoverageConstructorId::Tuple { owner },
            VariantPayloadShape::Record(_) => CoverageConstructorId::Record { owner },
            VariantPayloadShape::Unit => {
                return Err(CheckedMatchBuildError::InvalidCheckedRow {
                    coordinate: coordinate.clone(),
                });
            }
        };
        Ok(vec![CoverageConstructor {
            identity,
            field_types: variant_payload_field_types(payload.shape()),
            variant_payload: None,
        }])
    }

    fn array_domain(
        &mut self,
        owner: SemanticTypeDigest,
        item: &TypeKind,
        length: usize,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        let length =
            u64::try_from(length).map_err(|_| CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::PatternNodes,
            })?;
        self.budget
            .charge(CheckedMatchLimitKind::PatternNodes, length)?;
        let count =
            usize::try_from(length).map_err(|_| CheckedMatchBuildError::ArithmeticOverflow {
                kind: CheckedMatchLimitKind::PatternNodes,
            })?;
        Ok(vec![CoverageConstructor {
            identity: CoverageConstructorId::Array { owner, length },
            field_types: vec![item.clone(); count].into_boxed_slice(),
            variant_payload: None,
        }])
    }

    fn symbolic_sequence_domain(
        &mut self,
        owner: SemanticTypeDigest,
        item: &TypeKind,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<CoverageTypeDomain, CheckedMatchBuildError> {
        if let Some(cached) = self.sequence_domains.get(&owner) {
            return Ok(CoverageTypeDomain::Constructors(cached.clone()));
        }
        let constructors = self.sequence_constructors(owner, item, coordinate)?;
        self.sequence_domains
            .insert(owner, constructors.clone().into_boxed_slice());
        Ok(CoverageTypeDomain::Constructors(
            constructors.into_boxed_slice(),
        ))
    }

    fn choice_domain(
        &mut self,
        owner: SemanticTypeDigest,
        alternatives: &[TypeKind],
        coordinate: &StableSemanticCoordinate,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        let mut constructors = Vec::new();
        for (ordinal, alternative) in alternatives.iter().enumerate() {
            if matches!(
                self.domain(alternative, coordinate)?,
                CoverageTypeDomain::Empty
            ) {
                continue;
            }
            let ordinal =
                u32::try_from(ordinal).map_err(|_| CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::PatternNodes,
                })?;
            constructors.push(CoverageConstructor {
                identity: CoverageConstructorId::Choice {
                    owner,
                    ordinal,
                    alternative: alternative.semantic_identity_digest(),
                },
                field_types: vec![alternative.clone()].into_boxed_slice(),
                variant_payload: None,
            });
        }
        Ok(constructors)
    }

    fn variant_constructors(
        owner: &CheckedVariantOwner,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        if !owner.has_valid_case_rows() {
            return Err(CheckedMatchBuildError::InvalidCheckedRow {
                coordinate: coordinate.clone(),
            });
        }
        let semantic_type = owner.semantic_type();
        owner
            .cases()
            .iter()
            .map(|case| {
                let payload = case.payload().clone();
                Ok(CoverageConstructor {
                    identity: CoverageConstructorId::Variant {
                        owner: semantic_type,
                        case: case.semantic_id(),
                        ordinal: case.ordinal(),
                    },
                    field_types: variant_payload_field_types(&payload),
                    variant_payload: Some(payload),
                })
            })
            .collect()
    }

    fn open_constructors(
        &mut self,
        ty: &TypeKind,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        let semantic_type = ty.semantic_identity_digest();
        let mut literals = BTreeSet::new();
        let mut entities = BTreeSet::new();
        for index in 0..self.observed_patterns.len() {
            let (owner, coordinate) = self.observed_patterns[index].clone();
            let checked = self.analysis.pattern(owner).ok_or_else(|| {
                CheckedMatchBuildError::MissingExactOwner {
                    coordinate: coordinate.clone(),
                }
            })?;
            if checked.ty().semantic_identity_digest() != semantic_type {
                continue;
            }
            match checked.resolution() {
                CheckedPatternResolution::Literal(literal) => {
                    literals.insert(self.canonical_literal(
                        owner,
                        literal,
                        checked.ty(),
                        &coordinate,
                    )?);
                }
                CheckedPatternResolution::Entity(item) => {
                    entities.insert(item.semantic_id());
                }
                CheckedPatternResolution::Structural
                | CheckedPatternResolution::Record(_)
                | CheckedPatternResolution::Variant(_)
                | CheckedPatternResolution::TypedBinding(_) => {}
            }
        }
        let mut constructors = literals
            .into_iter()
            .map(|literal| CoverageConstructor::nullary(CoverageConstructorId::Literal(literal)))
            .chain(entities.into_iter().map(|item| {
                CoverageConstructor::nullary(CoverageConstructorId::Entity {
                    owner: semantic_type,
                    item,
                })
            }))
            .collect::<Vec<_>>();
        constructors.push(CoverageConstructor::nullary(CoverageConstructorId::Other(
            semantic_type,
        )));
        Ok(constructors)
    }

    fn closed_opaque_atomic_constructors(ty: &TypeKind) -> Vec<CoverageConstructor> {
        vec![CoverageConstructor::nullary(CoverageConstructorId::Other(
            ty.semantic_identity_digest(),
        ))]
    }

    fn sequence_constructors(
        &mut self,
        owner: SemanticTypeDigest,
        item: &TypeKind,
        _domain_coordinate: &StableSemanticCoordinate,
    ) -> Result<Vec<CoverageConstructor>, CheckedMatchBuildError> {
        let mut plan = SequencePartitionPlan::new(self)?;
        for index in 0..self.observed_patterns.len() {
            self.poll()?;
            let (pattern_id, coordinate) = self.observed_patterns[index].clone();
            let checked = self.analysis.pattern(pattern_id).ok_or_else(|| {
                CheckedMatchBuildError::MissingExactOwner {
                    coordinate: coordinate.clone(),
                }
            })?;
            if checked.ty().semantic_identity_digest() != owner {
                continue;
            }
            let hir = self.module.resolve_pattern(pattern_id).map_err(|_| {
                CheckedMatchBuildError::MissingExactOwner {
                    coordinate: coordinate.clone(),
                }
            })?;
            let HirPatternKind::BracketSequence { elements, rest } = hir.kind() else {
                continue;
            };
            let length = checked_len(elements.len(), CheckedMatchLimitKind::PatternNodes)?;
            plan.insert_cut_point(self, length)?;
            match rest {
                HirPatternSequenceRest::Absent => {
                    plan.insert_exact(self, length)?;
                    let successor = length.checked_add(1).ok_or(
                        CheckedMatchBuildError::ArithmeticOverflow {
                            kind: CheckedMatchLimitKind::SequencePartitions,
                        },
                    )?;
                    plan.insert_cut_point(self, successor)?;
                }
                HirPatternSequenceRest::Unbound | HirPatternSequenceRest::Bound(_) => {
                    plan.observe_rest(length)?;
                }
                HirPatternSequenceRest::Recovered(_) => {
                    return Err(CheckedMatchBuildError::PoisonedSemanticNode { coordinate });
                }
            }
        }
        plan.materialize(self, owner, item)
    }

    pub(super) fn domain_digest(
        &mut self,
        ty: &TypeKind,
        domain: &CoverageTypeDomain,
    ) -> Result<CheckedCoverageDomainDigest, CheckedMatchBuildError> {
        let mut hasher = TranscriptHasher::new(self.budget);
        hasher.update(b"arcweft.lang.checked-match-coverage.v1\0")?;
        hasher.update(ty.semantic_identity_digest().as_bytes())?;
        match domain {
            CoverageTypeDomain::Empty => {
                hasher.update(&[0])?;
            }
            CoverageTypeDomain::Constructors(constructors) => {
                hasher.update(&[1])?;
                hasher.update(
                    &checked_len(constructors.len(), CheckedMatchLimitKind::MatrixRows)?
                        .to_le_bytes(),
                )?;
                for constructor in constructors {
                    write_constructor_identity(&mut hasher, &constructor.identity)?;
                    if let Some(payload) = constructor.variant_payload.as_ref() {
                        write_variant_payload_identity(&mut hasher, payload)?;
                    }
                    hasher.update(
                        &checked_len(
                            constructor.field_types.len(),
                            CheckedMatchLimitKind::PatternNodes,
                        )?
                        .to_le_bytes(),
                    )?;
                    for field in &constructor.field_types {
                        hasher.update(field.semantic_identity_digest().as_bytes())?;
                    }
                }
            }
        }
        Ok(CheckedCoverageDomainDigest::from_bytes(hasher.finalize()))
    }
}

pub(super) fn variant_payload_field_types(payload: &VariantPayloadShape) -> Box<[TypeKind]> {
    match payload {
        VariantPayloadShape::Unit => Box::new([]),
        VariantPayloadShape::Tuple(fields) => fields
            .iter()
            .map(|field| field.ty().clone())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        VariantPayloadShape::Record(fields) => fields
            .iter()
            .map(|field| field.ty().clone())
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}

fn write_variant_payload_identity(
    hasher: &mut CoverageTranscriptHasher<'_>,
    payload: &VariantPayloadShape,
) -> Result<(), CheckedMatchBuildError> {
    hasher.update(&[payload.semantic_shape_tag()])?;
    hasher.update(
        &checked_len(payload.field_count(), CheckedMatchLimitKind::PatternNodes)?.to_le_bytes(),
    )?;
    match payload {
        VariantPayloadShape::Unit => {}
        VariantPayloadShape::Tuple(fields) => {
            for field in fields {
                hasher.update(&field.ordinal().to_le_bytes())?;
                hasher.update(field.semantic_id().as_bytes())?;
                hasher.update(field.ty().semantic_identity_digest().as_bytes())?;
            }
        }
        VariantPayloadShape::Record(fields) => {
            for field in fields {
                hasher.update(&field.ordinal().to_le_bytes())?;
                hasher.update(field.semantic_id().as_bytes())?;
                hasher.update(field.ty().semantic_identity_digest().as_bytes())?;
            }
        }
    }
    Ok(())
}

fn write_constructor_identity(
    hasher: &mut CoverageTranscriptHasher<'_>,
    value: &CoverageConstructorId,
) -> Result<(), CheckedMatchBuildError> {
    match value {
        CoverageConstructorId::Unit => {
            hasher.update(&[0])?;
        }
        CoverageConstructorId::Bool(value) => {
            hasher.update(&[1, u8::from(*value)])?;
        }
        CoverageConstructorId::Variant {
            owner,
            case,
            ordinal,
        } => {
            hasher.update(&[2])?;
            hasher.update(owner.as_bytes())?;
            hasher.update(case.as_bytes())?;
            hasher.update(&ordinal.to_le_bytes())?;
        }
        CoverageConstructorId::Tuple { owner } => {
            hasher.update(&[3])?;
            hasher.update(owner.as_bytes())?;
        }
        CoverageConstructorId::Record { owner } => {
            hasher.update(&[4])?;
            hasher.update(owner.as_bytes())?;
        }
        CoverageConstructorId::Array { owner, length } => {
            hasher.update(&[5])?;
            hasher.update(owner.as_bytes())?;
            hasher.update(&length.to_le_bytes())?;
        }
        CoverageConstructorId::Sequence { owner, partition } => {
            hasher.update(&[6])?;
            hasher.update(owner.as_bytes())?;
            match partition {
                SequencePartition::Exact(value) => {
                    hasher.update(&[0])?;
                    hasher.update(&value.to_le_bytes())?;
                }
                SequencePartition::Interval {
                    lower,
                    upper_exclusive,
                } => {
                    hasher.update(&[1])?;
                    hasher.update(&lower.to_le_bytes())?;
                    match upper_exclusive {
                        Some(upper) => {
                            hasher.update(&[1])?;
                            hasher.update(&upper.to_le_bytes())?;
                        }
                        None => {
                            hasher.update(&[0])?;
                        }
                    }
                }
            }
        }
        CoverageConstructorId::Choice {
            owner,
            ordinal,
            alternative,
        } => {
            hasher.update(&[7])?;
            hasher.update(owner.as_bytes())?;
            hasher.update(&ordinal.to_le_bytes())?;
            hasher.update(alternative.as_bytes())?;
        }
        CoverageConstructorId::Literal(literal) => {
            hasher.update(&[8])?;
            hasher.update(literal.semantic_type.as_bytes())?;
            let length = u64::try_from(literal.bytes.len()).map_err(|_| {
                CheckedMatchBuildError::ArithmeticOverflow {
                    kind: CheckedMatchLimitKind::TranscriptBytes,
                }
            })?;
            hasher.update(&length.to_le_bytes())?;
            hasher.update(&literal.bytes)?;
        }
        CoverageConstructorId::Entity { owner, item } => {
            hasher.update(&[9])?;
            hasher.update(owner.as_bytes())?;
            hasher.update(item.as_bytes())?;
        }
        CoverageConstructorId::Other(owner) => {
            hasher.update(&[10])?;
            hasher.update(owner.as_bytes())?;
        }
    }
    Ok(())
}
