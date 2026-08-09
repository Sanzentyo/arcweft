use super::AwbcCodecError;
use super::wire::{Reader, Wire, Writer, wire_enum};
use crate::awbc::schema::{
    AwbcBlockId, AwbcCallableExecutable, AwbcCodeLocation, AwbcContentUnit, AwbcContentUnitId,
    AwbcDigest, AwbcDisplayMapEntry, AwbcDisplayMapId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
    AwbcFlowBinding, AwbcFlowExecutable, AwbcFunctionId, AwbcHeader, AwbcInstructionId,
    AwbcLineTaskGroupId, AwbcProgram, AwbcRegisterId, AwbcResourceId, AwbcResourceRef,
    AwbcResourceResidency, AwbcResumePointId, AwbcRoute, AwbcRouteBinding, AwbcRouteBindingSource,
    AwbcSignatureId, AwbcSourceMapEntry, AwbcSourceMapId, AwbcStringId, AwbcTraitMethod,
    AwbcTraitReceiverMode,
};
use crate::entry::{
    AgentBudget, AgentPolicyHash, CallableContractHash, EntryBindingIdentity, FlowContractHash,
    RootExecutionLimits, RuntimeAgentEntryRoles, RuntimeBytesFormat, RuntimeCallableId,
    RuntimeCallableRole, RuntimeCommandConstructorId, RuntimeCommandContract, RuntimeCommandPolicy,
    RuntimeCommandTargetId, RuntimeEntryRoles, RuntimeEnumRepr, RuntimeEnumTagStyle,
    RuntimeFlowExecutable, RuntimeFlowExecutableParameter, RuntimeFlowParameterMode,
    RuntimeFlowRole, RuntimeNominalRole, RuntimeNominalTypeId, RuntimeSchemaField,
    RuntimeSchemaLimits, RuntimeSchemaVariant, RuntimeStatefulEntryRoles, RuntimeTypeSchema,
    TypeLayoutHash,
};
use crate::plan::{EntryRuntimeId, FlowRuntimeId};

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
        writer.write_table(&self.audio_commands)?;
        writer.write_table(&self.effect_plans)?;
        writer.write_table(&self.choices)?;
        writer.write_table(&self.choice_options)?;
        writer.write_table(&self.content_units)?;
        writer.write_table(&self.line_task_groups)?;
        writer.write_table(&self.line_task_nodes)?;
        writer.write_table(&self.stream_plans)?;
        writer.write_table(&self.source_plans)?;
        writer.write_table(&self.pure_helpers)?;
        writer.write_table(&self.trait_methods)?;
        writer.write_table(&self.display_map)?;
        writer.write_table(&self.source_map)?;
        writer.write_table(&self.resources)?;
        writer.write_table(&self.callable_executables)?;
        writer.write_table(&self.flow_bindings)?;
        writer.write_table(&self.flow_executables)?;
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
            audio_commands: reader.read_table("audio_commands", budget.audio_commands)?,
            effect_plans: reader.read_table("effect_plans", budget.effect_plans)?,
            choices: reader.read_table("choices", budget.choices)?,
            choice_options: reader.read_table("choice_options", budget.choice_options)?,
            content_units: reader.read_table("content_units", budget.content_units)?,
            line_task_groups: reader.read_table("line_task_groups", budget.line_task_groups)?,
            line_task_nodes: reader.read_table("line_task_nodes", budget.line_task_nodes)?,
            stream_plans: reader.read_table("stream_plans", budget.stream_plans)?,
            source_plans: reader.read_table("source_plans", budget.source_plans)?,
            pure_helpers: reader.read_table("pure_helpers", budget.pure_helpers)?,
            trait_methods: reader.read_table("trait_methods", budget.trait_methods)?,
            display_map: reader.read_table("display_map", budget.display_map)?,
            source_map: reader.read_table("source_map", budget.source_map)?,
            resources: reader.read_table("resources", budget.resources)?,
            callable_executables: reader
                .read_table("callable_executables", budget.callable_executables)?,
            flow_bindings: reader.read_table("flow_bindings", budget.flow_bindings)?,
            flow_executables: reader.read_table("flow_executables", budget.flow_executables)?,
            entries: reader.read_table("entries", budget.entries)?,
        })
    }
}

