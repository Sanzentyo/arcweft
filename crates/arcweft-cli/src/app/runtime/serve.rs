use super::entry::{select_server_entry, server_routes};
use super::options::ServeOptions;
use crate::app::project::{
    SourceSelection, load_and_check_selection, native_host_policy_for_selection_with_adapter,
    require_profile_kind, resolve_source_selection, runtime_pure_config_for_selection,
};
use crate::app::shared::print_json;
use crate::server_adapter::{NativeHttpServerConfig, serve_native_http};
use arcweft_adapter_context::standard;
use arcweft_launch::{LaunchKind, manifest::LaunchListenAddress, resolve::ResolvedLaunchProfile};
use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
use arcweft_runtime_host::NativeAdapterRegistrar;
use std::net::SocketAddr;
use std::process::ExitCode;

#[derive(serde::Serialize)]
struct ServePlanReport {
    status: String,
    entry: String,
    adapter: String,
    routes: Vec<ServeRouteReport>,
}

#[derive(serde::Serialize)]
struct ServeRouteReport {
    method: String,
    path: String,
    target: String,
}

#[derive(serde::Serialize)]
struct ServeRunReport {
    plan: ServePlanReport,
    server: crate::server_adapter::NativeHttpServerReport,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct RuntimeServeSelectionConfig {
    pub(in crate::app) listen: Option<SocketAddr>,
    pub(in crate::app) once: bool,
    pub(in crate::app) max_ops: usize,
    pub(in crate::app) pure_config: RuntimePureAcceleratorConfig,
    pub(in crate::app) json: bool,
}

pub(in crate::app) fn runtime_serve_command(
    options: &ServeOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Server, "serve")?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    );
    runtime_serve_selection(
        &selection,
        options.entry.as_deref(),
        options.adapter.as_deref(),
        RuntimeServeSelectionConfig {
            listen: options.listen,
            once: options.once,
            max_ops: options.max_ops,
            pure_config,
            json: options.json,
        },
        adapter_registrars,
    )
}

pub(in crate::app) fn runtime_serve_selection(
    selection: &SourceSelection,
    entry_override: Option<&str>,
    adapter_override: Option<&str>,
    config: RuntimeServeSelectionConfig,
    _adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let adapter = adapter_override
        .or(selection.adapter())
        .unwrap_or(standard::SANS_IO_ADAPTER_ID);
    let checked = load_and_check_selection(selection, adapter_override)?;
    let host_policy = native_host_policy_for_selection_with_adapter(selection, adapter_override)?;
    let plan = checked.runtime_plan().plan.clone();
    let entry = select_server_entry(&plan, selection.command_entry(entry_override)?)?;
    let Some(routes) = server_routes(entry) else {
        eprintln!(
            "error: server entry `{}` has no runnable routes",
            entry.id.public_label()
        );
        return Err(ExitCode::FAILURE);
    };
    if routes.is_empty() {
        eprintln!(
            "error: server entry `{}` has no runnable routes",
            entry.id.public_label()
        );
        return Err(ExitCode::FAILURE);
    }
    for route in routes {
        if !plan.flows().iter().any(|flow| flow.id == route.target) {
            eprintln!(
                "error: server route {} {} targets unknown flow `{}`",
                route.method.as_str(),
                route.path,
                route.target.public_label()
            );
            return Err(ExitCode::FAILURE);
        }
    }
    let report = ServePlanReport {
        status: "planned".to_owned(),
        entry: entry.id.public_label().into_string(),
        adapter: adapter.to_owned(),
        routes: routes
            .iter()
            .map(|route| ServeRouteReport {
                method: route.method.as_str().to_owned(),
                path: route.path.to_string(),
                target: route.target.public_label().into_string(),
            })
            .collect(),
    };
    let listen = match config.listen {
        Some(listen) => Some(listen),
        None => serve_profile_listen_addr(selection),
    };
    if let Some(listen) = listen {
        let server_report = serve_native_http(
            &plan,
            routes,
            &NativeHttpServerConfig {
                listen,
                once: config.once,
                max_ops: config.max_ops,
                pure_config: config.pure_config,
                host_policy,
            },
            &checked.execution_diagnostics,
        )
        .map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })?;
        let report = ServeRunReport {
            plan: report,
            server: server_report,
        };
        return if config.json {
            print_json(&report)
        } else {
            println!(
                "ok: served {} request(s) on {}",
                report.server.handled_requests, report.server.listen
            );
            Ok(())
        };
    }
    if config.json {
        print_json(&report)
    } else {
        for route in &report.routes {
            println!("{} {} -> {}", route.method, route.path, route.target);
        }
        println!(
            "ok: {} (server entry {}, adapter={}, {} route(s), status={})",
            selection.path().display(),
            report.entry,
            report.adapter,
            report.routes.len(),
            report.status
        );
        Ok(())
    }
}

fn serve_profile_listen_addr(selection: &SourceSelection) -> Option<SocketAddr> {
    selection
        .profile()
        .and_then(ResolvedLaunchProfile::listen)
        .map(LaunchListenAddress::socket_addr)
}
