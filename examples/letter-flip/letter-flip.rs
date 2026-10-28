#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]
#[cfg(target_arch = "powerpc")]
use letter_flip::{card_storage, engine, narration, render, save, session, sound, voice};
#[cfg(not(target_arch = "powerpc"))]
fn main() {
    eprintln!("Letter Flip requires a Wii. Use the host tests to check shared code.");
    std::process::exit(2);
}
#[cfg(target_arch = "powerpc")]
#[used]
static ALLOCATOR_LINK: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;
#[cfg(target_arch = "powerpc")]
mod wii {
    use crate::card_storage::{CardStatus, CardStore};
    use crate::engine::ResultKind;
    use crate::render::{BAD, BG, Canvas, DIM, HEIGHT, Renderer, WIDTH};
    use crate::session::{Screen, Session, TICKS_PER_SECOND};
    use crate::voice::{VoiceError, VoicePlayer};
    use crate::{session, sound};
    use core::ffi::c_void;
    use core::ptr::null_mut;
    use ogc_controller::{Buttons, ControllerManager, ControllerSource};
    use ogc_core::mem_k0_to_k1;
    use ogc_system::{self as system, time};
    use ogc_video as video;
    use wpad_sys as wpad;
    struct Framebuffer;
    impl Canvas for Framebuffer {
        fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
            rect(x, y, w, h, color);
        }
    }
    static mut FRAMEBUFFER_A: *mut c_void = null_mut();
    static mut FRAMEBUFFER_B: *mut c_void = null_mut();
    static mut DISPLAY: *mut c_void = null_mut();
    static mut DRAW: *mut c_void = null_mut();
    static mut MODE: *mut video::GxrModeObj = null_mut();

    #[unsafe(no_mangle)]
    pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
        system::init();
        if !init_video() {
            system::report(format_args!("Letter Flip: video initialization failed\n"));
            return -1;
        }
        let mut controller = ControllerManager::new();
        controller.init();
        let mut wpad_pending = enable_remote(&mut controller);
        let mut voice = VoicePlayer::new();
        let (mut card, mut status) = CardStore::open();
        let data = card
            .as_ref()
            .map_or_else(crate::save::Data::default, |s| s.data().clone());
        let mut game = Session::new(data, time());
        let mut voice_error: Option<VoiceError> = None;
        let mut last_save = 0_u64;
        let mut last_input_report = 0_u64;
        let mut canvas = Framebuffer;
        let mut renderer = Renderer::new(&mut canvas);
        loop {
            let now = time();
            if wpad_pending {
                wpad_pending = enable_remote(&mut controller);
            }
            let snapshot = controller.scan_at(now);
            if now.saturating_sub(last_input_report) >= TICKS_PER_SECOND {
                for error in snapshot.errors() {
                    system::report(format_args!("Letter Flip: input error: {error:?}\n"));
                }
                last_input_report = now;
            }
            let mut input = None;
            let mut owner_connected = false;
            let mut any_connected = false;
            let mut owner_remote = false;
            for device in snapshot.controllers() {
                let (id, remote) = match device.source {
                    ControllerSource::GameCube(0) => (0, false),
                    ControllerSource::WiiBluetooth(n) => (4 + n, true),
                    _ => continue,
                };
                if !device.connected {
                    continue;
                }
                any_connected = true;
                if game.owner == Some(id) {
                    owner_connected = true;
                    owner_remote = remote;
                }
                let raw = button_bits(device.buttons_down);
                let mut keys = session::controls(raw, remote, game.data.settings.sideways);
                // Also inspect held buttons. A+B (or 1+2) must never submit an answer.
                let held = session::controls(
                    button_bits(device.buttons),
                    remote,
                    game.data.settings.sideways,
                );
                if held & (session::A | session::B) == session::A | session::B {
                    keys &= !(session::A | session::B);
                }
                if keys != 0 && (game.owner == Some(id) || game.owner.is_none()) && input.is_none()
                {
                    input = Some((id, keys));
                    owner_remote = remote;
                }
            }
            game.speech_busy = voice.busy();
            let events = game.step(now, input, owner_connected);
            if events.cancel_voice {
                if let Err(error) = voice.cancel() {
                    report_voice(error, &mut voice_error);
                }
                if let Err(error) = sound::stop() {
                    report_sound(error, &mut game);
                }
            }
            let options = game.data.settings;
            if let Err(error) = voice.set_preferences(options.voice, options.voice_volume) {
                report_voice(error, &mut voice_error);
            }
            if let Some(symbol) = events.speak {
                match voice.queue(symbol, now) {
                    Ok(()) => voice_error = None,
                    Err(error) => report_voice(error, &mut voice_error),
                }
            }
            if events.narration.names[0].is_some() {
                match voice.narrate(events.narration, now) {
                    Ok(()) => voice_error = None,
                    Err(error) => report_voice(error, &mut voice_error),
                }
            }
            if let Some(correct) = events.effect {
                if let Err(error) = sound::play(correct, options.effects, options.effects_volume) {
                    report_sound(error, &mut game);
                }
            }
            if events.preview {
                match voice.narrate(crate::narration::Narration::single("preview"), now) {
                    Ok(()) => voice_error = None,
                    Err(error) => report_voice(error, &mut voice_error),
                }
                if let Err(error) = sound::play(true, options.effects, options.effects_volume) {
                    report_sound(error, &mut game);
                }
            }
            if let Err(error) = voice.tick(now) {
                report_voice(error, &mut voice_error);
            }
            let mut save_notice: Option<&'static str> = None;
            let outside = matches!(game.screen, Screen::Title | Screen::Settings | Screen::Over);
            if events.retry_save && outside {
                drop(card.take());
                let (opened, new_status) = CardStore::open();
                card = opened;
                status = new_status;
                save_notice = match status {
                    CardStatus::NoCard => Some("no_card"),
                    CardStatus::Error => Some("save_failed"),
                    CardStatus::Recovered => Some("save_recovered"),
                    _ => None,
                };
                if let Some(storage) = card.as_ref() {
                    game.merge_disk(storage.data());
                    game.dirty = true;
                }
            }
            if card.as_ref().is_some_and(|store| !store.attached()) {
                status = CardStatus::NoCard;
            }
            if game.dirty
                && outside
                && card.is_some()
                && matches!(
                    status,
                    CardStatus::Ready | CardStatus::Saved | CardStatus::Pending
                )
                && (events.retry_save || now.saturating_sub(last_save) >= 3 * TICKS_PER_SECOND)
            {
                status = CardStatus::Saving;
                clear(BG);
                renderer.draw_screen(&game, owner_remote, any_connected);
                renderer.draw_save_status(status);
                flip();
                last_save = time();
                if let Some(storage) = card.as_mut() {
                    match storage.write(&game.data) {
                        Ok(()) => {
                            game.dirty = false;
                            status = CardStatus::Saved;
                            if events.retry_save {
                                save_notice = Some("saved");
                            }
                        }
                        Err(error) => {
                            status = CardStatus::Error;
                            if events.retry_save {
                                save_notice = Some("save_failed");
                            }
                            system::report(format_args!("Letter Flip: save failed ({error})\n"));
                        }
                    }
                }
            }
            if let Some(name) = save_notice {
                match voice.narrate(crate::narration::Narration::single(name), time()) {
                    Ok(()) => voice_error = None,
                    Err(error) => report_voice(error, &mut voice_error),
                }
            }
            let flash = options.flashes
                && game.screen == Screen::Feedback
                && game.feedback != ResultKind::Correct
                && game.feedback_elapsed < session::millis_ticks(100);
            clear(if flash { BAD } else { BG });
            renderer.draw_screen(&game, owner_remote, any_connected);
            let shown_status =
                if game.dirty && matches!(status, CardStatus::Ready | CardStatus::Saved) {
                    CardStatus::Pending
                } else {
                    status
                };
            renderer.draw_save_status(shown_status);
            if let Some(error) = voice_error {
                renderer.notice(
                    429,
                    1,
                    match error {
                        VoiceError::Missing => b"VOICE CLIP MISSING - YOU CAN STILL PLAY",
                        _ => b"VOICE ERROR - USE REPEAT TO TRY AGAIN",
                    },
                    DIM,
                );
            } else if game.audio_error {
                renderer.notice(429, 1, b"SOUND EFFECT ERROR - CHECK SETTINGS", DIM);
            }
            flip();
        }
    }
    fn enable_remote(controller: &mut ControllerManager) -> bool {
        match controller.enable_wpad() {
            code if code == wpad::WPAD_ERR_NONE as i32 => false,
            code if code == wpad::WPAD_ERR_NOT_READY as i32 => true,
            code => {
                system::report(format_args!("Letter Flip: WPAD start failed ({code})\n"));
                false
            }
        }
    }
    fn report_voice(error: VoiceError, status: &mut Option<VoiceError>) {
        if *status != Some(error) {
            system::report(format_args!("Letter Flip: voice error: {error:?}\n"));
        }
        *status = Some(error);
    }
    fn report_sound(error: sound::SoundError, game: &mut Session) {
        system::report(format_args!(
            "Letter Flip: sound effect error ({})\n",
            error.0
        ));
        game.audio_failure();
    }
    fn button_bits(buttons: Buttons) -> u16 {
        let mut bits = 0;
        for (button, bit) in [
            (Buttons::UP, session::UP),
            (Buttons::DOWN, session::DOWN),
            (Buttons::LEFT, session::LEFT),
            (Buttons::RIGHT, session::RIGHT),
            (Buttons::A, session::A),
            (Buttons::B, session::B),
            (Buttons::START, session::MENU),
            (Buttons::SELECT, session::REPEAT),
            (Buttons::Z, session::REPEAT),
            (Buttons::X, session::ONE),
            (Buttons::Y, session::TWO),
        ] {
            if buttons.contains(button) {
                bits |= bit;
            }
        }
        bits
    }
    fn rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
        unsafe {
            if DRAW.is_null() || MODE.is_null() {
                return;
            }
            let fw = (*MODE).fb_width as usize;
            let fh = (*MODE).xfb_height as usize;
            let x0 = (x.min(WIDTH).saturating_mul(fw) / WIDTH) & !1;
            let x1 = (x.saturating_add(w).min(WIDTH).saturating_mul(fw) / WIDTH) & !1;
            let y0 = y.min(HEIGHT).saturating_mul(fh) / HEIGHT;
            let y1 = y.saturating_add(h).min(HEIGHT).saturating_mul(fh) / HEIGHT;
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            let dst = mem_k0_to_k1(DRAW as usize) as *mut u32;
            for yy in y0..y1 {
                for xx in (x0..x1).step_by(2) {
                    dst.add(yy * (fw / 2) + xx / 2).write_volatile(color);
                }
            }
        }
    }
    fn clear(color: u32) {
        unsafe {
            clear_framebuffer(DRAW, color);
        }
    }
    fn clear_framebuffer(framebuffer: *mut c_void, color: u32) {
        unsafe {
            if MODE.is_null() || framebuffer.is_null() {
                return;
            }
            // Use the same uncached alias as rect() to avoid stale cache lines.
            let uncached = mem_k0_to_k1(framebuffer as usize) as *mut c_void;
            video::clear_frame_buffer(MODE, uncached, color);
        }
    }
    fn init_video() -> bool {
        unsafe {
            video::init();
            MODE = video::get_preferred_mode(null_mut());
            if MODE.is_null() {
                return false;
            }
            FRAMEBUFFER_A = system::allocate_framebuffer(MODE);
            FRAMEBUFFER_B = system::allocate_framebuffer(MODE);
            if FRAMEBUFFER_A.is_null() || FRAMEBUFFER_B.is_null() {
                return false;
            }
            DISPLAY = FRAMEBUFFER_A;
            DRAW = FRAMEBUFFER_B;
            clear_framebuffer(FRAMEBUFFER_A, BG);
            clear_framebuffer(FRAMEBUFFER_B, BG);
            video::configure(MODE);
            video::set_next_framebuffer(DISPLAY);
            video::set_black(false);
            video::flush();
            video::wait_vsync();
            video::wait_vsync();
        }
        true
    }
    fn flip() {
        unsafe {
            core::ptr::swap(
                core::ptr::addr_of_mut!(DRAW),
                core::ptr::addr_of_mut!(DISPLAY),
            );
            video::set_next_framebuffer(DISPLAY);
            video::flush();
            video::wait_vsync();
        }
    }
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    ogc_system_probe::probe_support::panic_screen()
}
