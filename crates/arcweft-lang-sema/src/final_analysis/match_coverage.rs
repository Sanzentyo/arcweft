//! Private bounded usefulness analysis for ordinary `Match` expressions.
//!
//! This module is the sole coverage authority.  It consumes checked pattern
//! rows and semantic type identities, deconstructs them into a transaction-
//! local pattern matrix, and publishes only the final checked coverage fact.

use std::collections::BTreeMap;

use arcweft_lang_hir::{identity::PatternId, leaf::HirLiteral, module::HirModule};

use super::{CheckedCoverageDomainDigest, FinalSemanticAnalysis, FinalSemanticAnalysisControl};
use crate::{
    semantic_coordinate::{
        CheckedSemanticPath, StablePatternCoordinate, StablePatternCoordinateStep,
        StableSemanticCoordinate,
    },
    types::{SemanticTypeDigest, TypeKind},
};

#[cfg(test)]
use crate::types::AcceptedVariantCaseSemanticId;
#[cfg(test)]
use crate::types::{ArrayLength, VariantPayloadShape};
#[cfg(test)]
use arcweft_lang_hir::pattern::HirPatternKind;

pub(super) use super::canonical_literal::{
    CanonicalCoverageLiteral, CanonicalLiteralEncodingError, encode_canonical_literal,
};
pub use super::match_transaction::CheckedMatchLimitKind;
pub use super::match_transaction::CheckedMatchLimits;
pub(super) use super::match_transaction::{
    CheckedMatchBudget, CheckedMatchBuildError, CheckedMatchWork, checked_depth_successor,
    checked_len,
};

mod deconstruct;
mod domain;
mod matrix;
mod model;

#[cfg(test)]
use self::domain::variant_payload_field_types;
pub(super) use self::model::{
    CheckedCoverageWitness, CheckedSequencePartitionWitness, CheckedVariantCoverageWitness,
    CheckedVariantRecordCoverageWitnessField,
};
use self::model::{
    CoverageConstructor, CoverageConstructorId, CoverageTypeDomain, DeconstructedPattern,
    DeconstructedPatternKind, ExpandedPattern, Matrix, PatternVector, SequencePartition,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedGuardClass {
    Absent,
    ConstantTrue,
    ConstantFalse,
    Dynamic,
}

/// Stable accepted-rooted coordinate of one Match arm.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableMatchArmCoordinate {
    owner: CheckedSemanticPath,
    ordinal: u32,
}

impl StableMatchArmCoordinate {
    pub(crate) const fn new(owner: CheckedSemanticPath, ordinal: u32) -> Self {
        Self { owner, ordinal }
    }

    pub const fn owner(&self) -> &CheckedSemanticPath {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) fn pattern_coordinate(
        &self,
        pattern: StablePatternCoordinate,
    ) -> StableSemanticCoordinate {
        StableSemanticCoordinate::pattern(self.owner.clone(), self.ordinal, pattern)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedUnreachableReason {
    CoveredByPriorUsefulArms,
    CoveredByEarlierOrAlternative,
    ConstantFalseGuard,
    UninhabitedDomain,
}

/// Read-only path step locating an unreachable pattern alternative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedPatternCoordinateStep<'a> {
    TupleElement(u32),
    RecordField {
        semantic_id: &'a [u8; 32],
        source_ordinal: u32,
    },
    SequenceElement(u32),
    VariantPayload,
    WholeBindingInner,
    OrAlternative(u32),
    TypedBindingInner,
}

/// Borrowed view of the exact accepted pattern path used by coverage.
#[derive(Clone, Copy, Debug)]
pub struct CheckedPatternCoordinateView<'a>(&'a StablePatternCoordinate);

