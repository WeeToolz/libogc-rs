//! Loader options shared by standalone and combined acceptance probes.
pub fn value(name: &str) -> Result<Option<String>, &'static str> {
    parse(
        name,
        std::env::args_os()
            .skip(1)
            .map(|arg| arg.as_encoded_bytes().to_vec()),
    )
}
fn parse(
    name: &str,
    arguments: impl Iterator<Item = Vec<u8>>,
) -> Result<Option<String>, &'static str> {
    let key = format!("--{name}");
    let prefix = format!("{key}=");
    let mut value = None;
    for argument in arguments {
        if argument == key.as_bytes() {
            return Err("option requires '=value'");
        }
        if let Some(bytes) = argument.strip_prefix(prefix.as_bytes()) {
            if bytes.is_empty() || value.is_some() {
                return Err("option is empty or repeated");
            }
            value =
                Some(String::from_utf8(bytes.to_vec()).map_err(|_| "option value is not UTF-8")?);
        }
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn options(input: &[&[u8]]) -> Result<Option<String>, &'static str> {
        parse("mode", input.iter().map(|arg| arg.to_vec()))
    }
    #[test]
    fn distinguishes_absence_named_values_and_other_options() {
        assert_eq!(options(&[]), Ok(None));
        assert_eq!(
            options(&[b"--mode=normal", b"--utc-offset-seconds=0"]),
            Ok(Some("normal".into()))
        );
        for invalid in [
            vec![b"--mode".as_slice()],
            vec![b"--mode="],
            vec![b"--mode=\xff"],
            vec![b"--mode=normal", b"--mode=panic"],
        ] {
            assert!(options(&invalid).is_err());
        }
    }
}
