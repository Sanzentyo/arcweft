//! Retained data uses the same definition as checked analysis. Canonical
//! coordinate bytes preserve their original owner's encoding; they are data,
//! never an executable, checked admission, or task-plan proof.

use serde::{Deserialize, Serialize};

use super::{
    CanonicalMatchSource, MatchSourceArm, MatchSourceArmCoordinate, MatchSourceArmCoordinateOwner,
    MatchSourceArmResult, MatchSourceBinding, MatchSourceCoverage, MatchSourceCoverageOwner,
    MatchSourceDefinition, MatchSourceEncodingError, MatchSourceGuard, MatchSourceSink,
    MatchSourceUnreachable, MatchSourceUnreachableOwner, MatchSourceUnreachableReason,
    MatchSourceWriter,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct MatchSourceDataCoordinate(Box<[u8]>);

impl CanonicalMatchSource for MatchSourceDataCoordinate {
    fn write_source<W: MatchSourceSink>(
        &self,
        output: &mut MatchSourceWriter<W>,
    ) -> Result<(), MatchSourceEncodingError> {
        output.extend_from_slice(&self.0);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct MatchSourceDataIdentity([u8; 32]);

impl AsRef<[u8; 32]> for MatchSourceDataIdentity {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

pub type MatchSourceDataBinding =
    MatchSourceBinding<MatchSourceDataCoordinate, MatchSourceDataIdentity>;
pub type MatchSourceDataArm = MatchSourceArm<
    MatchSourceArmCoordinate<MatchSourceDataCoordinate>,
    MatchSourceDataIdentity,
    MatchSourceGuard<MatchSourceDataIdentity>,
    MatchSourceArmResult<MatchSourceDataIdentity, MatchSourceDataIdentity>,
    MatchSourceDataBinding,
>;
pub type MatchSourceDataUnreachable = MatchSourceUnreachable<
    MatchSourceArmCoordinate<MatchSourceDataCoordinate>,
    MatchSourceDataCoordinate,
    MatchSourceUnreachableReason,
>;
pub type MatchSourceDataCoverage =
    MatchSourceCoverage<MatchSourceDataIdentity, MatchSourceDataUnreachable>;
pub type MatchSourceData = MatchSourceDefinition<
    MatchSourceDataCoordinate,
    MatchSourceDataIdentity,
    MatchSourceDataIdentity,
    MatchSourceDataArm,
    MatchSourceDataCoverage,
>;

impl MatchSourceData {
    /// Arm references belong to this exact root and use dense source ordinals.
    pub fn has_canonical_arm_coordinates(&self) -> bool {
        self.arms.iter().enumerate().all(|(ordinal, arm)| {
            arm.coordinate.owner() == &self.coordinate
                && usize::try_from(arm.coordinate.ordinal()).ok() == Some(ordinal)
        }) && self.coverage.unreachable.iter().all(|row| {
            row.arm.owner() == &self.coordinate
                && usize::try_from(row.arm.ordinal()).is_ok_and(|ordinal| ordinal < self.arms.len())
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MatchSourceMaterializationError {
    #[error(transparent)]
    Encoding(#[from] MatchSourceEncodingError),
    #[error("Match source byte limit {limit} exceeded by attempt {attempted}")]
    LimitExceeded { limit: u64, attempted: u64 },
    #[error("Match source byte accounting overflow")]
    ArithmeticOverflow,
    #[error("Match source coordinate encoding changed after preflight")]
    ChangedCoordinateEncoding,
}

/// One parent allowance shared by all retained Match definitions in a product.
/// Counts retained row storage and canonical coordinate bytes before copying.
pub struct MatchSourceMaterializationBudget {
    limit: u64,
    used: u64,
    error: Option<MatchSourceMaterializationError>,
}

impl MatchSourceMaterializationBudget {
    #[must_use]
    pub const fn new(limit: u64) -> Self {
        Self {
            limit,
            used: 0,
            error: None,
        }
    }

    #[must_use]
    pub const fn used_bytes(&self) -> u64 {
        self.used
    }

    fn charge(&mut self, bytes: u64) -> Result<(), MatchSourceMaterializationError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let attempted = self
            .used
            .checked_add(bytes)
            .ok_or(MatchSourceMaterializationError::ArithmeticOverflow);
        let result = attempted.and_then(|attempted| {
            if attempted > self.limit {
                Err(MatchSourceMaterializationError::LimitExceeded {
                    limit: self.limit,
                    attempted,
                })
            } else {
                self.used = attempted;
                Ok(())
            }
        });
        if let Err(error) = result {
            self.error = Some(error);
        }
        result
    }

    fn charge_rows<T>(&mut self, count: usize) -> Result<(), MatchSourceMaterializationError> {
        let bytes = count
            .checked_mul(size_of::<T>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(MatchSourceMaterializationError::ArithmeticOverflow)?;
        self.charge(bytes)
    }
}

impl MatchSourceSink for MatchSourceMaterializationBudget {
    type Error = MatchSourceMaterializationError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let count = u64::try_from(bytes.len())
            .map_err(|_| MatchSourceMaterializationError::ArithmeticOverflow)?;
        self.charge(count)
    }
}

struct CoordinateCopy<'a> {
    bytes: Vec<u8>,
    length: usize,
    remaining: &'a mut u64,
}

impl MatchSourceSink for CoordinateCopy<'_> {
    type Error = MatchSourceMaterializationError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|end| end > self.length)
        {
            return Err(MatchSourceMaterializationError::ChangedCoordinateEncoding);
        }
        let count = u64::try_from(bytes.len())
            .map_err(|_| MatchSourceMaterializationError::ArithmeticOverflow)?;
        let remaining = self
            .remaining
            .checked_sub(count)
            .ok_or(MatchSourceMaterializationError::ChangedCoordinateEncoding)?;
        *self.remaining = remaining;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

fn copy_coordinate<C: CanonicalMatchSource>(
    coordinate: &C,
    remaining: &mut u64,
) -> Result<MatchSourceDataCoordinate, MatchSourceMaterializationError> {
    let mut measured = MatchSourceWriter::new(CoordinateLength(*remaining));
    let encoded = coordinate.write_source(&mut measured);
    let length = measured.byte_len();
    measured.finish(encoded)?;
    let length =
        usize::try_from(length).map_err(|_| MatchSourceMaterializationError::ArithmeticOverflow)?;
    let mut writer = MatchSourceWriter::new(CoordinateCopy {
        bytes: Vec::with_capacity(length),
        length,
        remaining,
    });
    let encoded = coordinate.write_source(&mut writer);
    let copied = writer.finish(encoded)?;
    if copied.bytes.len() != length {
        return Err(MatchSourceMaterializationError::ChangedCoordinateEncoding);
    }
    Ok(MatchSourceDataCoordinate(copied.bytes.into_boxed_slice()))
}

struct CoordinateLength(u64);
impl MatchSourceSink for CoordinateLength {
    type Error = MatchSourceMaterializationError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0 = self
            .0
            .checked_sub(u64::try_from(bytes.len()).map_err(|_| Self::Error::ArithmeticOverflow)?)
            .ok_or(Self::Error::ChangedCoordinateEncoding)?;
        Ok(())
    }
}

fn copy_arm_coordinate<A: MatchSourceArmCoordinateOwner>(
    arm: &A,
    remaining: &mut u64,
) -> Result<MatchSourceArmCoordinate<MatchSourceDataCoordinate>, MatchSourceMaterializationError> {
    let source = arm.source_coordinate();
    Ok(MatchSourceArmCoordinate::new(
        copy_coordinate(source.owner(), remaining)?,
        source.ordinal(),
    ))
}

fn identity<I: AsRef<[u8; 32]>>(source: &I) -> MatchSourceDataIdentity {
    MatchSourceDataIdentity(*source.as_ref())
}

impl<C, E, T, A, P, G, R, B, BC, BT, V>
    MatchSourceDefinition<
        C,
        E,
        T,
        MatchSourceArm<
            A,
            P,
            MatchSourceGuard<G>,
            MatchSourceArmResult<R, B>,
            MatchSourceBinding<BC, BT>,
        >,
        V,
    >
where
    C: CanonicalMatchSource,
    E: AsRef<[u8; 32]>,
    T: AsRef<[u8; 32]>,
    A: MatchSourceArmCoordinateOwner,
    P: AsRef<[u8; 32]>,
    G: Copy + AsRef<[u8; 32]>,
    R: AsRef<[u8; 32]>,
    B: AsRef<[u8; 32]>,
    BC: CanonicalMatchSource,
    BT: AsRef<[u8; 32]>,
    V: MatchSourceCoverageOwner,
{
    /// Materializes inert source data after a complete, allocation-free pass.
    pub fn materialize_source(
        &self,
        budget: &mut MatchSourceMaterializationBudget,
    ) -> Result<MatchSourceData, MatchSourceMaterializationError> {
        let result = self.materialize_preflighted(budget);
        if let Err(error) = result {
            return Err(*budget.error.get_or_insert(error));
        }
        result
    }

    /// Checks the complete retained storage requirement without allocating it.
    pub fn preflight_source(
        &self,
        budget: &mut MatchSourceMaterializationBudget,
    ) -> Result<(), MatchSourceMaterializationError> {
        let result = self.source_copy_allowance(budget).map(|_| ());
        if let Err(error) = result {
            return Err(*budget.error.get_or_insert(error));
        }
        result
    }

    fn source_copy_allowance(
        &self,
        budget: &mut MatchSourceMaterializationBudget,
    ) -> Result<u64, MatchSourceMaterializationError> {
        budget.charge_rows::<MatchSourceData>(1)?;
        budget.charge_rows::<MatchSourceDataArm>(self.arms.len())?;
        for arm in &self.arms {
            budget.charge_rows::<MatchSourceDataBinding>(arm.bindings.len())?;
        }
        budget.charge_rows::<MatchSourceDataUnreachable>(
            self.coverage.source_coverage().unreachable.len(),
        )?;
        let mut coordinate_bytes = MatchSourceWriter::new(CoordinatePreflight { budget, bytes: 0 });
        let encoded = self.visit_coordinates(&mut coordinate_bytes);
        Ok(coordinate_bytes.finish(encoded)?.bytes)
    }

    fn materialize_preflighted(
        &self,
        budget: &mut MatchSourceMaterializationBudget,
    ) -> Result<MatchSourceData, MatchSourceMaterializationError> {
        let mut remaining = self.source_copy_allowance(budget)?;
        let coordinate = copy_coordinate(&self.coordinate, &mut remaining)?;
        let mut arms = Vec::with_capacity(self.arms.len());
        for arm in &self.arms {
            let mut bindings = Vec::with_capacity(arm.bindings.len());
            for binding in &arm.bindings {
                bindings.push(MatchSourceBinding::new(
                    copy_coordinate(&binding.coordinate, &mut remaining)?,
                    identity(&binding.ty),
                ));
            }
            let guard = match &arm.guard {
                MatchSourceGuard::Absent => MatchSourceGuard::Absent,
                MatchSourceGuard::ConstantTrue(value) => {
                    MatchSourceGuard::ConstantTrue(identity(value))
                }
                MatchSourceGuard::ConstantFalse(value) => {
                    MatchSourceGuard::ConstantFalse(identity(value))
                }
                MatchSourceGuard::Dynamic(value) => MatchSourceGuard::Dynamic(identity(value)),
            };
            let result = match &arm.result {
                MatchSourceArmResult::Expression(value) => {
                    MatchSourceArmResult::Expression(identity(value))
                }
                MatchSourceArmResult::Body(value) => MatchSourceArmResult::Body(identity(value)),
            };
            arms.push(MatchSourceArm::new(
                copy_arm_coordinate(&arm.coordinate, &mut remaining)?,
                identity(&arm.pattern),
                guard,
                result,
                bindings.into_boxed_slice(),
            ));
        }
        let coverage = self.coverage.source_coverage();
        let mut unreachable = Vec::with_capacity(coverage.unreachable.len());
        for row in &coverage.unreachable {
            let row = row.source_unreachable();
            unreachable.push(MatchSourceUnreachable::new(
                copy_arm_coordinate(&row.arm, &mut remaining)?,
                row.alternative
                    .as_ref()
                    .map(|alternative| copy_coordinate(alternative, &mut remaining))
                    .transpose()?,
                row.reason,
            ));
        }
        if remaining != 0 {
            return Err(MatchSourceMaterializationError::ChangedCoordinateEncoding);
        }
        Ok(MatchSourceDefinition::new(
            coordinate,
            identity(&self.scrutinee),
            identity(&self.scrutinee_type),
            arms.into_boxed_slice(),
            MatchSourceCoverage::new(
                coverage.exhaustive,
                identity(&coverage.domain_digest),
                unreachable.into_boxed_slice(),
            ),
        ))
    }

    fn visit_coordinates<W: MatchSourceSink>(
        &self,
        writer: &mut MatchSourceWriter<W>,
    ) -> Result<(), MatchSourceEncodingError> {
        self.coordinate.write_source(writer)?;
        for arm in &self.arms {
            if !writer.is_active() {
                return Ok(());
            }
            arm.coordinate
                .source_coordinate()
                .owner()
                .write_source(writer)?;
            for binding in &arm.bindings {
                if !writer.is_active() {
                    return Ok(());
                }
                binding.coordinate.write_source(writer)?;
            }
        }
        for row in &self.coverage.source_coverage().unreachable {
            if !writer.is_active() {
                return Ok(());
            }
            let row = row.source_unreachable();
            row.arm.source_coordinate().owner().write_source(writer)?;
            if let Some(alternative) = &row.alternative {
                alternative.write_source(writer)?;
            }
        }
        Ok(())
    }
}

struct CoordinatePreflight<'a> {
    budget: &'a mut MatchSourceMaterializationBudget,
    bytes: u64,
}
impl MatchSourceSink for CoordinatePreflight<'_> {
    type Error = MatchSourceMaterializationError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let count = u64::try_from(bytes.len()).map_err(|_| Self::Error::ArithmeticOverflow)?;
        self.budget.charge(count)?;
        self.bytes = self
            .bytes
            .checked_add(count)
            .ok_or(Self::Error::ArithmeticOverflow)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