impl<'a> CheckedPatternCoordinateView<'a> {
    pub fn steps(&self) -> impl ExactSizeIterator<Item = CheckedPatternCoordinateStep<'a>> + 'a {
        self.0.steps().iter().map(|step| match step {
            StablePatternCoordinateStep::TupleElement(ordinal) => {
                CheckedPatternCoordinateStep::TupleElement(*ordinal)
            }
            StablePatternCoordinateStep::RecordField {
                field,
                source_ordinal,
            } => CheckedPatternCoordinateStep::RecordField {
                semantic_id: field.as_bytes(),
                source_ordinal: *source_ordinal,
            },
            StablePatternCoordinateStep::SequenceElement(ordinal) => {
                CheckedPatternCoordinateStep::SequenceElement(*ordinal)
            }
            StablePatternCoordinateStep::VariantPayload => {
                CheckedPatternCoordinateStep::VariantPayload
            }
            StablePatternCoordinateStep::WholeBindingInner => {
                CheckedPatternCoordinateStep::WholeBindingInner
            }
            StablePatternCoordinateStep::OrAlternative(ordinal) => {
                CheckedPatternCoordinateStep::OrAlternative(*ordinal)
            }
            StablePatternCoordinateStep::TypedBindingInner => {
                CheckedPatternCoordinateStep::TypedBindingInner
            }
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedUnreachablePattern {
    arm: StableMatchArmCoordinate,
    alternative: Option<StablePatternCoordinate>,
    reason: CheckedUnreachableReason,
}

impl CheckedUnreachablePattern {
    pub const fn arm(&self) -> &StableMatchArmCoordinate {
        &self.arm
    }

    pub(crate) const fn alternative(&self) -> Option<&StablePatternCoordinate> {
        self.alternative.as_ref()
    }

    pub const fn has_alternative(&self) -> bool {
        self.alternative.is_some()
    }

    pub const fn alternative_view(&self) -> Option<CheckedPatternCoordinateView<'_>> {
        match self.alternative.as_ref() {
            Some(coordinate) => Some(CheckedPatternCoordinateView(coordinate)),
            None => None,
        }
    }

    pub const fn reason(&self) -> CheckedUnreachableReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchCoverage {
    exhaustive: bool,
    unreachable: Box<[CheckedUnreachablePattern]>,
    witness: Option<CheckedCoverageWitness>,
    domain_digest: CheckedCoverageDomainDigest,
    work: CheckedMatchWork,
}

impl CheckedMatchCoverage {
    pub const fn exhaustive(&self) -> bool {
        self.exhaustive
    }

    pub fn unreachable(&self) -> &[CheckedUnreachablePattern] {
        self.unreachable.as_ref()
    }

    pub(crate) const fn witness(&self) -> Option<&CheckedCoverageWitness> {
        self.witness.as_ref()
    }

    pub const fn domain_digest(&self) -> CheckedCoverageDomainDigest {
        self.domain_digest
    }

    pub const fn observed_work(&self, kind: CheckedMatchLimitKind) -> u64 {
        self.work.observed(kind)
    }

    pub(super) fn finish_transaction_work(&mut self, work: CheckedMatchWork) {
        self.work = work;
    }
}

#[derive(Clone)]
pub(super) struct CoverageArmInput {
    pub(super) coordinate: StableMatchArmCoordinate,
    pub(super) pattern: PatternId,
    pub(super) guard: CheckedGuardClass,
}

pub(super) struct MatchCoverageAnalyzer<'a, 'control> {
    analysis: &'a FinalSemanticAnalysis,
    module: &'a HirModule,
    control: FinalSemanticAnalysisControl<'control>,
    budget: &'a mut CheckedMatchBudget,
    match_coordinate: StableSemanticCoordinate,
    observed_patterns: Vec<(PatternId, StableSemanticCoordinate)>,
    canonical_literals: BTreeMap<CanonicalCoverageLiteral, CanonicalCoverageLiteral>,
    canonical_literals_by_pattern: BTreeMap<PatternId, CanonicalCoverageLiteral>,
    retained_literal_bytes: u64,
    sequence_domains: BTreeMap<SemanticTypeDigest, Box<[CoverageConstructor]>>,
    #[cfg(test)]
    domain_overrides: BTreeMap<SemanticTypeDigest, CoverageTypeDomain>,
}

impl<'a, 'control> MatchCoverageAnalyzer<'a, 'control> {
    pub(super) fn new(
        analysis: &'a FinalSemanticAnalysis,
        module: &'a HirModule,
        control: FinalSemanticAnalysisControl<'control>,
        budget: &'a mut CheckedMatchBudget,
        match_coordinate: StableSemanticCoordinate,
        observed_patterns: Vec<(PatternId, StableSemanticCoordinate)>,
    ) -> Self {
        Self {
            analysis,
            module,
            control,
            budget,
            match_coordinate,
            observed_patterns,
            canonical_literals: BTreeMap::new(),
            canonical_literals_by_pattern: BTreeMap::new(),
            retained_literal_bytes: 0,
            sequence_domains: BTreeMap::new(),
            #[cfg(test)]
            domain_overrides: BTreeMap::new(),
        }
    }

    pub(super) fn analyze(
        mut self,
        scrutinee_type: &TypeKind,
        arms: &[CoverageArmInput],
    ) -> Result<CheckedMatchCoverage, CheckedMatchBuildError> {
        self.poll()?;
        let root_coordinate = self.match_coordinate.clone();
        let root_domain = self.domain(scrutinee_type, &root_coordinate)?;
        let domain_digest = self.domain_digest(scrutinee_type, &root_domain)?;
        let mut matrix = Matrix::new();
        let mut unreachable = Vec::new();

        if matches!(root_domain, CoverageTypeDomain::Empty) {
            for arm in arms {
                self.push_unreachable(
                    &mut unreachable,
                    arm.coordinate.clone(),
                    None,
                    CheckedUnreachableReason::UninhabitedDomain,
                )?;
            }
            return Ok(CheckedMatchCoverage {
                exhaustive: true,
                unreachable: unreachable.into_boxed_slice(),
                witness: None,
                domain_digest,
                work: self.budget.work(),
            });
        }

        for arm in arms {
            self.analyze_arm(arm, scrutinee_type, &mut matrix, &mut unreachable)?;
        }

        let wildcard = DeconstructedPattern::wildcard(
            StablePatternCoordinate::new([]),
            self.match_coordinate.clone(),
        );
        let witness = self
            .useful(
                &matrix,
                &[wildcard],
                std::slice::from_ref(scrutinee_type),
                0,
                &mut Vec::new(),
            )?
            .and_then(|mut witnesses| (witnesses.len() == 1).then(|| witnesses.remove(0)));
        let exhaustive = witness.is_none();
        Ok(CheckedMatchCoverage {
            exhaustive,
            unreachable: unreachable.into_boxed_slice(),
            witness,
            domain_digest,
            work: self.budget.work(),
        })
    }

    fn analyze_arm(
        &mut self,
        arm: &CoverageArmInput,
        scrutinee_type: &TypeKind,
        matrix: &mut Matrix,
        unreachable: &mut Vec<CheckedUnreachablePattern>,
    ) -> Result<(), CheckedMatchBuildError> {
        self.poll()?;
        if arm.guard == CheckedGuardClass::ConstantFalse {
            self.push_unreachable(
                unreachable,
                arm.coordinate.clone(),
                None,
                CheckedUnreachableReason::ConstantFalseGuard,
            )?;
            return Ok(());
        }
        let pattern = self.deconstruct(
            arm.pattern,
            scrutinee_type,
            &arm.coordinate,
            StablePatternCoordinate::new([]),
            0,
        )?;
        let expanded = self.expand_or(pattern, 0)?;
        let mut arm_matrix = self.clone_matrix(matrix)?;
        let global_len = arm_matrix.len();
        let mut useful_rows = Vec::new();
        for alternative in expanded {
            self.poll()?;
            let query = vec![alternative.pattern.clone()];
            let useful = self
                .useful(
                    &arm_matrix,
                    &query,
                    std::slice::from_ref(scrutinee_type),
                    0,
                    &mut Vec::new(),
                )?
                .is_some();
            if useful {
                self.push_matrix_row(&mut arm_matrix, query.clone())?;
                useful_rows.push(query);
            } else {
                let reason = if arm_matrix.len() > global_len {
                    CheckedUnreachableReason::CoveredByEarlierOrAlternative
                } else {
                    CheckedUnreachableReason::CoveredByPriorUsefulArms
                };
                self.push_unreachable(
                    unreachable,
                    arm.coordinate.clone(),
                    alternative.alternative,
                    reason,
                )?;
            }
        }
        if useful_rows.is_empty()
            && unreachable
                .last()
                .is_none_or(|row| row.arm != arm.coordinate)
        {
            self.push_unreachable(
                unreachable,
                arm.coordinate.clone(),
                None,
                CheckedUnreachableReason::CoveredByPriorUsefulArms,
            )?;
        }
        if matches!(
            arm.guard,
            CheckedGuardClass::Absent | CheckedGuardClass::ConstantTrue
        ) {
            for row in useful_rows {
                self.poll()?;
                self.push_matrix_row(matrix, row)?;
            }
        }
        Ok(())
    }

    fn poll(&self) -> Result<(), CheckedMatchBuildError> {
        self.control
            .check()
            .map_err(|_| CheckedMatchBuildError::Cancelled)
    }

    fn canonical_literal(
        &mut self,
        owner: PatternId,
        literal: &HirLiteral,
        ty: &TypeKind,
        coordinate: &StableSemanticCoordinate,
    ) -> Result<CanonicalCoverageLiteral, CheckedMatchBuildError> {
        if let Some(canonical) = self.canonical_literals_by_pattern.get(&owner) {
            return Ok(canonical.clone());
        }
        let (candidate, retained_literal_bytes) = CanonicalCoverageLiteral::from_checked(
            literal,
            ty,
            self.budget,
            self.retained_literal_bytes,
            coordinate,
        )?;
        let canonical = if let Some(canonical) = self.canonical_literals.get(&candidate) {
            canonical.clone()
        } else {
            self.retained_literal_bytes = retained_literal_bytes;
            self.canonical_literals
                .insert(candidate.clone(), candidate.clone());
            candidate
        };
        self.canonical_literals_by_pattern
            .insert(owner, canonical.clone());
        Ok(canonical)
    }

    fn push_unreachable(
        &mut self,
        output: &mut Vec<CheckedUnreachablePattern>,
        arm: StableMatchArmCoordinate,
        alternative: Option<StablePatternCoordinate>,
        reason: CheckedUnreachableReason,
    ) -> Result<(), CheckedMatchBuildError> {
        self.budget
            .charge(CheckedMatchLimitKind::UnreachableRows, 1)?;
        output.push(CheckedUnreachablePattern {
            arm,
            alternative,
            reason,
        });
        Ok(())
    }
}

fn append_coordinate(
    parent: &StablePatternCoordinate,
    step: StablePatternCoordinateStep,
) -> StablePatternCoordinate {
    let mut steps = parent.steps().to_vec();
    steps.push(step);
    StablePatternCoordinate::new(steps)
}

fn append_record_coordinate(
    parent: &StablePatternCoordinate,
    field: &super::CheckedRecordPatternField,
) -> StablePatternCoordinate {
    append_coordinate(
        parent,
        StablePatternCoordinateStep::RecordField {
            field: field.semantic_id(),
            source_ordinal: field.source_ordinal(),
        },
    )
}

#[cfg(test)]
#[path = "match_coverage/tests.rs"]
mod tests;
