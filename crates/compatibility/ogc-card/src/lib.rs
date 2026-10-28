#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

mod metadata_submit_error;
pub use metadata_submit_error::CardMetadataSubmitError;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicI32, AtomicPtr, AtomicU8, Ordering};

use ogc_cache::{dc_flush_range, dc_invalidate_range};

pub use ogc_card_sys as sys;
pub use sys::*;

/// A validated memory-card slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum CardSlot {
    A = CARD_SLOTA,
    B = CARD_SLOTB,
}

impl CardSlot {
    pub const fn channel(self) -> i32 {
        self as i32
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// Error returned when an integer is not a valid memory-card slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCardSlot(pub i32);

impl TryFrom<i32> for CardSlot {
    type Error = InvalidCardSlot;

    fn try_from(channel: i32) -> Result<Self, Self::Error> {
        match channel {
            CARD_SLOTA => Ok(Self::A),
            CARD_SLOTB => Ok(Self::B),
            _ => Err(InvalidCardSlot(channel)),
        }
    }
}

/// Memory-card geometry reported by `probe_ex`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardProbeInfo {
    pub memory_size: i32,
    pub sector_size: i32,
}

#[repr(C, align(32))]
#[derive(Clone, Copy)]
struct CardCacheLine {
    _bytes: [u8; 32],
}

fn status_value<T>(status: i32, value: T) -> Result<T, i32> {
    if status == CARD_ERROR_READY {
        Ok(value)
    } else {
        Err(status)
    }
}

fn transfer_len(len: usize, offset: u32) -> Result<u32, i32> {
    let len = u32::try_from(len).map_err(|_| CARD_ERROR_FATAL_ERROR)?;
    if len == 0
        || len % CARD_READSIZE != 0
        || !offset.is_multiple_of(CARD_READSIZE)
        || offset.checked_add(len).is_none()
    {
        return Err(CARD_ERROR_FATAL_ERROR);
    }
    Ok(len)
}

fn aligned_storage(len: usize) -> Result<Vec<CardCacheLine>, i32> {
    let lines = len.checked_add(31).ok_or(CARD_ERROR_FATAL_ERROR)? / 32;
    let mut storage = Vec::new();
    storage
        .try_reserve_exact(lines)
        .map_err(|_| CARD_ERROR_FATAL_ERROR)?;
    storage.resize(lines, CardCacheLine { _bytes: [0; 32] });
    Ok(storage)
}

const REQUEST_IDLE: u8 = 0;
const REQUEST_RUNNING: u8 = 1;
const REQUEST_COMPLETE: u8 = 2;
const REQUEST_ABANDONED: u8 = 3;
const MOUNT_ACTIVE: u8 = 4;
const MOUNT_DETACHED: u8 = 5;

#[repr(C, align(32))]
struct CardOwnedWorkArea([u8; CARD_WORKAREA_SIZE as usize]);

static MOUNT_PHASE: [AtomicU8; 2] = [const { AtomicU8::new(REQUEST_IDLE) }; 2];
static MOUNT_RESULT: [AtomicI32; 2] = [const { AtomicI32::new(CARD_ERROR_BUSY) }; 2];
static mut SLOT_A_WORKAREA: CardOwnedWorkArea = CardOwnedWorkArea([0; CARD_WORKAREA_SIZE as usize]);
static mut SLOT_B_WORKAREA: CardOwnedWorkArea = CardOwnedWorkArea([0; CARD_WORKAREA_SIZE as usize]);

fn mount_workarea(slot: CardSlot) -> *mut c_void {
    // SAFETY: only a raw address is formed. Per-slot request state prevents a
    // second safe mount from aliasing the same static work area.
    unsafe {
        match slot {
            CardSlot::A => ptr::addr_of_mut!(SLOT_A_WORKAREA.0).cast::<c_void>(),
            CardSlot::B => ptr::addr_of_mut!(SLOT_B_WORKAREA.0).cast::<c_void>(),
        }
    }
}

unsafe extern "C" fn owned_mount_attach_callback(channel: i32, result: i32) {
    let Ok(slot) = CardSlot::try_from(channel) else {
        return;
    };
    let index = slot.index();
    MOUNT_RESULT[index].store(result, Ordering::Release);
    match MOUNT_PHASE[index].compare_exchange(
        REQUEST_RUNNING,
        REQUEST_COMPLETE,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(REQUEST_ABANDONED) => {
            if result == CARD_ERROR_READY {
                let _ = sys::unmount(channel);
            }
            MOUNT_PHASE[index].store(REQUEST_IDLE, Ordering::Release);
        }
        Err(_) => {}
    }
}

unsafe extern "C" fn owned_mount_detach_callback(channel: i32, result: i32) {
    let Ok(slot) = CardSlot::try_from(channel) else {
        return;
    };
    let index = slot.index();
    MOUNT_RESULT[index].store(result, Ordering::Release);
    let phase = MOUNT_PHASE[index].load(Ordering::Acquire);
    if matches!(phase, REQUEST_RUNNING | REQUEST_COMPLETE | MOUNT_ACTIVE) {
        MOUNT_PHASE[index].store(MOUNT_DETACHED, Ordering::Release);
    }
}

/// Polling state shared by owned asynchronous CARD operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardAsyncStatus {
    Pending,
    Complete(i32),
}

/// A pending asynchronous mount of a validated card slot.
///
/// Dropping this value before completion abandons the request. If mounting
/// later succeeds, the internal callback immediately unmounts the slot so the
/// static work area cannot remain attached without an owning handle.
pub struct CardMountRequest {
    slot: CardSlot,
    active: bool,
}

impl CardMountRequest {
    pub const fn slot(&self) -> CardSlot {
        self.slot
    }

    pub fn status(&self) -> CardAsyncStatus {
        let index = self.slot.index();
        match MOUNT_PHASE[index].load(Ordering::Acquire) {
            REQUEST_COMPLETE | MOUNT_DETACHED => {
                CardAsyncStatus::Complete(MOUNT_RESULT[index].load(Ordering::Acquire))
            }
            _ => CardAsyncStatus::Pending,
        }
    }

    /// Converts a completed request into a mounted-card owner.
    ///
    /// An incomplete request is returned unchanged so it can be polled again.
    pub fn try_complete(mut self) -> Result<CardMountCompletion, Self> {
        let index = self.slot.index();
        let phase = MOUNT_PHASE[index].load(Ordering::Acquire);
        if phase == MOUNT_DETACHED {
            let result = MOUNT_RESULT[index].load(Ordering::Acquire);
            MOUNT_PHASE[index].store(REQUEST_IDLE, Ordering::Release);
            self.active = false;
            return Ok(CardMountCompletion::Failed(result));
        }
        if phase != REQUEST_COMPLETE {
            return Err(self);
        }

        let result = MOUNT_RESULT[index].load(Ordering::Acquire);
        if result == CARD_ERROR_READY {
            if MOUNT_PHASE[index]
                .compare_exchange(
                    REQUEST_COMPLETE,
                    MOUNT_ACTIVE,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_err()
            {
                return Err(self);
            }
            self.active = false;
            Ok(CardMountCompletion::Mounted(MountedCard {
                slot: self.slot,
                active: true,
            }))
        } else {
            MOUNT_PHASE[index].store(REQUEST_IDLE, Ordering::Release);
            self.active = false;
            Ok(CardMountCompletion::Failed(result))
        }
    }
}

impl Drop for CardMountRequest {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let index = self.slot.index();
        match MOUNT_PHASE[index].compare_exchange(
            REQUEST_RUNNING,
            REQUEST_ABANDONED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {}
            Err(REQUEST_COMPLETE) => {
                if MOUNT_RESULT[index].load(Ordering::Acquire) == CARD_ERROR_READY {
                    let _ = sys::unmount(self.slot.channel());
                }
                MOUNT_PHASE[index].store(REQUEST_IDLE, Ordering::Release);
            }
            Err(MOUNT_DETACHED) => {
                MOUNT_PHASE[index].store(REQUEST_IDLE, Ordering::Release);
            }
            Err(_) => {}
        }
    }
}

