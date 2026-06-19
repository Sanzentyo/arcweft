use arcweft_core::task::{HostTaskRequest, TaskSpec};
use arcweft_core::value::{RuntimePayload, RuntimeValue};
use arcweft_desktop_contract::{DesktopError, DesktopRequest, DesktopResponse};
use arcweft_host_adapter::{HostTaskMetrics, HostTaskOutcome};

pub(crate) fn decode_request(task: &TaskSpec) -> Result<DesktopRequest, String> {
    let HostTaskRequest::Custom { args, .. } = &task.request else {
        return Err("desktop adapter expected a custom host request".to_owned());
    };
    if args.is_empty() && task.request.host_call_id() == "desktop.platform.capabilities" {
        return Ok(DesktopRequest::Capabilities);
    }
    let [RuntimePayload(RuntimeValue::String(json))] = args.as_slice() else {
        return Err("desktop host call expects exactly one JSON string argument".to_owned());
    };
    serde_json::from_str(json).map_err(|error| format!("invalid desktop request JSON: {error}"))
}

pub(crate) fn outcome(
    request: &DesktopRequest,
    result: Result<DesktopResponse, DesktopError>,
) -> HostTaskOutcome {
    match result {
        Ok(response) => {
            let metrics = metrics(request, &response);
            let result = serde_json::to_string(&response)
                .map(RuntimePayload::from)
                .map_err(|error| format!("failed to encode desktop response: {error}"));
            HostTaskOutcome { result, metrics }
        }
        Err(error) => HostTaskOutcome {
            result: Err(serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())),
            metrics: HostTaskMetrics::default(),
        },
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
