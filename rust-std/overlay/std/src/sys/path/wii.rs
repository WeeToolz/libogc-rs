use crate::ffi::OsStr;
use crate::path::{Path, PathBuf, Prefix};
use crate::sys::wii::fs_abi;
use crate::{env, io};
path_separator_bytes!(b'/');
pub const HAS_PREFIXES: bool = true;
pub const fn is_verbatim_sep(b: u8) -> bool {
    is_sep_byte(b)
}
pub fn parse_prefix(path: &OsStr) -> Option<Prefix<'_>> {
    let end = fs_abi::mount_prefix(path.as_encoded_bytes())?;
    // ASCII mount names are valid standalone OsStr bytes.
    Some(Prefix::WiiMount(unsafe {
        OsStr::from_encoded_bytes_unchecked(&path.as_encoded_bytes()[..end])
    }))
}
pub(crate) fn is_absolute(path: &Path) -> bool {
    path.has_root() && path.prefix().is_some()
}
pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if bytes.contains(&0) {
        return Err(io::Error::from_raw_os_error(22));
    }
    if bytes.contains(&b':') {
        // Wii has no separate current directory for each mount.
        fs_abi::validate_path(bytes).map_err(io::Error::from_raw_os_error)?;
    }
    let mut result = if path.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir()?
    };
    result.push(path);
    Ok(result)
}
