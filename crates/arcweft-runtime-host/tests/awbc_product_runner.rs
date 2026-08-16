use arcweft_runtime_host::BundleRunnerOptions;

#[test]
fn bundle_runner_defaults_are_product_safe() {
    let options = BundleRunnerOptions::default();
    assert_eq!(options.steps, 8);
    assert_eq!(options.max_ops, 32);
}
