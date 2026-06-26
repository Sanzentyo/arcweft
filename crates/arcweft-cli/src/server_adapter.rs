use arcweft_core::engine::{FlowExit, FlowFiberStatus};
use arcweft_core::executor::{RuntimeExecutor, VmExecutor};
use arcweft_core::plan::{RuntimePlan, RuntimeRouteBindingSource, RuntimeRouteSpec};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::value::{RuntimeBinding, RuntimeFieldValue, RuntimeValue};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureAcceleratorConfig};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeHttpServerConfig {
    pub(crate) listen: SocketAddr,
    pub(crate) once: bool,
    pub(crate) max_ops: usize,
    pub(crate) pure_config: RuntimePureAcceleratorConfig,
    pub(crate) host_policy: HostCallPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(crate) struct NativeHttpServerReport {
    pub(crate) listen: String,
    pub(crate) handled_requests: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeHttpResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum ServerAdapterError {
    #[error("native HTTP adapter failed to bind {addr}: {message}")]
    Bind { addr: SocketAddr, message: String },
    #[error("native HTTP adapter failed to accept a request: {0}")]
    Accept(String),
    #[error("native HTTP adapter failed to read a request: {0}")]
    Read(String),
    #[error("native HTTP adapter failed to write a response: {0}")]
    Write(String),
    #[error("native HTTP adapter requires host call `{0}` from the active adapter manifest")]
    MissingHostCall(String),
    #[error("invalid HTTP request")]
    InvalidRequest,
}

pub(crate) fn serve_native_http(
    plan: &RuntimePlan,
    routes: &[RuntimeRouteSpec],
    config: &NativeHttpServerConfig,
) -> Result<NativeHttpServerReport, ServerAdapterError> {
    let listener = TcpListener::bind(config.listen).map_err(|error| ServerAdapterError::Bind {
        addr: config.listen,
        message: error.to_string(),
    })?;
    let listen = listener
        .local_addr()
        .map_or_else(|_| config.listen.to_string(), |addr| addr.to_string());
    let mut handled_requests = 0;
    loop {
        let (stream, _) = listener
            .accept()
            .map_err(|error| ServerAdapterError::Accept(error.to_string()))?;
        serve_stream(
            stream,
            plan,
            routes,
            config.max_ops,
            config.pure_config,
            &config.host_policy,
        )?;
        handled_requests += 1;
        if config.once {
            break;
        }
    }
    Ok(NativeHttpServerReport {
        listen,
        handled_requests,
    })
}

fn serve_stream(
    mut stream: TcpStream,
    plan: &RuntimePlan,
    routes: &[RuntimeRouteSpec],
    max_ops: usize,
    pure_config: RuntimePureAcceleratorConfig,
    host_policy: &HostCallPolicy,
) -> Result<(), ServerAdapterError> {
    let mut buffer = vec![0_u8; 64 * 1024];
    let read = stream
        .read(&mut buffer)
        .map_err(|error| ServerAdapterError::Read(error.to_string()))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let response = handle_http_request(plan, routes, &request, max_ops, pure_config, host_policy)?;
    stream
        .write_all(http_response_bytes(&response).as_bytes())
        .map_err(|error| ServerAdapterError::Write(error.to_string()))
}

pub(crate) fn handle_http_request(
    plan: &RuntimePlan,
    routes: &[RuntimeRouteSpec],
    request: &str,
    max_ops: usize,
    pure_config: RuntimePureAcceleratorConfig,
    host_policy: &HostCallPolicy,
) -> Result<NativeHttpResponse, ServerAdapterError> {
    require_host_call(host_policy, "http.respond")?;
    let parsed = parse_http_request(request)?;
    let Some((route, params)) = routes
        .iter()
        .find_map(|route| route_match(route, &parsed).map(|params| (route, params)))
    else {
        return Ok(NativeHttpResponse {
            status: 404,
            body: "not found".to_owned(),
        });
    };
    Ok(run_route_flow(
        plan,
        route,
        &parsed,
        &params,
        max_ops,
        pure_config,
    ))
}

fn require_host_call(host_policy: &HostCallPolicy, id: &str) -> Result<(), ServerAdapterError> {
    if host_policy.contains(id) {
        Ok(())
    } else {
        Err(ServerAdapterError::MissingHostCall(id.to_owned()))
    }
}

fn run_route_flow(
    plan: &RuntimePlan,
    route: &RuntimeRouteSpec,
    request: &HttpRequestHead,
    params: &[(String, String)],
    max_ops: usize,
    pure_config: RuntimePureAcceleratorConfig,
) -> NativeHttpResponse {
    let mut plan = plan.clone();
    plan.entry_flow = Some(route.target.clone());
    let mut pure = RuntimePureAccelerator::with_config(pure_config, &plan.pure_helpers);
    let mut executor = VmExecutor::new(plan);
    let result = executor.step_with_pure_backend(
        RuntimeStepInput {
            bindings: request_bindings(request, route, params),
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions {
            mode: RuntimeStepMode::Server,
            budget: RuntimeStepBudget { max_ops },
        },
        &mut pure,
    );
    if let Some(diagnostic) = result.output.diagnostics.first() {
        return NativeHttpResponse {
            status: 500,
            body: diagnostic.message.clone(),
        };
    }
    match &executor.fiber().status {
        FlowFiberStatus::Done(FlowExit::Return(value)) => NativeHttpResponse {
            status: 200,
            body: value.clone(),
        },
        FlowFiberStatus::Done(FlowExit::Done) => NativeHttpResponse {
            status: 204,
            body: String::new(),
        },
        FlowFiberStatus::Failed(message) => NativeHttpResponse {
            status: 500,
            body: message.clone(),
        },
        FlowFiberStatus::Running
        | FlowFiberStatus::Dialogue(_)
        | FlowFiberStatus::Waiting(_)
        | FlowFiberStatus::WaitingMany(_)
        | FlowFiberStatus::HostCall(_)
        | FlowFiberStatus::Choice(_) => NativeHttpResponse {
            status: 202,
            body: "route did not complete in this server step".to_owned(),
        },
    }
}

fn request_bindings(
    request: &HttpRequestHead,
    route: &RuntimeRouteSpec,
    params: &[(String, String)],
) -> Vec<RuntimeBinding> {
    let route_param_bindings = route
        .bindings
        .iter()
        .filter_map(|binding| match &binding.source {
            RuntimeRouteBindingSource::PathParam(param) => params
                .iter()
                .find(|(name, _)| name == param)
                .map(|(_, value)| RuntimeBinding {
                    name: binding.name.clone(),
                    value: RuntimeValue::String(value.clone()),
                }),
        });
    std::iter::once(RuntimeBinding {
        name: "request".to_owned(),
        value: RuntimeValue::Record(vec![
            RuntimeFieldValue {
                name: "method".to_owned(),
                value: RuntimeValue::String(request.method.clone()),
            },
            RuntimeFieldValue {
                name: "path".to_owned(),
                value: RuntimeValue::String(request.path.clone()),
            },
            RuntimeFieldValue {
                name: "body".to_owned(),
                value: RuntimeValue::String(request.body.clone()),
            },
        ]),
    })
    .chain(route_param_bindings)
    .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HttpRequestHead {
    method: String,
    path: String,
    body: String,
}

fn parse_http_request(request: &str) -> Result<HttpRequestHead, ServerAdapterError> {
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((request, ""));
    let request_line = head
        .lines()
        .next()
        .ok_or(ServerAdapterError::InvalidRequest)?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or(ServerAdapterError::InvalidRequest)?
        .to_owned();
    let raw_path = parts
        .next()
        .ok_or(ServerAdapterError::InvalidRequest)?
        .to_owned();
    let path = raw_path
        .split_once('?')
        .map_or(raw_path.as_str(), |(path, _)| path)
        .to_owned();
    Ok(HttpRequestHead {
        method,
        path,
        body: body.to_owned(),
    })
}

fn route_match(
    route: &RuntimeRouteSpec,
    request: &HttpRequestHead,
) -> Option<Vec<(String, String)>> {
    let method_matches = route.method == "*" || route.method.eq_ignore_ascii_case(&request.method);
    if !method_matches {
        return None;
    }
    match_path(&route.path, &request.path)
}

fn match_path(pattern: &str, path: &str) -> Option<Vec<(String, String)>> {
    if pattern == "*" {
        return Some(Vec::new());
    }
    let pattern_segments = split_path(pattern);
    let path_segments = split_path(path);
    if pattern_segments.len() != path_segments.len() {
        return None;
    }
    pattern_segments.iter().zip(path_segments).try_fold(
        Vec::new(),
        |mut params, (pattern, value)| {
            if let Some(name) = pattern.strip_prefix(':') {
                params.push((name.to_owned(), value.to_owned()));
                Some(params)
            } else if pattern == &value {
                Some(params)
            } else {
                None
            }
        },
    )
}

fn split_path(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn http_response_bytes(response: &NativeHttpResponse) -> String {
    let status_text = match response.status {
        202 => "Accepted",
        204 => "No Content",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    format!(
        "HTTP/1.1 {} {}\r\ncontent-length: {}\r\ncontent-type: text/plain; charset=utf-8\r\nconnection: close\r\n\r\n{}",
        response.status,
        status_text,
        response.body.len(),
        response.body
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_adapter_context::standard;
    use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow};

    #[test]
    fn native_http_adapter_routes_request_to_flow() {
        let plan = plan_with_flow("flow.health", vec![FlowOp::Return("ok".to_owned())]);
        let routes = vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/health".to_owned(),
            target: FlowRuntimeId("flow.health".to_owned()),
            bindings: Vec::new(),
        }];

        let response = handle_http_request(
            &plan,
            &routes,
            "GET /health HTTP/1.1\r\nhost: localhost\r\n\r\n",
            8,
            RuntimePureAcceleratorConfig::default(),
            &native_http_host_calls(),
        )
        .expect("request is handled");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "ok");
    }

    #[test]
    fn native_http_adapter_binds_explicit_route_parameters() {
        let plan = plan_with_flow(
            "flow.hello",
            vec![FlowOp::ReturnExpr(arcweft_core::value::RuntimeExpr::Local(
                "name".to_owned(),
            ))],
        );
        let routes = vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/hello/:name".to_owned(),
            target: FlowRuntimeId("flow.hello".to_owned()),
            bindings: vec![arcweft_core::plan::RuntimeRouteBinding {
                name: "name".to_owned(),
                source: RuntimeRouteBindingSource::PathParam("name".to_owned()),
            }],
        }];

        let response = handle_http_request(
            &plan,
            &routes,
            "GET /hello/alice HTTP/1.1\r\nhost: localhost\r\n\r\n",
            8,
            RuntimePureAcceleratorConfig::default(),
            &native_http_host_calls(),
        )
        .expect("request is handled");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "alice");
    }

    #[test]
    fn native_http_adapter_requires_respond_host_call_manifest() {
        let plan = plan_with_flow("flow.health", vec![FlowOp::Return("ok".to_owned())]);
        let routes = vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/health".to_owned(),
            target: FlowRuntimeId("flow.health".to_owned()),
            bindings: Vec::new(),
        }];

        let error = handle_http_request(
            &plan,
            &routes,
            "GET /health HTTP/1.1\r\nhost: localhost\r\n\r\n",
            8,
            RuntimePureAcceleratorConfig::default(),
            &HostCallPolicy::default(),
        )
        .expect_err("missing http.respond manifest is rejected");

        assert!(matches!(
            error,
            ServerAdapterError::MissingHostCall(id) if id == "http.respond"
        ));
    }

    fn native_http_host_calls() -> HostCallPolicy {
        HostCallPolicy::from_manifests([standard::native_http_manifest()])
    }

    fn plan_with_flow(id: &str, ops: Vec<FlowOp>) -> RuntimePlan {
        RuntimePlan::new(
            Some(FlowRuntimeId(id.to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId(id.to_owned()),
                ops,
            }],
            Vec::new(),
        )
        .expect("plan is valid")
    }
}
