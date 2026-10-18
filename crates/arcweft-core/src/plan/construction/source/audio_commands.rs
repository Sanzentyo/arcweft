//! Complete audio source transport through the one phase-parameterized grammar.
//! Child frames remain borrowed until the original expression owner admits them.

mod cursor;
mod encoding;
#[cfg(test)]
mod tests;

use super::RuntimePlanSourceConstruction;
use super::expression::{
    RuntimeExpressionSourceError, RuntimeExpressionSourceHandles, RuntimeExpressionSourceLimits,
};
use crate::audio::RuntimeAudioCommand;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::RuntimeAudioCommandSeed;
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::RuntimeExpr;

const DOMAIN: &[u8] = b"arcweft.runtime.audio-command-source.v1\0";
#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeAudioCommandSourceLimits {
    pub nodes: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    /// Named seed/final command, borrowed view and child-codec working payload.
    pub decoded_bytes: usize,
    pub expression: RuntimeExpressionSourceLimits,
}
#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeAudioCommandSourceError {
    #[error("audio-command source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid audio-command source {kind} at byte {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("audio-command source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("audio-command source arithmetic overflow")]
    Arithmetic,
    #[error("audio-command source is truncated at byte {offset}")]
    Truncated { offset: usize },
    #[error("audio-command source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("audio-command source metadata is invalid: {0}")]
    Metadata(#[from] arcweft_interaction_model::audio::AudioValueError),
    #[error("audio-command source expression is invalid: {0}")]
    Expression(#[from] RuntimeExpressionSourceError),
    #[error("audio-command source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeAudioCommandSourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}
impl Error {
    fn reject(self, meter: &mut TaskSemanticMeter) -> Self {
        let error = match meter.status() {
            Err(cause) if cause != TaskSemanticEncodingError::OwnerRejected => {
                Self::Resource(cause)
            }
            _ => self,
        };
        meter.reject_owner();
        error
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    nodes: usize,
    elements: usize,
    storage: usize,
}
struct Counts {
    limits: RuntimeAudioCommandSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeAudioCommandSourceLimits) -> Result<Self, Error> {
        let mut result = Self {
            limits,
            footprint: Footprint::default(),
        };
        result.child(
            1,
            1,
            size_of::<RuntimeAudioCommandSeed>()
                + size_of::<RuntimeAudioCommand>()
                + 2 * size_of::<Box<()>>()
                + size_of::<RuntimeAudioCommand<&[u8]>>(),
        )?;
        Ok(result)
    }
    fn child(&mut self, nodes: usize, elements: usize, storage: usize) -> Result<(), Error> {
        self.footprint.nodes = self
            .footprint
            .nodes
            .checked_add(nodes)
            .ok_or(Error::Arithmetic)?;
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(elements)
            .ok_or(Error::Arithmetic)?;
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(storage)
            .ok_or(Error::Arithmetic)?;
        limit("nodes", self.footprint.nodes, self.limits.nodes as usize)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )?;
        limit(
            "decoded bytes",
            self.footprint.storage,
            self.limits.decoded_bytes,
        )
    }
    fn expression(&self, n: usize) -> RuntimeExpressionSourceLimits {
        RuntimeExpressionSourceLimits {
            nodes: self
                .limits
                .expression
                .nodes
                .min((self.limits.nodes as usize - self.footprint.nodes) as u32),
            elements: self
                .limits
                .expression
                .elements
                .min((self.limits.elements as usize - self.footprint.elements) as u32),
            encoded_bytes: self.limits.expression.encoded_bytes.min(n),
            decoded_bytes: self
                .limits
                .expression
                .decoded_bytes
                .min(self.limits.decoded_bytes - self.footprint.storage),
            ..self.limits.expression
        }
    }
}
pub(in crate::plan) struct PreparedRuntimeAudioCommandSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeAudioCommandSourceLimits,
    footprint: Footprint,
}
impl<'a> PreparedRuntimeAudioCommandSource<'a> {
    pub(in crate::plan) fn preflight(
        bytes: &'a [u8],
        limits: RuntimeAudioCommandSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = cursor::Cursor::new(bytes);
            let command = cursor.command()?;
            cursor.finish()?;
            let mut counts = Counts::new(limits)?;
            command.try_map_expressions(|bytes| {
                let child = RuntimeExpr::preflight_sources(
                    bytes,
                    counts.expression(bytes.len()),
                    resources,
                    meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())
            })?;
            Ok(Self {
                bytes,
                limits,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) fn materialize(
        &self,
        source: &mut RuntimePlanSourceConstruction,
        handles: &RuntimeExpressionSourceHandles<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<RuntimeAudioCommandSeed, Error> {
        source.meter.status()?;
        let result = (|| {
            handles.validate(source, self.limits.expression)?;
            charge(&mut source.meter, self.bytes.len())?;
            let mut cursor = cursor::Cursor::new(self.bytes);
            let command = cursor.command()?;
            cursor.finish()?;
            let mut counts = Counts::new(self.limits)?;
            let seed = command.try_map_expressions(|bytes| {
                let child = RuntimeExpr::preflight_sources(
                    bytes,
                    counts.expression(bytes.len()),
                    resources,
                    &mut source.meter,
                )?;
                counts.child(child.node_count(), child.elements(), child.storage_bytes())?;
                Ok::<_, Error>(child.materialize(source, handles, resources)?)
            })?;
            debug_assert_eq!(counts.footprint, self.footprint);
            Ok(seed)
        })();
        result.map_err(|error: Error| error.reject(&mut source.meter))
    }
}
fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), Error> {
    if actual > maximum {
        return Err(Error::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn charge(meter: &mut TaskSemanticMeter, n: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(n)
            .map_err(|_| Error::Arithmetic)?
            .checked_mul(3)
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
