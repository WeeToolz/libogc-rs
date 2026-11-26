#[path = "../../crates/compatibility/libdb-sys/src/tcp_clock.rs"]
mod tcp_clock;
#[path = "../../crates/compatibility/libdb-sys/src/tcp_state.rs"]
mod tcp_state;

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

#[test]
fn running_time_handles_tick_counter_wrap() {
    let mut clock = tcp_clock::TcpClock::default();
    clock.start(u64::MAX - 2);
    assert_eq!(clock.elapsed(2), 5);
}

#[test]
fn simultaneous_close_enters_time_wait() {
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::FinWait1, true, true),
        tcp_state::CloseProgress::TimeWait
    );
}

#[test]
fn handshake_and_peer_close_transitions_match_tcp_states() {
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::SynReceived, false, true),
        tcp_state::CloseProgress::State(tcp_state::TcpState::Established)
    );
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::Established, true, false),
        tcp_state::CloseProgress::State(tcp_state::TcpState::CloseWait)
    );
}

#[test]
fn crossed_fin_waits_for_local_fin_ack_before_time_wait() {
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::FinWait1, true, false),
        tcp_state::CloseProgress::State(tcp_state::TcpState::Closing)
    );
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::Closing, false, true),
        tcp_state::CloseProgress::TimeWait
    );
}

#[test]
fn local_fin_ack_enters_fin_wait_two_and_peer_fin_enters_time_wait() {
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::FinWait1, false, true),
        tcp_state::CloseProgress::State(tcp_state::TcpState::FinWait2)
    );
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::FinWait2, true, false),
        tcp_state::CloseProgress::TimeWait
    );
}

#[test]
fn last_ack_closes_after_the_peer_ack() {
    assert_eq!(
        tcp_state::advance_close_state(tcp_state::TcpState::LastAck, false, true),
        tcp_state::CloseProgress::Closed
    );
}

#[test]
fn close_state_expiry_uses_c_timeout_boundaries() {
    assert!(!tcp_state::close_state_expired(
        tcp_state::TcpState::FinWait2,
        20_000,
        0,
        1000
    ));
    assert!(tcp_state::close_state_expired(
        tcp_state::TcpState::FinWait2,
        20_001,
        0,
        1000
    ));
    assert!(!tcp_state::close_state_expired(
        tcp_state::TcpState::TimeWait,
        120_000,
        0,
        1000
    ));
    assert!(tcp_state::close_state_expired(
        tcp_state::TcpState::TimeWait,
        120_001,
        0,
        1000
    ));
    assert!(tcp_state::close_state_expired(
        tcp_state::TcpState::TimeWait,
        60_001,
        u64::MAX - 60_000,
        1000
    ));
}

#[test]
fn time_wait_receive_next_preserves_duplicates_and_consumes_new_sequence_space() {
    assert_eq!(tcp_state::time_wait_receive_next(100, 1, 101), Some(101));
    assert_eq!(tcp_state::time_wait_receive_next(101, 4, 101), Some(105));
    assert_eq!(tcp_state::time_wait_receive_next(101, 0, 101), None);
}
