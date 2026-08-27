use super::{
    AwaitState, AwaitTarget, AwbcContentUnitId, AwbcFunctionId, AwbcHostCallId,
    AwbcProductExecutorStatus, AwbcProductStepExecutor, AwbcResumePointId, AwbcTaskPlanId,
    AwbcTrapCode, ChoiceRuntimeOption, ChoiceState, FiberAwaitTarget, FiberStatus,
    FiberSuspensionReason, FiberTerminalValue, FiberTrap, FlowExit, FlowFiberStatus, HostCallState,
    HostTaskRequestTemplate, MappedEffect, NeedId, ProductStepError, RuntimeDiagnostic,
    RuntimeDiagnosticCategory, RuntimeHostCallId, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepOutput, RuntimeStepStopReason, TaskId, has_host_requests, has_visible_output,
    runtime_value_label, source_diagnostic,
};

impl AwbcProductStepExecutor {
    pub(super) fn resume_at(
        &mut self,
        resume: AwbcResumePointId,
        output: &mut RuntimeStepOutput,
    ) -> bool {
        match self.fiber.resume_at(&self.program, resume) {
            Ok(()) => true,
            Err(error) => {
                self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                false
            }
        }
    }

    pub(super) fn fail_with_error(
        &mut self,
        error: ProductStepError,
        output: &mut RuntimeStepOutput,
    ) {
        let (category, code, message) = match error {
            ProductStepError::Type(message) => (
                RuntimeDiagnosticCategory::Type,
                AwbcTrapCode::TypeMismatch,
                message,
            ),
            ProductStepError::Host(message) => (
                RuntimeDiagnosticCategory::Host,
                AwbcTrapCode::HostAbiMismatch,
                message,
            ),
            ProductStepError::Input(message) => (
                RuntimeDiagnosticCategory::Input,
                AwbcTrapCode::InternalInvariant,
                message,
            ),
            ProductStepError::Internal(message) => (
                RuntimeDiagnosticCategory::Internal,
                AwbcTrapCode::InternalInvariant,
                message,
            ),
            ProductStepError::Line(error) => (
                RuntimeDiagnosticCategory::Internal,
                AwbcTrapCode::InternalInvariant,
                error.to_string(),
            ),
            ProductStepError::LineTaskCompletion(error) => (
                RuntimeDiagnosticCategory::Internal,
                AwbcTrapCode::InternalInvariant,
                error.to_string(),
            ),
            error @ (ProductStepError::DialogueContentIdentityOverflow
            | ProductStepError::DialogueOccurrenceOverflow
            | ProductStepError::ChildGenerationOverflow
            | ProductStepError::DialogueLineCursorOverflow
            | ProductStepError::StaleLineTaskChildContent { .. }) => (
                RuntimeDiagnosticCategory::Internal,
                AwbcTrapCode::InternalInvariant,
                error.to_string(),
            ),
            ProductStepError::RuntimeIdentity(error) => (
                RuntimeDiagnosticCategory::Internal,
                AwbcTrapCode::InternalInvariant,
                error.to_string(),
            ),
            ProductStepError::Fiber(error) => (
                RuntimeDiagnosticCategory::Internal,
                AwbcTrapCode::InternalInvariant,
                error.to_string(),
            ),
        };
        output
            .diagnostics
            .push(RuntimeDiagnostic::categorized(category, message.clone()));
        self.terminate_with_trap(
            FiberTrap {
                code,
                message: Some(message),
                source_map: None,
            },
            output,
        );
    }

    pub(super) fn fail_with_trap(
        &mut self,
        code: AwbcTrapCode,
        message: String,
        source_map: Option<crate::awbc::schema::AwbcSourceMapId>,
        output: &mut RuntimeStepOutput,
    ) {
        let trap = FiberTrap {
            code,
            message: Some(message),
            source_map,
        };
        self.terminate_with_trap(trap, output);
    }

    pub(super) fn terminate_with_trap(&mut self, trap: FiberTrap, output: &mut RuntimeStepOutput) {
        if matches!(
            self.fiber.status,
            FiberStatus::Returned | FiberStatus::Cancelled | FiberStatus::Trapped
        ) {
            return;
        }
        for cleanup in self.fiber.take_unwind_cleanups() {
            self.emit_effect(cleanup.effect, &cleanup.args, output);
        }
        self.record_trap(&trap, output);
        self.fiber.mark_trapped(trap);
    }

