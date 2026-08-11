use crate::{
    awbc::schema::AwbcRegisterId,
    runtime_id::{
        ExecutionInstanceId, RuntimeCaptureSlotId, RuntimeChildInstanceId, RuntimeChildPacketId,
        RuntimeCleanupScopeId, RuntimeCleanupSlotId, RuntimeClosureInstanceId,
        RuntimeFiberInstanceId, RuntimeFrameInstanceId, RuntimeFrameLocalId, RuntimeLocalSlotId,
        RuntimeMailboxInstanceId, RuntimeMailboxLaneId, RuntimeTransferInstanceId,
        RuntimeTransferPacketId,
    },
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{cmp::Ordering, fmt};

/// Canonical diagnostic identity for every runtime value-storage domain.
///
/// This enum is evidence only. Storage remains in each owning runtime domain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeOwnedSlotId {
    EnvironmentLocal {
        execution: ExecutionInstanceId,
        local: RuntimeLocalSlotId,
    },
    ClosureCapture {
        execution: ExecutionInstanceId,
        closure: RuntimeClosureInstanceId,
        capture: RuntimeCaptureSlotId,
    },
    AwbcRegister {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        register: AwbcRegisterId,
    },
    AwbcFrameLocal {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        local: RuntimeFrameLocalId,
    },
    MailboxLane {
        execution: ExecutionInstanceId,
        mailbox: RuntimeMailboxInstanceId,
        lane: RuntimeMailboxLaneId,
    },
    ChildPacket {
        execution: ExecutionInstanceId,
        child: RuntimeChildInstanceId,
        packet: RuntimeChildPacketId,
    },
    TransferPacket {
        execution: ExecutionInstanceId,
        transfer: RuntimeTransferInstanceId,
        packet: RuntimeTransferPacketId,
    },
    CleanupSlot {
        execution: ExecutionInstanceId,
        scope: RuntimeCleanupScopeId,
        slot: RuntimeCleanupSlotId,
    },
}

impl RuntimeOwnedSlotId {
    #[must_use]
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::EnvironmentLocal { .. } => 0,
            Self::ClosureCapture { .. } => 1,
            Self::AwbcRegister { .. } => 2,
            Self::AwbcFrameLocal { .. } => 3,
            Self::MailboxLane { .. } => 4,
            Self::ChildPacket { .. } => 5,
            Self::TransferPacket { .. } => 6,
            Self::CleanupSlot { .. } => 7,
        }
    }

    #[must_use]
    pub const fn execution(self) -> ExecutionInstanceId {
        match self {
            Self::EnvironmentLocal { execution, .. }
            | Self::ClosureCapture { execution, .. }
            | Self::AwbcRegister { execution, .. }
            | Self::AwbcFrameLocal { execution, .. }
            | Self::MailboxLane { execution, .. }
            | Self::ChildPacket { execution, .. }
            | Self::TransferPacket { execution, .. }
            | Self::CleanupSlot { execution, .. } => execution,
        }
    }

    #[must_use]
    pub fn render_canonical(self) -> String {
        let execution = self.execution().get().get();
        match self {
            Self::EnvironmentLocal { local, .. } => {
                format!("exec/{execution}/env/{}", local.get())
            }
            Self::ClosureCapture {
                closure, capture, ..
            } => format!(
                "exec/{execution}/closure/{}/capture/{}",
                closure.get(),
                capture.get()
            ),
            Self::AwbcRegister {
                fiber,
                frame,
                register,
                ..
            } => format!(
                "exec/{execution}/awbc/fiber/{}/frame/{}/register/{}",
                fiber.get(),
                frame.get(),
                register.0
            ),
            Self::AwbcFrameLocal {
                fiber,
                frame,
                local,
                ..
            } => format!(
                "exec/{execution}/awbc/fiber/{}/frame/{}/local/{}",
                fiber.get(),
                frame.get(),
                local.get()
            ),
            Self::MailboxLane { mailbox, lane, .. } => format!(
                "exec/{execution}/mailbox/{}/lane/{}",
                mailbox.get(),
                lane.get()
            ),
            Self::ChildPacket { child, packet, .. } => format!(
                "exec/{execution}/child/{}/packet/{}",
                child.get(),
                packet.get()
            ),
            Self::TransferPacket {
                transfer, packet, ..
            } => format!(
                "exec/{execution}/transfer/{}/packet/{}",
                transfer.get(),
                packet.get()
            ),
            Self::CleanupSlot { scope, slot, .. } => format!(
                "exec/{execution}/cleanup/{}/slot/{}",
                scope.get(),
                slot.get()
            ),
        }
    }
}

