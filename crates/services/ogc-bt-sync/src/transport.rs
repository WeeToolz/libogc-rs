//! IOS transport for the Wii internal Bluetooth adapter.
extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use ogc_cache::{dc_flush_range, dc_invalidate_range};
use ogc_ipc::{
    FileDescriptor, IPC_EINVAL, IPC_OPEN_RW, Ioctlv, IoctlvEntry, Ipccallback, open_file,
};

use crate::protocol::*;
use crate::request::{TRANSFER_BYTES, TransferRequest, TransferState};
const USB_TRANSFER_BUFFER_BYTES: usize = 1024;
const PENDING_EVENT_CAPACITY: usize = 4;

struct PendingEvents {
    buffers: [[u8; MAX_EVENT_PACKET_BYTES]; PENDING_EVENT_CAPACITY],
    lengths: [usize; PENDING_EVENT_CAPACITY],
    head: usize,
    count: usize,
}

impl PendingEvents {
    const fn new() -> Self {
        Self {
            buffers: [[0; MAX_EVENT_PACKET_BYTES]; PENDING_EVENT_CAPACITY],
            lengths: [0; PENDING_EVENT_CAPACITY],
            head: 0,
            count: 0,
        }
    }

    fn push(&mut self, event: &[u8]) -> Result<()> {
        if self.count == PENDING_EVENT_CAPACITY || event.len() > MAX_EVENT_PACKET_BYTES {
            return Err(Error::BufferTooSmall);
        }
        let index = (self.head + self.count) % PENDING_EVENT_CAPACITY;
        self.buffers[index][..event.len()].copy_from_slice(event);
        self.lengths[index] = event.len();
        self.count += 1;
        Ok(())
    }

    fn pop(&mut self, output: &mut [u8]) -> Result<Option<usize>> {
        if self.count == 0 {
            return Ok(None);
        }
        let len = self.lengths[self.head];
        if output.len() < len {
            return Err(Error::BufferTooSmall);
        }
        output[..len].copy_from_slice(&self.buffers[self.head][..len]);
        self.head = (self.head + 1) % PENDING_EVENT_CAPACITY;
        self.count -= 1;
        Ok(Some(len))
    }
}

#[repr(align(32))]
struct UsbTransferBuffer([u8; USB_TRANSFER_BUFFER_BYTES]);

#[repr(align(32))]
struct UsbByte([u8; 1]);

#[repr(align(32))]
struct UsbWord([u8; 2]);

#[repr(C, align(32))]
struct AsyncHciCommand {
    argv: [Ioctlv; 7],
    bm_request_type: UsbByte,
    b_request: UsbByte,
    w_value: UsbWord,
    w_index: UsbWord,
    w_length: UsbWord,
    null: UsbByte,
    packet: UsbTransferBuffer,
    cb: Ipccallback,
    userdata: *mut c_void,
}

unsafe extern "C" fn hci_cmd_async_cb(result: i32, userdata: *mut c_void) -> i32 {
    if userdata.is_null() {
        return result;
    }

    let cmd = unsafe { Box::from_raw(userdata.cast::<AsyncHciCommand>()) };
    match cmd.cb {
        Some(cb) => unsafe { cb(result, cmd.userdata) },
        None => result,
    }
}

struct HciCommandRequestState {
    complete: AtomicBool,
    result: AtomicI32,
}

impl HciCommandRequestState {
    const fn new() -> Self {
        Self {
            complete: AtomicBool::new(false),
            result: AtomicI32::new(0),
        }
    }

    fn complete(&self, result: i32) {
        self.result.store(result, Ordering::Relaxed);
        self.complete.store(true, Ordering::Release);
    }

    fn raw_result(&self) -> Option<i32> {
        self.complete
            .load(Ordering::Acquire)
            .then(|| self.result.load(Ordering::Relaxed))
    }
}

type OwnedHciCallback = Box<dyn FnOnce(Result<()>) + Send + 'static>;

struct OwnedHciCompletion {
    state: Arc<HciCommandRequestState>,
    callback: Option<OwnedHciCallback>,
}

/// Pollable ownership handle for an asynchronously submitted HCI command.
///
/// The command packet and callback context remain owned by the asynchronous
/// operation until IOS reports completion. Dropping this handle detaches the
/// caller from the result; it does not cancel the IOS request.
#[derive(Clone)]
pub struct HciCommandRequest {
    state: Arc<HciCommandRequestState>,
}

