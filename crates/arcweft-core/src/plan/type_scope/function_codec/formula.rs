//! Specialization arguments use the same membership/decision grammar as a
//! function contract, interpreted in their actual target lexical scope.

use super::{
    Counts, EffectFormula, FunctionScope, Reader, RuntimeBoundEffectReference,
    RuntimeFunctionTypeContract, RuntimeFunctionTypeSourceError as Error,
    RuntimeFunctionTypeSourceLimits, RuntimeTypeBinder, RuntimeTypeScopeRef, TaskSemanticMeter,
    Writer, charge_passes, limit,
};
use crate::effect_row::DecisionEncoding;

type Formula = EffectFormula<RuntimeBoundEffectReference>;

pub(in crate::plan) struct PreparedRuntimeEffectFormulaEncoding<'a> {
    formula: &'a Formula,
    scope: FunctionScope<'a>,
    limits: RuntimeFunctionTypeSourceLimits,
    encoded_bytes: usize,
    storage_bytes: usize,
    elements: usize,
}
impl PreparedRuntimeEffectFormulaEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(Error::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(Some(output), None, self.scope, Counts::new(self.limits, 1)?);
        writer.formula(self.formula)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.storage, self.storage_bytes);
        Ok(())
    }
}

pub(in crate::plan) struct PreparedRuntimeEffectFormulaSource<'a> {
    bytes: &'a [u8],
    scope: FunctionScope<'a>,
    limits: RuntimeFunctionTypeSourceLimits,
    storage_bytes: usize,
    elements: usize,
}
impl PreparedRuntimeEffectFormulaSource<'_> {
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }
    pub(in crate::plan) fn materialize(
        self,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Formula, Error> {
        meter.status()?;
        let result = (|| {
            let mut reader = Reader::new(
                self.bytes,
                self.scope.incoming,
                Counts::new(self.limits, 1)?,
            );
            let formula = reader
                .formula(self.scope, true)?
                .expect("retaining the original effect argument");
            debug_assert_eq!(reader.counts.storage, self.storage_bytes);
            Ok(formula)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl Formula {
    pub(in crate::plan) fn prepare_source_encoding<'a>(
        &'a self,
        scope: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeEffectFormulaEncoding<'a>, Error> {
        meter.status()?;
        self.count_source_encoding(
            FunctionScope::new(scope.into(), RuntimeTypeBinder::EMPTY)?,
            limits,
            Some(meter),
        )
    }
    fn count_source_encoding<'a>(
        &'a self,
        scope: FunctionScope<'a>,
        limits: RuntimeFunctionTypeSourceLimits,
        mut meter: Option<&mut TaskSemanticMeter>,
    ) -> Result<PreparedRuntimeEffectFormulaEncoding<'a>, Error> {
        let result = (|| {
            let mut writer =
                Writer::new(None, meter.as_deref_mut(), scope, Counts::new(limits, 1)?);
            writer.formula(self)?;
            let encoded_bytes = writer.length;
            let storage_bytes = writer.counts.storage;
            let elements = writer
                .counts
                .nodes
                .checked_add(writer.counts.overrides)
                .ok_or(Error::Arithmetic)?;
            if let Some(meter) = &mut meter {
                charge_passes(meter, encoded_bytes, 2)?;
            }
            Ok(PreparedRuntimeEffectFormulaEncoding {
                formula: self,
                scope,
                limits,
                encoded_bytes,
                storage_bytes,
                elements,
            })
        })();
        if result.is_err()
            && let Some(meter) = meter
        {
            meter.reject_owner();
        }
        result
    }
    pub(in crate::plan) fn preflight_source<'a>(
        bytes: &'a [u8],
        scope: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeEffectFormulaSource<'a>, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_passes(meter, bytes.len(), 3)?;
            Self::preflight_source_in_frame(bytes, scope, limits)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
    pub(in crate::plan) fn preflight_source_in_frame<'a>(
        bytes: &'a [u8],
        scope: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
    ) -> Result<PreparedRuntimeEffectFormulaSource<'a>, Error> {
        limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
        Self::preflight_with_scope(
            bytes,
            FunctionScope::new(scope.into(), RuntimeTypeBinder::EMPTY)?,
            limits,
        )
    }

    fn preflight_with_scope<'a>(
        bytes: &'a [u8],
        scope: FunctionScope<'a>,
        limits: RuntimeFunctionTypeSourceLimits,
    ) -> Result<PreparedRuntimeEffectFormulaSource<'a>, Error> {
        limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
        let mut reader = Reader::new(bytes, scope.incoming, Counts::new(limits, 1)?);
        reader.formula(scope, false)?;
        Ok(PreparedRuntimeEffectFormulaSource {
            bytes,
            scope,
            limits,
            storage_bytes: reader.counts.storage,
            elements: reader
                .counts
                .nodes
                .checked_add(reader.counts.overrides)
                .ok_or(Error::Arithmetic)?,
        })
    }
}

impl RuntimeFunctionTypeContract {
    /// The target contract contributes its real binder to the original incoming
    /// scope. This view borrows that scope; no owned scope or contract is made.
    pub(in crate::plan) fn prepare_argument_source_in_frame<'a>(
        &self,
        argument: &'a Formula,
        incoming: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
    ) -> Result<PreparedRuntimeEffectFormulaEncoding<'a>, Error> {
        argument.count_source_encoding(
            FunctionScope::new(incoming.into(), self.binder())?,
            limits,
            None,
        )
    }

    pub(in crate::plan) fn prepare_argument_source_encoding<'a>(
        &self,
        argument: &'a Formula,
        incoming: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeEffectFormulaEncoding<'a>, Error> {
        meter.status()?;
        let result = FunctionScope::new(incoming.into(), self.binder());
        match result {
            Ok(scope) => argument.count_source_encoding(scope, limits, Some(meter)),
            Err(error) => {
                meter.reject_owner();
                Err(error)
            }
        }
    }

    pub(in crate::plan) fn preflight_argument_source_in_frame<'a>(
        &self,
        bytes: &'a [u8],
        incoming: impl Into<RuntimeTypeScopeRef<'a>>,
        limits: RuntimeFunctionTypeSourceLimits,
    ) -> Result<PreparedRuntimeEffectFormulaSource<'a>, Error> {
        Formula::preflight_with_scope(
            bytes,
            FunctionScope::new(incoming.into(), self.binder())?,
            limits,
        )
    }
}

impl Writer<'_, '_> {
    fn formula(&mut self, formula: &Formula) -> Result<(), Error> {
        self.tag(1)?;
        formula.encode(self)
    }
}
impl Reader<'_> {
    fn formula(
        &mut self,
        scope: FunctionScope<'_>,
        retain: bool,
    ) -> Result<Option<Formula>, Error> {
        let actual = self.tag()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let membership = self.membership(scope, retain)?;
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing);
        }
        membership
            .map(Formula::try_from)
            .transpose()
            .map_err(Error::from)
    }
}
