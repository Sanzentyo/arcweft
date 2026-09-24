//! Canonical wire representation of the shared callable-state grammar.

use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer};
use crate::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableDefault, RuntimeCallableInputSource,
    RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
    RuntimeCallableParameterKind, RuntimeCallablePartialTransition, RuntimeCallablePosition,
    RuntimeCallableRetainedInput, RuntimeCallableRetainedRole, RuntimeCallableStateDefinition,
    RuntimeCallableTransition,
};
use crate::runtime_id::RuntimeCallableStateId;

impl Wire for RuntimeCallableStateId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.get().get().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let encoded = u32::read_wire(reader)?;
        let state = encoded
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(RuntimeCallableStateId::for_index)
            .ok_or_else(|| AwbcCodecError::InvalidMetadata {
                kind: "callable state",
                message: "state identity must be a valid nonzero index".to_owned(),
                offset,
            })?;
        Ok(state)
    }
}

impl Wire for RuntimeCallableParameterCoordinate {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.group.write_wire(writer)?;
        self.parameter.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            group: u32::read_wire(reader)?,
            parameter: u32::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeCallablePosition {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Unapplied => writer.write_u8(0),
            Self::WithinGroup { group, bound } => {
                writer.write_u8(1);
                group.write_wire(writer)?;
                bound.write_wire(writer)?;
            }
            Self::AfterGroup { completed } => {
                writer.write_u8(2);
                completed.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Unapplied,
            1 => Self::WithinGroup {
                group: u32::read_wire(reader)?,
                bound: Box::<[RuntimeCallableParameterCoordinate]>::read_wire(reader)?,
            },
            2 => Self::AfterGroup {
                completed: u32::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "callable position",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for RuntimeCallableRetainedRole {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Capture { position } => {
                writer.write_u8(0);
                position.write_wire(writer)?;
            }
            Self::Parameter(coordinate) => {
                writer.write_u8(1);
                coordinate.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Capture {
                position: u32::read_wire(reader)?,
            },
            1 => Self::Parameter(RuntimeCallableParameterCoordinate::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "callable retained role",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for RuntimeCallableParameterKind {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(match self {
            Self::Fixed => 0,
            Self::Rest => 1,
        });
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Fixed),
            1 => Ok(Self::Rest),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "callable parameter kind",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for RuntimeCallableInputSource {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Retained { position } => {
                writer.write_u8(0);
                position.write_wire(writer)?;
            }
            Self::Argument { position } => {
                writer.write_u8(1);
                position.write_wire(writer)?;
            }
            Self::Attached => writer.write_u8(2),
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Retained {
                position: u32::read_wire(reader)?,
            },
            1 => Self::Argument {
                position: u32::read_wire(reader)?,
            },
            2 => Self::Attached,
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "callable input source",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl<T: Wire> Wire for RuntimeCallableRetainedInput<T> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.role.write_wire(writer)?;
        self.ty.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            role: RuntimeCallableRetainedRole::read_wire(reader)?,
            ty: T::read_wire(reader)?,
        })
    }
}

impl<T: Wire> Wire for RuntimeCallableParameterInput<T> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.coordinate.write_wire(writer)?;
        self.kind.write_wire(writer)?;
        self.abi_ty.write_wire(writer)?;
        self.binding_ty.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            coordinate: RuntimeCallableParameterCoordinate::read_wire(reader)?,
            kind: RuntimeCallableParameterKind::read_wire(reader)?,
            abi_ty: T::read_wire(reader)?,
            binding_ty: T::read_wire(reader)?,
        })
    }
}

impl<F: Wire> Wire for RuntimeCallableDefault<F> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.function.write_wire(writer)?;
        self.captures.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            function: F::read_wire(reader)?,
            captures: Box::<[RuntimeCallableInputSource]>::read_wire(reader)?,
        })
    }
}

impl<T: Wire, F: Wire> Wire for RuntimeCallableAttachedContract<T, F> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::None => writer.write_u8(0),
            Self::Required { ty } => {
                writer.write_u8(1);
                ty.write_wire(writer)?;
            }
            Self::Optional { value, binding } => {
                writer.write_u8(2);
                value.write_wire(writer)?;
                binding.write_wire(writer)?;
            }
            Self::Defaulted { ty, default } => {
                writer.write_u8(3);
                ty.write_wire(writer)?;
                default.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::None,
            1 => Self::Required {
                ty: T::read_wire(reader)?,
            },
            2 => Self::Optional {
                value: T::read_wire(reader)?,
                binding: T::read_wire(reader)?,
            },
            3 => Self::Defaulted {
                ty: T::read_wire(reader)?,
                default: RuntimeCallableDefault::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "callable attached contract",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl<S: Wire> Wire for RuntimeCallablePartialTransition<S> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.parameters.write_wire(writer)?;
        self.state.write_wire(writer)?;
        self.values.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            parameters: Box::<[RuntimeCallableParameterCoordinate]>::read_wire(reader)?,
            state: S::read_wire(reader)?,
            values: Box::<[RuntimeCallableInputSource]>::read_wire(reader)?,
        })
    }
}

impl<F: Wire, S: Wire> Wire for RuntimeCallableTransition<F, S> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Retain { state, values } => {
                writer.write_u8(0);
                state.write_wire(writer)?;
                values.write_wire(writer)?;
            }
            Self::Invoke {
                function,
                captures,
                arguments,
            } => {
                writer.write_u8(1);
                function.write_wire(writer)?;
                captures.write_wire(writer)?;
                arguments.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Retain {
                state: S::read_wire(reader)?,
                values: Box::<[RuntimeCallableInputSource]>::read_wire(reader)?,
            },
            1 => Self::Invoke {
                function: F::read_wire(reader)?,
                captures: Box::<[RuntimeCallableInputSource]>::read_wire(reader)?,
                arguments: Box::<[RuntimeCallableInputSource]>::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "callable transition",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl<T: Wire, F: Wire> Wire for RuntimeCallableStateDefinition<T, F> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.function_type.write_wire(writer)?;
        self.origin.write_wire(writer)?;
        self.position.write_wire(writer)?;
        self.retained.write_wire(writer)?;
        self.parameters.write_wire(writer)?;
        self.result.write_wire(writer)?;
        self.attached.write_wire(writer)?;
        self.transition.write_wire(writer)?;
        self.partials.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            function_type: T::read_wire(reader)?,
            origin: RuntimeCallableStateId::read_wire(reader)?,
            position: RuntimeCallablePosition::read_wire(reader)?,
            retained: Box::<[RuntimeCallableRetainedInput<T>]>::read_wire(reader)?,
            parameters: Box::<[RuntimeCallableParameterInput<T>]>::read_wire(reader)?,
            result: T::read_wire(reader)?,
            attached: RuntimeCallableAttachedContract::read_wire(reader)?,
            transition: RuntimeCallableTransition::read_wire(reader)?,
            partials: Box::<[RuntimeCallablePartialTransition<RuntimeCallableStateId>]>::read_wire(
                reader,
            )?,
        })
    }
}

impl<T: Wire> Wire for Box<[T]> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_table(self.as_ref())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Vec::<T>::read_wire(reader).map(Vec::into_boxed_slice)
    }
}