/// Current state of an owned asynchronous HCI command.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HciCommandRequestStatus {
    Pending,
    Complete(Result<()>),
}

impl HciCommandRequest {
    /// Returns `true` after IOS has completed the command callback.
    pub fn is_complete(&self) -> bool {
        self.state.complete.load(Ordering::Acquire)
    }

    /// Returns the raw IOS completion result when available.
    pub fn raw_result(&self) -> Option<i32> {
        self.state.raw_result()
    }

    /// Returns the mapped command result when available.
    pub fn result(&self) -> Option<Result<()>> {
        self.raw_result().map(map_async_hci_result)
    }

    /// Returns a snapshot of the request's completion state.
    pub fn status(&self) -> HciCommandRequestStatus {
        match self.result() {
            Some(result) => HciCommandRequestStatus::Complete(result),
            None => HciCommandRequestStatus::Pending,
        }
    }
}

fn map_async_hci_result(result: i32) -> Result<()> {
    if result < 0 {
        Err(Error::Ipc(result))
    } else {
        Ok(())
    }
}

unsafe extern "C" fn owned_hci_completion_cb(result: i32, userdata: *mut c_void) -> i32 {
    if userdata.is_null() {
        return result;
    }

    // SAFETY: `submit_owned_hci_command` creates exactly one box for this
    // callback and transfers it to the raw async request only after successful
    // submission. IOS invokes this completion callback at most once.
    let mut completion = unsafe { Box::from_raw(userdata.cast::<OwnedHciCompletion>()) };
    completion.state.complete(result);

    if let Some(callback) = completion.callback.take() {
        callback(map_async_hci_result(result));
    }

    result
}

#[repr(C, align(32))]
struct AsyncTransfer {
    vectors: [Ioctlv; 3],
    endpoint: UsbByte,
    length: UsbWord,
    state: Arc<TransferState>,
    read: bool,
    requested: usize,
}

unsafe extern "C" fn transfer_complete(result: i32, userdata: *mut c_void) -> i32 {
    // IOS has released every vector before it calls this completion function.
    let transfer = unsafe { Box::from_raw(userdata.cast::<AsyncTransfer>()) };
    if transfer.read {
        unsafe {
            dc_invalidate_range(
                transfer.state.buffer.0.get().cast(),
                transfer.requested as u32,
            )
        };
    }
    let result = if result > transfer.requested as i32 {
        IPC_EINVAL
    } else {
        result
    };
    transfer.state.complete(result);
    result
}

struct AdapterOpenState {
    result: AtomicI32,
    complete: AtomicBool,
    claimed: AtomicBool,
}
impl Drop for AdapterOpenState {
    fn drop(&mut self) {
        if self.complete.load(Ordering::Acquire) && !self.claimed.load(Ordering::Acquire) {
            let fd = self.result.load(Ordering::Relaxed);
            if fd >= 0 {
                // A detached open still owns its descriptor until its late reply.
                unsafe {
                    FileDescriptor::new(fd)
                        .close_async(Some(detached_close_complete), core::ptr::null_mut());
                }
            }
        }
    }
}
unsafe extern "C" fn detached_close_complete(result: i32, _: *mut c_void) -> i32 {
    result
}
unsafe extern "C" fn adapter_open_complete(result: i32, userdata: *mut c_void) -> i32 {
    let state = unsafe { Box::from_raw(userdata.cast::<Arc<AdapterOpenState>>()) };
    state.result.store(result, Ordering::Relaxed);
    state.complete.store(true, Ordering::Release);
    result
}

/// A pending adapter open. Dropping it closes any descriptor returned later.
pub struct AdapterOpenRequest {
    state: Arc<AdapterOpenState>,
}
impl AdapterOpenRequest {
    pub fn take_host(&self) -> Option<Result<BtSyncHost>> {
        if !self.state.complete.load(Ordering::Acquire) {
            return None;
        }
        let result = self.state.result.load(Ordering::Relaxed);
        if result < 0 {
            return Some(Err(Error::Ipc(result)));
        }
        if self.state.claimed.swap(true, Ordering::AcqRel) {
            return Some(Err(Error::UnexpectedEvent));
        }
        Some(BtSyncHost::from_file_descriptor(
            FileDescriptor::new(result),
            DEFAULT_ACL_OUT_ENDPOINT,
            DEFAULT_EVENT_IN_ENDPOINT,
            DEFAULT_ACL_IN_ENDPOINT,
        ))
    }
}

