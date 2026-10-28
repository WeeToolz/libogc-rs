//! Key and value storage shared by the production LWP TLS operations.

use core::ffi::c_void;
use core::ptr;

pub type TlsKey = u32;
pub type TlsDestructor = unsafe extern "C" fn(*mut c_void);
pub const TLS_KEY_CAPACITY: usize = 64;
pub const TLS_DESTRUCTOR_ITERATIONS: usize = 4;
const INDEX_BITS: u32 = 6;
const MAX_GENERATION: u32 = u32::MAX >> INDEX_BITS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TlsError {
    InvalidKey,
    KeyCapacity,
    DestructorLimit,
    ReentrantCleanup,
    ExceptionContext,
}

#[derive(Clone, Copy)]
struct KeySlot {
    generation: u32,
    used: bool,
    destructor: Option<TlsDestructor>,
}

#[derive(Clone, Copy)]
struct ValueSlot {
    generation: u32,
    value: *mut c_void,
}

pub(crate) struct TlsRegistry {
    keys: [KeySlot; TLS_KEY_CAPACITY],
}

#[derive(Clone, Copy)]
pub(crate) struct TlsValues {
    values: [ValueSlot; TLS_KEY_CAPACITY],
}

impl TlsRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            keys: [KeySlot {
                generation: 0,
                used: false,
                destructor: None,
            }; TLS_KEY_CAPACITY],
        }
    }

    pub(crate) fn create(&mut self, destructor: Option<TlsDestructor>) -> Result<TlsKey, TlsError> {
        for (index, slot) in self.keys.iter_mut().enumerate() {
            if !slot.used && slot.generation < MAX_GENERATION {
                slot.generation += 1;
                slot.used = true;
                slot.destructor = destructor;
                return Ok((slot.generation << INDEX_BITS) | index as u32);
            }
        }
        Err(TlsError::KeyCapacity)
    }

    fn index(&self, key: TlsKey) -> Result<usize, TlsError> {
        let index = (key & (TLS_KEY_CAPACITY as u32 - 1)) as usize;
        let slot = &self.keys[index];
        if !slot.used || slot.generation != key >> INDEX_BITS {
            Err(TlsError::InvalidKey)
        } else {
            Ok(index)
        }
    }

    pub(crate) fn destroy(&mut self, key: TlsKey) -> Result<(), TlsError> {
        let index = self.index(key)?;
        self.keys[index].used = false;
        self.keys[index].destructor = None;
        Ok(())
    }

    pub(crate) fn get(&self, values: &TlsValues, key: TlsKey) -> Result<*mut c_void, TlsError> {
        let index = self.index(key)?;
        let slot = &values.values[index];
        Ok(if slot.generation == key >> INDEX_BITS {
            slot.value
        } else {
            ptr::null_mut()
        })
    }

    pub(crate) fn set(
        &self,
        values: &mut TlsValues,
        key: TlsKey,
        value: *mut c_void,
    ) -> Result<(), TlsError> {
        let index = self.index(key)?;
        values.values[index] = ValueSlot {
            generation: key >> INDEX_BITS,
            value,
        };
        Ok(())
    }

    /// Remove the value before the caller invokes its destructor without a lock.
    pub(crate) fn take_destructor(
        &self,
        values: &mut TlsValues,
        index: usize,
    ) -> Option<(TlsDestructor, *mut c_void)> {
        let key = self.keys.get(index)?;
        let slot = values.values.get_mut(index)?;
        if !key.used || key.generation != slot.generation || slot.value.is_null() {
            return None;
        }
        let destructor = key.destructor?;
        let value = core::mem::take(&mut slot.value);
        Some((destructor, value))
    }

    pub(crate) fn has_destructors(&self, values: &TlsValues) -> bool {
        self.keys
            .iter()
            .zip(values.values.iter())
            .any(|(key, value)| {
                key.used
                    && key.generation == value.generation
                    && key.destructor.is_some()
                    && !value.value.is_null()
            })
    }
}

