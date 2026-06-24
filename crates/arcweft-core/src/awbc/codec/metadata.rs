use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer, wire_enum};
use crate::awbc::schema::{
    AwbcBlockId, AwbcCodeLocation, AwbcContentUnit, AwbcContentUnitId, AwbcDigest,
    AwbcDisplayMapEntry, AwbcDisplayMapId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
    AwbcFunctionId, AwbcHeader, AwbcInstructionId, AwbcLineTaskGroupId, AwbcProgram,
    AwbcRegisterId, AwbcResourceId, AwbcResourceRef, AwbcResourceResidency, AwbcResumePointId,
    AwbcRoute, AwbcRouteBinding, AwbcRouteBindingSource, AwbcSignatureId, AwbcSourceMapEntry,
    AwbcSourceMapId, AwbcStringId,
};

impl Wire for AwbcProgram {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.header.write_wire(writer)?;
        writer.write_table(&self.strings)?;
        writer.write_table(&self.runtime_types)?;
        writer.write_table(&self.constants)?;
        writer.write_table(&self.effect_sets)?;
        writer.write_table(&self.signatures)?;
        writer.write_table(&self.frame_layouts)?;
        writer.write_table(&self.functions)?;
        writer.write_table(&self.blocks)?;
        writer.write_table(&self.instructions)?;
        writer.write_table(&self.resume_points)?;
        writer.write_table(&self.patterns)?;
        writer.write_table(&self.match_arms)?;
        writer.write_table(&self.intrinsics)?;
        writer.write_table(&self.host_calls)?;
        writer.write_table(&self.task_plans)?;
        writer.write_table(&self.effect_plans)?;
        writer.write_table(&self.choices)?;
        writer.write_table(&self.choice_options)?;
        writer.write_table(&self.content_units)?;
        writer.write_table(&self.line_task_groups)?;
        writer.write_table(&self.line_task_nodes)?;
        writer.write_table(&self.stream_plans)?;
        writer.write_table(&self.source_plans)?;
        writer.write_table(&self.pure_helpers)?;
        writer.write_table(&self.display_map)?;
        writer.write_table(&self.source_map)?;
        writer.write_table(&self.resources)?;
        writer.write_table(&self.entries)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let budget = reader.budget();
        Ok(Self {
            header: AwbcHeader::read_wire(reader)?,
            strings: reader.read_string_table(budget.strings)?,
            runtime_types: reader.read_table("runtime_types", budget.runtime_types)?,
            constants: reader.read_table("constants", budget.constants)?,
            effect_sets: reader.read_table("effect_sets", budget.effect_sets)?,
            signatures: reader.read_table("signatures", budget.signatures)?,
            frame_layouts: reader.read_table("frame_layouts", budget.frame_layouts)?,
            functions: reader.read_table("functions", budget.functions)?,
            blocks: reader.read_table("blocks", budget.blocks)?,
            instructions: reader.read_table("instructions", budget.instructions)?,
            resume_points: reader.read_table("resume_points", budget.resume_points)?,
            patterns: reader.read_table("patterns", budget.patterns)?,
            match_arms: reader.read_table("match_arms", budget.match_arms)?,
            intrinsics: reader.read_table("intrinsics", budget.intrinsics)?,
            host_calls: reader.read_table("host_calls", budget.host_calls)?,
            task_plans: reader.read_table("task_plans", budget.task_plans)?,
            effect_plans: reader.read_table("effect_plans", budget.effect_plans)?,
            choices: reader.read_table("choices", budget.choices)?,
            choice_options: reader.read_table("choice_options", budget.choice_options)?,
            content_units: reader.read_table("content_units", budget.content_units)?,
            line_task_groups: reader.read_table("line_task_groups", budget.line_task_groups)?,
            line_task_nodes: reader.read_table("line_task_nodes", budget.line_task_nodes)?,
            stream_plans: reader.read_table("stream_plans", budget.stream_plans)?,
            source_plans: reader.read_table("source_plans", budget.source_plans)?,
            pure_helpers: reader.read_table("pure_helpers", budget.pure_helpers)?,
            display_map: reader.read_table("display_map", budget.display_map)?,
            source_map: reader.read_table("source_map", budget.source_map)?,
            resources: reader.read_table("resources", budget.resources)?,
            entries: reader.read_table("entries", budget.entries)?,
        })
    }
}