/// Open handle to the Wii internal Bluetooth USB device.
pub struct BtSyncHost {
    fd: FileDescriptor,
    acl_out_endpoint: u8,
    event_in_endpoint: u8,
    acl_in_endpoint: u8,
    pending_events: UnsafeCell<PendingEvents>,
}

impl BtSyncHost {
    /// Starts an adapter open without waiting for IOS.
    pub fn open_async() -> Result<AdapterOpenRequest> {
        let state = Arc::new(AdapterOpenState {
            result: AtomicI32::new(0),
            complete: AtomicBool::new(false),
            claimed: AtomicBool::new(false),
        });
        let context = Box::into_raw(Box::new(state.clone()));
        let rc = ogc_ipc::open_file_async(
            INTERNAL_BT_PATH,
            IPC_OPEN_RW,
            Some(adapter_open_complete),
            context.cast(),
        );
        if rc < 0 {
            unsafe {
                drop(Box::from_raw(context));
            }
            return Err(Error::Ipc(rc));
        }
        Ok(AdapterOpenRequest { state })
    }

    pub fn read_event_async(&self) -> Result<TransferRequest> {
        self.submit_transfer(
            IOCTL_USB_INTRMSG,
            self.event_in_endpoint,
            MAX_EVENT_PACKET_BYTES,
            None,
        )
    }
    pub fn read_acl_async(&self) -> Result<TransferRequest> {
        self.submit_transfer(IOCTL_USB_BLKMSG, self.acl_in_endpoint, TRANSFER_BYTES, None)
    }
    /// Copies output data into memory owned by the asynchronous request.
    pub fn write_acl_async(&self, data: &[u8]) -> Result<TransferRequest> {
        self.submit_transfer(
            IOCTL_USB_BLKMSG,
            self.acl_out_endpoint,
            data.len(),
            Some(data),
        )
    }
    pub fn close_async(&self) -> Result<HciCommandRequest> {
        let state = Arc::new(HciCommandRequestState::new());
        let context = Box::into_raw(Box::new(OwnedHciCompletion {
            state: state.clone(),
            callback: None,
        }));
        let rc = unsafe {
            FileDescriptor::new(self.fd.raw())
                .close_async(Some(owned_hci_completion_cb), context.cast())
        };
        if rc < 0 {
            unsafe {
                drop(Box::from_raw(context));
            }
            return Err(Error::Ipc(rc));
        }
        Ok(HciCommandRequest { state })
    }

    fn submit_transfer(
        &self,
        ioctl: i32,
        endpoint: u8,
        length: usize,
        output: Option<&[u8]>,
    ) -> Result<TransferRequest> {
        if length == 0 || length > TRANSFER_BYTES {
            return Err(Error::BufferTooSmall);
        }
        let state = Arc::new(TransferState::new());
        if let Some(data) = output {
            unsafe {
                (&mut *state.buffer.0.get())[..length].copy_from_slice(data);
            }
        }
        let mut transfer = Box::new(AsyncTransfer {
            vectors: [Ioctlv::default(); 3],
            endpoint: UsbByte([endpoint]),
            length: UsbWord((length as u16).to_be_bytes()),
            state: state.clone(),
            read: output.is_none(),
            requested: length,
        });
        transfer.vectors[0] = Ioctlv {
            data: transfer.endpoint.0.as_mut_ptr().cast(),
            len: 1,
        };
        transfer.vectors[1] = Ioctlv {
            data: transfer.length.0.as_mut_ptr().cast(),
            len: 2,
        };
        transfer.vectors[2] = Ioctlv {
            data: state.buffer.0.get().cast(),
            len: length as u32,
        };
        // IOS ioctlv flushes all input vectors and invalidates all output vectors.
        // The OH1 ABI uses two input vectors and one data vector in both directions.
        unsafe {
            if output.is_some() {
                dc_flush_range(state.buffer.0.get().cast(), length as u32);
            } else {
                dc_invalidate_range(state.buffer.0.get().cast(), length as u32);
            }
        }
        let raw = Box::into_raw(transfer);
        let rc = unsafe {
            self.fd.ioctlv_async_raw(
                ioctl,
                2,
                1,
                (*raw).vectors.as_mut_ptr(),
                Some(transfer_complete),
                raw.cast(),
            )
        };
        if rc < 0 {
            unsafe {
                drop(Box::from_raw(raw));
            }
            return Err(Error::Ipc(rc));
        }
        Ok(TransferRequest { state })
    }

