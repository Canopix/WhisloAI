pub(crate) const ANCHOR_MONITOR_ACTIVE_POLL_MS: u64 = 180;
pub(crate) const ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS: u64 = 700;

pub(crate) fn anchor_monitor_poll_interval_ms(
    floating_mode: bool,
    contextual_tracking_supported: bool,
) -> u64 {
    if !floating_mode && !contextual_tracking_supported {
        return ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS;
    }
    ANCHOR_MONITOR_ACTIVE_POLL_MS
}

#[cfg(test)]
mod tests {
    use super::{
        anchor_monitor_poll_interval_ms, ANCHOR_MONITOR_ACTIVE_POLL_MS,
        ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS,
    };

    #[test]
    fn anchor_monitor_poll_interval_stays_fast_for_active_tracking() {
        assert_eq!(
            anchor_monitor_poll_interval_ms(true, false),
            ANCHOR_MONITOR_ACTIVE_POLL_MS
        );
        assert_eq!(
            anchor_monitor_poll_interval_ms(false, true),
            ANCHOR_MONITOR_ACTIVE_POLL_MS
        );
    }

    #[test]
    fn anchor_monitor_poll_interval_slows_down_when_contextual_tracking_is_unavailable() {
        assert_eq!(
            anchor_monitor_poll_interval_ms(false, false),
            ANCHOR_MONITOR_IDLE_UNSUPPORTED_POLL_MS
        );
    }
}