impl Wire for AwbcTraitMethod {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.public_id.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.function.write_wire(writer)?;
        self.receiver.write_wire(writer)?;
        self.receiver_state_slot.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            public_id: AwbcStringId::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            function: AwbcFunctionId::read_wire(reader)?,
            receiver: AwbcTraitReceiverMode::read_wire(reader)?,
            receiver_state_slot: Option::<AwbcRegisterId>::read_wire(reader)?,
        })
    }
}

wire_enum!(AwbcTraitReceiverMode, "trait receiver mode", {
    0 => AwbcTraitReceiverMode::Owned,
    1 => AwbcTraitReceiverMode::SharedRef,
    2 => AwbcTraitReceiverMode::MutRef,
});

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
        self.runtime_id.write_wire(writer)?;
        self.binding.write_wire(writer)?;
        self.public_id.write_wire(writer)?;
        self.kind.write_wire(writer)?;
        self.signature.write_wire(writer)?;
        self.target.write_wire(writer)?;
        self.roles.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            runtime_id: EntryRuntimeId::read_wire(reader)?,
            binding: EntryBindingIdentity::read_wire(reader)?,
            public_id: AwbcStringId::read_wire(reader)?,
            kind: AwbcEntryKind::read_wire(reader)?,
            signature: AwbcSignatureId::read_wire(reader)?,
            target: AwbcEntryTarget::read_wire(reader)?,
            roles: RuntimeEntryRoles::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcCallableExecutable {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.role.write_wire(writer)?;
        self.function.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            role: RuntimeCallableRole::read_wire(reader)?,
            function: AwbcFunctionId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFlowBinding {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.flow.write_wire(writer)?;
        self.function.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            flow: FlowRuntimeId::read_wire(reader)?,
            function: AwbcFunctionId::read_wire(reader)?,
        })
    }
}

impl Wire for AwbcFlowExecutable {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.metadata.write_wire(writer)?;
        self.function.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            metadata: RuntimeFlowExecutable::read_wire(reader)?,
            function: AwbcFunctionId::read_wire(reader)?,
        })
    }
}

macro_rules! wire_digest {
    ($($type:ty),+ $(,)?) => {
        $(
            impl Wire for $type {
                fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
                    self.as_bytes().write_wire(writer)
                }

                fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
                    <[u8; 32]>::read_wire(reader).map(Self::from_bytes)
                }
            }
        )+
    };
}

wire_digest!(
    EntryBindingIdentity,
    TypeLayoutHash,
    CallableContractHash,
    FlowContractHash,
    AgentPolicyHash,
);

impl Wire for RuntimeNominalTypeId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.as_str().to_owned().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let value = String::read_wire(reader)?;
        Self::try_new(value).map_err(|error| AwbcCodecError::InvalidMetadata {
            kind: "nominal type identity",
            message: error.to_string(),
            offset,
        })
    }
}

impl Wire for RuntimeCallableId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.as_str().to_owned().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let value = String::read_wire(reader)?;
        Self::try_new(value).map_err(|error| AwbcCodecError::InvalidMetadata {
            kind: "callable identity",
            message: error.to_string(),
            offset,
        })
    }
}

impl Wire for RuntimeCommandConstructorId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.as_str().to_owned().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let value = String::read_wire(reader)?;
        Self::try_new(value).map_err(|error| AwbcCodecError::InvalidMetadata {
            kind: "command constructor identity",
            message: error.to_string(),
            offset,
        })
    }
}

impl Wire for RuntimeCommandTargetId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.as_str().to_owned().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let value = String::read_wire(reader)?;
        Self::try_new(value).map_err(|error| AwbcCodecError::InvalidMetadata {
            kind: "command target identity",
            message: error.to_string(),
            offset,
        })
    }
}

impl Wire for FlowRuntimeId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.canonical_label().write_wire(writer)?;
        self.public_label().into_string().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let identity = String::read_wire(reader)?;
        let public_id = String::read_wire(reader)?;
        Self::from_runtime_contract(&identity, &public_id).map_err(|error| {
            AwbcCodecError::InvalidMetadata {
                kind: "flow runtime identity",
                message: error.to_string(),
                offset,
            }
        })
    }
}

impl Wire for EntryRuntimeId {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.canonical_label().write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        let value = String::read_wire(reader)?;
        Self::canonical(&value).map_err(|error| AwbcCodecError::InvalidMetadata {
            kind: "entry runtime identity",
            message: error.to_string(),
            offset,
        })
    }
}

