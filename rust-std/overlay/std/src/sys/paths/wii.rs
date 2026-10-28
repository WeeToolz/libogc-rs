//! Wii application paths. Path lists use ';' because ':' names a FAT mount.
use crate::ffi::{OsStr, OsString};
use crate::path::{Path, PathBuf};
use crate::sync::Mutex;
use crate::sys::wii::fs_abi;
use crate::{fmt, io};

static CWD: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn current_exe() -> io::Result<PathBuf> {
    let path = crate::sys::args::args()
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::from_raw_os_error(2))?;
    fs_abi::validate_path(path.as_os_str().as_encoded_bytes())
        .map_err(io::Error::from_raw_os_error)?;
    Ok(path)
}
pub fn getcwd() -> io::Result<PathBuf> {
    let mut state = CWD.lock().unwrap_or_else(|p| p.into_inner());
    if state.is_none() {
        let exe = current_exe()?;
        let parent = exe
            .parent()
            .ok_or_else(|| io::Error::from_raw_os_error(2))?;
        // Do not cache an absent mount. The application can mount it and retry.
        *state = Some(crate::fs::canonicalize(parent)?);
    }
    let path = state.as_ref().unwrap().clone();
    if !crate::fs::metadata(&path)?.is_dir() {
        return Err(io::Error::from_raw_os_error(20));
    }
    Ok(path)
}
pub fn chdir(path: &Path) -> io::Result<()> {
    let mut state = CWD.lock().unwrap_or_else(|p| p.into_inner());
    // Resolve under this lock, so two relative chdir calls cannot use stale cwd.
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let base = match state.as_ref() {
            Some(base) => base.clone(),
            None => current_exe()?
                .parent()
                .ok_or_else(|| io::Error::from_raw_os_error(2))?
                .to_path_buf(),
        };
        base.join(path)
    };
    fs_abi::validate_path(absolute.as_os_str().as_encoded_bytes())
        .map_err(io::Error::from_raw_os_error)?;
    let resolved = crate::fs::canonicalize(absolute)?;
    if !crate::fs::metadata(&resolved)?.is_dir() {
        return Err(io::Error::from_raw_os_error(20));
    }
    *state = Some(resolved);
    Ok(())
}

pub struct SplitPathsRef<'a>(crate::slice::Split<'a, u8, fn(&u8) -> bool>);
pub struct SplitPaths<'a>(SplitPathsRef<'a>);
fn separator(byte: &u8) -> bool {
    *byte == b';'
}
pub fn split_paths_ref(value: &OsStr) -> Option<SplitPathsRef<'_>> {
    Some(SplitPathsRef(value.as_encoded_bytes().split(separator)))
}
pub fn split_paths(value: &OsStr) -> SplitPaths<'_> {
    SplitPaths(split_paths_ref(value).unwrap())
}
impl<'a> Iterator for SplitPathsRef<'a> {
    type Item = &'a Path;
    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|bytes| Path::new(unsafe { OsStr::from_encoded_bytes_unchecked(bytes) }))
    }
}
impl Iterator for SplitPaths<'_> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> {
        self.0.next().map(Path::to_path_buf)
    }
}
#[derive(Debug)]
pub struct JoinPathsError;
impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("path contains the Wii path-list separator ';'")
    }
}
impl crate::error::Error for JoinPathsError {}
pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    let mut bytes = Vec::new();
    for (index, path) in paths.enumerate() {
        let path = path.as_ref().as_encoded_bytes();
        if path.contains(&b';') {
            return Err(JoinPathsError);
        }
        if index != 0 {
            bytes.push(b';');
        }
        bytes.extend_from_slice(path);
    }
    Ok(unsafe { OsString::from_encoded_bytes_unchecked(bytes) })
}
pub fn home_dir() -> Option<PathBuf> {
    crate::sys::env::getenv(OsStr::new("HOME"))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}
pub fn temp_dir() -> PathBuf {
    if let Some(path) = crate::sys::env::getenv(OsStr::new("TMPDIR")).filter(|s| !s.is_empty()) {
        return PathBuf::from(path);
    }
    // Wii has no system temporary directory. Use the application's real cwd.
    getcwd().expect("Wii temp_dir needs TMPDIR or an available current directory")
}
