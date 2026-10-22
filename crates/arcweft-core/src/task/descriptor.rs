//! Borrowed descriptor fields shared by live and inert task storage.

use super::{HostTaskRequest, RuntimeNeedHandleError, TaskOutcomeContract, TaskSpec};
use crate::entry::RuntimeSchemaError;

#[cfg(test)]
mod tests;

pub(crate) trait TaskDescriptorVisitor {
    fn node(&mut self, depth: usize) -> Result<(), RuntimeSchemaError>;
    fn fixed(&mut self, bytes: &[u8], depth: usize) -> Result<(), RuntimeSchemaError>;
    fn string(&mut self, text: &str, depth: usize) -> Result<(), RuntimeSchemaError>;
    fn collection(&mut self, count: usize, depth: usize) -> Result<(), RuntimeSchemaError>;
    fn blob(&mut self, bytes: &[u8], depth: usize) -> Result<(), RuntimeSchemaError>;
}

impl<P, S: super::TaskExecutionSource> TaskSpec<P, super::TaskPlanSemanticDigest, S> {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        self.admit_retained_with_handle(None, admission)
    }

    pub(super) fn admit_retained_with_handle(
        &self,
        handle: Option<super::TaskHandle>,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        self.request.admit_source(admission)?;
        admission.admit_composite(P::admission_origin(), |owner, resources| {
            self.validate_descriptor_inputs(resources, 0, P::borrowed_view)?;
            if let Some(handle) = handle {
                self.validate_handle(handle).map_err(|error| {
                    crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: error.to_string(),
                    }
                })?;
            }
            self.validate_admitted_storage(owner, resources)
        })
    }

    pub(crate) fn validate_admitted_storage(
        &self,
        owner: Option<&super::RuntimeProgramOwner>,
        resources: &crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        self.validate_outcome().map_err(|error| {
            crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            }
        })?;
        self.validate_outcome_storage_owner(owner, resources)?;
        for payload in self.request.payloads() {
            let view = payload.borrowed_view();
            crate::entry::schema::value_encoding::visit_admitted_storage(
                view,
                &resources.work(),
                |node| {
                    if node.intrinsic_ownership().permits_copy() {
                        Ok(())
                    } else {
                        Err(crate::value::AwbcRuntimeValueSnapshotError::AffineValue)
                    }
                },
            )?;
            crate::value::AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                view, owner, resources,
            )?;
        }
        Ok(())
    }
    pub(super) fn validate_task_descriptor(&self) -> Result<(), super::TaskEnsureError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            P::admission_origin(),
        );
        self.validate_descriptor_inputs(&mut resources, 0, P::borrowed_view)
            .map_err(|error| super::TaskEnsureError::Descriptor(Box::new(error)))
    }
    pub(super) fn validate_need_descriptor(&self) -> Result<(), RuntimeNeedHandleError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            P::admission_origin(),
        );
        self.validate_descriptor_inputs(&mut resources, 0, P::borrowed_view)
            .map_err(|error| RuntimeNeedHandleError::Descriptor(Box::new(error)))
    }
}

impl<P, S: super::TaskExecutionSource> TaskSpec<P, super::TaskPlanSemanticDigest, S> {
    pub(crate) fn validate_descriptor_inputs(
        &self,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
        depth: usize,
        view: impl Fn(&P) -> crate::value::RuntimeValueView<'_>,
    ) -> Result<(), RuntimeSchemaError> {
        resources.validate_task_descriptor(self, depth)?;
        for value in self.request.payloads() {
            resources.validate_view(view(value), depth + 1, None)?;
        }
        Ok(())
    }
    pub(crate) fn visit_descriptor(
        &self,
        visitor: &mut impl TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        visitor.node(depth)?;
        let next = depth + 1;
        visitor.fixed(&self.generation.get().to_le_bytes(), next)?;
        for digest in [
            self.producer.key().as_bytes(),
            self.producer.contract().as_bytes(),
            self.producer.plan().as_bytes(),
            self.producer.payload_type().as_bytes(),
            self.producer.arguments().as_bytes(),
        ] {
            visitor.fixed(digest, next)?;
        }
        // The issued key already commits the invocation's presence. Absent
        // metadata has no rows, nodes or bytes in this logical descriptor.
        if let Some(binding) = self.producer.binding() {
            visitor.fixed(binding.context().as_bytes(), next)?;
            visitor.collection(binding.source_rows().len(), next)?;
            for row in binding.source_rows() {
                visitor.collection(row.len(), next + 1)?;
                for effect in row {
                    visitor.string(effect.as_str(), next + 2)?;
                }
            }
        }
        visitor.fixed(&[self.class.semantic_tag()], next)?;
        visitor.fixed(&self.priority.0.to_le_bytes(), next)?;
        visitor.string(&self.cancel_scope.0, next)?;
        visitor.fixed(&[self.policy.semantic_tag()], next)?;
        match &self.outcome {
            TaskOutcomeContract::Program { payload } => {
                visitor.fixed(&[1], next)?;
                visitor.fixed(payload.as_bytes(), next)?;
            }
            TaskOutcomeContract::Standalone { payload } => {
                visitor.fixed(&[0], next)?;
                payload.visit_standalone_descriptor(visitor, next)?;
            }
        }
        self.request.visit_descriptor(visitor, next)?;
        visitor.string(&self.debug_label, next)
    }
}

