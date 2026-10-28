use core::fmt::Write;
use core::ptr::null_mut;

use ogc_controller::{ControllerError, ControllerManager};
use ogc_gx::{FrameConfig, GxColor, GxFifo};
use ogc_system as system;
use ogc_ui::{ControllerInputAdapter, ControllerInputConfig, GxCanvas, UiEvent};
use ogc_video as video;

use crate::probe_support::{self, ProbeLogger};
use crate::ui_input_probe_core::{UiInputProbe, UiProbeObservation};

struct DisplayGuard {
    previous_callback: video::ViRetraceCallback,
    previous_framebuffer: *mut core::ffi::c_void,
    logger: *mut ProbeLogger,
    restored: bool,
}

impl DisplayGuard {
    fn restore(&mut self) -> bool {
        if self.restored {
            return true;
        }
        let mut ok = true;
        if !self.previous_framebuffer.is_null() {
            // SAFETY: This is the framebuffer that was active before the probe
            // took ownership of VI and GX. It remains allocated by libogc.
            unsafe { video::set_next_framebuffer(self.previous_framebuffer) };
            video::flush();
            ok &= video::wait_vsync_timeout_us(100_000);
        } else {
            ok = false;
        }
        let replaced = video::set_post_retrace_callback(self.previous_callback);
        ok &= replaced.is_none();
        // SAFETY: `logger` points to the live logger for this monitor call.
        unsafe { (*self.logger).set_console_ready(true) };
        self.restored = true;
        ok
    }
}

impl Drop for DisplayGuard {
    fn drop(&mut self) {
        // Normal return paths call `restore` explicitly and record its result.
        // Drop is a best-effort fallback for an unexpected unwind.
        let _ = self.restore();
    }
}

