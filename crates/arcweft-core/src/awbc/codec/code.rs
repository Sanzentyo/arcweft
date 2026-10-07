#![allow(
    clippy::too_many_lines,
    reason = "AWBC wire tables encode large tagged instruction families in one canonical order"
)]

use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer};
use crate::awbc::schema::{
    AwbcAwaitObserverResume, AwbcBinaryOp, AwbcBindMode, AwbcBlock, AwbcBlockId, AwbcChoiceId,
    AwbcConstantId, AwbcContentUnitId, AwbcDeferOwner, AwbcDialogueContentEffectBinding,
    AwbcDialogueResultTarget, AwbcDialogueValueBinding, AwbcDialogueValueRole, AwbcDropPolicy,
    AwbcEffectPlanId, AwbcFieldProjection, AwbcFormatAttemptOperand, AwbcFormatOperand,
    AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
    AwbcHostCallId, AwbcInstruction, AwbcIntrinsicId, AwbcLineOperationId, AwbcMatchArm,
    AwbcMutablePlace, AwbcOpcode, AwbcOpcodeClass, AwbcPattern, AwbcPatternId, AwbcPatternRest,
    AwbcPlaceReadMode, AwbcProjectCall, AwbcProjectCallAttachedMaterialization,
    AwbcProjectCallAttachedPresence, AwbcProjectCallOperand, AwbcProjectCallOperandMode,
    AwbcProjectCallOrdinaryMaterialization, AwbcPureHelperId, AwbcRecordPatternField,
    AwbcRegisterId, AwbcResumePoint, AwbcResumePointId, AwbcSafePointKind, AwbcScopeId,
    AwbcSignatureId, AwbcSourceMapId, AwbcStreamPlanId, AwbcStringId, AwbcTableRange,
    AwbcTaskPlanId, AwbcTerminator, AwbcTraitMethodId, AwbcTrapCode, AwbcTypeId, AwbcUnaryOp,
};
use crate::runtime_id::{RuntimeCallableSpecializationId, RuntimeCallableStateId};
use crate::value::{
    RuntimeAgentConstructor, RuntimeDisplacedField, RuntimeFmtParameterId,
    RuntimePlaceDisplacement, RuntimePlaceInitialization, RuntimeRecordFieldId,
};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueCustomFieldId, CharacterDialogueFieldCoordinate, CharacterDialogueOperation,
    CharacterDialoguePatchField, CharacterDialoguePatchOperation,
};

impl Wire for AwbcMutablePlace {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Local(register) => {
                writer.write_u8(0);
                register.write_wire(writer)?;
            }
            Self::Fields { base, fields } => {
                writer.write_u8(1);
                base.write_wire(writer)?;
                writer.write_table(fields)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Local(AwbcRegisterId::read_wire(reader)?)),
            1 => Ok(Self::Fields {
                base: AwbcRegisterId::read_wire(reader)?,
                fields: Vec::<RuntimeRecordFieldId>::read_wire(reader)?.into_boxed_slice(),
            }),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "mutable place",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for RuntimePlaceInitialization {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(match self {
            Self::Initialized => 0,
            Self::Uninitialized => 1,
            Self::Conditional => 2,
        });
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Initialized),
            1 => Ok(Self::Uninitialized),
            2 => Ok(Self::Conditional),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "place initialization",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for RuntimeDisplacedField<RuntimeRecordFieldId> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_table(&self.fields)?;
        self.initialization.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            fields: Vec::<RuntimeRecordFieldId>::read_wire(reader)?.into_boxed_slice(),
            initialization: RuntimePlaceInitialization::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimePlaceDisplacement<RuntimeRecordFieldId> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Unreachable => writer.write_u8(0),
            Self::Reachable {
                initialization,
                fields,
            } => {
                writer.write_u8(1);
                initialization.write_wire(writer)?;
                writer.write_table(fields)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Unreachable),
            1 => Ok(Self::Reachable {
                initialization: RuntimePlaceInitialization::read_wire(reader)?,
                fields: Vec::<RuntimeDisplacedField<RuntimeRecordFieldId>>::read_wire(reader)?
                    .into_boxed_slice(),
            }),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "place displacement",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for crate::runtime_id::RuntimeDeferSiteId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.get().get().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let ordinal = u32::read_wire(reader)?;
        std::num::NonZeroU32::new(ordinal)
            .map(Self::from_accepted_ordinal)
            .ok_or_else(|| AwbcCodecError::InvalidMetadata {
                kind: "defer site",
                message: "defer-site identity must be nonzero".to_owned(),
                offset,
            })
    }
}

impl Wire for crate::line_task::RuntimeDeferOutcomeFilter {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(match self {
            Self::Always => 0,
            Self::Completed => 1,
            Self::Cancelled => 2,
            Self::Failed => 3,
        });
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(Self::Always),
            1 => Ok(Self::Completed),
            2 => Ok(Self::Cancelled),
            3 => Ok(Self::Failed),
            _ => Err(AwbcCodecError::UnknownTag {
                kind: "defer outcome filter",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for AwbcDeferOwner {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(match self {
            Self::CurrentScope => 0,
            Self::LineRoot => 1,
        });
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(Self::CurrentScope),
            1 => Ok(Self::LineRoot),
            _ => Err(AwbcCodecError::UnknownTag {
                kind: "defer owner",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for CharacterDialogueOperation {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(match self {
            Self::Factory => 0,
            Self::Reconfigure => 1,
        });
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(Self::Factory),
            1 => Ok(Self::Reconfigure),
            _ => Err(AwbcCodecError::UnknownTag {
                kind: "CharacterDialogue operation",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for CharacterDialogueFieldCoordinate {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.semantic_tag());
        if let Self::Custom(id) = self {
            id.as_str().to_owned().write_wire(writer)?;
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Ok(match tag {
            0 => Self::Voice,
            1 => Self::Look,
            2 => Self::Stage,
            3 => Self::Portrait,
            4 => Self::Focus,
            5 => Self::Cleanup,
            6 => Self::View,
            7 => Self::SourceLocale,
            8 => Self::Hooks,
            9 => Self::Style,
            10 => Self::RichText,
            11 => Self::InlineFailure,
            12 => Self::Custom(
                CharacterDialogueCustomFieldId::try_new(String::read_wire(reader)?).map_err(
                    |error| AwbcCodecError::InvalidMetadata {
                        kind: "CharacterDialogue custom field",
                        message: error.to_string(),
                        offset,
                    },
                )?,
            ),
            _ => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "CharacterDialogue field",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl<T: Wire> Wire for CharacterDialoguePatchOperation<T> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Set(value) => {
                writer.write_u8(0);
                value.write_wire(writer)?;
            }
            Self::Clear => writer.write_u8(1),
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(Self::Set(T::read_wire(reader)?)),
            1 => Ok(Self::Clear),
            _ => Err(AwbcCodecError::UnknownTag {
                kind: "CharacterDialogue patch operation",
                tag,
                offset,
            }),
        }
    }
}

impl<T: Wire> Wire for CharacterDialoguePatchField<T> {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.coordinate.write_wire(writer)?;
        self.operation.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            coordinate: CharacterDialogueFieldCoordinate::read_wire(reader)?,
            operation: CharacterDialoguePatchOperation::read_wire(reader)?,
        })
    }
}

impl Wire for crate::plan::RuntimeFunctionSemanticRole {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.semantic_tag());
        Ok(())
    }
    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_semantic_tag(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "function semantic role",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcFunction {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.semantic_role.write_wire(writer)?;
        self.public_id.write_wire(writer)?;
        self.kind.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.type_context.write_wire(writer)?;
        self.input_ownership.write_wire(writer)?;
        self.frame_layout.write_wire(writer)?;
        self.blocks.write_wire(writer)?;
        self.entry_block.write_wire(writer)?;
        self.flags.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            semantic_role: crate::plan::RuntimeFunctionSemanticRole::read_wire(reader)?,
            public_id: Option::<AwbcStringId>::read_wire(reader)?,
            kind: AwbcFunctionKind::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            type_context: Option::<crate::awbc::schema::AwbcTypeId>::read_wire(reader)?,
            input_ownership: Vec::read_wire(reader)?,
            frame_layout: AwbcFrameLayoutId::read_wire(reader)?,
            blocks: AwbcTableRange::read_wire(reader)?,
            entry_block: AwbcBlockId::read_wire(reader)?,
            flags: AwbcFunctionFlags::read_wire(reader)?,
        })
    }
}

impl Wire for crate::awbc::schema::AwbcFunctionInputOwnership {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.origin.write_wire(writer)?;
        self.source.write_wire(writer)?;
        self.transfer.write_wire(writer)?;
        self.requirement.write_wire(writer)?;
        self.pattern.write_wire(writer)?;
        self.unrestricted_bindings.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            origin: crate::plan::RuntimeLocalOrigin::read_wire(reader)?,
            source: crate::plan::RuntimeFunctionInputSource::read_wire(reader)?,
            transfer: crate::plan::RuntimeFunctionInputTransfer::read_wire(reader)?,
            requirement: crate::plan::RuntimeFunctionInputOwnershipRequirement::read_wire(reader)?,
            pattern: Option::<crate::awbc::schema::AwbcPatternId>::read_wire(reader)?,
            unrestricted_bindings: Vec::read_wire(reader)?,
        })
    }
}

