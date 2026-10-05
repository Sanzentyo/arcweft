//! Validation policy for the common static task semantic seal.
//! These bounds are not semantic digest inputs.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTaskPlanSealLimits {
    pub max_task_plan_rows: u32,
    pub max_executable_rows: u32,
    pub max_children_per_row: u32,
    pub max_function_roles: u32,
    pub max_request_roles: u32,
    pub max_control_effect_rows: u32,
    pub max_view_bindings: u32,
    pub max_transcript_bytes: u64,
    pub max_semantic_work: u64,
}

impl Default for RuntimeTaskPlanSealLimits {
    fn default() -> Self {
        Self {
            max_task_plan_rows: 65_536,
            max_executable_rows: 1_048_576,
            max_children_per_row: 65_536,
            max_function_roles: 65_536,
            max_request_roles: 65_536,
            max_control_effect_rows: 65_536,
            max_view_bindings: 65_536,
            max_transcript_bytes: 67_108_864,
            max_semantic_work: 4_194_304,
        }
    }
}
