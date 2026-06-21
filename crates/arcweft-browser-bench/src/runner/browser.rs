use arcweft_runtime_accelerator::math::browser_webgpu::BrowserWebGpuError;
use wasm_bindgen_futures::JsFuture;

pub(crate) fn fallback_reason(error: &BrowserWebGpuError) -> String {
    error
        .reason()
        .map(|reason| format!("{reason:?}"))
        .unwrap_or_else(|| "Math".to_owned())
}

pub(crate) async fn yield_to_browser() {
    let _ = JsFuture::from(js_sys::Promise::resolve(&wasm_bindgen::JsValue::NULL)).await;
}

pub(crate) fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map(|performance| performance.now())
        .unwrap_or_else(js_sys::Date::now)
}
