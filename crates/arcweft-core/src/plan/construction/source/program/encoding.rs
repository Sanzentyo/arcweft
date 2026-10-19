use super::*;
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::RuntimePlan;
use crate::plan::control_effect::source_codec::RuntimeControlSourceLimits;
use crate::plan::local_declarations::source_codec::RuntimeLocalSourceLimits;
use crate::plan::type_table::source_codec::RuntimeTypeSourceLimits;

pub(in crate::plan) struct EncodedRuntimeProgramSource {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeProgramSource {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn encode_program_source(
        &self,
        limits: RuntimeProgramSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeProgramSource, Error> {
        meter.status()?;
        let result = (|| {
            let namespace = Namespaces {
                types: self.type_table().len(),
                locals: self.local_declarations().len(),
                controls: self.control_effect_contracts().len(),
                functions: self.function_sites().len(),
                states: self.callable_states().len(),
                specializations: self.callable_specializations().len(),
                helpers: self.pure_helpers().len(),
                methods: self.trait_methods().len(),
                templates: self.dialogue_content_templates().rows().len(),
                attempts: self.format_attempts().len(),
                contents: self.dialogue_content().rows().len(),
                defers: self.defer_sites().len(),
                tasks: self.task_plans().len(),
                groups: self.line_task_groups().len(),
                projects: self.project_call_sites().len(),
            };
            let mut writer = Writer::new(limits, namespace)?;
            let construction_storage = RuntimePlanSourceConstruction::storage_bytes(
                namespace.types,
                namespace.locals,
                namespace.controls,
            )
            .ok_or(Error::Arithmetic)?;
            writer.bounds.admit(0, 0, construction_storage)?;
            let type_limits = RuntimeTypeSourceLimits {
                rows: limits.rows,
                elements: writer.bounds.declarations(writer.available()).elements,
                encoded_bytes: writer.available(),
                decoded_bytes: writer.bounds.declarations(writer.available()).decoded_bytes,
                policy_depth: limits.policy_depth,
            };
            let prepared = self
                .type_table()
                .prepare_sources_encoding(type_limits, meter)
                .map_err(RuntimePlanDeclarationSourceError::Types)?;
            writer.store(
                Section::Types,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| {
                    prepared.append_to(output).map_err(|error| {
                        Error::Declarations(RuntimePlanDeclarationSourceError::Types(error))
                    })
                },
            )?;
            let bounds = writer.bounds.declarations(writer.available());
            let prepared = self
                .inventory
                .prepare_nominal_sources_encoding(
                    RuntimeTypeSourceLimits {
                        rows: limits.rows,
                        elements: bounds.elements,
                        encoded_bytes: writer.available(),
                        decoded_bytes: bounds.decoded_bytes,
                        policy_depth: limits.policy_depth,
                    },
                    meter,
                )
                .map_err(RuntimePlanDeclarationSourceError::Nominals)?;
            writer.store(
                Section::Nominals,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| {
                    prepared.append_to(output).map_err(|error| {
                        Error::Declarations(RuntimePlanDeclarationSourceError::Nominals(error))
                    })
                },
            )?;
            let bounds = writer.bounds.declarations(writer.available());
            let prepared = self
                .local_declarations()
                .prepare_sources_encoding(
                    RuntimeLocalSourceLimits {
                        rows: limits.rows,
                        encoded_bytes: writer.available(),
                        decoded_bytes: bounds.decoded_bytes,
                        type_count: namespace.types,
                    },
                    meter,
                )
                .map_err(RuntimePlanDeclarationSourceError::Locals)?;
            writer.store(
                Section::Locals,
                prepared.encoded_bytes(),
                0,
                0,
                prepared.storage_bytes(),
                |output| {
                    prepared.append_to(output).map_err(|error| {
                        Error::Declarations(RuntimePlanDeclarationSourceError::Locals(error))
                    })
                },
            )?;
            let bounds = writer.bounds.declarations(writer.available());
            let prepared = self
                .control_effect_contracts()
                .prepare_sources_encoding(
                    RuntimeControlSourceLimits {
                        rows: limits.rows,
                        elements: bounds.elements as usize,
                        encoded_bytes: writer.available(),
                        decoded_bytes: bounds.decoded_bytes,
                        type_count: namespace.types,
                    },
                    meter,
                )
                .map_err(RuntimePlanDeclarationSourceError::Controls)?;
            writer.store(
                Section::Controls,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| {
                    prepared.append_to(output).map_err(|error| {
                        Error::Declarations(RuntimePlanDeclarationSourceError::Controls(error))
                    })
                },
            )?;
            let prepared = self.function_sites().prepare_declaration_sources_encoding(
                writer.bounds.functions(writer.available()),
                resources,
                meter,
            )?;
            writer.store(
                Section::Functions,
                prepared.encoded_bytes(),
                prepared.pattern_nodes(),
                prepared.elements(),
                prepared.storage_bytes(),
                |output| prepared.append_to(output).map_err(Into::into),
            )?;
            let prepared = self
                .callable_states()
                .prepare_sources_encoding(writer.bounds.states(writer.available()), meter)?;
            writer.store(
                Section::States,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| prepared.append_to(output).map_err(Into::into),
            )?;
            let prepared = self.prepare_pure_callable_declaration_sources_encoding(
                writer.bounds.pure(writer.available()),
                meter,
            )?;
            writer.store(
                Section::Pure,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| prepared.append_to(output).map_err(Into::into),
            )?;
            let prepared = self.prepare_content_declaration_sources_encoding(
                writer.bounds.content_declarations(writer.available()),
                meter,
            )?;
            writer.store(
                Section::ContentDeclarations,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| prepared.append_to(output).map_err(Into::into),
            )?;
            let prepared = self.prepare_invocation_sources_encoding(
                writer.bounds.invocations(writer.available()),
                meter,
            )?;
            writer.store(
                Section::Invocations,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| prepared.append_to(output).map_err(Into::into),
            )?;
            let encoded = self.encode_content_plan_sources(
                writer.bounds.contents(writer.available()),
                resources,
                meter,
            )?;
            writer.store(
                Section::Contents,
                encoded.bytes().len(),
                encoded.node_count(),
                encoded.elements(),
                encoded.storage_bytes(),
                |output| encoded.append_to(output).map_err(Into::into),
            )?;
            let encoded = self.encode_stream_sources(
                writer.bounds.streams(writer.available()),
                resources,
                meter,
            )?;
            writer.store(
                Section::Streams,
                encoded.bytes().len(),
                encoded.node_count(),
                encoded.elements(),
                encoded.storage_bytes(),
                |output| encoded.append_to(output).map_err(Into::into),
            )?;
            let encoded = self.encode_function_body_sources(
                writer.bounds.bodies(writer.available()),
                resources,
                meter,
            )?;
            writer.store(
                Section::Bodies,
                encoded.bytes().len(),
                encoded.node_count(),
                encoded.elements(),
                encoded.storage_bytes(),
                |output| encoded.append_to(output).map_err(Into::into),
            )?;
            let encoded = self.encode_line_group_sources(
                writer.bounds.lines(writer.available()),
                resources,
                meter,
            )?;
            writer.store(
                Section::Lines,
                encoded.bytes().len(),
                encoded.node_count(),
                encoded.elements(),
                encoded.storage_bytes(),
                |output| encoded.append_to(output).map_err(Into::into),
            )?;
            let prepared = self
                .prepare_entry_sources_encoding(writer.bounds.entries(writer.available()), meter)?;
            writer.store(
                Section::Entries,
                prepared.encoded_bytes(),
                prepared.node_count(),
                prepared.elements(),
                prepared.storage_bytes(),
                |output| prepared.append_to(output).map_err(Into::into),
            )?;
            // The context-dependent specialization grammar receives the same
            // remaining allowance as the decoder after its other children.
            let prepared = self.prepare_specialization_sources_encoding(
                writer.bounds.specializations(writer.available()),
                meter,
            )?;
            writer.store(
                Section::Specializations,
                prepared.encoded_bytes(),
                0,
                prepared.elements(),
                prepared.storage_bytes(),
                |output| prepared.append_to(output).map_err(Into::into),
            )?;
            meter.charge_work(u64::try_from(writer.length).map_err(|_| Error::Arithmetic)?)?;
            writer.finish()
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
struct Writer {
    bounds: Bounds,
    sections: [Vec<u8>; SECTION_COUNT],
    length: usize,
}
impl Writer {
    fn new(limits: RuntimeProgramSourceLimits, namespace: Namespaces) -> Result<Self, Error> {
        let length = DOMAIN
            .len()
            .checked_add(1 + 15 * 4 + SECTION_COUNT * 4)
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", length, limits.encoded_bytes)?;
        Ok(Self {
            bounds: Bounds::new(limits, namespace)?,
            sections: std::array::from_fn(|_| Vec::new()),
            length,
        })
    }
    fn available(&self) -> usize {
        self.bounds.limits.encoded_bytes - self.length
    }
    fn store(
        &mut self,
        section: Section,
        length: usize,
        nodes: usize,
        elements: usize,
        storage: usize,
        append: impl FnOnce(&mut Vec<u8>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let next = self.length.checked_add(length).ok_or(Error::Arithmetic)?;
        limit("encoded bytes", next, self.bounds.limits.encoded_bytes)?;
        self.bounds.admit(nodes, elements, storage)?;
        let mut bytes = Vec::with_capacity(length);
        append(&mut bytes)?;
        debug_assert_eq!(bytes.len(), length);
        self.sections[section.index()] = bytes;
        self.length = next;
        Ok(())
    }
    fn finish(self) -> Result<EncodedRuntimeProgramSource, Error> {
        let mut bytes = Vec::with_capacity(self.length);
        bytes.extend_from_slice(DOMAIN);
        bytes.push(1);
        for count in self.bounds.namespace.fields() {
            bytes.extend_from_slice(
                &u32::try_from(count)
                    .map_err(|_| Error::Arithmetic)?
                    .to_le_bytes(),
            );
        }
        for section in self.sections {
            bytes.extend_from_slice(
                &u32::try_from(section.len())
                    .map_err(|_| Error::Arithmetic)?
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(&section);
        }
        debug_assert_eq!(bytes.len(), self.length);
        Ok(EncodedRuntimeProgramSource {
            bytes,
            footprint: self.bounds.footprint,
        })
    }
}