impl Wire for crate::plan::RuntimeLocalOrigin {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Binding(bytes) => {
                writer.write_u8(0);
                bytes.write_wire(writer)
            }
            Self::Parameter(identity) => {
                writer.write_u8(1);
                identity.as_bytes().write_wire(writer)
            }
            Self::EvaluatedResult(definition) => {
                writer.write_u8(2);
                definition.as_bytes().write_wire(writer)
            }
            Self::Generated(coordinate) => {
                writer.write_u8(3);
                coordinate.as_bytes().write_wire(writer)
            }
        }
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Ok(match tag {
            0 => Self::Binding(<[u8; 32]>::read_wire(reader)?),
            1 => Self::Parameter(
                crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
                    <[u8; 32]>::read_wire(reader)?,
                ),
            ),
            2 => Self::EvaluatedResult(
                crate::plan::RuntimeFunctionDefinitionIdentity::from_accepted_identity(
                    <[u8; 32]>::read_wire(reader)?,
                ),
            ),
            3 => Self::Generated(
                crate::plan::RuntimeGeneratedLocalOrigin::from_accepted_identity(
                    <[u8; 32]>::read_wire(reader)?,
                ),
            ),
            _ => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "function input origin",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for crate::plan::RuntimeFunctionInputTransfer {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Transferred(mode) => {
                writer.write_u8(0);
                mode.write_wire(writer)
            }
            Self::ExternalBinding => {
                writer.write_u8(1);
                Ok(())
            }
            Self::Formal => {
                writer.write_u8(2);
                Ok(())
            }
        }
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Transferred(
                crate::plan::RuntimeFunctionCaptureMode::read_wire(reader)?,
            )),
            1 => Ok(Self::ExternalBinding),
            2 => Ok(Self::Formal),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "function input transfer",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for crate::plan::RuntimeFunctionCaptureMode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(match self {
            Self::Copy => 0,
            Self::SnapshotClone => 1,
            Self::Move => 2,
        });
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Copy),
            1 => Ok(Self::SnapshotClone),
            2 => Ok(Self::Move),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "function capture mode",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for crate::plan::RuntimeFunctionParameterPassing {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.semantic_tag());
        Ok(())
    }
    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_semantic_tag(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "function parameter passing",
            tag,
            offset,
        })
    }
}

