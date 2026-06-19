use arcweft_desktop_contract::{
    DesktopError, ExternalWindowRequest, ExternalWindowResponse, OwnedCursorRequest,
    OwnedWindowRequest, OwnedWindowResponse,
};

/// Event-loop-owned window implementation supplied by the native player.
///
/// Arcweft invokes this trait only from the host main-thread lane. A winit,
/// SDL, Qt, or embedding-specific player can therefore keep its native window
/// handles entirely outside the runtime boundary.
pub trait OwnedWindowDriver: Send + Sync + 'static {
    fn execute_window(
        &self,
        request: OwnedWindowRequest,
    ) -> Result<OwnedWindowResponse, DesktopError>;

    fn execute_cursor(&self, request: OwnedCursorRequest) -> Result<(), DesktopError>;

    fn supports_absolute_position(&self) -> bool {
        true
    }
}

/// Optional high-authority controller for windows owned by other processes.
///
/// This is intentionally separate from observation and is never installed by
/// the standard native profile.
pub trait ExternalWindowControlDriver: Send + Sync + 'static {
    fn execute_external_window(
        &self,
        request: ExternalWindowRequest,
    ) -> Result<ExternalWindowResponse, DesktopError>;
}
