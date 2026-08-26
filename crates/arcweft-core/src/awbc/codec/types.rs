use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer, wire_enum, wire_id};
use crate::awbc::schema::{
    AwbcAgentTypeShape, AwbcAudioCommandId, AwbcBlockId, AwbcChoiceId, AwbcChoiceOptionId,
    AwbcConstant, AwbcConstantId, AwbcContentUnitId, AwbcDigest, AwbcDisplayMapId,
    AwbcEffectPlanId, AwbcEffectSet, AwbcEffectSetId, AwbcEntryId, AwbcFrameLayout,
    AwbcFrameLayoutId, AwbcFrameSlot, AwbcFrameSlotRole, AwbcFunctionFlags, AwbcFunctionId,
    AwbcHeader, AwbcHostCallId, AwbcInstructionId, AwbcIntrinsicId, AwbcLineTaskGroupId,
    AwbcLineTaskNodeId, AwbcMatchArmId, AwbcPatternId, AwbcPureHelperId, AwbcRecordField,
    AwbcRegisterId, AwbcResourceId, AwbcResumePointId, AwbcRuntimeType, AwbcRuntimeTypeShape,
    AwbcScopeId, AwbcSignature, AwbcSignatureId, AwbcSignedIntKind, AwbcSourceMapId,
    AwbcStreamPlanId, AwbcStringId, AwbcTableRange, AwbcTaskPlanId, AwbcTraitMethodId, AwbcTypeId,
    AwbcUnsignedIntKind, AwbcVariantCase, AwbcVariantIdentity,
};
use crate::pattern::RuntimeOpaqueTypeAdmission;
use crate::plan::RuntimeAgentOperationalType;
use crate::value::{RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

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

impl Wire for AwbcVariantIdentity {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Nominal { public_id } => {
                writer.write_u8(0);
                public_id.write_wire(writer)
            }
            Self::Option => {
                writer.write_u8(1);
                Ok(())
            }
            Self::Result => {
                writer.write_u8(2);
                Ok(())
            }
        }
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Nominal {
                public_id: AwbcStringId::read_wire(reader)?,
            }),
            1 => Ok(Self::Option),
            2 => Ok(Self::Result),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "variant identity",
                tag,
                offset,
            }),
        }
    }
}

wire_enum!(RuntimeOpaqueTypeAdmission, "opaque type admission", {
    0 => RuntimeOpaqueTypeAdmission::ExactIdentity,
    1 => RuntimeOpaqueTypeAdmission::ProducerWide,
});

wire_enum!(RuntimeHandleKind, "runtime handle kind", {
    0 => RuntimeHandleKind::StageActor,
    1 => RuntimeHandleKind::Cue,
    2 => RuntimeHandleKind::Voice,
});

impl Wire for RuntimeOpaqueValueClass {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Plain => writer.write_u8(0),
            Self::AffineHandle(kind) => {
                writer.write_u8(1);
                kind.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Plain),
            1 => RuntimeHandleKind::read_wire(reader).map(Self::AffineHandle),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "opaque value class",
                tag,
                offset,
            }),
        }
    }
}

wire_enum!(RuntimeOpaquePersistence, "opaque persistence", {
    0 => RuntimeOpaquePersistence::ConstantAndSnapshot,
    1 => RuntimeOpaquePersistence::SnapshotOnly,
});

