use core::cell::Cell;
use core::ffi::c_void;
use core::ptr;

use libdb_sys as debug;
use ogc_usbgecko as usbgecko;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("DEBUGGER PROBE");

    probe_usb_gecko_lock_retry(logger);
    probe_usb_gecko(logger);
    probe_usb_debugger(logger);
    probe_tcp_debugger(logger);
}

fn probe_usb_gecko_lock_retry(logger: &mut ProbeLogger) {
    let Some(queue) = usbgecko::wait_queue_probe(0) else {
        logger.record("USB Gecko wait queue initializes", false);
        return;
    };
    let tries = Cell::new(0);
    let wakeups = Cell::new(0);
    let callback_registered = Cell::new(false);
    let callback = Cell::new(None);
    let result = usbgecko::lock_retry_probe_with(
        queue,
        |unlock_callback| {
            tries.set(tries.get() + 1);
            callback_registered.set(unlock_callback.is_some());
            callback.set(unlock_callback);
            tries.get() > 1
        },
        |waiting_queue| {
            if waiting_queue == queue {
                if let Some(unlock_callback) = callback.get() {
                    // Invoke the production EXI callback; it broadcasts on this queue.
                    unsafe { unlock_callback(0, 0) };
                } else {
                    return -1;
                }
                wakeups.set(wakeups.get() + 1);
                0
            } else {
                -1
            }
        },
    );
    logger.record(
        "USB Gecko lock retries after callback wakeup",
        result == Ok(()) && tries.get() == 2 && wakeups.get() == 1 && callback_registered.get(),
    );
}

fn probe_usb_gecko(logger: &mut ProbeLogger) {
    logger.section("USB GECKO PROBE");

    for chan in 0..3 {
        let alive = usbgecko::is_gecko_alive(chan);
        logger.note_i32_index("usbgecko alive ch", chan, alive);
        if alive == 0 {
            continue;
        }

        usbgecko::flush(chan);

        let payload = *b"DBG";
        // SAFETY: `payload` is readable for `payload.len()` bytes and remains
        // alive for the duration of this synchronous transfer.
        let sent = unsafe {
            usbgecko::sendbuffer_safe_ex(
                chan,
                payload.as_ptr().cast(),
                payload.len() as i32,
                payload.len() as i32,
            )
        };
        logger.note_i32_index("usbgecko send ch", chan, sent);

        let mut recv = [0u8; 4];
        // SAFETY: `recv` is writable for `recv.len()` bytes, uniquely borrowed,
        // and remains alive for the duration of this synchronous transfer.
        let received = unsafe {
            usbgecko::recvbuffer_safe_ex(chan, recv.as_mut_ptr().cast(), recv.len() as i32, 1)
        };
        logger.note_i32_index("usbgecko recv ch", chan, received);

        let verify = usbgecko::flashverify(chan);
        logger.note_i32_index("usbgecko flashverify ch", chan, verify);

        let mut flash_id = [0u8; 2];
        // SAFETY: `flash_id` is writable for `flash_id.len()` bytes and remains
        // alive for the duration of the call.
        let flash_read =
            unsafe { usbgecko::flashread(chan, 0, flash_id.as_mut_ptr().cast(), flash_id.len()) };
        logger.note_i32_index("usbgecko flashread ch", chan, flash_read);
        if flash_read == 1 {
            let flash_word = u16::from_be_bytes(flash_id);
            logger.note_hex32_index("usbgecko flash id ch", chan, flash_word as u32);
            logger.record("usbgecko flash mismatch sentinel", flash_word != 0x1234);
        }

        let mut bounds = [0u8; 2];
        logger.record(
            "usbgecko flash bounds rejected",
            // SAFETY: `bounds` is writable for `bounds.len()` bytes and remains
            // alive for the duration of the call.
            unsafe { usbgecko::flashread(chan, 0x80000, bounds.as_mut_ptr().cast(), bounds.len()) }
                == 0,
        );
    }
}

