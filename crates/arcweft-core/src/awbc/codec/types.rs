use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer, wire_enum, wire_id};
use crate::awbc::schema::{
    AwbcAudioCommandId, AwbcBlockId, AwbcChoiceId, AwbcChoiceOptionId, AwbcConstant,
    AwbcConstantId, AwbcContentUnitId, AwbcDigest, AwbcDisplayMapId, AwbcEffectPlanId,
    AwbcEffectSet, AwbcEffectSetId, AwbcEntryId, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot,
    AwbcFrameSlotRole, AwbcFunctionFlags, AwbcFunctionId, AwbcHeader, AwbcHostCallId,
    AwbcInstructionId, AwbcIntrinsicId, AwbcLineTaskGroupId, AwbcLineTaskNodeId, AwbcMatchArmId,
    AwbcPatternId, AwbcPureHelperId, AwbcRecordField, AwbcRegisterId, AwbcResourceId,
    AwbcResumePointId, AwbcRuntimeType, AwbcScopeId, AwbcSignature, AwbcSignatureId,
    AwbcSignedIntKind, AwbcSourceMapId, AwbcSourcePlanId, AwbcStreamPlanId, AwbcStringId,
    AwbcTableRange, AwbcTaskPlanId, AwbcTraitMethodId, AwbcTypeId, AwbcUnsignedIntKind,
    AwbcVariantCase,
};

wire_id!(
    AwbcStringId,
    AwbcTypeId,
    AwbcConstantId,
    AwbcEffectSetId,
    AwbcSignatureId,
    AwbcFrameLayoutId,
    AwbcFunctionId,
    AwbcBlockId,
    AwbcInstructionId,
    AwbcRegisterId,
    AwbcScopeId,
    AwbcResumePointId,
    AwbcPatternId,
    AwbcMatchArmId,
    AwbcChoiceId,
    AwbcChoiceOptionId,
    AwbcIntrinsicId,
    AwbcHostCallId,
    AwbcTaskPlanId,
    AwbcAudioCommandId,
    AwbcEffectPlanId,
    AwbcContentUnitId,
    AwbcLineTaskGroupId,
    AwbcLineTaskNodeId,
    AwbcStreamPlanId,
    AwbcSourcePlanId,
    AwbcPureHelperId,
    AwbcTraitMethodId,
    AwbcDisplayMapId,
    AwbcSourceMapId,
    AwbcResourceId,
    AwbcEntryId,
);

impl Wire for AwbcDigest {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.0.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        <[u8; 32]>::read_wire(reader).map(Self)
    }
}

