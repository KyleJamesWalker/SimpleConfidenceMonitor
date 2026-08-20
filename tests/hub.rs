use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::room::RoomName;

#[test]
fn starts_with_no_rooms() {
    assert_eq!(Hub::new().room_count(), 0);
}

#[test]
fn creates_a_room_on_first_reference() {
    let hub = Hub::new();
    hub.get_or_create(&RoomName::parse("keynote").unwrap());
    assert_eq!(hub.room_count(), 1);
}

#[test]
fn returns_the_same_room_for_the_same_name() {
    let hub = Hub::new();
    let name = RoomName::parse("keynote").unwrap();
    let first = hub.get_or_create(&name);
    let second = hub.get_or_create(&name);
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(hub.room_count(), 1);
}

#[test]
fn keeps_separate_rooms_apart() {
    let hub = Hub::new();
    hub.get_or_create(&RoomName::parse("one").unwrap());
    hub.get_or_create(&RoomName::parse("two").unwrap());
    assert_eq!(hub.room_count(), 2);
}