fn probe_usb_debugger(logger: &mut ProbeLogger) {
    logger.section("USB DEBUGGER TRANSPORT");

    debug::debug_init(debug::GDBSTUB_DEVICE_USB, debug::GDBSTUB_DEF_CHANNEL);
    // SAFETY: the selected channel is owned by this probe until the interface
    // is closed below.
    let iface = unsafe { debug::init(debug::GDBSTUB_DEF_CHANNEL) };
    logger.record("usb debugger iface exported", !iface.is_null());
    if iface.is_null() {
        return;
    }

    let mut copied_iface = unsafe { *iface };
    copied_iface.fhndl = -1;
    let mut buffer = [0u8; 1];
    let read_rc = unsafe {
        copied_iface.read.expect("missing usb read callback")(
            &mut copied_iface,
            buffer.as_mut_ptr().cast::<c_void>(),
            buffer.len() as i32,
        )
    };
    let write_rc = unsafe {
        copied_iface.write.expect("missing usb write callback")(
            &mut copied_iface,
            buffer.as_ptr().cast::<c_void>(),
            buffer.len() as i32,
        )
    };
    logger.record(
        "usb copied callbacks preserve zero transfer result",
        read_rc == 0 && write_rc == 0,
    );
    logger.record(
        "usb callback result helper preserves zero",
        debug::usb_transfer_callback_result(0) == 0,
    );

    let open_rc = unsafe { ((*iface).open.expect("missing usb open callback"))(iface) };
    logger.note_i32("usb debugger open rc", open_rc);

    if open_rc == 0 {
        let mut buf = [0u8; 4];
        let write_rc = unsafe {
            ((*iface).write.expect("missing usb write callback"))(
                iface,
                buf.as_ptr().cast::<c_void>(),
                buf.len() as i32,
            )
        };
        logger.note_i32("usb debugger write rc", write_rc);
        let read_rc = unsafe {
            ((*iface).read.expect("missing usb read callback"))(
                iface,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as i32,
            )
        };
        logger.note_i32("usb debugger read rc", read_rc);
        let wait_rc = unsafe { ((*iface).wait.expect("missing usb wait callback"))(iface) };
        logger.note_i32("usb debugger wait rc", wait_rc);
        let close_rc = unsafe { ((*iface).close.expect("missing usb close callback"))(iface) };
        logger.note_i32("usb debugger close rc", close_rc);

        let reopen_rc = unsafe { ((*iface).open.expect("missing usb open callback"))(iface) };
        logger.note_i32("usb debugger reopen rc", reopen_rc);
        if reopen_rc == 0 {
            let _ = unsafe { ((*iface).close.expect("missing usb close callback"))(iface) };
        }
    }

    debug::_break();
}

fn probe_tcp_debugger(logger: &mut ProbeLogger) {
    logger.section("TCP DEBUGGER TRANSPORT");

    let mut buf = [0u8; 4];
    logger.record(
        "tcp bind invalid rejected",
        // SAFETY: the invalid socket is rejected before either null pointer is
        // accessed.
        unsafe { debug::tcpip_bind(-1, ptr::null_mut(), ptr::null_mut()) } == -1,
    );
    logger.record(
        "tcp listen invalid rejected",
        debug::tcpip_listen(-1, 1) == -1,
    );
    logger.record("tcp accept invalid rejected", debug::tcpip_accept(-1) == -1);
    logger.record(
        "tcp read invalid rejected",
        unsafe { debug::tcpip_read(-1, buf.as_mut_ptr().cast::<c_void>(), buf.len() as u32) } == -1,
    );
    logger.record(
        "tcp write invalid rejected",
        // SAFETY: `buf` remains readable for its full length during the call.
        unsafe { debug::tcpip_write(-1, buf.as_ptr().cast::<c_void>(), buf.len() as u32) } == -1,
    );
    debug::tcpip_starttimer(-1);
    debug::tcpip_stoptimer(-1);
    debug::tcpip_close(-1);

    let socket = debug::tcpip_socket();
    logger.record("tcp socket handle allocated", (0..32).contains(&socket));
    if socket >= 0 {
        debug::tcpip_close(socket);
    }

    debug::debug_init(debug::GDBSTUB_DEVICE_TCP, debug::GDBSTUB_DEF_TCPPORT);
    // SAFETY: null addresses request the backend defaults.
    let iface = unsafe {
        debug::tcpip_init(
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            debug::GDBSTUB_DEF_TCPPORT as u16,
        )
    };
    logger.record("tcp debugger iface exported", !iface.is_null());
    if iface.is_null() {
        return;
    }

    let mut copied_iface = unsafe { *iface };
    let mut buf = [0u8; 4];
    let read_rc = unsafe {
        copied_iface.read.expect("missing tcp read callback")(
            &mut copied_iface,
            buf.as_mut_ptr().cast::<c_void>(),
            buf.len() as i32,
        )
    };
    let write_rc = unsafe {
        copied_iface.write.expect("missing tcp write callback")(
            &mut copied_iface,
            buf.as_ptr().cast::<c_void>(),
            buf.len() as i32,
        )
    };
    logger.record(
        "tcp copied callbacks return zero without handle",
        read_rc == 0 && write_rc == 0,
    );
    logger.record(
        "tcp read size validator rejects negative length",
        debug::validate_tcp_read_size(-1).is_none(),
    );
    logger.record(
        "tcp read size validator accepts valid length",
        debug::validate_tcp_read_size(buf.len() as i32) == Some(buf.len() as i32),
    );

    // SAFETY: the previous interface has been closed and null addresses request
    // the backend defaults.
    let reinit = unsafe {
        debug::tcpip_init(
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            debug::GDBSTUB_DEF_TCPPORT as u16,
        )
    };
    logger.record("tcp debugger reinit exported", !reinit.is_null());
    if !reinit.is_null() {
        let _ = unsafe { ((*reinit).close.expect("missing tcp close callback"))(reinit) };
    }
}
