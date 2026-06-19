use crate::GlobalPointerPolicy;
use arcweft_desktop_contract::{
    DesktopError, DesktopFeature, GlobalPointerRequest, GlobalPointerResponse, PlatformKind,
};

#[cfg(feature = "global-pointer")]
pub(crate) fn execute_global_pointer(
    platform: PlatformKind,
    policy: GlobalPointerPolicy,
    request: &GlobalPointerRequest,
) -> Result<GlobalPointerResponse, DesktopError> {
    use arcweft_desktop_contract::{
        PermissionKind, PhysicalPosition, PointerCoordinateSpace, PointerPosition,
    };
    use enigo::{Coordinate, Enigo, Mouse, Settings};

    if matches!(
        platform,
        PlatformKind::LinuxWayland | PlatformKind::Web | PlatformKind::Other
    ) {
        return Err(unsupported(platform, request));
    }
    match request {
        GlobalPointerRequest::Position if !policy.allows_observe() => {
            return Err(unsupported(platform, request));
        }
        GlobalPointerRequest::Move { .. } if !policy.allows_control() => {
            return Err(DesktopError::PermissionDenied {
                permission: PermissionKind::InputControl,
                detail: "host policy does not permit global pointer movement".to_owned(),
            });
        }
        GlobalPointerRequest::Position | GlobalPointerRequest::Move { .. } => {}
    }

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|error| DesktopError::BackendUnavailable {
            backend: "enigo".to_owned(),
            detail: error.to_string(),
        })?;
    match request {
        GlobalPointerRequest::Position => {
            let (x, y) = enigo.location().map_err(|error| DesktopError::Platform {
                operation: "global_pointer_position".to_owned(),
                code: None,
                detail: error.to_string(),
            })?;
            Ok(GlobalPointerResponse::Position(PointerPosition {
                position: PhysicalPosition { x, y },
                space: PointerCoordinateSpace::GlobalPhysical,
            }))
        }
        GlobalPointerRequest::Move { position } => enigo
            .move_mouse(position.x, position.y, Coordinate::Abs)
            .map(|()| GlobalPointerResponse::Applied)
            .map_err(|error| DesktopError::Platform {
                operation: "global_pointer_move".to_owned(),
                code: None,
                detail: error.to_string(),
            }),
    }
}

#[cfg(not(feature = "global-pointer"))]
pub(crate) fn execute_global_pointer(
    platform: PlatformKind,
    _policy: GlobalPointerPolicy,
    request: &GlobalPointerRequest,
) -> Result<GlobalPointerResponse, DesktopError> {
    Err(unsupported(platform, request))
}

fn unsupported(platform: PlatformKind, request: &GlobalPointerRequest) -> DesktopError {
    DesktopError::Unsupported {
        feature: match request {
            GlobalPointerRequest::Position => DesktopFeature::GlobalPointerObserve,
            GlobalPointerRequest::Move { .. } => DesktopFeature::GlobalPointerControl,
        },
        platform,
        detail: "global pointer access is disabled or unavailable".to_owned(),
    }
}
