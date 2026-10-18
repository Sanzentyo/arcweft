//! The retained phase creates the existing seed algebra directly. The framing
//! phase owns no graph rows, lists or surrogate type/function definitions.

use super::{Counts, DOMAIN, Error, Footprint, RuntimeCallableSourceLimits, limit, reference};
use crate::pattern::RuntimeSemanticTypeId;
use crate::plan::type_table::RuntimePlanTypeTableBuilder;
use crate::plan::{
    RuntimeCallableAttachedContract as Attached, RuntimeCallableDefault as Default,
    RuntimeCallableInputSource as Input, RuntimeCallableParameterCoordinate as Coordinate,
    RuntimeCallableParameterInput as Parameter, RuntimeCallableParameterKind as Kind,
    RuntimeCallablePartialTransition as Partial, RuntimeCallablePosition as Position,
    RuntimeCallableRetainedInput as Retained, RuntimeCallableRetainedRole as Role,
    RuntimeCallableStateDefinition, RuntimeCallableStateSeed, RuntimeCallableStateSeedId,
    RuntimeCallableTransition as Transition, RuntimeFunctionSiteSeedId,
};
use crate::runtime_id::{RuntimeCallableStateId, RuntimePlanTypeId};

pub(super) struct Owner<'a> {
    pub(super) types: &'a RuntimePlanTypeTableBuilder,
    pub(super) functions: &'a [RuntimeFunctionSiteSeedId],
    pub(super) states: &'a [RuntimeCallableStateSeedId],
}
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    counts: Counts,
}
impl<'a> Reader<'a> {
    pub(super) fn header_count(
        bytes: &[u8],
        limits: RuntimeCallableSourceLimits,
    ) -> Result<usize, Error> {
        limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
        let header = bytes
            .get(..DOMAIN.len() + 5)
            .ok_or(Error::Truncated { offset: 0 })?;
        if &header[..DOMAIN.len()] != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = header[DOMAIN.len()];
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let rows = u32::from_le_bytes(
            header[DOMAIN.len() + 1..]
                .try_into()
                .expect("source row width"),
        ) as usize;
        limit("callable states", rows, limits.rows as usize)?;
        Ok(rows)
    }
    pub(super) fn new(
        bytes: &'a [u8],
        limits: RuntimeCallableSourceLimits,
        rows: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            bytes,
            offset: 0,
            counts: Counts::new(limits, rows)?,
        })
    }
    pub(super) fn header(&mut self) -> Result<(), Error> {
        let rows = Self::header_count(self.bytes, self.counts.limits)?;
        debug_assert_eq!(rows, self.counts.footprint.rows);
        self.offset = DOMAIN.len() + 5;
        Ok(())
    }
    fn bytes(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Arithmetic)?;
        let value = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn count(&mut self) -> Result<usize, Error> {
        let count = self.u32()? as usize;
        limit(
            "source elements",
            count,
            self.counts.limits.elements as usize,
        )?;
        Ok(count)
    }
    fn ty(&mut self, owner: Option<&Owner<'_>>) -> Result<Option<RuntimeSemanticTypeId>, Error> {
        let id = self.u32()?;
        reference("type", id as usize, self.counts.limits.type_count, true)?;
        Ok(owner.map(|owner| {
            owner
                .types
                .get(RuntimePlanTypeId::from_accepted_ordinal(
                    std::num::NonZeroU32::new(id).expect("source type is checked"),
                ))
                .expect("source type belongs to its actual table")
                .semantic_identity()
        }))
    }
    fn function(
        &mut self,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<RuntimeFunctionSiteSeedId>, Error> {
        let id = self.u32()? as usize;
        reference("function", id, self.counts.limits.function_count, true)?;
        Ok(owner.map(|owner| owner.functions[id - 1].clone()))
    }
    fn state(
        &mut self,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<RuntimeCallableStateSeedId>, Error> {
        let id = self.u32()? as usize;
        reference("state", id, self.counts.footprint.rows, false)?;
        Ok(owner.map(|owner| owner.states[id].clone()))
    }
    fn coordinate(&mut self) -> Result<Coordinate, Error> {
        Ok(Coordinate {
            group: self.u32()?,
            parameter: self.u32()?,
        })
    }
    fn list<T>(
        &mut self,
        count: usize,
        retain: bool,
        mut read: impl FnMut(&mut Self) -> Result<Option<T>, Error>,
    ) -> Result<Option<Box<[T]>>, Error> {
        let mut values = retain.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let value = read(self)?;
            if let Some(values) = &mut values {
                values.push(value.expect("retaining under the original owner"));
            }
        }
        Ok(values.map(Vec::into_boxed_slice))
    }
    fn coordinates(&mut self, retain: bool) -> Result<Option<Box<[Coordinate]>>, Error> {
        let count = self.count()?;
        self.counts.array::<Coordinate>(count)?;
        self.list(count, retain, |reader| {
            Ok(retain.then_some(reader.coordinate()?))
        })
    }
    fn input(&mut self) -> Result<Input, Error> {
        Ok(match self.byte()? {
            0 => Input::Retained {
                position: self.u32()?,
            },
            1 => Input::Argument {
                position: self.u32()?,
            },
            2 => Input::Attached,
            tag => {
                return Err(Error::Tag {
                    kind: "input source",
                    tag,
                });
            }
        })
    }
    fn inputs(&mut self, retain: bool) -> Result<Option<Box<[Input]>>, Error> {
        let count = self.count()?;
        self.counts.array::<Input>(count)?;
        self.list(
            count,
            retain,
            |reader| Ok(retain.then_some(reader.input()?)),
        )
    }
    fn position(&mut self, retain: bool) -> Result<Option<Position>, Error> {
        Ok(match self.byte()? {
            0 => retain.then_some(Position::Unapplied),
            1 => {
                let group = self.u32()?;
                self.coordinates(retain)?
                    .map(|bound| Position::WithinGroup { group, bound })
            }
            2 => retain.then_some(Position::AfterGroup {
                completed: self.u32()?,
            }),
            tag => {
                return Err(Error::Tag {
                    kind: "position",
                    tag,
                });
            }
        })
    }
    fn retained(
        &mut self,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<Box<[Retained<RuntimeSemanticTypeId>]>>, Error> {
        let count = self.count()?;
        self.counts
            .mapped::<Retained<RuntimeSemanticTypeId>, Retained<RuntimePlanTypeId>>(count)?;
        self.list(count, owner.is_some(), |reader| {
            let role = match reader.byte()? {
                0 => Role::Capture {
                    position: reader.u32()?,
                },
                1 => Role::Parameter(reader.coordinate()?),
                tag => {
                    return Err(Error::Tag {
                        kind: "retained role",
                        tag,
                    });
                }
            };
            Ok(reader.ty(owner)?.map(|ty| Retained { role, ty }))
        })
    }
    fn parameters(
        &mut self,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<Box<[Parameter<RuntimeSemanticTypeId>]>>, Error> {
        let count = self.count()?;
        self.counts
            .mapped::<Parameter<RuntimeSemanticTypeId>, Parameter<RuntimePlanTypeId>>(count)?;
        self.list(count, owner.is_some(), |reader| {
            let coordinate = reader.coordinate()?;
            let kind = match reader.byte()? {
                0 => Kind::Fixed,
                1 => Kind::Rest,
                tag => {
                    return Err(Error::Tag {
                        kind: "parameter kind",
                        tag,
                    });
                }
            };
            let abi_ty = reader.ty(owner)?;
            let binding_ty = reader.ty(owner)?;
            Ok(abi_ty
                .zip(binding_ty)
                .map(|(abi_ty, binding_ty)| Parameter {
                    coordinate,
                    kind,
                    abi_ty,
                    binding_ty,
                }))
        })
    }
    fn attached(
        &mut self,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<Attached<RuntimeSemanticTypeId, RuntimeFunctionSiteSeedId>>, Error> {
        Ok(match self.byte()? {
            0 => owner.map(|_| Attached::None),
            1 => self.ty(owner)?.map(|ty| Attached::Required { ty }),
            2 => {
                let value = self.ty(owner)?;
                let binding = self.ty(owner)?;
                value
                    .zip(binding)
                    .map(|(value, binding)| Attached::Optional { value, binding })
            }
            3 => {
                let ty = self.ty(owner)?;
                let default = match self.byte()? {
                    0 => owner.map(|_| Default::RequiresSpecialization),
                    1 => {
                        let function = self.function(owner)?;
                        let captures = self.inputs(owner.is_some())?;
                        function
                            .zip(captures)
                            .map(|(function, captures)| Default::Body { function, captures })
                    }
                    tag => {
                        return Err(Error::Tag {
                            kind: "attached default",
                            tag,
                        });
                    }
                };
                ty.zip(default)
                    .map(|(ty, default)| Attached::Defaulted { ty, default })
            }
            tag => {
                return Err(Error::Tag {
                    kind: "attached contract",
                    tag,
                });
            }
        })
    }
    fn transition(
        &mut self,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<Transition<RuntimeFunctionSiteSeedId, RuntimeCallableStateSeedId>>, Error>
    {
        Ok(match self.byte()? {
            0 => owner.map(|_| Transition::RequiresSpecialization),
            1 => {
                let state = self.state(owner)?;
                let values = self.inputs(owner.is_some())?;
                state
                    .zip(values)
                    .map(|(state, values)| Transition::Retain { state, values })
            }
            2 => {
                let function = self.function(owner)?;
                let captures = self.inputs(owner.is_some())?;
                let arguments = self.inputs(owner.is_some())?;
                function
                    .zip(captures)
                    .zip(arguments)
                    .map(|((function, captures), arguments)| Transition::Invoke {
                        function,
                        captures,
                        arguments,
                    })
            }
            tag => {
                return Err(Error::Tag {
                    kind: "transition",
                    tag,
                });
            }
        })
    }
    fn partials(
        &mut self,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<Box<[Partial<RuntimeCallableStateSeedId>]>>, Error> {
        let count = self.count()?;
        self.counts
            .mapped::<Partial<RuntimeCallableStateSeedId>, Partial<RuntimeCallableStateId>>(
                count,
            )?;
        self.list(count, owner.is_some(), |reader| {
            let parameters = reader.coordinates(owner.is_some())?;
            let state = reader.state(owner)?;
            let values = reader.inputs(owner.is_some())?;
            Ok(parameters
                .zip(state)
                .zip(values)
                .map(|((parameters, state), values)| Partial {
                    parameters,
                    state,
                    values,
                }))
        })
    }
    pub(super) fn row(
        &mut self,
        index: usize,
        owner: Option<&Owner<'_>>,
    ) -> Result<Option<RuntimeCallableStateSeed>, Error> {
        let expected = u32::try_from(index).map_err(|_| Error::Arithmetic)?;
        let actual = self.u32()?;
        if actual != expected {
            return Err(Error::Ordinal { expected, actual });
        }
        let function_type = self.ty(owner)?;
        let origin = self.state(owner)?;
        let position = self.position(owner.is_some())?;
        let retained = self.retained(owner)?;
        let parameters = self.parameters(owner)?;
        let result = self.ty(owner)?;
        let attached = self.attached(owner)?;
        let transition = self.transition(owner)?;
        let partials = self.partials(owner)?;
        Ok(owner.map(|_| RuntimeCallableStateDefinition {
            function_type: function_type.expect("owned type"),
            origin: origin.expect("owned state"),
            position: position.expect("owned position"),
            retained: retained.expect("owned retained"),
            parameters: parameters.expect("owned parameters"),
            result: result.expect("owned result"),
            attached: attached.expect("owned attached contract"),
            transition: transition.expect("owned transition"),
            partials: partials.expect("owned partials"),
        }))
    }
    pub(super) const fn footprint(&self) -> Footprint {
        self.counts.footprint
    }
    pub(super) fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
}
