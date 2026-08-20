use std::time::Duration;

use simple_confidence_monitor::discovery::{SERVICE_TYPE, advertise};

#[test]
fn an_advertised_server_is_discoverable() {
    let advertisement = advertise(18099, Some("test-stage")).expect("advertise");
    assert!(advertisement.fullname().contains("test-stage"));

    let daemon = mdns_sd::ServiceDaemon::new().expect("browser daemon");
    let receiver = daemon.browse(SERVICE_TYPE).expect("browse");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(500))
            && let mdns_sd::ServiceEvent::ServiceResolved(info) = event
            && info.get_fullname().contains("test-stage")
        {
            assert_eq!(info.get_port(), 18099);
            return;
        }
    }
    panic!("the advertised service never resolved");
}