/// Completion value produced by [`CardMountRequest::try_complete`].
pub enum CardMountCompletion {
    Mounted(MountedCard),
    Failed(i32),
}

/// Owns a mounted slot and the wrapper's static aligned CARD work area.
///
/// Dropping the handle unmounts an attached card. A hardware detach marks the
/// handle inactive through the internal detach callback; dropping it then only
/// releases the wrapper's slot reservation.
pub struct MountedCard {
    slot: CardSlot,
    active: bool,
}

impl MountedCard {
    pub const fn slot(&self) -> CardSlot {
        self.slot
    }

    pub fn is_attached(&self) -> bool {
        MOUNT_PHASE[self.slot.index()].load(Ordering::Acquire) == MOUNT_ACTIVE
    }

    pub fn detach_result(&self) -> Option<i32> {
        (MOUNT_PHASE[self.slot.index()].load(Ordering::Acquire) == MOUNT_DETACHED)
            .then(|| MOUNT_RESULT[self.slot.index()].load(Ordering::Acquire))
    }

    /// Explicitly unmounts the slot and consumes the owner.
    pub fn unmount(mut self) -> i32 {
        let result = release_mounted_slot(self.slot);
        self.active = false;
        result
    }
}

impl Drop for MountedCard {
    fn drop(&mut self) {
        if self.active {
            let _ = release_mounted_slot(self.slot);
        }
    }
}

fn release_mounted_slot(slot: CardSlot) -> i32 {
    let index = slot.index();
    let phase = MOUNT_PHASE[index].load(Ordering::Acquire);
    let result = if phase == MOUNT_ACTIVE {
        sys::unmount(slot.channel())
    } else if phase == MOUNT_DETACHED {
        MOUNT_RESULT[index].load(Ordering::Acquire)
    } else {
        CARD_ERROR_READY
    };
    MOUNT_PHASE[index].store(REQUEST_IDLE, Ordering::Release);
    result
}

/// Begins mounting a card using wrapper-owned, 32-byte-aligned static storage.
pub fn mount_async_owned(slot: CardSlot) -> Result<CardMountRequest, i32> {
    let index = slot.index();
    MOUNT_PHASE[index]
        .compare_exchange(
            REQUEST_IDLE,
            REQUEST_RUNNING,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .map_err(|_| CARD_ERROR_BUSY)?;
    MOUNT_RESULT[index].store(CARD_ERROR_BUSY, Ordering::Release);

    // SAFETY: each slot has a distinct static, 32-byte-aligned work area that
    // remains allocated for the program lifetime. Internal callbacks use only
    // per-slot atomic state and remain valid for the entire mount lifetime.
    let status = unsafe {
        sys::mount_async(
            slot.channel(),
            mount_workarea(slot),
            Some(owned_mount_detach_callback),
            Some(owned_mount_attach_callback),
        )
    };
    if status != CARD_ERROR_READY {
        MOUNT_PHASE[index].store(REQUEST_IDLE, Ordering::Release);
        return Err(status);
    }

    Ok(CardMountRequest { slot, active: true })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CardTransferKind {
    Read,
    Write,
}

struct CardTransferPayload {
    file: CardFile,
    storage: Vec<CardCacheLine>,
    bytes: Vec<u8>,
}

struct CardTransferState {
    phase: AtomicU8,
    result: AtomicI32,
    kind: CardTransferKind,
    buffer: *mut c_void,
    len: u32,
    payload: UnsafeCell<CardTransferPayload>,
}

// SAFETY: the asynchronous worker accesses only the aligned transfer buffer
// and atomic completion fields. The payload is extracted only after the
// callback publishes `REQUEST_COMPLETE`, or destroyed by the callback after a
// request has atomically transitioned to `REQUEST_ABANDONED`.
unsafe impl Send for CardTransferState {}
unsafe impl Sync for CardTransferState {}

static TRANSFER_STATE: [AtomicPtr<CardTransferState>; 2] =
    [const { AtomicPtr::new(ptr::null_mut()) }; 2];

unsafe extern "C" fn owned_transfer_callback(channel: i32, result: i32) {
    let Ok(slot) = CardSlot::try_from(channel) else {
        return;
    };
    let index = slot.index();
    let pointer = TRANSFER_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return;
    }
    // SAFETY: a non-null global pointer owns a live boxed state until either a
    // completed request consumes it or this callback observes abandonment.
    let state = unsafe { &*pointer };
    if state.kind == CardTransferKind::Read && result == CARD_ERROR_READY {
        // SAFETY: the callback runs after the backend has finished writing the
        // complete aligned DMA allocation retained by `state`.
        unsafe { dc_invalidate_range(state.buffer.cast_const(), state.len) };
    }
    state.result.store(result, Ordering::Release);
    match state.phase.compare_exchange(
        REQUEST_RUNNING,
        REQUEST_COMPLETE,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(REQUEST_ABANDONED) => {
            if TRANSFER_STATE[index]
                .compare_exchange(
                    pointer,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // SAFETY: the successful exchange transfers the global Box
                // ownership to this callback exactly once.
                unsafe { drop(Box::from_raw(pointer)) };
            }
        }
        Err(_) => {}
    }
}

fn zeroed_bytes(len: usize) -> Result<Vec<u8>, i32> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| CARD_ERROR_FATAL_ERROR)?;
    bytes.resize(len, 0);
    Ok(bytes)
}

fn submit_owned_transfer(
    kind: CardTransferKind,
    file: CardFile,
    bytes: Vec<u8>,
    offset: u32,
) -> Result<CardSlot, CardTransferPayloadError> {
    let slot = match CardSlot::try_from(file.chn) {
        Ok(slot) => slot,
        Err(_) => {
            return Err(CardTransferPayloadError {
                status: CARD_ERROR_NOCARD,
                file,
                bytes,
            });
        }
    };
    let len = match transfer_len(bytes.len(), offset) {
        Ok(len) => len,
        Err(status) => {
            return Err(CardTransferPayloadError {
                status,
                file,
                bytes,
            });
        }
    };
    let mut storage = match aligned_storage(bytes.len()) {
        Ok(storage) => storage,
        Err(status) => {
            return Err(CardTransferPayloadError {
                status,
                file,
                bytes,
            });
        }
    };
    let buffer = storage.as_mut_ptr().cast::<c_void>();

    if kind == CardTransferKind::Write {
        // SAFETY: the aligned destination covers the complete input and cannot
        // overlap the separately allocated Vec storage.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast::<u8>(), bytes.len());
            dc_flush_range(buffer.cast_const(), len);
        }
    } else {
        // SAFETY: flushing the live aligned allocation prevents dirty cache
        // lines from later overwriting DMA output.
        unsafe { dc_flush_range(buffer.cast_const(), len) };
    }

    let state = Box::new(CardTransferState {
        phase: AtomicU8::new(REQUEST_RUNNING),
        result: AtomicI32::new(CARD_ERROR_BUSY),
        kind,
        buffer,
        len,
        payload: UnsafeCell::new(CardTransferPayload {
            file,
            storage,
            bytes,
        }),
    });
    let pointer = Box::into_raw(state);
    let index = slot.index();
    if TRANSFER_STATE[index]
        .compare_exchange(
            ptr::null_mut(),
            pointer,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        // SAFETY: the global rejected the pointer, so ownership never left
        // this function.
        let state = unsafe { *Box::from_raw(pointer) };
        let payload = state.payload.into_inner();
        return Err(CardTransferPayloadError {
            status: CARD_ERROR_BUSY,
            file: payload.file,
            bytes: payload.bytes,
        });
    }

    // SAFETY: the global Box keeps `file` and the aligned buffer stationary
    // until callback completion. Only one owned request is admitted per slot.
    let submit_status = unsafe {
        let payload = &mut *(*pointer).payload.get();
        match kind {
            CardTransferKind::Read => sys::read_async(
                &mut payload.file,
                buffer,
                len,
                offset,
                Some(owned_transfer_callback),
            ),
            CardTransferKind::Write => sys::write_async(
                &mut payload.file,
                buffer,
                len,
                offset,
                Some(owned_transfer_callback),
            ),
        }
    };
    if submit_status != CARD_ERROR_READY {
        let removed = TRANSFER_STATE[index]
            .compare_exchange(
                pointer,
                ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if removed {
            // SAFETY: failed submission does not invoke the callback, and the
            // successful exchange restores unique Box ownership here.
            let state = unsafe { *Box::from_raw(pointer) };
            let payload = state.payload.into_inner();
            return Err(CardTransferPayloadError {
                status: submit_status,
                file: payload.file,
                bytes: payload.bytes,
            });
        }
    }

    Ok(slot)
}