impl Wire for AwbcRuntimeType {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.semantic_identity().write_wire(writer)?;
        write_runtime_type_shape(self.shape(), writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let semantic_identity = crate::pattern::RuntimeSemanticTypeId::read_wire(reader)?;
        let offset = reader.offset();
        let shape = match reader.read_u8()? {
            0 => AwbcRuntimeTypeShape::Unit,
            1 => AwbcRuntimeTypeShape::Bool,
            2 => AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::read_wire(reader)?),
            3 => AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::read_wire(reader)?),
            4 => AwbcRuntimeTypeShape::F32,
            5 => AwbcRuntimeTypeShape::F64,
            6 => AwbcRuntimeTypeShape::String,
            7 => AwbcRuntimeTypeShape::Char,
            8 => AwbcRuntimeTypeShape::Duration,
            9 => AwbcRuntimeTypeShape::EntityRef,
            10 => AwbcRuntimeTypeShape::Tuple(Vec::<AwbcTypeId>::read_wire(reader)?),
            11 => AwbcRuntimeTypeShape::Sequence(AwbcTypeId::read_wire(reader)?),
            12 => AwbcRuntimeTypeShape::Record {
                public_id: Option::<AwbcStringId>::read_wire(reader)?,
                fields: Vec::<AwbcRecordField>::read_wire(reader)?,
            },
            13 => AwbcRuntimeTypeShape::Variant {
                owner: AwbcVariantIdentity::read_wire(reader)?,
                arguments: Vec::<AwbcTypeId>::read_wire(reader)?,
                cases: Vec::<AwbcVariantCase>::read_wire(reader)?,
            },
            14 => AwbcRuntimeTypeShape::MatrixF32,
            15 => AwbcRuntimeTypeShape::MatrixF64,
            16 => AwbcRuntimeTypeShape::TensorF32,
            17 => AwbcRuntimeTypeShape::TensorF64,
            18 => AwbcRuntimeTypeShape::Task(AwbcTypeId::read_wire(reader)?),
            19 => AwbcRuntimeTypeShape::Need(AwbcTypeId::read_wire(reader)?),
            20 => AwbcRuntimeTypeShape::Dynamic,
            21 => AwbcRuntimeTypeShape::Choice(Vec::<AwbcTypeId>::read_wire(reader)?),
            22 => AwbcRuntimeTypeShape::Nominal {
                public_id: AwbcStringId::read_wire(reader)?,
                layout: <[u8; 32]>::read_wire(reader)?,
                arguments: Vec::<AwbcTypeId>::read_wire(reader)?,
            },
            23 => AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId::read_wire(reader)?,
                admission: RuntimeOpaqueTypeAdmission::read_wire(reader)?,
                value_class: RuntimeOpaqueValueClass::read_wire(reader)?,
                persistence: RuntimeOpaquePersistence::read_wire(reader)?,
                arguments: Vec::<AwbcTypeId>::read_wire(reader)?,
            },
            24 => AwbcRuntimeTypeShape::NominalRecord {
                public_id: AwbcStringId::read_wire(reader)?,
                layout: <[u8; 32]>::read_wire(reader)?,
                arguments: Vec::<AwbcTypeId>::read_wire(reader)?,
                fields: Vec::<AwbcRecordField>::read_wire(reader)?,
            },
            25 => AwbcRuntimeTypeShape::Bytes,
            26 => AwbcRuntimeTypeShape::Never,
            27 => AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::read_wire(reader)?),
            28 => AwbcRuntimeTypeShape::Progress,
            29 => AwbcRuntimeTypeShape::Range(AwbcTypeId::read_wire(reader)?),
            30 => AwbcRuntimeTypeShape::Iterator(AwbcTypeId::read_wire(reader)?),
            31 => AwbcRuntimeTypeShape::Array {
                item: AwbcTypeId::read_wire(reader)?,
                length: u64::read_wire(reader)?,
            },
            32 => AwbcRuntimeTypeShape::Map {
                key: AwbcTypeId::read_wire(reader)?,
                value: AwbcTypeId::read_wire(reader)?,
            },
            33 => AwbcRuntimeTypeShape::Stream {
                item: AwbcTypeId::read_wire(reader)?,
                error: AwbcTypeId::read_wire(reader)?,
            },
            34 => AwbcRuntimeTypeShape::Shared(AwbcTypeId::read_wire(reader)?),
            35 => AwbcRuntimeTypeShape::Reference(AwbcTypeId::read_wire(reader)?),
            36 => AwbcRuntimeTypeShape::Function {
                parameters: Vec::<AwbcTypeId>::read_wire(reader)?,
                result: AwbcTypeId::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "runtime type",
                    tag,
                    offset,
                });
            }
        };
        Ok(AwbcRuntimeType::new(semantic_identity, shape))
    }
}

