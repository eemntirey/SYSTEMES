//! Monotonic kernel clock driven by the calibrated Local APIC scheduler timer.
//!
//! The initial clock has 10 ms resolution (100 Hz). Time advances only after a
//! delivered Local APIC timer interrupt; it never depends on wall-clock time.

use core::sync::atomic::{AtomicU64, Ordering};

pub const TICK_HZ: u64 = 100;
pub const TICK_NANOS: u64 = 1_000_000_000 / TICK_HZ;

static TICKS: AtomicU64 = AtomicU64::new(0);

/// Reset the clock before enabling the calibrated Local APIC timer.
pub fn initialize() {
    TICKS.store(0, Ordering::Release);
}

/// Account for one delivered scheduler-clock interrupt.
#[inline]
pub fn on_timer_interrupt() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Number of elapsed monotonic clock ticks.
#[inline]
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Acquire)
}

/// Monotonic elapsed nanoseconds since clock initialization.
///
/// This first implementation has 10 ms granularity. It is independent of
/// wall-clock adjustments and is not a calendar/UTC clock.
#[inline]
pub fn now_nanos() -> u64 {
    ticks().saturating_mul(TICK_NANOS)
}

/// Monotonic elapsed milliseconds since clock initialization.
#[inline]
pub fn now_millis() -> u64 {
    ticks().saturating_mul(TICK_NANOS / 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_starts_at_zero_and_advances_monotonically() {
        initialize();
        assert_eq!(ticks(), 0);
        assert_eq!(now_nanos(), 0);
        on_timer_interrupt();
        assert_eq!(ticks(), 1);
        assert_eq!(now_nanos(), 10_000_000);
        assert_eq!(now_millis(), 10);
        on_timer_interrupt();
        assert_eq!(now_millis(), 20);
    }
}
