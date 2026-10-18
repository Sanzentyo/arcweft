use super::*;
use crate::effect_row::EffectFormula;
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::{
    RuntimeArrayLength, RuntimeBoundConstReference, RuntimeCallableSpecializationState,
    RuntimeFunctionSpecializationArguments,
};
use std::num::NonZeroU32;

pub(super) struct Reader<'a, 'types> {
    bytes: &'a [u8],
    types: Types<'types>,
    offset: usize,
    pub(super) counts: Counts,
}
impl<'a, 'types> Reader<'a, 'types> {
    pub(super) fn new(
        bytes: &'a [u8],
        limits: RuntimeSpecializationSourceLimits,
        types: Types<'types>,
    ) -> Result<Self, Error> {
        limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
        let prefix = bytes
            .get(..DOMAIN.len() + 5)
            .ok_or(Error::Truncated { offset: 0 })?;
        if &prefix[..DOMAIN.len()] != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = prefix[DOMAIN.len()];
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let rows = u32::from_le_bytes(
            prefix[DOMAIN.len() + 1..]
                .try_into()
                .expect("specialization count width"),
        ) as usize;
        Ok(Self {
            bytes,
            types,
            offset: DOMAIN.len() + 5,
            counts: Counts::new(limits, rows)?,
        })
    }
    pub(super) fn resume(
        bytes: &'a [u8],
        types: Types<'types>,
        offset: usize,
        counts: Counts,
    ) -> Self {
        Self {
            bytes,
            types,
            offset,
            counts,
        }
    }
    pub(super) fn into_parts(self) -> (usize, Counts) {
        (self.offset, self.counts)
    }
    fn bytes(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn tag(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().expect("u32 width"),
        ))
    }
    fn count(&mut self) -> Result<usize, Error> {
        Ok(self.u32()? as usize)
    }
    fn ty(&mut self) -> Result<RuntimePlanTypeId, Error> {
        let value = self.u32()?;
        reference("type", value as usize, self.counts.limits.type_count, true)?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(
            NonZeroU32::new(value).expect("checked source coordinate"),
        ))
    }
    fn list<T>(
        &mut self,
        count: usize,
        retain: bool,
        mut read: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Option<Box<[T]>>, Error> {
        let mut rows = retain.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let row = read(self)?;
            if let Some(rows) = &mut rows {
                rows.push(row);
            }
        }
        Ok(rows.map(Vec::into_boxed_slice))
    }
    pub(super) fn row(
        &mut self,
        index: usize,
        states: Option<&[RuntimeCallableStateSeedId]>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Option<RuntimeCallableSpecializationSeed>, Error> {
        let actual = self.u32()?;
        let expected = u32::try_from(index).map_err(|_| Error::Arithmetic)?;
        if actual != expected {
            return Err(Error::Ordinal { actual, expected });
        }
        let source_type = self.ty()?;
        let target_type = self.ty()?;
        let (target, scope) = self.types.target(target_type)?;
        let retain = states.is_some();
        let count = self.count()?;
        self.counts.array(
            count,
            size_of::<RuntimeSemanticTypeId>() + size_of::<RuntimePlanTypeId>(),
        )?;
        let types = self.list(count, retain, |reader| {
            let ty = reader.ty()?;
            Ok(reader.types.get(ty).semantic_identity())
        })?;
        let count = self.count()?;
        self.counts.array(count, size_of::<RuntimeArrayLength>())?;
        let const_lengths = self.list(count, retain, |reader| {
            Ok(match reader.tag()? {
                0 => RuntimeArrayLength::Constant(u64::from_le_bytes(
                    reader.bytes(8)?.try_into().expect("u64 width"),
                )),
                1 => {
                    let depth = reader.u32()?;
                    let slot = u16::from_le_bytes(reader.bytes(2)?.try_into().expect("u16 width"));
                    RuntimeArrayLength::Bound(RuntimeBoundConstReference::from_coordinates(
                        depth, slot,
                    ))
                }
                tag => return Err(Error::Tag { tag }),
            })
        })?;
        let count = self.count()?;
        self.counts.array(
            count,
            size_of::<EffectFormula<crate::plan::RuntimeBoundEffectReference>>(),
        )?;
        let mut effects = retain.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let length = self.count()?;
            let bytes = self.bytes(length)?;
            let prepared = target.preflight_argument_source_in_frame(
                bytes,
                scope,
                self.counts.effect_limits(length)?,
            )?;
            self.counts
                .child(prepared.elements(), prepared.storage_bytes())?;
            if let Some(effects) = &mut effects {
                effects.push(prepared.materialize(meter)?);
            }
        }
        let count = self.count()?;
        self.counts.array(
            count,
            size_of::<RuntimeCallableSpecializationState<RuntimeCallableStateSeedId>>()
                + size_of::<RuntimeCallableSpecializationState<RuntimeCallableStateId>>(),
        )?;
        let mut pairs = retain.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let source = self.count()?;
            let target = self.count()?;
            reference("state", source, self.counts.limits.state_count, false)?;
            reference("state", target, self.counts.limits.state_count, false)?;
            if let (Some(states), Some(pairs)) = (states, &mut pairs) {
                pairs.push(RuntimeCallableSpecializationState {
                    source: states[source].clone(),
                    target: states[target].clone(),
                });
            }
        }
        Ok(states.map(|_| RuntimeCallableSpecializationDefinition {
            source_type: self.types.get(source_type).semantic_identity(),
            target_type: self.types.get(target_type).semantic_identity(),
            arguments: RuntimeFunctionSpecializationArguments {
                types: types.expect("retained type arguments"),
                const_lengths: const_lengths.expect("retained constants"),
                effects: effects.expect("retained effects").into_boxed_slice(),
            },
            states: pairs
                .expect("retained state correspondence")
                .into_boxed_slice(),
        }))
    }
    pub(super) fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing);
        }
        Ok(())
    }
}
