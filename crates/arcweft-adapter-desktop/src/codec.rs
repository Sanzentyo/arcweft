#[cfg(test)]
use arcweft_core::value::RuntimePayload;
use arcweft_core::{
    pattern::{RuntimeCheckedType, RuntimeVariantIdentity},
    task::{HostTaskRequest, TaskOutcomeContract, TaskSpec},
    value::RuntimeValue,
};
use arcweft_desktop_contract::{
    CursorIcon, DesktopError, DesktopRequest, DesktopResponse, OwnedCursorRequest,
    OwnedWindowRequest, PhysicalPosition, PhysicalRect, PhysicalSize, WindowMode, WindowTarget,
};
use arcweft_host_adapter::{
    HostCallArgs, HostCallVariantArg, HostTaskCompletion, HostTaskMetrics, HostTaskOutcome,
};

use crate::{
    DESKTOP_CAPABILITIES_CALL, DESKTOP_CURSOR_ICON_TYPE, DESKTOP_OWNED_CURSOR_SET_ICON_CALL,
    DESKTOP_OWNED_CURSOR_SET_POSITION_CALL, DESKTOP_OWNED_CURSOR_SET_VISIBLE_CALL,
    DESKTOP_OWNED_WINDOW_REQUEST_CLOSE_CALL, DESKTOP_OWNED_WINDOW_REQUEST_FOCUS_CALL,
    DESKTOP_OWNED_WINDOW_SET_BOUNDS_CALL, DESKTOP_OWNED_WINDOW_SET_MODE_CALL,
    DESKTOP_OWNED_WINDOW_SET_TITLE_CALL, DESKTOP_WINDOW_MODE_TYPE,
};

pub(crate) fn decode_request(task: &TaskSpec) -> Result<DesktopRequest, String> {
    let HostTaskRequest::Custom { .. } = &task.request else {
        return Err("desktop adapter expected a custom host request".to_owned());
    };
    let args = HostCallArgs::from_custom_request(&task.request)
        .expect("custom host task request has typed arguments");
    if task.request.host_call_id() == DESKTOP_CAPABILITIES_CALL {
        args.expect_len(0)?;
        return Ok(DesktopRequest::Capabilities);
    }
    if let Some(request) = decode_typed_owned_request(task, &args)? {
        return Ok(request);
    }
    Err(format!(
        "desktop host call `{}` has no typed decoder",
        task.request.host_call_id()
    ))
}

fn decode_typed_owned_request(
    task: &TaskSpec,
    args: &HostCallArgs<'_>,
) -> Result<Option<DesktopRequest>, String> {
    let call = task.request.host_call_id();
    let target = WindowTarget::PrimaryOwned;
    let request = match call.as_str() {
        DESKTOP_OWNED_WINDOW_SET_TITLE_CALL => {
            args.expect_len(1)?;
            DesktopRequest::OwnedWindow(OwnedWindowRequest::SetTitle {
                target,
                title: args.string(0)?,
            })
        }
        DESKTOP_OWNED_WINDOW_SET_BOUNDS_CALL => {
            args.expect_len(4)?;
            DesktopRequest::OwnedWindow(OwnedWindowRequest::SetBounds {
                target,
                bounds: PhysicalRect {
                    position: PhysicalPosition {
                        x: args.i32(0)?,
                        y: args.i32(1)?,
                    },
                    size: PhysicalSize {
                        width: args.u32(2)?,
                        height: args.u32(3)?,
                    },
                },
            })
        }
        DESKTOP_OWNED_WINDOW_SET_MODE_CALL => {
            args.expect_len(1)?;
            DesktopRequest::OwnedWindow(OwnedWindowRequest::SetMode {
                target,
                mode: window_mode(args.variant(0)?)?,
            })
        }
        DESKTOP_OWNED_WINDOW_REQUEST_FOCUS_CALL => {
            args.expect_len(0)?;
            DesktopRequest::OwnedWindow(OwnedWindowRequest::RequestFocus { target })
        }
        DESKTOP_OWNED_WINDOW_REQUEST_CLOSE_CALL => {
            args.expect_len(0)?;
            DesktopRequest::OwnedWindow(OwnedWindowRequest::RequestClose { target })
        }
        DESKTOP_OWNED_CURSOR_SET_ICON_CALL => {
            args.expect_len(1)?;
            DesktopRequest::OwnedCursor(OwnedCursorRequest::SetIcon {
                target,
                icon: cursor_icon(args.variant(0)?)?,
            })
        }
        DESKTOP_OWNED_CURSOR_SET_VISIBLE_CALL => {
            args.expect_len(1)?;
            DesktopRequest::OwnedCursor(OwnedCursorRequest::SetVisible {
                target,
                visible: args.bool(0)?,
            })
        }
        DESKTOP_OWNED_CURSOR_SET_POSITION_CALL => {
            args.expect_len(2)?;
            DesktopRequest::OwnedCursor(OwnedCursorRequest::SetPosition {
                target,
                position: PhysicalPosition {
                    x: args.i32(0)?,
                    y: args.i32(1)?,
                },
            })
        }
        _ => return Ok(None),
    };
    Ok(Some(request))
}