impl Wire for RuntimeCallableRole {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.callable.write_wire(writer)?;
        self.contract.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            callable: RuntimeCallableId::read_wire(reader)?,
            contract: CallableContractHash::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeFlowRole {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.flow.write_wire(writer)?;
        self.contract.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            flow: FlowRuntimeId::read_wire(reader)?,
            contract: FlowContractHash::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeFlowExecutableParameter {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.position.write_wire(writer)?;
        self.name.write_wire(writer)?;
        self.mode.write_wire(writer)?;
        self.nominal.write_wire(writer)?;
        self.layout.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            position: u32::read_wire(reader)?,
            name: String::read_wire(reader)?,
            mode: RuntimeFlowParameterMode::read_wire(reader)?,
            nominal: RuntimeNominalTypeId::read_wire(reader)?,
            layout: TypeLayoutHash::read_wire(reader)?,
        })
    }
}

wire_enum!(RuntimeFlowParameterMode, "runtime flow parameter mode", {
    0 => RuntimeFlowParameterMode::Owned,
    1 => RuntimeFlowParameterMode::Shared,
    2 => RuntimeFlowParameterMode::Mutable,
});

impl Wire for RuntimeFlowExecutable {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.flow.write_wire(writer)?;
        self.contract.write_wire(writer)?;
        self.parameters.write_wire(writer)?;
        self.controller.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            flow: FlowRuntimeId::read_wire(reader)?,
            contract: FlowContractHash::read_wire(reader)?,
            parameters: Vec::<RuntimeFlowExecutableParameter>::read_wire(reader)?,
            controller: Option::<RuntimeCallableRole>::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeNominalRole {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.identity.write_wire(writer)?;
        self.layout.write_wire(writer)?;
        self.schema.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            identity: RuntimeNominalTypeId::read_wire(reader)?,
            layout: TypeLayoutHash::read_wire(reader)?,
            schema: RuntimeTypeSchema::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeCommandContract {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.constructor.write_wire(writer)?;
        self.target.write_wire(writer)?;
        self.payload_layout.write_wire(writer)?;
        self.payload_schema.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            constructor: RuntimeCommandConstructorId::read_wire(reader)?,
            target: RuntimeCommandTargetId::read_wire(reader)?,
            payload_layout: TypeLayoutHash::read_wire(reader)?,
            payload_schema: RuntimeTypeSchema::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeCommandPolicy {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.admitted.write_wire(writer)?;
        self.root_limits.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            admitted: Vec::<RuntimeCommandContract>::read_wire(reader)?,
            root_limits: RootExecutionLimits::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeSchemaLimits {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.max_depth.write_wire(writer)?;
        self.max_nodes.write_wire(writer)?;
        self.max_sequence_items.write_wire(writer)?;
        self.max_string_bytes.write_wire(writer)?;
        self.max_encoded_bytes.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            max_depth: usize::read_wire(reader)?,
            max_nodes: usize::read_wire(reader)?,
            max_sequence_items: usize::read_wire(reader)?,
            max_string_bytes: usize::read_wire(reader)?,
            max_encoded_bytes: usize::read_wire(reader)?,
        })
    }
}

impl Wire for RootExecutionLimits {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.schema.write_wire(writer)?;
        self.max_commands_per_transition.write_wire(writer)?;
        self.max_command_bytes_per_transition.write_wire(writer)?;
        self.max_pending_events.write_wire(writer)?;
        self.max_pending_commands.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            schema: RuntimeSchemaLimits::read_wire(reader)?,
            max_commands_per_transition: usize::read_wire(reader)?,
            max_command_bytes_per_transition: usize::read_wire(reader)?,
            max_pending_events: usize::read_wire(reader)?,
            max_pending_commands: usize::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeStatefulEntryRoles {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.binding.write_wire(writer)?;
        self.state.write_wire(writer)?;
        self.initializer.write_wire(writer)?;
        self.event.write_wire(writer)?;
        self.reducer.write_wire(writer)?;
        self.initial_flow.write_wire(writer)?;
        self.command_policy.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            binding: EntryBindingIdentity::read_wire(reader)?,
            state: RuntimeNominalRole::read_wire(reader)?,
            initializer: RuntimeCallableRole::read_wire(reader)?,
            event: RuntimeNominalRole::read_wire(reader)?,
            reducer: RuntimeCallableRole::read_wire(reader)?,
            initial_flow: RuntimeFlowRole::read_wire(reader)?,
            command_policy: RuntimeCommandPolicy::read_wire(reader)?,
        })
    }
}