    pub fn open() -> Result<Self> {
        Self::open_with_endpoints(
            INTERNAL_BT_PATH,
            DEFAULT_ACL_OUT_ENDPOINT,
            DEFAULT_EVENT_IN_ENDPOINT,
            DEFAULT_ACL_IN_ENDPOINT,
        )
    }

    pub fn open_with_endpoints(
        path: &CStr,
        acl_out_endpoint: u8,
        event_in_endpoint: u8,
        acl_in_endpoint: u8,
    ) -> Result<Self> {
        let endpoints =
            HciEndpointAddresses::new(acl_out_endpoint, event_in_endpoint, acl_in_endpoint)?;
        let fd = open_file(path, IPC_OPEN_RW).map_err(Error::Ipc)?;
        Ok(Self::with_endpoints(fd, endpoints))
    }

    pub fn from_file_descriptor(
        fd: FileDescriptor,
        acl_out_endpoint: u8,
        event_in_endpoint: u8,
        acl_in_endpoint: u8,
    ) -> Result<Self> {
        let endpoints =
            match HciEndpointAddresses::new(acl_out_endpoint, event_in_endpoint, acl_in_endpoint) {
                Ok(endpoints) => endpoints,
                Err(error) => {
                    fd.close().map_err(Error::Ipc)?;
                    return Err(error);
                }
            };
        Ok(Self::with_endpoints(fd, endpoints))
    }

    const fn with_endpoints(fd: FileDescriptor, endpoints: HciEndpointAddresses) -> Self {
        Self {
            fd,
            acl_out_endpoint: endpoints.acl_out,
            event_in_endpoint: endpoints.event_in,
            acl_in_endpoint: endpoints.acl_in,
            pending_events: UnsafeCell::new(PendingEvents::new()),
        }
    }

    pub const fn fd(&self) -> &FileDescriptor {
        &self.fd
    }

    pub const fn acl_out_endpoint(&self) -> u8 {
        self.acl_out_endpoint
    }

    pub const fn event_in_endpoint(&self) -> u8 {
        self.event_in_endpoint
    }

    pub const fn acl_in_endpoint(&self) -> u8 {
        self.acl_in_endpoint
    }

    pub fn close(self) -> Result<()> {
        self.fd.close().map_err(Error::Ipc)
    }

    pub fn send_hci_command(&self, opcode: HciOpcode, params: &[u8]) -> Result<()> {
        if params.len() > u8::MAX as usize {
            return Err(Error::BufferTooSmall);
        }

        let mut packet = UsbTransferBuffer([0u8; USB_TRANSFER_BUFFER_BYTES]);
        let packet_len = 3 + params.len();
        packet.0[0..2].copy_from_slice(&opcode.raw().to_le_bytes());
        packet.0[2] = params.len() as u8;
        packet.0[3..packet_len].copy_from_slice(params);

        let packet = &mut packet.0[..packet_len];
        unsafe { dc_flush_range(packet.as_ptr().cast(), packet.len() as u32) };

        let bm_request_type = UsbByte([0x20]);
        let b_request = UsbByte([0x00]);
        let w_value = UsbWord(0u16.to_le_bytes());
        let w_index = UsbWord(0u16.to_le_bytes());
        let w_length = UsbWord((packet_len as u16).to_le_bytes());
        let null = UsbByte([0]);

        let rc = self.fd.ioctlv(
            IOCTL_USB_CTRLMSG,
            6,
            [
                IoctlvEntry::In(&bm_request_type.0),
                IoctlvEntry::In(&b_request.0),
                IoctlvEntry::In(&w_value.0),
                IoctlvEntry::In(&w_index.0),
                IoctlvEntry::In(&w_length.0),
                IoctlvEntry::In(&null.0),
                IoctlvEntry::Out(packet),
            ],
        );

        if rc < 0 { Err(Error::Ipc(rc)) } else { Ok(()) }
    }

