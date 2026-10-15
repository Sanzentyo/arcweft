//! Canonical data output. This writer issues no semantic proof or task key.

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MatchSourceEncodingError {
    #[error("semantic coordinate length does not fit the canonical u64 grammar")]
    LengthOverflow,
}

pub trait MatchSourceSink {
    type Error: From<MatchSourceEncodingError>;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}

impl MatchSourceSink for Vec<u8> {
    type Error = MatchSourceEncodingError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

impl<W: MatchSourceSink + ?Sized> MatchSourceSink for &mut W {
    type Error = W::Error;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        (**self).write_bytes(bytes)
    }
}

/// Keeps the first counter/output failure while coordinate owners write their
/// existing canonical grammar. No bytes reach the sink after that failure.
pub struct MatchSourceWriter<W: MatchSourceSink> {
    sink: W,
    error: Option<W::Error>,
    byte_len: u64,
}

impl<W: MatchSourceSink> MatchSourceWriter<W> {
    #[must_use]
    pub const fn new(sink: W) -> Self {
        Self {
            sink,
            error: None,
            byte_len: 0,
        }
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.error.is_none()
    }

    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub fn push(&mut self, byte: u8) {
        self.extend_from_slice(&[byte]);
    }

    pub fn extend_from_slice(&mut self, bytes: &[u8]) {
        if !self.is_active() {
            return;
        }
        let Some(next) = u64::try_from(bytes.len())
            .ok()
            .and_then(|len| self.byte_len.checked_add(len))
        else {
            self.error = Some(MatchSourceEncodingError::LengthOverflow.into());
            return;
        };
        match self.sink.write_bytes(bytes) {
            Ok(()) => self.byte_len = next,
            Err(error) => self.error = Some(error),
        }
    }

    pub fn finish(self, encoded: Result<(), MatchSourceEncodingError>) -> Result<W, W::Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        encoded.map_err(W::Error::from)?;
        Ok(self.sink)
    }
}

pub trait CanonicalMatchSource {
    fn write_source<W: MatchSourceSink>(
        &self,
        output: &mut MatchSourceWriter<W>,
    ) -> Result<(), MatchSourceEncodingError>;
}

pub trait MatchSourceArmCoordinateOwner {
    type Owner: CanonicalMatchSource;
    fn source_coordinate(&self) -> &super::MatchSourceArmCoordinate<Self::Owner>;
}

impl<C: CanonicalMatchSource> MatchSourceArmCoordinateOwner for super::MatchSourceArmCoordinate<C> {
    type Owner = C;
    fn source_coordinate(&self) -> &Self {
        self
    }
}

pub trait MatchSourceUnreachableOwner {
    type Arm: MatchSourceArmCoordinateOwner;
    type Alternative: CanonicalMatchSource;
    fn source_unreachable(
        &self,
    ) -> &super::MatchSourceUnreachable<
        Self::Arm,
        Self::Alternative,
        super::MatchSourceUnreachableReason,
    >;
}

impl<A: MatchSourceArmCoordinateOwner, P: CanonicalMatchSource> MatchSourceUnreachableOwner
    for super::MatchSourceUnreachable<A, P, super::MatchSourceUnreachableReason>
{
    type Arm = A;
    type Alternative = P;
    fn source_unreachable(&self) -> &Self {
        self
    }
}

pub trait MatchSourceCoverageOwner {
    type Domain: AsRef<[u8; 32]>;
    type Unreachable: MatchSourceUnreachableOwner;
    fn source_coverage(&self) -> &super::MatchSourceCoverage<Self::Domain, Self::Unreachable>;
}

impl<D: AsRef<[u8; 32]>, U: MatchSourceUnreachableOwner> MatchSourceCoverageOwner
    for super::MatchSourceCoverage<D, U>
{
    type Domain = D;
    type Unreachable = U;
    fn source_coverage(&self) -> &Self {
        self
    }
}

impl<C, E, T, A, P, G, R, B, BC, BT, V>
    super::MatchSourceDefinition<
        C,
        E,
        T,
        super::MatchSourceArm<
            A,
            P,
            super::MatchSourceGuard<G>,
            super::MatchSourceArmResult<R, B>,
            super::MatchSourceBinding<BC, BT>,
        >,
        V,
    >
where
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
    /// Complete version-one Match data. The root coordinate is the separate
    /// site identity; nested coordinates retain their original owners.
    pub fn write_semantic<W: MatchSourceSink>(
        &self,
        output: &mut MatchSourceWriter<W>,
    ) -> Result<(), MatchSourceEncodingError> {
        output.extend_from_slice(b"arcweft.lang.checked-match-semantic.v1\0");
        output.extend_from_slice(self.scrutinee.as_ref());
        output.extend_from_slice(self.scrutinee_type.as_ref());
        write_len(output, self.arms.len())?;
        for arm in &self.arms {
            if !output.is_active() {
                return Ok(());
            }
            output.extend_from_slice(&arm.coordinate.source_coordinate().ordinal().to_le_bytes());
            output.extend_from_slice(arm.pattern.as_ref());
            write_len(output, arm.bindings.len())?;
            for binding in &arm.bindings {
                if !output.is_active() {
                    return Ok(());
                }
                binding.coordinate.write_source(output)?;
                output.extend_from_slice(binding.ty.as_ref());
            }
            if let Some(expression) = arm.guard.expression() {
                output.push(1);
                output.extend_from_slice(expression.as_ref());
                output.push(arm.guard.class().semantic_tag());
            } else {
                output.push(0);
            }
            output.push(arm.result.tag());
            output.extend_from_slice(arm.result.as_bytes());
        }
        let coverage = self.coverage.source_coverage();
        output.push(u8::from(coverage.exhaustive));
        output.extend_from_slice(coverage.domain_digest.as_ref());
        write_len(output, coverage.unreachable.len())?;
        for row in &coverage.unreachable {
            if !output.is_active() {
                return Ok(());
            }
            let row = row.source_unreachable();
            let arm = row.arm.source_coordinate();
            arm.owner().write_source(output)?;
            output.extend_from_slice(&arm.ordinal().to_le_bytes());
            if let Some(alternative) = &row.alternative {
                output.push(1);
                alternative.write_source(output)?;
            } else {
                output.push(0);
            }
            output.push(row.reason.semantic_tag());
        }
        Ok(())
    }
}

fn write_len<W: MatchSourceSink>(
    output: &mut MatchSourceWriter<W>,
    value: usize,
) -> Result<(), MatchSourceEncodingError> {
    let value = u64::try_from(value).map_err(|_| MatchSourceEncodingError::LengthOverflow)?;
    output.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests;
