//! Stable standard registration providers shared by semantic and runtime data.

use serde::{Deserialize, Serialize};

/// Identity of a standard callable environment, independent of an authored path.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum StandardEnvironmentId {
    Core,
    SansIo,
    NativeHttp,
    InferenceTensor,
    SystemInfo,
    NativeFile,
    Math,
}

impl StandardEnvironmentId {
    pub const ALL: &[Self] = &[
        Self::Core,
        Self::SansIo,
        Self::NativeHttp,
        Self::InferenceTensor,
        Self::SystemInfo,
        Self::NativeFile,
        Self::Math,
    ];

    /// Version-one provider code used by canonical identity and wire encoders.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Core => 0,
            Self::SansIo => 1,
            Self::NativeHttp => 2,
            Self::InferenceTensor => 3,
            Self::SystemInfo => 4,
            Self::NativeFile => 5,
            Self::Math => 6,
        }
    }

    pub const fn from_canonical_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Core),
            1 => Some(Self::SansIo),
            2 => Some(Self::NativeHttp),
            3 => Some(Self::InferenceTensor),
            4 => Some(Self::SystemInfo),
            5 => Some(Self::NativeFile),
            6 => Some(Self::Math),
            _ => None,
        }
    }
}