    /// Submits an HCI command and invokes `ipc_cb` after IOS completes it.
    ///
    /// The command parameters are copied before this function returns.
    ///
    /// # Safety
    ///
    /// When `userdata` is non-null, it must satisfy every validity, alignment,
    /// aliasing, synchronization, and lifetime requirement imposed by `ipc_cb`.
    /// It must remain valid until the asynchronous operation completes and
    /// `ipc_cb` has returned.
    ///
    /// The callback must be safe to invoke from the IOS completion context and
    /// must not unwind across the callback boundary.
    pub unsafe fn send_hci_command_async(
        &self,
        opcode: HciOpcode,
        params: &[u8],
        ipc_cb: Ipccallback,
        userdata: *mut c_void,
    ) -> Result<()> {
        if ipc_cb.is_none() {
            return Err(Error::Ipc(IPC_EINVAL));
        }
        if params.len() > u8::MAX as usize {
            return Err(Error::BufferTooSmall);
        }

        let packet_len = 3 + params.len();
        let mut cmd = Box::new(AsyncHciCommand {
            argv: [Ioctlv::default(); 7],
            bm_request_type: UsbByte([0x20]),
            b_request: UsbByte([0x00]),
            w_value: UsbWord(0u16.to_le_bytes()),
            w_index: UsbWord(0u16.to_le_bytes()),
            w_length: UsbWord((packet_len as u16).to_le_bytes()),
            null: UsbByte([0]),
            packet: UsbTransferBuffer([0u8; USB_TRANSFER_BUFFER_BYTES]),
            cb: ipc_cb,
            userdata,
        });

        cmd.packet.0[0..2].copy_from_slice(&opcode.raw().to_le_bytes());
        cmd.packet.0[2] = params.len() as u8;
        cmd.packet.0[3..packet_len].copy_from_slice(params);

        cmd.argv[0] = Ioctlv {
            data: cmd.bm_request_type.0.as_mut_ptr().cast(),
            len: cmd.bm_request_type.0.len() as u32,
        };
        cmd.argv[1] = Ioctlv {
            data: cmd.b_request.0.as_mut_ptr().cast(),
            len: cmd.b_request.0.len() as u32,
        };
        cmd.argv[2] = Ioctlv {
            data: cmd.w_value.0.as_mut_ptr().cast(),
            len: cmd.w_value.0.len() as u32,
        };
        cmd.argv[3] = Ioctlv {
            data: cmd.w_index.0.as_mut_ptr().cast(),
            len: cmd.w_index.0.len() as u32,
        };
        cmd.argv[4] = Ioctlv {
            data: cmd.w_length.0.as_mut_ptr().cast(),
            len: cmd.w_length.0.len() as u32,
        };
        cmd.argv[5] = Ioctlv {
            data: cmd.null.0.as_mut_ptr().cast(),
            len: cmd.null.0.len() as u32,
        };
        cmd.argv[6] = Ioctlv {
            data: cmd.packet.0.as_mut_ptr().cast(),
            len: packet_len as u32,
        };

        let raw = Box::into_raw(cmd);
        let rc = unsafe {
            self.fd.ioctlv_async_raw(
                IOCTL_USB_CTRLMSG,
                6,
                1,
                (*raw).argv.as_mut_ptr(),
                Some(hci_cmd_async_cb),
                raw.cast::<c_void>(),
            )
        };

        if rc < 0 {
            let _ = unsafe { Box::from_raw(raw) };
            Err(Error::Ipc(rc))
        } else {
            Ok(())
        }
    }

    /// Submits an HCI command with fully owned packet and completion state.
    ///
    /// The returned handle can be polled for completion. Dropping it safely
    /// detaches from the result while the owned IOS request continues.
    pub fn send_hci_command_async_owned(
        &self,
        opcode: HciOpcode,
        params: &[u8],
    ) -> Result<HciCommandRequest> {
        self.submit_owned_hci_command(opcode, params, None)
    }

    /// Submits an HCI command with an owned completion callback.
    ///
    /// `callback` runs once from the deferred IPC completion context. It owns
    /// all captured state and therefore cannot retain borrowed stack pointers.
    /// Keep callback work short and hand longer work to the normal application
    /// loop where practical.
    pub fn send_hci_command_async_with<F>(
        &self,
        opcode: HciOpcode,
        params: &[u8],
        callback: F,
    ) -> Result<HciCommandRequest>
    where
        F: FnOnce(Result<()>) + Send + 'static,
    {
        self.submit_owned_hci_command(opcode, params, Some(Box::new(callback)))
    }

    fn submit_owned_hci_command(
        &self,
        opcode: HciOpcode,
        params: &[u8],
        callback: Option<OwnedHciCallback>,
    ) -> Result<HciCommandRequest> {
        let state = Arc::new(HciCommandRequestState::new());
        let completion = Box::new(OwnedHciCompletion {
            state: Arc::clone(&state),
            callback,
        });
        let completion_ptr = Box::into_raw(completion);

        // SAFETY: The callback context is heap-owned and remains valid until
        // `owned_hci_completion_cb` reconstructs and drops it. The raw command
        // implementation separately owns a copy of `params` for IOS.
        let submitted = unsafe {
            self.send_hci_command_async(
                opcode,
                params,
                Some(owned_hci_completion_cb),
                completion_ptr.cast::<c_void>(),
            )
        };

        match submitted {
            Ok(()) => Ok(HciCommandRequest { state }),
            Err(error) => {
                // SAFETY: Submission failed, so ownership was never transferred
                // to an IOS completion callback.
                drop(unsafe { Box::from_raw(completion_ptr) });
                Err(error)
            }
        }
    }