fn window_mode(value: HostCallVariantArg<'_>) -> Result<WindowMode, String> {
    expect_unit_variant(DESKTOP_WINDOW_MODE_TYPE, value)?;
    match (value.ordinal, value.name) {
        (0, "Normal") => Ok(WindowMode::Normal),
        (1, "Minimized") => Ok(WindowMode::Minimized),
        (2, "Maximized") => Ok(WindowMode::Maximized),
        (3, "BorderlessFullscreen") => Ok(WindowMode::BorderlessFullscreen),
        (4, "Fullscreen") => Ok(WindowMode::Fullscreen),
        (ordinal, name) => Err(format!(
            "unknown owned window mode case #{ordinal} `{name}`"
        )),
    }
}

fn cursor_icon(value: HostCallVariantArg<'_>) -> Result<CursorIcon, String> {
    expect_unit_variant(DESKTOP_CURSOR_ICON_TYPE, value)?;
    match (value.ordinal, value.name) {
        (0, "Default") => Ok(CursorIcon::Default),
        (1, "Pointer") => Ok(CursorIcon::Pointer),
        (2, "Text") => Ok(CursorIcon::Text),
        (3, "Crosshair") => Ok(CursorIcon::Crosshair),
        (4, "Move") => Ok(CursorIcon::Move),
        (5, "NotAllowed") => Ok(CursorIcon::NotAllowed),
        (6, "Wait") => Ok(CursorIcon::Wait),
        (7, "Progress") => Ok(CursorIcon::Progress),
        (8, "Help") => Ok(CursorIcon::Help),
        (9, "ZoomIn") => Ok(CursorIcon::ZoomIn),
        (10, "ZoomOut") => Ok(CursorIcon::ZoomOut),
        (11, "Grab") => Ok(CursorIcon::Grab),
        (12, "Grabbing") => Ok(CursorIcon::Grabbing),
        (13, "ResizeHorizontal") => Ok(CursorIcon::ResizeHorizontal),
        (14, "ResizeVertical") => Ok(CursorIcon::ResizeVertical),
        (15, "ResizeDiagonalNorthEastSouthWest") => {
            Ok(CursorIcon::ResizeDiagonalNorthEastSouthWest)
        }
        (16, "ResizeDiagonalNorthWestSouthEast") => {
            Ok(CursorIcon::ResizeDiagonalNorthWestSouthEast)
        }
        (17, "Hidden") => Ok(CursorIcon::Hidden),
        (ordinal, name) => Err(format!(
            "unknown owned cursor icon case #{ordinal} `{name}`"
        )),
    }
}

fn expect_unit_variant(expected_type: &str, value: HostCallVariantArg<'_>) -> Result<(), String> {
    let RuntimeVariantIdentity::Nominal { nominal, .. } = value.owner else {
        return Err(format!(
            "host-call variant `{}` is not owned by nominal `{expected_type}`",
            value.name
        ));
    };
    if nominal.as_str() != expected_type {
        return Err(format!(
            "host-call variant `{}` belongs to `{}`, expected `{expected_type}`",
            value.name,
            nominal.as_str()
        ));
    }
    if value.payload.is_some() {
        return Err(format!(
            "host-call variant `{}.{}` must not carry a payload",
            expected_type, value.name
        ));
    }
    Ok(())
}