impl Wire for AwbcTableRange {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.start.write_wire(writer)?;
        self.len.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            start: u32::read_wire(reader)?,
            len: u32::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcHeader {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.abi_version.write_wire(writer)?;
        self.minimum_runtime_abi.write_wire(writer)?;
        self.feature_bits.write_wire(writer)?;
        self.runtime_layout_digest.write_wire(writer)?;
        self.host_abi_digest.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            abi_version: u32::read_wire(reader)?,
            minimum_runtime_abi: u32::read_wire(reader)?,
            feature_bits: u64::read_wire(reader)?,
            runtime_layout_digest: AwbcDigest::read_wire(reader)?,
            host_abi_digest: AwbcDigest::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcSignedIntKind, "signed integer kind", {
    0 => AwbcSignedIntKind::I8,
    1 => AwbcSignedIntKind::I16,
    2 => AwbcSignedIntKind::I32,
    3 => AwbcSignedIntKind::I64,
    4 => AwbcSignedIntKind::I128,
    5 => AwbcSignedIntKind::ISize,
});

wire_enum!(AwbcUnsignedIntKind, "unsigned integer kind", {
    0 => AwbcUnsignedIntKind::U8,
    1 => AwbcUnsignedIntKind::U16,
    2 => AwbcUnsignedIntKind::U32,
    3 => AwbcUnsignedIntKind::U64,
    4 => AwbcUnsignedIntKind::U128,
    5 => AwbcUnsignedIntKind::USize,
});

impl Wire for AwbcRecordField {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.name.write_wire(writer)?;
        self.ty.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            name: AwbcStringId::read_wire(reader)?,
            ty: AwbcTypeId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcVariantCase {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.name.write_wire(writer)?;
        self.payload.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            name: AwbcStringId::read_wire(reader)?,
            payload: Option::<AwbcTypeId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcRuntimeType {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Unit => writer.write_u8(0),
            Self::Bool => writer.write_u8(1),
            Self::Int(kind) => {
                writer.write_u8(2);
                kind.write_wire(writer)?;
            }
            Self::UInt(kind) => {
                writer.write_u8(3);
                kind.write_wire(writer)?;
            }
            Self::F32 => writer.write_u8(4),
            Self::F64 => writer.write_u8(5),
            Self::String => writer.write_u8(6),
            Self::Char => writer.write_u8(7),
            Self::Duration => writer.write_u8(8),
            Self::EntityRef => writer.write_u8(9),
            Self::Tuple(items) => {
                writer.write_u8(10);
                items.write_wire(writer)?;
            }
            Self::Sequence(item) => {
                writer.write_u8(11);
                item.write_wire(writer)?;
            }
            Self::Record { public_id, fields } => {
                writer.write_u8(12);
                public_id.write_wire(writer)?;
                fields.write_wire(writer)?;
            }
            Self::Variant { public_id, cases } => {
                writer.write_u8(13);
                public_id.write_wire(writer)?;
                cases.write_wire(writer)?;
            }
            Self::MatrixF32 => writer.write_u8(14),
            Self::MatrixF64 => writer.write_u8(15),
            Self::TensorF32 => writer.write_u8(16),
            Self::TensorF64 => writer.write_u8(17),
            Self::TaskHandle => writer.write_u8(18),
            Self::NeedHandle => writer.write_u8(19),
            Self::Dynamic => writer.write_u8(20),
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Unit,
            1 => Self::Bool,
            2 => Self::Int(AwbcSignedIntKind::read_wire(reader)?),
            3 => Self::UInt(AwbcUnsignedIntKind::read_wire(reader)?),
            4 => Self::F32,
            5 => Self::F64,
            6 => Self::String,
            7 => Self::Char,
            8 => Self::Duration,
            9 => Self::EntityRef,
            10 => Self::Tuple(Vec::<AwbcTypeId>::read_wire(reader)?),
            11 => Self::Sequence(AwbcTypeId::read_wire(reader)?),
            12 => Self::Record {
                public_id: Option::<AwbcStringId>::read_wire(reader)?,
                fields: Vec::<AwbcRecordField>::read_wire(reader)?,
            },
            13 => Self::Variant {
                public_id: Option::<AwbcStringId>::read_wire(reader)?,
                cases: Vec::<AwbcVariantCase>::read_wire(reader)?,
            },
            14 => Self::MatrixF32,
            15 => Self::MatrixF64,
            16 => Self::TensorF32,
            17 => Self::TensorF64,
            18 => Self::TaskHandle,
            19 => Self::NeedHandle,
            20 => Self::Dynamic,
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "runtime type",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcConstant {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Unit => writer.write_u8(0),
            Self::Bool(value) => {
                writer.write_u8(1);
                value.write_wire(writer)?;
            }
            Self::Int { kind, bits } => {
                writer.write_u8(2);
                kind.write_wire(writer)?;
                bits.write_wire(writer)?;
            }
            Self::UInt { kind, bits } => {
                writer.write_u8(3);
                kind.write_wire(writer)?;
                bits.write_wire(writer)?;
            }
            Self::F32Bits(value) => {
                writer.write_u8(4);
                writer.write_u32_le(*value);
            }
            Self::F64Bits(value) => {
                writer.write_u8(5);
                writer.write_u64_le(*value);
            }
            Self::String(value) => {
                writer.write_u8(6);
                value.write_wire(writer)?;
            }
            Self::Char(value) => {
                writer.write_u8(7);
                value.write_wire(writer)?;
            }
            Self::DurationNanos(value) => {
                writer.write_u8(8);
                value.write_wire(writer)?;
            }
            Self::EntityRef(value) => {
                writer.write_u8(9);
                value.write_wire(writer)?;
            }
            Self::Tuple(items) => {
                writer.write_u8(10);
                items.write_wire(writer)?;
            }
            Self::Sequence(items) => {
                writer.write_u8(11);
                items.write_wire(writer)?;
            }
            Self::Record {
                ty,
                field_names,
                fields,
            } => {
                writer.write_u8(12);
                ty.write_wire(writer)?;
                field_names.write_wire(writer)?;
                fields.write_wire(writer)?;
            }
            Self::Variant {
                ty,
                case,
                case_name,
                payload,
            } => {
                writer.write_u8(13);
                ty.write_wire(writer)?;
                case.write_wire(writer)?;
                case_name.write_wire(writer)?;
                payload.write_wire(writer)?;
            }
            Self::Range {
                start,
                end,
                inclusive,
            } => {
                writer.write_u8(17);
                start.write_wire(writer)?;
                end.write_wire(writer)?;
                inclusive.write_wire(writer)?;
            }
            Self::Bytes(bytes) => {
                writer.write_u8(14);
                bytes.write_wire(writer)?;
            }
            Self::TensorF32 { shape, values } => write_tensor_f32_constant(writer, shape, values)?,
            Self::TensorF64 { shape, values } => write_tensor_f64_constant(writer, shape, values)?,
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Unit,
            1 => Self::Bool(bool::read_wire(reader)?),
            2 => Self::Int {
                kind: AwbcSignedIntKind::read_wire(reader)?,
                bits: <[u8; 16]>::read_wire(reader)?,
            },
            3 => Self::UInt {
                kind: AwbcUnsignedIntKind::read_wire(reader)?,
                bits: <[u8; 16]>::read_wire(reader)?,
            },
            4 => Self::F32Bits(reader.read_u32_le()?),
            5 => Self::F64Bits(reader.read_u64_le()?),
            6 => Self::String(AwbcStringId::read_wire(reader)?),
            7 => Self::Char(u32::read_wire(reader)?),
            8 => Self::DurationNanos(u64::read_wire(reader)?),
            9 => Self::EntityRef(AwbcStringId::read_wire(reader)?),
            10 => Self::Tuple(Vec::<AwbcConstantId>::read_wire(reader)?),
            11 => Self::Sequence(Vec::<AwbcConstantId>::read_wire(reader)?),
            12 => Self::Record {
                ty: AwbcTypeId::read_wire(reader)?,
                field_names: Vec::<AwbcStringId>::read_wire(reader)?,
                fields: Vec::<AwbcConstantId>::read_wire(reader)?,
            },
            13 => Self::Variant {
                ty: AwbcTypeId::read_wire(reader)?,
                case: u32::read_wire(reader)?,
                case_name: AwbcStringId::read_wire(reader)?,
                payload: Option::<AwbcConstantId>::read_wire(reader)?,
            },
            17 => Self::Range {
                start: Option::<AwbcConstantId>::read_wire(reader)?,
                end: Option::<AwbcConstantId>::read_wire(reader)?,
                inclusive: bool::read_wire(reader)?,
            },
            14 => Self::Bytes(Vec::<u8>::read_wire(reader)?),
            15 => {
                let shape = Vec::<u32>::read_wire(reader)?;
                let len = reader.read_len()?;
                Reader::check_limit("tensor_elements", len, reader.budget().tensor_elements)?;
                let values = (0..len)
                    .map(|_| reader.read_u32_le())
                    .collect::<Result<Vec<_>, _>>()?;
                Self::TensorF32 { shape, values }
            }
            16 => {
                let shape = Vec::<u32>::read_wire(reader)?;
                let len = reader.read_len()?;
                Reader::check_limit("tensor_elements", len, reader.budget().tensor_elements)?;
                let values = (0..len)
                    .map(|_| reader.read_u64_le())
                    .collect::<Result<Vec<_>, _>>()?;
                Self::TensorF64 { shape, values }
            }
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "constant",
                    tag,
                    offset,
                });
            }
        })
    }
}

