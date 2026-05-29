//! Host request lowering for awaited capability calls.
//!
//! This module keeps runtime-plan lowering Sans I/O: it recognizes capability
//! call shapes and emits data-only requests. Host adapters decide whether and
//! how those requests are executed.

use crate::labels::{duration_expr, expr_label, literal_label};
use arcweft_core::task::{
    AssetRequest, AudioDecodeRequest, FileReadBytesRequest, FileReadTextRequest,
    FileWriteBytesRequest, FileWriteTextRequest, HostTaskRequest, HttpFetchRequest,
    HttpRespondRequest, ProcessRunRequest, ShaderRequest, TtsRequest, WasmCallRequest,
};
use arcweft_core::value::{RuntimeFieldValue, RuntimePayload, RuntimeValue};
use arcweft_lang_hir::syntax::expr::{Expr, Literal};

struct CallParts<'a> {
    capability: String,
    operation: String,
    args: &'a [Expr],
}

/// Lowers an awaited expression to the typed request that should be offered to
/// the host. Unknown call namespaces remain structured `Custom` requests.
pub(crate) fn lower_host_task_request(expr: &Expr) -> HostTaskRequest {
    let Some(call) = call_parts(expr) else {
        return HostTaskRequest::custom("await", "expr", [RuntimePayload::from(expr_label(expr))]);
    };
    lower_known_request(&call).unwrap_or_else(|| lower_custom_request(&call))
}

fn lower_known_request(call: &CallParts<'_>) -> Option<HostTaskRequest> {
    match (call.capability.as_str(), call.operation.as_str()) {
        ("file" | "fs", "read_text") => Some(HostTaskRequest::FileReadText(FileReadTextRequest {
            path: string_arg(call.args.first()?)?,
        })),
        ("file" | "fs", "read_bytes") => {
            Some(HostTaskRequest::FileReadBytes(FileReadBytesRequest {
                path: string_arg(call.args.first()?)?,
            }))
        }
        ("file" | "fs", "write_text") => {
            Some(HostTaskRequest::FileWriteText(FileWriteTextRequest {
                path: string_arg(call.args.first()?)?,
                text: string_arg(positional_arg(call.args, 1)?)?,
            }))
        }
        ("file" | "fs", "write_bytes") => {
            Some(HostTaskRequest::FileWriteBytes(FileWriteBytesRequest {
                path: string_arg(call.args.first()?)?,
                bytes: byte_seq(positional_arg(call.args, 1)?)?,
            }))
        }
        ("http", "fetch") => Some(HostTaskRequest::HttpFetch(HttpFetchRequest {
            url: string_arg(call.args.first()?)?,
            method: named_string(call.args, "method").unwrap_or_else(|| "GET".to_owned()),
            headers: named_header_pairs(call.args, "headers").unwrap_or_default(),
            body: named_payload(call.args, "body"),
        })),
        ("http", "respond") => Some(HostTaskRequest::HttpRespond(HttpRespondRequest {
            request_id: string_arg(call.args.first()?)?,
            status: named_u16(call.args, "status").unwrap_or(200),
            headers: named_header_pairs(call.args, "headers").unwrap_or_default(),
            body: named_payload(call.args, "body"),
        })),
        ("process", "run") => Some(HostTaskRequest::ProcessRun(ProcessRunRequest {
            program: string_arg(call.args.first()?)?,
            args: named_string_seq(call.args, "args").unwrap_or_default(),
            env: named_header_pairs(call.args, "env").unwrap_or_default(),
        })),
        ("asset", "load") => Some(HostTaskRequest::AssetLoad(AssetRequest {
            id: string_arg(call.args.first()?)?,
            kind: named_string(call.args, "kind").unwrap_or_else(|| "asset".to_owned()),
        })),
        ("asset", kind) => Some(HostTaskRequest::AssetLoad(AssetRequest {
            id: string_arg(call.args.first()?)?,
            kind: kind.to_owned(),
        })),
        ("voice", "load") => Some(HostTaskRequest::AssetLoad(AssetRequest {
            id: string_arg(call.args.first()?)?,
            kind: "voice".to_owned(),
        })),
        ("shader", "compile") => Some(HostTaskRequest::ShaderCompile(ShaderRequest {
            id: string_arg(call.args.first()?)?,
            entry: named_string(call.args, "entry"),
        })),
        ("audio", "decode") => Some(HostTaskRequest::AudioDecode(AudioDecodeRequest {
            id: string_arg(call.args.first()?)?,
        })),
        ("tts", "synthesize" | "synthesis") => Some(HostTaskRequest::TtsSynthesis(TtsRequest {
            voice: named_string(call.args, "voice"),
            text: named_string(call.args, "text")
                .or_else(|| call.args.first().and_then(string_arg))?,
        })),
        ("wasm", "call") => Some(HostTaskRequest::WasmCall(WasmCallRequest {
            module: string_arg(call.args.first()?)?,
            function: string_arg(positional_arg(call.args, 1)?)?,
            args: positional_args_after(call.args, 2)
                .into_iter()
                .map(payload_arg)
                .collect(),
        })),
        _ => None,
    }
}

fn lower_custom_request(call: &CallParts<'_>) -> HostTaskRequest {
    HostTaskRequest::custom(
        call.capability.clone(),
        call.operation.clone(),
        call.args.iter().map(payload_arg),
    )
}

fn call_parts(expr: &Expr) -> Option<CallParts<'_>> {
    match expr {
        Expr::Call { callee, args } => {
            let name = expr_label(callee);
            let (capability, operation) = split_capability_operation(&name);
            Some(CallParts {
                capability,
                operation,
                args,
            })
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => Some(CallParts {
            capability: expr_label(receiver),
            operation: method_name(method).to_owned(),
            args,
        }),
        Expr::Await { expr, .. } | Expr::Try { expr } => call_parts(expr),
        _ => None,
    }
}