struct CardTransferPayloadError {
    status: i32,
    file: CardFile,
    bytes: Vec<u8>,
}

fn transfer_status(slot: CardSlot, kind: CardTransferKind) -> CardAsyncStatus {
    let pointer = TRANSFER_STATE[slot.index()].load(Ordering::Acquire);
    if pointer.is_null() {
        return CardAsyncStatus::Complete(CARD_ERROR_FATAL_ERROR);
    }
    // SAFETY: an active request keeps the global Box installed until its
    // completion is consumed or its Drop path abandons it.
    let state = unsafe { &*pointer };
    if state.kind != kind {
        return CardAsyncStatus::Complete(CARD_ERROR_BUSY);
    }
    if state.phase.load(Ordering::Acquire) == REQUEST_COMPLETE {
        CardAsyncStatus::Complete(state.result.load(Ordering::Acquire))
    } else {
        CardAsyncStatus::Pending
    }
}

fn take_transfer(slot: CardSlot, kind: CardTransferKind) -> Option<(i32, CardTransferPayload)> {
    let index = slot.index();
    let pointer = TRANSFER_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return None;
    }
    // SAFETY: the pointer remains globally owned while installed.
    let state = unsafe { &*pointer };
    if state.kind != kind || state.phase.load(Ordering::Acquire) != REQUEST_COMPLETE {
        return None;
    }
    TRANSFER_STATE[index]
        .compare_exchange(
            pointer,
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .ok()?;
    // SAFETY: completion callbacks never touch the state after publishing
    // `REQUEST_COMPLETE`, and the successful exchange transfers Box ownership.
    let state = unsafe { *Box::from_raw(pointer) };
    let result = state.result.load(Ordering::Acquire);
    Some((result, state.payload.into_inner()))
}

fn abandon_transfer(slot: CardSlot, kind: CardTransferKind) {
    let index = slot.index();
    let pointer = TRANSFER_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return;
    }
    // SAFETY: the pointer remains valid while globally installed.
    let state = unsafe { &*pointer };
    if state.kind != kind {
        return;
    }
    match state.phase.compare_exchange(
        REQUEST_RUNNING,
        REQUEST_ABANDONED,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(REQUEST_COMPLETE) => {
            if TRANSFER_STATE[index]
                .compare_exchange(
                    pointer,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // SAFETY: the callback has finished and the exchange transfers
                // unique Box ownership to this Drop path.
                unsafe { drop(Box::from_raw(pointer)) };
            }
        }
        Err(_) => {}
    }
}

/// Submission failure for an owned asynchronous read.
pub struct CardReadSubmitError {
    status: i32,
    file: CardFile,
}

impl CardReadSubmitError {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn file(&self) -> &CardFile {
        &self.file
    }

    pub fn into_file(self) -> CardFile {
        self.file
    }
}

/// Submission failure for an owned asynchronous write.
pub struct CardWriteSubmitError {
    status: i32,
    file: CardFile,
    bytes: Vec<u8>,
}

impl CardWriteSubmitError {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn file(&self) -> &CardFile {
        &self.file
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_parts(self) -> (i32, CardFile, Vec<u8>) {
        (self.status, self.file, self.bytes)
    }
}

/// Completed owned asynchronous read.
pub struct CardReadCompletion {
    status: i32,
    file: CardFile,
    bytes: Vec<u8>,
}

impl CardReadCompletion {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn file(&self) -> &CardFile {
        &self.file
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_parts(self) -> (i32, CardFile, Vec<u8>) {
        (self.status, self.file, self.bytes)
    }
}

/// Completed owned asynchronous write.
pub struct CardWriteCompletion {
    status: i32,
    file: CardFile,
    bytes: Vec<u8>,
}

impl CardWriteCompletion {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn file(&self) -> &CardFile {
        &self.file
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_parts(self) -> (i32, CardFile, Vec<u8>) {
        (self.status, self.file, self.bytes)
    }
}

/// Pollable owned asynchronous CARD read.
pub struct CardReadRequest {
    slot: CardSlot,
    active: bool,
}

impl CardReadRequest {
    pub fn status(&self) -> CardAsyncStatus {
        transfer_status(self.slot, CardTransferKind::Read)
    }

    pub fn try_complete(mut self) -> Result<CardReadCompletion, Self> {
        let Some((status, mut payload)) = take_transfer(self.slot, CardTransferKind::Read) else {
            return Err(self);
        };
        if status == CARD_ERROR_READY {
            // SAFETY: callback completion invalidated the aligned DMA buffer;
            // it contains at least `bytes.len()` initialized bytes and cannot
            // overlap the separately allocated destination Vec.
            unsafe {
                ptr::copy_nonoverlapping(
                    payload.storage.as_ptr().cast::<u8>(),
                    payload.bytes.as_mut_ptr(),
                    payload.bytes.len(),
                );
            }
        }
        self.active = false;
        Ok(CardReadCompletion {
            status,
            file: payload.file,
            bytes: payload.bytes,
        })
    }
}

impl Drop for CardReadRequest {
    fn drop(&mut self) {
        if self.active {
            abandon_transfer(self.slot, CardTransferKind::Read);
        }
    }
}

/// Pollable owned asynchronous CARD write.
pub struct CardWriteRequest {
    slot: CardSlot,
    active: bool,
}

impl CardWriteRequest {
    pub fn status(&self) -> CardAsyncStatus {
        transfer_status(self.slot, CardTransferKind::Write)
    }

