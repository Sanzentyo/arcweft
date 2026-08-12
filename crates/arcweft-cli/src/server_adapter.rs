use arcweft_compiler::runtime_diagnostics::ExecutionDiagnosticContext;
use arcweft_core::engine::{Engine, FlowExit, FlowFiberStatus};
use arcweft_core::plan::{RuntimePlan, RuntimeRouteBindingSource, RuntimeRouteSpec};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
};
use arcweft_core::value::{RuntimeBinding, RuntimeValue};
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
    pub(crate) assertion_diagnostics: Vec<NativeHttpRuntimeDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeHttpRuntimeDiagnostic {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) identity: &'static str,
}

impl NativeHttpResponse {
    fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            assertion_diagnostics: Vec::new(),
        }
    }
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
    #[error("fresh runtime assertion identity projection failed: {0}")]
    AssertionProjection(String),
}

pub(crate) fn serve_native_http(
    plan: &RuntimePlan,
    routes: &[RuntimeRouteSpec],
    config: &NativeHttpServerConfig,
    execution_diagnostics: &ExecutionDiagnosticContext,
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
            execution_diagnostics,
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
    assertion_projector: &impl NativeHttpAssertionProjector,
) -> Result<(), ServerAdapterError> {
    let mut buffer = vec![0_u8; 64 * 1024];
    let read = stream
        .read(&mut buffer)
        .map_err(|error| ServerAdapterError::Read(error.to_string()))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let response = handle_http_request(
        plan,
        routes,
        &request,
        max_ops,
        pure_config,
        host_policy,
        assertion_projector,
    )?;
    for diagnostic in &response.assertion_diagnostics {
        eprintln!("error[{}]: {}", diagnostic.code, diagnostic.message);
    }
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
    assertion_projector: &impl NativeHttpAssertionProjector,
) -> Result<NativeHttpResponse, ServerAdapterError> {
    require_host_call(host_policy, "http.respond")?;
    let parsed = parse_http_request(request)?;
    let Some((route, params)) = routes
        .iter()
        .find_map(|route| route_match(route, &parsed).map(|params| (route, params)))
    else {
        return Ok(NativeHttpResponse::new(404, "not found"));
    };
    run_route_flow(
        plan,
        route,
        &parsed,
        &params,
        max_ops,
        pure_config,
        assertion_projector,
    )
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
    assertion_projector: &impl NativeHttpAssertionProjector,
) -> Result<NativeHttpResponse, ServerAdapterError> {
    let mut pure = RuntimePureAccelerator::with_config(pure_config, &plan.pure_helpers);
    let mut executor = match Engine::for_flow(plan.clone(), &route.target) {
        Ok(executor) => executor,
        Err(error) => {
            return Ok(NativeHttpResponse::new(
                500,
                format!("failed to dispatch server route: {error}"),
            ));
        }
    };
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
    let assertion_diagnostics = result
        .output
        .effects
        .line
        .iter()
        .filter_map(|effect| match effect {
            arcweft_core::effect::LineEffectRequest::Assert(assertion) => Some(
                arcweft_core::effect::RuntimeAssertionFailure::new(assertion.clone()),
            ),
            _ => None,
        })
        .map(|failure| assertion_projector.project(failure))
        .collect::<Result<Vec<_>, _>>()?;
    let mut response = if let Some(diagnostic) = result.output.diagnostics.first() {
        NativeHttpResponse::new(500, diagnostic.message.clone())
    } else {
        match &executor.fiber().status {
            FlowFiberStatus::Done(FlowExit::Return(value)) => {
                NativeHttpResponse::new(200, value.clone())
            }
            FlowFiberStatus::Done(FlowExit::Done) => NativeHttpResponse::new(204, String::new()),
            FlowFiberStatus::Failed(message) => NativeHttpResponse::new(500, message.clone()),
            FlowFiberStatus::Running
            | FlowFiberStatus::Dialogue(_)
            | FlowFiberStatus::Waiting(_)
            | FlowFiberStatus::NeedWaiting(_)
            | FlowFiberStatus::WaitingMany(_)
            | FlowFiberStatus::HostCall(_)
            | FlowFiberStatus::Choice(_) => {
                NativeHttpResponse::new(202, "route did not complete in this server step")
            }
        }
    };
    response.assertion_diagnostics = assertion_diagnostics;
    Ok(response)
}

pub(crate) trait NativeHttpAssertionProjector {
    fn project(
        &self,
        failure: arcweft_core::effect::RuntimeAssertionFailure,
    ) -> Result<NativeHttpRuntimeDiagnostic, ServerAdapterError>;
}

