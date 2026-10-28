//! Versioned state and verified alternating-file commits. The legacy save is read only.
use crate::settings::{Category, Mode, RECORD_COUNT, Settings};
pub const RECORD_LEN: usize = 2048;
const DATA_END: usize = 64 + RECORD_COUNT * 4;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    pub settings: Settings,
    pub records: [u32; RECORD_COUNT],
    pub legacy_best: u64,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            records: [0; RECORD_COUNT],
            legacy_best: 0,
        }
    }
}
impl Data {
    fn valid_scores(&self) -> bool {
        self.records.iter().enumerate().all(|(i, &score)| {
            if (i / 20) % 2 == 1 {
                true
            } else if i % 20 < 10 {
                score <= ((i % 20 + 1) * 10) as u32
            } else {
                score == 0
            }
        })
    }
    pub fn best(&self, settings: Settings) -> u32 {
        self.records[settings.record_index()]
    }
    pub fn record(&mut self, settings: Settings, score: u32) -> bool {
        let best = &mut self.records[settings.record_index()];
        if score > *best {
            *best = score;
            true
        } else {
            false
        }
    }
    pub fn merge_records(&mut self, disk: &Self) {
        for (local, stored) in self.records.iter_mut().zip(disk.records.iter()) {
            *local = (*local).max(*stored);
        }
        self.legacy_best = self.legacy_best.max(disk.legacy_best);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    Length,
    Magic,
    Version,
    Reserved,
    Checksum,
    Settings,
    Verify,
    Sequence,
    Score,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub sequence: u64,
    pub data: Data,
}
pub fn encode(record: &Record) -> Result<[u8; RECORD_LEN], SaveError> {
    let s = record.data.settings;
    if !s.valid() {
        return Err(SaveError::Settings);
    }
    if !record.data.valid_scores() {
        return Err(SaveError::Score);
    }
    let mut b = [0; RECORD_LEN];
    b[..4].copy_from_slice(b"LFST");
    b[4..6].copy_from_slice(&2_u16.to_le_bytes());
    b[6..8].copy_from_slice(&(DATA_END as u16).to_le_bytes());
    b[8..16].copy_from_slice(&record.sequence.to_le_bytes());
    b[16..24].copy_from_slice(&record.data.legacy_best.to_le_bytes());
    b[24..36].copy_from_slice(&[
        s.category as u8,
        s.mode as u8,
        s.chances,
        s.cards,
        u8::from(s.asymmetric_only),
        (s.feedback_ms / 500) as u8,
        u8::from(s.voice),
        s.voice_volume,
        u8::from(s.effects),
        s.effects_volume,
        u8::from(s.flashes),
        u8::from(s.sideways),
    ]);
    for (i, score) in record.data.records.iter().enumerate() {
        b[64 + i * 4..68 + i * 4].copy_from_slice(&score.to_le_bytes());
    }
    let crc = crc32(&b[..DATA_END]);
    b[DATA_END..DATA_END + 4].copy_from_slice(&crc.to_le_bytes());
    Ok(b)
}
pub fn decode(b: &[u8]) -> Result<Record, SaveError> {
    if b.len() != RECORD_LEN {
        return Err(SaveError::Length);
    }
    if &b[..4] != b"LFST" {
        return Err(SaveError::Magic);
    }
    if b[4..6] != 2_u16.to_le_bytes() || b[6..8] != (DATA_END as u16).to_le_bytes() {
        return Err(SaveError::Version);
    }
    if b[36..64]
        .iter()
        .chain(b[DATA_END + 4..].iter())
        .any(|&v| v != 0)
    {
        return Err(SaveError::Reserved);
    }
    if u32::from_le_bytes(
        b[DATA_END..DATA_END + 4]
            .try_into()
            .map_err(|_| SaveError::Length)?,
    ) != crc32(&b[..DATA_END])
    {
        return Err(SaveError::Checksum);
    }
    if b[24] >= 4 || b[25] >= 2 || [b[28], b[30], b[32], b[34], b[35]].iter().any(|&v| v > 1) {
        return Err(SaveError::Settings);
    }
    let settings = Settings {
        category: Category::ALL[b[24] as usize],
        mode: if b[25] == 0 {
            Mode::Practice
        } else {
            Mode::Challenge
        },
        chances: b[26],
        cards: b[27],
        asymmetric_only: b[28] != 0,
        feedback_ms: u16::from(b[29]) * 500,
        voice: b[30] != 0,
        voice_volume: b[31],
        effects: b[32] != 0,
        effects_volume: b[33],
        flashes: b[34] != 0,
        sideways: b[35] != 0,
    };
    if !settings.valid() {
        return Err(SaveError::Settings);
    }
    let mut data = Data {
        settings,
        legacy_best: u64::from_le_bytes(b[16..24].try_into().map_err(|_| SaveError::Length)?),
        ..Data::default()
    };
    for (i, score) in data.records.iter_mut().enumerate() {
        *score = u32::from_le_bytes(
            b[64 + i * 4..68 + i * 4]
                .try_into()
                .map_err(|_| SaveError::Length)?,
        );
    }
    if !data.valid_scores() {
        return Err(SaveError::Score);
    }
    Ok(Record {
        sequence: u64::from_le_bytes(b[8..16].try_into().map_err(|_| SaveError::Length)?),
        data,
    })
}
pub fn decode_legacy(b: &[u8]) -> Result<u64, SaveError> {
    if b.len() != 512 {
        return Err(SaveError::Length);
    }
    if &b[..4] != b"LFHS" {
        return Err(SaveError::Magic);
    }
    if b[4..8] != [1, 0, 16, 0] {
        return Err(SaveError::Version);
    }
    if b[20..].iter().any(|&v| v != 0) {
        return Err(SaveError::Reserved);
    }
    if b[16..20] != crc32(&b[..16]).to_le_bytes() {
        return Err(SaveError::Checksum);
    }
    Ok(u64::from_le_bytes(
        b[8..16].try_into().map_err(|_| SaveError::Length)?,
    ))
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
/// Both files are in separate CARD allocations. A failed commit leaves the active file untouched.
pub trait Medium {
    type Error;
    fn write(&mut self, bank: usize, bytes: &[u8; RECORD_LEN]) -> Result<(), Self::Error>;
    fn read(&mut self, bank: usize) -> Result<[u8; RECORD_LEN], Self::Error>;
}
#[derive(Debug, PartialEq, Eq)]
pub enum CommitError<E> {
    Medium(E),
    Invalid(SaveError),
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Journal {
    pub active: Option<usize>,
    pub sequence: u64,
}
impl Journal {
    pub fn select(records: [Option<Record>; 2]) -> Result<(Self, Data), SaveError> {
        if let [Some(a), Some(b)] = &records {
            if a.sequence == b.sequence && a.data != b.data {
                return Err(SaveError::Sequence);
            }
        }
        let active = match (&records[0], &records[1]) {
            (Some(a), Some(b)) => Some(if b.sequence > a.sequence { 1 } else { 0 }),
            (Some(_), None) => Some(0),
            (None, Some(_)) => Some(1),
            _ => None,
        };
        if let Some(bank) = active {
            let r = records[bank].as_ref().ok_or(SaveError::Sequence)?;
            Ok((
                Self {
                    active,
                    sequence: r.sequence,
                },
                r.data.clone(),
            ))
        } else {
            Ok((Self::default(), Data::default()))
        }
    }
    pub fn commit<M: Medium>(
        &mut self,
        medium: &mut M,
        data: &Data,
    ) -> Result<(), CommitError<M::Error>> {
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(CommitError::Invalid(SaveError::Sequence))?;
        let next = Record {
            sequence,
            data: data.clone(),
        };
        let bytes = encode(&next).map_err(CommitError::Invalid)?;
        let bank = self.active.map_or(0, |b| 1 - b);
        medium.write(bank, &bytes).map_err(CommitError::Medium)?;
        let read = medium.read(bank).map_err(CommitError::Medium)?;
        if read != bytes || decode(&read).map_err(CommitError::Invalid)? != next {
            return Err(CommitError::Invalid(SaveError::Verify));
        }
        self.active = Some(bank);
        self.sequence = sequence;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Disk {
        banks: [[u8; RECORD_LEN]; 2],
        failure: u8,
    }
    impl Medium for Disk {
        type Error = u8;
        fn write(&mut self, bank: usize, b: &[u8; RECORD_LEN]) -> Result<(), u8> {
            if self.failure == 1 {
                self.banks[bank][..100].copy_from_slice(&b[..100]);
                return Err(1);
            }
            self.banks[bank] = *b;
            Ok(())
        }
        fn read(&mut self, bank: usize) -> Result<[u8; RECORD_LEN], u8> {
            if self.failure == 2 {
                return Err(2);
            }
            let mut b = self.banks[bank];
            if self.failure == 3 {
                b[64] ^= 1;
            }
            Ok(b)
        }
    }
    #[test]
    fn version_two_record_has_independent_fixed_header_and_crc() {
        let bytes = encode(&Record {
            sequence: 1,
            data: Data::default(),
        })
        .unwrap();
        assert_eq!(
            &bytes[..16],
            b"LFST\x02\x00\x40\x05\x01\x00\x00\x00\x00\x00\x00\x00"
        );
        assert_eq!(&bytes[24..36], &[0, 0, 10, 20, 0, 4, 1, 50, 1, 35, 0, 0]);
        assert_eq!(&bytes[1344..1348], &[72, 163, 163, 116]);
        let mut invalid = Record {
            sequence: 1,
            data: Data::default(),
        };
        invalid.data.settings.chances = 0;
        assert_eq!(encode(&invalid), Err(SaveError::Settings));
    }
    #[test]
    fn impossible_practice_scores_are_rejected_even_with_valid_crc() {
        let mut record = Record {
            sequence: 1,
            data: Data::default(),
        };
        let mut bytes = encode(&record).unwrap();
        bytes[68..72].copy_from_slice(&100_u32.to_le_bytes());
        // Fixed CRC from Python zlib for this independent malformed-record fixture.
        bytes[1344..1348].copy_from_slice(&[185, 35, 0, 244]);
        assert_eq!(decode(&bytes), Err(SaveError::Score));
        record.data.records[1] = 100;
        assert_eq!(encode(&record), Err(SaveError::Score));
        record.data.records[1] = 0;
        record.data.records[10] = 1;
        assert_eq!(encode(&record), Err(SaveError::Score));
    }
    #[test]
    fn legacy_fixed_bytes_are_preserved_and_validated() {
        let mut b = [0; 512];
        b[..20].copy_from_slice(
            b"LFHS\x01\x00\x10\x00\xd2\x04\x00\x00\x00\x00\x00\x00\x65\xeb\x3a\x47",
        );
        assert_eq!(decode_legacy(&b), Ok(1234));
        b[8] ^= 1;
        assert_eq!(decode_legacy(&b), Err(SaveError::Checksum));
    }
    #[test]
    fn records_reload_settings_scores_and_reject_invalid_data() {
        let mut data = Data::default();
        data.settings.chances = 17;
        data.settings.sideways = true;
        data.record(data.settings, 13);
        let r = Record { sequence: 3, data };
        let b = encode(&r).unwrap();
        assert_eq!(decode(&b), Ok(r));
        let mut bad = b;
        bad[70] ^= 1;
        assert_eq!(decode(&bad), Err(SaveError::Checksum));
        let mut bad = b;
        bad[4] = 9;
        assert_eq!(decode(&bad), Err(SaveError::Version));
        let mut bad = b;
        bad[1800] = 1;
        assert_eq!(decode(&bad), Err(SaveError::Reserved));
        assert_eq!(decode(&b[..511]), Err(SaveError::Length));
    }
    #[test]
    fn failure_at_write_read_and_verify_preserves_committed_record() {
        for failure in [1, 2, 3] {
            let mut disk = Disk {
                banks: [[0; RECORD_LEN]; 2],
                failure: 0,
            };
            let mut journal = Journal::default();
            let mut data = Data::default();
            data.record(data.settings, 7);
            journal.commit(&mut disk, &data).unwrap();
            disk.failure = failure;
            data.record(data.settings, 12);
            assert!(journal.commit(&mut disk, &data).is_err());
            assert_eq!(journal.active, Some(0));
            assert_eq!(journal.sequence, 1);
            let saved = decode(&disk.banks[0]).unwrap();
            assert_eq!(saved.data.best(saved.data.settings), 7);
        }
    }
    #[test]
    fn ambiguous_sequences_and_sequence_overflow_are_rejected() {
        let a = Record {
            sequence: 4,
            data: Data::default(),
        };
        let mut b = a.clone();
        b.data.settings.chances = 11;
        assert_eq!(
            Journal::select([Some(a), Some(b)]).unwrap_err(),
            SaveError::Sequence
        );
        let mut j = Journal {
            active: Some(0),
            sequence: u64::MAX,
        };
        let mut disk = Disk {
            banks: [[0; RECORD_LEN]; 2],
            failure: 0,
        };
        assert_eq!(
            j.commit(&mut disk, &Data::default()),
            Err(CommitError::Invalid(SaveError::Sequence))
        );
        assert_eq!(disk.banks, [[0; RECORD_LEN]; 2]);
    }
    #[test]
    fn restart_selects_latest_verified_record_and_merge_preserves_higher_scores() {
        let mut disk = Disk {
            banks: [[0; RECORD_LEN]; 2],
            failure: 0,
        };
        let mut j = Journal::default();
        let mut data = Data::default();
        data.record(data.settings, 7);
        j.commit(&mut disk, &data).unwrap();
        data.record(data.settings, 9);
        j.commit(&mut disk, &data).unwrap();
        let (j, reloaded) = Journal::select([
            Some(decode(&disk.banks[0]).unwrap()),
            Some(decode(&disk.banks[1]).unwrap()),
        ])
        .unwrap();
        assert_eq!(j.active, Some(1));
        assert_eq!(reloaded.best(reloaded.settings), 9);
        let mut local = Data::default();
        local.record(local.settings, 11);
        local.merge_records(&reloaded);
        assert_eq!(local.best(local.settings), 11);
    }
}
