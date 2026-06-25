use arcweft_runtime_host::BundleRunnerExecutor;

#[test]
fn native_player_metadata_can_report_awbc_product_executor() {
    assert_eq!(
        format!("{:?}", BundleRunnerExecutor::AwbcProduct),
        "AwbcProduct"
    );
}
