use arcweft_runtime_host::{BundleRunnerExecutor, BundleRunnerOptions};

#[test]
fn bundle_runner_default_executor_is_awbc_product() {
    let options = BundleRunnerOptions::default();
    assert_eq!(options.executor, BundleRunnerExecutor::AwbcProduct);
}
