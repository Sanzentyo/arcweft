use super::super::runtime_observation::agent_capture_scope_for_options;
use super::super::{
    AgentCaptureScope, AgentObserveOptions, agent_capture_time_millis,
    agent_capture_time_seconds_from_step,
};

/// Sampling belongs to the observed frame, separately from an image crop or
/// file-output request. A capture-step sample is local to the initial observe.
#[derive(Clone, Copy, Debug)]
pub(in crate::app::agent::native) enum NativeFrameSample {
    Implicit,
    Seconds(f32),
    CaptureStep(usize),
}

impl NativeFrameSample {
    pub(in crate::app::agent::native) fn new(seconds: Option<f32>, step: Option<usize>) -> Self {
        match (seconds, step) {
            (Some(seconds), _) => Self::Seconds(seconds),
            (None, Some(step)) => Self::CaptureStep(step),
            (None, None) => Self::Implicit,
        }
    }

    pub(in crate::app::agent::native) fn seconds(self) -> f32 {
        match self {
            Self::Implicit => 60.0,
            Self::Seconds(seconds) => seconds,
            Self::CaptureStep(step) => agent_capture_time_seconds_from_step(step),
        }
    }

    pub(in crate::app::agent::native) fn reported_millis(self) -> Option<u32> {
        (!matches!(self, Self::Implicit)).then(|| agent_capture_time_millis(self.seconds()))
    }
}

/// The complete request consumed by an already-created native runtime.
/// Launch, image encoding/output, URI reads and crop selectors are not retained
/// as session execution settings. Initial scope diagnostics use the same typed
/// capture-scope authority as actual capture requests.
#[derive(Clone, Debug)]
pub(in crate::app::agent::native) struct NativeFrameObservationRequest {
    pub(super) steps: usize,
    pub(super) force_capture_step: bool,
    pub(super) viewport_width: u32,
    pub(super) viewport_height: u32,
    pub(super) sample: NativeFrameSample,
    pub(super) diagnostic_scope: AgentCaptureScope,
}

impl From<&AgentObserveOptions> for NativeFrameObservationRequest {
    fn from(options: &AgentObserveOptions) -> Self {
        Self {
            steps: options.capture_step.unwrap_or(options.steps),
            force_capture_step: options.capture_step.is_some(),
            viewport_width: options.viewport_width,
            viewport_height: options.viewport_height,
            sample: NativeFrameSample::new(options.capture_time_seconds, options.capture_step),
            diagnostic_scope: agent_capture_scope_for_options(options),
        }
    }
}

impl NativeFrameObservationRequest {
    /// Session progress keeps runtime bounds, viewport and explicit sampling,
    /// without carrying a previous request's crop or capture-step frontier.
    pub(in crate::app::agent::native) fn for_progress(options: &AgentObserveOptions) -> Self {
        Self {
            steps: options.steps,
            force_capture_step: false,
            viewport_width: options.viewport_width,
            viewport_height: options.viewport_height,
            sample: NativeFrameSample::new(options.capture_time_seconds, None),
            diagnostic_scope: AgentCaptureScope::Viewport,
        }
    }

    pub(in crate::app::agent::native) fn with_step_count(mut self, count: usize) -> Self {
        self.steps = count.max(1);
        self.force_capture_step = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_drops_initial_crop_and_capture_frontier_but_retains_viewport_and_step_bounds() {
        let options = crate::app::agent::native::mcp_resources::agent_mcp_observe_options(
            &serde_json::json!({
                "source": "dialogue.arcw", "entry": "entry.main",
                "steps": 4, "capture_step": 2, "max_ops": 128,
                "image": "png", "layer": "dialogue.rich_text",
                "viewport_width": 900, "viewport_height": 600,
            }),
        )
        .unwrap();
        let initial = NativeFrameObservationRequest::from(&options);
        assert_eq!(initial.steps, 2);
        assert!(initial.force_capture_step);
        assert_eq!(initial.sample.reported_millis(), Some(2_000));
        assert!(matches!(
            initial.diagnostic_scope,
            AgentCaptureScope::Layer(_)
        ));
        let progress = NativeFrameObservationRequest::for_progress(&options);
        assert_eq!(progress.steps, 4);
        assert!(!progress.force_capture_step);
        assert_eq!(
            (progress.viewport_width, progress.viewport_height),
            (900, 600)
        );
        assert_eq!(progress.sample.reported_millis(), None);
        assert!(matches!(
            progress.diagnostic_scope,
            AgentCaptureScope::Viewport
        ));
        assert_eq!(progress.clone().with_step_count(1).steps, 1);
        assert_eq!(progress.with_step_count(3).steps, 3);
        assert_eq!(options.max_ops, 128);
    }

    #[test]
    fn explicit_zero_sample_is_distinct_from_implicit_time_and_overrides_capture_step() {
        let implicit = NativeFrameSample::new(None, None);
        assert_eq!(implicit.seconds(), 60.0);
        assert_eq!(implicit.reported_millis(), None);
        let explicit = NativeFrameSample::new(Some(0.0), Some(3));
        assert_eq!(explicit.seconds(), 0.0);
        assert_eq!(explicit.reported_millis(), Some(0));
        assert_eq!(
            NativeFrameSample::new(None, Some(3)).reported_millis(),
            Some(3_000)
        );
    }
}
