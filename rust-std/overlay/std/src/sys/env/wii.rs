//! Application environment. IOS and HBC supply no inherited environment block.
use crate::ffi::{OsStr, OsString};
use crate::sync::Mutex;
use crate::{fmt, io};

static ENVIRONMENT: Mutex<Vec<(OsString, OsString)>> = Mutex::new(Vec::new());

pub struct Env(crate::vec::IntoIter<(OsString, OsString)>);
impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.0.as_slice()).finish()
    }
}
impl Iterator for Env {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
fn valid_key(key: &OsStr) -> bool {
    let bytes = key.as_encoded_bytes();
    !bytes.is_empty() && !bytes.contains(&0) && !bytes.contains(&b'=')
}
pub fn env() -> Env {
    Env(ENVIRONMENT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .into_iter())
}
pub fn getenv(key: &OsStr) -> Option<OsString> {
    if !valid_key(key) {
        return None;
    }
    ENVIRONMENT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}
pub unsafe fn setenv(key: &OsStr, value: &OsStr) -> io::Result<()> {
    if !valid_key(key) || value.as_encoded_bytes().contains(&0) {
        return Err(io::Error::from_raw_os_error(22));
    }
    let mut state = ENVIRONMENT.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((_, old)) = state.iter_mut().find(|(name, _)| name == key) {
        *old = value.to_owned();
    } else {
        state.push((key.to_owned(), value.to_owned()));
    }
    Ok(())
}
pub unsafe fn unsetenv(key: &OsStr) -> io::Result<()> {
    if !valid_key(key) {
        return Err(io::Error::from_raw_os_error(22));
    }
    ENVIRONMENT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|(name, _)| name != key);
    Ok(())
}