    pub fn read_event(&self, buf: &mut [u8]) -> Result<usize> {
        if let Some(used) = unsafe { &mut *self.pending_events.get() }.pop(buf)? {
            return Ok(used);
        }
        self.read_usb_buffer(IOCTL_USB_INTRMSG, self.event_in_endpoint, buf)
    }

    pub fn read_acl(&self, buf: &mut [u8]) -> Result<usize> {
        self.read_usb_buffer(IOCTL_USB_BLKMSG, self.acl_in_endpoint, buf)
    }

    pub fn read_acl_packet<'a>(&self, buf: &'a mut [u8]) -> Result<AclPacket<'a>> {
        let used = self.read_acl(buf)?;
        parse_acl_packet(&buf[..used])
    }

    pub fn write_acl(&self, buf: &[u8]) -> Result<usize> {
        self.write_usb_buffer(IOCTL_USB_BLKMSG, self.acl_out_endpoint, buf)
    }

    pub fn send_l2cap_connection_request(
        &self,
        scratch: &mut [u8],
        handle: u16,
        signal_id: u8,
        psm: u16,
        source_cid: u16,
    ) -> Result<usize> {
        let len = build_l2cap_connection_request(scratch, handle, signal_id, psm, source_cid)?;
        self.write_acl(&scratch[..len])
    }

    pub fn send_l2cap_configuration_request(
        &self,
        scratch: &mut [u8],
        handle: u16,
        signal_id: u8,
        dest_cid: u16,
    ) -> Result<usize> {
        let len = build_l2cap_configuration_request(scratch, handle, signal_id, dest_cid)?;
        self.write_acl(&scratch[..len])
    }

    pub fn send_l2cap_configuration_request_with_mtu(
        &self,
        scratch: &mut [u8],
        handle: u16,
        signal_id: u8,
        dest_cid: u16,
        mtu: u16,
    ) -> Result<usize> {
        let len =
            build_l2cap_configuration_request_with_mtu(scratch, handle, signal_id, dest_cid, mtu)?;
        self.write_acl(&scratch[..len])
    }

    pub fn send_l2cap_configuration_response(
        &self,
        scratch: &mut [u8],
        handle: u16,
        signal_id: u8,
        source_cid: u16,
        result: u16,
    ) -> Result<usize> {
        let len =
            build_l2cap_configuration_response(scratch, handle, signal_id, source_cid, result)?;
        self.write_acl(&scratch[..len])
    }

    pub fn send_l2cap_hid_data(
        &self,
        scratch: &mut [u8],
        handle: u16,
        dest_cid: u16,
        payload: &[u8],
    ) -> Result<usize> {
        let len = build_l2cap_data(scratch, handle, dest_cid, payload)?;
        self.write_acl(&scratch[..len])
    }

    pub fn reset(&self, event_buf: &mut [u8]) -> Result<()> {
        self.send_hci_command(HciOpcode::RESET, &[])?;
        let used = self.expect_command_complete(HciOpcode::RESET, event_buf)?;
        if used < 6 {
            return Err(Error::MalformedEvent);
        }
        if event_buf[5] != 0 {
            return Err(Error::CommandFailed(event_buf[5]));
        }
        Ok(())
    }

    pub fn read_bd_addr(&self, event_buf: &mut [u8]) -> Result<BluetoothAddress> {
        let used = self.command_complete(HciOpcode::READ_BD_ADDR, &[], event_buf)?;
        if used < 12 {
            return Err(Error::MalformedEvent);
        }
        if event_buf[5] != 0 {
            return Err(Error::CommandFailed(event_buf[5]));
        }
        let mut addr = [0u8; 6];
        addr.copy_from_slice(&event_buf[6..12]);
        Ok(BluetoothAddress::new(addr))
    }

    pub fn set_scan_enable(&self, mode: ScanEnable, event_buf: &mut [u8]) -> Result<()> {
        let params = [mode as u8];
        let used = self.command_complete(HciOpcode::WRITE_SCAN_ENABLE, &params, event_buf)?;
        if used < 6 {
            return Err(Error::MalformedEvent);
        }
        if event_buf[5] != 0 {
            return Err(Error::CommandFailed(event_buf[5]));
        }
        Ok(())
    }

    pub fn set_event_mask(&self, mask: u64) -> Result<()> {
        self.send_hci_command(HciOpcode::SET_EVENT_MASK, &mask.to_le_bytes())
    }

    pub fn set_authentication_enable(&self, enable: bool) -> Result<()> {
        self.send_hci_command(HciOpcode::WRITE_AUTH_ENABLE, &[u8::from(enable)])
    }

    pub fn start_inquiry(&self, lap: [u8; 3], inquiry_length: u8, max_responses: u8) -> Result<()> {
        let params = [lap[0], lap[1], lap[2], inquiry_length, max_responses];
        self.send_hci_command(HciOpcode::INQUIRY, &params)
    }

    pub fn create_connection(
        &self,
        address: BluetoothAddress,
        packet_type: u16,
        page_scan_rep_mode: u8,
        page_scan_mode: u8,
        clock_offset: u16,
        allow_role_switch: bool,
    ) -> Result<()> {
        let mut params = [0u8; 13];
        params[0..6].copy_from_slice(&address.bytes);
        params[6..8].copy_from_slice(&packet_type.to_le_bytes());
        params[8] = page_scan_rep_mode;
        params[9] = page_scan_mode;
        params[10..12].copy_from_slice(&clock_offset.to_le_bytes());
        params[12] = u8::from(allow_role_switch);
        self.send_hci_command(HciOpcode::CREATE_CONNECTION, &params)
    }

    pub fn remote_name_request(
        &self,
        address: BluetoothAddress,
        page_scan_rep_mode: u8,
        page_scan_mode: u8,
        clock_offset: u16,
    ) -> Result<()> {
        let mut params = [0u8; 10];
        params[0..6].copy_from_slice(&address.bytes);
        params[6] = page_scan_rep_mode;
        params[7] = page_scan_mode;
        params[8..10].copy_from_slice(&clock_offset.to_le_bytes());
        self.send_hci_command(HciOpcode::REMOTE_NAME_REQUEST, &params)
    }

    pub fn pin_code_request_reply(&self, address: BluetoothAddress, pin: &[u8]) -> Result<()> {
        if pin.len() > 16 {
            return Err(Error::BufferTooSmall);
        }
        let mut params = [0u8; 23];
        params[0..6].copy_from_slice(&address.bytes);
        params[6] = pin.len() as u8;
        params[7..7 + pin.len()].copy_from_slice(pin);
        self.send_hci_command(HciOpcode::PIN_CODE_REQUEST_REPLY, &params)
    }

    pub fn pin_code_request_negative_reply(&self, address: BluetoothAddress) -> Result<()> {
        self.send_hci_command(HciOpcode::PIN_CODE_REQUEST_NEG_REPLY, &address.bytes)
    }

    pub fn link_key_request_reply(&self, address: BluetoothAddress, key: &[u8; 16]) -> Result<()> {
        let mut params = [0u8; 22];
        params[0..6].copy_from_slice(&address.bytes);
        params[6..22].copy_from_slice(key);
        self.send_hci_command(HciOpcode::LINK_KEY_REQUEST_REPLY, &params)
    }

    pub fn link_key_request_negative_reply(&self, address: BluetoothAddress) -> Result<()> {
        self.send_hci_command(HciOpcode::LINK_KEY_REQUEST_NEG_REPLY, &address.bytes)
    }

    pub fn command_complete(
        &self,
        opcode: HciOpcode,
        params: &[u8],
        event_buf: &mut [u8],
    ) -> Result<usize> {
        self.send_hci_command(opcode, params)?;
        self.expect_command_complete(opcode, event_buf)
    }

    pub fn parse_event<'a>(&self, buf: &'a [u8]) -> Result<HciEvent<'a>> {
        parse_event(buf)
    }

    pub fn scan_inquiry(
        &self,
        event_buf: &mut [u8],
        results: &mut [InquiryResult; MAX_INQUIRY_RESULTS],
    ) -> Result<usize> {
        self.start_inquiry(INQUIRY_GIAC, 0x08, 0x08)?;

        let mut count = 0usize;
        loop {
            let used = self.read_event(event_buf)?;
            let event = parse_event(&event_buf[..used])?;
            match event {
                HciEvent::InquiryResults {
                    count: event_count,
                    results: event_results,
                } => {
                    for event_result in event_results.iter().take(event_count) {
                        if count == results.len() {
                            break;
                        }
                        results[count] = *event_result;
                        count += 1;
                    }
                }
                HciEvent::InquiryComplete { status } => {
                    return if status == 0 {
                        Ok(count)
                    } else {
                        Err(Error::CommandFailed(status))
                    };
                }
                HciEvent::CommandStatus {
                    opcode: HciOpcode::INQUIRY,
                    status,
                } if status != 0 => return Err(Error::CommandFailed(status)),
                HciEvent::PinCodeRequest { address } => {
                    self.pin_code_request_reply(address, b"0000")?;
                }
                _ => {}
            }
        }
    }

    fn expect_command_complete(&self, opcode: HciOpcode, event_buf: &mut [u8]) -> Result<usize> {
        loop {
            let used =
                self.read_usb_buffer(IOCTL_USB_INTRMSG, self.event_in_endpoint, event_buf)?;
            if used < 2 || used < 2 + event_buf[1] as usize {
                return Err(Error::MalformedEvent);
            }
            if event_buf[0] == HCI_EVENT_COMMAND_COMPLETE {
                if used < 6 {
                    return Err(Error::MalformedEvent);
                }
                let seen = u16::from_le_bytes([event_buf[3], event_buf[4]]);
                if seen == opcode.raw() {
                    return Ok(used);
                }
            }
            unsafe { &mut *self.pending_events.get() }.push(&event_buf[..used])?;
        }
    }

    fn read_usb_buffer(&self, ioctl: i32, endpoint: u8, buf: &mut [u8]) -> Result<usize> {
        if buf.is_empty() {
            return Err(Error::BufferTooSmall);
        }
        if buf.len() > USB_TRANSFER_BUFFER_BYTES || buf.len() > u16::MAX as usize {
            return Err(Error::BufferTooSmall);
        }

        let endpoint_buf = UsbByte([endpoint]);
        let length_buf = UsbWord((buf.len() as u16).to_be_bytes());
        let mut scratch = UsbTransferBuffer([0u8; USB_TRANSFER_BUFFER_BYTES]);
        let data = &mut scratch.0[..buf.len()];
        unsafe { dc_invalidate_range(data.as_mut_ptr().cast(), data.len() as u32) };

        let rc = self.fd.ioctlv(
            ioctl,
            2,
            [
                IoctlvEntry::In(&endpoint_buf.0),
                IoctlvEntry::In(&length_buf.0),
                IoctlvEntry::Out(data),
            ],
        );

        if rc < 0 {
            Err(Error::Ipc(rc))
        } else {
            let used = (rc as usize).min(buf.len()).min(scratch.0.len());
            buf[..used].copy_from_slice(&scratch.0[..used]);
            Ok(used)
        }
    }

    fn write_usb_buffer(&self, ioctl: i32, endpoint: u8, buf: &[u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if buf.len() > USB_TRANSFER_BUFFER_BYTES || buf.len() > u16::MAX as usize {
            return Err(Error::BufferTooSmall);
        }

        let endpoint_buf = UsbByte([endpoint]);
        let length_buf = UsbWord((buf.len() as u16).to_be_bytes());
        let mut scratch = UsbTransferBuffer([0u8; USB_TRANSFER_BUFFER_BYTES]);
        scratch.0[..buf.len()].copy_from_slice(buf);
        let data = &mut scratch.0[..buf.len()];
        unsafe { dc_flush_range(data.as_ptr().cast(), data.len() as u32) };

        let rc = self.fd.ioctlv(
            ioctl,
            2,
            [
                IoctlvEntry::In(&endpoint_buf.0),
                IoctlvEntry::In(&length_buf.0),
                IoctlvEntry::Out(data),
            ],
        );

        if rc < 0 {
            Err(Error::Ipc(rc))
        } else {
            Ok(rc as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_event_queue_preserves_order() {
        let mut queue = PendingEvents::new();
        queue.push(&[1, 2, 3]).unwrap();
        queue.push(&[4, 5]).unwrap();
        let mut output = [0u8; 4];
        assert_eq!(queue.pop(&mut output).unwrap(), Some(3));
        assert_eq!(&output[..3], &[1, 2, 3]);
        assert_eq!(queue.pop(&mut output).unwrap(), Some(2));
        assert_eq!(&output[..2], &[4, 5]);
        assert_eq!(queue.pop(&mut output).unwrap(), None);
    }
}