impl Wire for crate::plan::RuntimeFunctionInputSource {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Capture { position } => {
                writer.write_u8(0);
                position.write_wire(writer)
            }
            Self::Parameter { position, passing } => {
                writer.write_u8(1);
                position.write_wire(writer)?;
                passing.write_wire(writer)
            }
            Self::CapturedParameter { position, passing } => {
                writer.write_u8(2);
                position.write_wire(writer)?;
                passing.write_wire(writer)
            }
        }
    }
    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(Self::Capture {
                position: u32::read_wire(reader)?,
            }),
            1 => Ok(Self::Parameter {
                position: u32::read_wire(reader)?,
                passing: crate::plan::RuntimeFunctionParameterPassing::read_wire(reader)?,
            }),
            2 => Ok(Self::CapturedParameter {
                position: u32::read_wire(reader)?,
                passing: crate::plan::RuntimeFunctionParameterPassing::read_wire(reader)?,
            }),
            _ => Err(AwbcCodecError::UnknownTag {
                kind: "function input source",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for crate::plan::RuntimeFunctionInputOwnershipRequirement {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(match self {
            Self::Owned => 0,
            Self::Unrestricted => 1,
        });
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(Self::Owned),
            1 => Ok(Self::Unrestricted),
            _ => Err(AwbcCodecError::UnknownTag {
                kind: "function input ownership requirement",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for AwbcFunctionKind {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "function kind",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcDialogueValueRole {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "dialogue value role",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcDialogueValueBinding {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.slot.get().get().write_wire(writer)?;
        self.role.write_wire(writer)?;
        self.value.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let encoded = u32::read_wire(reader)?;
        let slot = encoded
            .checked_sub(1)
            .and_then(|index| {
                crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index as usize)
            })
            .ok_or_else(|| AwbcCodecError::InvalidMetadata {
                kind: "dialogue value slot",
                message: "slot identity must be nonzero".to_owned(),
                offset,
            })?;
        Ok(Self {
            slot,
            role: AwbcDialogueValueRole::read_wire(reader)?,
            value: AwbcRegisterId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcDialogueContentEffectBinding {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.site.get().get().write_wire(writer)?;
        self.state.write_wire(writer)?;
        self.captures.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let site = u32::read_wire(reader)?;
        let site = std::num::NonZeroU32::new(site)
            .map(crate::runtime_id::RuntimeDialogueEffectSiteId::from_accepted_ordinal)
            .ok_or_else(|| AwbcCodecError::InvalidMetadata {
                kind: "dialogue content effect site",
                message: "effect-site identity must be nonzero".to_owned(),
                offset,
            })?;
        Ok(Self {
            site,
            state: crate::runtime_id::RuntimeCallableStateId::read_wire(reader)?,
            captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcDialogueResultTarget {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.ty.write_wire(writer)?;
        self.pattern.write_wire(writer)?;
        self.destination.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            ty: AwbcTypeId::read_wire(reader)?,
            pattern: AwbcPatternId::read_wire(reader)?,
            destination: AwbcRegisterId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcBlock {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.owner.write_wire(writer)?;
        self.instructions.write_wire(writer)?;
        self.terminator.write_wire(writer)?;
        self.safe_point.write_wire(writer)?;
        self.source_map.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            owner: AwbcFunctionId::read_wire(reader)?,
            instructions: AwbcTableRange::read_wire(reader)?,
            terminator: AwbcTerminator::read_wire(reader)?,
            safe_point: AwbcSafePointKind::read_wire(reader)?,
            source_map: Option::<AwbcSourceMapId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFieldProjection {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Named(field) => {
                writer.write_u8(0);
                field.write_wire(writer)?;
            }
            Self::OpaqueRecord {
                owner,
                field,
                field_type,
            } => {
                writer.write_u8(1);
                owner.write_wire(writer)?;
                field.write_wire(writer)?;
                field_type.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Named(AwbcStringId::read_wire(reader)?),
            1 => Self::OpaqueRecord {
                owner: AwbcTypeId::read_wire(reader)?,
                field: u32::read_wire(reader)?,
                field_type: AwbcTypeId::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "field projection",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcFormatOperand {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(
            u8::try_from(self.parameter.index()).expect("fmt parameter IDs fit in one byte"),
        );
        self.function.write_wire(writer)?;
        self.captures.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        let parameter = RuntimeFmtParameterId::from_index(usize::from(tag)).ok_or(
            AwbcCodecError::UnknownTag {
                kind: "fmt parameter",
                tag,
                offset,
            },
        )?;
        Ok(Self {
            parameter,
            function: AwbcFunctionId::read_wire(reader)?,
            captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFormatAttemptOperand {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(
            u8::try_from(self.parameter.index()).expect("fmt parameter IDs fit in one byte"),
        );
        self.ty.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        let parameter = RuntimeFmtParameterId::from_index(usize::from(tag)).ok_or(
            AwbcCodecError::UnknownTag {
                kind: "fmt parameter",
                tag,
                offset,
            },
        )?;
        Ok(Self {
            parameter,
            ty: AwbcTypeId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcInstruction {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.opcode().encoded());
        match self {
            Self::Nop => {}
            Self::LoadConst { dst, constant } => {
                dst.write_wire(writer)?;
                constant.write_wire(writer)?;
            }
            Self::Move { dst, src } | Self::CopyValue { dst, src } => {
                dst.write_wire(writer)?;
                src.write_wire(writer)?;
            }
            Self::Clear { register } => register.write_wire(writer)?,
            Self::Drop { register, policy } => {
                register.write_wire(writer)?;
                policy.write_wire(writer)?;
            }
            Self::EnterScope { scope } | Self::ExitScope { scope } => scope.write_wire(writer)?,
            Self::BindPattern {
                pattern,
                value,
                mode,
            } => {
                pattern.write_wire(writer)?;
                value.write_wire(writer)?;
                mode.write_wire(writer)?;
            }
            Self::TestPattern {
                dst,
                pattern,
                value,
            } => {
                dst.write_wire(writer)?;
                pattern.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::MakeTuple { dst, items } | Self::MakeSequence { dst, items } => {
                dst.write_wire(writer)?;
                items.write_wire(writer)?;
            }
            Self::RepeatSequence { dst, value, len } => {
                dst.write_wire(writer)?;
                value.write_wire(writer)?;
                len.write_wire(writer)?;
            }
            Self::SequenceLen { dst, sequence } => {
                dst.write_wire(writer)?;
                sequence.write_wire(writer)?;
            }
            Self::SequenceGet {
                dst,
                sequence,
                index,
            } => {
                dst.write_wire(writer)?;
                sequence.write_wire(writer)?;
                index.write_wire(writer)?;
            }
            Self::SequenceSlice {
                dst,
                sequence,
                start,
            } => {
                dst.write_wire(writer)?;
                sequence.write_wire(writer)?;
                start.write_wire(writer)?;
            }
            Self::SequencePush { sequence, value } => {
                sequence.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::SequencePopFront { dst, place } | Self::VecPop { dst, place } => {
                dst.write_wire(writer)?;
                place.write_wire(writer)?;
            }
            Self::VecPush { place, value } => {
                place.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::MakeRecord { dst, ty, fields } => {
                dst.write_wire(writer)?;
                ty.write_wire(writer)?;
                fields.write_wire(writer)?;
            }
            Self::MakeVariant {
                dst,
                ty,
                case,
                case_name,
                payload,
            } => {
                dst.write_wire(writer)?;
                ty.write_wire(writer)?;
                case.write_wire(writer)?;
                case_name.write_wire(writer)?;
                payload.write_wire(writer)?;
            }
            Self::ProjectTuple {
                dst,
                target,
                ordinal,
            }
            | Self::ProjectRecord {
                dst,
                target,
                ordinal,
            } => {
                dst.write_wire(writer)?;
                target.write_wire(writer)?;
                ordinal.write_wire(writer)?;
            }
            Self::ProjectField { dst, target, field } => {
                dst.write_wire(writer)?;
                target.write_wire(writer)?;
                field.write_wire(writer)?;
            }
            Self::ReadPlace {
                dst,
                root,
                fields,
                mode,
            } => {
                dst.write_wire(writer)?;
                root.write_wire(writer)?;
                fields.write_wire(writer)?;
                mode.write_wire(writer)?;
            }
            Self::Unary { dst, op, src } => {
                dst.write_wire(writer)?;
                op.write_wire(writer)?;
                src.write_wire(writer)?;
            }
            Self::Binary { dst, op, lhs, rhs } => {
                dst.write_wire(writer)?;
                op.write_wire(writer)?;
                lhs.write_wire(writer)?;
                rhs.write_wire(writer)?;
            }
            Self::CallPureHelper { dst, helper, args } => {
                dst.write_wire(writer)?;
                helper.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::CallIntrinsic {
                dst,
                intrinsic,
                args,
            } => {
                dst.write_wire(writer)?;
                intrinsic.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::EnsureContent { content } => content.write_wire(writer)?,
            Self::MakeDialogueContent {
                destination,
                template,
                values,
                effects,
            } => {
                destination.write_wire(writer)?;
                template.write_wire(writer)?;
                values.write_wire(writer)?;
                effects.write_wire(writer)?;
            }
            Self::FormatContent {
                destination,
                template,
                attempt,
                attempt_operands,
                project_method,
                project_option,
                project_result,
                operands,
            } => {
                destination.write_wire(writer)?;
                template.write_wire(writer)?;
                attempt.write_wire(writer)?;
                attempt_operands.write_wire(writer)?;
                project_method.write_wire(writer)?;
                project_option.write_wire(writer)?;
                project_result.write_wire(writer)?;
                operands.write_wire(writer)?;
            }
            Self::FormatOperandAttempt { attempt, parameter } => {
                attempt.write_wire(writer)?;
                writer.write_u8(
                    u8::try_from(parameter.index()).expect("fmt parameter IDs fit in one byte"),
                );
            }
            Self::CompleteFormatOperand {
                attempt,
                parameter,
                value,
            } => {
                attempt.write_wire(writer)?;
                writer.write_u8(
                    u8::try_from(parameter.index()).expect("fmt parameter IDs fit in one byte"),
                );
                value.write_wire(writer)?;
            }
            Self::AbandonFormatAttempt { attempt } => attempt.write_wire(writer)?,
            Self::CharacterDialogue {
                destination,
                operation,
                target,
                fields,
            } => {
                destination.write_wire(writer)?;
                operation.write_wire(writer)?;
                target.write_wire(writer)?;
                fields.write_wire(writer)?;
            }
            Self::EmitEffect { effect, args } => {
                effect.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::StartNeed { dst, plan, args } => {
                dst.write_wire(writer)?;
                plan.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::SpawnFiber {
                dst,
                function,
                args,
            } => {
                dst.write_wire(writer)?;
                function.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::StreamYield { stream, value } => {
                stream.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::StreamClose { stream } => stream.write_wire(writer)?,
            Self::ExecuteLineOperation {
                dst,
                operation,
                args,
            } => {
                dst.write_wire(writer)?;
                operation.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::CommitDialogueResult { source } => source.write_wire(writer)?,
            Self::Assign {
                place,
                value,
                displacement,
            } => {
                place.write_wire(writer)?;
                value.write_wire(writer)?;
                displacement.write_wire(writer)?;
            }
            Self::CallTraitMethod {
                dst,
                method,
                receiver,
                args,
                receiver_out,
            } => {
                dst.write_wire(writer)?;
                method.write_wire(writer)?;
                receiver.write_wire(writer)?;
                args.write_wire(writer)?;
                receiver_out.write_wire(writer)?;
            }
            Self::RegisterCleanup { key, effect, args } => {
                key.write_wire(writer)?;
                effect.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::CancelCleanup { key } => key.write_wire(writer)?,
            Self::RegisterDefer {
                site,
                outcome,
                owner,
                captures,
            } => {
                site.write_wire(writer)?;
                outcome.write_wire(writer)?;
                owner.write_wire(writer)?;
                captures.write_wire(writer)?;
            }
            Self::MakeCallable {
                dst,
                state,
                captures,
            } => {
                dst.write_wire(writer)?;
                state.write_wire(writer)?;
                captures.write_wire(writer)?;
            }
            Self::SpecializeCallable {
                dst,
                src,
                specialization,
            } => {
                dst.write_wire(writer)?;
                src.write_wire(writer)?;
                specialization.write_wire(writer)?;
            }
            Self::ApplyGroup { dst, callee, args } => {
                dst.write_wire(writer)?;
                callee.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::MakeAgent {
                dst,
                constructor,
                operands,
            } => {
                dst.write_wire(writer)?;
                constructor.write_wire(writer)?;
                operands.write_wire(writer)?;
            }
            Self::MakeReductionUnchanged { dst, ty, state } => {
                dst.write_wire(writer)?;
                ty.write_wire(writer)?;
                state.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let encoded = reader.read_u8()?;
        let Some(opcode) = AwbcOpcode::from_encoded(encoded) else {
            return Err(AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: encoded,
                offset,
            });
        };
        if opcode.class() != AwbcOpcodeClass::Instruction {
            return Err(AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: encoded,
                offset,
            });
        }
        Ok(match opcode {
            AwbcOpcode::Nop => Self::Nop,
            AwbcOpcode::LoadConst => Self::LoadConst {
                dst: AwbcRegisterId::read_wire(reader)?,
                constant: AwbcConstantId::read_wire(reader)?,
            },
            AwbcOpcode::Move => Self::Move {
                dst: AwbcRegisterId::read_wire(reader)?,
                src: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::CopyValue => Self::CopyValue {
                dst: AwbcRegisterId::read_wire(reader)?,
                src: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Clear => Self::Clear {
                register: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::EnterScope => Self::EnterScope {
                scope: AwbcScopeId::read_wire(reader)?,
            },
            AwbcOpcode::ExitScope => Self::ExitScope {
                scope: AwbcScopeId::read_wire(reader)?,
            },
            AwbcOpcode::BindPattern => Self::BindPattern {
                pattern: AwbcPatternId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
                mode: AwbcBindMode::read_wire(reader)?,
            },
            AwbcOpcode::TestPattern => Self::TestPattern {
                dst: AwbcRegisterId::read_wire(reader)?,
                pattern: AwbcPatternId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::MakeTuple => Self::MakeTuple {
                dst: AwbcRegisterId::read_wire(reader)?,
                items: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeSequence => Self::MakeSequence {
                dst: AwbcRegisterId::read_wire(reader)?,
                items: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::RepeatSequence => Self::RepeatSequence {
                dst: AwbcRegisterId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
                len: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequenceLen => Self::SequenceLen {
                dst: AwbcRegisterId::read_wire(reader)?,
                sequence: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequenceGet => Self::SequenceGet {
                dst: AwbcRegisterId::read_wire(reader)?,
                sequence: AwbcRegisterId::read_wire(reader)?,
                index: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequenceSlice => Self::SequenceSlice {
                dst: AwbcRegisterId::read_wire(reader)?,
                sequence: AwbcRegisterId::read_wire(reader)?,
                start: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequencePush => Self::SequencePush {
                sequence: AwbcRegisterId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::SequencePopFront => Self::SequencePopFront {
                dst: AwbcRegisterId::read_wire(reader)?,
                place: AwbcMutablePlace::read_wire(reader)?,
            },
            AwbcOpcode::VecPush => Self::VecPush {
                place: AwbcMutablePlace::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::VecPop => Self::VecPop {
                dst: AwbcRegisterId::read_wire(reader)?,
                place: AwbcMutablePlace::read_wire(reader)?,
            },
            AwbcOpcode::MakeRecord => Self::MakeRecord {
                dst: AwbcRegisterId::read_wire(reader)?,
                ty: AwbcTypeId::read_wire(reader)?,
                fields: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeVariant => Self::MakeVariant {
                dst: AwbcRegisterId::read_wire(reader)?,
                ty: AwbcTypeId::read_wire(reader)?,
                case: u32::read_wire(reader)?,
                case_name: AwbcStringId::read_wire(reader)?,
                payload: Option::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::ProjectTuple => Self::ProjectTuple {
                dst: AwbcRegisterId::read_wire(reader)?,
                target: AwbcRegisterId::read_wire(reader)?,
                ordinal: u32::read_wire(reader)?,
            },
            AwbcOpcode::ProjectRecord => Self::ProjectRecord {
                dst: AwbcRegisterId::read_wire(reader)?,
                target: AwbcRegisterId::read_wire(reader)?,
                ordinal: u32::read_wire(reader)?,
            },
            AwbcOpcode::ProjectField => Self::ProjectField {
                dst: AwbcRegisterId::read_wire(reader)?,
                target: AwbcRegisterId::read_wire(reader)?,
                field: AwbcFieldProjection::read_wire(reader)?,
            },
            AwbcOpcode::ReadPlace => Self::ReadPlace {
                dst: AwbcRegisterId::read_wire(reader)?,
                root: AwbcRegisterId::read_wire(reader)?,
                fields: Vec::<RuntimeRecordFieldId>::read_wire(reader)?,
                mode: AwbcPlaceReadMode::read_wire(reader)?,
            },
            AwbcOpcode::Unary => Self::Unary {
                dst: AwbcRegisterId::read_wire(reader)?,
                op: AwbcUnaryOp::read_wire(reader)?,
                src: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Binary => Self::Binary {
                dst: AwbcRegisterId::read_wire(reader)?,
                op: AwbcBinaryOp::read_wire(reader)?,
                lhs: AwbcRegisterId::read_wire(reader)?,
                rhs: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::CallPureHelper => Self::CallPureHelper {
                dst: AwbcRegisterId::read_wire(reader)?,
                helper: AwbcPureHelperId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::CallIntrinsic => Self::CallIntrinsic {
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                intrinsic: AwbcIntrinsicId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::EnsureContent => Self::EnsureContent {
                content: AwbcContentUnitId::read_wire(reader)?,
            },
            AwbcOpcode::MakeDialogueContent => Self::MakeDialogueContent {
                destination: AwbcRegisterId::read_wire(reader)?,
                template: crate::runtime_id::RuntimeDialogueContentTemplateId::read_wire(reader)?,
                values: Vec::<AwbcDialogueValueBinding>::read_wire(reader)?,
                effects: Vec::<AwbcDialogueContentEffectBinding>::read_wire(reader)?,
            },
            AwbcOpcode::FormatContent => Self::FormatContent {
                destination: AwbcRegisterId::read_wire(reader)?,
                template: crate::runtime_id::RuntimeDialogueContentTemplateId::read_wire(reader)?,
                attempt: Option::<crate::runtime_id::RuntimeFormatAttemptId>::read_wire(reader)?,
                attempt_operands: Vec::<AwbcFormatAttemptOperand>::read_wire(reader)?,
                project_method: Option::<AwbcTraitMethodId>::read_wire(reader)?,
                project_option: bool::read_wire(reader)?,
                project_result: Option::<AwbcRegisterId>::read_wire(reader)?,
                operands: Vec::<AwbcFormatOperand>::read_wire(reader)?,
            },
            AwbcOpcode::FormatOperandAttempt => {
                let attempt = crate::runtime_id::RuntimeFormatAttemptId::read_wire(reader)?;
                let offset = reader.offset();
                let tag = reader.read_u8()?;
                let parameter = RuntimeFmtParameterId::from_index(usize::from(tag)).ok_or(
                    AwbcCodecError::UnknownTag {
                        kind: "fmt parameter",
                        tag,
                        offset,
                    },
                )?;
                Self::FormatOperandAttempt { attempt, parameter }
            }
            AwbcOpcode::CompleteFormatOperand => {
                let attempt = crate::runtime_id::RuntimeFormatAttemptId::read_wire(reader)?;
                let offset = reader.offset();
                let tag = reader.read_u8()?;
                let parameter = RuntimeFmtParameterId::from_index(usize::from(tag)).ok_or(
                    AwbcCodecError::UnknownTag {
                        kind: "fmt parameter",
                        tag,
                        offset,
                    },
                )?;
                Self::CompleteFormatOperand {
                    attempt,
                    parameter,
                    value: AwbcRegisterId::read_wire(reader)?,
                }
            }
            AwbcOpcode::AbandonFormatAttempt => Self::AbandonFormatAttempt {
                attempt: crate::runtime_id::RuntimeFormatAttemptId::read_wire(reader)?,
            },
            AwbcOpcode::CharacterDialogue => Self::CharacterDialogue {
                destination: AwbcRegisterId::read_wire(reader)?,
                operation: CharacterDialogueOperation::read_wire(reader)?,
                target: AwbcRegisterId::read_wire(reader)?,
                fields: Vec::<CharacterDialoguePatchField<AwbcRegisterId>>::read_wire(reader)?,
            },
            AwbcOpcode::EmitEffect => Self::EmitEffect {
                effect: AwbcEffectPlanId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::StartNeed => Self::StartNeed {
                dst: AwbcRegisterId::read_wire(reader)?,
                plan: AwbcTaskPlanId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::SpawnFiber => Self::SpawnFiber {
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                function: AwbcFunctionId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::StreamYield => Self::StreamYield {
                stream: AwbcStreamPlanId::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::StreamClose => Self::StreamClose {
                stream: AwbcStreamPlanId::read_wire(reader)?,
            },
            AwbcOpcode::ExecuteLineOperation => Self::ExecuteLineOperation {
                dst: AwbcRegisterId::read_wire(reader)?,
                operation: AwbcLineOperationId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::CommitDialogueResult => Self::CommitDialogueResult {
                source: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Drop => Self::Drop {
                register: AwbcRegisterId::read_wire(reader)?,
                policy: AwbcDropPolicy::read_wire(reader)?,
            },
            AwbcOpcode::Assign => Self::Assign {
                place: AwbcMutablePlace::read_wire(reader)?,
                value: AwbcRegisterId::read_wire(reader)?,
                displacement: RuntimePlaceDisplacement::read_wire(reader)?,
            },
            AwbcOpcode::CallTraitMethod => Self::CallTraitMethod {
                dst: AwbcRegisterId::read_wire(reader)?,
                method: AwbcTraitMethodId::read_wire(reader)?,
                receiver: AwbcRegisterId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
                receiver_out: Option::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::RegisterCleanup => Self::RegisterCleanup {
                key: AwbcStringId::read_wire(reader)?,
                effect: AwbcEffectPlanId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::CancelCleanup => Self::CancelCleanup {
                key: AwbcStringId::read_wire(reader)?,
            },
            AwbcOpcode::RegisterDefer => Self::RegisterDefer {
                site: crate::runtime_id::RuntimeDeferSiteId::read_wire(reader)?,
                outcome: crate::line_task::RuntimeDeferOutcomeFilter::read_wire(reader)?,
                owner: AwbcDeferOwner::read_wire(reader)?,
                captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeCallable => Self::MakeCallable {
                dst: AwbcRegisterId::read_wire(reader)?,
                state: RuntimeCallableStateId::read_wire(reader)?,
                captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::SpecializeCallable => Self::SpecializeCallable {
                dst: AwbcRegisterId::read_wire(reader)?,
                src: AwbcRegisterId::read_wire(reader)?,
                specialization: RuntimeCallableSpecializationId::read_wire(reader)?,
            },
            AwbcOpcode::ApplyGroup => Self::ApplyGroup {
                dst: AwbcRegisterId::read_wire(reader)?,
                callee: AwbcRegisterId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeAgent => Self::MakeAgent {
                dst: AwbcRegisterId::read_wire(reader)?,
                constructor: RuntimeAgentConstructor::read_wire(reader)?,
                operands: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::MakeReductionUnchanged => Self::MakeReductionUnchanged {
                dst: AwbcRegisterId::read_wire(reader)?,
                ty: AwbcTypeId::read_wire(reader)?,
                state: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Jump
            | AwbcOpcode::Branch
            | AwbcOpcode::Match
            | AwbcOpcode::CallFunction
            | AwbcOpcode::GotoStatic
            | AwbcOpcode::GotoDynamic
            | AwbcOpcode::Dialogue
            | AwbcOpcode::Choice
            | AwbcOpcode::Await
            | AwbcOpcode::AwaitMany
            | AwbcOpcode::HostCall
            | AwbcOpcode::Return
            | AwbcOpcode::SelectDialogueResult
            | AwbcOpcode::SequenceNext
            | AwbcOpcode::ProjectCall
            | AwbcOpcode::Trap
            | AwbcOpcode::BudgetYield
            | AwbcOpcode::Unreachable => unreachable!("terminator opcode rejected above"),
        })
    }
}

impl Wire for AwbcDropPolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Default => writer.write_u8(0),
            Self::Cancel => writer.write_u8(1),
            Self::Stop { fade } => {
                writer.write_u8(2);
                fade.write_wire(writer)?;
            }
            Self::Finish => writer.write_u8(3),
            Self::Release => writer.write_u8(4),
            Self::Detach => writer.write_u8(5),
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Default,
            1 => Self::Cancel,
            2 => Self::Stop {
                fade: AwbcRegisterId::read_wire(reader)?,
            },
            3 => Self::Finish,
            4 => Self::Release,
            5 => Self::Detach,
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "drop policy",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallOperandMode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "project-call operand mode",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcProjectCallOperand {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.value.write_wire(writer)?;
        self.mode.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            value: AwbcRegisterId::read_wire(reader)?,
            mode: AwbcProjectCallOperandMode::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcProjectCallAttachedPresence {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::RequiredPresent => writer.write_u8(0),
            Self::OptionalPresent => writer.write_u8(1),
            Self::OptionalOmitted => writer.write_u8(2),
            Self::DefaultedPresent => writer.write_u8(3),
            Self::DefaultedOmitted => {
                writer.write_u8(4);
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::RequiredPresent,
            1 => Self::OptionalPresent,
            2 => Self::OptionalOmitted,
            3 => Self::DefaultedPresent,
            4 => Self::DefaultedOmitted,
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "project-call attached presence",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallOrdinaryMaterialization {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Fixed {
                parameter,
                source_index,
            } => {
                writer.write_u8(0);
                parameter.write_wire(writer)?;
                source_index.write_wire(writer)?;
            }
            Self::Rest {
                parameter,
                source_indices,
            } => {
                writer.write_u8(1);
                parameter.write_wire(writer)?;
                source_indices.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Fixed {
                parameter: u32::read_wire(reader)?,
                source_index: u32::read_wire(reader)?,
            },
            1 => Self::Rest {
                parameter: u32::read_wire(reader)?,
                source_indices: Vec::<u32>::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "project-call ordinary materialization",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcProjectCallAttachedMaterialization {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.source_index.write_wire(writer)?;
        self.presence.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            source_index: Option::<u32>::read_wire(reader)?,
            presence: AwbcProjectCallAttachedPresence::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcProjectCall {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.callee.write_wire(writer)?;
        self.state.write_wire(writer)?;
        self.completed_group.write_wire(writer)?;
        self.operands.write_wire(writer)?;
        self.ordinary.write_wire(writer)?;
        self.attached.write_wire(writer)?;
        self.result_pattern.write_wire(writer)?;
        self.resume.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            callee: AwbcRegisterId::read_wire(reader)?,
            state: RuntimeCallableStateId::read_wire(reader)?,
            completed_group: u32::read_wire(reader)?,
            operands: Vec::<AwbcProjectCallOperand>::read_wire(reader)?,
            ordinary: Vec::<AwbcProjectCallOrdinaryMaterialization>::read_wire(reader)?,
            attached: Option::<AwbcProjectCallAttachedMaterialization>::read_wire(reader)?,
            result_pattern: AwbcPatternId::read_wire(reader)?,
            resume: AwbcResumePointId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcTerminator {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.opcode().encoded());
        match self {
            Self::Jump { target } => target.write_wire(writer)?,
            Self::Branch {
                condition,
                then_block,
                else_block,
            } => {
                condition.write_wire(writer)?;
                then_block.write_wire(writer)?;
                else_block.write_wire(writer)?;
            }
            Self::SequenceNext {
                sequence,
                item,
                some_block,
                none_block,
            } => {
                sequence.write_wire(writer)?;
                item.write_wire(writer)?;
                some_block.write_wire(writer)?;
                none_block.write_wire(writer)?;
            }
            Self::Match {
                scrutinee,
                arms,
                default,
            } => {
                scrutinee.write_wire(writer)?;
                arms.write_wire(writer)?;
                default.write_wire(writer)?;
            }
            Self::CallFunction {
                function,
                args,
                dst,
                resume,
            } => {
                function.write_wire(writer)?;
                args.write_wire(writer)?;
                dst.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::GotoStatic { function, args } => {
                function.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::GotoDynamic { target, args } => {
                target.write_wire(writer)?;
                args.write_wire(writer)?;
            }
            Self::Dialogue {
                target,
                content,
                values,
                effects,
                line_task_captures,
                result,
                resume,
            } => {
                target.write_wire(writer)?;
                content.write_wire(writer)?;
                values.write_wire(writer)?;
                effects.write_wire(writer)?;
                line_task_captures.write_wire(writer)?;
                result.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::Choice {
                choice,
                dst,
                resume,
            } => {
                choice.write_wire(writer)?;
                dst.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::Await {
                handle,
                binding,
                observer,
                resume,
            } => {
                handle.write_wire(writer)?;
                binding.write_wire(writer)?;
                observer.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::AwaitMany {
                plan,
                source,
                binding,
                resume,
            } => {
                plan.write_wire(writer)?;
                source.write_wire(writer)?;
                binding.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::HostCall {
                call,
                args,
                dst,
                resume,
            } => {
                call.write_wire(writer)?;
                args.write_wire(writer)?;
                dst.write_wire(writer)?;
                resume.write_wire(writer)?;
            }
            Self::ProjectCall { call } => call.write_wire(writer)?,
            Self::Return { value } => value.write_wire(writer)?,
            Self::SelectDialogueResult { value } => value.write_wire(writer)?,
            Self::Trap { code, message } => {
                code.write_wire(writer)?;
                message.write_wire(writer)?;
            }
            Self::BudgetYield { resume } => resume.write_wire(writer)?,
            Self::Unreachable => {}
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let encoded = reader.read_u8()?;
        let Some(opcode) = AwbcOpcode::from_encoded(encoded) else {
            return Err(AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: encoded,
                offset,
            });
        };
        if opcode.class() != AwbcOpcodeClass::Terminator {
            return Err(AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: encoded,
                offset,
            });
        }
        Ok(match opcode {
            AwbcOpcode::Jump => Self::Jump {
                target: AwbcBlockId::read_wire(reader)?,
            },
            AwbcOpcode::Branch => Self::Branch {
                condition: AwbcRegisterId::read_wire(reader)?,
                then_block: AwbcBlockId::read_wire(reader)?,
                else_block: AwbcBlockId::read_wire(reader)?,
            },
            AwbcOpcode::SequenceNext => Self::SequenceNext {
                sequence: AwbcRegisterId::read_wire(reader)?,
                item: AwbcRegisterId::read_wire(reader)?,
                some_block: AwbcBlockId::read_wire(reader)?,
                none_block: AwbcBlockId::read_wire(reader)?,
            },
            AwbcOpcode::Match => Self::Match {
                scrutinee: AwbcRegisterId::read_wire(reader)?,
                arms: AwbcTableRange::read_wire(reader)?,
                default: AwbcBlockId::read_wire(reader)?,
            },
            AwbcOpcode::CallFunction => Self::CallFunction {
                function: AwbcFunctionId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::GotoStatic => Self::GotoStatic {
                function: AwbcFunctionId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::GotoDynamic => Self::GotoDynamic {
                target: AwbcRegisterId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::Dialogue => Self::Dialogue {
                target: AwbcRegisterId::read_wire(reader)?,
                content: AwbcContentUnitId::read_wire(reader)?,
                values: Vec::<AwbcDialogueValueBinding>::read_wire(reader)?,
                effects: Vec::<AwbcDialogueContentEffectBinding>::read_wire(reader)?,
                line_task_captures: Vec::<AwbcRegisterId>::read_wire(reader)?,
                result: AwbcDialogueResultTarget::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Choice => Self::Choice {
                choice: AwbcChoiceId::read_wire(reader)?,
                dst: AwbcRegisterId::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Await => Self::Await {
                handle: AwbcRegisterId::read_wire(reader)?,
                binding: Option::<AwbcPatternId>::read_wire(reader)?,
                observer: Option::<AwbcAwaitObserverResume>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::AwaitMany => Self::AwaitMany {
                plan: AwbcTaskPlanId::read_wire(reader)?,
                source: AwbcRegisterId::read_wire(reader)?,
                binding: Option::<AwbcPatternId>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::HostCall => Self::HostCall {
                call: AwbcHostCallId::read_wire(reader)?,
                args: Vec::<AwbcRegisterId>::read_wire(reader)?,
                dst: Option::<AwbcRegisterId>::read_wire(reader)?,
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::ProjectCall => Self::ProjectCall {
                call: AwbcProjectCall::read_wire(reader)?,
            },
            AwbcOpcode::Return => Self::Return {
                value: Option::<AwbcRegisterId>::read_wire(reader)?,
            },
            AwbcOpcode::SelectDialogueResult => Self::SelectDialogueResult {
                value: AwbcRegisterId::read_wire(reader)?,
            },
            AwbcOpcode::Trap => Self::Trap {
                code: AwbcTrapCode::read_wire(reader)?,
                message: Option::<AwbcStringId>::read_wire(reader)?,
            },
            AwbcOpcode::BudgetYield => Self::BudgetYield {
                resume: AwbcResumePointId::read_wire(reader)?,
            },
            AwbcOpcode::Unreachable => Self::Unreachable,
            AwbcOpcode::Nop
            | AwbcOpcode::LoadConst
            | AwbcOpcode::Move
            | AwbcOpcode::CopyValue
            | AwbcOpcode::Clear
            | AwbcOpcode::EnterScope
            | AwbcOpcode::ExitScope
            | AwbcOpcode::BindPattern
            | AwbcOpcode::TestPattern
            | AwbcOpcode::MakeTuple
            | AwbcOpcode::MakeSequence
            | AwbcOpcode::RepeatSequence
            | AwbcOpcode::SequenceLen
            | AwbcOpcode::SequenceGet
            | AwbcOpcode::SequenceSlice
            | AwbcOpcode::SequencePush
            | AwbcOpcode::SequencePopFront
            | AwbcOpcode::VecPush
            | AwbcOpcode::VecPop
            | AwbcOpcode::MakeRecord
            | AwbcOpcode::MakeVariant
            | AwbcOpcode::ProjectTuple
            | AwbcOpcode::ProjectRecord
            | AwbcOpcode::ProjectField
            | AwbcOpcode::ReadPlace
            | AwbcOpcode::Unary
            | AwbcOpcode::Binary
            | AwbcOpcode::CallPureHelper
            | AwbcOpcode::CallIntrinsic
            | AwbcOpcode::EnsureContent
            | AwbcOpcode::MakeDialogueContent
            | AwbcOpcode::CharacterDialogue
            | AwbcOpcode::FormatContent
            | AwbcOpcode::FormatOperandAttempt
            | AwbcOpcode::CompleteFormatOperand
            | AwbcOpcode::AbandonFormatAttempt
            | AwbcOpcode::EmitEffect
            | AwbcOpcode::StartNeed
            | AwbcOpcode::SpawnFiber
            | AwbcOpcode::StreamYield
            | AwbcOpcode::StreamClose
            | AwbcOpcode::ExecuteLineOperation
            | AwbcOpcode::CommitDialogueResult
            | AwbcOpcode::Drop
            | AwbcOpcode::Assign
            | AwbcOpcode::CallTraitMethod
            | AwbcOpcode::RegisterCleanup
            | AwbcOpcode::CancelCleanup
            | AwbcOpcode::RegisterDefer
            | AwbcOpcode::MakeCallable
            | AwbcOpcode::SpecializeCallable
            | AwbcOpcode::ApplyGroup
            | AwbcOpcode::MakeAgent
            | AwbcOpcode::MakeReductionUnchanged => {
                unreachable!("instruction opcode rejected above")
            }
        })
    }
}

impl Wire for AwbcAwaitObserverResume {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.destination.write_wire(writer)?;
        self.resume.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            destination: AwbcRegisterId::read_wire(reader)?,
            resume: AwbcResumePointId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcPlaceReadMode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }
    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "place read mode",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcBindMode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "bind mode",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcUnaryOp {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "unary operator",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcBinaryOp {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "binary operator",
            tag,
            offset,
        })
    }
}

impl Wire for RuntimeAgentConstructor {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.semantic_tag());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_semantic_tag(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "Agent constructor",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcSafePointKind {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "safe point kind",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcResumePoint {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.function.write_wire(writer)?;
        self.block.write_wire(writer)?;
        self.frame_layout.write_wire(writer)?;
        self.kind.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            function: AwbcFunctionId::read_wire(reader)?,
            block: AwbcBlockId::read_wire(reader)?,
            frame_layout: AwbcFrameLayoutId::read_wire(reader)?,
            kind: AwbcSafePointKind::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcTrapCode {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        writer.write_u8(self.encoded());
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let tag = reader.read_u8()?;
        Self::from_encoded(tag).ok_or(AwbcCodecError::UnknownTag {
            kind: "trap code",
            tag,
            offset,
        })
    }
}

impl Wire for AwbcPattern {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Bind {
                target,
                mutable,
                expected,
            } => {
                writer.write_u8(0);
                target.write_wire(writer)?;
                mutable.write_wire(writer)?;
                expected.write_wire(writer)?;
            }
            Self::Discard => writer.write_u8(1),
            Self::Literal(value) => {
                writer.write_u8(2);
                value.write_wire(writer)?;
            }
            Self::Entity(value) => {
                writer.write_u8(3);
                value.write_wire(writer)?;
            }
            Self::Or(items) => {
                writer.write_u8(9);
                items.write_wire(writer)?;
            }
            Self::Tuple(items) => {
                writer.write_u8(4);
                items.write_wire(writer)?;
            }
            Self::Record { ty, fields, rest } => {
                writer.write_u8(5);
                ty.write_wire(writer)?;
                fields.write_wire(writer)?;
                rest.write_wire(writer)?;
            }
            Self::Sequence { items, rest } => {
                writer.write_u8(6);
                items.write_wire(writer)?;
                rest.write_wire(writer)?;
            }
            Self::Variant {
                ty,
                case,
                case_name,
                payload,
            } => {
                writer.write_u8(7);
                ty.write_wire(writer)?;
                case.write_wire(writer)?;
                case_name.write_wire(writer)?;
                payload.write_wire(writer)?;
            }
            Self::Whole { target, inner } => {
                writer.write_u8(8);
                target.write_wire(writer)?;
                inner.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Bind {
                target: AwbcRegisterId::read_wire(reader)?,
                mutable: bool::read_wire(reader)?,
                expected: Option::<AwbcTypeId>::read_wire(reader)?,
            },
            1 => Self::Discard,
            2 => Self::Literal(AwbcConstantId::read_wire(reader)?),
            3 => Self::Entity(crate::value::RuntimeEntityReference::read_wire(reader)?),
            4 => Self::Tuple(Vec::<AwbcPatternId>::read_wire(reader)?),
            9 => Self::Or(Vec::<AwbcPatternId>::read_wire(reader)?),
            5 => Self::Record {
                ty: Option::<AwbcTypeId>::read_wire(reader)?,
                fields: Vec::<AwbcRecordPatternField>::read_wire(reader)?,
                rest: AwbcPatternRest::read_wire(reader)?,
            },
            6 => Self::Sequence {
                items: Vec::<AwbcPatternId>::read_wire(reader)?,
                rest: AwbcPatternRest::read_wire(reader)?,
            },
            7 => Self::Variant {
                ty: AwbcTypeId::read_wire(reader)?,
                case: u32::read_wire(reader)?,
                case_name: AwbcStringId::read_wire(reader)?,
                payload: Option::<AwbcPatternId>::read_wire(reader)?,
            },
            8 => Self::Whole {
                target: AwbcRegisterId::read_wire(reader)?,
                inner: AwbcPatternId::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "pattern",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcPatternRest {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Exact => writer.write_u8(0),
            Self::Ignore => writer.write_u8(1),
            Self::Bind(register) => {
                writer.write_u8(2);
                register.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::Exact),
            1 => Ok(Self::Ignore),
            2 => Ok(Self::Bind(AwbcRegisterId::read_wire(reader)?)),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "pattern rest",
                tag,
                offset,
            }),
        }
    }
}

impl Wire for AwbcRecordPatternField {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.field.write_wire(writer)?;
        self.pattern.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            field: u32::read_wire(reader)?,
            pattern: AwbcPatternId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcMatchArm {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.pattern.write_wire(writer)?;
        self.guard.write_wire(writer)?;
        self.target.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            pattern: AwbcPatternId::read_wire(reader)?,
            guard: Option::<AwbcFunctionId>::read_wire(reader)?,
            target: AwbcBlockId::read_wire(reader)?,
        })
    }
}

#[cfg(test)]
mod opcode_class_tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;

    #[test]
    fn instruction_decoder_rejects_a_known_terminator_opcode() {
        let bytes = [AwbcOpcode::Jump.encoded()];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcInstruction::read_wire(&mut reader)
                .expect_err("terminator cannot enter instruction table"),
            AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: AwbcOpcode::Jump.encoded(),
                offset: 0,
            }
        );
    }

    #[test]
    fn terminator_decoder_rejects_a_known_instruction_opcode() {
        let bytes = [AwbcOpcode::Nop.encoded()];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcTerminator::read_wire(&mut reader)
                .expect_err("instruction cannot enter terminator table"),
            AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: AwbcOpcode::Nop.encoded(),
                offset: 0,
            }
        );
    }

    #[test]
    fn both_opcode_decoders_reject_an_unassigned_byte() {
        let bytes = [0xff];
        let mut instruction = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(matches!(
            AwbcInstruction::read_wire(&mut instruction),
            Err(AwbcCodecError::UnknownTag {
                kind: "instruction opcode",
                tag: 0xff,
                offset: 0,
            })
        ));
        let mut terminator = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert!(matches!(
            AwbcTerminator::read_wire(&mut terminator),
            Err(AwbcCodecError::UnknownTag {
                kind: "terminator opcode",
                tag: 0xff,
                offset: 0,
            })
        ));
    }
}

#[cfg(test)]
mod character_dialogue_wire_tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;

    #[test]
    fn version_one_dialogue_terminator_requires_and_retains_its_target_register() {
        let dialogue = |target| AwbcTerminator::Dialogue {
            target: AwbcRegisterId(target),
            content: AwbcContentUnitId(2),
            values: Vec::new(),
            effects: Vec::new(),
            line_task_captures: Vec::new(),
            result: AwbcDialogueResultTarget {
                ty: AwbcTypeId(3),
                pattern: AwbcPatternId(4),
                destination: AwbcRegisterId(5),
            },
            resume: AwbcResumePointId(6),
        };
        let encode = |terminator: &AwbcTerminator| {
            let mut writer = Writer::with_capacity(32);
            terminator.write_wire(&mut writer).expect("encode dialogue");
            writer.into_bytes()
        };
        let original = dialogue(7);
        let bytes = encode(&original);
        assert_ne!(bytes, encode(&dialogue(8)), "target is part of the v1 wire");
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(AwbcTerminator::read_wire(&mut reader).unwrap(), original);
        reader.finish().expect("complete dialogue terminator");

        let missing_target = [AwbcOpcode::Dialogue.encoded()];
        let mut missing = Reader::new(&missing_target, &AwbcDecodeBudget::default());
        assert!(AwbcTerminator::read_wire(&mut missing).is_err());
    }

    #[test]
    fn version_one_instruction_retains_ordered_set_and_clear_contributions() {
        let instruction = AwbcInstruction::CharacterDialogue {
            destination: AwbcRegisterId(4),
            operation: CharacterDialogueOperation::Factory,
            target: AwbcRegisterId(0),
            fields: vec![
                CharacterDialoguePatchField {
                    coordinate: CharacterDialogueFieldCoordinate::View,
                    operation: CharacterDialoguePatchOperation::Set(AwbcRegisterId(1)),
                },
                CharacterDialoguePatchField {
                    coordinate: CharacterDialogueFieldCoordinate::View,
                    operation: CharacterDialoguePatchOperation::Clear,
                },
                CharacterDialoguePatchField {
                    coordinate: CharacterDialogueFieldCoordinate::Custom(
                        CharacterDialogueCustomFieldId::try_new("character_dialogue_field.mood")
                            .expect("valid custom field"),
                    ),
                    operation: CharacterDialoguePatchOperation::Set(AwbcRegisterId(3)),
                },
            ],
        };
        let mut writer = Writer::with_capacity(64);
        instruction
            .write_wire(&mut writer)
            .expect("encode instruction");
        let bytes = writer.into_bytes();
        assert_eq!(bytes[0], AwbcOpcode::CharacterDialogue.encoded());
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        let decoded = AwbcInstruction::read_wire(&mut reader).expect("decode instruction");
        reader.finish().expect("complete canonical instruction");
        assert_eq!(decoded, instruction);
    }

    #[test]
    fn character_dialogue_wire_rejects_unknown_operation_and_field_tags() {
        let budget = AwbcDecodeBudget::default();
        let mut operation = Reader::new(&[2], &budget);
        assert!(matches!(
            CharacterDialogueOperation::read_wire(&mut operation),
            Err(AwbcCodecError::UnknownTag {
                kind: "CharacterDialogue operation",
                tag: 2,
                ..
            })
        ));
        let mut field = Reader::new(&[0xff], &budget);
        assert!(matches!(
            CharacterDialogueFieldCoordinate::read_wire(&mut field),
            Err(AwbcCodecError::UnknownTag {
                kind: "CharacterDialogue field",
                tag: 0xff,
                ..
            })
        ));
    }
}

#[cfg(test)]
mod callable_specialization_wire_tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;
    use crate::runtime_id::RuntimeCallableSpecializationId;

    #[test]
    fn version_one_callable_specialization_instruction_round_trips() {
        let instruction = AwbcInstruction::SpecializeCallable {
            dst: AwbcRegisterId(4),
            src: AwbcRegisterId(2),
            specialization: RuntimeCallableSpecializationId::from_zero_based(9).unwrap(),
        };
        let mut writer = Writer::with_capacity(16);
        instruction.write_wire(&mut writer).unwrap();
        let bytes = writer.into_bytes();
        assert_eq!(bytes[0], AwbcOpcode::SpecializeCallable.encoded());

        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcInstruction::read_wire(&mut reader).unwrap(),
            instruction
        );
        reader.finish().unwrap();
    }
}

#[cfg(test)]
mod format_content_wire_tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;
    use crate::runtime_id::RuntimeDialogueContentTemplateId;

    #[test]
    fn version_one_format_content_preserves_parameter_and_operand_order() {
        let instruction = AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(1),
            template: RuntimeDialogueContentTemplateId::from_zero_based(0)
                .expect("template identity"),
            attempt: None,
            attempt_operands: Vec::new(),
            project_method: None,
            project_option: false,
            project_result: None,
            operands: vec![
                AwbcFormatOperand {
                    parameter: RuntimeFmtParameterId::Style,
                    function: AwbcFunctionId(7),
                    captures: vec![AwbcRegisterId(10), AwbcRegisterId(11)],
                },
                AwbcFormatOperand {
                    parameter: RuntimeFmtParameterId::Value,
                    function: AwbcFunctionId(8),
                    captures: Vec::new(),
                },
            ],
        };
        let mut writer = Writer::with_capacity(64);
        instruction
            .write_wire(&mut writer)
            .expect("encode FormatContent");
        let bytes = writer.into_bytes();
        assert_eq!(
            bytes,
            [
                0x2a, // opcode
                1,    // destination
                1,    // template
                0,    // no Flow attempt
                0,    // no Flow attempt operands
                0,    // no project DisplayText method
                0,    // project option mode
                0,    // no project-result temporary
                2,    // operand count
                1,    // Style parameter
                7,    // Style function
                2,    // Style capture count
                10, 11, // Style captures, in source order
                0,  // Value parameter
                8,  // Value function
                0,  // Value capture count
            ]
        );
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcInstruction::read_wire(&mut reader).expect("decode FormatContent"),
            instruction
        );
        reader.finish().expect("complete FormatContent instruction");
    }

    #[test]
    fn format_content_wire_rejects_unknown_parameter_identity() {
        let bytes = [
            0x2a, 0, // opcode, destination
            1, // nonzero template identity
            0, 0, // no Flow attempt or attempt operands
            0, 0, 0,    // no project method, option mode, or project-result temporary
            1,    // operand count
            0xff, // unknown parameter
        ];
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcInstruction::read_wire(&mut reader).expect_err("reject forged parameter tag"),
            AwbcCodecError::UnknownTag {
                kind: "fmt parameter",
                tag: 0xff,
                offset: 9,
            }
        );
    }

    #[test]
    fn flow_format_attempt_instructions_round_trip_the_typed_coordinates() {
        let attempt = crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(0)
            .expect("attempt identity");
        let instructions = [
            AwbcInstruction::FormatOperandAttempt {
                attempt,
                parameter: RuntimeFmtParameterId::Style,
            },
            AwbcInstruction::CompleteFormatOperand {
                attempt,
                parameter: RuntimeFmtParameterId::Value,
                value: AwbcRegisterId(6),
            },
            AwbcInstruction::AbandonFormatAttempt { attempt },
        ];
        let expected_opcodes = [0x2b, 0x2c, 0x2d];
        for (instruction, expected_opcode) in instructions.into_iter().zip(expected_opcodes) {
            let mut writer = Writer::with_capacity(16);
            instruction
                .write_wire(&mut writer)
                .expect("encode Flow attempt instruction");
            let bytes = writer.into_bytes();
            assert_eq!(bytes[0], expected_opcode);
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                AwbcInstruction::read_wire(&mut reader).expect("decode Flow attempt instruction"),
                instruction
            );
            reader.finish().expect("complete Flow attempt instruction");
        }
    }

    #[test]
    fn attempted_format_content_wire_keeps_manifest_parameter_and_type_order() {
        let attempt = crate::runtime_id::RuntimeFormatAttemptId::from_zero_based(3)
            .expect("attempt identity");
        let instruction = AwbcInstruction::FormatContent {
            destination: AwbcRegisterId(4),
            template: RuntimeDialogueContentTemplateId::from_zero_based(2)
                .expect("template identity"),
            attempt: Some(attempt),
            attempt_operands: vec![
                AwbcFormatAttemptOperand {
                    parameter: RuntimeFmtParameterId::Style,
                    ty: AwbcTypeId(6),
                },
                AwbcFormatAttemptOperand {
                    parameter: RuntimeFmtParameterId::Value,
                    ty: AwbcTypeId(2),
                },
            ],
            project_method: None,
            project_option: false,
            project_result: None,
            operands: Vec::new(),
        };
        let mut writer = Writer::with_capacity(64);
        instruction
            .write_wire(&mut writer)
            .expect("encode attempt FormatContent");
        let bytes = writer.into_bytes();
        let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
        assert_eq!(
            AwbcInstruction::read_wire(&mut reader).expect("decode attempt FormatContent"),
            instruction
        );
        reader.finish().expect("complete attempt FormatContent");
    }

    #[test]
    fn format_operand_structured_parameter_is_a_stable_numeric_coordinate() {
        let operand = AwbcFormatOperand {
            parameter: RuntimeFmtParameterId::Fallback,
            function: AwbcFunctionId(4),
            captures: vec![AwbcRegisterId(3)],
        };
        let encoded = serde_json::to_value(&operand).expect("serialize format operand");
        assert_eq!(encoded["parameter"], serde_json::json!(7));
        assert_eq!(
            serde_json::from_value::<AwbcFormatOperand>(encoded)
                .expect("deserialize format operand"),
            operand
        );
        assert!(
            serde_json::from_value::<AwbcFormatOperand>(serde_json::json!({
                "parameter": 255,
                "function": 4,
                "captures": []
            }))
            .is_err()
        );
    }
}

#[cfg(test)]
mod function_semantic_role_wire_tests {
    use super::*;
    use crate::awbc::codec::AwbcDecodeBudget;
    use crate::plan::RuntimeFunctionSemanticRole;
    #[test]
    fn all_function_semantic_roles_round_trip_independently_of_execution_kind() {
        for &role in RuntimeFunctionSemanticRole::ALL {
            let function = AwbcFunction {
                semantic_role: role,
                public_id: None,
                kind: AwbcFunctionKind::Ordinary,
                signature: AwbcSignatureId(0),
                type_context: None,
                input_ownership: Vec::new(),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 0),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags::empty(),
            };
            let mut writer = Writer::with_capacity(64);
            function.write_wire(&mut writer).unwrap();
            let bytes = writer.into_bytes();
            assert_eq!(bytes[0], role.semantic_tag());
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(AwbcFunction::read_wire(&mut reader).unwrap(), function);
            reader.finish().unwrap();
            let json = serde_json::to_value(&function).unwrap();
            assert_eq!(
                serde_json::from_value::<AwbcFunction>(json.clone()).unwrap(),
                function
            );
            let mut missing = json;
            missing.as_object_mut().unwrap().remove("semantic_role");
            assert!(serde_json::from_value::<AwbcFunction>(missing).is_err());
        }
    }
    #[test]
    fn function_semantic_role_decoder_rejects_every_unassigned_tag() {
        for tag in 6..=u8::MAX {
            let bytes = [tag];
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                AwbcFunction::read_wire(&mut reader).unwrap_err(),
                AwbcCodecError::UnknownTag {
                    kind: "function semantic role",
                    tag,
                    offset: 0
                }
            );
        }
    }
}

#[cfg(test)]
mod function_input_source_wire_tests {
    use super::*;
    use crate::awbc::{codec::AwbcDecodeBudget, schema::AwbcFunctionInputOwnership};
    use crate::plan::{RuntimeFunctionInputSource, RuntimeFunctionParameterPassing};

    #[test]
    fn mandatory_input_origins_preserve_every_domain_and_fixed_payload() {
        use crate::plan::{
            RuntimeFunctionCaptureMode, RuntimeFunctionDefinitionIdentity,
            RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
            RuntimeGeneratedLocalOrigin, RuntimeLocalOrigin,
        };
        for (tag, origin) in [
            (0, RuntimeLocalOrigin::Binding([0x31; 32])),
            (
                1,
                RuntimeLocalOrigin::Parameter(
                    RuntimeFunctionParameterIdentity::from_accepted_identity([0x31; 32]),
                ),
            ),
            (
                2,
                RuntimeLocalOrigin::EvaluatedResult(
                    RuntimeFunctionDefinitionIdentity::from_accepted_identity([0x31; 32]),
                ),
            ),
            (
                3,
                RuntimeLocalOrigin::Generated(RuntimeGeneratedLocalOrigin::from_accepted_identity(
                    [0x31; 32],
                )),
            ),
        ] {
            let row =
                AwbcFunctionInputOwnership::capture(origin, 0, RuntimeFunctionCaptureMode::Move);
            let mut writer = Writer::with_capacity(64);
            row.write_wire(&mut writer).unwrap();
            let bytes = writer.into_bytes();
            assert_eq!(bytes[0], tag);
            assert_eq!(&bytes[1..33], &[0x31; 32]);
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                AwbcFunctionInputOwnership::read_wire(&mut reader).unwrap(),
                row
            );
            reader.finish().unwrap();
            let mut json = serde_json::to_value(&row).unwrap();
            assert_eq!(
                serde_json::from_value::<AwbcFunctionInputOwnership>(json.clone()).unwrap(),
                row
            );
            json.as_object_mut().unwrap().remove("origin");
            assert!(serde_json::from_value::<AwbcFunctionInputOwnership>(json).is_err());
            for length in 0..33 {
                let mut reader = Reader::new(&bytes[..length], &AwbcDecodeBudget::default());
                assert!(RuntimeLocalOrigin::read_wire(&mut reader).is_err());
            }
            assert!(row.source.accepts_local_origin(row.origin));
            assert_eq!(
                row.transfer,
                RuntimeFunctionInputTransfer::Transferred(RuntimeFunctionCaptureMode::Move)
            );
        }
        for tag in 4..=u8::MAX {
            let bytes = [tag];
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                RuntimeLocalOrigin::read_wire(&mut reader).unwrap_err(),
                AwbcCodecError::UnknownTag {
                    kind: "function input origin",
                    tag,
                    offset: 0
                }
            );
        }
    }

    fn declared_origin(source: RuntimeFunctionInputSource) -> crate::plan::RuntimeLocalOrigin {
        match source {
            RuntimeFunctionInputSource::Capture { .. } => {
                crate::plan::RuntimeLocalOrigin::Binding([0x31; 32])
            }
            RuntimeFunctionInputSource::Parameter { .. }
            | RuntimeFunctionInputSource::CapturedParameter { .. } => {
                crate::plan::RuntimeLocalOrigin::Parameter(
                    crate::plan::RuntimeFunctionParameterIdentity::from_accepted_identity(
                        [0x71; 32],
                    ),
                )
            }
        }
    }

    #[test]
    fn mandatory_transfer_operations_round_trip_and_reject_unknown_tags() {
        use crate::plan::{
            RuntimeFunctionCaptureMode as Mode, RuntimeFunctionInputTransfer as Transfer,
        };
        for transfer in [
            Transfer::Transferred(Mode::Copy),
            Transfer::Transferred(Mode::SnapshotClone),
            Transfer::Transferred(Mode::Move),
            Transfer::ExternalBinding,
            Transfer::Formal,
        ] {
            let source = if transfer == Transfer::Formal {
                RuntimeFunctionInputSource::Parameter {
                    position: 0,
                    passing: RuntimeFunctionParameterPassing::Value,
                }
            } else {
                RuntimeFunctionInputSource::Capture { position: 0 }
            };
            let row = AwbcFunctionInputOwnership::owned(declared_origin(source), source, transfer);
            let mut writer = Writer::with_capacity(32);
            row.write_wire(&mut writer).unwrap();
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                AwbcFunctionInputOwnership::read_wire(&mut reader).unwrap(),
                row
            );
            reader.finish().unwrap();
            let mut json = serde_json::to_value(&row).unwrap();
            assert_eq!(
                serde_json::from_value::<AwbcFunctionInputOwnership>(json.clone()).unwrap(),
                row
            );
            json.as_object_mut().unwrap().remove("transfer");
            assert!(serde_json::from_value::<AwbcFunctionInputOwnership>(json).is_err());
        }
        for tag in 3..=u8::MAX {
            let bytes = [tag];
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert!(matches!(
                Transfer::read_wire(&mut reader),
                Err(AwbcCodecError::UnknownTag {
                    kind: "function input transfer",
                    ..
                })
            ));
            let bytes = [0, tag];
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert!(matches!(
                Transfer::read_wire(&mut reader),
                Err(AwbcCodecError::UnknownTag {
                    kind: "function capture mode",
                    ..
                })
            ));
        }
    }

    #[test]
    fn mandatory_input_sources_and_all_passing_classes_round_trip() {
        let mut sources = vec![RuntimeFunctionInputSource::Capture { position: 7 }];
        for passing in [
            RuntimeFunctionParameterPassing::Value,
            RuntimeFunctionParameterPassing::Shared,
            RuntimeFunctionParameterPassing::Affine,
        ] {
            sources.extend([
                RuntimeFunctionInputSource::Parameter {
                    position: 7,
                    passing,
                },
                RuntimeFunctionInputSource::CapturedParameter {
                    position: 7,
                    passing,
                },
            ]);
        }
        for source in sources {
            let row = AwbcFunctionInputOwnership::owned(
                declared_origin(source),
                source,
                match source {
                    RuntimeFunctionInputSource::Capture { .. } => {
                        crate::plan::RuntimeFunctionInputTransfer::Transferred(
                            crate::plan::RuntimeFunctionCaptureMode::Move,
                        )
                    }
                    _ => crate::plan::RuntimeFunctionInputTransfer::Formal,
                },
            );
            let mut writer = Writer::with_capacity(32);
            row.write_wire(&mut writer).unwrap();
            let bytes = writer.into_bytes();
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                AwbcFunctionInputOwnership::read_wire(&mut reader).unwrap(),
                row
            );
            reader.finish().unwrap();
            let json = serde_json::to_value(&row).unwrap();
            assert_eq!(
                serde_json::from_value::<AwbcFunctionInputOwnership>(json.clone()).unwrap(),
                row
            );
            let mut missing = json;
            missing.as_object_mut().unwrap().remove("source");
            assert!(serde_json::from_value::<AwbcFunctionInputOwnership>(missing).is_err());
        }
        for kind in ["Parameter", "CapturedParameter"] {
            let json = serde_json::json!({ kind: { "position": 0 } });
            assert!(serde_json::from_value::<RuntimeFunctionInputSource>(json).is_err());
        }
    }

    #[test]
    fn input_source_and_passing_decoders_reject_every_unassigned_tag() {
        for tag in 3..=u8::MAX {
            let bytes = [tag];
            let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
            assert_eq!(
                RuntimeFunctionInputSource::read_wire(&mut reader).unwrap_err(),
                AwbcCodecError::UnknownTag {
                    kind: "function input source",
                    tag,
                    offset: 0
                }
            );
            for source_tag in [1, 2] {
                let mut writer = Writer::with_capacity(8);
                writer.write_u8(source_tag);
                0u32.write_wire(&mut writer).unwrap();
                let mut bytes = writer.into_bytes();
                let offset = bytes.len();
                bytes.push(tag);
                let mut reader = Reader::new(&bytes, &AwbcDecodeBudget::default());
                assert_eq!(
                    RuntimeFunctionInputSource::read_wire(&mut reader).unwrap_err(),
                    AwbcCodecError::UnknownTag {
                        kind: "function parameter passing",
                        tag,
                        offset
                    }
                );
            }
        }
    }
}
