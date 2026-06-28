//! macOS AppKit helper-process bridge for live `NSTextInputClient` validation.
//!
//! This module deliberately keeps AppKit object identity out of Rust data.  The
//! Swift helper owns `NSApplication`, `NSWindow`, `NSView`, `NSTextInputContext`,
//! `NSRange`, and attributed strings. Rust receives JSON-lines callback facts,
//! resolves them through `macos_text_input`, and sends only redacted, serializable
//! state back to the helper.

use super::macos_text_input::{MacosAppKitRect, MacosNativeRange};
use arcweft_presentation::text_input::TextInputOptions;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacosAppKitBridgeMode {
    TextField,
    TextArea,
    SecureField,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacosAppKitBridgeSpawnOptions {
    mode: MacosAppKitBridgeMode,
    title: String,
}

#[derive(Debug)]
pub struct MacosAppKitBridge {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MacosAppKitBridgeState {
    pub session: u64,
    pub revision: u64,
    pub mode: String,
    pub display_text: String,
    pub selected_range: MacosAppKitWireRange,
    pub marked_range: MacosAppKitWireRange,
    pub has_marked_text: bool,
    pub first_rect: MacosAppKitWireRect,
    pub actual_range: MacosAppKitWireRange,
    pub character_bounds: Vec<MacosAppKitWireCharacterBounds>,
    pub secure: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct MacosAppKitWireRange {
    pub location: u64,
    pub length: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct MacosAppKitWireCharacterBounds {
    pub range: MacosAppKitWireRange,
    pub rect: MacosAppKitWireRect,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct MacosAppKitWireRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum MacosAppKitBridgeEvent {
    Ready {
        screen_height_points: f64,
        view_origin_x: f64,
        view_origin_y: f64,
    },
    Focus,
    Blur,
    GeometryRefresh {
        screen_height_points: f64,
        view_origin_x: f64,
        view_origin_y: f64,
    },
    SetMarkedText {
        text: String,
        selected_range: MacosAppKitWireRange,
        replacement_range: MacosAppKitWireRange,
    },
    InsertText {
        text: String,
        replacement_range: MacosAppKitWireRange,
    },
    UnmarkText,
    Command {
        selector: String,
    },
    BridgeError {
        message: String,
    },
    Exit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacosAppKitBridgeError {
    HelperUnavailable,
    Io(String),
    Json(String),
    HelperExited,
}

impl Default for MacosAppKitBridgeSpawnOptions {
    fn default() -> Self {
        Self {
            mode: MacosAppKitBridgeMode::TextField,
            title: "Arcweft macOS IME Sample".to_owned(),
        }
    }
}

impl MacosAppKitBridgeSpawnOptions {
    pub fn new(mode: MacosAppKitBridgeMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub const fn mode(&self) -> MacosAppKitBridgeMode {
        self.mode
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

impl MacosAppKitBridgeMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TextField => "text-field",
            Self::TextArea => "text-area",
            Self::SecureField => "secure-field",
        }
    }

    pub fn options(self) -> TextInputOptions {
        match self {
            Self::TextField => TextInputOptions::default(),
            Self::TextArea => TextInputOptions::default().multiline(true),
            Self::SecureField => TextInputOptions::default().secure(true),
        }
    }

    pub const fn is_secure(self) -> bool {
        matches!(self, Self::SecureField)
    }
}

impl MacosAppKitBridge {
    pub fn spawn(options: &MacosAppKitBridgeSpawnOptions) -> Result<Self, MacosAppKitBridgeError> {
        let helper = helper_path().ok_or(MacosAppKitBridgeError::HelperUnavailable)?;
        Self::spawn_path(helper, options)
    }

    pub fn spawn_path(
        helper: impl AsRef<Path>,
        options: &MacosAppKitBridgeSpawnOptions,
    ) -> Result<Self, MacosAppKitBridgeError> {
        let mut child = Command::new(helper.as_ref())
            .arg("--mode")
            .arg(options.mode().as_str())
            .arg("--title")
            .arg(options.title())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| MacosAppKitBridgeError::Io(error.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or(MacosAppKitBridgeError::HelperExited)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(MacosAppKitBridgeError::HelperExited)?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub fn read_event(&mut self) -> Result<Option<MacosAppKitBridgeEvent>, MacosAppKitBridgeError> {
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .map_err(|error| MacosAppKitBridgeError::Io(error.to_string()))?;
        if read == 0 {
            return Ok(None);
        }
        let event = serde_json::from_str(line.trim_end())
            .map_err(|error| MacosAppKitBridgeError::Json(error.to_string()))?;
        Ok(Some(event))
    }

    pub fn write_state(
        &mut self,
        state: &MacosAppKitBridgeState,
    ) -> Result<(), MacosAppKitBridgeError> {
        serde_json::to_writer(&mut self.stdin, state)
            .map_err(|error| MacosAppKitBridgeError::Json(error.to_string()))?;
        self.stdin
            .write_all(b"\n")
            .and_then(|()| self.stdin.flush())
            .map_err(|error| MacosAppKitBridgeError::Io(error.to_string()))
    }

    pub fn wait(mut self) -> Result<(), MacosAppKitBridgeError> {
        let status = self
            .child
            .wait()
            .map_err(|error| MacosAppKitBridgeError::Io(error.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(MacosAppKitBridgeError::HelperExited)
        }
    }
}

impl Drop for MacosAppKitBridge {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
    }
}

impl MacosAppKitWireRange {
    pub const fn not_found() -> Self {
        Self {
            location: u64::MAX,
            length: 0,
        }
    }

    pub const fn native(self) -> MacosNativeRange {
        if self.location == u64::MAX {
            MacosNativeRange::not_found()
        } else {
            MacosNativeRange::new(self.location, self.length)
        }
    }
}

impl From<MacosNativeRange> for MacosAppKitWireRange {
    fn from(range: MacosNativeRange) -> Self {
        Self {
            location: range.location(),
            length: range.length(),
        }
    }
}

impl From<MacosAppKitRect> for MacosAppKitWireRect {
    fn from(rect: MacosAppKitRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

impl fmt::Display for MacosAppKitBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelperUnavailable => write!(
                f,
                "macOS AppKit text-input helper was not built; enable feature macos-appkit-text-input on macOS"
            ),
            Self::Io(error) => write!(f, "macOS AppKit bridge I/O failed: {error}"),
            Self::Json(error) => write!(f, "macOS AppKit bridge JSON failed: {error}"),
            Self::HelperExited => write!(f, "macOS AppKit bridge helper exited"),
        }
    }
}

impl std::error::Error for MacosAppKitBridgeError {}

fn helper_path() -> Option<PathBuf> {
    option_env!("ARCWEFT_MACOS_TEXT_INPUT_BRIDGE").map(PathBuf::from)
}
