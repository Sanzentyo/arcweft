use anstyle::{AnsiColor, Style};
use std::fmt;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const CARGO_STATUS_STYLE: Style = AnsiColor::Green.on_default().bold();
const CARGO_FAILURE_STATUS_STYLE: Style = AnsiColor::Red.on_default().bold();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum CliProgressStatus {
    Building,
    Checking,
    Compiling,
    Encoding,
    Running,
    Verifying,
    Writing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct CliProgress {
    enabled: bool,
}

impl CliProgress {
    pub(in crate::app) const fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub(in crate::app) fn run<T>(
        self,
        status: CliProgressStatus,
        message: impl fmt::Display,
        run: impl FnOnce() -> Result<T, ExitCode>,
    ) -> Result<T, ExitCode> {
        let message = message.to_string();
        if self.enabled {
            emit_progress_line(status.as_str(), CARGO_STATUS_STYLE, &message);
        }
        let started = Instant::now();
        let result = run();
        if self.enabled {
            let (final_status, style) = if result.is_ok() {
                ("Finished", CARGO_STATUS_STYLE)
            } else {
                ("Failed", CARGO_FAILURE_STATUS_STYLE)
            };
            emit_progress_line(
                final_status,
                style,
                format_args!("{message} in {}", format_elapsed(started.elapsed())),
            );
        }
        result
    }

    pub(in crate::app) fn emit_status(self, status: &'static str, message: impl fmt::Display) {
        if self.enabled {
            emit_progress_line(status, CARGO_STATUS_STYLE, message);
        }
    }
}

fn emit_progress_line(status: &'static str, style: Style, message: impl fmt::Display) {
    anstream::eprintln!("{} {message}", ProgressStatusLabel { status, style });
}

struct ProgressStatusLabel {
    status: &'static str,
    style: Style,
}

impl fmt::Display for ProgressStatusLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{:>12}{}",
            self.style,
            self.status,
            self.style.render_reset()
        )
    }
}

impl CliProgressStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Building => "Building",
            Self::Checking => "Checking",
            Self::Compiling => "Compiling",
            Self::Encoding => "Encoding",
            Self::Running => "Running",
            Self::Verifying => "Verifying",
            Self::Writing => "Writing",
        }
    }
}

pub(in crate::app) fn format_elapsed(elapsed: Duration) -> String {
    let millis = elapsed.as_millis();
    if millis == 0 {
        "<1ms".to_owned()
    } else if millis < 1_000 {
        format!("{millis}ms")
    } else {
        format!("{:.2}s", elapsed.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CARGO_FAILURE_STATUS_STYLE, CARGO_STATUS_STYLE, ProgressStatusLabel, format_elapsed,
    };
    use std::time::Duration;

    #[test]
    fn progress_duration_format_is_compact() {
        assert_eq!(format_elapsed(Duration::from_nanos(1)), "<1ms");
        assert_eq!(format_elapsed(Duration::from_millis(42)), "42ms");
        assert_eq!(format_elapsed(Duration::from_millis(1_250)), "1.25s");
    }

    #[test]
    fn progress_status_label_uses_cargo_style() {
        let compiling = ProgressStatusLabel {
            status: "Compiling",
            style: CARGO_STATUS_STYLE,
        };
        let failed = ProgressStatusLabel {
            status: "Failed",
            style: CARGO_FAILURE_STATUS_STYLE,
        };

        assert_eq!(
            compiling.to_string(),
            "\u{1b}[1m\u{1b}[32m   Compiling\u{1b}[0m"
        );
        assert_eq!(
            failed.to_string(),
            "\u{1b}[1m\u{1b}[31m      Failed\u{1b}[0m"
        );
    }
}
