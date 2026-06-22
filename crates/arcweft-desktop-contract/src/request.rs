use crate::GrantLifetime;
use crate::{
    DesktopCapabilities, DesktopFeature, ExternalWindowRequest, ExternalWindowResponse,
    GlobalPointerRequest, GlobalPointerResponse, OwnedCursorRequest, OwnedWindowRequest,
    OwnedWindowResponse, UserFileRequest, UserFileResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "domain", content = "request")]
pub enum DesktopRequest {
    Capabilities,
    OwnedWindow(OwnedWindowRequest),
    ExternalWindow(ExternalWindowRequest),
    OwnedCursor(OwnedCursorRequest),
    GlobalPointer(GlobalPointerRequest),
    UserFile(UserFileRequest),
}

impl DesktopRequest {
    pub const fn required_feature(&self) -> Option<DesktopFeature> {
        match self {
            Self::Capabilities => None,
            Self::OwnedWindow(OwnedWindowRequest::List | OwnedWindowRequest::Get { .. }) => {
                Some(DesktopFeature::OwnedWindowObserve)
            }
            Self::OwnedWindow(_) => Some(DesktopFeature::OwnedWindowControl),
            Self::ExternalWindow(
                ExternalWindowRequest::List | ExternalWindowRequest::Get { .. },
            ) => Some(DesktopFeature::ExternalWindowObserve),
            Self::ExternalWindow(_) => Some(DesktopFeature::ExternalWindowControl),
            Self::OwnedCursor(_) => Some(DesktopFeature::OwnedCursorControl),
            Self::GlobalPointer(GlobalPointerRequest::Position) => {
                Some(DesktopFeature::GlobalPointerObserve)
            }
            Self::GlobalPointer(GlobalPointerRequest::Move { .. }) => {
                Some(DesktopFeature::GlobalPointerControl)
            }
            Self::UserFile(UserFileRequest::ShowDialog(_)) => Some(DesktopFeature::UserFileDialog),
            Self::UserFile(UserFileRequest::GrantKnownDirectory { .. }) => {
                Some(DesktopFeature::KnownDirectoryGrant)
            }
            Self::UserFile(_) => Some(DesktopFeature::GrantedFileIo),
        }
    }

    pub const fn supplemental_feature(&self) -> Option<DesktopFeature> {
        match self {
            Self::UserFile(request) => match request.requested_grant_lifetime() {
                Some(GrantLifetime::Persistent) => Some(DesktopFeature::PersistentFileGrant),
                Some(GrantLifetime::Session) | None => None,
            },
            Self::Capabilities
            | Self::OwnedWindow(_)
            | Self::ExternalWindow(_)
            | Self::OwnedCursor(_)
            | Self::GlobalPointer(_) => None,
        }
    }

    pub const fn required_features(&self) -> [Option<DesktopFeature>; 2] {
        [self.required_feature(), self.supplemental_feature()]
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "domain", content = "response")]
pub enum DesktopResponse {
    Capabilities(DesktopCapabilities),
    OwnedWindow(OwnedWindowResponse),
    ExternalWindow(ExternalWindowResponse),
    OwnedCursorApplied,
    GlobalPointer(GlobalPointerResponse),
    UserFile(UserFileResponse),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileDialogMode, FileDialogRequest, GrantAccess, UserFileRequest};

    #[test]
    fn persistent_dialog_requires_primary_and_supplemental_features() {
        let request = DesktopRequest::UserFile(UserFileRequest::ShowDialog(FileDialogRequest {
            mode: FileDialogMode::OpenFile,
            title: "Open".to_owned(),
            suggested_name: None,
            filters: Vec::new(),
            access: GrantAccess::Read,
            lifetime: GrantLifetime::Persistent,
        }));
        assert_eq!(
            request.required_features(),
            [
                Some(DesktopFeature::UserFileDialog),
                Some(DesktopFeature::PersistentFileGrant),
            ]
        );
    }
}