fn write_runtime_type_shape(
    shape: &AwbcRuntimeTypeShape,
    writer: &mut Writer,
) -> Result<(), AwbcCodecError> {
    match shape {
        AwbcRuntimeTypeShape::Unit => {
            writer.write_u8(0);
            Ok(())
        }
        AwbcRuntimeTypeShape::Bool => {
            writer.write_u8(1);
            Ok(())
        }
        AwbcRuntimeTypeShape::Int(kind) => write_tagged(writer, 2, kind),
        AwbcRuntimeTypeShape::UInt(kind) => write_tagged(writer, 3, kind),
        AwbcRuntimeTypeShape::F32 => {
            writer.write_u8(4);
            Ok(())
        }
        AwbcRuntimeTypeShape::F64 => {
            writer.write_u8(5);
            Ok(())
        }
        AwbcRuntimeTypeShape::String => {
            writer.write_u8(6);
            Ok(())
        }
        AwbcRuntimeTypeShape::Char => {
            writer.write_u8(7);
            Ok(())
        }
        AwbcRuntimeTypeShape::Duration => {
            writer.write_u8(8);
            Ok(())
        }
        AwbcRuntimeTypeShape::EntityRef => {
            writer.write_u8(9);
            Ok(())
        }
        AwbcRuntimeTypeShape::MatrixF32 => {
            writer.write_u8(14);
            Ok(())
        }
        AwbcRuntimeTypeShape::MatrixF64 => {
            writer.write_u8(15);
            Ok(())
        }
        AwbcRuntimeTypeShape::TensorF32 => {
            writer.write_u8(16);
            Ok(())
        }
        AwbcRuntimeTypeShape::TensorF64 => {
            writer.write_u8(17);
            Ok(())
        }
        AwbcRuntimeTypeShape::Dynamic => {
            writer.write_u8(20);
            Ok(())
        }
        AwbcRuntimeTypeShape::Bytes => {
            writer.write_u8(25);
            Ok(())
        }
        AwbcRuntimeTypeShape::Never => {
            writer.write_u8(26);
            Ok(())
        }
        AwbcRuntimeTypeShape::Progress => {
            writer.write_u8(28);
            Ok(())
        }
        _ => write_runtime_type_composite_shape(shape, writer),
    }
}

