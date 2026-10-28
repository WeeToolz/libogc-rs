//! Real-Wii acceptance probe. It uses production storage and audio, with separate test files.
#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]
#[cfg(target_arch = "powerpc")]
use letter_flip::{card_storage, narration, save, session, settings, sound, voice};
#[cfg(not(target_arch = "powerpc"))]
fn main() {
    eprintln!("This probe requires a real Wii. A host run is not a hardware pass.");
    std::process::exit(2);
}
#[cfg(target_arch = "powerpc")]
#[used]
static ALLOCATOR_LINK: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;
#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    use ogc_controller::{Buttons, ControllerManager, ControllerSource};
    use ogc_system as system;
    system::init();
    let display = ogc_system_probe::probe_support::ProbeDisplay::init();
    if !display.console_ready() {
        system::report(format_args!("Letter Flip probe: display failed\n"));
        return -1;
    }
    display.println("Letter Flip real-Wii check");
    display.println("This writes LFPROBE0 and LFPROBE1 on a memory card.");
    display.println("Normal game saves are not changed. Keep the card inserted.");
    display.println("Hold START/PLUS, then press A to start. Press B to cancel.");
    let mut input = ControllerManager::new();
    input.init();
    let mut pending = true;
    loop {
        if pending {
            match input.enable_wpad() {
                code if code == wpad_sys::WPAD_ERR_NONE as i32 => pending = false,
                code if code == wpad_sys::WPAD_ERR_NOT_READY as i32 => {}
                code => {
                    system::report(format_args!(
                        "Letter Flip probe: WPAD start failed ({code})\n"
                    ));
                    pending = false;
                }
            }
        }
        let snapshot = input.scan_at(system::time());
        for error in snapshot.errors() {
            system::report(format_args!("Letter Flip probe: input error {error:?}\n"));
        }
        let supported = snapshot.controllers().iter().filter(|d| {
            d.connected
                && matches!(
                    d.source,
                    ControllerSource::GameCube(0) | ControllerSource::WiiBluetooth(_)
                )
        });
        let mut confirmed = false;
        for d in supported {
            if d.buttons_down.contains(Buttons::B) {
                display.println("Cancelled. No probe files were written.");
                return 0;
            }
            if d.buttons.contains(Buttons::START) && d.buttons_down.contains(Buttons::A) {
                confirmed = true;
            }
        }
        if confirmed {
            break;
        }
        display.wait_vsync();
    }
    let result = match run() {
        Ok(()) => {
            display.println("PASS: two writes, read-back, remount, score and settings reload.");
            display.println("PASS: MP3 started and completed through the real audio backend.");
            display.println("Listen: the short effects and Jamie's voice must be audible.");
            0
        }
        Err(code) => {
            display.println("FAILED. Keep the card and its data. No recovery writes will run.");
            system::report(format_args!("Letter Flip hardware probe failed ({code})\n"));
            -1
        }
    };
    display.println("Press B to leave the probe.");
    loop {
        let snapshot = input.scan_at(system::time());
        for error in snapshot.errors() {
            system::report(format_args!("Letter Flip probe: input error {error:?}\n"));
        }
        if snapshot
            .controllers()
            .iter()
            .any(|d| d.connected && d.buttons_down.contains(Buttons::B))
        {
            return result;
        }
        display.wait_vsync();
    }
}
#[cfg(target_arch = "powerpc")]
fn run() -> Result<(), i32> {
    use card_storage::{CardStatus, CardStore};
    use ogc_system::{report, time};
    let (store, status) = CardStore::open_probe();
    if status != CardStatus::Ready {
        return Err(if status == CardStatus::NoCard {
            ogc_card::sys::CARD_ERROR_NOCARD
        } else {
            ogc_card::sys::CARD_ERROR_FATAL_ERROR
        });
    }
    let mut store = store.ok_or(ogc_card::sys::CARD_ERROR_NOCARD)?;
    let mut expected = save::Data::default();
    expected.settings.category = settings::Category::Numbers;
    expected.settings.chances = 17;
    expected.settings.cards = 30;
    expected.settings.voice_volume = 35;
    if !expected.record(expected.settings, 7) {
        return Err(ogc_card::sys::CARD_ERROR_FATAL_ERROR);
    }
    store.write(&expected)?;
    if !expected.record(expected.settings, 12) {
        return Err(ogc_card::sys::CARD_ERROR_FATAL_ERROR);
    }
    store.write(&expected)?;
    store.finish()?;
    let (reopened, status) = CardStore::open_probe();
    if status != CardStatus::Ready {
        return Err(ogc_card::sys::CARD_ERROR_FATAL_ERROR);
    }
    let reopened = reopened.ok_or(ogc_card::sys::CARD_ERROR_NOCARD)?;
    if reopened.data() != &expected {
        report(format_args!(
            "Letter Flip probe: reloaded bytes do not match\n"
        ));
        return Err(ogc_card::sys::CARD_ERROR_FATAL_ERROR);
    }
    reopened.finish()?;
    let mut voice = voice::VoicePlayer::new();
    voice.set_preferences(true, 35).map_err(|e| {
        report(format_args!("Letter Flip probe: voice settings {e:?}\n"));
        -1
    })?;
    for symbol in [b'A', b'z', b'0', b'9'] {
        voice.queue(symbol, time()).map_err(|e| {
            report(format_args!("Letter Flip probe: voice start {e:?}\n"));
            -1
        })?;
        while voice.busy() {
            voice.tick(time()).map_err(|e| {
                report(format_args!("Letter Flip probe: voice completion {e:?}\n"));
                -1
            })?;
            ogc_video::wait_vsync();
        }
        if !voice.completed() {
            return Err(-1);
        }
    }
    voice
        .narrate(narration::Narration::single("good_job"), time())
        .map_err(|e| {
            report(format_args!("Letter Flip probe: encouragement {e:?}\n"));
            -1
        })?;
    while voice.busy() {
        voice.tick(time()).map_err(|e| {
            report(format_args!(
                "Letter Flip probe: encouragement completion {e:?}\n"
            ));
            -1
        })?;
        ogc_video::wait_vsync();
    }
    if !voice.completed() {
        return Err(-1);
    }
    for correct in [true, false] {
        sound::play(correct, true, 35).map_err(|e| e.0)?;
        if asnd_sys::status_voice(1) != asnd_sys::SND_WORKING {
            return Err(-1);
        }
        let started = time();
        while time().saturating_sub(started) < session::millis_ticks(300) {
            ogc_video::wait_vsync();
        }
        if asnd_sys::status_voice(1) != asnd_sys::SND_UNUSED {
            return Err(-1);
        }
        sound::stop().map_err(|e| e.0)?;
    }
    Ok(())
}
#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    ogc_system_probe::probe_support::panic_screen()
}
