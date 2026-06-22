//! CPAL output/native-input adapter and browser microphone bridge.

#[cfg(feature = "output")]
mod output;

#[cfg(all(feature = "native-microphone", not(target_arch = "wasm32")))]
mod native_microphone;

#[cfg(all(feature = "web-microphone", target_arch = "wasm32"))]
mod web_microphone;

#[cfg(feature = "output")]
pub use output::{CpalOutput, CpalOutputConfig, CpalOutputError};

#[cfg(all(feature = "native-microphone", not(target_arch = "wasm32")))]
pub use native_microphone::{NativeMicrophone, NativeMicrophoneError};

#[cfg(all(feature = "web-microphone", target_arch = "wasm32"))]
pub use web_microphone::{BrowserMicrophone, BrowserMicrophoneError};