impl Wire for AwbcContentUnit {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.line_task_group.write_wire(writer)?;
        self.display.write_wire(writer)?;
        self.source.write_wire(writer)?;
        self.resources.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            line_task_group: Option::<AwbcLineTaskGroupId>::read_wire(reader)?,
            display: Option::<AwbcDisplayMapId>::read_wire(reader)?,
            source: Option::<AwbcSourceMapId>::read_wire(reader)?,
            resources: Vec::<AwbcResourceId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcDisplayMapEntry {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.content.write_wire(writer)?;
        self.display_key.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            content: AwbcContentUnitId::read_wire(reader)?,
            display_key: AwbcStringId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcSourceMapEntry {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.location.write_wire(writer)?;
        self.source_file.write_wire(writer)?;
        self.start.write_wire(writer)?;
        self.end.write_wire(writer)?;
        self.anchor.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            location: AwbcCodeLocation::read_wire(reader)?,
            source_file: AwbcStringId::read_wire(reader)?,
            start: u32::read_wire(reader)?,
            end: u32::read_wire(reader)?,
            anchor: Option::<AwbcStringId>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcCodeLocation {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Instruction(id) => {
                writer.write_u8(0);
                id.write_wire(writer)?;
            }
            Self::Block(id) => {
                writer.write_u8(1);
                id.write_wire(writer)?;
            }
            Self::ResumePoint(id) => {
                writer.write_u8(2);
                id.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Instruction(AwbcInstructionId::read_wire(reader)?),
            1 => Self::Block(AwbcBlockId::read_wire(reader)?),
            2 => Self::ResumePoint(AwbcResumePointId::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "code location",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcResourceRef {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.kind.write_wire(writer)?;
        self.digest.write_wire(writer)?;
        self.decoded_len.write_wire(writer)?;
        self.residency.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            kind: AwbcStringId::read_wire(reader)?,
            digest: AwbcDigest::read_wire(reader)?,
            decoded_len: u64::read_wire(reader)?,
            residency: AwbcResourceResidency::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcResourceResidency, "resource residency", {
    0 => AwbcResourceResidency::Startup,
    1 => AwbcResourceResidency::OnDemand,
    2 => AwbcResourceResidency::Streaming,
});

impl Wire for AwbcEntry {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.kind.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.target.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            kind: AwbcEntryKind::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            target: AwbcEntryTarget::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcEntryKind {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Game => writer.write_u8(0),
            Self::Cli => writer.write_u8(1),
            Self::Server => writer.write_u8(2),
            Self::Activity => writer.write_u8(3),
            Self::Test => writer.write_u8(4),
            Self::Bench => writer.write_u8(5),
            Self::Custom(value) => {
                writer.write_u8(6);
                value.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Game,
            1 => Self::Cli,
            2 => Self::Server,
            3 => Self::Activity,
            4 => Self::Test,
            5 => Self::Bench,
            6 => Self::Custom(AwbcStringId::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "entry kind",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcEntryTarget {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Function(function) => {
                writer.write_u8(0);
                function.write_wire(writer)?;
            }
            Self::Routes(routes) => {
                writer.write_u8(1);
                routes.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Function(AwbcFunctionId::read_wire(reader)?),
            1 => Self::Routes(Vec::<AwbcRoute>::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "entry target",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for AwbcRoute {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.method.write_wire(writer)?;
        self.path.write_wire(writer)?;
        self.target.write_wire(writer)?;
        self.bindings.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            method: AwbcStringId::read_wire(reader)?,
            path: AwbcStringId::read_wire(reader)?,
            target: AwbcFunctionId::read_wire(reader)?,
            bindings: Vec::<AwbcRouteBinding>::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcRouteBinding {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.register.write_wire(writer)?;
        self.source.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            register: AwbcRegisterId::read_wire(reader)?,
            source: AwbcRouteBindingSource::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcRouteBindingSource {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::PathParameter(name) => {
                writer.write_u8(0);
                name.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        match reader.read_u8()? {
            0 => Ok(Self::PathParameter(AwbcStringId::read_wire(reader)?)),
            tag => Err(AwbcCodecError::UnknownTag {
                kind: "route binding source",
                tag,
                offset,
            }),
        }
    }
}