impl NativeHttpAssertionProjector for ExecutionDiagnosticContext {
    fn project(
        &self,
        failure: arcweft_core::effect::RuntimeAssertionFailure,
    ) -> Result<NativeHttpRuntimeDiagnostic, ServerAdapterError> {
        let fault = self
            .project_assertion_failure(failure)
            .map_err(|error| ServerAdapterError::AssertionProjection(error.to_string()))?;
        let diagnostic =
            arcweft_tooling::runtime_diagnostic::project_runtime_assertion_fault(&fault);
        Ok(NativeHttpRuntimeDiagnostic {
            code: diagnostic.code(),
            message: diagnostic.message().to_owned(),
            identity: "session",
        })
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
        value: RuntimeValue::try_record(vec![
            (
                "method".to_owned(),
                RuntimeValue::String(request.method.clone()),
            ),
            (
                "path".to_owned(),
                RuntimeValue::String(request.path.clone()),
            ),
            (
                "body".to_owned(),
                RuntimeValue::String(request.body.clone()),
            ),
        ])
        .expect("HTTP request runtime record has fixed unique fields"),
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
    use arcweft_core::effect::{
        LineEffectRequest, RuntimeAssertion, RuntimeAssertionGuardId, RuntimeAssertionProfile,
    };
    use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow};

    struct SessionTestAssertionProjector;

    impl NativeHttpAssertionProjector for SessionTestAssertionProjector {
        fn project(
            &self,
            failure: arcweft_core::effect::RuntimeAssertionFailure,
        ) -> Result<NativeHttpRuntimeDiagnostic, ServerAdapterError> {
            Ok(NativeHttpRuntimeDiagnostic {
                code: "runtime.assertion_failed",
                message: failure.assertion().message().to_owned(),
                identity: "session",
            })
        }
    }

    #[test]
    fn native_http_adapter_routes_request_to_flow() {
        let plan = plan_with_flow("flow.health", vec![FlowOp::Return("ok".to_owned())]);
        let routes = vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/health".to_owned(),
            target: FlowRuntimeId::from_runtime_target_value("flow.health")
                .expect("flow runtime id"),
            bindings: Vec::new(),
        }];

        let response = handle_http_request(
            &plan,
            &routes,
            "GET /health HTTP/1.1\r\nhost: localhost\r\n\r\n",
            8,
            RuntimePureAcceleratorConfig::default(),
            &native_http_host_calls(),
            &SessionTestAssertionProjector,
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
            target: FlowRuntimeId::from_runtime_target_value("flow.hello")
                .expect("flow runtime id"),
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
            &SessionTestAssertionProjector,
        )
        .expect("request is handled");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "alice");
    }

    #[test]
    fn native_http_adapter_reports_runtime_assertion_without_changing_flow_status() {
        let plan = plan_with_flow(
            "flow.assertion",
            vec![
                FlowOp::Effect(LineEffectRequest::Assert(RuntimeAssertion::new(
                    RuntimeAssertionGuardId::try_from_bytes([0x41; 16])
                        .expect("fixture assertion guard"),
                    "ready".to_owned(),
                    "runtime condition failed".to_owned(),
                    RuntimeAssertionProfile::Always,
                ))),
                FlowOp::Return("ok".to_owned()),
            ],
        );
        let routes = vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/assertion".to_owned(),
            target: FlowRuntimeId::from_runtime_target_value("flow.assertion")
                .expect("flow runtime id"),
            bindings: Vec::new(),
        }];

        let response = handle_http_request(
            &plan,
            &routes,
            "GET /assertion HTTP/1.1\r\nhost: localhost\r\n\r\n",
            8,
            RuntimePureAcceleratorConfig::default(),
            &native_http_host_calls(),
            &SessionTestAssertionProjector,
        )
        .expect("request is handled");

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "ok");
        assert_eq!(response.assertion_diagnostics.len(), 1);
        assert_eq!(
            response.assertion_diagnostics[0].code,
            "runtime.assertion_failed"
        );
        assert_eq!(
            response.assertion_diagnostics[0].message,
            "runtime condition failed"
        );
        assert_eq!(response.assertion_diagnostics[0].identity, "session");
    }

    #[test]
    fn native_http_adapter_requires_respond_host_call_manifest() {
        let plan = plan_with_flow("flow.health", vec![FlowOp::Return("ok".to_owned())]);
        let routes = vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/health".to_owned(),
            target: FlowRuntimeId::from_runtime_target_value("flow.health")
                .expect("flow runtime id"),
            bindings: Vec::new(),
        }];

        let error = handle_http_request(
            &plan,
            &routes,
            "GET /health HTTP/1.1\r\nhost: localhost\r\n\r\n",
            8,
            RuntimePureAcceleratorConfig::default(),
            &HostCallPolicy::default(),
            &SessionTestAssertionProjector,
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
        let id = FlowRuntimeId::from_runtime_target_value(id).expect("flow runtime id");
        RuntimePlan::new(vec![RuntimeFlow { id, ops }], Vec::new()).expect("plan is valid")
    }
}
