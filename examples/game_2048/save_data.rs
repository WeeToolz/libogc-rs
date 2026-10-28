//! Versioned, checksummed 2048 records and audio settings for memory-card saves.

pub const SAVE_BYTES: usize = 512;
const HEADER_BYTES: usize = 24;
const VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SaveData {
    pub best_score: u64,
    pub effects_volume: u8,
    pub music_volume: u8,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            best_score: 0,
            effects_volume: 96,
            music_volume: 36,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveError {
    WrongLength,
    WrongMagic,
    UnsupportedVersion,
    InvalidLength,
    InvalidVolume,
    InvalidChecksum,
}

impl SaveData {
    pub fn encode(self) -> Result<[u8; SAVE_BYTES], SaveError> {
        if self.effects_volume > 192 || self.music_volume > 128 {
            return Err(SaveError::InvalidVolume);
        }
        let mut bytes = [0_u8; SAVE_BYTES];
        bytes[0..4].copy_from_slice(b"2048");
        bytes[4..6].copy_from_slice(&VERSION.to_le_bytes());
        bytes[6..8].copy_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
        bytes[8..16].copy_from_slice(&self.best_score.to_le_bytes());
        bytes[16] = self.effects_volume;
        bytes[17] = self.music_volume;
        let checksum = crc32(&bytes[..20]);
        bytes[20..24].copy_from_slice(&checksum.to_le_bytes());
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SaveError> {
        if bytes.len() != SAVE_BYTES {
            return Err(SaveError::WrongLength);
        }
        if &bytes[0..4] != b"2048" {
            return Err(SaveError::WrongMagic);
        }
        if u16::from_le_bytes([bytes[4], bytes[5]]) != VERSION {
            return Err(SaveError::UnsupportedVersion);
        }
        if u16::from_le_bytes([bytes[6], bytes[7]]) as usize != HEADER_BYTES {
            return Err(SaveError::InvalidLength);
        }
        if bytes[18..20] != [0, 0] || bytes[24..].iter().any(|&byte| byte != 0) {
            return Err(SaveError::InvalidLength);
        }
        if bytes[16] > 192 || bytes[17] > 128 {
            return Err(SaveError::InvalidVolume);
        }
        let expected = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        if crc32(&bytes[..20]) != expected {
            return Err(SaveError::InvalidChecksum);
        }
        Ok(Self {
            best_score: u64::from_le_bytes(
                bytes[8..16]
                    .try_into()
                    .map_err(|_| SaveError::InvalidLength)?,
            ),
            effects_volume: bytes[16],
            music_volume: bytes[17],
        })
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &byte in bytes {
        crc ^= byte as u32;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_save_matches_independent_wire_bytes() {
        let bytes = SaveData::default().encode().unwrap();
        assert_eq!(
            &bytes[..24],
            &[
                0x32, 0x30, 0x34, 0x38, 0x01, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x60, 0x24, 0x00, 0x00, 0xF2, 0x26, 0xA9, 0x52,
            ]
        );
        assert_eq!(SaveData::decode(&bytes), Ok(SaveData::default()));
    }

    #[test]
    fn nondefault_record_uses_little_endian_score_and_settings() {
        let record = SaveData {
            best_score: 0x0102_0304_0506_0708,
            effects_volume: 128,
            music_volume: 64,
        };
        let bytes = record.encode().unwrap();
        assert_eq!(
            &bytes[..24],
            &[
                0x32, 0x30, 0x34, 0x38, 0x01, 0x00, 0x18, 0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03,
                0x02, 0x01, 0x80, 0x40, 0x00, 0x00, 0x9F, 0xCE, 0xBC, 0x66,
            ]
        );
        assert_eq!(SaveData::decode(&bytes), Ok(record));
    }

    #[test]
    fn save_rejects_corruption_versions_and_invalid_settings() {
        let mut bytes = SaveData::default().encode().unwrap();
        bytes[8] ^= 1;
        assert_eq!(SaveData::decode(&bytes), Err(SaveError::InvalidChecksum));
        let mut bytes = SaveData::default().encode().unwrap();
        bytes[4] = 2;
        assert_eq!(SaveData::decode(&bytes), Err(SaveError::UnsupportedVersion));
        let mut bytes = SaveData::default().encode().unwrap();
        bytes[16] = 193;
        assert_eq!(SaveData::decode(&bytes), Err(SaveError::InvalidVolume));
        assert_eq!(SaveData::decode(&bytes[..511]), Err(SaveError::WrongLength));
        let mut bytes = SaveData::default().encode().unwrap();
        bytes[0] = b'X';
        assert_eq!(SaveData::decode(&bytes), Err(SaveError::WrongMagic));
        let mut bytes = SaveData::default().encode().unwrap();
        bytes[40] = 1;
        assert_eq!(SaveData::decode(&bytes), Err(SaveError::InvalidLength));
        assert_eq!(
            SaveData {
                effects_volume: 193,
                ..SaveData::default()
            }
            .encode(),
            Err(SaveError::InvalidVolume)
        );
    }
}
