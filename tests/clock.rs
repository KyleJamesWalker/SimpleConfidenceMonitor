use simple_confidence_monitor::clock::now_ms;

#[test]
fn reports_a_plausible_epoch_time() {
    // Later than 2023-11-14, earlier than 2100.
    let now = now_ms();
    assert!(now > 1_700_000_000_000, "got {now}");
    assert!(now < 4_102_444_800_000, "got {now}");
}

#[test]
fn does_not_move_backwards() {
    let first = now_ms();
    let second = now_ms();
    assert!(second >= first);
}