impl Wire for AgentBudget {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.logical_timeout_millis.write_wire(writer)?;
        self.max_vm_steps.write_wire(writer)?;
        self.max_host_calls.write_wire(writer)?;
        self.max_observations.write_wire(writer)?;
        self.max_captures.write_wire(writer)?;
        self.max_capture_bytes.write_wire(writer)?;
        self.max_rag_queries.write_wire(writer)?;
        self.max_context_bytes.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            logical_timeout_millis: u64::read_wire(reader)?,
            max_vm_steps: u64::read_wire(reader)?,
            max_host_calls: u32::read_wire(reader)?,
            max_observations: u32::read_wire(reader)?,
            max_captures: u32::read_wire(reader)?,
            max_capture_bytes: u64::read_wire(reader)?,
            max_rag_queries: u32::read_wire(reader)?,
            max_context_bytes: u64::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeAgentEntryRoles {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.binding.write_wire(writer)?;
        self.controller.write_wire(writer)?;
        self.policy.write_wire(writer)?;
        self.budget.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            binding: EntryBindingIdentity::read_wire(reader)?,
            controller: RuntimeCallableRole::read_wire(reader)?,
            policy: AgentPolicyHash::read_wire(reader)?,
            budget: AgentBudget::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeEntryRoles {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::None => writer.write_u8(0),
            Self::Stateful(roles) => {
                writer.write_u8(1);
                roles.write_wire(writer)?;
            }
            Self::Agent(roles) => {
                writer.write_u8(2);
                roles.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::None,
            1 => Self::Stateful(Box::new(RuntimeStatefulEntryRoles::read_wire(reader)?)),
            2 => Self::Agent(Box::new(RuntimeAgentEntryRoles::read_wire(reader)?)),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "entry roles",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for RuntimeSchemaField {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.rust_name.write_wire(writer)?;
        self.wire_name.write_wire(writer)?;
        self.schema.write_wire(writer)?;
        self.has_default.write_wire(writer)?;
        self.skip.write_wire(writer)?;
        self.bytes_format.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            rust_name: String::read_wire(reader)?,
            wire_name: String::read_wire(reader)?,
            schema: RuntimeTypeSchema::read_wire(reader)?,
            has_default: bool::read_wire(reader)?,
            skip: bool::read_wire(reader)?,
            bytes_format: Option::<RuntimeBytesFormat>::read_wire(reader)?,
        })
    }
}

impl Wire for RuntimeSchemaVariant {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        self.rust_name.write_wire(writer)?;
        self.wire_name.write_wire(writer)?;
        self.payload.write_wire(writer)?;
        self.discriminant.write_wire(writer)
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        Ok(Self {
            rust_name: String::read_wire(reader)?,
            wire_name: String::read_wire(reader)?,
            payload: Option::<RuntimeTypeSchema>::read_wire(reader)?,
            discriminant: Option::<i128>::read_wire(reader)?,
        })
    }
}

wire_enum!(RuntimeBytesFormat, "runtime bytes format", {
    0 => RuntimeBytesFormat::Binary,
    1 => RuntimeBytesFormat::Base64,
    2 => RuntimeBytesFormat::Hex,
    3 => RuntimeBytesFormat::Array,
});

wire_enum!(RuntimeEnumRepr, "runtime enum repr", {
    0 => RuntimeEnumRepr::I8,
    1 => RuntimeEnumRepr::I16,
    2 => RuntimeEnumRepr::I32,
    3 => RuntimeEnumRepr::I64,
    4 => RuntimeEnumRepr::I128,
    5 => RuntimeEnumRepr::ISize,
    6 => RuntimeEnumRepr::U8,
    7 => RuntimeEnumRepr::U16,
    8 => RuntimeEnumRepr::U32,
    9 => RuntimeEnumRepr::U64,
    10 => RuntimeEnumRepr::U128,
    11 => RuntimeEnumRepr::USize,
});

impl Wire for RuntimeEnumTagStyle {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::External => writer.write_u8(0),
            Self::Internal { tag } => {
                writer.write_u8(1);
                tag.write_wire(writer)?;
            }
            Self::Adjacent { tag, content } => {
                writer.write_u8(2);
                tag.write_wire(writer)?;
                content.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::External,
            1 => Self::Internal {
                tag: String::read_wire(reader)?,
            },
            2 => Self::Adjacent {
                tag: String::read_wire(reader)?,
                content: String::read_wire(reader)?,
            },
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "runtime enum tag style",
                    tag,
                    offset,
                });
            }
        })
    }
}

