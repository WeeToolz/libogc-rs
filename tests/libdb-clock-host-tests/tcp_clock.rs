#[path = "../../crates/compatibility/libdb-sys/src/tcp_clock.rs"]
mod tcp_clock;
extern crate alloc;

#[path = "../../crates/compatibility/libdb-sys/src/gdb_packet.rs"]
mod gdb_packet;
#[path = "../../crates/compatibility/libdb-sys/src/tcp_accept.rs"]
mod tcp_accept;
#[path = "../../crates/compatibility/libdb-sys/src/tcp_poll.rs"]
mod tcp_poll;
#[path = "../../crates/compatibility/libdb-sys/src/tcp_state.rs"]
mod tcp_state;
use std::collections::VecDeque;

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
    assert!(!tcp_state::state_timeout_expired(
        tcp_state::TcpState::FinWait2,
        20_000,
        0,
        1000
    ));
    assert!(tcp_state::state_timeout_expired(
        tcp_state::TcpState::FinWait2,
        20_001,
        0,
        1000
    ));
    assert!(!tcp_state::state_timeout_expired(
        tcp_state::TcpState::TimeWait,
        120_000,
        0,
        1000
    ));
    assert!(tcp_state::state_timeout_expired(
        tcp_state::TcpState::TimeWait,
        120_001,
        0,
        1000
    ));
    assert!(tcp_state::state_timeout_expired(
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

#[test]
fn completed_tcp_connections_are_accepted_in_fifo_order() {
    let mut accepted = tcp_accept::AcceptedConnections::default();
    accepted.enqueue(17);
    accepted.enqueue(23);
    accepted.enqueue(41);

    assert_eq!(accepted.dequeue(), Some(17));
    assert_eq!(accepted.dequeue(), Some(23));
    assert_eq!(accepted.dequeue(), Some(41));
    assert_eq!(accepted.dequeue(), None);
}

#[test]
fn closing_a_queued_connection_does_not_shift_other_accepts() {
    let mut accepted = tcp_accept::AcceptedConnections::default();
    accepted.enqueue(17);
    accepted.enqueue(23);
    accepted.enqueue(41);
    accepted.remove(23);

    assert_eq!(accepted.dequeue(), Some(17));
    assert_eq!(accepted.dequeue(), Some(41));
    assert_eq!(accepted.dequeue(), None);
}

#[test]
fn tcp_connection_limit_matches_four_c_control_blocks() {
    assert_eq!(tcp_accept::TCP_CONTROL_BLOCK_LIMIT, 4);
    assert!(tcp_accept::has_control_block_capacity(0));
    assert!(tcp_accept::has_control_block_capacity(3));
    assert!(!tcp_accept::has_control_block_capacity(4));
}

#[derive(Default)]
struct PollPipeline {
    frames: VecDeque<usize>,
    events: Vec<&'static str>,
}

#[test]
fn tcp_poll_processes_all_queued_frames_before_timers() {
    let mut pipeline = PollPipeline {
        frames: [1, 2].into(),
        events: Vec::new(),
    };
    let result = tcp_poll::poll_frames_before_timers(
        &mut pipeline,
        |pipeline| {
            pipeline.events.push("poll");
            Ok::<_, ()>(pipeline.frames.pop_front())
        },
        |pipeline, _frame| {
            pipeline.events.push("process");
            Ok(())
        },
        |pipeline| {
            pipeline.events.push("timers");
            Ok(())
        },
    );

    assert_eq!(result, Ok(()));
    assert_eq!(
        pipeline.events,
        ["poll", "process", "poll", "process", "poll", "timers"]
    );
}

#[test]
fn tcp_poll_runs_timers_after_a_frame_error_and_keeps_draining() {
    let mut pipeline = PollPipeline {
        frames: [1, 2].into(),
        events: Vec::new(),
    };
    let result = tcp_poll::poll_frames_before_timers(
        &mut pipeline,
        |pipeline| Ok::<_, u8>(pipeline.frames.pop_front()),
        |pipeline, frame| {
            pipeline.events.push("process");
            if frame == 1 { Err(7) } else { Ok(()) }
        },
        |pipeline| {
            pipeline.events.push("timers");
            Ok(())
        },
    );

    assert_eq!(result, Err(7));
    assert_eq!(pipeline.events, ["process", "process", "timers"]);
}
