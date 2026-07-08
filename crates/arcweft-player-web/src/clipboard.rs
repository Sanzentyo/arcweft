//! Browser text clipboard adapter for runtime text controls.
//!
//! The module keeps browser permission and `navigator.clipboard` details out of
//! the Sans I/O editor model while preserving the same request/outcome contract
//! used by the native player.

use arcweft_presentation::clipboard::{
    ClipboardText, TextClipboardError, TextClipboardErrorKind, TextClipboardOperation,
    TextClipboardOutcome, TextClipboardRequest,
};
use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

pub async fn apply_clipboard_request(request: TextClipboardRequest) -> TextClipboardOutcome {
    match request.operation() {
        TextClipboardOperation::Copy | TextClipboardOperation::Cut => write_text(request).await,
        TextClipboardOperation::Paste => read_text(request).await,
        TextClipboardOperation::Clear => failed(
            request.request_id(),
            request.operation(),
            TextClipboardErrorKind::Unavailable,
            "web clipboard clear is not exposed as a stable text-control operation",
        ),
    }
}

async fn write_text(request: TextClipboardRequest) -> TextClipboardOutcome {
    let request_id = request.request_id();
    let operation = request.operation();
    let TextClipboardRequest::Write(write) = request else {
        return failed(
            request_id,
            operation,
            TextClipboardErrorKind::InternalFailure,
            "non-write request routed to clipboard write",
        );
    };
    let Ok(function) = clipboard_function("writeText") else {
        return failed(
            request_id,
            operation,
            TextClipboardErrorKind::Unavailable,
            "navigator.clipboard.writeText unavailable",
        );
    };
    let Some(clipboard) = clipboard_object() else {
        return failed(
            request_id,
            operation,
            TextClipboardErrorKind::Unavailable,
            "navigator.clipboard unavailable",
        );
    };
    let promise = function.call1(&clipboard, &JsValue::from_str(write.text().as_str()));
    match await_promise(promise).await {
        Ok(_) => TextClipboardOutcome::WriteCommitted { request_id },
        Err(error) => failed_js(request_id, operation, error),
    }
}

async fn read_text(request: TextClipboardRequest) -> TextClipboardOutcome {
    let request_id = request.request_id();
    let operation = request.operation();
    let Ok(function) = clipboard_function("readText") else {
        return failed(
            request_id,
            operation,
            TextClipboardErrorKind::Unavailable,
            "navigator.clipboard.readText unavailable",
        );
    };
    let Some(clipboard) = clipboard_object() else {
        return failed(
            request_id,
            operation,
            TextClipboardErrorKind::Unavailable,
            "navigator.clipboard unavailable",
        );
    };
    let promise = function.call0(&clipboard);
    match await_promise(promise).await {
        Ok(value) => {
            let text = value.as_string().unwrap_or_default();
            TextClipboardOutcome::ReadCommitted {
                request_id,
                text: ClipboardText::new(text),
            }
        }
        Err(error) => failed_js(request_id, operation, error),
    }
}

fn clipboard_function(name: &str) -> Result<Function, ()> {
    if !secure_context() {
        return Err(());
    }
    let Some(clipboard) = clipboard_object() else {
        return Err(());
    };
    Reflect::get(&clipboard, &JsValue::from_str(name))
        .ok()
        .filter(JsValue::is_function)
        .map(JsCast::unchecked_into)
        .ok_or(())
}

fn clipboard_object() -> Option<JsValue> {
    let navigator = Reflect::get(&js_sys::global(), &JsValue::from_str("navigator")).ok()?;
    Reflect::get(&navigator, &JsValue::from_str("clipboard"))
        .ok()
        .filter(|value| !value.is_null() && !value.is_undefined())
}

fn secure_context() -> bool {
    Reflect::get(&js_sys::global(), &JsValue::from_str("isSecureContext"))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

async fn await_promise(promise: Result<JsValue, JsValue>) -> Result<JsValue, JsValue> {
    let promise = promise?;
    let promise: Promise = promise.dyn_into()?;
    JsFuture::from(promise).await
}

fn failed_js(
    request_id: arcweft_presentation::clipboard::TextClipboardRequestId,
    operation: TextClipboardOperation,
    error: JsValue,
) -> TextClipboardOutcome {
    failed(
        request_id,
        operation,
        web_clipboard_error_kind(&error),
        js_error_to_string(&error),
    )
}

fn failed(
    request_id: arcweft_presentation::clipboard::TextClipboardRequestId,
    operation: TextClipboardOperation,
    kind: TextClipboardErrorKind,
    diagnostic: impl Into<String>,
) -> TextClipboardOutcome {
    TextClipboardOutcome::Failed {
        request_id,
        error: TextClipboardError::new(kind, operation).with_diagnostic(diagnostic),
    }
}

fn web_clipboard_error_kind(error: &JsValue) -> TextClipboardErrorKind {
    match js_error_name(error).as_deref() {
        Some("NotAllowedError" | "SecurityError") => TextClipboardErrorKind::Denied,
        Some("NotFoundError") => TextClipboardErrorKind::UnsupportedFormat,
        _ => TextClipboardErrorKind::InternalFailure,
    }
}

fn js_error_name(error: &JsValue) -> Option<String> {
    Reflect::get(error, &JsValue::from_str("name"))
        .ok()
        .and_then(|value| value.as_string())
}

fn js_error_to_string(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| js_error_name(error))
        .unwrap_or_else(|| "non-string JavaScript error".to_owned())
}
