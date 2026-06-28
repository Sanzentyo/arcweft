use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseConsumeVerificationReport {
    pub archive: String,
}