/// Run the live UI probe with the real manager, input mapper, retained tree,
/// and GX canvas. A complete collection does not certify hardware acceptance.
pub fn monitor(logger: &mut ProbeLogger) -> i32 {
    logger.section("UI INPUT PROBE");
    let mut capture_complete = log_text(
        logger,
        "collection_candidate",
        &format_args!("STARTED hardware_verdict=PENDING"),
    );

    let mode = unsafe { video::get_preferred_mode(null_mut()) };
    if mode.is_null() {
        report_incomplete(logger, "preferred_video_mode_unavailable", capture_complete);
        return 1;
    }
    // SAFETY: the preferred mode pointer is returned by libogc and is valid
    // while the video subsystem remains initialized.
    let (vi_mode, width, height, xfb_height) = unsafe {
        (
            (*mode).vi_tv_mode,
            (*mode).fb_width,
            (*mode).efb_height,
            (*mode).xfb_height,
        )
    };
    if width == 0 || height == 0 || width > 1024 || height > 1024 || xfb_height == 0 {
        report_incomplete(logger, "invalid_video_dimensions", capture_complete);
        return 1;
    }

    let previous_framebuffer: *mut core::ffi::c_void = video::get_current_framebuffer().cast();
    if previous_framebuffer.is_null() {
        report_incomplete(logger, "previous_framebuffer_unavailable", capture_complete);
        return 1;
    }
    let frame_rate_hz = match video::video_standard_for_mode(vi_mode) {
        Some(video::VideoStandard::Pal) => 50,
        Some(video::VideoStandard::Ntsc) => 60,
        None => {
            report_incomplete(logger, "unsupported_video_standard", capture_complete);
            return 1;
        }
    };
    logger.note_u32("UI repeat frame rate Hz", frame_rate_hz);
    let framebuffer_a = unsafe { system::allocate_framebuffer(mode) };
    let framebuffer_b = unsafe { system::allocate_framebuffer(mode) };
    if framebuffer_a.is_null() || framebuffer_b.is_null() || framebuffer_a == framebuffer_b {
        report_incomplete(logger, "framebuffer_allocation_failed", capture_complete);
        return 1;
    }
    let framebuffer_a = probe_support::uncached_framebuffer(framebuffer_a);
    let framebuffer_b = probe_support::uncached_framebuffer(framebuffer_b);
    let Some(config_a) = FrameConfig::new(framebuffer_a, width, height, xfb_height) else {
        report_incomplete(logger, "invalid_frame_config_a", capture_complete);
        return 1;
    };
    let Some(config_b) = FrameConfig::new(framebuffer_b, width, height, xfb_height) else {
        report_incomplete(logger, "invalid_frame_config_b", capture_complete);
        return 1;
    };

    // SAFETY: one FIFO is kept alive for the full GX session. This process is
    // single-threaded and no other task owns GX.
    let _fifo = match unsafe { GxFifo::initialize_default_mem2() } {
        Ok(fifo) => fifo,
        Err(error) => {
            capture_complete &= log_text(logger, "gx_fifo_error", &format_args!("{error:?}"));
            report_incomplete(logger, "gx_fifo_initialization_failed", capture_complete);
            return 1;
        }
    };
    let previous_callback = video::set_post_retrace_callback(None);
    logger.set_console_ready(false);
    let mut display = DisplayGuard {
        previous_callback,
        previous_framebuffer,
        logger,
        restored: false,
    };

    let initialized = unsafe {
        ogc_gx::initialize_display(
            config_a,
            GxColor {
                r: 12,
                g: 18,
                b: 28,
                a: 255,
            },
        )
    };
    if !initialized {
        report_display_incomplete(
            logger,
            &mut display,
            "gx_display_initialization_failed",
            capture_complete,
        );
        return 1;
    }
    // SAFETY: GX and its FIFO are active and the selected render mode matches
    // the EFB and XFB configuration above.
    let mut canvas = match unsafe { GxCanvas::from_video_mode(&*mode) } {
        Ok(canvas) => canvas,
        Err(error) => {
            capture_complete &= log_text(logger, "gx_canvas_error", &format_args!("{error:?}"));
            report_display_incomplete(
                logger,
                &mut display,
                "gx_canvas_creation_failed",
                capture_complete,
            );
            return 1;
        }
    };

    let Some(input_config) = ControllerInputConfig::new(frame_rate_hz as u16) else {
        report_display_incomplete(
            logger,
            &mut display,
            "invalid_repeat_frame_rate",
            capture_complete,
        );
        return 1;
    };
    let mut adapter = ControllerInputAdapter::new(input_config);
    let mut probe = match UiInputProbe::new([width, height]) {
        Ok(probe) => probe,
        Err(error) => {
            capture_complete &= log_text(logger, "ui_tree_error", &format_args!("{error:?}"));
            report_display_incomplete(
                logger,
                &mut display,
                "ui_tree_creation_failed",
                capture_complete,
            );
            return 1;
        }
    };
    let disabled_hit_test = match probe.disabled_pointer_is_skipped() {
        Ok(skipped) => skipped,
        Err(error) => {
            capture_complete &= log_text(
                logger,
                "disabled_hit_test_error",
                &format_args!("{error:?}"),
            );
            false
        }
    };
    logger.note(if disabled_hit_test {
        "observation.disabled_control=SKIPPED_BY_PRODUCTION_HIT_TEST"
    } else {
        "observation.disabled_control=NOT_OBSERVED"
    });
    capture_complete &= log_text(
        logger,
        "disabled_control",
        &format_args!(
            "{}",
            if disabled_hit_test {
                "SKIPPED_BY_PRODUCTION_HIT_TEST"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );

    let mut manager = ControllerManager::new();
    manager.init();
    let bluetooth_rc = manager.enable_wpad();
    manager.enable_usb_hid();
    crate::input_probe::attach_trace(&mut manager);
    let mut complete = capture_complete;

    if bluetooth_rc != 0 {
        complete = false;
        logger.note_i32("Bluetooth HID initialization result", bluetooth_rc);
    }
    let mut frame_number = 0u32;
    let mut draw_a = true;
    let mut saw_device = false;
    let mut saw_activation = false;
    let mut saw_pointer_activation = false;
    let mut saw_pointer_target = false;
    let mut saw_dialog_scope = false;
    let mut saw_list_focus = false;
    let mut saw_grid_focus = false;
    let mut prior_input: Option<ogc_ui::UiInput> = None;
    let mut prior_identity = None;
    let mut first_frame = true;
    while !probe.finished() && frame_number < 9_000 {
        let snapshot = manager.scan_at(system::time());
        saw_device |= !snapshot.controllers().is_empty();
        for error in snapshot.errors() {
            if !log_controller_error(logger, error) {
                logger.note("controller_error_capture=FAILED");
            }
            complete = false;
        }
        let mapped = match adapter.update(&snapshot, [width, height]) {
            Ok(frame) => frame,
            Err(error) => {
                let captured = log_text(logger, "mapper_error", &format_args!("{error:?}"));
                logger.record("mapper error captured", captured);
                complete = false;
                break;
            }
        };
        let observation = match probe.update(mapped) {
            Ok(observation) => observation,
            Err(error) => {
                let captured = log_text(logger, "ui_update_error", &format_args!("{error:?}"));
                logger.record("UI update error captured", captured);
                complete = false;
                break;
            }
        };
        saw_activation |= matches!(observation.event, Some(UiEvent::Activated(_)));
        saw_pointer_target |= observation.target.is_some();
        saw_pointer_activation |= observation.mapped.pointer_activate
            && matches!(
                observation.event,
                Some(UiEvent::Activated(widget)) if Some(widget) == observation.target
            );
        saw_dialog_scope |= observation.scope_changed;
        saw_list_focus |= [observation.focus_before, observation.focus_after]
            .into_iter()
            .flatten()
            .any(|widget| probe.widget_name(widget) == Some("list"));
        saw_grid_focus |= [observation.focus_before, observation.focus_after]
            .into_iter()
            .flatten()
            .any(|widget| matches!(probe.widget_name(widget), Some("grid_a" | "grid_b")));
        if observation.scope_changed {
            adapter.reset();
        }
        let identity = observation
            .source
            .map(|source| (source, snapshot.controller_id(source)));
        let should_log = first_frame
            || observation.mapped != prior_input.unwrap_or_default()
            || observation.event.is_some()
            || observation.focus_before != observation.focus_after
            || identity != prior_identity;
        if should_log && !log_observation(logger, frame_number, &observation, &snapshot) {
            complete = false;
            break;
        }
        first_frame = false;
        prior_input = Some(observation.mapped);
        prior_identity = identity;
        if !crate::input_probe::write_trace_events(logger) {
            logger.note("trace_capture=FAILED");
            complete = false;
            break;
        }

        if let Err(error) = probe.draw(&mut canvas) {
            let captured = log_text(logger, "ui_draw_error", &format_args!("{error:?}"));
            logger.record("UI draw error captured", captured);
            complete = false;
            break;
        }
        let config = if draw_a { config_a } else { config_b };
        if !unsafe { ogc_gx::present_frame_and_wait(config) } {
            logger.note("collection_candidate=INCOMPLETE reason=gx_frame_wait_failed");
            complete = false;
            break;
        }
        // SAFETY: this framebuffer is allocated, aligned, and has been fully
        // rendered by GX before VI is asked to display it.
        unsafe { video::set_next_framebuffer(config.xfb) };
        video::flush();
        if !video::wait_vsync_timeout_us(100_000) {
            logger.note("collection_candidate=INCOMPLETE reason=vi_retrace_timeout");
            complete = false;
            break;
        }
        draw_a = !draw_a;
        frame_number = frame_number.saturating_add(1);
    }

    if frame_number >= 9_000 && !probe.finished() {
        logger.note("collection_candidate=INCOMPLETE reason=observation_window_expired");
        complete = false;
    }

    let controller_cleanup_ok =
        crate::input_probe::controller_cleanup::shutdown_controller_manager(logger, &mut manager);
    complete &= controller_cleanup_ok;
    if !crate::input_probe::write_trace_events(logger) {
        logger.note("cleanup_trace_capture=FAILED");
        complete = false;
    }

    let display_restored = display.restore();
    complete &= display_restored;
    logger.record("restore VI callback and framebuffer", display_restored);
    logger.note(if saw_device {
        "observation.device_input=OBSERVED"
    } else {
        "observation.device_input=NOT_OBSERVED"
    });
    complete &= log_text(
        logger,
        "observation.device_input",
        &format_args!(
            "{}",
            if saw_device {
                "OBSERVED"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );
    logger.note(if saw_activation {
        "observation.widget_activation=OBSERVED"
    } else {
        "observation.widget_activation=NOT_OBSERVED"
    });
    complete &= log_text(
        logger,
        "observation.widget_activation",
        &format_args!(
            "{}",
            if saw_activation {
                "OBSERVED"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );
    logger.note(if saw_pointer_target {
        "observation.pointer_target=OBSERVED"
    } else {
        "observation.pointer_target=NOT_OBSERVED"
    });
    complete &= log_text(
        logger,
        "observation.pointer_target",
        &format_args!(
            "{}",
            if saw_pointer_target {
                "OBSERVED"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );
    logger.note(if saw_pointer_activation {
        "observation.pointer_activation=OBSERVED"
    } else {
        "observation.pointer_activation=NOT_OBSERVED"
    });
    complete &= log_text(
        logger,
        "observation.pointer_activation",
        &format_args!(
            "{}",
            if saw_pointer_activation {
                "OBSERVED"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );
    logger.note(if saw_dialog_scope {
        "observation.dialog_scope=OBSERVED"
    } else {
        "observation.dialog_scope=NOT_OBSERVED"
    });
    complete &= log_text(
        logger,
        "observation.dialog_scope",
        &format_args!(
            "{}",
            if saw_dialog_scope {
                "OBSERVED"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );
    logger.note(if saw_list_focus {
        "observation.list_focus=OBSERVED"
    } else {
        "observation.list_focus=NOT_OBSERVED"
    });
    complete &= log_text(
        logger,
        "observation.list_focus",
        &format_args!(
            "{}",
            if saw_list_focus {
                "OBSERVED"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );
    logger.note(if saw_grid_focus {
        "observation.grid_focus=OBSERVED"
    } else {
        "observation.grid_focus=NOT_OBSERVED"
    });
    complete &= log_text(
        logger,
        "observation.grid_focus",
        &format_args!(
            "{}",
            if saw_grid_focus {
                "OBSERVED"
            } else {
                "NOT_OBSERVED"
            }
        ),
    );
    let all_required_observed = saw_device
        && saw_activation
        && saw_pointer_target
        && saw_pointer_activation
        && saw_dialog_scope
        && saw_list_focus
        && saw_grid_focus
        && disabled_hit_test
        && probe.finished();
    logger.record("required controller input observed", saw_device);
    logger.record("required UI activation observed", saw_activation);
    logger.record("required pointer target observed", saw_pointer_target);
    logger.record(
        "required pointer activation observed",
        saw_pointer_activation,
    );
    logger.record("required dialog scope observed", saw_dialog_scope);
    logger.record("required list focus observed", saw_list_focus);
    logger.record("required grid focus observed", saw_grid_focus);
    logger.record("disabled control excluded by hit test", disabled_hit_test);
    logger.record("operator finished probe sequence", probe.finished());
    complete &= logger.checks_passed();
    logger.note(if complete && all_required_observed {
        "collection_candidate=COMPLETE hardware_verdict=PENDING"
    } else {
        "collection_candidate=INCOMPLETE hardware_verdict=PENDING"
    });
    complete &= log_text(
        logger,
        "collection_candidate",
        &format_args!(
            "{} hardware_verdict=PENDING",
            if complete && all_required_observed {
                "COMPLETE"
            } else {
                "INCOMPLETE"
            }
        ),
    );
    if !all_required_observed {
        complete = false;
    }
    logger.summary();
    i32::from(!complete)
}

fn log_controller_error(logger: &mut ProbeLogger, error: &ControllerError) -> bool {
    let mut line = crate::event_line::EventLineBuf::new();
    if crate::ui_input_probe_core::write_controller_error_json(&mut line, error).is_err() {
        return false;
    }
    log_event(logger, &line)
}

fn report_incomplete(logger: &mut ProbeLogger, reason: &str, prior_capture_succeeded: bool) {
    logger.note("collection_candidate=INCOMPLETE hardware_verdict=PENDING");
    logger.record("failure details captured", prior_capture_succeeded);
    let captured = log_text(
        logger,
        "collection_candidate",
        &format_args!("INCOMPLETE reason={reason} hardware_verdict=PENDING"),
    );
    logger.record("incomplete collection record captured", captured);
    logger.summary();
}

fn report_display_incomplete(
    logger: &mut ProbeLogger,
    display: &mut DisplayGuard,
    reason: &str,
    prior_capture_succeeded: bool,
) {
    logger.note("collection_candidate=INCOMPLETE hardware_verdict=PENDING");
    logger.record("failure details captured", prior_capture_succeeded);
    let restored = display.restore();
    logger.record("restore VI callback and framebuffer", restored);
    let restore_captured = log_text(
        logger,
        "display_restore",
        &format_args!("success={restored}"),
    );
    logger.record("display restore result captured", restore_captured);
    let captured = log_text(
        logger,
        "collection_candidate",
        &format_args!("INCOMPLETE reason={reason} hardware_verdict=PENDING"),
    );
    logger.record("incomplete collection record captured", captured);
    logger.summary();
}

fn log_observation(
    logger: &mut ProbeLogger,
    frame_number: u32,
    observation: &UiProbeObservation,
    snapshot: &ogc_controller::ControllerSnapshot,
) -> bool {
    let mut line = crate::event_line::EventLineBuf::new();
    let pointer_source = observation.pointer_source;
    let identity = observation
        .source
        .and_then(|source| snapshot.controller_id(source));
    let pointer_identity = pointer_source.and_then(|source| snapshot.controller_id(source));
    let metadata = observation
        .source
        .and_then(|source| snapshot.metadata(source));
    let pointer_metadata = pointer_source.and_then(|source| snapshot.metadata(source));
    if crate::ui_input_probe_core::write_observation_json(
        &mut line,
        frame_number,
        observation,
        identity,
        pointer_identity,
        metadata,
        pointer_metadata,
    )
    .is_err()
    {
        return false;
    }
    log_event(logger, &line)
}

fn log_text(logger: &mut ProbeLogger, kind: &str, details: &core::fmt::Arguments<'_>) -> bool {
    let mut line = crate::event_line::EventLineBuf::new();
    if line.write_str("{\"type\":").is_err()
        || line.write_json_string(kind).is_err()
        || line.write_str(",\"details\":").is_err()
        || line
            .write_json_string(&alloc::format!("{details}"))
            .is_err()
        || line.write_char('}').is_err()
    {
        logger.note("event_format=FAILED");
        return false;
    }
    log_event(logger, &line)
}

fn log_event(logger: &mut ProbeLogger, line: &crate::event_line::EventLineBuf) -> bool {
    let Ok(record) = line.as_str() else {
        logger.note("event_format=FAILED");
        return false;
    };
    let Some(length) = record.len().checked_add(1) else {
        logger.note("event_capture=FAILED reason=length_overflow");
        return false;
    };
    let mut bytes = [0u8; 1301];
    if length > bytes.len() {
        logger.note("event_capture=FAILED reason=record_too_long");
        return false;
    }
    bytes[..record.len()].copy_from_slice(record.as_bytes());
    bytes[record.len()] = b'\n';
    let console_ready = logger.display().console_ready();
    #[cfg(feature = "agent-report")]
    if probe_support::agent_log_checked(&bytes[..length]).is_err() {
        logger.note("event_capture=FAILED reason=report_buffer");
        return false;
    }
    // The checked append above owns the report copy. Mute only the logger's
    // console mirror while it writes the same record to its durable log, so
    // `screen_write` does not append a second copy to the agent capture.
    logger.set_console_ready(false);
    let written = logger.write_event_record(record);
    logger.set_console_ready(console_ready);
    written
}