impl fmt::Display for RuntimeOwnedSlotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render_canonical())
    }
}

impl Ord for RuntimeOwnedSlotId {
    fn cmp(&self, other: &Self) -> Ordering {
        let tag_order = self.canonical_tag().cmp(&other.canonical_tag());
        if tag_order != Ordering::Equal {
            return tag_order;
        }
        match self.canonical_tag() {
            0 => cmp_environment_local(*self, *other),
            1 => cmp_closure_capture(*self, *other),
            2 => cmp_awbc_register(*self, *other),
            3 => cmp_awbc_frame_local(*self, *other),
            4 => cmp_mailbox_lane(*self, *other),
            5 => cmp_child_packet(*self, *other),
            6 => cmp_transfer_packet(*self, *other),
            7 => cmp_cleanup_slot(*self, *other),
            _ => unreachable!("canonical owned-slot tags are exhaustive"),
        }
    }
}

fn cmp_environment_local(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::EnvironmentLocal {
        execution: left_execution,
        local: left_local,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::EnvironmentLocal {
        execution: right_execution,
        local: right_local,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_local).cmp(&(right_execution, right_local))
}

fn cmp_closure_capture(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::ClosureCapture {
        execution: left_execution,
        closure: left_closure,
        capture: left_capture,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::ClosureCapture {
        execution: right_execution,
        closure: right_closure,
        capture: right_capture,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_closure, left_capture).cmp(&(
        right_execution,
        right_closure,
        right_capture,
    ))
}

fn cmp_awbc_register(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::AwbcRegister {
        execution: left_execution,
        fiber: left_fiber,
        frame: left_frame,
        register: left_register,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::AwbcRegister {
        execution: right_execution,
        fiber: right_fiber,
        frame: right_frame,
        register: right_register,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_fiber, left_frame, left_register).cmp(&(
        right_execution,
        right_fiber,
        right_frame,
        right_register,
    ))
}

fn cmp_awbc_frame_local(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::AwbcFrameLocal {
        execution: left_execution,
        fiber: left_fiber,
        frame: left_frame,
        local: left_local,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::AwbcFrameLocal {
        execution: right_execution,
        fiber: right_fiber,
        frame: right_frame,
        local: right_local,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_fiber, left_frame, left_local).cmp(&(
        right_execution,
        right_fiber,
        right_frame,
        right_local,
    ))
}

fn cmp_mailbox_lane(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::MailboxLane {
        execution: left_execution,
        mailbox: left_mailbox,
        lane: left_lane,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::MailboxLane {
        execution: right_execution,
        mailbox: right_mailbox,
        lane: right_lane,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_mailbox, left_lane).cmp(&(right_execution, right_mailbox, right_lane))
}

fn cmp_child_packet(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::ChildPacket {
        execution: left_execution,
        child: left_child,
        packet: left_packet,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::ChildPacket {
        execution: right_execution,
        child: right_child,
        packet: right_packet,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_child, left_packet).cmp(&(right_execution, right_child, right_packet))
}

fn cmp_transfer_packet(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::TransferPacket {
        execution: left_execution,
        transfer: left_transfer,
        packet: left_packet,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::TransferPacket {
        execution: right_execution,
        transfer: right_transfer,
        packet: right_packet,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_transfer, left_packet).cmp(&(
        right_execution,
        right_transfer,
        right_packet,
    ))
}

fn cmp_cleanup_slot(left: RuntimeOwnedSlotId, right: RuntimeOwnedSlotId) -> Ordering {
    let RuntimeOwnedSlotId::CleanupSlot {
        execution: left_execution,
        scope: left_scope,
        slot: left_slot,
    } = left
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    let RuntimeOwnedSlotId::CleanupSlot {
        execution: right_execution,
        scope: right_scope,
        slot: right_slot,
    } = right
    else {
        unreachable!("equal canonical tags select the same variant")
    };
    (left_execution, left_scope, left_slot).cmp(&(right_execution, right_scope, right_slot))
}

impl PartialOrd for RuntimeOwnedSlotId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HumanOwnedSlot {
    EnvironmentLocal {
        execution: ExecutionInstanceId,
        local: RuntimeLocalSlotId,
    },
    ClosureCapture {
        execution: ExecutionInstanceId,
        closure: RuntimeClosureInstanceId,
        capture: RuntimeCaptureSlotId,
    },
    AwbcRegister {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        register: AwbcRegisterId,
    },
    AwbcFrameLocal {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        local: RuntimeFrameLocalId,
    },
    MailboxLane {
        execution: ExecutionInstanceId,
        mailbox: RuntimeMailboxInstanceId,
        lane: RuntimeMailboxLaneId,
    },
    ChildPacket {
        execution: ExecutionInstanceId,
        child: RuntimeChildInstanceId,
        packet: RuntimeChildPacketId,
    },
    TransferPacket {
        execution: ExecutionInstanceId,
        transfer: RuntimeTransferInstanceId,
        packet: RuntimeTransferPacketId,
    },
    CleanupSlot {
        execution: ExecutionInstanceId,
        scope: RuntimeCleanupScopeId,
        slot: RuntimeCleanupSlotId,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum HumanOwnedSlotInput {
    EnvironmentLocal {
        execution: ExecutionInstanceId,
        local: RuntimeLocalSlotId,
    },
    ClosureCapture {
        execution: ExecutionInstanceId,
        closure: RuntimeClosureInstanceId,
        capture: RuntimeCaptureSlotId,
    },
    AwbcRegister {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        register: AwbcRegisterId,
    },
    AwbcFrameLocal {
        execution: ExecutionInstanceId,
        fiber: RuntimeFiberInstanceId,
        frame: RuntimeFrameInstanceId,
        local: RuntimeFrameLocalId,
    },
    MailboxLane {
        execution: ExecutionInstanceId,
        mailbox: RuntimeMailboxInstanceId,
        lane: RuntimeMailboxLaneId,
    },
    ChildPacket {
        execution: ExecutionInstanceId,
        child: RuntimeChildInstanceId,
        packet: RuntimeChildPacketId,
    },
    TransferPacket {
        execution: ExecutionInstanceId,
        transfer: RuntimeTransferInstanceId,
        packet: RuntimeTransferPacketId,
    },
    CleanupSlot {
        execution: ExecutionInstanceId,
        scope: RuntimeCleanupScopeId,
        slot: RuntimeCleanupSlotId,
    },
}

impl From<RuntimeOwnedSlotId> for HumanOwnedSlot {
    fn from(slot: RuntimeOwnedSlotId) -> Self {
        match slot {
            RuntimeOwnedSlotId::EnvironmentLocal { execution, local } => {
                Self::EnvironmentLocal { execution, local }
            }
            RuntimeOwnedSlotId::ClosureCapture {
                execution,
                closure,
                capture,
            } => Self::ClosureCapture {
                execution,
                closure,
                capture,
            },
            RuntimeOwnedSlotId::AwbcRegister {
                execution,
                fiber,
                frame,
                register,
            } => Self::AwbcRegister {
                execution,
                fiber,
                frame,
                register,
            },
            RuntimeOwnedSlotId::AwbcFrameLocal {
                execution,
                fiber,
                frame,
                local,
            } => Self::AwbcFrameLocal {
                execution,
                fiber,
                frame,
                local,
            },
            RuntimeOwnedSlotId::MailboxLane {
                execution,
                mailbox,
                lane,
            } => Self::MailboxLane {
                execution,
                mailbox,
                lane,
            },
            RuntimeOwnedSlotId::ChildPacket {
                execution,
                child,
                packet,
            } => Self::ChildPacket {
                execution,
                child,
                packet,
            },
            RuntimeOwnedSlotId::TransferPacket {
                execution,
                transfer,
                packet,
            } => Self::TransferPacket {
                execution,
                transfer,
                packet,
            },
            RuntimeOwnedSlotId::CleanupSlot {
                execution,
                scope,
                slot,
            } => Self::CleanupSlot {
                execution,
                scope,
                slot,
            },
        }
    }
}

impl From<HumanOwnedSlotInput> for RuntimeOwnedSlotId {
    fn from(slot: HumanOwnedSlotInput) -> Self {
        match slot {
            HumanOwnedSlotInput::EnvironmentLocal { execution, local } => {
                Self::EnvironmentLocal { execution, local }
            }
            HumanOwnedSlotInput::ClosureCapture {
                execution,
                closure,
                capture,
            } => Self::ClosureCapture {
                execution,
                closure,
                capture,
            },
            HumanOwnedSlotInput::AwbcRegister {
                execution,
                fiber,
                frame,
                register,
            } => Self::AwbcRegister {
                execution,
                fiber,
                frame,
                register,
            },
            HumanOwnedSlotInput::AwbcFrameLocal {
                execution,
                fiber,
                frame,
                local,
            } => Self::AwbcFrameLocal {
                execution,
                fiber,
                frame,
                local,
            },
            HumanOwnedSlotInput::MailboxLane {
                execution,
                mailbox,
                lane,
            } => Self::MailboxLane {
                execution,
                mailbox,
                lane,
            },
            HumanOwnedSlotInput::ChildPacket {
                execution,
                child,
                packet,
            } => Self::ChildPacket {
                execution,
                child,
                packet,
            },
            HumanOwnedSlotInput::TransferPacket {
                execution,
                transfer,
                packet,
            } => Self::TransferPacket {
                execution,
                transfer,
                packet,
            },
            HumanOwnedSlotInput::CleanupSlot {
                execution,
                scope,
                slot,
            } => Self::CleanupSlot {
                execution,
                scope,
                slot,
            },
        }
    }
}

impl Serialize for RuntimeOwnedSlotId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        HumanOwnedSlot::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuntimeOwnedSlotId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        HumanOwnedSlotInput::deserialize(deserializer).map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_json<T: for<'de> Deserialize<'de>>(json: &str) -> T {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn owned_slot_json_and_rendering_are_canonical() {
        let slot: RuntimeOwnedSlotId =
            from_json(r#"{"kind":"environment_local","execution":"1","local":"2"}"#);
        assert_eq!(slot.canonical_tag(), 0);
        assert_eq!(slot.render_canonical(), "exec/1/env/2");
        assert_eq!(
            serde_json::to_string(&slot).unwrap(),
            r#"{"kind":"environment_local","execution":"1","local":"2"}"#
        );
        assert!(
            serde_json::from_str::<RuntimeOwnedSlotId>(r#"{"kind":"other","execution":"1"}"#)
                .is_err()
        );
        for invalid in [
            r#"{"kind":"environment_local","execution":"1"}"#,
            r#"{"kind":"environment_local","execution":"01","local":"2"}"#,
            r#"{"kind":"environment_local","execution":"1","local":"0"}"#,
            r#"{"kind":"environment_local","execution":"1","local":"2","extra":true}"#,
            r#"{"kind":"environment_local","kind":"environment_local","execution":"1","local":"2"}"#,
        ] {
            assert!(serde_json::from_str::<RuntimeOwnedSlotId>(invalid).is_err());
        }
    }

    #[test]
    fn owned_slot_order_uses_exact_domain_tags() {
        let slots = [
            from_json(r#"{"kind":"environment_local","execution":"1","local":"2"}"#),
            from_json(r#"{"kind":"closure_capture","execution":"1","closure":"2","capture":3}"#),
            from_json(
                r#"{"kind":"awbc_register","execution":"1","fiber":"2","frame":"3","register":4}"#,
            ),
            from_json(
                r#"{"kind":"awbc_frame_local","execution":"1","fiber":"2","frame":"3","local":4}"#,
            ),
            from_json(r#"{"kind":"mailbox_lane","execution":"1","mailbox":"2","lane":3}"#),
            from_json(r#"{"kind":"child_packet","execution":"1","child":"2","packet":3}"#),
            from_json(r#"{"kind":"transfer_packet","execution":"1","transfer":"2","packet":3}"#),
            from_json(r#"{"kind":"cleanup_slot","execution":"1","scope":"2","slot":3}"#),
        ];
        assert!(slots.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            slots.map(RuntimeOwnedSlotId::canonical_tag),
            [0, 1, 2, 3, 4, 5, 6, 7]
        );
    }
}
