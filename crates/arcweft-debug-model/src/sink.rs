use crate::event::DebugEvent;

/// Sink boundary used by the runner without depending on `SQLite`.
pub trait DebugEventSink {
    type Error: std::error::Error + Send + Sync + 'static;

    fn append(&mut self, event: &DebugEvent) -> Result<(), Self::Error>;

    fn flush(&mut self) -> Result<(), Self::Error>;
}

/// Sink used when recording is disabled.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullDebugEventSink;

impl DebugEventSink for NullDebugEventSink {
    type Error = std::convert::Infallible;

    fn append(&mut self, _event: &DebugEvent) -> Result<(), Self::Error> {
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
