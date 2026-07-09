//! Host-side runtime execution boundary for Arcweft bundles and native tasks.

pub mod activity_host;
pub mod bundle_runner;
pub mod capabilities;
pub mod clipboard_host;
pub mod native_system;
pub mod native_task;
pub mod player_text_input_bridge;
pub mod presentation_dispatch;
pub mod stats;
pub mod text_input_dispatch;
pub mod view_frame;

pub use activity_host::{
    ActivityHost, ActivityHostError, ActivityHostRegistrationError, ActivityHostRegistry,
    ActivityHostStepError, ActivityStepInputRef, ActivityStepOutput, ActivityStepOutputSink,
};
pub use arcweft_core::value::RuntimeBinding;
pub use arcweft_runtime_accelerator::RuntimePureAcceleratorConfig;
pub use bundle_runner::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerPhase,
    BundleRunnerReport, BundleRunnerSession, BundleRunnerSessionStep, BundleRunnerStepMode,
    BundleRunnerStepSummary, run_bundle_file_with_native_adapters, run_bundle_with_native_adapters,
};
pub use capabilities::{
    RuntimeHostCapabilities, RuntimeHostConformanceDiagnostic,
    RuntimeHostConformanceDiagnosticKind, RuntimeHostConformanceReport, RuntimeHostRunnerKind,
};
pub use native_system::{HostSystemInfo, host_system_info, system_info_value};
pub use native_task::{
    INTERNAL_SCHEDULER_ADAPTER_ID, NativeAdapterRegistrar, NativeFileRoots, NativeSchedulerStats,
    NativeTaskBridge, NativeTaskClassCounts, NativeTaskStats, internal_scheduler_manifest,
};
pub use player_text_input_bridge::{
    PlayerTextInputBridgeCore, PlayerTextInputEdit, PlayerTextInputFocusedControl,
    PlayerTextInputHostCommandSink, PlayerTextInputSync, PlayerTextInputSyncPhase,
};
pub use presentation_dispatch::{
    DispatchedPresentationAction, PresentationActionDestination, PresentationActionDispatchError,
    PresentationActionDispatchPlan, PresentationActionEffectTarget,
    PresentationActionExecutionError, PresentationActionHandlerEffect,
    PresentationActionHandlerError, PresentationActionHandlerOutput,
    PresentationActionHandlerRegistration, PresentationActionHandlerRegistrationError,
    PresentationActionHandlerRegistry, PresentationActionHandlers, PresentationHostEventSource,
    dispatch_presentation_action, dispatch_presentation_action_batch, dispatch_semantic_invoke,
    execute_presentation_action_plan,
};
pub use stats::{
    RuntimeExecutorMathStatsSummary, RuntimeExecutorPureAccelerationSummary,
    RuntimeExecutorPureCompileStatsSummary, RuntimeExecutorPureConfigSummary,
    RuntimeExecutorPureWorkerSummary, RuntimeExecutorStats, runtime_executor_stats,
};
pub use text_input_dispatch::{
    FocusedTextInputSession, TextInputDispatchError, TextInputDispatchOutput,
    TextInputDispatchState, TextInputFocusTransaction, dispatch_event_suppresses_shortcuts,
    web_edit_context_capabilities,
};
pub use view_frame::{
    ViewFrameCommit, ViewFrameCommitBuilder, ViewFrameCommitError, ViewFrameImageItem,
    ViewFrameLayer, ViewFrameResolvedLayer,
};
