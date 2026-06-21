use std::ffi::OsStr;
use std::io;
use std::process::{Command, ExitStatus, Output};

pub fn arcw_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arcw"))
}

#[derive(Debug)]
pub struct CommandOutput {
    output: Output,
}

impl CommandOutput {
    pub fn run<I, S>(args: I) -> io::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        arcw_command()
            .args(args)
            .output()
            .map(|output| Self { output })
    }

    #[must_use]
    pub fn status(&self) -> &ExitStatus {
        &self.output.status
    }

    #[must_use]
    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    #[must_use]
    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }

    pub fn assert_success(&self) {
        assert!(
            self.status().success(),
            "command failed with {}\nstdout:\n{}\nstderr:\n{}",
            self.status(),
            self.stdout(),
            self.stderr()
        );
    }

    pub fn assert_failure(&self) {
        assert!(
            !self.status().success(),
            "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            self.stdout(),
            self.stderr()
        );
    }
}
