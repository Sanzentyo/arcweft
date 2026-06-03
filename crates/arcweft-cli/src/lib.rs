//! Library entry points for embedding the Arcweft CLI runner.

mod app;
mod native_system;
mod native_task;
mod output;
mod server_adapter;
mod toolchain_profile;

pub use app::{run, run_with_native_adapters};
pub use native_task::NativeAdapterRegistrar;

pub(crate) use app::print_json;