fn split_capability_operation(name: &str) -> (String, String) {
    name.rsplit_once('.').map_or_else(
        || ("await".to_owned(), name.to_owned()),
        |(capability, operation)| (capability.to_owned(), operation.to_owned()),
    )
}

fn method_name(method: &str) -> &str {
    method.split_once('<').map_or(method, |(name, _)| name)
}

fn positional_arg(args: &[Expr], index: usize) -> Option<&Expr> {
    args.iter()
        .filter(|arg| !matches!(arg, Expr::NamedArg { .. }))
        .nth(index)
}

fn positional_args_after(args: &[Expr], count: usize) -> Vec<&Expr> {
    args.iter()
        .filter(|arg| !matches!(arg, Expr::NamedArg { .. }))
        .skip(count)
        .collect()
}

fn named_arg<'a>(args: &'a [Expr], name: &str) -> Option<&'a Expr> {
    args.iter().find_map(|arg| match arg {
        Expr::NamedArg {
            name: arg_name,
            value,
        } if arg_name == name => Some(value.as_ref()),
        _ => None,
    })
}

fn string_arg(expr: &Expr) -> Option<String> {
    Some(match expr {
        Expr::Literal(Literal::String(value)) => value.clone(),
        Expr::Literal(literal) => literal_label(literal),
        Expr::EntityRef(entity) => entity.body().to_owned(),
        Expr::Path(path) => path.clone(),
        Expr::NamedArg { value, .. } => string_arg(value)?,
        other => expr_label(other),
    })
}

fn named_string(args: &[Expr], name: &str) -> Option<String> {
    named_arg(args, name).and_then(string_arg)
}

fn named_payload(args: &[Expr], name: &str) -> Option<RuntimePayload> {
    named_arg(args, name).map(payload_arg)
}

fn named_u16(args: &[Expr], name: &str) -> Option<u16> {
    match named_arg(args, name)? {
        Expr::Literal(Literal::Int { value, .. }) => u16::try_from(*value).ok(),
        expr => string_arg(expr)?.parse().ok(),
    }
}

fn named_string_seq(args: &[Expr], name: &str) -> Option<Vec<String>> {
    match named_arg(args, name)? {
        Expr::BracketSeq(items) | Expr::Tuple(items) => {
            items.iter().map(string_arg).collect::<Option<Vec<_>>>()
        }
        expr => Some(vec![string_arg(expr)?]),
    }
}

fn named_header_pairs(args: &[Expr], name: &str) -> Option<Vec<(String, String)>> {
    let expr = named_arg(args, name)?;
    header_pairs(expr)
}

fn header_pairs(expr: &Expr) -> Option<Vec<(String, String)>> {
    match expr {
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .map(|(name, value)| Some((name.clone(), string_arg(value)?)))
            .collect(),
        Expr::BracketSeq(items) | Expr::Tuple(items) => items.iter().map(header_pair).collect(),
        _ => None,
    }
}

fn header_pair(expr: &Expr) -> Option<(String, String)> {
    match expr {
        Expr::Tuple(items) if items.len() == 2 => {
            Some((string_arg(&items[0])?, string_arg(&items[1])?))
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            let key = fields
                .iter()
                .find(|(name, _)| name == "key" || name == "name")
                .and_then(|(_, value)| string_arg(value))?;
            let value = fields
                .iter()
                .find(|(name, _)| name == "value")
                .and_then(|(_, value)| string_arg(value))?;
            Some((key, value))
        }
        _ => None,
    }
}

fn byte_seq(expr: &Expr) -> Option<Vec<u8>> {
    let Expr::BracketSeq(items) = expr else {
        return None;
    };
    items
        .iter()
        .map(|item| match item {
            Expr::Literal(Literal::Int { value, .. }) => u8::try_from(*value).ok(),
            _ => None,
        })
        .collect()
}

fn payload_arg(expr: &Expr) -> RuntimePayload {
    RuntimePayload::new(payload_value(expr))
}

fn payload_value(expr: &Expr) -> RuntimeValue {
    match expr {
        Expr::Literal(Literal::String(value)) => RuntimeValue::String(value.clone()),
        Expr::Literal(Literal::Char { value, .. }) => RuntimeValue::Char(*value),
        Expr::Literal(Literal::Int { value, .. }) => RuntimeValue::Int(*value),
        Expr::Literal(Literal::Float { raw, .. }) => RuntimeValue::Float(raw.clone()),
        Expr::Literal(Literal::Bool(value)) => RuntimeValue::Bool(*value),
        Expr::Literal(Literal::Duration { .. }) => duration_expr(expr).map_or_else(
            || RuntimeValue::String(expr_label(expr)),
            RuntimeValue::Duration,
        ),
        Expr::EntityRef(entity) => RuntimeValue::EntityRef(entity.body().to_owned()),
        Expr::Tuple(items) => RuntimeValue::Tuple(items.iter().map(payload_value).collect()),
        Expr::BracketSeq(items) => {
            RuntimeValue::BracketSeq(items.iter().map(payload_value).collect())
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => RuntimeValue::Record(
            fields
                .iter()
                .map(|(name, value)| RuntimeFieldValue {
                    name: name.clone(),
                    value: payload_value(value),
                })
                .collect(),
        ),
        Expr::NamedArg { name, value } => RuntimeValue::Record(vec![RuntimeFieldValue {
            name: name.clone(),
            value: payload_value(value),
        }]),
        other => RuntimeValue::String(expr_label(other)),
    }
}
