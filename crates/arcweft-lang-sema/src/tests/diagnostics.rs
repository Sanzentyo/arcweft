use super::support::*;

#[test]
fn reports_unclosed_flow_block() {
    let errors = parse_errors("flow @flow.bad bad {");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message().contains("unclosed block"));
    assert!(!errors[0].recovery().is_empty());
}

#[test]
fn reports_invalid_entity_reference() {
    let errors = parse_errors("flow @<> bad { }");
    assert!(errors[0].message().contains("entity reference"));
}