    pub fn try_complete(mut self) -> Result<CardWriteCompletion, Self> {
        let Some((status, payload)) = take_transfer(self.slot, CardTransferKind::Write) else {
            return Err(self);
        };
        self.active = false;
        Ok(CardWriteCompletion {
            status,
            file: payload.file,
            bytes: payload.bytes,
        })
    }
}

impl Drop for CardWriteRequest {
    fn drop(&mut self) {
        if self.active {
            abandon_transfer(self.slot, CardTransferKind::Write);
        }
    }
}

/// Starts a read into wrapper-owned aligned storage.
///
/// The returned request owns a stable copy of `file`, the DMA allocation, and
/// the eventual output Vec until completion is consumed or abandoned.
pub fn read_async_owned(
    file: CardFile,
    len: usize,
    offset: u32,
) -> Result<CardReadRequest, CardReadSubmitError> {
    if CardSlot::try_from(file.chn).is_err() {
        return Err(CardReadSubmitError {
            status: CARD_ERROR_NOCARD,
            file,
        });
    }
    if let Err(status) = transfer_len(len, offset) {
        return Err(CardReadSubmitError { status, file });
    }
    let bytes = zeroed_bytes(len).map_err(|status| CardReadSubmitError { status, file })?;
    match submit_owned_transfer(CardTransferKind::Read, file, bytes, offset) {
        Ok(slot) => Ok(CardReadRequest { slot, active: true }),
        Err(error) => Err(CardReadSubmitError {
            status: error.status,
            file: error.file,
        }),
    }
}

/// Starts a write from wrapper-owned bytes copied into aligned DMA storage.
pub fn write_async_owned(
    file: CardFile,
    bytes: Vec<u8>,
    offset: u32,
) -> Result<CardWriteRequest, CardWriteSubmitError> {
    match submit_owned_transfer(CardTransferKind::Write, file, bytes, offset) {
        Ok(slot) => Ok(CardWriteRequest { slot, active: true }),
        Err(error) => Err(CardWriteSubmitError {
            status: error.status,
            file: error.file,
            bytes: error.bytes,
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CardFileOperationKind {
    Create,
    Delete,
}

struct CardFileOperationPayload {
    filename: Vec<u8>,
    file: CardFile,
}

struct CardFileOperationState {
    kind: CardFileOperationKind,
    phase: AtomicU8,
    result: AtomicI32,
    payload: UnsafeCell<CardFileOperationPayload>,
}

// SAFETY: the payload is only mutated by the CARD worker before the callback
// publishes `REQUEST_COMPLETE`. Safe request handles cannot access it until an
// acquire operation observes completion. Per-slot admission prevents another
// safe request from accessing the same state concurrently.
unsafe impl Sync for CardFileOperationState {}

static FILE_OPERATION_STATE: [AtomicPtr<CardFileOperationState>; 2] =
    [const { AtomicPtr::new(ptr::null_mut()) }; 2];

fn owned_card_filename(filename: &[u8]) -> Result<Vec<u8>, i32> {
    if filename.len() > CARD_FILENAMELEN {
        return Err(CARD_ERROR_NAMETOOLONG);
    }
    if filename.contains(&0) {
        return Err(CARD_ERROR_FATAL_ERROR);
    }

    let mut owned = Vec::new();
    owned
        .try_reserve_exact(filename.len().saturating_add(1))
        .map_err(|_| CARD_ERROR_FATAL_ERROR)?;
    owned.extend_from_slice(filename);
    owned.push(0);
    Ok(owned)
}

unsafe extern "C" fn owned_file_operation_callback(channel: i32, result: i32) {
    let Ok(slot) = CardSlot::try_from(channel) else {
        return;
    };
    let index = slot.index();
    let pointer = FILE_OPERATION_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return;
    }

    // SAFETY: an installed state remains allocated until completion is
    // consumed or an abandoned callback removes it below.
    let state = unsafe { &*pointer };
    state.result.store(result, Ordering::Release);
    match state.phase.compare_exchange(
        REQUEST_RUNNING,
        REQUEST_COMPLETE,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(REQUEST_ABANDONED) => {
            if FILE_OPERATION_STATE[index]
                .compare_exchange(
                    pointer,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // SAFETY: the callback has finished all payload access and the
                // successful exchange transfers unique Box ownership here.
                unsafe { drop(Box::from_raw(pointer)) };
            }
        }
        Err(_) => {}
    }
}

fn submit_owned_file_operation(
    kind: CardFileOperationKind,
    slot: CardSlot,
    filename: &[u8],
    size: u32,
) -> Result<(), i32> {
    let filename = owned_card_filename(filename)?;
    let state = Box::new(CardFileOperationState {
        kind,
        phase: AtomicU8::new(REQUEST_RUNNING),
        result: AtomicI32::new(CARD_ERROR_BUSY),
        payload: UnsafeCell::new(CardFileOperationPayload {
            filename,
            file: CardFile::default(),
        }),
    });
    let pointer = Box::into_raw(state);
    let index = slot.index();
    if FILE_OPERATION_STATE[index]
        .compare_exchange(
            ptr::null_mut(),
            pointer,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        // SAFETY: installation failed, so ownership never left this function.
        unsafe { drop(Box::from_raw(pointer)) };
        return Err(CARD_ERROR_BUSY);
    }

    // SAFETY: the installed Box keeps the filename allocation and output file
    // stationary until callback completion. Only the CARD worker writes the
    // output file before publishing completion through the callback.
    let submit_status = unsafe {
        let payload = &mut *(*pointer).payload.get();
        match kind {
            CardFileOperationKind::Create => sys::create_async(
                slot.channel(),
                payload.filename.as_ptr().cast(),
                size,
                &mut payload.file,
                Some(owned_file_operation_callback),
            ),
            CardFileOperationKind::Delete => sys::delete_async(
                slot.channel(),
                payload.filename.as_ptr().cast(),
                Some(owned_file_operation_callback),
            ),
        }
    };

    if submit_status != CARD_ERROR_READY {
        if FILE_OPERATION_STATE[index]
            .compare_exchange(
                pointer,
                ptr::null_mut(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            // SAFETY: failed submission does not retain the pointers or invoke
            // the callback, and the exchange restores unique ownership.
            unsafe { drop(Box::from_raw(pointer)) };
        }
        return Err(submit_status);
    }

    Ok(())
}

fn file_operation_status(slot: CardSlot, kind: CardFileOperationKind) -> CardAsyncStatus {
    let pointer = FILE_OPERATION_STATE[slot.index()].load(Ordering::Acquire);
    if pointer.is_null() {
        return CardAsyncStatus::Complete(CARD_ERROR_FATAL_ERROR);
    }
    // SAFETY: an active request keeps the state installed. A state is only
    // reclaimed by a callback after its request has been abandoned.
    let state = unsafe { &*pointer };
    if state.kind != kind {
        return CardAsyncStatus::Complete(CARD_ERROR_BUSY);
    }
    if state.phase.load(Ordering::Acquire) == REQUEST_COMPLETE {
        CardAsyncStatus::Complete(state.result.load(Ordering::Acquire))
    } else {
        CardAsyncStatus::Pending
    }
}

fn take_file_operation(
    slot: CardSlot,
    kind: CardFileOperationKind,
) -> Option<(i32, CardFileOperationPayload)> {
    let index = slot.index();
    let pointer = FILE_OPERATION_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return None;
    }
    // SAFETY: the pointer remains globally owned while installed.
    let state = unsafe { &*pointer };
    if state.kind != kind || state.phase.load(Ordering::Acquire) != REQUEST_COMPLETE {
        return None;
    }
    FILE_OPERATION_STATE[index]
        .compare_exchange(
            pointer,
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .ok()?;
    // SAFETY: the callback performs no state access after publishing complete,
    // and the successful exchange transfers unique Box ownership here.
    let state = unsafe { *Box::from_raw(pointer) };
    let result = state.result.load(Ordering::Acquire);
    Some((result, state.payload.into_inner()))
}

fn abandon_file_operation(slot: CardSlot, kind: CardFileOperationKind) {
    let index = slot.index();
    let pointer = FILE_OPERATION_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return;
    }
    // SAFETY: the pointer remains valid while installed.
    let state = unsafe { &*pointer };
    if state.kind != kind {
        return;
    }
    match state.phase.compare_exchange(
        REQUEST_RUNNING,
        REQUEST_ABANDONED,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(REQUEST_COMPLETE) => {
            if FILE_OPERATION_STATE[index]
                .compare_exchange(
                    pointer,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // SAFETY: completion has been published and the exchange
                // transfers unique Box ownership to this Drop path.
                unsafe { drop(Box::from_raw(pointer)) };
            }
        }
        Err(_) => {}
    }
}

/// Completion of an owned asynchronous CARD file creation.
pub struct CardCreateCompletion {
    status: i32,
    file: CardFile,
}

impl CardCreateCompletion {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub fn file(&self) -> Option<&CardFile> {
        (self.status == CARD_ERROR_READY).then_some(&self.file)
    }

    pub fn into_result(self) -> Result<CardFile, i32> {
        if self.status == CARD_ERROR_READY {
            Ok(self.file)
        } else {
            Err(self.status)
        }
    }

    pub fn into_parts(self) -> (i32, CardFile) {
        (self.status, self.file)
    }
}

/// Completion of an owned asynchronous CARD file deletion.
pub struct CardDeleteCompletion {
    status: i32,
}

impl CardDeleteCompletion {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub fn into_result(self) -> Result<(), i32> {
        if self.status == CARD_ERROR_READY {
            Ok(())
        } else {
            Err(self.status)
        }
    }
}

/// Pollable owned asynchronous CARD file-creation request.
pub struct CardCreateRequest {
    slot: CardSlot,
    active: bool,
}

impl CardCreateRequest {
    pub const fn slot(&self) -> CardSlot {
        self.slot
    }

    pub fn status(&self) -> CardAsyncStatus {
        file_operation_status(self.slot, CardFileOperationKind::Create)
    }

    pub fn try_complete(mut self) -> Result<CardCreateCompletion, Self> {
        let Some((status, payload)) = take_file_operation(self.slot, CardFileOperationKind::Create)
        else {
            return Err(self);
        };
        self.active = false;
        Ok(CardCreateCompletion {
            status,
            file: payload.file,
        })
    }
}

impl Drop for CardCreateRequest {
    fn drop(&mut self) {
        if self.active {
            abandon_file_operation(self.slot, CardFileOperationKind::Create);
        }
    }
}

/// Pollable owned asynchronous CARD file-deletion request.
pub struct CardDeleteRequest {
    slot: CardSlot,
    active: bool,
}

impl CardDeleteRequest {
    pub const fn slot(&self) -> CardSlot {
        self.slot
    }

    pub fn status(&self) -> CardAsyncStatus {
        file_operation_status(self.slot, CardFileOperationKind::Delete)
    }

    pub fn try_complete(mut self) -> Result<CardDeleteCompletion, Self> {
        let Some((status, _payload)) =
            take_file_operation(self.slot, CardFileOperationKind::Delete)
        else {
            return Err(self);
        };
        self.active = false;
        Ok(CardDeleteCompletion { status })
    }
}

impl Drop for CardDeleteRequest {
    fn drop(&mut self) {
        if self.active {
            abandon_file_operation(self.slot, CardFileOperationKind::Delete);
        }
    }
}

/// Starts file creation while owning the filename and output `CardFile` until
/// the CARD callback completes.
///
/// `filename` is copied and may contain any non-NUL bytes accepted by CARD, up
/// to [`CARD_FILENAMELEN`] bytes. One safe create/delete request may be active
/// per slot.
pub fn create_async_owned(
    slot: CardSlot,
    filename: &[u8],
    size: u32,
) -> Result<CardCreateRequest, i32> {
    submit_owned_file_operation(CardFileOperationKind::Create, slot, filename, size)?;
    Ok(CardCreateRequest { slot, active: true })
}

/// Starts file deletion while owning the NUL-terminated filename until the
/// CARD callback completes.
pub fn delete_async_owned(slot: CardSlot, filename: &[u8]) -> Result<CardDeleteRequest, i32> {
    submit_owned_file_operation(CardFileOperationKind::Delete, slot, filename, 0)?;
    Ok(CardDeleteRequest { slot, active: true })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CardMetadataOperationKind {
    CreateEntry,
    DeleteEntry,
    SetStatus,
}

struct CardMetadataPayload {
    entry: CardDir,
    file: CardFile,
    stat: CardStat,
    fileno: i32,
}

struct CardMetadataState {
    kind: CardMetadataOperationKind,
    phase: AtomicU8,
    result: AtomicI32,
    payload: UnsafeCell<CardMetadataPayload>,
}

// SAFETY: the CARD worker is the only code that may mutate `payload` before
// the callback publishes `REQUEST_COMPLETE`. Request handles inspect only
// atomics until an acquire operation observes completion, after which the
// state is removed from the global slot before its payload is read.
unsafe impl Send for CardMetadataState {}
unsafe impl Sync for CardMetadataState {}

static METADATA_STATE: [AtomicPtr<CardMetadataState>; 2] =
    [const { AtomicPtr::new(ptr::null_mut()) }; 2];

fn validate_owned_dir_entry(entry: &CardDir) -> Result<(), i32> {
    if entry.filename.iter().all(|byte| *byte != 0) {
        return Err(CARD_ERROR_NAMETOOLONG);
    }
    Ok(())
}

unsafe extern "C" fn owned_metadata_callback(channel: i32, result: i32) {
    let Ok(slot) = CardSlot::try_from(channel) else {
        return;
    };
    let index = slot.index();
    let pointer = METADATA_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return;
    }

    // SAFETY: an installed pointer owns a live boxed state until completion is
    // consumed or an abandoned callback removes it below.
    let state = unsafe { &*pointer };
    state.result.store(result, Ordering::Release);
    match state.phase.compare_exchange(
        REQUEST_RUNNING,
        REQUEST_COMPLETE,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(REQUEST_ABANDONED) => {
            if METADATA_STATE[index]
                .compare_exchange(
                    pointer,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // SAFETY: the callback has finished all payload access and the
                // successful exchange transfers unique Box ownership here.
                unsafe { drop(Box::from_raw(pointer)) };
            }
        }
        Err(_) => {}
    }
}

fn install_metadata_state(
    slot: CardSlot,
    kind: CardMetadataOperationKind,
    payload: CardMetadataPayload,
) -> Result<*mut CardMetadataState, Box<CardMetadataState>> {
    let state = Box::new(CardMetadataState {
        kind,
        phase: AtomicU8::new(REQUEST_RUNNING),
        result: AtomicI32::new(CARD_ERROR_BUSY),
        payload: UnsafeCell::new(payload),
    });
    let pointer = Box::into_raw(state);
    if METADATA_STATE[slot.index()]
        .compare_exchange(
            ptr::null_mut(),
            pointer,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
    {
        Ok(pointer)
    } else {
        // SAFETY: installation failed, so ownership never left this function.
        Err(unsafe { Box::from_raw(pointer) })
    }
}

fn recover_failed_metadata_submit(
    slot: CardSlot,
    pointer: *mut CardMetadataState,
) -> Option<CardMetadataPayload> {
    METADATA_STATE[slot.index()]
        .compare_exchange(
            pointer,
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .ok()?;
    // SAFETY: failed submission cannot invoke the callback, and the successful
    // exchange restores unique Box ownership to this function.
    let state = unsafe { *Box::from_raw(pointer) };
    Some(state.payload.into_inner())
}

fn metadata_status(slot: CardSlot, kind: CardMetadataOperationKind) -> CardAsyncStatus {
    let pointer = METADATA_STATE[slot.index()].load(Ordering::Acquire);
    if pointer.is_null() {
        return CardAsyncStatus::Complete(CARD_ERROR_FATAL_ERROR);
    }
    // SAFETY: an active request keeps the state installed until completion is
    // consumed or an abandoned callback destroys it.
    let state = unsafe { &*pointer };
    if state.kind != kind {
        return CardAsyncStatus::Complete(CARD_ERROR_BUSY);
    }
    if state.phase.load(Ordering::Acquire) == REQUEST_COMPLETE {
        CardAsyncStatus::Complete(state.result.load(Ordering::Acquire))
    } else {
        CardAsyncStatus::Pending
    }
}

fn take_metadata(
    slot: CardSlot,
    kind: CardMetadataOperationKind,
) -> Option<(i32, CardMetadataPayload)> {
    let index = slot.index();
    let pointer = METADATA_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return None;
    }
    // SAFETY: the pointer remains globally owned while installed.
    let state = unsafe { &*pointer };
    if state.kind != kind || state.phase.load(Ordering::Acquire) != REQUEST_COMPLETE {
        return None;
    }
    METADATA_STATE[index]
        .compare_exchange(
            pointer,
            ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .ok()?;
    // SAFETY: the callback performs no state access after publishing complete,
    // and the successful exchange transfers unique Box ownership here.
    let state = unsafe { *Box::from_raw(pointer) };
    let result = state.result.load(Ordering::Acquire);
    Some((result, state.payload.into_inner()))
}

fn abandon_metadata(slot: CardSlot, kind: CardMetadataOperationKind) {
    let index = slot.index();
    let pointer = METADATA_STATE[index].load(Ordering::Acquire);
    if pointer.is_null() {
        return;
    }
    // SAFETY: the pointer remains valid while globally installed.
    let state = unsafe { &*pointer };
    if state.kind != kind {
        return;
    }
    match state.phase.compare_exchange(
        REQUEST_RUNNING,
        REQUEST_ABANDONED,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {}
        Err(REQUEST_COMPLETE) => {
            if METADATA_STATE[index]
                .compare_exchange(
                    pointer,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // SAFETY: completion has been published and the exchange
                // transfers unique Box ownership to this Drop path.
                unsafe { drop(Box::from_raw(pointer)) };
            }
        }
        Err(_) => {}
    }
}

/// Completion of an owned asynchronous directory-entry creation.
pub struct CardCreateEntryCompletion {
    status: i32,
    entry: CardDir,
    file: CardFile,
}

impl CardCreateEntryCompletion {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn entry(&self) -> &CardDir {
        &self.entry
    }

    pub fn file(&self) -> Option<&CardFile> {
        (self.status == CARD_ERROR_READY).then_some(&self.file)
    }

    pub fn into_parts(self) -> (i32, CardDir, CardFile) {
        (self.status, self.entry, self.file)
    }
}

/// Completion of an owned asynchronous directory-entry deletion.
pub struct CardDeleteEntryCompletion {
    status: i32,
    entry: CardDir,
}

impl CardDeleteEntryCompletion {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn entry(&self) -> &CardDir {
        &self.entry
    }

    pub fn into_parts(self) -> (i32, CardDir) {
        (self.status, self.entry)
    }
}

/// Completion of an owned asynchronous status update.
pub struct CardSetStatusCompletion {
    status: i32,
    stat: CardStat,
}

impl CardSetStatusCompletion {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub const fn stat(&self) -> &CardStat {
        &self.stat
    }

    pub fn into_parts(self) -> (i32, CardStat) {
        (self.status, self.stat)
    }
}

/// Pollable owned asynchronous directory-entry creation.
pub struct CardCreateEntryRequest {
    slot: CardSlot,
    active: bool,
}

impl CardCreateEntryRequest {
    pub fn status(&self) -> CardAsyncStatus {
        metadata_status(self.slot, CardMetadataOperationKind::CreateEntry)
    }

    pub fn try_complete(mut self) -> Result<CardCreateEntryCompletion, Self> {
        let Some((status, payload)) =
            take_metadata(self.slot, CardMetadataOperationKind::CreateEntry)
        else {
            return Err(self);
        };
        self.active = false;
        Ok(CardCreateEntryCompletion {
            status,
            entry: payload.entry,
            file: payload.file,
        })
    }
}

impl Drop for CardCreateEntryRequest {
    fn drop(&mut self) {
        if self.active {
            abandon_metadata(self.slot, CardMetadataOperationKind::CreateEntry);
        }
    }
}

/// Pollable owned asynchronous directory-entry deletion.
pub struct CardDeleteEntryRequest {
    slot: CardSlot,
    active: bool,
}

impl CardDeleteEntryRequest {
    pub fn status(&self) -> CardAsyncStatus {
        metadata_status(self.slot, CardMetadataOperationKind::DeleteEntry)
    }

    pub fn try_complete(mut self) -> Result<CardDeleteEntryCompletion, Self> {
        let Some((status, payload)) =
            take_metadata(self.slot, CardMetadataOperationKind::DeleteEntry)
        else {
            return Err(self);
        };
        self.active = false;
        Ok(CardDeleteEntryCompletion {
            status,
            entry: payload.entry,
        })
    }
}

impl Drop for CardDeleteEntryRequest {
    fn drop(&mut self) {
        if self.active {
            abandon_metadata(self.slot, CardMetadataOperationKind::DeleteEntry);
        }
    }
}

/// Pollable owned asynchronous `CardStat` update.
pub struct CardSetStatusRequest {
    slot: CardSlot,
    active: bool,
}

impl CardSetStatusRequest {
    pub fn status(&self) -> CardAsyncStatus {
        metadata_status(self.slot, CardMetadataOperationKind::SetStatus)
    }

    pub fn try_complete(mut self) -> Result<CardSetStatusCompletion, Self> {
        let Some((status, payload)) =
            take_metadata(self.slot, CardMetadataOperationKind::SetStatus)
        else {
            return Err(self);
        };
        self.active = false;
        Ok(CardSetStatusCompletion {
            status,
            stat: payload.stat,
        })
    }
}

impl Drop for CardSetStatusRequest {
    fn drop(&mut self) {
        if self.active {
            abandon_metadata(self.slot, CardMetadataOperationKind::SetStatus);
        }
    }
}

/// Starts directory-entry creation while owning both the input entry and output
/// `CardFile` until callback completion.
pub fn create_entry_async_owned(
    slot: CardSlot,
    mut entry: CardDir,
) -> Result<CardCreateEntryRequest, CardMetadataSubmitError<CardDir>> {
    validate_owned_dir_entry(&entry)
        .map_err(|status| CardMetadataSubmitError::new(status, entry))?;
    entry.chn = slot.channel();
    let payload = CardMetadataPayload {
        entry,
        file: CardFile::default(),
        stat: CardStat::default(),
        fileno: 0,
    };
    let pointer = install_metadata_state(slot, CardMetadataOperationKind::CreateEntry, payload)
        .map_err(|state| {
            let payload = state.payload.into_inner();
            CardMetadataSubmitError::new(CARD_ERROR_BUSY, payload.entry)
        })?;

    // SAFETY: the installed Box keeps both the input `CardDir` and output
    // `CardFile` stationary until callback completion or failed submission.
    let status = unsafe {
        let payload = &mut *(*pointer).payload.get();
        sys::create_entry_async(
            slot.channel(),
            &mut payload.entry,
            &mut payload.file,
            Some(owned_metadata_callback),
        )
    };
    if status != CARD_ERROR_READY {
        let payload = recover_failed_metadata_submit(slot, pointer)
            .expect("failed CARD submission must retain installed metadata state");
        return Err(CardMetadataSubmitError::new(status, payload.entry));
    }

    Ok(CardCreateEntryRequest { slot, active: true })
}

/// Starts directory-entry deletion while retaining a stable owned copy of the
/// entry until callback completion.
pub fn delete_entry_async_owned(
    slot: CardSlot,
    mut entry: CardDir,
) -> Result<CardDeleteEntryRequest, CardMetadataSubmitError<CardDir>> {
    validate_owned_dir_entry(&entry)
        .map_err(|status| CardMetadataSubmitError::new(status, entry))?;
    entry.chn = slot.channel();
    let payload = CardMetadataPayload {
        entry,
        file: CardFile::default(),
        stat: CardStat::default(),
        fileno: 0,
    };
    let pointer = install_metadata_state(slot, CardMetadataOperationKind::DeleteEntry, payload)
        .map_err(|state| {
            let payload = state.payload.into_inner();
            CardMetadataSubmitError::new(CARD_ERROR_BUSY, payload.entry)
        })?;

    // SAFETY: the installed Box keeps the directory entry stationary and
    // exclusively owned until callback completion or failed submission.
    let status = unsafe {
        let payload = &mut *(*pointer).payload.get();
        sys::delete_entry_async(
            slot.channel(),
            &mut payload.entry,
            Some(owned_metadata_callback),
        )
    };
    if status != CARD_ERROR_READY {
        let payload = recover_failed_metadata_submit(slot, pointer)
            .expect("failed CARD submission must retain installed metadata state");
        return Err(CardMetadataSubmitError::new(status, payload.entry));
    }

    Ok(CardDeleteEntryRequest { slot, active: true })
}

/// Starts a status update while retaining an owned `CardStat` until callback
/// completion.
pub fn set_status_async_owned(
    slot: CardSlot,
    fileno: i32,
    stat: CardStat,
) -> Result<CardSetStatusRequest, CardMetadataSubmitError<CardStat>> {
    if !(0..CARD_MAXFILES as i32).contains(&fileno) {
        return Err(CardMetadataSubmitError::new(CARD_ERROR_FATAL_ERROR, stat));
    }
    let payload = CardMetadataPayload {
        entry: CardDir::default(),
        file: CardFile::default(),
        stat,
        fileno,
    };
    let pointer = install_metadata_state(slot, CardMetadataOperationKind::SetStatus, payload)
        .map_err(|state| {
            let payload = state.payload.into_inner();
            CardMetadataSubmitError::new(CARD_ERROR_BUSY, payload.stat)
        })?;

    // SAFETY: the installed Box keeps the `CardStat` stationary and exclusively
    // owned until callback completion or failed submission.
    let status = unsafe {
        let payload = &mut *(*pointer).payload.get();
        sys::set_status_async(
            slot.channel(),
            payload.fileno,
            &mut payload.stat,
            Some(owned_metadata_callback),
        )
    };
    if status != CARD_ERROR_READY {
        let payload = recover_failed_metadata_submit(slot, pointer)
            .expect("failed CARD submission must retain installed metadata state");
        return Err(CardMetadataSubmitError::new(status, payload.stat));
    }

    Ok(CardSetStatusRequest { slot, active: true })
}

/// Probes a validated memory-card slot.
pub fn probe_slot(slot: CardSlot) -> i32 {
    sys::probe(slot.channel())
}

/// Returns memory and sector sizes without exposing writable raw pointers.
pub fn probe_info(slot: CardSlot) -> Result<CardProbeInfo, i32> {
    let mut memory_size = 0;
    let mut sector_size = 0;
    // SAFETY: both output references are initialized, aligned, live, and
    // uniquely borrowed for the duration of the synchronous call.
    let status = unsafe { sys::probe_ex(slot.channel(), &mut memory_size, &mut sector_size) };
    status_value(
        status,
        CardProbeInfo {
            memory_size,
            sector_size,
        },
    )
}

/// Returns the mounted card's sector size.
pub fn sector_size(slot: CardSlot) -> Result<u32, i32> {
    let mut value = 0;
    // SAFETY: `value` is a live, aligned, uniquely borrowed output object.
    let status = unsafe { sys::get_sector_size(slot.channel(), &mut value) };
    status_value(status, value)
}

/// Returns the mounted card's block count.
pub fn block_count(slot: CardSlot) -> Result<u32, i32> {
    let mut value = 0;
    // SAFETY: `value` is a live, aligned, uniquely borrowed output object.
    let status = unsafe { sys::get_block_count(slot.channel(), &mut value) };
    status_value(status, value)
}

/// Returns the mounted card's free-block count.
pub fn free_blocks(slot: CardSlot) -> Result<u16, i32> {
    let mut value = 0;
    // SAFETY: `value` is a live, aligned, uniquely borrowed output object.
    let status = unsafe { sys::get_free_blocks(slot.channel(), &mut value) };
    status_value(status, value)
}

/// Returns a file's status without exposing a writable raw pointer.
pub fn file_status(slot: CardSlot, fileno: i32) -> Result<CardStat, i32> {
    let mut value = CardStat::default();
    // SAFETY: `value` is a live, aligned, uniquely borrowed output object.
    let status = unsafe { sys::get_status(slot.channel(), fileno, &mut value) };
    status_value(status, value)
}

/// Returns the two-word card serial number.
pub fn serial_number(slot: CardSlot) -> Result<(u32, u32), i32> {
    let mut first = 0;
    let mut second = 0;
    // SAFETY: both values are live, aligned, uniquely borrowed outputs.
    let status = unsafe { sys::get_serial_no(slot.channel(), &mut first, &mut second) };
    status_value(status, (first, second))
}

pub fn probe(chn: i32) -> i32 {
    sys::probe(chn)
}

/// # Safety
///
/// `mem_size` and `sect_size` must either be null or valid writable pointers.
pub unsafe fn probe_ex(chn: i32, mem_size: *mut i32, sect_size: *mut i32) -> i32 {
    unsafe { sys::probe_ex(chn, mem_size, sect_size) }
}

pub fn get_error_code(chn: i32) -> i32 {
    sys::get_error_code(chn)
}

/// # Safety
///
/// `sector_size` must either be null or a valid writable pointer.
pub unsafe fn get_sector_size(chn: i32, sector_size: *mut u32) -> i32 {
    unsafe { sys::get_sector_size(chn, sector_size) }
}

/// # Safety
///
/// `block_count` must either be null or a valid writable pointer.
pub unsafe fn get_block_count(chn: i32, block_count: *mut u32) -> i32 {
    unsafe { sys::get_block_count(chn, block_count) }
}

/// # Safety
///
/// `freeblocks` must either be null or a valid writable pointer.
pub unsafe fn get_free_blocks(chn: i32, freeblocks: *mut u16) -> i32 {
    unsafe { sys::get_free_blocks(chn, freeblocks) }
}

/// # Safety
///
/// `workarea` must be valid for the CARD workarea and 32-byte aligned.
pub unsafe fn mount(chn: i32, workarea: *mut c_void, detach_cb: CardCallback) -> i32 {
    unsafe { sys::mount(chn, workarea, detach_cb) }
}

/// # Safety
///
/// `workarea` must be valid for the CARD workarea and 32-byte aligned.
pub unsafe fn mount_async(
    chn: i32,
    workarea: *mut c_void,
    detach_cb: CardCallback,
    attach_cb: CardCallback,
) -> i32 {
    unsafe { sys::mount_async(chn, workarea, detach_cb, attach_cb) }
}

pub fn unmount(chn: i32) -> i32 {
    sys::unmount(chn)
}

/// # Safety
///
/// Raw pointers must follow the C CARD API lifetime and alignment rules.
pub unsafe fn read(file: *mut CardFile, buffer: *mut c_void, len: u32, offset: u32) -> i32 {
    unsafe { sys::read(file, buffer, len, offset) }
}

/// # Safety
///
/// Raw pointers must follow the C CARD API lifetime and alignment rules.
pub unsafe fn write(file: *mut CardFile, buffer: *mut c_void, len: u32, offset: u32) -> i32 {
    unsafe { sys::write(file, buffer, len, offset) }
}

/// Reads whole CARD transfer units into a Rust slice.
///
/// The Wii EXI DMA engine requires a 32-byte-aligned address, while ordinary
/// Rust slices provide no such guarantee. This wrapper receives into an
/// internal aligned bounce buffer, performs the required cache maintenance,
/// and copies into `output` only after a successful synchronous transfer.
pub fn read_slice(file: &mut CardFile, output: &mut [u8], offset: u32) -> i32 {
    let len = match transfer_len(output.len(), offset) {
        Ok(len) => len,
        Err(status) => return status,
    };
    let mut storage = match aligned_storage(output.len()) {
        Ok(storage) => storage,
        Err(status) => return status,
    };
    let pointer = storage.as_mut_ptr().cast::<c_void>();

    // SAFETY: `storage` is a live 32-byte-aligned allocation covering `len`
    // bytes. Flushing prevents dirty cache lines from later overwriting DMA
    // output; invalidation after completion makes device writes visible.
    unsafe { dc_flush_range(pointer.cast_const(), len) };
    // SAFETY: `file` is uniquely borrowed for the synchronous call, and the
    // aligned allocation remains live and unaliased until the call returns.
    let status = unsafe { sys::read(file, pointer, len, offset) };
    if status == CARD_ERROR_READY {
        // SAFETY: the same live allocation covers the entire DMA output.
        unsafe { dc_invalidate_range(pointer.cast_const(), len) };
        // SAFETY: `storage` contains at least `output.len()` initialized bytes
        // and the independently borrowed destination cannot overlap it.
        unsafe {
            core::ptr::copy_nonoverlapping(pointer.cast::<u8>(), output.as_mut_ptr(), output.len());
        }
    }
    status
}

/// Writes whole CARD transfer units from a Rust slice.
///
/// Input is copied to a 32-byte-aligned bounce buffer and flushed before the
/// synchronous EXI DMA transfer. This accepts normally aligned Rust slices
/// without exposing the raw CARD buffer contract.
pub fn write_slice(file: &mut CardFile, input: &[u8], offset: u32) -> i32 {
    let len = match transfer_len(input.len(), offset) {
        Ok(len) => len,
        Err(status) => return status,
    };
    let mut storage = match aligned_storage(input.len()) {
        Ok(storage) => storage,
        Err(status) => return status,
    };
    let pointer = storage.as_mut_ptr().cast::<c_void>();

    // SAFETY: the aligned allocation covers `input.len()` bytes and cannot
    // overlap the independently borrowed input slice.
    unsafe {
        core::ptr::copy_nonoverlapping(input.as_ptr(), pointer.cast::<u8>(), input.len());
        dc_flush_range(pointer.cast_const(), len);
    }
    // SAFETY: `file` is uniquely borrowed and `storage` remains live,
    // aligned, initialized, and unaliased for the synchronous call.
    unsafe { sys::write(file, pointer, len, offset) }
}

/// # Safety
///
/// `filename` must be a valid NUL-terminated C string and `file` writable.
pub unsafe fn open(chn: i32, filename: *const i8, file: *mut CardFile) -> i32 {
    unsafe { sys::open(chn, filename.cast(), file) }
}

/// # Safety
///
/// `filename` must be a valid NUL-terminated C string and `file` writable.
pub unsafe fn create(chn: i32, filename: *const i8, size: u32, file: *mut CardFile) -> i32 {
    unsafe { sys::create(chn, filename.cast(), size, file) }
}

/// # Safety
///
/// `filename` must be a valid NUL-terminated C string.
pub unsafe fn delete(chn: i32, filename: *const i8) -> i32 {
    unsafe { sys::delete(chn, filename.cast()) }
}

/// # Safety
///
/// `stats` must be writable.
pub unsafe fn get_status(chn: i32, fileno: i32, stats: *mut CardStat) -> i32 {
    unsafe { sys::get_status(chn, fileno, stats) }
}

/// # Safety
///
/// `serial1` and `serial2` must be writable.
pub unsafe fn get_serial_no(chn: i32, serial1: *mut u32, serial2: *mut u32) -> i32 {
    unsafe { sys::get_serial_no(chn, serial1, serial2) }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helpers_exist() {
        let _ = init as fn(Option<CardGameCode>, Option<CardCompanyCode>) -> i32;
        let _ = probe as fn(i32) -> i32;
        let _ = probe_ex as unsafe fn(i32, *mut i32, *mut i32) -> i32;
        let _ = set_gamecode as fn(Option<CardGameCode>);
        let _ = set_company as fn(Option<CardCompanyCode>);
        let _ = CardGameCode::new(*b"RUST");
        let _ = CardCompanyCode::new(*b"RS");
        let _ = get_error_code as fn(i32) -> i32;
        let _ = get_sector_size as unsafe fn(i32, *mut u32) -> i32;
        let _ = get_block_count as unsafe fn(i32, *mut u32) -> i32;
        let _ = get_free_blocks as unsafe fn(i32, *mut u16) -> i32;
        let _ = mount as unsafe fn(i32, *mut c_void, CardCallback) -> i32;
        let _ = mount_async as unsafe fn(i32, *mut c_void, CardCallback, CardCallback) -> i32;
        let _ = mount_async_owned as fn(CardSlot) -> Result<CardMountRequest, i32>;
        let _ = unmount as fn(i32) -> i32;
        let _ = read as unsafe fn(*mut CardFile, *mut c_void, u32, u32) -> i32;
        let _ = write as unsafe fn(*mut CardFile, *mut c_void, u32, u32) -> i32;
        let _ = read_async_owned
            as fn(CardFile, usize, u32) -> Result<CardReadRequest, CardReadSubmitError>;
        let _ = write_async_owned
            as fn(CardFile, Vec<u8>, u32) -> Result<CardWriteRequest, CardWriteSubmitError>;
        let _ = create_async_owned as fn(CardSlot, &[u8], u32) -> Result<CardCreateRequest, i32>;
        let _ = delete_async_owned as fn(CardSlot, &[u8]) -> Result<CardDeleteRequest, i32>;
        let _ = create_entry_async_owned
            as fn(
                CardSlot,
                CardDir,
            ) -> Result<CardCreateEntryRequest, CardMetadataSubmitError<CardDir>>;
        let _ = delete_entry_async_owned
            as fn(
                CardSlot,
                CardDir,
            ) -> Result<CardDeleteEntryRequest, CardMetadataSubmitError<CardDir>>;
        let _ = set_status_async_owned
            as fn(
                CardSlot,
                i32,
                CardStat,
            ) -> Result<CardSetStatusRequest, CardMetadataSubmitError<CardStat>>;
        let _ = read_slice as fn(&mut CardFile, &mut [u8], u32) -> i32;
        let _ = write_slice as fn(&mut CardFile, &[u8], u32) -> i32;
        let _ = probe_slot as fn(CardSlot) -> i32;
        let _ = probe_info as fn(CardSlot) -> Result<CardProbeInfo, i32>;
        let _ = sector_size as fn(CardSlot) -> Result<u32, i32>;
        let _ = block_count as fn(CardSlot) -> Result<u32, i32>;
        let _ = free_blocks as fn(CardSlot) -> Result<u16, i32>;
        let _ = file_status as fn(CardSlot, i32) -> Result<CardStat, i32>;
        let _ = serial_number as fn(CardSlot) -> Result<(u32, u32), i32>;
        let _ = open as unsafe fn(i32, *const i8, *mut CardFile) -> i32;
        let _ = create as unsafe fn(i32, *const i8, u32, *mut CardFile) -> i32;
        let _ = delete as unsafe fn(i32, *const i8) -> i32;
        let _ = get_status as unsafe fn(i32, i32, *mut CardStat) -> i32;
        let _ = get_serial_no as unsafe fn(i32, *mut u32, *mut u32) -> i32;
    }
}
