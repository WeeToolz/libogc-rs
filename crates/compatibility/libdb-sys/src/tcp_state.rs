#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum TcpState {
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    Closing,
    CloseWait,
    LastAck,
    TimeWait,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum CloseProgress {
    Unchanged,
    State(TcpState),
    TimeWait,
    Closed,
}

pub(crate) const FIN_WAIT_2_TIMEOUT_MS: u64 = 20_000;
pub(crate) const SYN_RECEIVED_TIMEOUT_MS: u64 = 20_000;
pub(crate) const TIME_WAIT_TIMEOUT_MS: u64 = 120_000;

pub(crate) fn time_wait_receive_next(seq: u32, tcp_len: u32, rcv_nxt: u32) -> Option<u32> {
    if tcp_len == 0 {
        return None;
    }
    let seq_end = seq.wrapping_add(tcp_len);
    Some(if seq_after(seq_end, rcv_nxt) {
        seq_end
    } else {
        rcv_nxt
    })
}

pub(crate) fn seq_before(left: u32, right: u32) -> bool {
    (left.wrapping_sub(right) as i32) < 0
}

pub(crate) fn seq_after(left: u32, right: u32) -> bool {
    seq_before(right, left)
}

pub(crate) fn advance_close_state(
    state: TcpState,
    peer_fin_received: bool,
    local_fin_acked: bool,
) -> CloseProgress {
    match (state, peer_fin_received, local_fin_acked) {
        (TcpState::SynReceived, _, true) => CloseProgress::State(TcpState::Established),
        (TcpState::Established, true, _) => CloseProgress::State(TcpState::CloseWait),
        (TcpState::FinWait1, true, true) => CloseProgress::TimeWait,
        (TcpState::FinWait1, true, false) => CloseProgress::State(TcpState::Closing),
        (TcpState::FinWait1, false, true) => CloseProgress::State(TcpState::FinWait2),
        (TcpState::FinWait2, true, _) => CloseProgress::TimeWait,
        (TcpState::Closing, _, true) => CloseProgress::TimeWait,
        (TcpState::LastAck, _, true) => CloseProgress::Closed,
        _ => CloseProgress::Unchanged,
    }
}

pub(crate) fn state_timeout_expired(
    state: TcpState,
    now: u64,
    entered_at: u64,
    ticks_per_second: u64,
) -> bool {
    let timeout_ms = match state {
        TcpState::SynReceived => SYN_RECEIVED_TIMEOUT_MS,
        TcpState::FinWait2 => FIN_WAIT_2_TIMEOUT_MS,
        TcpState::TimeWait => TIME_WAIT_TIMEOUT_MS,
        _ => return false,
    };
    now.wrapping_sub(entered_at) > timeout_ms.saturating_mul(ticks_per_second) / 1000
}

#[cfg(test)]
mod tests {
    use super::{TcpState, state_timeout_expired};

    #[test]
    fn syn_received_expires_after_the_libogc_timeout() {
        let ticks_per_second = 1000;
        let timeout_ticks = 20_000;

        assert!(!state_timeout_expired(
            TcpState::SynReceived,
            timeout_ticks,
            0,
            ticks_per_second,
        ));
        assert!(state_timeout_expired(
            TcpState::SynReceived,
            timeout_ticks + 1,
            0,
            ticks_per_second,
        ));
    }
}
