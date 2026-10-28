use crate::time::Duration;
#[path = "wii_value.rs"]
mod value;
pub use value::{Instant, SystemTime, UNIX_EPOCH};

fn now(wall: bool) -> Duration {
    let mut seconds = 0;
    let mut nanos = 0;
    let result = unsafe { crate::sys::wii::clock_now(wall, &mut seconds, &mut nanos) };
    assert_eq!(result, 0, "Wii clock read failed with OS error {result}");
    assert!(
        nanos < 1_000_000_000,
        "Wii clock returned invalid nanoseconds"
    );
    Duration::new(seconds, nanos)
}
impl Instant {
    pub fn now() -> Self {
        Self(now(false))
    }
}
impl SystemTime {
    pub fn now() -> Self {
        Self::from_duration(now(true)).expect("Wii wall clock exceeds supported range")
    }
}