    pub(super) fn record_error(&self, error: ProductStepError, output: &mut RuntimeStepOutput) {
        let (category, message) = match error {
            ProductStepError::Input(message) => (RuntimeDiagnosticCategory::Input, message),
            ProductStepError::Type(message) => (RuntimeDiagnosticCategory::Type, message),
            ProductStepError::Host(message) => (RuntimeDiagnosticCategory::Host, message),
            ProductStepError::Internal(message) => (RuntimeDiagnosticCategory::Internal, message),
            ProductStepError::Line(error) => {
                (RuntimeDiagnosticCategory::Internal, error.to_string())
            }
            ProductStepError::LineTaskCompletion(error) => {
                (RuntimeDiagnosticCategory::Internal, error.to_string())
            }
            error @ (ProductStepError::DialogueContentIdentityOverflow
            | ProductStepError::DialogueOccurrenceOverflow
            | ProductStepError::ChildGenerationOverflow
            | ProductStepError::DialogueLineCursorOverflow
            | ProductStepError::StaleLineTaskChildContent { .. }) => {
                (RuntimeDiagnosticCategory::Internal, error.to_string())
            }
            ProductStepError::RuntimeIdentity(error) => {
                (RuntimeDiagnosticCategory::Internal, error.to_string())
            }
            ProductStepError::Fiber(error) => {
                (RuntimeDiagnosticCategory::Internal, error.to_string())
            }
        };
        output
            .diagnostics
            .push(source_diagnostic(&self.program, None, category, message));
    }

    pub(super) fn record_trap(&self, trap: &FiberTrap, output: &mut RuntimeStepOutput) {
        let category = match trap.code {
            AwbcTrapCode::TypeMismatch => RuntimeDiagnosticCategory::Type,
            AwbcTrapCode::PatternMismatch => RuntimeDiagnosticCategory::Pattern,
            AwbcTrapCode::HostAbiMismatch => RuntimeDiagnosticCategory::Host,
            AwbcTrapCode::CapabilityDenied => RuntimeDiagnosticCategory::Capability,
            AwbcTrapCode::DivisionByZero
            | AwbcTrapCode::InvalidIndex
            | AwbcTrapCode::MissingDynamicTarget
            | AwbcTrapCode::ExplicitPanic
            | AwbcTrapCode::UninitializedRegister => RuntimeDiagnosticCategory::Runtime,
            AwbcTrapCode::InternalInvariant => RuntimeDiagnosticCategory::Internal,
        };
        let message = trap
            .message
            .clone()
            .unwrap_or_else(|| format!("AWBC trap {:?}", trap.code));
        let diagnostic = source_diagnostic(&self.program, trap.source_map, category, message);
        if !output.diagnostics.contains(&diagnostic) {
            output.diagnostics.push(diagnostic);
        }
    }

