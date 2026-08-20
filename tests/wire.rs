use simple_confidence_monitor::room::Command;
use simple_confidence_monitor::wire::{ClientMsg, parse_client_msg};

#[test]
fn reads_a_ping() {
    assert_eq!(
        parse_client_msg(r#"{"cmd":"ping","client_time_ms":42}"#).unwrap(),
        ClientMsg::Ping { client_time_ms: 42 }
    );
}

#[test]
fn reads_a_transport_command() {
    assert_eq!(
        parse_client_msg(r#"{"cmd":"start"}"#).unwrap(),
        ClientMsg::Cmd(Command::Start)
    );
}

#[test]
fn reads_a_command_with_a_value() {
    assert_eq!(
        parse_client_msg(r#"{"cmd":"adjust","ms":-30000}"#).unwrap(),
        ClientMsg::Cmd(Command::Adjust { ms: -30_000 })
    );
}

#[test]
fn reports_an_unknown_command_by_name() {
    let err = parse_client_msg(r#"{"cmd":"explode"}"#).unwrap_err();
    assert!(err.contains("explode"), "got {err}");
}

#[test]
fn rejects_a_ping_without_a_client_time() {
    assert!(parse_client_msg(r#"{"cmd":"ping"}"#).is_err());
}

#[test]
fn rejects_a_frame_with_no_command() {
    assert!(parse_client_msg(r#"{"ms":1}"#).is_err());
}

#[test]
fn rejects_text_that_is_not_json() {
    assert!(parse_client_msg("start").is_err());
}
