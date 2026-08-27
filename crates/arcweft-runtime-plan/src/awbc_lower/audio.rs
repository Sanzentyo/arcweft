use crate::awbc_lower::expr::AwbcExprLowerer;
use crate::awbc_lower::frame::FrameBuilder;
use crate::awbc_lower::inventory::{AwbcInventory, AwbcLowerDiagnostic};
use crate::awbc_lower::table_index;
use arcweft_core::audio::RuntimeAudioCommand;
use arcweft_core::awbc::schema::{
    AwbcAudioArg, AwbcAudioCommand, AwbcAudioValueRef, AwbcRegisterId,
};
use arcweft_core::plan::RuntimePlan;
use arcweft_core::value::{RuntimeExpr, RuntimeExprKind, RuntimeValue};

/// Lowers typed audio commands into compact-VM evaluated payload references.
pub(crate) struct AwbcAudioLowerer<'a, 'b, 'plan> {
    inventory: &'a mut AwbcInventory,
    frame: &'b mut FrameBuilder,
    plan: &'plan RuntimePlan,
    path: String,
    args: Vec<AwbcRegisterId>,
}

impl<'a, 'b, 'plan> AwbcAudioLowerer<'a, 'b, 'plan> {
    pub(crate) fn new(
        inventory: &'a mut AwbcInventory,
        frame: &'b mut FrameBuilder,
        path: impl Into<String>,
        plan: &'plan RuntimePlan,
    ) -> Self {
        Self {
            inventory,
            frame,
            plan,
            path: path.into(),
            args: Vec::new(),
        }
    }

    pub(crate) fn lower(
        mut self,
        command: &RuntimeAudioCommand,
    ) -> (AwbcAudioCommand, Vec<AwbcRegisterId>) {
        let command = self.lower_command(command);
        (command, self.args)
    }

    fn arg(&mut self, expr: &RuntimeExpr) -> AwbcAudioValueRef {
        let register =
            AwbcExprLowerer::new(self.inventory, self.frame, self.path.clone(), self.plan)
                .lower(expr);
        let arg = AwbcAudioArg::new(table_index(self.args.len()));
        self.args.push(register);
        AwbcAudioValueRef::Arg(arg)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Audio lowering mirrors each stable command variant and expression field."
    )]
    fn lower_command(&mut self, command: &RuntimeAudioCommand) -> AwbcAudioCommand {
        match command {
            RuntimeAudioCommand::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                loop_mode,
                start_frame,
                fade_in_millis,
            } => AwbcAudioCommand::Play {
                voice: self.arg(voice),
                resource: self.arg(resource),
                bus: self.arg(bus),
                gain_db_milli: self.arg(gain_db_milli),
                pan_milli: self.arg(pan_milli),
                loop_mode: *loop_mode,
                start_frame: self.arg(start_frame),
                fade_in_millis: self.arg(fade_in_millis),
            },
            RuntimeAudioCommand::Stop {
                voice,
                fade_out_millis,
            } => AwbcAudioCommand::Stop {
                voice: self.arg(voice),
                fade_out_millis: self.arg(fade_out_millis),
            },
            RuntimeAudioCommand::StopAll { fade_out_millis } => AwbcAudioCommand::StopAll {
                fade_out_millis: self.arg(fade_out_millis),
            },
            RuntimeAudioCommand::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => AwbcAudioCommand::SetVoiceGain {
                voice: self.arg(voice),
                gain_db_milli: self.arg(gain_db_milli),
                transition_millis: self.arg(transition_millis),
            },
            RuntimeAudioCommand::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => AwbcAudioCommand::SetVoicePan {
                voice: self.arg(voice),
                pan_milli: self.arg(pan_milli),
                transition_millis: self.arg(transition_millis),
            },
            RuntimeAudioCommand::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => AwbcAudioCommand::SetBusGain {
                bus: self.arg(bus),
                gain_db_milli: self.arg(gain_db_milli),
                transition_millis: self.arg(transition_millis),
            },
            RuntimeAudioCommand::SetBusMute { bus, muted } => AwbcAudioCommand::SetBusMute {
                bus: self.arg(bus),
                muted: self.arg(muted),
            },
            RuntimeAudioCommand::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => AwbcAudioCommand::SetEffectEnabled {
                bus: self.arg(bus),
                effect: self.arg(effect),
                enabled: self.arg(enabled),
            },
            RuntimeAudioCommand::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => AwbcAudioCommand::SetEffectParameter {
                bus: self.arg(bus),
                effect: self.arg(effect),
                parameter: *parameter,
                value: self.arg(value),
                transition_millis: self.arg(transition_millis),
            },
            RuntimeAudioCommand::ApplySnapshot {
                snapshot,
                transition_millis,
            } => AwbcAudioCommand::ApplySnapshot {
                snapshot: self.arg(snapshot),
                transition_millis: self.arg(transition_millis),
            },
            RuntimeAudioCommand::RequestMicrophone {
                capture,
                constraints,
            } => AwbcAudioCommand::RequestMicrophone {
                capture: self.arg(capture),
                constraints: *constraints,
            },
            RuntimeAudioCommand::StopMicrophone { capture } => AwbcAudioCommand::StopMicrophone {
                capture: self.arg(capture),
            },
            RuntimeAudioCommand::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => AwbcAudioCommand::SetCaptureMonitor {
                capture: self.arg(capture),
                bus: bus.as_ref().map(|bus| self.arg(bus)),
                gain_db_milli: self.arg(gain_db_milli),
            },
        }
    }
}