fn write_runtime_type_composite_shape(
    shape: &AwbcRuntimeTypeShape,
    writer: &mut Writer,
) -> Result<(), AwbcCodecError> {
    match shape {
        AwbcRuntimeTypeShape::Tuple(items) => write_tagged(writer, 10, items),
        AwbcRuntimeTypeShape::Sequence(item) => write_tagged(writer, 11, item),
        AwbcRuntimeTypeShape::Record { public_id, fields } => {
            write_tagged_fields(writer, 12, |writer| {
                public_id.write_wire(writer)?;
                fields.write_wire(writer)
            })
        }
        AwbcRuntimeTypeShape::Variant {
            owner,
            arguments,
            cases,
        } => write_tagged_fields(writer, 13, |writer| {
            owner.write_wire(writer)?;
            arguments.write_wire(writer)?;
            cases.write_wire(writer)
        }),
        AwbcRuntimeTypeShape::Task(result) => write_tagged(writer, 18, result),
        AwbcRuntimeTypeShape::Need(value) => write_tagged(writer, 19, value),
        AwbcRuntimeTypeShape::Choice(alternatives) => write_tagged(writer, 21, alternatives),
        AwbcRuntimeTypeShape::Nominal {
            public_id,
            layout,
            arguments,
        } => write_tagged_fields(writer, 22, |writer| {
            public_id.write_wire(writer)?;
            layout.write_wire(writer)?;
            arguments.write_wire(writer)
        }),
        AwbcRuntimeTypeShape::Opaque {
            producer,
            admission,
            value_class,
            persistence,
            arguments,
        } => write_tagged_fields(writer, 23, |writer| {
            producer.write_wire(writer)?;
            admission.write_wire(writer)?;
            value_class.write_wire(writer)?;
            persistence.write_wire(writer)?;
            arguments.write_wire(writer)
        }),
        AwbcRuntimeTypeShape::NominalRecord {
            public_id,
            layout,
            arguments,
            fields,
        } => write_tagged_fields(writer, 24, |writer| {
            public_id.write_wire(writer)?;
            layout.write_wire(writer)?;
            arguments.write_wire(writer)?;
            fields.write_wire(writer)
        }),
        AwbcRuntimeTypeShape::Agent(agent) => write_tagged(writer, 27, agent),
        AwbcRuntimeTypeShape::Range(item) => write_tagged(writer, 29, item),
        AwbcRuntimeTypeShape::Iterator(item) => write_tagged(writer, 30, item),
        AwbcRuntimeTypeShape::Array { item, length } => write_tagged_fields(writer, 31, |writer| {
            item.write_wire(writer)?;
            length.write_wire(writer)
        }),
        AwbcRuntimeTypeShape::Map { key, value } => write_tagged_fields(writer, 32, |writer| {
            key.write_wire(writer)?;
            value.write_wire(writer)
        }),
        AwbcRuntimeTypeShape::Stream { item, error } => write_tagged_fields(writer, 33, |writer| {
            item.write_wire(writer)?;
            error.write_wire(writer)
        }),
        AwbcRuntimeTypeShape::Shared(value) => write_tagged(writer, 34, value),
        AwbcRuntimeTypeShape::Reference(value) => write_tagged(writer, 35, value),
        AwbcRuntimeTypeShape::Function { parameters, result } => {
            write_tagged_fields(writer, 36, |writer| {
                parameters.write_wire(writer)?;
                result.write_wire(writer)
            })
        }
        AwbcRuntimeTypeShape::Unit
        | AwbcRuntimeTypeShape::Bool
        | AwbcRuntimeTypeShape::Int(_)
        | AwbcRuntimeTypeShape::UInt(_)
        | AwbcRuntimeTypeShape::F32
        | AwbcRuntimeTypeShape::F64
        | AwbcRuntimeTypeShape::String
        | AwbcRuntimeTypeShape::Char
        | AwbcRuntimeTypeShape::Duration
        | AwbcRuntimeTypeShape::EntityRef
        | AwbcRuntimeTypeShape::MatrixF32
        | AwbcRuntimeTypeShape::MatrixF64
        | AwbcRuntimeTypeShape::TensorF32
        | AwbcRuntimeTypeShape::TensorF64
        | AwbcRuntimeTypeShape::Dynamic
        | AwbcRuntimeTypeShape::Bytes
        | AwbcRuntimeTypeShape::Never
        | AwbcRuntimeTypeShape::Progress => {
            unreachable!("scalar runtime type delegated to scalar writer")
        }
    }
}

fn write_tagged<T: Wire>(writer: &mut Writer, tag: u8, value: &T) -> Result<(), AwbcCodecError> {
    writer.write_u8(tag);
    value.write_wire(writer)
}

fn write_tagged_fields(
    writer: &mut Writer,
    tag: u8,
    write_fields: impl FnOnce(&mut Writer) -> Result<(), AwbcCodecError>,
) -> Result<(), AwbcCodecError> {
    writer.write_u8(tag);
    write_fields(writer)
}

