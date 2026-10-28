//! State owned by one USB disc. The caller serializes selection and close.
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

pub struct DriveState {
    pub mounted: AtomicBool,
    pub lun: AtomicU32,
    pub device_id: AtomicI32,
    pub vid: AtomicU32,
    pub pid: AtomicU32,
    pub last_used: AtomicU32,
    pub usb2_mode: AtomicBool,
    pub removed: AtomicBool,
}

impl Default for DriveState {
    fn default() -> Self {
        Self::new()
    }
}

impl DriveState {
    pub const fn new() -> Self {
        Self {
            mounted: AtomicBool::new(false),
            lun: AtomicU32::new(0),
            device_id: AtomicI32::new(-1),
            vid: AtomicU32::new(0),
            pid: AtomicU32::new(0),
            last_used: AtomicU32::new(0),
            usb2_mode: AtomicBool::new(false),
            removed: AtomicBool::new(false),
        }
    }

    /// Release identity only after the transport has actually closed.
    pub fn reset(&self) {
        self.mounted.store(false, Ordering::Release);
        self.lun.store(0, Ordering::Release);
        self.device_id.store(-1, Ordering::Release);
        self.vid.store(0, Ordering::Release);
        self.pid.store(0, Ordering::Release);
    }

    /// Removal callbacks retain ownership until the next serialized close.
    pub fn remove(&self) {
        self.removed.store(true, Ordering::Release);
        self.mounted.store(false, Ordering::Release);
    }

    pub fn can_select(&self, candidate_id: i32) -> bool {
        candidate_id >= 0 && self.device_id.load(Ordering::Acquire) != candidate_id
    }

    /// Publish only after LUN setup; a callback during setup wins.
    pub fn publish(&self) -> bool {
        self.mounted.store(true, Ordering::Release);
        if self.removed.load(Ordering::Acquire) {
            self.mounted.store(false, Ordering::Release);
            false
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_drive_starts_unmounted_and_unclaimed() {
        let drive = DriveState::new();
        assert!(!drive.mounted.load(Ordering::Acquire));
        assert_eq!(drive.device_id.load(Ordering::Acquire), -1);
        assert_eq!(drive.lun.load(Ordering::Acquire), 0);
        assert_eq!(drive.vid.load(Ordering::Acquire), 0);
        assert_eq!(drive.pid.load(Ordering::Acquire), 0);
        assert_eq!(drive.last_used.load(Ordering::Acquire), 0);
        assert!(!drive.usb2_mode.load(Ordering::Acquire));
        assert!(!drive.removed.load(Ordering::Acquire));
    }
    #[test]
    fn identical_models_remain_distinct_by_ios_id() {
        let first = DriveState::new();
        first.device_id.store(41, Ordering::Release);
        first.vid.store(0x1234, Ordering::Release);
        first.pid.store(0x5678, Ordering::Release);
        assert!(!first.can_select(41));
        assert!(first.can_select(42));
        assert!(!first.can_select(-1));
    }
    #[test]
    fn removing_or_closing_one_drive_preserves_the_other() {
        let drives = [DriveState::new(), DriveState::new()];
        for (index, drive) in drives.iter().enumerate() {
            drive.device_id.store(41 + index as i32, Ordering::Release);
            drive.last_used.store(100 + index as u32, Ordering::Release);
            drive.lun.store(index as u32, Ordering::Release);
            assert!(drive.publish());
        }
        drives[0].remove();
        assert!(!drives[0].mounted.load(Ordering::Acquire));
        assert!(!drives[0].can_select(41)); // failed close retains the claim
        assert!(drives[1].mounted.load(Ordering::Acquire));
        assert_eq!(drives[1].lun.load(Ordering::Acquire), 1);
        assert_eq!(drives[1].last_used.load(Ordering::Acquire), 101);
        drives[0].reset(); // successful close
        assert!(drives[0].can_select(41));
        assert_eq!(drives[1].device_id.load(Ordering::Acquire), 42);
        assert!(drives[1].mounted.load(Ordering::Acquire));
    }
    #[test]
    fn removal_during_mount_cannot_be_published_as_ready() {
        let drive = DriveState::new();
        drive.device_id.store(41, Ordering::Release);
        drive.remove();
        assert!(!drive.publish());
        assert!(!drive.mounted.load(Ordering::Acquire));
        assert!(!drive.can_select(41));
    }
}