pub(crate) fn constant_audio_command(
    inventory: &mut AwbcInventory,
    command: &RuntimeAudioCommand,
    path: impl Into<String>,
) -> AwbcAudioCommand {
    ConstantAudioLowerer {
        inventory,
        path: path.into(),
    }
    .lower(command)
}

struct ConstantAudioLowerer<'a> {
    inventory: &'a mut AwbcInventory,
    path: String,
}

impl ConstantAudioLowerer<'_> {
    fn expr(&mut self, expr: &RuntimeExpr) -> AwbcAudioValueRef {
        let value = match expr.kind() {
            RuntimeExprKind::Value(value) => value.clone(),
            RuntimeExprKind::EntityRef(value) => RuntimeValue::EntityRef(value.clone()),
            _ => {
                self.inventory.diagnostic(AwbcLowerDiagnostic::error(
                    self.path.clone(),
                    format!(
                        "line-task audio expression `{expr}` requires a flow frame and cannot be lowered as a constant AWBC audio payload"
                    ),
                ));
                RuntimeValue::Unit
            }
        };
        AwbcAudioValueRef::Const(self.inventory.constant_runtime_value(&value))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Constant audio lowering mirrors each stable command variant and literal field."
    )]
    fn lower(&mut self, command: &RuntimeAudioCommand) -> AwbcAudioCommand {
        match command {
            RuntimeAudioCommand::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                loop_mode,
                start_frame,
                fade_in_millis,
            } => AwbcAudioCommand::Play {
                voice: self.expr(voice),
                resource: self.expr(resource),
                bus: self.expr(bus),
                gain_db_milli: self.expr(gain_db_milli),
                pan_milli: self.expr(pan_milli),
                loop_mode: *loop_mode,
                start_frame: self.expr(start_frame),
                fade_in_millis: self.expr(fade_in_millis),
            },
            RuntimeAudioCommand::Stop {
                voice,
                fade_out_millis,
            } => AwbcAudioCommand::Stop {
                voice: self.expr(voice),
                fade_out_millis: self.expr(fade_out_millis),
            },
            RuntimeAudioCommand::StopAll { fade_out_millis } => AwbcAudioCommand::StopAll {
                fade_out_millis: self.expr(fade_out_millis),
            },
            RuntimeAudioCommand::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => AwbcAudioCommand::SetVoiceGain {
                voice: self.expr(voice),
                gain_db_milli: self.expr(gain_db_milli),
                transition_millis: self.expr(transition_millis),
            },
            RuntimeAudioCommand::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => AwbcAudioCommand::SetVoicePan {
                voice: self.expr(voice),
                pan_milli: self.expr(pan_milli),
                transition_millis: self.expr(transition_millis),
            },
            RuntimeAudioCommand::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => AwbcAudioCommand::SetBusGain {
                bus: self.expr(bus),
                gain_db_milli: self.expr(gain_db_milli),
                transition_millis: self.expr(transition_millis),
            },
            RuntimeAudioCommand::SetBusMute { bus, muted } => AwbcAudioCommand::SetBusMute {
                bus: self.expr(bus),
                muted: self.expr(muted),
            },
            RuntimeAudioCommand::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => AwbcAudioCommand::SetEffectEnabled {
                bus: self.expr(bus),
                effect: self.expr(effect),
                enabled: self.expr(enabled),
            },
            RuntimeAudioCommand::SetEffectParameter {
                bus,
                effect,
                parameter,
                value,
                transition_millis,
            } => AwbcAudioCommand::SetEffectParameter {
                bus: self.expr(bus),
                effect: self.expr(effect),
                parameter: *parameter,
                value: self.expr(value),
                transition_millis: self.expr(transition_millis),
            },
            RuntimeAudioCommand::ApplySnapshot {
                snapshot,
                transition_millis,
            } => AwbcAudioCommand::ApplySnapshot {
                snapshot: self.expr(snapshot),
                transition_millis: self.expr(transition_millis),
            },
            RuntimeAudioCommand::RequestMicrophone {
                capture,
                constraints,
            } => AwbcAudioCommand::RequestMicrophone {
                capture: self.expr(capture),
                constraints: *constraints,
            },
            RuntimeAudioCommand::StopMicrophone { capture } => AwbcAudioCommand::StopMicrophone {
                capture: self.expr(capture),
            },
            RuntimeAudioCommand::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => AwbcAudioCommand::SetCaptureMonitor {
                capture: self.expr(capture),
                bus: bus.as_ref().map(|bus| self.expr(bus)),
                gain_db_milli: self.expr(gain_db_milli),
            },
        }
    }
}