fn write_tensor_f32_constant(
    writer: &mut Writer,
    shape: &[u32],
    values: &[u32],
) -> Result<(), AwbcCodecError> {
    writer.write_u8(15);
    write_u32_slice(writer, shape)?;
    writer.write_len(values.len())?;
    for value in values {
        writer.write_u32_le(*value);
    }
    Ok(())
}

fn write_tensor_f64_constant(
    writer: &mut Writer,
    shape: &[u32],
    values: &[u64],
) -> Result<(), AwbcCodecError> {
    writer.write_u8(16);
    write_u32_slice(writer, shape)?;
    writer.write_len(values.len())?;
    for value in values {
        writer.write_u64_le(*value);
    }
    Ok(())
}

fn write_u32_slice(writer: &mut Writer, values: &[u32]) -> Result<(), AwbcCodecError> {
    writer.write_len(values.len())?;
    for value in values {
        writer.write_u32_le(*value);
    }
    Ok(())
}

impl Wire for AwbcEffectSet {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.effects.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            effects: Vec::<AwbcStringId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcSignature {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.params.write_wire(writer)?;
        self.result.write_wire(writer)?;
        self.effects.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            params: Vec::<AwbcTypeId>::read_wire(reader)?,
            result: Option::<AwbcTypeId>::read_wire(reader)?,
            effects: AwbcEffectSetId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFrameLayout {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.slots.write_wire(writer)?;
        self.max_scope_depth.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            slots: Vec::<AwbcFrameSlot>::read_wire(reader)?,
            max_scope_depth: u32::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFrameSlot {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.name.write_wire(writer)?;
        self.ty.write_wire(writer)?;
        self.role.write_wire(writer)?;
        self.scope_depth.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            name: Option::<AwbcStringId>::read_wire(reader)?,
            ty: AwbcTypeId::read_wire(reader)?,
            role: AwbcFrameSlotRole::read_wire(reader)?,
            scope_depth: u32::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcFrameSlotRole, "frame slot role", {
    0 => AwbcFrameSlotRole::Parameter,
    1 => AwbcFrameSlotRole::Local,
    2 => AwbcFrameSlotRole::Temporary,
    3 => AwbcFrameSlotRole::ReturnValue,
    4 => AwbcFrameSlotRole::RuntimeState,
});

impl Wire for AwbcFunctionFlags {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.0.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        u32::read_wire(reader).map(Self)
    }
}
