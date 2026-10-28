//! Real CARD transport for the shared verified journal. Legacy files are never modified.
use crate::save::{self, Data, Journal, Medium, RECORD_LEN, Record};
use ogc_card as card;
use ogc_system as system;

const NAMES: [&[u8]; 2] = [b"LFSTATE0\0", b"LFSTATE1\0"];
const LEGACY: &[u8] = b"LETTERFLIP\0";
pub use crate::card_storage_status::CardStatus;
pub struct CardStore {
    names: [&'static [u8]; 2],
    legacy: Option<&'static [u8]>,
    mounted: Option<card::MountedCard>,
    journal: Journal,
    data: Data,
    writable: bool,
}
fn checked(status: i32) -> Result<(), i32> {
    if status == card::sys::CARD_ERROR_READY {
        Ok(())
    } else {
        Err(status)
    }
}
fn invalid(error: save::SaveError) -> i32 {
    system::report(format_args!(
        "Letter Flip: save validation failed: {error:?}\n"
    ));
    card::sys::CARD_ERROR_FATAL_ERROR
}
/// Always close a file, including when validation or the operation fails.
fn with_file<T>(
    slot: card::CardSlot,
    name: &[u8],
    create: bool,
    minimum: usize,
    operation: impl FnOnce(&mut card::sys::CardFile) -> Result<T, i32>,
) -> Result<Option<T>, i32> {
    let mut file = card::sys::CardFile::default();
    // SAFETY: callers pass static NUL-terminated names and a live initialized output.
    let opened = unsafe { card::sys::open(slot.channel(), name.as_ptr().cast(), &mut file) };
    if opened == card::sys::CARD_ERROR_NOFILE {
        if !create {
            return Ok(None);
        }
        let sector = card::sector_size(slot)?;
        if sector < RECORD_LEN as u32 || sector % card::sys::CARD_READSIZE != 0 {
            return Err(card::sys::CARD_ERROR_FATAL_ERROR);
        }
        if card::free_blocks(slot)? == 0 {
            return Err(card::sys::CARD_ERROR_INSSPACE);
        }
        // SAFETY: the same valid name/output cover this synchronous create.
        checked(unsafe {
            card::sys::create(slot.channel(), name.as_ptr().cast(), sector, &mut file)
        })?;
    } else {
        checked(opened)?;
    }
    let result = if file.len < 0 || (file.len as usize) < minimum {
        Err(card::sys::CARD_ERROR_FATAL_ERROR)
    } else {
        operation(&mut file)
    };
    // SAFETY: file is an open, uniquely borrowed descriptor; operation has completed.
    let closed = unsafe { card::sys::close(&mut file) };
    if closed != card::sys::CARD_ERROR_READY {
        system::report(format_args!(
            "Letter Flip: close failed ({closed}); operation: {:?}\n",
            result.as_ref().err()
        ));
        return Err(closed);
    }
    result.map(Some)
}
impl CardStore {
    pub fn open() -> (Option<Self>, CardStatus) {
        Self::open_named(NAMES, Some(LEGACY))
    }
    #[cfg(feature = "letter-flip-hardware-probe")]
    pub fn open_probe() -> (Option<Self>, CardStatus) {
        Self::open_named([b"LFPROBE0\0", b"LFPROBE1\0"], None)
    }
    fn open_named(
        names: [&'static [u8]; 2],
        legacy: Option<&'static [u8]>,
    ) -> (Option<Self>, CardStatus) {
        let game = match card::sys::CardGameCode::new(*b"LFLP") {
            Ok(v) => v,
            Err(e) => {
                system::report(format_args!("Letter Flip: game code: {e:?}\n"));
                return (None, CardStatus::Error);
            }
        };
        let company = match card::sys::CardCompanyCode::new(*b"RS") {
            Ok(v) => v,
            Err(e) => {
                system::report(format_args!("Letter Flip: company code: {e:?}\n"));
                return (None, CardStatus::Error);
            }
        };
        if let Err(e) = checked(card::sys::init(Some(game), Some(company))) {
            system::report(format_args!("Letter Flip: CARD init failed ({e})\n"));
            return (None, CardStatus::Error);
        }
        let mut status = CardStatus::NoCard;
        for slot in [card::CardSlot::A, card::CardSlot::B] {
            let mounted = match mount_slot(slot) {
                Ok(Some(m)) => m,
                Ok(None) => continue,
                Err(e) => {
                    system::report(format_args!("Letter Flip: CARD mount failed ({e})\n"));
                    status = CardStatus::Error;
                    continue;
                }
            };
            let mut store = Self {
                names,
                legacy,
                mounted: Some(mounted),
                journal: Journal::default(),
                data: Data::default(),
                writable: true,
            };
            match store.load() {
                Ok(()) => {
                    let status = if store.writable {
                        CardStatus::Ready
                    } else {
                        CardStatus::Recovered
                    };
                    return (Some(store), status);
                }
                Err(e) => {
                    system::report(format_args!("Letter Flip: CARD load failed ({e})\n"));
                    status = CardStatus::Error;
                }
            }
            // Drop checks and reports unmount errors.
        }
        (None, status)
    }
    fn slot(&self) -> Result<card::CardSlot, i32> {
        self.mounted
            .as_ref()
            .filter(|m| m.is_attached())
            .map(card::MountedCard::slot)
            .ok_or(card::sys::CARD_ERROR_NOCARD)
    }
    fn load(&mut self) -> Result<(), i32> {
        let slot = self.slot()?;
        let mut records: [Option<Record>; 2] = [None, None];
        let mut damaged = false;
        for (bank, record) in records.iter_mut().enumerate() {
            let bytes = with_file(slot, self.names[bank], false, RECORD_LEN, |file| {
                let mut b = [0; RECORD_LEN];
                checked(card::read_slice(file, &mut b, 0))?;
                Ok(b)
            })?;
            if let Some(bytes) = bytes {
                match save::decode(&bytes) {
                    Ok(r) => *record = Some(r),
                    Err(e) => {
                        invalid(e);
                        damaged = true;
                    }
                }
            }
        }
        if damaged && records.iter().all(Option::is_none) {
            return Err(card::sys::CARD_ERROR_FATAL_ERROR);
        }
        let (journal, mut data) = Journal::select(records).map_err(invalid)?;
        if let Some(legacy) = self.legacy.filter(|_| journal.active.is_none()) {
            if let Some(score) = with_file(slot, legacy, false, 512, |file| {
                let mut bytes = [0; 512];
                checked(card::read_slice(file, &mut bytes, 0))?;
                save::decode_legacy(&bytes).map_err(invalid)
            })? {
                data.legacy_best = data.legacy_best.max(score);
            }
        }
        self.journal = journal;
        self.data = data;
        self.writable = !damaged;
        Ok(())
    }
    #[cfg(feature = "letter-flip-hardware-probe")]
    pub fn finish(mut self) -> Result<(), i32> {
        if let Some(mounted) = self.mounted.take() {
            checked(mounted.unmount())
        } else {
            Err(card::sys::CARD_ERROR_NOCARD)
        }
    }
    pub fn attached(&self) -> bool {
        self.slot().is_ok()
    }
    pub fn data(&self) -> &Data {
        &self.data
    }
    pub fn write(&mut self, data: &Data) -> Result<(), i32> {
        if !self.writable {
            return Err(card::sys::CARD_ERROR_FATAL_ERROR);
        }
        self.slot()?;
        // The shared engine commits in-memory sequence only after write, close, read, and verification.
        let mut journal = self.journal;
        journal.commit(self, data).map_err(|error| match error {
            save::CommitError::Medium(e) => e,
            save::CommitError::Invalid(e) => invalid(e),
        })?;
        self.journal = journal;
        self.data = data.clone();
        Ok(())
    }
}
impl Medium for CardStore {
    type Error = i32;
    fn write(&mut self, bank: usize, bytes: &[u8; RECORD_LEN]) -> Result<(), i32> {
        let slot = self.slot()?;
        let name = *self
            .names
            .get(bank)
            .ok_or(card::sys::CARD_ERROR_FATAL_ERROR)?;
        // Revalidate an existing target before any write. Never replace malformed or unknown data.
        let previous = with_file(slot, name, false, RECORD_LEN, |file| {
            let mut b = [0; RECORD_LEN];
            checked(card::read_slice(file, &mut b, 0))?;
            save::decode(&b).map_err(invalid)
        })?;
        if let Some(record) = previous {
            if record.sequence > self.journal.sequence {
                return Err(card::sys::CARD_ERROR_FATAL_ERROR);
            }
        }
        let sector = card::sector_size(slot)?;
        let result = with_file(slot, name, true, RECORD_LEN, |file| {
            if file.len < 0 || file.len as u32 != sector {
                return Err(card::sys::CARD_ERROR_FATAL_ERROR);
            }
            checked(card::write_slice(file, bytes, 0))
        })?;
        result.ok_or(card::sys::CARD_ERROR_NOFILE)
    }
    fn read(&mut self, bank: usize) -> Result<[u8; RECORD_LEN], i32> {
        let slot = self.slot()?;
        let name = *self
            .names
            .get(bank)
            .ok_or(card::sys::CARD_ERROR_FATAL_ERROR)?;
        with_file(slot, name, false, RECORD_LEN, |file| {
            let mut b = [0; RECORD_LEN];
            checked(card::read_slice(file, &mut b, 0))?;
            Ok(b)
        })?
        .ok_or(card::sys::CARD_ERROR_NOFILE)
    }
}
impl Drop for CardStore {
    fn drop(&mut self) {
        if let Some(mounted) = self.mounted.take() {
            let result = mounted.unmount();
            if result != card::sys::CARD_ERROR_READY {
                system::report(format_args!("Letter Flip: unmount failed ({result})\n"));
            }
        }
    }
}
fn mount_slot(slot: card::CardSlot) -> Result<Option<card::MountedCard>, i32> {
    let mut request = match card::mount_async_owned(slot) {
        Ok(r) => r,
        Err(e) if e == card::sys::CARD_ERROR_NOCARD => return Ok(None),
        Err(e) => return Err(e),
    };
    let start = system::time();
    while system::time().saturating_sub(start) < 3 * crate::session::TICKS_PER_SECOND {
        if let card::CardAsyncStatus::Complete(_) = request.status() {
            match request.try_complete() {
                Ok(card::CardMountCompletion::Mounted(m)) => return Ok(Some(m)),
                Ok(card::CardMountCompletion::Failed(e)) if e == card::sys::CARD_ERROR_NOCARD => {
                    return Ok(None);
                }
                Ok(card::CardMountCompletion::Failed(e)) => return Err(e),
                Err(pending) => request = pending,
            }
        }
        ogc_video::wait_vsync();
    }
    drop(request);
    Err(card::sys::CARD_ERROR_BUSY)
}
