use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("progress ratio must be finite and in the range 0.0..=1.0")]
pub struct ProgressError;

#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    ratio: f32,
    label: Option<String>,
}

impl Progress {
    pub fn new(ratio: f32) -> Result<Self, ProgressError> {
        if ratio.is_finite() && (0.0..=1.0).contains(&ratio) {
            Ok(Self { ratio, label: None })
        } else {
            Err(ProgressError)
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub const fn ratio(&self) -> f32 {
        self.ratio
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Need<T, E> {
    NotStarted,
    Pending(Progress),
    Ready(T),
    Err(E),
    Cancelled,
}

impl<T, E> Need<T, E> {
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// Whether this state has committed its single terminal outcome.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Ready(_) | Self::Err(_) | Self::Cancelled)
    }

    pub fn ready(self) -> Option<T> {
        match self {
            Self::Ready(value) => Some(value),
            Self::NotStarted | Self::Pending(_) | Self::Err(_) | Self::Cancelled => None,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Need<U, E> {
        match self {
            Self::Ready(value) => Need::Ready(f(value)),
            Self::NotStarted => Need::NotStarted,
            Self::Pending(progress) => Need::Pending(progress),
            Self::Err(err) => Need::Err(err),
            Self::Cancelled => Need::Cancelled,
        }
    }

    pub fn map_err<F>(self, f: impl FnOnce(E) -> F) -> Need<T, F> {
        match self {
            Self::Ready(value) => Need::Ready(value),
            Self::NotStarted => Need::NotStarted,
            Self::Pending(progress) => Need::Pending(progress),
            Self::Err(err) => Need::Err(f(err)),
            Self::Cancelled => Need::Cancelled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Need, Progress};

    #[test]
    fn progress_rejects_out_of_range_ratio() {
        assert!(Progress::new(1.2).is_err());
        assert!(Progress::new(f32::NAN).is_err());
    }

    #[test]
    fn need_maps_ready_only() {
        let ready = Need::<u8, ()>::Ready(2).map(u16::from);
        assert_eq!(ready, Need::Ready(2_u16));

        let pending =
            Need::<u8, ()>::Pending(Progress::new(0.5).expect("valid progress")).map(u16::from);
        assert!(pending.is_pending());
    }

    #[test]
    fn terminal_need_states_are_exactly_ready_err_and_cancelled() {
        assert!(!Need::<u8, u8>::NotStarted.is_terminal());
        assert!(
            !Need::<u8, u8>::Pending(Progress::new(0.5).expect("valid progress")).is_terminal()
        );
        assert!(Need::<u8, u8>::Ready(1).is_terminal());
        assert!(Need::<u8, u8>::Err(2).is_terminal());
        assert!(Need::<u8, u8>::Cancelled.is_terminal());
    }
}
