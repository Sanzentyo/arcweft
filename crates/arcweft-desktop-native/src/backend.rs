use crate::NativeDesktopOptions;
use crate::capabilities::{
    NativeCapabilityInputs, OptionalDriverInput, OwnedWindowCapabilityInput, native_capabilities,
};
use crate::driver::{ExternalWindowControlDriver, OwnedWindowDriver};
use crate::external::observe_external_windows;
use crate::files::execute_user_file;
use crate::grant_store::{GrantStore, PersistentGrantStore};
use crate::platform::native_platform_kind;
use crate::pointer::execute_global_pointer;
use arcweft_desktop_contract::{
    DesktopCapabilities, DesktopError, DesktopRequest, DesktopResponse, ExternalWindowRequest,
    SupportLevel, UserFileRequest,
};
use arcweft_desktop_host::{BackendSubmission, DesktopBackend, DesktopTaskId, ExecutionLane};
use std::sync::Arc;

pub struct NativeDesktopBuilder {
    platform: arcweft_desktop_contract::PlatformKind,
    options: NativeDesktopOptions,
    owned_window: Option<Arc<dyn OwnedWindowDriver>>,
    external_window_control: Option<Arc<dyn ExternalWindowControlDriver>>,
    persistent_grants: Option<Arc<dyn PersistentGrantStore>>,
}

impl Default for NativeDesktopBuilder {
    fn default() -> Self {
        Self {
            platform: native_platform_kind(),
            options: NativeDesktopOptions::default(),
            owned_window: None,
            external_window_control: None,
            persistent_grants: None,
        }
    }
}

impl NativeDesktopBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_options(mut self, options: NativeDesktopOptions) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub fn with_owned_window_driver(mut self, driver: Arc<dyn OwnedWindowDriver>) -> Self {
        self.owned_window = Some(driver);
        self
    }

    #[must_use]
    pub fn with_external_window_control_driver(
        mut self,
        driver: Arc<dyn ExternalWindowControlDriver>,
    ) -> Self {
        self.external_window_control = Some(driver);
        self
    }

    #[must_use]
    pub fn with_persistent_grant_store(mut self, store: Arc<dyn PersistentGrantStore>) -> Self {
        self.persistent_grants = Some(store);
        self
    }

    /// Overrides platform detection for contract tests and embedding hosts.
    #[must_use]
    pub const fn with_platform(mut self, platform: arcweft_desktop_contract::PlatformKind) -> Self {
        self.platform = platform;
        self
    }

    /// Build a native desktop backend.
    ///
    /// # Panics
    ///
    /// Panics if an installed persistent grant provider fails while loading
    /// restored grants. Embedding hosts that need to report that failure should
    /// call [`Self::try_build`] instead.
    pub fn build(self) -> NativeDesktopBackend {
        self.try_build()
            .expect("persistent grant store failed during native desktop backend construction")
    }

    pub fn try_build(self) -> Result<NativeDesktopBackend, DesktopError> {
        let owned_absolute_position = self
            .owned_window
            .as_ref()
            .is_some_and(|driver| driver.supports_absolute_position());
        let grants = GrantStore::new(self.platform, self.persistent_grants)?;
        let owned_window = match (self.owned_window.is_some(), owned_absolute_position) {
            (false, _) => OwnedWindowCapabilityInput::Missing,
            (true, false) => OwnedWindowCapabilityInput::RelativeOnly,
            (true, true) => OwnedWindowCapabilityInput::AbsolutePosition,
        };
        let capabilities = native_capabilities(
            self.platform,
            &self.options,
            NativeCapabilityInputs {
                owned_window,
                external_control: optional_driver(self.external_window_control.is_some()),
                persistent_grants: optional_driver(grants.has_persistent_store()),
            },
        );
        Ok(NativeDesktopBackend {
            platform: self.platform,
            options: self.options,
            capabilities,
            owned_window: self.owned_window,
            external_window_control: self.external_window_control,
            grants,
        })
    }
}

const fn optional_driver(installed: bool) -> OptionalDriverInput {
    if installed {
        OptionalDriverInput::Installed
    } else {
        OptionalDriverInput::Missing
    }
}

/// Safe native implementation shared by the logical Arcweft desktop adapters.
pub struct NativeDesktopBackend {
    platform: arcweft_desktop_contract::PlatformKind,
    options: NativeDesktopOptions,
    capabilities: DesktopCapabilities,
    owned_window: Option<Arc<dyn OwnedWindowDriver>>,
    external_window_control: Option<Arc<dyn ExternalWindowControlDriver>>,
    grants: GrantStore,
}

impl NativeDesktopBackend {
    pub fn builder() -> NativeDesktopBuilder {
        NativeDesktopBuilder::new()
    }

    pub fn capabilities(&self) -> &DesktopCapabilities {
        &self.capabilities
    }

