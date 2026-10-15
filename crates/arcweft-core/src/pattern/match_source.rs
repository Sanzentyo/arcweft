//! One Match definition grammar across checked and retained phases.
//! Phase-specific coordinates and identities remain on their original owners.
//! These data records do not issue an executable or task semantic proof.

use serde::{Deserialize, Serialize};

mod encoding;
mod materialization;
pub use encoding::{
    CanonicalMatchSource, MatchSourceArmCoordinateOwner, MatchSourceCoverageOwner,
    MatchSourceEncodingError, MatchSourceSink, MatchSourceUnreachableOwner, MatchSourceWriter,
};
pub use materialization::{
    MatchSourceData, MatchSourceDataArm, MatchSourceDataBinding, MatchSourceDataCoordinate,
    MatchSourceDataCoverage, MatchSourceDataIdentity, MatchSourceDataUnreachable,
    MatchSourceMaterializationBudget, MatchSourceMaterializationError,
};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSourceArmCoordinate<Owner> {
    owner: Owner,
    ordinal: u32,
}

impl<O> MatchSourceArmCoordinate<O> {
    #[must_use]
    pub const fn new(owner: O, ordinal: u32) -> Self {
        Self { owner, ordinal }
    }
    pub const fn owner(&self) -> &O {
        &self.owner
    }
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl MatchSourceGuardClass {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Absent => 0,
            Self::ConstantTrue => 1,
            Self::ConstantFalse => 2,
            Self::Dynamic => 3,
        }
    }
}

impl MatchSourceUnreachableReason {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::CoveredByPriorUsefulArms => 0,
            Self::ConstantFalseGuard => 1,
            Self::CoveredByEarlierOrAlternative => 2,
            Self::UninhabitedDomain => 3,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSourceDefinition<Coordinate, Expression, Type, Arm, Coverage> {
    coordinate: Coordinate,
    scrutinee: Expression,
    scrutinee_type: Type,
    arms: Box<[Arm]>,
    coverage: Coverage,
}

impl<C, E, T, A, V> MatchSourceDefinition<C, E, T, A, V> {
    #[must_use]
    pub fn new(
        coordinate: C,
        scrutinee: E,
        scrutinee_type: T,
        arms: Box<[A]>,
        coverage: V,
    ) -> Self {
        Self {
            coordinate,
            scrutinee,
            scrutinee_type,
            arms,
            coverage,
        }
    }

    pub const fn coordinate(&self) -> &C {
        &self.coordinate
    }
    pub const fn scrutinee(&self) -> E
    where
        E: Copy,
    {
        self.scrutinee
    }
    pub const fn scrutinee_type(&self) -> T
    where
        T: Copy,
    {
        self.scrutinee_type
    }
    pub fn arms(&self) -> &[A] {
        &self.arms
    }
    pub const fn coverage(&self) -> &V {
        &self.coverage
    }

    pub fn coverage_mut(&mut self) -> &mut V {
        &mut self.coverage
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSourceBinding<Coordinate, Type> {
    coordinate: Coordinate,
    ty: Type,
}

impl<C, T> MatchSourceBinding<C, T> {
    #[must_use]
    pub const fn new(coordinate: C, ty: T) -> Self {
        Self { coordinate, ty }
    }
    pub const fn coordinate(&self) -> &C {
        &self.coordinate
    }
    pub const fn ty(&self) -> T
    where
        T: Copy,
    {
        self.ty
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSourceArm<Coordinate, Pattern, Guard, Result, Binding> {
    coordinate: Coordinate,
    pattern: Pattern,
    guard: Guard,
    result: Result,
    bindings: Box<[Binding]>,
}

impl<C, P, G, R, B> MatchSourceArm<C, P, G, R, B> {
    #[must_use]
    pub fn new(coordinate: C, pattern: P, guard: G, result: R, bindings: Box<[B]>) -> Self {
        Self {
            coordinate,
            pattern,
            guard,
            result,
            bindings,
        }
    }
    pub const fn coordinate(&self) -> &C {
        &self.coordinate
    }
    pub const fn pattern(&self) -> P
    where
        P: Copy,
    {
        self.pattern
    }
    pub const fn guard(&self) -> G
    where
        G: Copy,
    {
        self.guard
    }
    pub const fn result(&self) -> R
    where
        R: Copy,
    {
        self.result
    }
    pub fn bindings(&self) -> &[B] {
        &self.bindings
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum MatchSourceGuardClass {
    Absent,
    ConstantTrue,
    ConstantFalse,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MatchSourceGuard<Expression> {
    Absent,
    ConstantTrue(Expression),
    ConstantFalse(Expression),
    Dynamic(Expression),
}

impl<E: Copy> MatchSourceGuard<E> {
    pub const fn expression(self) -> Option<E> {
        match self {
            Self::Absent => None,
            Self::ConstantTrue(value) | Self::ConstantFalse(value) | Self::Dynamic(value) => {
                Some(value)
            }
        }
    }
    pub const fn class(self) -> MatchSourceGuardClass {
        match self {
            Self::Absent => MatchSourceGuardClass::Absent,
            Self::ConstantTrue(_) => MatchSourceGuardClass::ConstantTrue,
            Self::ConstantFalse(_) => MatchSourceGuardClass::ConstantFalse,
            Self::Dynamic(_) => MatchSourceGuardClass::Dynamic,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MatchSourceArmResult<Expression, Body> {
    Expression(Expression),
    Body(Body),
}

impl<E, B> MatchSourceArmResult<E, B> {
    pub const fn tag(&self) -> u8 {
        match self {
            Self::Expression(_) => 0,
            Self::Body(_) => 1,
        }
    }
    pub fn as_bytes(&self) -> &[u8; 32]
    where
        E: AsRef<[u8; 32]>,
        B: AsRef<[u8; 32]>,
    {
        match self {
            Self::Expression(value) => value.as_ref(),
            Self::Body(value) => value.as_ref(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSourceCoverage<Domain, Unreachable> {
    exhaustive: bool,
    domain_digest: Domain,
    unreachable: Box<[Unreachable]>,
}

impl<D, U> MatchSourceCoverage<D, U> {
    #[must_use]
    pub fn new(exhaustive: bool, domain_digest: D, unreachable: Box<[U]>) -> Self {
        Self {
            exhaustive,
            domain_digest,
            unreachable,
        }
    }
    pub const fn exhaustive(&self) -> bool {
        self.exhaustive
    }
    pub const fn domain_digest(&self) -> D
    where
        D: Copy,
    {
        self.domain_digest
    }
    pub fn unreachable(&self) -> &[U] {
        &self.unreachable
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum MatchSourceUnreachableReason {
    CoveredByPriorUsefulArms,
    CoveredByEarlierOrAlternative,
    ConstantFalseGuard,
    UninhabitedDomain,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSourceUnreachable<Arm, Alternative, Reason> {
    arm: Arm,
    alternative: Option<Alternative>,
    reason: Reason,
}

impl<A, P, R> MatchSourceUnreachable<A, P, R> {
    #[must_use]
    pub const fn new(arm: A, alternative: Option<P>, reason: R) -> Self {
        Self {
            arm,
            alternative,
            reason,
        }
    }
    pub const fn arm(&self) -> &A {
        &self.arm
    }
    pub const fn alternative(&self) -> Option<&P> {
        self.alternative.as_ref()
    }
    pub const fn reason(&self) -> R
    where
        R: Copy,
    {
        self.reason
    }
}
