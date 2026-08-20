use simple_confidence_monitor::discovery::{SERVICE_TYPE, instance_name};

#[test]
fn the_service_type_is_a_full_mdns_name() {
    assert!(SERVICE_TYPE.starts_with('_'));
    assert!(SERVICE_TYPE.ends_with(".local."));
}

#[test]
fn the_default_name_carries_the_port() {
    assert_eq!(instance_name(None, 8080), "confidence-monitor-8080");
    assert_eq!(instance_name(Some("  "), 9000), "confidence-monitor-9000");
}

#[test]
fn a_given_name_wins() {
    assert_eq!(instance_name(Some("Main Stage"), 8080), "Main Stage");
}

#[test]
fn a_dot_becomes_a_dash() {
    assert_eq!(instance_name(Some("stage.one"), 8080), "stage-one");
}

#[test]
fn a_long_name_fits_a_dns_label() {
    let name = instance_name(Some(&"a".repeat(100)), 8080);
    assert_eq!(name.chars().count(), 63);
}
