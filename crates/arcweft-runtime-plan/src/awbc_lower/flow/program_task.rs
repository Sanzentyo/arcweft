//! Generated task kernels borrow their complete code and lexical context from
//! the original Source. Capture ingress is independent of the parent's ABI.

use super::*;
use arcweft_core::awbc::schema::AwbcFunctionTypeContext;
use arcweft_core::plan::{
    RuntimeFunctionInputSource, RuntimeFunctionInputTransfer, RuntimeFunctionSemanticRole,
    RuntimeGeneratedFunctionRole, RuntimeProgramTaskBodyRef,
};

impl AwbcFlowLowerer<'_, '_> {
    fn lower_program_fanout(
        &mut self,
        frame: &mut FrameBuilder,
        body: &mut FlowBodyBuilder,
        source: &RuntimeProgramTaskBodyRef<'_>,
        fanout: &arcweft_core::plan::RuntimeProgramFanout,
        path: &str,
    ) {
        let Some(task) =
            self.inventory
                .intern_program_task(self.plan, &source.producer().plan, path)
        else {
            return;
        };
        let source_type = self.local_type(fanout.source);
        let collection = frame.local(fanout.source, source_type);
        let payload = self
            .plan
            .type_table()
            .id_for_semantic(arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes(
                *source.producer().handle_type.payload_type().as_bytes(),
            ))
            .expect("original Fanout Source owns its aggregate payload");
        let payload = admitted_plan_type(self.inventory, self.plan, payload);
        let result = frame.temp(payload);
        let binding = self.inventory.intern_pattern(AwbcPattern::Bind {
            target: result,
            mutable: false,
            expected: Some(payload),
        });
        body.suspend(self.inventory, AwbcSafePointKind::AwaitMany, |resume| {
            AwbcTerminator::AwaitMany {
                plan: task,
                source: collection,
                binding: Some(binding),
                resume,
            }
        });
        body.terminate(
            self.inventory,
            AwbcTerminator::Return {
                value: Some(result),
            },
            AwbcSafePointKind::CallableBoundary,
        );
    }

    pub(crate) fn lower_program_task(
        &mut self,
        owner: AwbcFunctionId,
        source: &RuntimeProgramTaskBodyRef<'_>,
        path: &str,
    ) {
        let producer = source.producer();
        let mut frame = FrameBuilder::new();
        let mut params = Vec::with_capacity(source.captures().len());
        let mut input_ownership = Vec::with_capacity(source.captures().len());
        for (position, capture) in source.captures().iter().enumerate() {
            let local = capture.local;
            let declaration = self
                .plan
                .local_declarations()
                .get(local)
                .expect("original Program Source owns every capture declaration");
            let ty = admitted_plan_type(self.inventory, self.plan, declaration.ty());
            let name = self.inventory.local_name(local);
            frame.named_parameter(local, ty, name);
            params.push(ty);
            input_ownership.push(AwbcFunctionInputOwnership::owned(
                declaration.origin(),
                RuntimeFunctionInputSource::Capture {
                    position: table_index(position),
                },
                RuntimeFunctionInputTransfer::ExternalBinding,
            ));
        }
        let mut body =
            FlowBodyBuilder::new(self.inventory, owner, AwbcSafePointKind::CallableBoundary);
        let previous_effects = self
            .active_effect_set
            .replace(source.function().invocation_effects().clone());
        match source.body() {
            arcweft_core::plan::RuntimeProgramTaskKernelRef::Flow(ops) => {
                self.lower_ops(&mut frame, &mut body, ops, path)
            }
            arcweft_core::plan::RuntimeProgramTaskKernelRef::Fanout(fanout) => {
                self.lower_program_fanout(&mut frame, &mut body, source, fanout, path)
            }
        }
        self.active_effect_set = previous_effects;
        let result = self
            .plan
            .type_table()
            .id_for_semantic(arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes(
                *producer.handle_type.payload_type().as_bytes(),
            ))
            .expect("original Program Source owns its payload type");
        let result_type = self.close_executable_body(&mut frame, &mut body, result, false);
        let body = body.finish(self.inventory);
        let layout = self
            .inventory
            .intern_frame_layout(format!("{path}:frame"), frame.finish());
        for resume in &body.resume_points {
            self.inventory.program.resume_points[resume.index()].frame_layout = layout;
        }
        let effects = self
            .inventory
            .intern_effect_set(source.function().invocation_effects());
        let signature = self
            .inventory
            .intern_signature(params, Some(result_type), effects);
        let type_context = source.function().function_type().map(|ty| {
            AwbcFunctionTypeContext::ProgramBody(admitted_plan_type(self.inventory, self.plan, ty))
        });
        let mut flags = AwbcFunctionFlags::empty()
            .with(AwbcFunctionFlag::Deterministic)
            .with(AwbcFunctionFlag::MayAllocate)
            .with(AwbcFunctionFlag::MaySuspend);
        if body.has_dynamic_target {
            flags = flags.with(AwbcFunctionFlag::HasDynamicTarget);
        }
        self.inventory.replace_function(
            owner,
            AwbcFunction {
                definition: producer
                    .function
                    .generated_child(RuntimeGeneratedFunctionRole::TaskBody),
                semantic_role: RuntimeFunctionSemanticRole::Ordinary,
                public_id: None,
                kind: AwbcFunctionKind::ProgramTask,
                signature,
                type_context,
                input_ownership,
                frame_layout: layout,
                blocks: body.blocks,
                entry_block: body.entry_block,
                flags,
            },
        );
    }
}
