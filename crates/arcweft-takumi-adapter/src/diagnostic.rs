use crate::metadata::TakumiPath;
use arcweft_ui::NodeId;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TakumiDiagnosticCode {
    CssParseFailed,
    UnsupportedDirectCss,
    CssCoverageGap,
    UnsupportedCssSelector,
    UnresolvedCssVariable,
    CpuRasterFallbackForbidden,
    MissingFragmentRoot,
    MissingTakumiLayout,
    CapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakumiDiagnostic {
    code: TakumiDiagnosticCode,
    path: Option<TakumiPath>,
    message: String,
}

#[derive(Debug, Error)]
pub enum TakumiAdapterError {
    #[error("Takumi CSS parse failed: {message}")]
    CssParseFailed { message: String },
    #[error("UI fragment root {0:?} does not exist")]
    MissingFragmentRoot(NodeId),
    #[error("Takumi scene extraction failed: {message}")]
    SceneExtractionFailed { message: String },
    #[error("too many UI primitives or capture records")]
    CapacityExceeded,
}

impl TakumiDiagnostic {
    pub fn new(code: TakumiDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            path: None,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn with_path(mut self, path: TakumiPath) -> Self {
        self.path = Some(path);
        self
    }

    pub fn code(&self) -> TakumiDiagnosticCode {
        self.code
    }

    pub fn path(&self) -> Option<&TakumiPath> {
        self.path.as_ref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn cpu_raster_fallback_forbidden() -> Self {
        Self::new(
            TakumiDiagnosticCode::CpuRasterFallbackForbidden,
            "CPU RGBA UI-surface fallback is forbidden for the seq06 direct-wgpu path",
        )
    }

    pub fn unsupported_css(property: impl Into<String>) -> Self {
        let property = property.into();
        Self::new(
            TakumiDiagnosticCode::UnsupportedDirectCss,
            format!(
                "CSS property `{property}` is accepted by Takumi but has no direct-wgpu lowering in this cut"
            ),
        )
    }

    pub fn css_coverage_gap(feature: impl Into<String>, status: impl std::fmt::Debug) -> Self {
        let feature = feature.into();
        Self::new(
            TakumiDiagnosticCode::CssCoverageGap,
            format!(
                "CSS feature `{feature}` is classified as {status:?} by the seq06.12 coverage matrix"
            ),
        )
    }

    pub fn unsupported_css_selector(selector: impl Into<String>) -> Self {
        let selector = selector.into();
        Self::new(
            TakumiDiagnosticCode::UnsupportedCssSelector,
            format!(
                "CSS selector `{selector}` is outside the seq06.12 retained UI selector subset"
            ),
        )
    }

    pub fn unresolved_css_variable(name: impl Into<String>) -> Self {
        let name = name.into();
        Self::new(
            TakumiDiagnosticCode::UnresolvedCssVariable,
            format!(
                "CSS variable `{name}` is unresolved and has no fallback in the seq06.12 coverage cut"
            ),
        )
    }
}

impl TakumiAdapterError {
    pub fn css_parse(message: impl Into<String>) -> Self {
        Self::CssParseFailed {
            message: message.into(),
        }
    }

    pub fn scene_extraction(message: impl Into<String>) -> Self {
        Self::SceneExtractionFailed {
            message: message.into(),
        }
    }
}
