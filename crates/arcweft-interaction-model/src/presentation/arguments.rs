//! Semantic argument coordinates shared by checking and executable commands.

use arcweft_id::{DeclarationName, DeclarationNameError};
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use thiserror::Error;

macro_rules! argument_roles {
    ($($role:ident = $tag:literal => $source:literal),+ $(,)?) => {
        /// Destination selected by the callable grammar, independently of the
        /// argument's source position. Metadata roles also require a key.
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum PresentationArgumentRole { $($role),+ }

        impl PresentationArgumentRole {
            pub const ALL: &'static [Self] = &[$(Self::$role),+];

            pub const fn semantic_tag(self) -> u8 {
                match self { $(Self::$role => $tag),+ }
            }

            pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
                match tag { $($tag => Some(Self::$role)),+, _ => None }
            }

            /// Fixed spelling, or the prefix for a metadata argument.
            pub const fn source_name(self) -> &'static str {
                match self { $(Self::$role => $source),+ }
            }
        }
    };
}

argument_roles! {
    View = 0 => "view",
    Asset = 1 => "asset",
    Lifetime = 2 => "lifetime",
    TargetEntity = 3 => "target",
    TargetPublicId = 4 => "target",
    LayerPublicId = 5 => "layer",
    Slot = 6 => "slot",
    Scope = 7 => "scope",
    Fade = 8 => "fade",
    Fit = 9 => "fit",
    Opacity = 10 => "opacity",
    Id = 11 => "id",
    Handle = 12 => "handle",
    Key = 13 => "key",
    Mount = 14 => "mount",
    Depth = 15 => "depth",
    Enabled = 16 => "enabled",
    Visible = 17 => "visible",
    Action = 18 => "action",
    Actions = 19 => "actions",
    X = 20 => "x",
    Y = 21 => "y",
    Width = 22 => "width",
    Height = 23 => "height",
    Focus = 24 => "focus",
    InputCapture = 25 => "input_capture",
    Owner = 26 => "owner",
    Drop = 27 => "drop",
    AlignmentX = 28 => "alignment.x",
    AlignmentY = 29 => "alignment.y",
    PlaybackStart = 30 => "playback.start",
    PlaybackPausedAt = 31 => "playback.paused_at",
    PlaybackLocalTime = 32 => "playback.local_time",
    PlaybackRate = 33 => "playback.rate",
    TransformTx = 34 => "transform.tx",
    TransformTy = 35 => "transform.ty",
    TransformM11 = 36 => "transform.m11",
    TransformM12 = 37 => "transform.m12",
    TransformM21 = 38 => "transform.m21",
    TransformM22 = 39 => "transform.m22",
    ProxyId = 40 => "proxy.id",
    ProxyType = 41 => "proxy.type",
    ProxyRole = 42 => "proxy.role",
    ProxyLayer = 43 => "proxy.layer",
    ProxyDepth = 44 => "proxy.depth",
    ProxyHitTest = 45 => "proxy.hit_test",
    Parameter = 46 => "param.",
    ProxyParameter = 47 => "proxy.param.",
    Look = 48 => "look",
    Source = 49 => "source",
    Character = 50 => "character",
    ViewValue = 51 => "view",
}

impl PresentationArgumentRole {
    pub const fn requires_parameter(self) -> bool {
        matches!(self, Self::Parameter | Self::ProxyParameter)
    }

    pub(super) fn matches_source_name(self, name: &str) -> bool {
        if self.requires_parameter() {
            name.strip_prefix(self.source_name())
                .is_some_and(|key| PresentationArgumentKey::validate_parameter(key).is_ok())
        } else {
            name == self.source_name()
        }
    }
}

/// One exact input destination. Two metadata fields of the same role retain
/// distinct keys; fixed fields cannot smuggle a metadata key into the ABI.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(try_from = "ArgumentKeyWire")]
pub struct PresentationArgumentKey {
    role: PresentationArgumentRole,
    parameter: Option<String>,
}

/// Borrowed, validated destination used before an owner allocates metadata.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PresentationArgumentKeyRef<'a> {
    role: PresentationArgumentRole,
    parameter: Option<&'a str>,
}

impl<'a> PresentationArgumentKeyRef<'a> {
    pub const fn role(self) -> PresentationArgumentRole {
        self.role
    }

    pub fn try_new(
        role: PresentationArgumentRole,
        parameter: Option<&'a str>,
    ) -> Result<Self, PresentationArgumentKeyError> {
        if role.requires_parameter() != parameter.is_some() {
            return Err(PresentationArgumentKeyError::ParameterPresence { role });
        }
        if let Some(key) = parameter {
            PresentationArgumentKey::validate_parameter(key)?;
        }
        Ok(Self { role, parameter })
    }
}

impl PresentationArgumentKey {
    pub fn try_new(
        role: PresentationArgumentRole,
        parameter: Option<String>,
    ) -> Result<Self, PresentationArgumentKeyError> {
        PresentationArgumentKeyRef::try_new(role, parameter.as_deref())?;
        Ok(Self { role, parameter })
    }

    pub fn as_borrowed(&self) -> PresentationArgumentKeyRef<'_> {
        PresentationArgumentKeyRef {
            role: self.role,
            parameter: self.parameter(),
        }
    }

    /// Checks a borrowed key before a bounded consumer copies it.
    pub fn validate_parameter(key: &str) -> Result<(), PresentationArgumentKeyError> {
        for segment in key.split('.') {
            DeclarationName::validate_source(segment)?;
        }
        Ok(())
    }

    pub const fn role(&self) -> PresentationArgumentRole {
        self.role
    }

    pub fn parameter(&self) -> Option<&str> {
        self.parameter.as_deref()
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PresentationArgumentKeyError {
    #[error("presentation argument {role:?} has an invalid metadata-key presence")]
    ParameterPresence { role: PresentationArgumentRole },
    #[error("invalid presentation metadata key: {0}")]
    Parameter(#[from] DeclarationNameError),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArgumentKeyWire {
    role: PresentationArgumentRole,
    parameter: Option<String>,
}

impl TryFrom<ArgumentKeyWire> for PresentationArgumentKey {
    type Error = PresentationArgumentKeyError;

    fn try_from(value: ArgumentKeyWire) -> Result<Self, Self::Error> {
        Self::try_new(value.role, value.parameter)
    }
}

impl Serialize for PresentationArgumentKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_borrowed().serialize(serializer)
    }
}

impl Serialize for PresentationArgumentKeyRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut wire = serializer.serialize_struct("ArgumentKeyWire", 2)?;
        wire.serialize_field("role", &self.role)?;
        wire.serialize_field("parameter", &self.parameter)?;
        wire.end()
    }
}
