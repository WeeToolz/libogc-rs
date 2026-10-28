use gc_wad::{Content, SignaturePolicy, Ticket, Tmd, Wad};
use std::{env, fs, io::Write, path::Path, process::ExitCode};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const USAGE: &str = "gc-wad channel-dol INPUT_DOL NEW_OUTPUT_DOL\ngc-wad pack TEMPLATE_DIR CONTENT_DIR KEY_FILE OUTPUT [--legacy-fakesign] [--title-id HEX16] [--version U16]\ngc-wad inspect|verify WAD KEY_FILE\ngc-wad unpack WAD KEY_FILE NEW_DIRECTORY\nKEY_FILE must contain the 16-byte common key selected by the ticket. No key is printed.\nLegacy fakesigning requires explicit selection. RSA checks use the supplied certificates; root trust is not checked.";
fn read(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()).into())
}
fn key(path: &str) -> Result<[u8; 16]> {
    read(path)?
        .try_into()
        .map_err(|_| "common key file must contain exactly 16 bytes".into())
}
/// Publish only a complete, verified file; do not replace an existing output.
fn publish(path: &Path, bytes: &[u8], validate: impl Fn(&[u8]) -> Result<()>) -> Result<()> {
    let mut part_name = path.as_os_str().to_os_string();
    part_name.push(".part");
    let part = std::path::PathBuf::from(part_name);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        let persisted = read(&part)?;
        if persisted != bytes {
            return Err("persisted WAD differs from prepared bytes".into());
        }
        validate(&persisted)?;
        fs::hard_link(&part, path)?;
        Ok(())
    })();
    drop(file);
    let cleanup = fs::remove_file(&part);
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => {
            Err(format!("{error}; partial-file cleanup failed: {cleanup}").into())
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => {
            Err(format!("WAD published, but partial-file cleanup failed: {error}").into())
        }
        (Ok(()), Ok(())) => Ok(()),
    }
}
fn summary(wad: &Wad, require_signature: bool) -> Result<()> {
    let (ticket, tmd) = wad.signature_statuses()?;
    if require_signature {
        verify_statuses(ticket, tmd)?;
    }
    println!(
        "Title {:016X}; IOS {:016X}; version {}; boot index {}; common key index {}",
        wad.tmd.title_id(),
        wad.tmd.ios_title_id(),
        wad.tmd.version(),
        wad.tmd.boot_index(),
        wad.ticket.common_key_index()
    );
    println!(
        "Ticket signature: {ticket:?}; TMD signature: {tmd:?}; certificate root trust: unchecked"
    );
    for r in wad.tmd.records() {
        println!(
            "Content {:08X}: index {}, type {:04X}, {} bytes, SHA-1 verified",
            r.id, r.index, r.kind, r.size
        );
    }
    Ok(())
}
/// Verification accepts a valid supplied-certificate signature or the reported
/// legacy form. It does not confer certificate root trust.
fn verify_statuses(ticket: gc_wad::SignatureStatus, tmd: gc_wad::SignatureStatus) -> Result<()> {
    use gc_wad::SignatureStatus::{LegacyFakesigned, ValidRsa};
    if [ticket, tmd]
        .iter()
        .all(|status| matches!(status, ValidRsa | LegacyFakesigned))
    {
        Ok(())
    } else {
        Err(format!("metadata verification failed: ticket {ticket:?}, TMD {tmd:?}").into())
    }
}
fn main_result() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("channel-dol") if args.len() == 3 => {
            let bytes = gc_elf::prepare_channel_dol(&read(&args[1])?)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&args[2])?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            println!("Channel DOL written: {} ({} bytes)", args[2], bytes.len());
        }
        Some("pack") if args.len() >= 5 => {
            let template = Path::new(&args[1]);
            let content = Path::new(&args[2]);
            let common = key(&args[3])?;
            let mut ticket = Ticket::parse(&read(template.join("cetk"))?)?;
            let mut tmd = Tmd::parse(&read(template.join("tmd"))?)?;
            let mut policy = SignaturePolicy::Preserve;
            let mut title = None;
            let mut version = None;
            let mut n = 5;
            while n < args.len() {
                match args[n].as_str() {
                    "--legacy-fakesign" if policy == SignaturePolicy::Preserve => {
                        policy = SignaturePolicy::LegacyFakesign
                    }
                    "--title-id" if title.is_none() => {
                        n += 1;
                        let value = args.get(n).ok_or("missing title ID")?;
                        if value.len() != 16 {
                            return Err("title ID must have 16 hex digits".into());
                        }
                        title = Some(u64::from_str_radix(value, 16)?);
                    }
                    "--version" if version.is_none() => {
                        n += 1;
                        version = Some(args.get(n).ok_or("missing version")?.parse::<u16>()?);
                    }
                    _ => return Err(format!("invalid or duplicate option: {}", args[n]).into()),
                }
                n += 1;
            }
            if let Some(id) = title {
                ticket.retitle(id, &common)?;
                tmd.retitle(id);
            }
            if let Some(version) = version {
                tmd.set_version(version);
            }
            let contents = tmd
                .records()
                .iter()
                .map(|r| {
                    Ok(Content {
                        id: r.id,
                        data: read(content.join(format!("{:08X}", r.id)))?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let wad = Wad {
                certificates: read(template.join("certs"))?,
                revocations: match fs::read(template.join("crl")) {
                    Ok(bytes) => bytes,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                    Err(error) => return Err(error.into()),
                },
                ticket,
                tmd,
                contents,
                footer: read(content.join("footer"))?,
            };
            let bytes = wad.encode(&common, policy)?;
            let verified = Wad::decode(&bytes, &common)?;
            publish(Path::new(&args[4]), &bytes, |data| {
                Wad::decode(data, &common)?;
                Ok(())
            })?;
            println!(
                "WAD written and read back: {} ({} bytes)",
                args[4],
                bytes.len()
            );
            summary(&verified, true)?;
        }
        Some("inspect" | "verify" | "unpack")
            if args.len() == if args[0] == "unpack" { 4 } else { 3 } =>
        {
            let wad = Wad::decode(&read(&args[1])?, &key(&args[2])?)?;
            summary(&wad, args[0] == "verify")?;
            if args[0] == "unpack" {
                let output = Path::new(&args[3]);
                fs::create_dir(output)?;
                for (name, bytes) in [
                    ("certs", wad.certificates.as_slice()),
                    ("crl", &wad.revocations),
                    ("cetk", wad.ticket.as_bytes()),
                    ("tmd", wad.tmd.as_bytes()),
                    ("footer", &wad.footer),
                ] {
                    fs::write(output.join(name), bytes)?;
                }
                for content in wad.contents {
                    fs::write(output.join(format!("{:08X}", content.id)), content.data)?;
                }
                println!("Unpacked to {}", output.display());
            }
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}
fn main() -> ExitCode {
    match main_result() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("gc-wad: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verification_rejects_invalid_and_unsupported_metadata() {
        use gc_wad::SignatureStatus::*;
        assert!(verify_statuses(ValidRsa, LegacyFakesigned).is_ok());
        for status in [Invalid, Unsupported] {
            assert!(verify_statuses(status, ValidRsa).is_err());
            assert!(verify_statuses(LegacyFakesigned, status).is_err());
        }
    }
    #[test]
    fn publication_checks_disk_bytes_and_preserves_existing_outputs() {
        let root = env::temp_dir().join(format!(
            "gc-wad-publish-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let output = root.join("package.wad");
        let check = |bytes: &[u8]| -> Result<()> {
            if bytes != b"prepared bytes" {
                return Err("unexpected persisted bytes".into());
            }
            Ok(())
        };
        publish(&output, b"prepared bytes", check).unwrap();
        assert_eq!(read(&output).unwrap(), b"prepared bytes");
        assert!(publish(&output, b"replacement", |_| Ok(())).is_err());
        assert_eq!(read(&output).unwrap(), b"prepared bytes");
        assert!(!root.join("package.wad.part").exists());
        let failed = root.join("failed.wad");
        assert!(
            publish(&failed, b"prepared bytes", |_| Err(
                "injected read-back validation failure".into()
            ))
            .is_err()
        );
        assert!(!failed.exists());
        assert!(!root.join("failed.wad.part").exists());
        fs::write(root.join("failed.wad.part"), b"existing work").unwrap();
        assert!(publish(&failed, b"prepared bytes", check).is_err());
        assert_eq!(
            read(root.join("failed.wad.part")).unwrap(),
            b"existing work"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