impl Wire for RuntimeTypeSchema {
    fn write_wire(&self, writer: &mut Writer) -> Result<(), AwbcCodecError> {
        match self {
            Self::Unit => writer.write_u8(0),
            Self::Bool => writer.write_u8(1),
            Self::I8 => writer.write_u8(2),
            Self::I16 => writer.write_u8(3),
            Self::I32 => writer.write_u8(4),
            Self::I64 => writer.write_u8(5),
            Self::I128 => writer.write_u8(6),
            Self::ISize => writer.write_u8(7),
            Self::U8 => writer.write_u8(8),
            Self::U16 => writer.write_u8(9),
            Self::U32 => writer.write_u8(10),
            Self::U64 => writer.write_u8(11),
            Self::U128 => writer.write_u8(12),
            Self::USize => writer.write_u8(13),
            Self::F32 => writer.write_u8(14),
            Self::F64 => writer.write_u8(15),
            Self::String => writer.write_u8(16),
            Self::Char => writer.write_u8(17),
            Self::Bytes { format } => {
                writer.write_u8(18);
                format.write_wire(writer)?;
            }
            Self::Option(value) => {
                writer.write_u8(19);
                value.write_wire(writer)?;
            }
            Self::Seq(value) => {
                writer.write_u8(20);
                value.write_wire(writer)?;
            }
            Self::Map { key, value } => {
                writer.write_u8(21);
                key.write_wire(writer)?;
                value.write_wire(writer)?;
            }
            Self::Record {
                name,
                fields,
                deny_unknown_fields,
            } => {
                writer.write_u8(22);
                name.write_wire(writer)?;
                fields.write_wire(writer)?;
                deny_unknown_fields.write_wire(writer)?;
            }
            Self::Enum {
                name,
                variants,
                tag,
                repr,
            } => {
                writer.write_u8(23);
                name.write_wire(writer)?;
                variants.write_wire(writer)?;
                tag.write_wire(writer)?;
                repr.write_wire(writer)?;
            }
            Self::Named(name) => {
                writer.write_u8(24);
                name.write_wire(writer)?;
            }
        }
        Ok(())
    }

    fn read_wire(reader: &mut Reader<'_>) -> Result<Self, AwbcCodecError> {
        let offset = reader.offset();
        Ok(match reader.read_u8()? {
            0 => Self::Unit,
            1 => Self::Bool,
            2 => Self::I8,
            3 => Self::I16,
            4 => Self::I32,
            5 => Self::I64,
            6 => Self::I128,
            7 => Self::ISize,
            8 => Self::U8,
            9 => Self::U16,
            10 => Self::U32,
            11 => Self::U64,
            12 => Self::U128,
            13 => Self::USize,
            14 => Self::F32,
            15 => Self::F64,
            16 => Self::String,
            17 => Self::Char,
            18 => Self::Bytes {
                format: RuntimeBytesFormat::read_wire(reader)?,
            },
            19 => Self::Option(Box::new(Self::read_wire(reader)?)),
            20 => Self::Seq(Box::new(Self::read_wire(reader)?)),
            21 => Self::Map {
                key: Box::new(Self::read_wire(reader)?),
                value: Box::new(Self::read_wire(reader)?),
            },
            22 => Self::Record {
                name: String::read_wire(reader)?,
                fields: Vec::<RuntimeSchemaField>::read_wire(reader)?,
                deny_unknown_fields: bool::read_wire(reader)?,
            },
            23 => Self::Enum {
                name: String::read_wire(reader)?,
                variants: Vec::<RuntimeSchemaVariant>::read_wire(reader)?,
                tag: RuntimeEnumTagStyle::read_wire(reader)?,
                repr: Option::<RuntimeEnumRepr>::read_wire(reader)?,
            },
            24 => Self::Named(String::read_wire(reader)?),
            tag => {
                return Err(AwbcCodecError::UnknownTag {
                    kind: "runtime type schema",
                    tag,
                    offset,
                });
            }
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
            Self::Editor => writer.write_u8(7),
            Self::Agent => writer.write_u8(8),
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
            7 => Self::Editor,
            8 => Self::Agent,
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