    fn execute(&self, request: DesktopRequest) -> Result<DesktopResponse, DesktopError> {
        self.ensure_supported(&request)?;
        match request {
            DesktopRequest::Capabilities => {
                Ok(DesktopResponse::Capabilities(self.capabilities.clone()))
            }
            DesktopRequest::OwnedWindow(request) => self
                .owned_window
                .as_ref()
                .ok_or_else(|| self.unsupported_for(&DesktopRequest::OwnedWindow(request.clone())))?
                .execute_window(request)
                .map(DesktopResponse::OwnedWindow),
            DesktopRequest::OwnedCursor(request) => self
                .owned_window
                .as_ref()
                .ok_or_else(|| self.unsupported_for(&DesktopRequest::OwnedCursor(request.clone())))?
                .execute_cursor(request)
                .map(|()| DesktopResponse::OwnedCursorApplied),
            DesktopRequest::ExternalWindow(
                request @ (ExternalWindowRequest::List | ExternalWindowRequest::Get { .. }),
            ) => observe_external_windows(self.platform, request)
                .map(DesktopResponse::ExternalWindow),
            DesktopRequest::ExternalWindow(request) => self
                .external_window_control
                .as_ref()
                .ok_or_else(|| {
                    self.unsupported_for(&DesktopRequest::ExternalWindow(request.clone()))
                })?
                .execute_external_window(request)
                .map(DesktopResponse::ExternalWindow),
            DesktopRequest::GlobalPointer(request) => {
                execute_global_pointer(self.platform, self.options.global_pointer, &request)
                    .map(DesktopResponse::GlobalPointer)
            }
            DesktopRequest::UserFile(request) => {
                execute_user_file(self.platform, &self.options, &self.grants, request)
                    .map(DesktopResponse::UserFile)
            }
        }
    }

    fn ensure_supported(&self, request: &DesktopRequest) -> Result<(), DesktopError> {
        let Some(feature) = request.required_feature() else {
            return Ok(());
        };
        let Some(support) = self.capabilities.support(feature) else {
            return Err(self.unsupported_for(request));
        };
        if support.level == SupportLevel::Unsupported {
            Err(DesktopError::Unsupported {
                feature,
                platform: self.platform,
                detail: support.detail.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn unsupported_for(&self, request: &DesktopRequest) -> DesktopError {
        let feature = request
            .required_feature()
            .unwrap_or(arcweft_desktop_contract::DesktopFeature::GrantedFileIo);
        DesktopError::Unsupported {
            feature,
            platform: self.platform,
            detail: self.capabilities.support(feature).map_or_else(
                || "feature is not declared".to_owned(),
                |support| support.detail.clone(),
            ),
        }
    }
}

impl DesktopBackend for NativeDesktopBackend {
    fn execution_lane(&self, request: &DesktopRequest) -> ExecutionLane {
        match request {
            DesktopRequest::OwnedWindow(_)
            | DesktopRequest::OwnedCursor(_)
            | DesktopRequest::UserFile(UserFileRequest::ShowDialog(_))
            | DesktopRequest::ExternalWindow(
                ExternalWindowRequest::Activate { .. }
                | ExternalWindowRequest::SetBounds { .. }
                | ExternalWindowRequest::RequestClose { .. },
            ) => ExecutionLane::HostMainThread,
            DesktopRequest::Capabilities
            | DesktopRequest::ExternalWindow(
                ExternalWindowRequest::List | ExternalWindowRequest::Get { .. },
            )
            | DesktopRequest::GlobalPointer(_)
            | DesktopRequest::UserFile(_) => ExecutionLane::AnyThread,
        }
    }

    fn submit(&self, _task: DesktopTaskId, request: DesktopRequest) -> BackendSubmission {
        BackendSubmission::Completed(self.execute(request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GlobalPointerPolicy;
    use crate::{PersistentGrantRecord, PersistentGrantStore};
    use arcweft_desktop_contract::{DesktopFeature, KnownDirectory, PlatformKind, SupportLevel};

    #[derive(Default)]
    struct EmptyPersistentGrantStore;

    impl PersistentGrantStore for EmptyPersistentGrantStore {
        fn load(&self) -> Result<Vec<PersistentGrantRecord>, DesktopError> {
            Ok(Vec::new())
        }

        fn persist(&self, _record: PersistentGrantRecord) -> Result<(), DesktopError> {
            Ok(())
        }

        fn revoke(&self, _id: &arcweft_desktop_contract::FileGrantId) -> Result<(), DesktopError> {
            Ok(())
        }
    }

    #[test]
    fn high_authority_features_are_disabled_by_default() {
        let backend = NativeDesktopBuilder::new()
            .with_platform(PlatformKind::Windows)
            .build();
        assert_eq!(
            backend
                .capabilities()
                .support(DesktopFeature::GlobalPointerControl)
                .map(|support| support.level),
            Some(SupportLevel::Unsupported)
        );
    }

    #[test]
    fn options_are_reflected_in_capability_negotiation() {
        let options = NativeDesktopOptions::default()
            .allow_known_directory(KnownDirectory::Documents)
            .with_global_pointer(GlobalPointerPolicy::Observe);
        let backend = NativeDesktopBuilder::new()
            .with_platform(PlatformKind::Windows)
            .with_options(options)
            .build();
        assert!(
            backend
                .capabilities()
                .is_available(DesktopFeature::KnownDirectoryGrant)
        );
        assert!(
            backend
                .capabilities()
                .is_available(DesktopFeature::GlobalPointerObserve)
                == cfg!(feature = "global-pointer")
        );
    }

    #[test]
    fn persistent_grant_store_enables_persistent_capability() {
        let backend = NativeDesktopBuilder::new()
            .with_platform(PlatformKind::Windows)
            .with_persistent_grant_store(Arc::new(EmptyPersistentGrantStore))
            .build();
        assert_eq!(
            backend
                .capabilities()
                .support(DesktopFeature::PersistentFileGrant)
                .map(|support| support.level),
            Some(SupportLevel::SupportedWithUserConsent)
        );
    }
}