impl<P> HostTaskRequest<P> {
    #[expect(
        clippy::too_many_lines,
        reason = "The closed fourteen-family request grammar owns all metadata and collection bounds"
    )]
    pub(super) fn visit_descriptor(
        &self,
        visitor: &mut impl TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        visitor.node(depth)?;
        let next = depth + 1;
        match self {
            Self::FileReadText(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::FileReadText.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.path, next)?;
            }
            Self::FileReadBytes(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::FileReadBytes.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.path, next)?;
            }
            Self::FileWriteText(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::FileWriteText.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.path, next)?;
                visitor.string(&r.text, next)?;
            }
            Self::FileWriteBytes(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::FileWriteBytes.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.path, next)?;
                visitor.blob(&r.bytes, next)?;
            }
            Self::HttpFetch(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::HttpFetch.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.url, next)?;
                visitor.string(&r.method, next)?;
                visitor.collection(r.headers.len(), next)?;
                for (name, value) in &r.headers {
                    visitor.string(name, next + 1)?;
                    visitor.string(value, next + 1)?;
                }
                visitor.fixed(&[u8::from(r.body.is_some())], next)?;
            }
            Self::HttpRespond(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::HttpRespond.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.request_id, next)?;
                visitor.fixed(&r.status.to_le_bytes(), next)?;
                visitor.collection(r.headers.len(), next)?;
                for (name, value) in &r.headers {
                    visitor.string(name, next + 1)?;
                    visitor.string(value, next + 1)?;
                }
                visitor.fixed(&[u8::from(r.body.is_some())], next)?;
            }
            Self::ProcessRun(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::ProcessRun.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.program, next)?;
                visitor.collection(r.args.len(), next)?;
                for arg in &r.args {
                    visitor.string(arg, next + 1)?;
                }
                visitor.collection(r.env.len(), next)?;
                for (name, value) in &r.env {
                    visitor.string(name, next + 1)?;
                    visitor.string(value, next + 1)?;
                }
            }
            Self::AssetLoad(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::AssetLoad.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.id, next)?;
                visitor.string(&r.kind, next)?;
            }
            Self::ShaderCompile(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::ShaderCompile.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.id, next)?;
                visitor.fixed(&[u8::from(r.entry.is_some())], next)?;
                if let Some(entry) = &r.entry {
                    visitor.string(entry, next)?;
                }
            }
            Self::AudioDecode(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::AudioDecode.semantic_tag()],
                    next,
                )?;
                visitor.string(&r.id, next)?;
            }
            Self::TtsSynthesis(r) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::TtsSynthesis.semantic_tag()],
                    next,
                )?;
                visitor.fixed(&[u8::from(r.voice.is_some())], next)?;
                if let Some(voice) = &r.voice {
                    visitor.string(voice, next)?;
                }
                visitor.string(&r.text, next)?;
            }
            Self::WasmCall(r) => {
                visitor.fixed(&[super::HostTaskRequestKind::WasmCall.semantic_tag()], next)?;
                visitor.string(&r.module, next)?;
                visitor.string(&r.function, next)?;
                visitor.collection(r.args.len(), next)?;
            }
            Self::SystemInfo(_) => {
                visitor.fixed(
                    &[super::HostTaskRequestKind::SystemInfo.semantic_tag()],
                    next,
                )?;
                visitor.fixed(
                    &[super::HostTaskRequestKind::FileReadText.semantic_tag()],
                    next,
                )?;
            }
            Self::Custom {
                capability,
                operation,
                args,
                named_args,
                manifest_contract,
            } => {
                visitor.fixed(&[super::HostTaskRequestKind::Custom.semantic_tag()], next)?;
                visitor.string(&capability.0, next)?;
                visitor.string(operation, next)?;
                visitor.collection(args.len(), next)?;
                visitor.collection(named_args.len(), next)?;
                for arg in named_args {
                    visitor.string(&arg.name, next + 1)?;
                }
                visitor.fixed(&[u8::from(manifest_contract.is_some())], next)?;
                if let Some(contract) = manifest_contract {
                    visitor.fixed(contract.as_bytes(), next)?;
                }
            }
        }
        Ok(())
    }
}
