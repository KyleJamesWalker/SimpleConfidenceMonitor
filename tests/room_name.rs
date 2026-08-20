use simple_confidence_monitor::room::{MAX_NAME_LEN, NameError, RoomName};

#[test]
fn accepts_a_simple_name() {
    assert_eq!(RoomName::parse("keynote").unwrap().as_str(), "keynote");
}

#[test]
fn lowercases_the_name() {
    assert_eq!(RoomName::parse("KeyNote").unwrap().as_str(), "keynote");
}

#[test]
fn accepts_dash_underscore_and_digits() {
    assert_eq!(
        RoomName::parse("main_stage-2").unwrap().as_str(),
        "main_stage-2"
    );
}

#[test]
fn rejects_an_empty_name() {
    assert_eq!(RoomName::parse(""), Err(NameError::Empty));
}

#[test]
fn rejects_a_name_over_the_length_cap() {
    let long = "a".repeat(MAX_NAME_LEN + 1);
    assert_eq!(RoomName::parse(&long), Err(NameError::TooLong));
    assert!(RoomName::parse(&"a".repeat(MAX_NAME_LEN)).is_ok());
}

#[test]
fn rejects_reserved_names_in_any_case() {
    assert_eq!(RoomName::parse("api"), Err(NameError::Reserved));
    assert_eq!(RoomName::parse("Assets"), Err(NameError::Reserved));
    assert_eq!(RoomName::parse("healthz"), Err(NameError::Reserved));
}

#[test]
fn rejects_path_and_shell_characters() {
    for bad in ["../etc", "a/b", "a b", "a.json", "a%2e", "a:b", "café"] {
        assert_eq!(
            RoomName::parse(bad),
            Err(NameError::BadCharacter),
            "expected {bad} to be rejected"
        );
    }
}
