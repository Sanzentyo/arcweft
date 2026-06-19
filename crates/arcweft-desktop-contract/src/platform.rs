use serde::{Deserialize, Serialize};

/// Host family relevant to desktop capability negotiation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformKind {
    Windows,
    MacOs,
    LinuxX11,
    LinuxWayland,
    Web,
    Other,
}

/// Permission or user-mediated authority required by a desktop feature.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    UserFileSelection,
    KnownDirectoryAccess,
    Accessibility,
    InputControl,
    ScreenRecording,
    DesktopPortal,
    HostMainThread,
}

/// Portable feature identifiers used for capability probing and diagnostics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopFeature {
    OwnedWindowObserve,
    OwnedWindowControl,
    OwnedWindowAbsolutePosition,
    OwnedCursorControl,
    UserFileDialog,
    KnownDirectoryGrant,
    GrantedFileIo,
    PersistentFileGrant,
    ExternalWindowObserve,
    ExternalWindowControl,
    GlobalPointerObserve,
    GlobalPointerControl,
}

/// How reliably one feature can be provided by the current host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Supported,
    SupportedWithUserConsent,
    BestEffort,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeatureSupport {
    pub feature: DesktopFeature,
    pub level: SupportLevel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PermissionKind>,
    pub detail: String,
}

impl FeatureSupport {
    pub fn supported(feature: DesktopFeature, detail: impl Into<String>) -> Self {
        Self {
            feature,
            level: SupportLevel::Supported,
            permissions: Vec::new(),
            detail: detail.into(),
        }
    }

    pub fn with_consent(
        feature: DesktopFeature,
        permissions: impl IntoIterator<Item = PermissionKind>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            feature,
            level: SupportLevel::SupportedWithUserConsent,
            permissions: permissions.into_iter().collect(),
            detail: detail.into(),
        }
    }

    pub fn best_effort(
        feature: DesktopFeature,
        permissions: impl IntoIterator<Item = PermissionKind>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            feature,
            level: SupportLevel::BestEffort,
            permissions: permissions.into_iter().collect(),
            detail: detail.into(),
        }
    }

    pub fn unsupported(feature: DesktopFeature, detail: impl Into<String>) -> Self {
        Self {
            feature,
            level: SupportLevel::Unsupported,
            permissions: Vec::new(),
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DesktopCapabilities {
    pub platform: PlatformKind,
    pub features: Vec<FeatureSupport>,
}

impl DesktopCapabilities {
    pub fn new(platform: PlatformKind, features: impl IntoIterator<Item = FeatureSupport>) -> Self {
        let mut features = features.into_iter().collect::<Vec<_>>();
        features.sort_by_key(|support| support.feature);
        features.dedup_by_key(|support| support.feature);
        Self { platform, features }
    }

    pub fn support(&self, feature: DesktopFeature) -> Option<&FeatureSupport> {
        self.features
            .binary_search_by_key(&feature, |support| support.feature)
            .ok()
            .map(|index| &self.features[index])
    }

    pub fn is_available(&self, feature: DesktopFeature) -> bool {
        self.support(feature)
            .is_some_and(|support| !matches!(support.level, SupportLevel::Unsupported))
    }
}
