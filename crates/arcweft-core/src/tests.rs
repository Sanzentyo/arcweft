macro_rules! runtime_record {
    ([$(RuntimeFieldValue { name: $name:expr, value: $value:expr, }),* $(,)?]) => {
        $crate::value::RuntimeValue::try_record(vec![$(($name, $value)),*])
            .expect("test record fields are unique")
    };
}

pub(crate) use runtime_record;

mod flow;
mod line_task_reducer;
mod pure;
mod step_stats_delta;
mod stream;
mod task;
mod value;
