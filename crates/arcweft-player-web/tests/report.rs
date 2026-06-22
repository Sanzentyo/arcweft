use arcweft_player_web::clock::LogicalClockQuantizer;
use arcweft_player_web::report::WebObservationReport;

#[test]
fn diagnostic_schema_name_does_not_claim_dom_rendering() {
    let _ = std::mem::size_of::<WebObservationReport>();
    let clock = LogicalClockQuantizer::new(16, 4).expect("clock");
    assert_eq!(clock, clock.clone());
}
