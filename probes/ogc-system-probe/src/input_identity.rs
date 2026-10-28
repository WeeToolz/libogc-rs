/// Validate report arguments before an input event includes run identity.
pub fn validate_run_identity<'run, 'build>(
    run_id: &'run [u8],
    build_id: &'build [u8],
) -> Option<(&'run str, &'build str)> {
    let run_id = core::str::from_utf8(run_id).ok()?;
    let build_id = core::str::from_utf8(build_id).ok()?;
    if run_id.is_empty()
        || run_id.len() > 64
        || !run_id.is_ascii()
        || build_id.len() != 64
        || !build_id.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some((run_id, build_id))
}

#[cfg(test)]
mod tests {
    use super::validate_run_identity;

    #[test]
    fn accepts_bounded_ascii_run_id_and_sha256_hex() {
        let build_id = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            validate_run_identity(b"run-123", build_id),
            Some(("run-123", core::str::from_utf8(build_id).unwrap()))
        );
        assert!(validate_run_identity("run-雪".as_bytes(), build_id).is_none());
    }

    #[test]
    fn rejects_empty_or_oversized_run_id() {
        let build_id = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(validate_run_identity(b"", build_id).is_none());
        assert!(validate_run_identity(&[b'x'; 65], build_id).is_none());
    }

    #[test]
    fn rejects_invalid_utf8_and_malformed_build_hash() {
        let valid_hash = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(validate_run_identity(&[0xff], valid_hash).is_none());
        assert!(
            validate_run_identity(
                b"run",
                b"g123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            )
            .is_none()
        );
        assert!(validate_run_identity(b"run", &valid_hash[..63]).is_none());
    }
}