    pub(super) fn stop_reason(
        &self,
        options: RuntimeStepOptions,
        executed_ops: usize,
        output: &RuntimeStepOutput,
    ) -> RuntimeStepStopReason {
        if self.fiber.status == FiberStatus::Trapped {
            return RuntimeStepStopReason::Failed;
        }
        if matches!(
            self.fiber.status,
            FiberStatus::Returned | FiberStatus::Cancelled
        ) && self.child_fibers.is_empty()
        {
            return RuntimeStepStopReason::Done;
        }
        if matches!(
            self.fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::BudgetYield)
        ) {
            return RuntimeStepStopReason::BudgetExhausted;
        }
        if self.fiber.status == FiberStatus::Suspended {
            return if has_visible_output(output) || has_host_requests(output) {
                RuntimeStepStopReason::Output
            } else {
                RuntimeStepStopReason::Blocked
            };
        }
        if options.mode == RuntimeStepMode::Game && has_visible_output(output) {
            return RuntimeStepStopReason::Output;
        }
        if options.mode == RuntimeStepMode::OneOp && executed_ops > 0 {
            return RuntimeStepStopReason::OneOp;
        }
        if executed_ops >= options.budget.max_ops {
            return RuntimeStepStopReason::BudgetExhausted;
        }
        if has_host_requests(output) {
            RuntimeStepStopReason::Output
        } else {
            RuntimeStepStopReason::OneOp
        }
    }

    pub(super) fn should_return_to_host(
        &self,
        mode: RuntimeStepMode,
        output: &RuntimeStepOutput,
        executed_ops: usize,
    ) -> bool {
        if self.fiber.status == FiberStatus::Trapped {
            return true;
        }
        if matches!(
            self.fiber.status,
            FiberStatus::Returned | FiberStatus::Cancelled
        ) && self.child_fibers.is_empty()
        {
            return true;
        }
        if matches!(
            self.fiber
                .suspension
                .as_ref()
                .map(|suspension| &suspension.reason),
            Some(FiberSuspensionReason::BudgetYield)
        ) {
            return true;
        }
        if self.fiber.status == FiberStatus::Suspended {
            return true;
        }
        match mode {
            RuntimeStepMode::OneOp => executed_ops > 0,
            RuntimeStepMode::Game => has_visible_output(output),
            RuntimeStepMode::Drain | RuntimeStepMode::Server => false,
        }
    }

    pub(super) fn sync_facade(&mut self) {
        self.facade_fiber.line_cursor =
            usize::try_from(self.fiber.line_cursor).unwrap_or(usize::MAX);
        self.facade_fiber.status = self.effective_status();
    }

    pub(super) fn effective_status(&self) -> FlowFiberStatus {
        match self.product_status() {
            AwbcProductExecutorStatus::Shared(status) => *status,
            AwbcProductExecutorStatus::WaitingMany(state) => {
                // The shared structured facade has no evaluated-source
                // waiting-many carrier. Preserve the exact items and binding
                // in the compact status, and expose only a coarse suspension.
                FlowFiberStatus::Waiting(Box::new(AwaitState {
                    binding: None,
                    target: AwaitTarget::new(
                        self.task_need_id(state.plan),
                        TaskId(self.task_public_id(state.plan)),
                        HostTaskRequestTemplate::new("awbc", "await_many", []),
                    ),
                    observers: Vec::new(),
                    resume: None,
                    observed_through: None,
                    queued: std::collections::VecDeque::new(),
                }))
            }
        }
    }

    fn product_status(&self) -> AwbcProductExecutorStatus {
        if !self.child_fibers.is_empty()
            && matches!(
                self.fiber.status,
                FiberStatus::Returned | FiberStatus::Cancelled | FiberStatus::Suspended
            )
        {
            return AwbcProductExecutorStatus::Shared(Box::new(FlowFiberStatus::Running));
        }
        AwbcProductExecutorStatus::Shared(Box::new(match self.fiber.status {
            FiberStatus::Running => FlowFiberStatus::Running,
            FiberStatus::Returned => match self.fiber.terminal.as_ref() {
                Some(FiberTerminalValue::Returned(Some(value))) => {
                    FlowFiberStatus::Done(FlowExit::Return(runtime_value_label(value)))
                }
                _ => FlowFiberStatus::Done(FlowExit::Done),
            },
            FiberStatus::Cancelled => FlowFiberStatus::Done(FlowExit::Done),
            FiberStatus::Trapped => match self.fiber.terminal.as_ref() {
                Some(FiberTerminalValue::Trapped(trap)) => FlowFiberStatus::Failed(
                    trap.message
                        .clone()
                        .unwrap_or_else(|| format!("AWBC trap {:?}", trap.code)),
                ),
                _ => FlowFiberStatus::Failed("AWBC fiber trapped".to_owned()),
            },
            FiberStatus::Suspended => return self.product_suspension_status(),
        }))
    }

    fn product_suspension_status(&self) -> AwbcProductExecutorStatus {
        let Some(suspension) = self.fiber.suspension.as_ref() else {
            return AwbcProductExecutorStatus::Shared(Box::new(FlowFiberStatus::Running));
        };
        if let FiberSuspensionReason::AwaitMany(state) = &suspension.reason {
            return AwbcProductExecutorStatus::WaitingMany(state.clone());
        }
        AwbcProductExecutorStatus::Shared(Box::new(self.suspension_status()))
    }

    fn suspension_status(&self) -> FlowFiberStatus {
        let Some(suspension) = self.fiber.suspension.as_ref() else {
            return FlowFiberStatus::Running;
        };
        match &suspension.reason {
            FiberSuspensionReason::Dialogue {
                content: _,
                values: _,
                line_task_captures: _,
                result: _,
            } => self.dialogues.active_frame().map_or(
                FlowFiberStatus::Failed(
                    "AWBC dialogue suspension is missing its active typed owner".to_owned(),
                ),
                |active| FlowFiberStatus::Dialogue(active.activation.clone()),
            ),
            FiberSuspensionReason::Choice { .. } => {
                let active = self.active_choice.as_ref();
                FlowFiberStatus::Choice(ChoiceState {
                    id: active.and_then(|active| active.public_id.clone()),
                    options: active
                        .map(|active| active.options.clone())
                        .unwrap_or_default(),
                    resume: None,
                })
            }
            FiberSuspensionReason::Await { target, .. } => match target {
                FiberAwaitTarget::Task(task) => {
                    let task = TaskId(runtime_value_label(task));
                    let plan = self.task_plan_for_id(&task.0);
                    FlowFiberStatus::Waiting(Box::new(AwaitState {
                        binding: None,
                        target: AwaitTarget::new(
                            plan.map_or_else(
                                || NeedId(task.0.clone()),
                                |plan| self.task_need_id(plan),
                            ),
                            task,
                            HostTaskRequestTemplate::new("awbc", "await", []),
                        ),
                        observers: Vec::new(),
                        resume: None,
                        observed_through: None,
                        queued: std::collections::VecDeque::new(),
                    }))
                }
                FiberAwaitTarget::Need(need) => FlowFiberStatus::NeedWaiting(need.clone()),
            },
            FiberSuspensionReason::HostCall { call, .. } => self.host_call_status(*call),
            FiberSuspensionReason::AwaitMany(_) | FiberSuspensionReason::BudgetYield => {
                FlowFiberStatus::Running
            }
        }
    }

    pub(super) fn host_call_status(&self, call: AwbcHostCallId) -> FlowFiberStatus {
        let record = self.program.host_calls.get(call.index());
        let public_id = record
            .and_then(|record| self.program.strings.get(record.public_id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.host_call.{}", call.0));
        let id = self.pending_host_call.as_ref().map_or_else(
            || RuntimeHostCallId(public_id.clone()),
            |pending| pending.id.clone(),
        );
        FlowFiberStatus::HostCall(HostCallState {
            binding: None,
            id,
            resume: None,
        })
    }

    pub(super) fn choice_runtime_option(
        &self,
        option: &crate::awbc::schema::AwbcChoiceOption,
    ) -> ChoiceRuntimeOption {
        let mut effects = Vec::new();
        for effect in &option.effects {
            if let Some(plan) = self.program.effect_plans.get(effect.index())
                && let MappedEffect::Line(effect) =
                    plan.kind.map_product_effect(&self.program, *effect, &[])
            {
                effects.push(effect);
            }
        }
        let out = option.out_effect.and_then(|effect| {
            let plan = self.program.effect_plans.get(effect.index())?;
            match plan.kind.map_product_effect(&self.program, effect, &[]) {
                MappedEffect::Line(crate::effect::LineEffectRequest::Out(out)) => Some(out),
                _ => None,
            }
        });
        ChoiceRuntimeOption {
            id: option
                .public_id
                .and_then(|id| self.program.strings.get(id.index()).cloned()),
            label: self
                .program
                .strings
                .get(option.label.index())
                .cloned()
                .unwrap_or_else(|| "choice".to_owned()),
            target: option.target.map(|target| {
                self.program
                    .flow_identity(target)
                    .cloned()
                    .expect("verified AWBC choice target must own a typed Flow binding")
            }),
            out,
            effects,
        }
    }

    pub(super) fn content_public_id(&self, content: AwbcContentUnitId) -> String {
        self.program
            .content_units
            .get(content.index())
            .and_then(|content| self.program.strings.get(content.public_id.index()))
            .cloned()
            .unwrap_or_else(|| format!("awbc.content.{}", content.0))
    }

    pub(super) fn flow_identity_for_function(
        &self,
        function: AwbcFunctionId,
    ) -> Result<crate::plan::FlowRuntimeId, ProductStepError> {
        self.program
            .flow_identity(function)
            .cloned()
            .ok_or_else(|| {
                ProductStepError::Internal(format!(
                    "AWBC Flow function {} has no typed semantic binding",
                    function.0
                ))
            })
    }

    pub(super) fn task_plan_for_id(&self, task: &str) -> Option<AwbcTaskPlanId> {
        self.program
            .task_plans
            .iter()
            .enumerate()
            .find_map(|(index, plan)| {
                self.program
                    .strings
                    .get(plan.public_id.index())
                    .filter(|public_id| public_id.as_str() == task)
                    .and_then(|_| u32::try_from(index).ok())
                    .map(AwbcTaskPlanId)
            })
    }

    pub(super) fn task_need_id(&self, plan: AwbcTaskPlanId) -> NeedId {
        NeedId(
            self.program
                .task_plans
                .get(plan.index())
                .and_then(|plan| self.program.strings.get(plan.need_id.index()))
                .cloned()
                .unwrap_or_else(|| format!("awbc.need.{}", plan.0)),
        )
    }
}