wire_enum!(RuntimeAgentOperationalType, "Agent runtime type", {
    0 => RuntimeAgentOperationalType::DebugStatePath,
    1 => RuntimeAgentOperationalType::ObservationFieldPath,
    2 => RuntimeAgentOperationalType::Probe,
    3 => RuntimeAgentOperationalType::Predicate,
    4 => RuntimeAgentOperationalType::Observation,
    5 => RuntimeAgentOperationalType::ObservedObject,
    6 => RuntimeAgentOperationalType::BoundingBox,
    7 => RuntimeAgentOperationalType::ActionName,
    8 => RuntimeAgentOperationalType::ActionTarget,
    9 => RuntimeAgentOperationalType::ActionResult,
    10 => RuntimeAgentOperationalType::AgentValue,
    11 => RuntimeAgentOperationalType::DataFormat,
    12 => RuntimeAgentOperationalType::DataShape,
    13 => RuntimeAgentOperationalType::EntityMetadata,
    14 => RuntimeAgentOperationalType::SourceAnchor,
    15 => RuntimeAgentOperationalType::ProjectGraphNeighborhood,
    16 => RuntimeAgentOperationalType::ProjectGraphSymbol,
    17 => RuntimeAgentOperationalType::ProjectGraphEdge,
    18 => RuntimeAgentOperationalType::CaptureTarget,
    19 => RuntimeAgentOperationalType::CaptureReference,
    20 => RuntimeAgentOperationalType::Resource,
    21 => RuntimeAgentOperationalType::ResourceBody,
    22 => RuntimeAgentOperationalType::RagContextPack,
    23 => RuntimeAgentOperationalType::ObservedObjectId,
    24 => RuntimeAgentOperationalType::CaptureFormat,
    25 => RuntimeAgentOperationalType::CaptureKind,
    26 => RuntimeAgentOperationalType::Diagnostics,
    27 => RuntimeAgentOperationalType::WaitError,
    28 => RuntimeAgentOperationalType::ViewportPoint,
    29 => RuntimeAgentOperationalType::PointerButton,
    30 => RuntimeAgentOperationalType::RagError,
});

impl Wire for AwbcAgentTypeShape {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Leaf(agent) => {
                writer.write_u8(0);
                agent.write_wire(writer)
            }
            Self::Probe(value) => {
                writer.write_u8(1);
                value.write_wire(writer)
            }
        }
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => RuntimeAgentOperationalType::read_wire(reader).map(Self::Leaf),
            1 => AwbcTypeId::read_wire(reader).map(Self::Probe),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "Agent runtime type shape",
                tag,
                offset,
            }),
        }
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
            Self::Opaque { ty, payload } => {
                writer.write_u8(18);
                ty.write_wire(writer)?;
                payload.write_wire(writer)?;
            }
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
            18 => Self::Opaque {
                ty: AwbcTypeId::read_wire(reader)?,
                payload: AwbcConstantId::read_wire(reader)?,
            },
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

#[cfg(test)]
mod opaque_wire_tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;
    use crate::pattern::RuntimeSemanticTypeId;

    #[test]
    fn opaque_type_and_constant_rows_have_canonical_tags_and_admission_bytes() {
        let ty = AwbcRuntimeType::new(
            RuntimeSemanticTypeId::from_bytes([9; 32]),
            AwbcRuntimeTypeShape::Opaque {
                producer: AwbcStringId(7),
                admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                value_class: RuntimeOpaqueValueClass::Plain,
                persistence: RuntimeOpaquePersistence::ConstantAndSnapshot,
                arguments: vec![],
            },
        );
        let mut writer = Writer::default();
        ty.write_wire(&mut writer).expect("encode opaque type");
        let mut expected = vec![9; 32];
        expected.extend([23, 7]);
        expected.extend([0, 0, 0, 0]);
        assert_eq!(writer.finish(), expected);

        let constant = AwbcConstant::Opaque {
            ty: AwbcTypeId(3),
            payload: AwbcConstantId(5),
        };
        let mut writer = Writer::default();
        constant
            .write_wire(&mut writer)
            .expect("encode opaque constant");
        assert_eq!(writer.finish(), vec![18, 3, 5]);
    }

    #[test]
    fn opaque_type_decode_rejects_unknown_admission_tag() {
        let mut bytes = vec![0; 32];
        bytes.extend([23, 0]);
        bytes.push(2);
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcRuntimeType::read_wire(&mut reader)
                .expect_err("unknown opaque admission must reject"),
            AwbcCodecError::UnknownTag {
                kind: "opaque type admission",
                tag: 2,
                offset: 34,
            }
        );
    }
}
