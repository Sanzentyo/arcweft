use std::fmt;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum CliProgressStatus {
    Building,
    Checking,
    Compiling,
    Encoding,
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
            eprintln!("{:>12} {message}", status.as_str());
        }
        let started = Instant::now();
        let result = run();
        if self.enabled {
            let final_status = if result.is_ok() { "Finished" } else { "Failed" };
            eprintln!(
                "{final_status:>12} {message} in {}",
                format_elapsed(started.elapsed())
            );
        }
        result
    }
}

impl CliProgressStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Building => "Building",
            Self::Checking => "Checking",
            Self::Compiling => "Compiling",
            Self::Encoding => "Encoding",
            Self::Verifying => "Verifying",
            Self::Writing => "Writing",
        }
    }
}

fn format_elapsed(elapsed: Duration) -> String {
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
    use super::format_elapsed;
    use std::time::Duration;

    #[test]
    fn progress_duration_format_is_compact() {
        assert_eq!(format_elapsed(Duration::from_nanos(1)), "<1ms");
        assert_eq!(format_elapsed(Duration::from_millis(42)), "42ms");
        assert_eq!(format_elapsed(Duration::from_millis(1_250)), "1.25s");
    }
}
