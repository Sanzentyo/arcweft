use arcweft_presentation::text_input::{
    TextInputCapabilities, TextInputCapabilitySupport, TextInputSecurityPolicy,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WindowsTsfFeature {
    SurroundingText,
    DeleteSurrounding,
    Reconversion,
    CompositionSegments,
    CharacterBounds,
    ProgrammaticCommit,
    ProgrammaticCancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsTsfFeatureStatus {
    Supported,
    Unsupported(&'static str),
    RuntimeUnavailable(&'static str),
    HostDependent(&'static str),
    SecureRedacted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsTsfCapabilityEntry {
    feature: WindowsTsfFeature,
    status: WindowsTsfFeatureStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsTsfRuntimeFacts {
    runtime: WindowsTsfRuntimeState,
    reconversion: WindowsTsfReconversionState,
    display_attributes: WindowsTsfDisplayAttributeState,
    layout: WindowsTsfLayoutState,
    security: TextInputSecurityPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsTsfRuntimeState {
    Ready,
    ThreadManagerInactive,
    DocumentManagerMissing,
    TextStoreNotAdvised,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsTsfReconversionState {
    FunctionAvailable,
    FunctionMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsTsfDisplayAttributeState {
    MappedWithFixtureCoverage,
    MappingMissing,
    FixtureCoverageMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsTsfLayoutState {
    Available,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsTsfCapabilityReport {
    entries: [WindowsTsfCapabilityEntry; 7],
}

impl WindowsTsfFeature {
    pub const fn code(self) -> &'static str {
        match self {
            Self::SurroundingText => "windows_tsf.surrounding_text",
            Self::DeleteSurrounding => "windows_tsf.delete_surrounding",
            Self::Reconversion => "windows_tsf.reconversion",
            Self::CompositionSegments => "windows_tsf.composition_segments",
            Self::CharacterBounds => "windows_tsf.character_bounds",
            Self::ProgrammaticCommit => "windows_tsf.programmatic_commit",
            Self::ProgrammaticCancel => "windows_tsf.programmatic_cancel",
        }
    }
}

impl WindowsTsfFeatureStatus {
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    pub const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported(code)
            | Self::RuntimeUnavailable(code)
            | Self::HostDependent(code) => code,
            Self::SecureRedacted => "secure_redacted",
        }
    }

    const fn to_common_support(self) -> TextInputCapabilitySupport {
        match self {
            Self::Supported => TextInputCapabilitySupport::Supported,
            Self::HostDependent(_) => TextInputCapabilitySupport::HostDependent,
            Self::SecureRedacted => TextInputCapabilitySupport::SecureRedacted,
            Self::RuntimeUnavailable(_) | Self::Unsupported(_) => {
                TextInputCapabilitySupport::Unsupported
            }
        }
    }
}

impl WindowsTsfCapabilityEntry {
    pub const fn new(feature: WindowsTsfFeature, status: WindowsTsfFeatureStatus) -> Self {
        Self { feature, status }
    }

    pub const fn feature(self) -> WindowsTsfFeature {
        self.feature
    }

    pub const fn status(self) -> WindowsTsfFeatureStatus {
        self.status
    }
}

impl Default for WindowsTsfRuntimeFacts {
    fn default() -> Self {
        Self {
            runtime: WindowsTsfRuntimeState::ThreadManagerInactive,
            reconversion: WindowsTsfReconversionState::FunctionMissing,
            display_attributes: WindowsTsfDisplayAttributeState::MappingMissing,
            layout: WindowsTsfLayoutState::Unavailable,
            security: TextInputSecurityPolicy::Plain,
        }
    }
}

impl WindowsTsfRuntimeFacts {
    #[must_use]
    pub const fn with_runtime_state(mut self, runtime: WindowsTsfRuntimeState) -> Self {
        self.runtime = runtime;
        self
    }

    #[must_use]
    pub const fn with_runtime_ready(self) -> Self {
        self.with_runtime_state(WindowsTsfRuntimeState::Ready)
    }

    #[must_use]
    pub const fn with_reconversion_state(
        mut self,
        reconversion: WindowsTsfReconversionState,
    ) -> Self {
        self.reconversion = reconversion;
        self
    }

    #[must_use]
    pub const fn with_reconversion_function_available(self) -> Self {
        self.with_reconversion_state(WindowsTsfReconversionState::FunctionAvailable)
    }

    #[must_use]
    pub const fn with_display_attribute_state(
        mut self,
        display_attributes: WindowsTsfDisplayAttributeState,
    ) -> Self {
        self.display_attributes = display_attributes;
        self
    }

    #[must_use]
    pub const fn with_mapped_display_attributes(self) -> Self {
        self.with_display_attribute_state(
            WindowsTsfDisplayAttributeState::MappedWithFixtureCoverage,
        )
    }

    #[must_use]
    pub const fn with_layout_state(mut self, layout: WindowsTsfLayoutState) -> Self {
        self.layout = layout;
        self
    }

    #[must_use]
    pub const fn with_layout_available(self) -> Self {
        self.with_layout_state(WindowsTsfLayoutState::Available)
    }

    #[must_use]
    pub const fn with_security(mut self, security: TextInputSecurityPolicy) -> Self {
        self.security = security;
        self
    }

    pub const fn runtime_ready(self) -> bool {
        matches!(self.runtime, WindowsTsfRuntimeState::Ready)
    }

    pub const fn secure_redacted(self) -> bool {
        matches!(self.security, TextInputSecurityPolicy::SecureRedacted)
    }

    pub const fn reconversion_function_available(self) -> bool {
        matches!(
            self.reconversion,
            WindowsTsfReconversionState::FunctionAvailable
        )
    }

    pub const fn display_attributes_ready(self) -> bool {
        matches!(
            self.display_attributes,
            WindowsTsfDisplayAttributeState::MappedWithFixtureCoverage
        )
    }

    pub const fn layout_available(self) -> bool {
        matches!(self.layout, WindowsTsfLayoutState::Available)
    }
}

impl WindowsTsfCapabilityReport {
    pub fn from_facts(facts: WindowsTsfRuntimeFacts) -> Self {
        let runtime = runtime_status(facts);
        let surrounding_text = if facts.secure_redacted() {
            WindowsTsfFeatureStatus::SecureRedacted
        } else {
            runtime
        };
        let reconversion = if facts.secure_redacted() {
            WindowsTsfFeatureStatus::SecureRedacted
        } else if !facts.runtime_ready() {
            runtime
        } else if facts.reconversion_function_available() {
            WindowsTsfFeatureStatus::Supported
        } else {
            WindowsTsfFeatureStatus::RuntimeUnavailable("tsf_reconversion_function_missing")
        };
        let composition_segments = if facts.display_attributes_ready() {
            runtime
        } else {
            WindowsTsfFeatureStatus::HostDependent("tsf_display_attributes_not_mapped")
        };
        let character_bounds = if facts.secure_redacted() {
            WindowsTsfFeatureStatus::SecureRedacted
        } else if facts.layout_available() {
            runtime
        } else {
            WindowsTsfFeatureStatus::HostDependent("tsf_layout_unavailable")
        };
        Self {
            entries: [
                WindowsTsfCapabilityEntry::new(
                    WindowsTsfFeature::SurroundingText,
                    surrounding_text,
                ),
                WindowsTsfCapabilityEntry::new(WindowsTsfFeature::DeleteSurrounding, runtime),
                WindowsTsfCapabilityEntry::new(WindowsTsfFeature::Reconversion, reconversion),
                WindowsTsfCapabilityEntry::new(
                    WindowsTsfFeature::CompositionSegments,
                    composition_segments,
                ),
                WindowsTsfCapabilityEntry::new(
                    WindowsTsfFeature::CharacterBounds,
                    character_bounds,
                ),
                WindowsTsfCapabilityEntry::new(WindowsTsfFeature::ProgrammaticCommit, runtime),
                WindowsTsfCapabilityEntry::new(WindowsTsfFeature::ProgrammaticCancel, runtime),
            ],
        }
    }

    pub fn entries(&self) -> &[WindowsTsfCapabilityEntry] {
        &self.entries
    }

    pub fn status(&self, feature: WindowsTsfFeature) -> WindowsTsfFeatureStatus {
        self.entries
            .iter()
            .find(|entry| entry.feature == feature)
            .map_or(
                WindowsTsfFeatureStatus::Unsupported("capability_not_declared"),
                |entry| entry.status,
            )
    }

    pub fn diagnostics(&self) -> Vec<WindowsTsfCapabilityEntry> {
        self.entries
            .iter()
            .copied()
            .filter(|entry| !entry.status.is_supported())
            .collect()
    }

    pub fn to_text_input_capabilities(&self) -> TextInputCapabilities {
        TextInputCapabilities {
            surrounding_text: self
                .status(WindowsTsfFeature::SurroundingText)
                .to_common_support(),
            delete_surrounding: self
                .status(WindowsTsfFeature::DeleteSurrounding)
                .to_common_support(),
            reconversion: self
                .status(WindowsTsfFeature::Reconversion)
                .to_common_support(),
            composition_segments: self
                .status(WindowsTsfFeature::CompositionSegments)
                .to_common_support(),
            character_bounds: self
                .status(WindowsTsfFeature::CharacterBounds)
                .to_common_support(),
            programmatic_commit: self
                .status(WindowsTsfFeature::ProgrammaticCommit)
                .to_common_support(),
            programmatic_cancel: self
                .status(WindowsTsfFeature::ProgrammaticCancel)
                .to_common_support(),
        }
    }
}

const fn runtime_status(facts: WindowsTsfRuntimeFacts) -> WindowsTsfFeatureStatus {
    if facts.runtime_ready() {
        WindowsTsfFeatureStatus::Supported
    } else {
        WindowsTsfFeatureStatus::RuntimeUnavailable("tsf_runtime_not_activated")
    }
}
