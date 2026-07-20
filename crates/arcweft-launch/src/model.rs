use serde::{Deserialize, Serialize};

/// Pure helper execution backend selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchPureBackend {
    Auto,
    Vm,
    Aot,
    Jit,
}

/// Matrix/tensor backend selected by a launch profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchMathBackend {
    Auto,
    Scalar,
    Glam,
    Ndarray,
    Wgpu,
}

/// Player viewport fit selected by a launch profile.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchPlayerViewportFit {
    /// Use the host surface coordinates directly.
    Raw,
    /// Preserve aspect ratio and fit the whole design viewport.
    #[default]
    Contain,
    /// Preserve aspect ratio and fill the host surface.
    Cover,
    /// Scale width and height independently to the host surface.
    Stretch,
}

/// Policy for selecting one profile ID from an accepted manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchProfileSelection<'a> {
    /// Select exactly the requested ID without fallback.
    Explicit(&'a str),
    /// Apply manifest-default, previous-profile, then lexical-first precedence.
    Automatic { previous: Option<&'a str> },
}
