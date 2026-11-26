#[path = "../../crates/compatibility/libdb-sys/src/tcp_clock.rs"]
mod tcp_clock;

#[test]
fn stopped_time_does_not_advance_tcp_retransmission_clock() {
    let mut clock = tcp_clock::TcpClock::default();
    clock.start(10);
    assert!(clock.is_running());
    assert_eq!(clock.elapsed(20), 10);
    clock.stop(20);
    assert!(!clock.is_running());
    assert_eq!(clock.elapsed(100), 10);
    clock.start(100);
    assert_eq!(clock.elapsed(105), 15);
}
