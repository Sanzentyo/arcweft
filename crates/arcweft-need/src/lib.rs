use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
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

impl Serialize for Progress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (self.ratio(), self.label()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Progress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (ratio, label): (f32, Option<String>) = Deserialize::deserialize(deserializer)?;
        let progress = Self::new(ratio).map_err(D::Error::custom)?;
        Ok(match label {
            Some(label) => progress.with_label(label),
            None => progress,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Need<T> {
    NotStarted,
    Pending(Progress),
    Ready(T),
    Cancelled,
}

impl<T> Need<T> {
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// Whether this state has committed its single terminal outcome.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Ready(_) | Self::Cancelled)
    }

    pub fn ready(self) -> Option<T> {
        match self {
            Self::Ready(value) => Some(value),
            Self::NotStarted | Self::Pending(_) | Self::Cancelled => None,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Need<U> {
        match self {
            Self::Ready(value) => Need::Ready(f(value)),
            Self::NotStarted => Need::NotStarted,
            Self::Pending(progress) => Need::Pending(progress),
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
        let ready = Need::<u8>::Ready(2).map(u16::from);
        assert_eq!(ready, Need::Ready(2_u16));

        let pending =
            Need::<u8>::Pending(Progress::new(0.5).expect("valid progress")).map(u16::from);
        assert!(pending.is_pending());
    }

    #[test]
    fn terminal_need_states_are_exactly_ready_and_cancelled() {
        assert!(!Need::<u8>::NotStarted.is_terminal());
        assert!(!Need::<u8>::Pending(Progress::new(0.5).expect("valid progress")).is_terminal());
        assert!(Need::<u8>::Ready(1).is_terminal());
        assert!(Need::<u8>::Cancelled.is_terminal());
    }

    #[test]
    fn progress_codec_preserves_the_validated_owner_shape() {
        let progress = Progress::new(0.5)
            .expect("fixture progress is valid")
            .with_label("loading");
        let encoded = serde_json::to_string(&progress).expect("Progress serializes");
        assert_eq!(encoded, "[0.5,\"loading\"]");
        assert_eq!(
            serde_json::from_str::<Progress>(&encoded).expect("Progress round-trips"),
            progress
        );
        assert!(serde_json::from_str::<Progress>("[1.5,null]").is_err());
    }
}
