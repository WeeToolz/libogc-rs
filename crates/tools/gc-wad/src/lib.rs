//! Host-side channel WAD tools, ported from HBC Reborn's pywii WAD path.
//!
//! Keys are supplied by the caller. This crate does not install titles or write
//! NAND. A legacy fakesign is reported as such; it is never a valid RSA signature.
use aes::Aes128;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use gc_binary::{align_up, checked_range, read_u16_be, read_u32_be};
use num_bigint::BigUint;
use sha1::{Digest, Sha1};
use std::{collections::BTreeSet, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
fn err(s: impl Into<String>) -> Error {
    Error(s.into())
}
fn range(b: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    Ok(&b[checked_range(b.len(), off, len).map_err(|e| err(e.to_string()))?])
}
fn u16be(b: &[u8], off: usize) -> Result<u16> {
    read_u16_be(b, off).map_err(|e| err(e.to_string()))
}
fn u32be(b: &[u8], off: usize) -> Result<u32> {
    read_u32_be(b, off).map_err(|e| err(e.to_string()))
}
fn u64be(b: &[u8], off: usize) -> Result<u64> {
    Ok(u64::from_be_bytes(
        range(b, off, 8)?
            .try_into()
            .map_err(|_| err("invalid u64"))?,
    ))
}
fn aligned(n: usize, a: usize) -> Result<usize> {
    align_up(n, a).ok_or_else(|| err("size overflow"))
}
fn size32(n: usize) -> Result<u32> {
    n.try_into().map_err(|_| err("WAD size exceeds u32"))
}
fn offset(b: &[u8]) -> Result<usize> {
    let n = match u32be(b, 0)? {
        0x10000 => 0x240,
        0x10001 => 0x140,
        0x10002 => 0x80,
        _ => return Err(err("unknown signature type")),
    };
    range(b, 0, n)?;
    Ok(n)
}
fn text(b: &[u8]) -> Result<&str> {
    let n = b
        .iter()
        .position(|v| *v == 0)
        .ok_or_else(|| err("unterminated name"))?;
    let s = std::str::from_utf8(&b[..n]).map_err(|_| err("invalid name encoding"))?;
    if s.is_empty() || !s.bytes().all(|v| (0x20..0x7f).contains(&v)) {
        return Err(err("invalid name"));
    }
    Ok(s)
}
fn digest(b: &[u8]) -> [u8; 20] {
    Sha1::digest(b).into()
}

/// AES-128 CBC without padding. The caller must supply whole blocks.
fn crypt(key: &[u8; 16], iv: [u8; 16], input: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    if !input.len().is_multiple_of(16) {
        return Err(err("AES input is not block aligned"));
    }
    let aes = Aes128::new(key.into());
    let mut chain = iv;
    let mut out = input.to_vec();
    for block in out.chunks_exact_mut(16) {
        if encrypt {
            for (v, previous) in block.iter_mut().zip(chain) {
                *v ^= previous;
            }
            aes.encrypt_block(aes::cipher::Block::<Aes128>::from_mut_slice(block));
            chain.copy_from_slice(block);
        } else {
            let mut next = [0; 16];
            next.copy_from_slice(block);
            aes.decrypt_block(aes::cipher::Block::<Aes128>::from_mut_slice(block));
            for (v, previous) in block.iter_mut().zip(chain) {
                *v ^= previous;
            }
            chain = next;
        }
    }
    Ok(out)
}
fn title_iv(id: u64) -> [u8; 16] {
    let mut iv = [0; 16];
    iv[..8].copy_from_slice(&id.to_be_bytes());
    iv
}
fn content_iv(index: u16) -> [u8; 16] {
    let mut iv = [0; 16];
    iv[..2].copy_from_slice(&index.to_be_bytes());
    iv
}

#[derive(Clone, Debug)]
pub struct Ticket {
    bytes: Vec<u8>,
    body: usize,
}
impl Ticket {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let body = offset(bytes)?;
        if bytes.len() != body + 0x164 {
            return Err(err("invalid ticket size"));
        }
        text(range(bytes, body, 0x40)?)?;
        if bytes[body + 0xb1] > 1 {
            return Err(err("unsupported common key index"));
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            body,
        })
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn title_id(&self) -> u64 {
        u64be(&self.bytes, self.body + 0x9c).expect("validated ticket")
    }
    pub fn common_key_index(&self) -> u8 {
        self.bytes[self.body + 0xb1]
    }
    fn key(&self, common: &[u8; 16]) -> Result<[u8; 16]> {
        crypt(
            common,
            title_iv(self.title_id()),
            &self.bytes[self.body + 0x7f..self.body + 0x8f],
            false,
        )?
        .try_into()
        .map_err(|_| err("invalid title key size"))
    }
    /// Re-encrypt the same title key under the new title ID IV.
    pub fn retitle(&mut self, id: u64, common: &[u8; 16]) -> Result<()> {
        let encrypted = crypt(common, title_iv(id), &self.key(common)?, true)?;
        self.bytes[self.body + 0x7f..self.body + 0x8f].copy_from_slice(&encrypted);
        self.bytes[self.body + 0x9c..self.body + 0xa4].copy_from_slice(&id.to_be_bytes());
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRecord {
    pub id: u32,
    pub index: u16,
    pub kind: u16,
    pub size: u64,
    pub sha1: [u8; 20],
}
#[derive(Clone, Debug)]
pub struct Tmd {
    bytes: Vec<u8>,
    body: usize,
    records: Vec<ContentRecord>,
}
impl Tmd {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let body = offset(bytes)?;
        range(bytes, body, 0xa4)?;
        text(range(bytes, body, 0x40)?)?;
        let count = usize::from(u16be(bytes, body + 0x9e)?);
        if count == 0 || count > 512 || bytes.len() != body + 0xa4 + count * 36 {
            return Err(err("invalid TMD content count or size"));
        }
        let mut records = Vec::with_capacity(count);
        let (mut ids, mut indices) = (BTreeSet::new(), BTreeSet::new());
        for n in 0..count {
            let off = body + 0xa4 + n * 36;
            let record = ContentRecord {
                id: u32be(bytes, off)?,
                index: u16be(bytes, off + 4)?,
                kind: u16be(bytes, off + 6)?,
                size: u64be(bytes, off + 8)?,
                sha1: range(bytes, off + 16, 20)?
                    .try_into()
                    .map_err(|_| err("invalid digest"))?,
            };
            if !ids.insert(record.id) || !indices.insert(record.index) {
                return Err(err("duplicate content ID or index"));
            }
            if !matches!(record.kind, 1 | 0x4001 | 0x8001) {
                return Err(err("unsupported content type"));
            }
            records.push(record);
        }
        if !indices.contains(&u16be(bytes, body + 0xa0)?) {
            return Err(err("boot index is absent from TMD"));
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            body,
            records,
        })
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn title_id(&self) -> u64 {
        u64be(&self.bytes, self.body + 0x4c).expect("validated TMD")
    }
    pub fn ios_title_id(&self) -> u64 {
        u64be(&self.bytes, self.body + 0x44).expect("validated TMD")
    }
    pub fn version(&self) -> u16 {
        u16be(&self.bytes, self.body + 0x9c).expect("validated TMD")
    }
    pub fn boot_index(&self) -> u16 {
        u16be(&self.bytes, self.body + 0xa0).expect("validated TMD")
    }
    pub fn records(&self) -> &[ContentRecord] {
        &self.records
    }
    pub fn retitle(&mut self, id: u64) {
        self.bytes[self.body + 0x4c..self.body + 0x54].copy_from_slice(&id.to_be_bytes());
    }
    pub fn set_version(&mut self, version: u16) {
        self.bytes[self.body + 0x9c..self.body + 0x9e].copy_from_slice(&version.to_be_bytes());
    }
    fn update_records(&mut self, contents: &[Content]) -> Result<()> {
        if contents.len() != self.records.len() {
            return Err(err("content count does not match TMD"));
        }
        let mut seen = BTreeSet::new();
        for (n, record) in self.records.iter_mut().enumerate() {
            let content = contents
                .iter()
                .find(|c| c.id == record.id)
                .ok_or_else(|| err(format!("missing content {:08X}", record.id)))?;
            if !seen.insert(content.id) {
                return Err(err("duplicate content"));
            }
            record.size = content.data.len() as u64;
            record.sha1 = digest(&content.data);
            let off = self.body + 0xa4 + n * 36;
            self.bytes[off + 8..off + 16].copy_from_slice(&record.size.to_be_bytes());
            self.bytes[off + 16..off + 36].copy_from_slice(&record.sha1);
        }
        Ok(())
    }
}

/// Metadata signature state under the supplied certificates. No root trust is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    ValidRsa,
    LegacyFakesigned,
    Invalid,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignaturePolicy {
    Preserve,
    LegacyFakesign,
}

struct Certificate<'a> {
    name: String,
    body: &'a [u8],
    key_type: u32,
}
fn certificates(bytes: &[u8]) -> Result<Vec<Certificate<'_>>> {
    let mut result = Vec::new();
    let mut cursor = 0;
    let mut names = BTreeSet::new();
    while cursor < bytes.len() {
        let b = &bytes[cursor..];
        let off = offset(b)?;
        let key_type = u32be(b, off + 0x40)?;
        let key_size = match key_type {
            0 => 516,
            1 => 260,
            2 => 60,
            _ => return Err(err("unknown certificate key type")),
        };
        let size = aligned(0x8c + key_size, 64)?;
        let body = range(b, off, size)?;
        let name = format!("{}-{}", text(&body[..0x40])?, text(&body[0x44..0x84])?);
        if !names.insert(name.clone()) {
            return Err(err("duplicate certificate name"));
        }
        result.push(Certificate {
            name,
            body,
            key_type,
        });
        cursor = cursor
            .checked_add(off + size)
            .ok_or_else(|| err("certificate size overflow"))?;
    }
    if result.is_empty() {
        return Err(err("empty certificate chain"));
    }
    Ok(result)
}
/// Check the complete PKCS#1 v1.5 encoding, rather than the vulnerable strcmp check.
pub fn signature_status(blob: &[u8], certs: &[u8]) -> Result<SignatureStatus> {
    let off = offset(blob)?;
    let issuer = text(range(blob, off, 0x40)?)?;
    let certs = certificates(certs)?;
    let cert = certs
        .iter()
        .find(|c| c.name == issuer)
        .ok_or_else(|| err(format!("missing certificate for {issuer}")))?;
    let sig_size = match u32be(blob, 0)? {
        0x10000 => 512,
        0x10001 => 256,
        _ => return Ok(SignatureStatus::Unsupported),
    };
    let expected_type = if sig_size == 512 { 0 } else { 1 };
    if cert.key_type != expected_type {
        return Err(err("signature and certificate key types differ"));
    }
    let signature = range(blob, 4, sig_size)?;
    let hash = digest(&blob[off..]);
    if signature.iter().all(|v| *v == 0) && hash[0] == 0 {
        return Ok(SignatureStatus::LegacyFakesigned);
    }
    let modulus = BigUint::from_bytes_be(range(cert.body, 0x88, sig_size)?);
    let exponent = BigUint::from_bytes_be(range(cert.body, 0x88 + sig_size, 4)?);
    let signature = BigUint::from_bytes_be(signature);
    if modulus.bits() != sig_size as u64 * 8
        || exponent < BigUint::from(3u8)
        || signature >= modulus
    {
        return Ok(SignatureStatus::Invalid);
    }
    let decoded = signature.modpow(&exponent, &modulus).to_bytes_be();
    let mut expected = vec![0xff; sig_size];
    expected[0] = 0;
    expected[1] = 1;
    // DER DigestInfo for SHA-1 (RFC 8017).
    const PREFIX: &[u8] = &[
        0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04, 0x14,
    ];
    let end = sig_size - 20;
    let start = end - PREFIX.len();
    expected[start - 1] = 0;
    expected[start..end].copy_from_slice(PREFIX);
    expected[end..].copy_from_slice(&hash);
    let decoded_start = sig_size
        .checked_sub(decoded.len())
        .ok_or_else(|| err("RSA output overflow"))?;
    let valid =
        expected[..decoded_start].iter().all(|v| *v == 0) && expected[decoded_start..] == decoded;
    Ok(if valid {
        SignatureStatus::ValidRsa
    } else {
        SignatureStatus::Invalid
    })
}
fn sign_legacy(blob: &mut [u8], fill: usize) -> Result<()> {
    let off = offset(blob)?;
    if u32be(blob, 0)? == 0x10002 {
        return Err(err("legacy fakesigning requires RSA metadata"));
    }
    range(blob, off + fill, 2)?;
    blob[4..off].fill(0);
    for counter in 0..=u16::MAX {
        blob[off + fill..off + fill + 2].copy_from_slice(&counter.to_be_bytes());
        if digest(&blob[off..])[0] == 0 {
            return Ok(());
        }
    }
    Err(err("legacy SHA-1 search exhausted"))
}
fn apply_policy(blob: &mut [u8], certs: &[u8], policy: SignaturePolicy, fill: usize) -> Result<()> {
    if signature_status(blob, certs)? == SignatureStatus::ValidRsa {
        return Ok(());
    }
    match policy {
        SignaturePolicy::Preserve => Err(err(
            "metadata lacks a valid RSA signature; explicit legacy fakesign is required",
        )),
        SignaturePolicy::LegacyFakesign => sign_legacy(blob, fill),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub id: u32,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct Wad {
    pub certificates: Vec<u8>,
    pub revocations: Vec<u8>,
    pub ticket: Ticket,
    pub tmd: Tmd,
    pub contents: Vec<Content>,
    pub footer: Vec<u8>,
}
impl Wad {
    /// Decode a standard channel WAD and check every plaintext content hash.
    pub fn decode(bytes: &[u8], common_key: &[u8; 16]) -> Result<Self> {
        if u32be(bytes, 0)? != 0x20 || range(bytes, 4, 2)? != b"Is" || u16be(bytes, 6)? != 0 {
            return Err(err("only standard Is channel WADs are supported"));
        }
        let mut cursor = 0x40;
        let mut sections = Vec::new();
        for off in [8, 12, 16, 20, 24, 28] {
            let size = u32be(bytes, off)? as usize;
            sections.push(range(bytes, cursor, size)?);
            cursor = aligned(
                cursor
                    .checked_add(size)
                    .ok_or_else(|| err("section overflow"))?,
                64,
            )?;
        }
        let logical_end = cursor - ((64 - sections[5].len() % 64) % 64);
        if bytes.len() < logical_end
            || bytes.len() > cursor + 64
            || bytes[logical_end..].iter().any(|v| *v != 0)
        {
            return Err(err("invalid WAD trailing data"));
        }
        let ticket = Ticket::parse(sections[2])?;
        let tmd = Tmd::parse(sections[3])?;
        validate_title(&ticket, &tmd)?;
        certificates(sections[0])?;
        let key = ticket.key(common_key)?;
        let mut position = 0;
        let mut contents = Vec::new();
        for record in tmd.records() {
            let size: usize = record
                .size
                .try_into()
                .map_err(|_| err("content size exceeds host bounds"))?;
            let encrypted_size = aligned(size, 16)?;
            let mut data = crypt(
                &key,
                content_iv(record.index),
                range(sections[4], position, encrypted_size)?,
                false,
            )?;
            if data[size..].iter().any(|v| *v != 0) {
                return Err(err("invalid content AES padding"));
            }
            data.truncate(size);
            if digest(&data) != record.sha1 {
                return Err(err(format!("content {:08X} SHA-1 mismatch", record.id)));
            }
            contents.push(Content {
                id: record.id,
                data,
            });
            position = aligned(
                position
                    .checked_add(encrypted_size)
                    .ok_or_else(|| err("content offset overflow"))?,
                64,
            )?;
        }
        if position != sections[4].len() {
            return Err(err("WAD data length does not match contents"));
        }
        Ok(Self {
            certificates: sections[0].to_vec(),
            revocations: sections[1].to_vec(),
            ticket,
            tmd,
            contents,
            footer: sections[5].to_vec(),
        })
    }
    /// Build all bytes in memory and verify the serialized result before success.
    pub fn encode(&self, common_key: &[u8; 16], policy: SignaturePolicy) -> Result<Vec<u8>> {
        validate_title(&self.ticket, &self.tmd)?;
        certificates(&self.certificates)?;
        let mut tmd = self.tmd.clone();
        tmd.update_records(&self.contents)?;
        let mut ticket = self.ticket.clone();
        apply_policy(&mut ticket.bytes, &self.certificates, policy, 0xb2)?;
        apply_policy(&mut tmd.bytes, &self.certificates, policy, 0x70)?;
        let key = ticket.key(common_key)?;
        let mut data = Vec::new();
        for record in tmd.records() {
            let content = self
                .contents
                .iter()
                .find(|c| c.id == record.id)
                .ok_or_else(|| err("missing content"))?;
            let mut padded = content.data.clone();
            padded.resize(aligned(padded.len(), 16)?, 0);
            data.extend(crypt(&key, content_iv(record.index), &padded, true)?);
            data.resize(aligned(data.len(), 64)?, 0);
        }
        let mut output = vec![0; 0x40];
        output[..4].copy_from_slice(&0x20u32.to_be_bytes());
        output[4..6].copy_from_slice(b"Is");
        for (n, section) in [
            &self.certificates[..],
            &self.revocations,
            &ticket.bytes,
            &tmd.bytes,
            &data,
            &self.footer,
        ]
        .into_iter()
        .enumerate()
        {
            output[8 + n * 4..12 + n * 4].copy_from_slice(&size32(section.len())?.to_be_bytes());
            output.extend_from_slice(section);
            output.resize(aligned(output.len(), 64)?, 0);
        }
        let decoded = Self::decode(&output, common_key)?;
        if decoded.contents.len() != self.contents.len()
            || self
                .contents
                .iter()
                .any(|content| !decoded.contents.contains(content))
        {
            return Err(err("serialized content verification failed"));
        }
        Ok(output)
    }
    pub fn signature_statuses(&self) -> Result<(SignatureStatus, SignatureStatus)> {
        Ok((
            signature_status(self.ticket.as_bytes(), &self.certificates)?,
            signature_status(self.tmd.as_bytes(), &self.certificates)?,
        ))
    }
}
fn validate_title(ticket: &Ticket, tmd: &Tmd) -> Result<()> {
    if ticket.title_id() != tmd.title_id() {
        return Err(err("ticket and TMD title IDs differ"));
    }
    // This host packager is for applications. System titles and boot2 require
    // separate preparation and recovery controls and are rejected here.
    if tmd.title_id() >> 32 != 0x00010001 {
        return Err(err(
            "only application channel titles (00010001) are supported",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
            .collect()
    }
    fn fixture() -> Wad {
        // Explicit format fixture. These certificates have no private key or
        // authority. They are used only to test legacy metadata and bounds.
        let mut cert = vec![0; 0x300];
        cert[..4].copy_from_slice(&0x10001u32.to_be_bytes());
        cert[0x140..0x144].copy_from_slice(b"Root");
        cert[0x180..0x184].copy_from_slice(&1u32.to_be_bytes());
        cert[0x184..0x186].copy_from_slice(b"CA");
        let id = 0x0001000148424345u64;
        let mut ticket = vec![0; 0x2a4];
        ticket[..4].copy_from_slice(&0x10001u32.to_be_bytes());
        ticket[0x140..0x147].copy_from_slice(b"Root-CA");
        ticket[0x1dc..0x1e4].copy_from_slice(&id.to_be_bytes());
        // The encrypted title key is a controlled arbitrary AES block.
        let mut tmd = vec![0; 0x208];
        tmd[..4].copy_from_slice(&0x10001u32.to_be_bytes());
        tmd[0x140..0x147].copy_from_slice(b"Root-CA");
        tmd[0x18c..0x194].copy_from_slice(&id.to_be_bytes());
        tmd[0x1de..0x1e0].copy_from_slice(&1u16.to_be_bytes());
        tmd[0x1e4..0x1e8].copy_from_slice(&7u32.to_be_bytes());
        tmd[0x1ea..0x1ec].copy_from_slice(&1u16.to_be_bytes());
        Wad {
            certificates: cert,
            revocations: vec![],
            ticket: Ticket::parse(&ticket).unwrap(),
            tmd: Tmd::parse(&tmd).unwrap(),
            contents: vec![Content {
                id: 7,
                data: b"abc".to_vec(),
            }],
            footer: b"footer".to_vec(),
        }
    }
    #[test]
    fn nist_aes_cbc_and_sha1_vectors() {
        let key: [u8; 16] = hex("2b7e151628aed2a6abf7158809cf4f3c").try_into().unwrap();
        let iv: [u8; 16] = hex("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
        let plain = hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
        let cipher = hex("7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b2");
        assert_eq!(crypt(&key, iv, &plain, true).unwrap(), cipher);
        assert_eq!(crypt(&key, iv, &cipher, false).unwrap(), plain);
        assert!(crypt(&key, iv, &[0; 15], true).is_err());
        assert_eq!(
            digest(b"abc").to_vec(),
            hex("a9993e364706816aba3e25717850c26c9cd0d89d")
        );
    }
    #[test]
    fn serialized_wad_checks_independent_sizes_hashes_and_legacy_status() {
        let bytes = fixture()
            .encode(&[0; 16], SignaturePolicy::LegacyFakesign)
            .unwrap();
        assert_eq!(bytes.len(), 0x8c0);
        assert_eq!(u32be(&bytes, 24).unwrap(), 64);
        let restored = Wad::decode(&bytes, &[0; 16]).unwrap();
        assert_eq!(restored.contents[0].data, b"abc");
        assert_eq!(restored.tmd.records()[0].size, 3);
        assert_eq!(
            restored.tmd.records()[0].sha1.to_vec(),
            hex("a9993e364706816aba3e25717850c26c9cd0d89d")
        );
        assert_eq!(
            restored.signature_statuses().unwrap(),
            (
                SignatureStatus::LegacyFakesigned,
                SignatureStatus::LegacyFakesigned
            )
        );
        assert!(
            restored
                .encode(&[0; 16], SignaturePolicy::Preserve)
                .is_err()
        );
    }
    #[test]
    fn corruption_wrong_key_and_truncation_fail() {
        let bytes = fixture()
            .encode(&[0; 16], SignaturePolicy::LegacyFakesign)
            .unwrap();
        for len in 0..bytes.len() - 64 {
            assert!(
                Wad::decode(&bytes[..len], &[0; 16]).is_err(),
                "length {len}"
            );
        }
        assert!(Wad::decode(&bytes, &[1; 16]).is_err());
        let mut broken = bytes.clone();
        broken[0x840] ^= 1;
        assert!(Wad::decode(&broken, &[0; 16]).is_err());
        let mut broken = bytes;
        broken[24..28].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(Wad::decode(&broken, &[0; 16]).is_err());
    }
    #[test]
    fn retitle_preserves_title_key_and_rejects_system_titles() {
        let mut wad = fixture();
        let original = wad.ticket.key(&[0; 16]).unwrap();
        let id = 0x000100014f484243;
        wad.ticket.retitle(id, &[0; 16]).unwrap();
        wad.tmd.retitle(id);
        wad.tmd.set_version(0x947);
        assert_eq!(wad.ticket.key(&[0; 16]).unwrap(), original);
        let bytes = wad
            .encode(&[0; 16], SignaturePolicy::LegacyFakesign)
            .unwrap();
        let restored = Wad::decode(&bytes, &[0; 16]).unwrap();
        assert_eq!(restored.ticket.title_id(), id);
        assert_eq!(restored.tmd.version(), 0x947);
        wad.tmd.retitle(0x100000002);
        assert!(
            wad.encode(&[0; 16], SignaturePolicy::LegacyFakesign)
                .is_err()
        );
        wad.ticket.retitle(0x100000002, &[0; 16]).unwrap();
        assert!(
            wad.encode(&[0; 16], SignaturePolicy::LegacyFakesign)
                .is_err()
        );
    }
    #[test]
    fn missing_duplicate_and_invalid_metadata_fail() {
        let mut wad = fixture();
        wad.contents.clear();
        assert!(
            wad.encode(&[0; 16], SignaturePolicy::LegacyFakesign)
                .is_err()
        );
        let mut wad = fixture();
        wad.contents.push(wad.contents[0].clone());
        assert!(
            wad.encode(&[0; 16], SignaturePolicy::LegacyFakesign)
                .is_err()
        );
        let mut tmd = fixture().tmd.as_bytes().to_vec();
        tmd[0x1e0..0x1e2].copy_from_slice(&1u16.to_be_bytes());
        assert!(Tmd::parse(&tmd).is_err());
        let mut ticket = fixture().ticket.as_bytes().to_vec();
        ticket[0x1f1] = 2;
        assert!(Ticket::parse(&ticket).is_err());
        for bytes in [&[][..], &[0; 4][..], &[0xff; 0x300][..]] {
            assert!(Ticket::parse(bytes).is_err());
            assert!(Tmd::parse(bytes).is_err());
        }
    }
}