impl TlsValues {
    pub(crate) const fn new() -> Self {
        Self {
            values: [ValueSlot {
                generation: 0,
                value: ptr::null_mut(),
            }; TLS_KEY_CAPACITY],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn increment(value: *mut c_void) {
        // SAFETY: These tests store a live, uniquely accessed u32.
        unsafe { *value.cast::<u32>() += 1 };
    }

    #[test]
    fn values_are_isolated_and_deleted_keys_cannot_read_reused_storage() {
        let mut keys = TlsRegistry::new();
        let mut first = TlsValues::new();
        let mut second = TlsValues::new();
        let key = keys.create(None).unwrap();
        let mut one = 1u32;
        let pointer = (&mut one as *mut u32).cast();
        keys.set(&mut first, key, pointer).unwrap();
        assert_eq!(keys.get(&first, key), Ok(pointer));
        assert_eq!(keys.get(&second, key), Ok(ptr::null_mut()));
        keys.destroy(key).unwrap();
        assert_eq!(keys.get(&first, key), Err(TlsError::InvalidKey));
        assert_eq!(
            keys.set(&mut second, key, pointer),
            Err(TlsError::InvalidKey)
        );
        assert_eq!(keys.destroy(key), Err(TlsError::InvalidKey));
        let replacement = keys.create(None).unwrap();
        assert_ne!(replacement, key);
        assert_eq!(keys.get(&first, replacement), Ok(ptr::null_mut()));
        keys.set(&mut second, replacement, pointer).unwrap();
        assert_eq!(keys.get(&first, replacement), Ok(ptr::null_mut()));
    }

    #[test]
    fn exhausted_registry_keeps_existing_keys_and_can_reuse_deleted_slots() {
        let mut keys = TlsRegistry::new();
        let mut handles = [0; TLS_KEY_CAPACITY];
        for key in &mut handles {
            *key = keys.create(None).unwrap();
        }
        assert_eq!(keys.create(None), Err(TlsError::KeyCapacity));
        assert_eq!(
            keys.get(&TlsValues::new(), handles[63]),
            Ok(ptr::null_mut())
        );
        keys.destroy(handles[17]).unwrap();
        let replacement = keys.create(None).unwrap();
        assert_ne!(replacement, handles[17]);
        assert_eq!(keys.create(None), Err(TlsError::KeyCapacity));
    }

    #[test]
    fn generation_exhaustion_does_not_revive_old_handles() {
        let mut keys = TlsRegistry::new();
        keys.keys[0].generation = MAX_GENERATION;
        let key = keys.create(None).unwrap();
        assert_eq!(key & 63, 1);
        assert_eq!(
            keys.get(&TlsValues::new(), MAX_GENERATION << INDEX_BITS),
            Err(TlsError::InvalidKey)
        );
    }

    #[test]
    fn destructor_value_is_removed_before_callback_and_can_be_reinitialized() {
        let mut keys = TlsRegistry::new();
        let mut values = TlsValues::new();
        let mut calls = 0u32;
        let pointer = (&mut calls as *mut u32).cast();
        let key = keys.create(Some(increment)).unwrap();
        keys.set(&mut values, key, pointer).unwrap();
        assert!(keys.has_destructors(&values));
        let (callback, value) = keys
            .take_destructor(&mut values, (key & 63) as usize)
            .unwrap();
        assert_eq!(keys.get(&values, key), Ok(ptr::null_mut()));
        unsafe { callback(value) };
        assert_eq!(calls, 1);
        assert!(!keys.has_destructors(&values));
        keys.set(&mut values, key, pointer).unwrap();
        assert!(keys.has_destructors(&values));
        keys.destroy(key).unwrap();
        assert!(
            keys.take_destructor(&mut values, (key & 63) as usize)
                .is_none()
        );
    }
}