pub(crate) fn outcome(
    request: &DesktopRequest,
    contract: &TaskOutcomeContract,
    result: Result<DesktopResponse, DesktopError>,
) -> HostTaskOutcome {
    match result {
        Ok(response) => {
            let metrics = metrics(request, &response);
            let completion = serde_json::to_string(&response)
                .map(RuntimeValue::String)
                .map_err(|error| format!("failed to encode desktop response: {error}"))
                .and_then(|value| contract.try_result_ok(value))
                .map_or_else(HostTaskCompletion::Failed, HostTaskCompletion::Ready);
            HostTaskOutcome {
                completion,
                metrics,
            }
        }
        Err(error) => {
            let completion = (|| {
                let Some(RuntimeCheckedType::Opaque { owner }) = contract.result_error() else {
                    return Err("desktop task has no exact opaque domain-error contract".to_owned());
                };
                let encoded = serde_json::to_string(&error).unwrap_or_else(|_| error.to_string());
                let error = owner
                    .try_wrap(RuntimeValue::String(encoded))
                    .map_err(|error| error.to_string())?;
                contract.try_result_err(error)
            })()
            .map_or_else(HostTaskCompletion::Failed, HostTaskCompletion::Ready);
            HostTaskOutcome {
                completion,
                metrics: HostTaskMetrics::default(),
            }
        }
    }
}

fn metrics(request: &DesktopRequest, response: &DesktopResponse) -> HostTaskMetrics {
    use arcweft_desktop_contract::{UserFileRequest, UserFileResponse};

    match (request, response) {
        (
            DesktopRequest::UserFile(UserFileRequest::ReadText { .. }),
            DesktopResponse::UserFile(UserFileResponse::Text(text)),
        ) => HostTaskMetrics {
            read_ops: 1,
            bytes_read: text.len(),
            ..HostTaskMetrics::default()
        },
        (
            DesktopRequest::UserFile(UserFileRequest::ReadBytes { .. }),
            DesktopResponse::UserFile(UserFileResponse::Bytes(bytes)),
        ) => HostTaskMetrics {
            read_ops: 1,
            bytes_read: bytes.len(),
            ..HostTaskMetrics::default()
        },
        (DesktopRequest::UserFile(UserFileRequest::WriteText { text, .. }), _) => HostTaskMetrics {
            write_ops: 1,
            bytes_written: text.len(),
            ..HostTaskMetrics::default()
        },
        (DesktopRequest::UserFile(UserFileRequest::WriteBytes { bytes, .. }), _) => {
            HostTaskMetrics {
                write_ops: 1,
                bytes_written: bytes.len(),
                ..HostTaskMetrics::default()
            }
        }
        _ => HostTaskMetrics::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::task::{
        CancelScopeId, HostTaskRequest, TaskClass, TaskId, TaskKey, TaskPolicy, TaskPriority,
    };
    use arcweft_core::value::RuntimeValue;

    #[test]
    fn typed_owned_window_mode_decodes_from_variant_payload() {
        let request = decode_request(&task(
            "desktop.window.owned",
            "set_mode",
            [variant_arg(
                DESKTOP_WINDOW_MODE_TYPE,
                3,
                "BorderlessFullscreen",
            )],
        ))
        .expect("variant mode decodes");

        assert_eq!(
            request,
            DesktopRequest::OwnedWindow(OwnedWindowRequest::SetMode {
                target: WindowTarget::PrimaryOwned,
                mode: WindowMode::BorderlessFullscreen,
            })
        );
    }

    #[test]
    fn typed_owned_cursor_icon_rejects_string_payload() {
        let error = decode_request(&task(
            "desktop.cursor.owned",
            "set_icon",
            [RuntimePayload::from("pointer")],
        ))
        .expect_err("string icon payload is not a typed enum variant");

        assert!(error.contains("must be Variant"));
    }

    #[test]
    fn desktop_decoder_has_no_json_request_fallback() {
        let error = decode_request(&task(
            "desktop.files.user",
            "read",
            [RuntimePayload::from("{\"operation\":\"read_text\"}")],
        ))
        .expect_err("JSON request fallback is removed");

        assert!(error.contains("has no typed decoder"));
    }

    fn variant_arg(owner: &str, ordinal: u32, name: &str) -> RuntimePayload {
        RuntimePayload(RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Nominal {
                nominal: arcweft_core::entry::RuntimeNominalTypeId::try_new(owner)
                    .expect("test nominal identity"),
                semantic_identity: arcweft_core::pattern::RuntimeSemanticTypeId::from_bytes(
                    [7; 32],
                ),
            },
            ordinal,
            name: name.to_owned(),
            payload: None,
        })
    }

    fn task<const N: usize>(
        capability: &str,
        operation: &str,
        args: [RuntimePayload; N],
    ) -> TaskSpec {
        let id = format!("{capability}.{operation}");
        TaskSpec::new(
            TaskId(id.clone()),
            TaskKey(id),
            TaskClass::Background,
            TaskPriority(0),
            CancelScopeId("desktop-codec-test".to_owned()),
            TaskPolicy::JoinSameKey,
            HostTaskRequest::custom(capability, operation, args),
        )
    }
}
