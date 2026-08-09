/// Exit outcome guard attached to scoped cleanup.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DeferOutcome {
    #[default]
    Always,
    Completed,
    Cancelled,
    Failed,
}
