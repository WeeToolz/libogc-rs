#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[path = "game_2048/engine.rs"]
mod game_2048_engine;

#[path = "game_2048/save_data.rs"]
mod game_2048_save_data;

#[path = "game_2048/flow.rs"]
mod game_2048_flow;

#[cfg(target_arch = "powerpc")]
#[path = "support/pcm.rs"]
mod pcm;

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[used]
static ALLOCATOR_LINK: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;

#[cfg(target_arch = "powerpc")]
#[path = "controller_global_input.rs"]
mod controller_global_input;
#[cfg(target_arch = "powerpc")]
#[path = "controller_input.rs"]
mod controller_input;
#[cfg(feature = "wii-controllers")]
#[path = "half_controller_input.rs"]
mod half_controller_input;

#[cfg(target_arch = "powerpc")]
mod wii {
    use core::ffi::c_void;
    use core::ptr::null_mut;

    use asnd_sys as asnd;
    use ogc_card as card;
    use ogc_controller::Buttons;
    use ogc_core::mem_k0_to_k1;
    use ogc_system::{self as system, time};
    use ogc_ui::glyph_rows as shared_glyph_rows;
    use ogc_video as video;

    use crate::game_2048_engine::{self as engine, Direction, DirectionRepeat, Game};
    use crate::game_2048_flow::{self as flow, Screen};
    use crate::game_2048_save_data::{SAVE_BYTES, SaveData};
    use crate::pcm;
    use crate::{controller_global_input, controller_input};

    static mut FRAMEBUFFER_A: *mut c_void = null_mut();
    static mut FRAMEBUFFER_B: *mut c_void = null_mut();
    static mut DRAW_FRAMEBUFFER: *mut c_void = null_mut();
    static mut DISPLAY_FRAMEBUFFER: *mut c_void = null_mut();
    static mut DISPLAY_MODE: *mut video::GxrModeObj = null_mut();

    const SIZE: usize = 4;
    const CELLS: usize = SIZE * SIZE;
    const LOGICAL_WIDTH: usize = 640;
    const LOGICAL_HEIGHT: usize = 480;
    const MENU_TEXT_SCALE: usize = 3;

    const BOARD_X: usize = 172;
    const BOARD_Y: usize = 80;
    const TILE: usize = 64;
    const GAP: usize = 8;

    const MOVE_FRAMES: usize = 8;
    const SCREEN_FADE_FRAMES: u32 = 8;
    const MENU_COUNT: usize = 3;
    const SAMPLE_COUNT: usize = 2_048;
    const MUSIC_SAMPLE_COUNT: usize = 176_400;
    const MUSIC_BUFFER_SIZE: usize = (MUSIC_SAMPLE_COUNT + 31) & !31;
    const SAVE_FILENAME: &[u8] = b"RUST2048\0";
    static mut EFFECT_VOLUME: i32 = 96;
    static mut MUSIC_VOLUME: i32 = 36;
    static mut SOUND_PLAYER: pcm::Player<SAMPLE_COUNT> = pcm::Player::new();
    #[repr(align(32))]
    struct MusicSamples([u8; MUSIC_BUFFER_SIZE]);
    static mut MUSIC_SAMPLES: MusicSamples = MusicSamples([128; MUSIC_BUFFER_SIZE]);
    static mut MUSIC_VOICE: i32 = -1;
    static mut SAVE_AVAILABLE: bool = false;
    static mut SAVE_ERROR: bool = false;
    static mut UI_BRIGHTNESS: u8 = 255;

    struct CardSave {
        _mounted: card::MountedCard,
        file: card::sys::CardFile,
        record: SaveData,
        writable: bool,
    }

    impl CardSave {
        fn mount() -> Option<Self> {
            let gamecode = match card::sys::CardGameCode::new(*b"R204") {
                Ok(code) => code,
                Err(error) => {
                    system::report(format_args!("2048: invalid memory-card game ID: {error:?}"));
                    mark_save_error();
                    return None;
                }
            };
            let company = match card::sys::CardCompanyCode::new(*b"RS") {
                Ok(code) => code,
                Err(error) => {
                    system::report(format_args!(
                        "2048: invalid memory-card company ID: {error:?}"
                    ));
                    mark_save_error();
                    return None;
                }
            };
            let init_status = card::sys::init(Some(gamecode), Some(company));
            if init_status != card::sys::CARD_ERROR_READY {
                system::report(format_args!(
                    "2048: memory-card service initialization failed ({init_status})"
                ));
                mark_save_error();
                return None;
            }

            for slot in [card::CardSlot::A, card::CardSlot::B] {
                let Some(mounted) = mount_card(slot) else {
                    continue;
                };
                let mut file = card::sys::CardFile::default();
                // SAFETY: the path is a static NUL-terminated byte string and
                // `file` is a live output object for this synchronous call.
                let mut status = unsafe {
                    card::sys::open(slot.channel(), SAVE_FILENAME.as_ptr().cast(), &mut file)
                };
                let new_file = status == card::sys::CARD_ERROR_NOFILE;
                if new_file {
                    let sector_size = match card::sector_size(slot) {
                        Ok(size)
                            if size >= SAVE_BYTES as u32
                                && size % card::sys::CARD_READSIZE == 0 =>
                        {
                            size
                        }
                        Ok(size) => {
                            system::report(format_args!(
                                "2048: unsupported memory-card sector size ({size})"
                            ));
                            mark_save_error();
                            release_mount(mounted);
                            continue;
                        }
                        Err(error) => {
                            system::report(format_args!(
                                "2048: cannot read memory-card sector size ({error})"
                            ));
                            mark_save_error();
                            release_mount(mounted);
                            continue;
                        }
                    };
                    match card::free_blocks(slot) {
                        Ok(free) if free > 0 => {}
                        Ok(_) => {
                            system::report(format_args!(
                                "2048: memory card has no free blocks for a save"
                            ));
                            mark_save_error();
                            release_mount(mounted);
                            continue;
                        }
                        Err(error) => {
                            system::report(format_args!(
                                "2048: cannot check memory-card space ({error})"
                            ));
                            mark_save_error();
                            release_mount(mounted);
                            continue;
                        }
                    }
                    // SAFETY: CARD owns the filename until this synchronous
                    // create call returns; the static string remains valid.
                    status = unsafe {
                        card::sys::create(
                            slot.channel(),
                            SAVE_FILENAME.as_ptr().cast(),
                            sector_size,
                            &mut file,
                        )
                    };
                }
                if status != card::sys::CARD_ERROR_READY {
                    if status != card::sys::CARD_ERROR_NOCARD {
                        system::report(format_args!(
                            "2048: cannot open memory-card save ({status})"
                        ));
                        mark_save_error();
                    }
                    release_mount(mounted);
                    continue;
                }

                if new_file {
                    let record = SaveData::default();
                    let mut storage = Self {
                        _mounted: mounted,
                        file,
                        record,
                        writable: true,
                    };
                    if let Err(error) = storage.write_record(record) {
                        system::report(format_args!(
                            "2048: cannot initialize memory-card save ({error})"
                        ));
                        mark_save_error();
                        storage.writable = false;
                    }
                    return Some(storage);
                }

                let mut bytes = [0_u8; SAVE_BYTES];
                let read_status = card::read_slice(&mut file, &mut bytes, 0);
                if read_status != card::sys::CARD_ERROR_READY {
                    system::report(format_args!(
                        "2048: cannot read memory-card save ({read_status})"
                    ));
                    mark_save_error();
                    release_mount(mounted);
                    continue;
                }
                match SaveData::decode(&bytes) {
                    Ok(record) => {
                        return Some(Self {
                            _mounted: mounted,
                            file,
                            record,
                            writable: true,
                        });
                    }
                    Err(error) => {
                        system::report(format_args!(
                            "2048: memory-card save is invalid ({error:?}); preserving it"
                        ));
                        mark_save_error();
                        release_mount(mounted);
                        continue;
                    }
                }
            }
            None
        }

        fn record(&self) -> SaveData {
            self.record
        }

        fn save(&mut self, record: SaveData) -> Result<(), i32> {
            if !self.writable || !self._mounted.is_attached() {
                return Err(card::sys::CARD_ERROR_NOCARD);
            }
            self.write_record(record)?;
            self.record = record;
            Ok(())
        }

        fn write_record(&mut self, record: SaveData) -> Result<(), i32> {
            let bytes = record
                .encode()
                .map_err(|_| card::sys::CARD_ERROR_FATAL_ERROR)?;
            let status = card::write_slice(&mut self.file, &bytes, 0);
            if status == card::sys::CARD_ERROR_READY {
                Ok(())
            } else {
                Err(status)
            }
        }
    }

    fn mount_card(slot: card::CardSlot) -> Option<card::MountedCard> {
        let mut request = match card::mount_async_owned(slot) {
            Ok(request) => request,
            Err(status) => {
                if status != card::sys::CARD_ERROR_NOCARD {
                    system::report(format_args!(
                        "2048: cannot start memory-card mount ({status})"
                    ));
                    mark_save_error();
                }
                return None;
            }
        };
        for frame in 0..300 {
            if let card::CardAsyncStatus::Complete(_) = request.status() {
                match request.try_complete() {
                    Ok(card::CardMountCompletion::Mounted(mounted)) => return Some(mounted),
                    Ok(card::CardMountCompletion::Failed(status)) => {
                        if status != card::sys::CARD_ERROR_NOCARD {
                            system::report(format_args!(
                                "2048: memory-card mount failed ({status})"
                            ));
                            mark_save_error();
                        }
                        return None;
                    }
                    Err(pending) => request = pending,
                }
            }
            draw_card_loading(frame);
        }
        drop(request);
        system::report(format_args!("2048: memory-card mount timed out"));
        mark_save_error();
        None
    }

    fn mark_save_error() {
        unsafe {
            SAVE_ERROR = true;
        }
    }

    fn release_mount(mounted: card::MountedCard) {
        let slot = mounted.slot();
        let status = mounted.unmount();
        if status != card::sys::CARD_ERROR_READY {
            system::report(format_args!(
                "2048: memory-card slot {} unmount failed ({status})",
                slot.channel()
            ));
            mark_save_error();
        }
    }

    fn direction(input: controller_input::Input) -> Option<Direction> {
        let buttons = input.down | input.held;
        let axis_x = input.axes.left_x as i32;
        let axis_y = input.axes.left_y as i32;
        if buttons.contains(Buttons::UP) {
            Some(Direction::Up)
        } else if buttons.contains(Buttons::RIGHT) {
            Some(Direction::Right)
        } else if buttons.contains(Buttons::DOWN) {
            Some(Direction::Down)
        } else if buttons.contains(Buttons::LEFT) {
            Some(Direction::Left)
        } else if axis_x.abs() > axis_y.abs() && axis_x.abs() >= 14_000 {
            Some(if axis_x > 0 {
                Direction::Right
            } else {
                Direction::Left
            })
        } else if axis_y.abs() >= 14_000 {
            Some(if axis_y > 0 {
                Direction::Up
            } else {
                Direction::Down
            })
        } else {
            None
        }
    }

    const COLOR_BG: u32 = 0x1480_1480;
    const COLOR_PANEL: u32 = 0x407A_4086;
    const COLOR_PANEL_EDGE: u32 = 0x587A_5886;
    const COLOR_PANEL_HIGHLIGHT: u32 = 0x507C_5084;
    const COLOR_PANEL_SHADOW: u32 = 0x1C7E_1C82;
    const COLOR_EMPTY: u32 = 0x2880_2880;
    const COLOR_TEXT: u32 = 0xE880_E880;
    const COLOR_TEXT_DIM: u32 = 0xA080_A080;
    const COLOR_GAME_OVER_BG: u32 = COLOR_BG;

    const TILE_COLORS: [u32; 16] = [
        COLOR_EMPTY,
        0xD080_D080,
        0xD070_D0A0,
        0xC060_C0D0,
        0xA070_A0E0,
        0xA080_A0D0,
        0x9060_90D0,
        0x8060_80C0,
        0x8060_80B0,
        0x8070_80A0,
        0x9070_9070,
        0x9070_9060,
        0x9060_9070,
        0xA070_9070,
        0xB080_9080,
        0xC080_A080,
    ];

    #[unsafe(no_mangle)]
    pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
        let rmode = init_display();
        if rmode.is_null() {
            return -1;
        }

        clear_draw_framebuffer(rmode, COLOR_BG);
        swap_framebuffers();

        controller_global_input::init(
            cfg!(feature = "wii-controllers"),
            cfg!(feature = "wii-controllers"),
        );
        pcm::Player::<SAMPLE_COUNT>::init();
        draw_card_loading(0);
        let mut card_save = CardSave::mount();
        let mut save_data = card_save
            .as_ref()
            .map_or_else(SaveData::default, CardSave::record);
        unsafe {
            SAVE_AVAILABLE = card_save.as_ref().is_some_and(|storage| storage.writable);
            if card_save.is_some() {
                SAVE_ERROR = !SAVE_AVAILABLE;
            }
            EFFECT_VOLUME = save_data.effects_volume as i32;
            MUSIC_VOLUME = save_data.music_volume as i32;
        }
        start_music();
        run_autoplay_2048(card_save.as_mut(), &mut save_data)
    }

    fn run_autoplay_2048(mut card_save: Option<&mut CardSave>, save_data: &mut SaveData) -> ! {
        let mut game = Game::new(time() as u32 ^ 0x2048_7137);
        game.best_score = save_data.best_score;
        let mut demo = Game::new(time() as u32 ^ 0x2048_A11D);
        let mut screen = Screen::Title;
        let mut options_return = Screen::MainMenu;
        let mut menu_cursor = 0_usize;
        let mut options_cursor = 0_usize;
        let mut won_screen_seen = false;
        let mut animation_frame = MOVE_FRAMES;
        let mut animation_before = [0_u32; CELLS];
        let mut movement_animation: Option<engine::MoveResult> = None;
        let mut idle_frames = 0_u32;
        let mut previous_buttons = Buttons::empty();
        let mut direction_repeat = DirectionRepeat::default();
        let mut save_dirty = false;
        let mut frame_counter = 0_u32;
        let mut displayed_screen = screen;
        let mut screen_fade_frame = SCREEN_FADE_FRAMES;
        let mut last_save_attempt = 0_u32.wrapping_sub(300);
        loop {
            frame_counter = frame_counter.wrapping_add(1);
            if screen != displayed_screen {
                displayed_screen = screen;
                screen_fade_frame = 0;
            } else {
                screen_fade_frame = screen_fade_frame.saturating_add(1).min(SCREEN_FADE_FRAMES);
            }
            unsafe {
                UI_BRIGHTNESS = (screen_fade_frame * u8::MAX as u32 / SCREEN_FADE_FRAMES) as u8;
            }
            #[cfg(feature = "wii-controllers")]
            let input = controller_global_input::poll_with_profile(
                crate::half_controller_input::HalfControllerProfile::new(
                    Buttons::A,
                    Buttons::B,
                    Buttons::START,
                ),
            );
            #[cfg(not(feature = "wii-controllers"))]
            let input = controller_global_input::poll();
            let pressed = Buttons::from_bits(
                input.down.bits() | (input.held.bits() & !previous_buttons.bits()),
            );
            previous_buttons = input.held;
            let held_direction = direction(controller_input::Input {
                held: input.held,
                down: Buttons::empty(),
                axes: input.axes,
                connected: input.connected,
                ..input
            });
            let edge_direction = direction(controller_input::Input {
                held: pressed,
                down: pressed,
                axes: Default::default(),
                connected: input.connected,
                ..input
            });
            let direction_pressed = direction_repeat.sample(held_direction, edge_direction);
            #[cfg(feature = "action-trace")]
            let screen_before_input = screen;
            #[cfg(feature = "action-trace")]
            let menu_cursor_before_input = menu_cursor;
            #[cfg(feature = "action-trace")]
            let options_cursor_before_input = options_cursor;
            #[cfg(feature = "action-trace")]
            let option_value_before_input = unsafe {
                if options_cursor == 0 {
                    EFFECT_VOLUME
                } else {
                    MUSIC_VOLUME
                }
            };

            match screen {
                Screen::Title => {
                    idle_frames = idle_frames.wrapping_add(1);
                    if idle_frames.is_multiple_of(24) {
                        if let Some(dir) = choose_bot_move(&demo) {
                            if demo.apply(dir).is_none() {
                                demo.reset();
                            }
                        } else {
                            demo.reset();
                        }
                    }
                    if pressed.contains(Buttons::A) || pressed.contains(Buttons::START) {
                        screen = Screen::MainMenu;
                        menu_cursor = 0;
                        sfx_confirm();
                    }
                    draw_title(&demo, idle_frames);
                }
                Screen::MainMenu => {
                    if direction_pressed == Some(Direction::Up) {
                        menu_cursor = (menu_cursor + MENU_COUNT - 1) % MENU_COUNT;
                        sfx_move();
                    }
                    if direction_pressed == Some(Direction::Down) {
                        menu_cursor = (menu_cursor + 1) % MENU_COUNT;
                        sfx_move();
                    }
                    if pressed.contains(Buttons::B) {
                        screen = Screen::Title;
                        sfx_back();
                    }
                    if pressed.contains(Buttons::A) {
                        match menu_cursor {
                            0 => {
                                game.reset();
                                screen = Screen::Playing;
                                animation_frame = MOVE_FRAMES;
                                won_screen_seen = false;
                            }
                            1 => screen = Screen::HowTo,
                            _ => {
                                screen = Screen::Options;
                                options_return = Screen::MainMenu;
                            }
                        }
                        sfx_confirm();
                    }
                    draw_main_menu(menu_cursor);
                }
                Screen::HowTo => {
                    if pressed.contains(Buttons::B)
                        || pressed.contains(Buttons::A)
                        || pressed.contains(Buttons::START)
                    {
                        screen = Screen::MainMenu;
                        if pressed.contains(Buttons::B) {
                            sfx_back();
                        } else {
                            sfx_confirm();
                        }
                    }
                    draw_help();
                }
                Screen::Options => {
                    if matches!(direction_pressed, Some(Direction::Up | Direction::Down)) {
                        options_cursor = (options_cursor + 1) % 2;
                        sfx_move();
                    }
                    let delta = match direction_pressed {
                        Some(Direction::Left) => -16,
                        Some(Direction::Right) => 16,
                        _ if pressed.contains(Buttons::A) => 16,
                        _ => 0,
                    };
                    if delta != 0 {
                        unsafe {
                            if options_cursor == 0 {
                                EFFECT_VOLUME = (EFFECT_VOLUME + delta).clamp(0, 192);
                            } else {
                                MUSIC_VOLUME = (MUSIC_VOLUME + delta).clamp(0, 128);
                                set_music_volume(MUSIC_VOLUME);
                            }
                        }
                        save_dirty = true;
                        sfx_confirm();
                    }
                    if pressed.contains(Buttons::B) || pressed.contains(Buttons::START) {
                        screen = options_return;
                        if pressed.contains(Buttons::B) {
                            sfx_back();
                        }
                    }
                    draw_options(options_cursor, unsafe { EFFECT_VOLUME }, unsafe {
                        MUSIC_VOLUME
                    });
                }
                Screen::Playing => {
                    if input.connected && pressed.contains(Buttons::START) {
                        screen = Screen::Pause;
                        menu_cursor = 0;
                    } else if input.connected && animation_frame >= MOVE_FRAMES {
                        if let Some(dir) = direction_pressed {
                            let before = game.board;
                            if let Some(movement) = game.apply(dir) {
                                #[cfg(feature = "action-trace")]
                                controller_input::record_action(
                                    "autoplay_2048",
                                    "playing",
                                    "board_move",
                                    frame_counter,
                                    input,
                                );
                                animation_before = before;
                                movement_animation = Some(movement);
                                animation_frame = 0;
                                if game.won && !won_screen_seen {
                                    screen = Screen::Won;
                                    won_screen_seen = true;
                                    menu_cursor = 0;
                                    sfx_win();
                                } else if !game.can_move() {
                                    screen = Screen::GameOver;
                                    menu_cursor = 0;
                                    sfx_game_over();
                                } else if movement.merge_len > 0 {
                                    sfx_merge(movement.score_delta);
                                } else {
                                    sfx_move();
                                }
                            }
                        }
                    }
                    if animation_frame < MOVE_FRAMES {
                        animation_frame += 1;
                    }
                    draw_game_contents(
                        &game,
                        animation_frame,
                        &animation_before,
                        movement_animation.as_ref(),
                    );
                    if animation_frame >= MOVE_FRAMES {
                        movement_animation = None;
                    }
                    if !input.connected {
                        draw_disconnected_prompt();
                    }
                    swap_framebuffers();
                }
                Screen::Pause => {
                    if direction_pressed == Some(Direction::Up) {
                        menu_cursor = (menu_cursor + 3) % 4;
                        sfx_move();
                    }
                    if direction_pressed == Some(Direction::Down) {
                        menu_cursor = (menu_cursor + 1) % 4;
                        sfx_move();
                    }
                    if pressed.contains(Buttons::B) || pressed.contains(Buttons::START) {
                        screen = Screen::Playing;
                        if pressed.contains(Buttons::B) {
                            sfx_back();
                        }
                    } else if pressed.contains(Buttons::A) {
                        match menu_cursor {
                            0 => screen = Screen::Playing,
                            1 => screen = Screen::ConfirmRestart,
                            2 => {
                                screen = Screen::Options;
                                options_return = Screen::Pause;
                            }
                            _ => {
                                screen = Screen::ConfirmTitle;
                                menu_cursor = 1;
                            }
                        }
                        sfx_confirm();
                    }
                    draw_overlay(
                        &game,
                        b"PAUSED",
                        &[b"RESUME", b"RESTART", b"OPTIONS", b"TITLE"],
                        menu_cursor,
                    );
                }
                Screen::ConfirmRestart => {
                    if matches!(direction_pressed, Some(Direction::Up | Direction::Down)) {
                        menu_cursor ^= 1;
                        sfx_move();
                    }
                    if pressed.contains(Buttons::B) {
                        screen = Screen::Pause;
                        menu_cursor = 0;
                        sfx_back();
                    }
                    if pressed.contains(Buttons::A) {
                        let transition = flow::restart_transition(menu_cursor == 0);
                        if transition.reset_game {
                            game.reset();
                            won_screen_seen = false;
                        }
                        screen = transition.destination;
                        menu_cursor = 0;
                    }
                    draw_overlay(&game, b"RESTART?", &[b"YES", b"CANCEL"], menu_cursor);
                }
                Screen::ConfirmTitle => {
                    if matches!(direction_pressed, Some(Direction::Up | Direction::Down)) {
                        menu_cursor ^= 1;
                        sfx_move();
                    }
                    if pressed.contains(Buttons::B) {
                        screen = Screen::Pause;
                        menu_cursor = 0;
                        sfx_back();
                    } else if pressed.contains(Buttons::A) {
                        screen = if menu_cursor == 0 {
                            Screen::Title
                        } else {
                            Screen::Pause
                        };
                        menu_cursor = 0;
                        sfx_confirm();
                    }
                    draw_overlay(&game, b"QUIT TO TITLE?", &[b"YES", b"CANCEL"], menu_cursor);
                }
                Screen::Won => {
                    if pressed.contains(Buttons::B) {
                        screen = Screen::Title;
                        sfx_back();
                    }
                    if matches!(direction_pressed, Some(Direction::Up | Direction::Down)) {
                        menu_cursor ^= 1;
                        sfx_move();
                    }
                    if pressed.contains(Buttons::A) || pressed.contains(Buttons::START) {
                        screen = if menu_cursor == 0 {
                            if game.can_move() {
                                Screen::Playing
                            } else {
                                sfx_game_over();
                                Screen::GameOver
                            }
                        } else {
                            Screen::Title
                        };
                        sfx_confirm();
                    }
                    draw_overlay(&game, b"YOU WIN!", &[b"CONTINUE", b"TITLE"], menu_cursor);
                }
                Screen::GameOver => {
                    if matches!(direction_pressed, Some(Direction::Up | Direction::Down)) {
                        menu_cursor ^= 1;
                        sfx_move();
                    } else if pressed.contains(Buttons::A) {
                        if menu_cursor == 0 {
                            game.reset();
                            won_screen_seen = false;
                            screen = Screen::Playing;
                        } else {
                            screen = Screen::Title;
                        }
                        menu_cursor = 0;
                    }
                    draw_overlay(&game, b"GAME OVER", &[b"RETRY", b"TITLE"], menu_cursor);
                }
            }
            #[cfg(feature = "action-trace")]
            {
                if screen != screen_before_input {
                    controller_input::record_action(
                        "autoplay_2048",
                        screen_name(screen),
                        "screen_transition",
                        frame_counter,
                        input,
                    );
                }
                if menu_cursor != menu_cursor_before_input && screen == screen_before_input {
                    controller_input::record_action(
                        "autoplay_2048",
                        screen_name(screen),
                        "menu_selection",
                        frame_counter,
                        input,
                    );
                }
                if options_cursor != options_cursor_before_input
                    && screen == Screen::Options
                    && screen_before_input == Screen::Options
                {
                    controller_input::record_action(
                        "autoplay_2048",
                        "options",
                        "option_selection",
                        frame_counter,
                        input,
                    );
                }
                let option_value_after_input = unsafe {
                    if options_cursor == 0 {
                        EFFECT_VOLUME
                    } else {
                        MUSIC_VOLUME
                    }
                };
                if options_cursor == options_cursor_before_input
                    && option_value_after_input != option_value_before_input
                {
                    controller_input::record_action(
                        "autoplay_2048",
                        "options",
                        "option_value_change",
                        frame_counter,
                        input,
                    );
                }
            }
            if game.best_score > save_data.best_score {
                save_dirty = true;
            }
            if save_dirty
                && !matches!(screen, Screen::Playing | Screen::Options)
                && frame_counter.wrapping_sub(last_save_attempt) >= 300
            {
                last_save_attempt = frame_counter;
                let candidate = SaveData {
                    best_score: game.best_score.max(save_data.best_score),
                    effects_volume: unsafe { EFFECT_VOLUME as u8 },
                    music_volume: unsafe { MUSIC_VOLUME as u8 },
                };
                if let Some(storage) = card_save.as_deref_mut().filter(|storage| storage.writable) {
                    match storage.save(candidate) {
                        Ok(()) => {
                            *save_data = candidate;
                            save_dirty = false;
                            unsafe {
                                SAVE_ERROR = false;
                            }
                        }
                        Err(status) => {
                            system::report(format_args!("2048: save write failed ({status})"));
                            unsafe {
                                SAVE_ERROR = true;
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(feature = "action-trace")]
    fn screen_name(screen: Screen) -> &'static str {
        match screen {
            Screen::Title => "title",
            Screen::MainMenu => "main_menu",
            Screen::HowTo => "how_to",
            Screen::Options => "options",
            Screen::Playing => "playing",
            Screen::Pause => "pause",
            Screen::ConfirmRestart => "confirm_restart",
            Screen::ConfirmTitle => "confirm_title",
            Screen::Won => "won",
            Screen::GameOver => "game_over",
        }
    }

    fn choose_bot_move(game: &Game) -> Option<Direction> {
        let mut best: Option<(Direction, i64)> = None;
        for direction in Direction::ALL {
            let Some(movement) = engine::calculate_move(&game.board, direction) else {
                continue;
            };
            let empty = engine::empty_count(&movement.board) as i64;
            let max = *movement.board.iter().max().unwrap_or(&0) as i64;
            let corner = [0, 3, 12, 15]
                .iter()
                .any(|&i| movement.board[i] as i64 == max);
            let score =
                empty * 10_000 + max * if corner { 5 } else { 1 } + movement.score_delta as i64;
            if best.is_none_or(|(_, current)| score > current) {
                best = Some((direction, score));
            }
        }
        best.map(|(direction, _)| direction)
    }

    fn tile_number(value: u32) -> usize {
        value as usize
    }

    fn tile_color(value: u32) -> u32 {
        if value == 0 {
            return TILE_COLORS[0];
        }
        let rank = (32 - value.leading_zeros()).saturating_sub(1) as usize;
        TILE_COLORS[rank.min(TILE_COLORS.len() - 1)]
    }

    fn begin_screen() {
        unsafe {
            let mode = DISPLAY_MODE;
            if !mode.is_null() {
                draw_backdrop((*mode).fb_width as usize, (*mode).xfb_height as usize);
            }
        }
    }

    fn draw_backdrop(width: usize, height: usize) {
        draw_rect(0, 0, width, height, COLOR_BG);
        let logical_bottom = LOGICAL_HEIGHT.saturating_sub(6);
        draw_rect(32, 14, width.saturating_sub(64), 2, COLOR_PANEL_SHADOW);
        draw_rect(
            32,
            logical_bottom,
            width.saturating_sub(64),
            2,
            COLOR_PANEL_SHADOW,
        );
        draw_rect(
            32,
            16,
            2,
            logical_bottom.saturating_sub(14),
            COLOR_PANEL_SHADOW,
        );
        draw_rect(
            width.saturating_sub(34),
            16,
            2,
            logical_bottom.saturating_sub(14),
            COLOR_PANEL_SHADOW,
        );
    }

    fn draw_panel(x: usize, y: usize, width: usize, height: usize) {
        draw_rect(x + 4, y + 4, width, height, COLOR_PANEL_SHADOW);
        draw_rect(x, y, width, height, COLOR_PANEL_EDGE);
        draw_rect(
            x + 2,
            y + 2,
            width.saturating_sub(4),
            height.saturating_sub(4),
            COLOR_PANEL,
        );
        draw_rect(
            x + 2,
            y + 2,
            width.saturating_sub(4),
            2,
            COLOR_PANEL_HIGHLIGHT,
        );
    }

    fn draw_title(game: &Game, pulse: u32) {
        begin_screen();
        draw_panel(140, 24, 360, 76);
        draw_text(236, 38, 7, b"2048", COLOR_TEXT);
        draw_board_scaled(game, 251, 112, 28, 5, (pulse as usize / 5) % MOVE_FRAMES);
        draw_text(176, 324, 3, b"PRESS A OR START", COLOR_TEXT);
        draw_rect(
            202,
            370,
            236,
            4,
            TILE_COLORS[(pulse as usize / 8) % TILE_COLORS.len()],
        );
        draw_save_notice(444, 2, 360);
        swap_framebuffers();
    }

    fn draw_main_menu(cursor: usize) {
        begin_screen();
        draw_panel(112, 40, 416, 400);
        draw_text(248, 66, 6, b"2048", COLOR_TEXT);
        draw_mini_badge(172, 78, 2);
        draw_mini_badge(204, 78, 4);
        draw_mini_badge(408, 78, 8);
        draw_mini_badge(440, 78, 16);
        menu_item_centered(184, b"PLAY", cursor == 0);
        menu_item_centered(240, b"HOW TO PLAY", cursor == 1);
        menu_item_centered(296, b"OPTIONS", cursor == 2);
        draw_button_hint(222, 356, b'A', b"SELECT");
        draw_button_hint(354, 356, b'B', b"BACK");
        draw_save_notice(388, 2, 376);
        swap_framebuffers();
    }

    fn draw_help() {
        begin_screen();
        draw_panel(100, 40, 440, 400);
        draw_text(188, 66, 4, b"HOW TO PLAY", COLOR_TEXT);
        draw_text(
            150,
            138,
            2,
            b"MOVE WITH D PAD OR LEFT STICK",
            COLOR_TEXT_DIM,
        );
        draw_text(150, 180, 2, b"MATCH EQUAL NUMBERS TO MERGE", COLOR_TEXT_DIM);
        draw_text(150, 222, 2, b"REACH 2048 TO WIN", COLOR_TEXT_DIM);
        draw_text(150, 264, 2, b"KEEP GOING AFTER 2048", COLOR_TEXT_DIM);
        draw_text(142, 328, 2, b"START PAUSE", COLOR_TEXT);
        draw_button_hint(278, 322, b'A', b"CONFIRM");
        draw_button_hint(410, 322, b'B', b"BACK");
        swap_framebuffers();
    }

    fn draw_options(cursor: usize, effects: i32, music: i32) {
        begin_screen();
        draw_panel(136, 64, 368, 352);
        draw_text(236, 80, 4, b"OPTIONS", COLOR_TEXT);
        menu_item(178, 184, b"SOUND FX", cursor == 0);
        let effects_percent = volume_percent(effects, 192);
        draw_number_right(390, 184, 2, effects_percent, 48, COLOR_TEXT);
        draw_text(444, 184, 2, b"%", COLOR_TEXT);
        draw_volume_bar(178, 216, effects_percent);
        menu_item(178, 248, b"MUSIC", cursor == 1);
        let music_percent = volume_percent(music, 128);
        draw_number_right(390, 248, 2, music_percent, 48, COLOR_TEXT);
        draw_text(444, 248, 2, b"%", COLOR_TEXT);
        draw_volume_bar(178, 280, music_percent);
        draw_rect(160, 314, 320, 2, COLOR_PANEL_EDGE);
        draw_text(176, 330, 2, b"LEFT RIGHT CHANGE", COLOR_TEXT_DIM);
        draw_button_hint(406, 330, b'B', b"BACK");
        swap_framebuffers();
    }

    fn volume_percent(value: i32, maximum: i32) -> usize {
        (value.clamp(0, maximum) * 100 / maximum.max(1)) as usize
    }

    fn draw_mini_badge(x: usize, y: usize, value: u32) {
        draw_rect(x, y, 28, 28, COLOR_PANEL_EDGE);
        draw_rect(x + 2, y + 2, 24, 24, tile_color(value));
        let number = tile_number(value);
        let digits = count_digits(number);
        let scale = 2;
        let text_width = digits * 6 * scale;
        let text_x = x + (28 - text_width) / 2;
        let text_y = y + (28 - 7 * scale) / 2;
        draw_number(text_x, text_y, scale, number, COLOR_BG);
    }

    fn draw_volume_bar(x: usize, y: usize, percent: usize) {
        const WIDTH: usize = 272;
        draw_rect(x, y, WIDTH, 8, COLOR_EMPTY);
        let fill = WIDTH * percent.min(100) / 100;
        if fill > 0 {
            draw_rect(x, y, fill, 8, TILE_COLORS[3]);
        }
    }

    fn menu_item(x: usize, y: usize, label: &[u8], selected: bool) {
        if selected {
            let highlight_width = label.len() * 6 * MENU_TEXT_SCALE + 24;
            draw_rect(x - 12, y - 12, highlight_width, 42, COLOR_PANEL_EDGE);
            draw_rect(x - 10, y - 10, highlight_width - 4, 38, TILE_COLORS[2]);
            draw_rect(x - 10, y - 10, 3, 38, COLOR_TEXT);
        }
        draw_text(
            x,
            y,
            MENU_TEXT_SCALE,
            label,
            if selected { COLOR_BG } else { COLOR_TEXT },
        );
    }

    fn menu_item_centered(y: usize, label: &[u8], selected: bool) {
        let text_width = label.len() * 6 * MENU_TEXT_SCALE;
        let x = LOGICAL_WIDTH.saturating_sub(text_width) / 2;
        menu_item(x, y, label, selected);
    }

    fn draw_button_hint(x: usize, y: usize, button: u8, label: &[u8]) {
        const BADGE_WIDTH: usize = 24;
        const BADGE_HEIGHT: usize = 24;
        draw_rect(x, y, BADGE_WIDTH, BADGE_HEIGHT, COLOR_PANEL_HIGHLIGHT);
        draw_rect(
            x + 2,
            y + 2,
            BADGE_WIDTH - 4,
            BADGE_HEIGHT - 4,
            COLOR_PANEL_EDGE,
        );
        draw_text(x + 6, y + 5, 2, &[button], COLOR_TEXT);
        draw_text(x + BADGE_WIDTH + 6, y + 5, 2, label, COLOR_TEXT_DIM);
    }

    fn draw_save_notice(y: usize, scale: usize, max_width: usize) {
        let (label, color, save_available): (&[u8], u32, bool) = unsafe {
            if SAVE_ERROR {
                (b"CARD ERROR", COLOR_TEXT, SAVE_AVAILABLE)
            } else if !SAVE_AVAILABLE {
                (
                    b"NO CARD - SCORES NOT SAVED",
                    COLOR_TEXT_DIM,
                    SAVE_AVAILABLE,
                )
            } else {
                (b"CARD READY", COLOR_TEXT_DIM, SAVE_AVAILABLE)
            }
        };
        let max_characters = max_width / (6 * scale);
        let compact_label = if label.len() > max_characters {
            if !save_available {
                b"NO CARD - NO SAVE".as_slice()
            } else {
                b"CARD READY".as_slice()
            }
        } else {
            label
        };
        draw_text(
            320usize.saturating_sub(compact_label.len() * 3 * scale),
            y,
            scale,
            compact_label,
            color,
        );
    }

    fn draw_overlay(game: &Game, title: &[u8], items: &[&[u8]], cursor: usize) {
        draw_game_contents(game, MOVE_FRAMES, &game.board, None);
        draw_rect(112, 78, 416, 390, COLOR_PANEL_SHADOW);
        draw_panel(120, 86, 400, 382);
        let title_width = title.len() * 6 * 5;
        draw_text(
            LOGICAL_WIDTH.saturating_sub(title_width) / 2,
            116,
            5,
            title,
            COLOR_TEXT,
        );
        let (first_item_y, item_gap, hint_y) = if items.len() > 3 {
            (202, 42, 360)
        } else {
            (206, 48, 346)
        };
        for (index, item) in items.iter().enumerate() {
            let item_y = first_item_y + index * item_gap;
            menu_item_centered(item_y, item, index == cursor);
        }
        draw_button_hint(222, hint_y, b'A', b"SELECT");
        draw_button_hint(354, hint_y, b'B', b"BACK");
        swap_framebuffers();
    }

    fn draw_disconnected_prompt() {
        draw_panel(144, 170, 352, 112);
        draw_text(182, 190, 3, b"CONTROLLER LOST", COLOR_TEXT);
        draw_text(176, 236, 2, b"CONNECT TO CONTINUE", COLOR_TEXT_DIM);
    }

    fn draw_card_loading(frame: usize) {
        begin_screen();
        draw_panel(144, 120, 352, 232);
        draw_text(248, 148, 6, b"2048", COLOR_TEXT);
        draw_text(190, 256, 2, b"CHECKING MEMORY CARD", COLOR_TEXT_DIM);
        draw_rect(200, 300, 240, 12, COLOR_EMPTY);
        let progress = frame.min(300) * 236 / 300;
        draw_rect(202, 302, progress, 8, TILE_COLORS[2]);
        swap_framebuffers();
    }

    fn draw_game_contents(
        game: &Game,
        anim_frame: usize,
        before: &[u32; CELLS],
        movement: Option<&engine::MoveResult>,
    ) {
        let rmode = unsafe { DISPLAY_MODE };
        if rmode.is_null() {
            return;
        }

        unsafe {
            draw_backdrop((*rmode).fb_width as usize, (*rmode).xfb_height as usize);
        }

        draw_panel(172, 20, 92, 52);
        draw_panel(274, 20, 92, 52);
        draw_panel(376, 20, 92, 52);
        draw_text(182, 24, 2, b"MOVES", COLOR_TEXT_DIM);
        draw_rect(182, 40, 72, 2, COLOR_PANEL_EDGE);
        draw_u64(182, 44, game.moves as u64, 76, COLOR_TEXT);
        draw_text(284, 24, 2, b"SCORE", COLOR_TEXT_DIM);
        draw_rect(284, 40, 72, 2, COLOR_PANEL_EDGE);
        draw_u64(282, 44, game.score, 76, COLOR_TEXT);
        draw_text(386, 24, 2, b"BEST", COLOR_TEXT_DIM);
        draw_rect(386, 40, 72, 2, COLOR_PANEL_EDGE);
        draw_u64(384, 44, game.best_score, 76, COLOR_TEXT);

        draw_panel(172, 390, 296, 78);
        draw_text(184, 396, 2, b"D-PAD / STICK", COLOR_TEXT_DIM);
        draw_text(390, 396, 2, b"START", COLOR_TEXT_DIM);
        draw_text(184, 414, 2, b"MOVE", COLOR_TEXT);
        draw_text(390, 414, 2, b"PAUSE", COLOR_TEXT);
        draw_rect(184, 434, 272, 2, COLOR_PANEL_EDGE);
        draw_save_notice(440, 2, 272);

        draw_board(game, anim_frame, before, movement);
    }

    fn draw_board(
        game: &Game,
        anim_frame: usize,
        before: &[u32; CELLS],
        movement: Option<&engine::MoveResult>,
    ) {
        let board_pixels = SIZE * TILE + (SIZE + 1) * GAP;
        draw_panel(BOARD_X, BOARD_Y, board_pixels, board_pixels);
        let moving = movement.filter(|_| anim_frame < MOVE_FRAMES);
        for row in 0..SIZE {
            for col in 0..SIZE {
                let index = row * SIZE + col;
                let is_travel_target = moving.is_some_and(|step| {
                    step.travel[..step.travel_len as usize]
                        .iter()
                        .any(|travel| travel.to as usize == index)
                });
                let value = game.board[index];
                if value != 0 && is_travel_target {
                    let px = BOARD_X + GAP + col * (TILE + GAP);
                    let py = BOARD_Y + GAP + row * (TILE + GAP);
                    draw_tile(px, py, TILE, 0);
                    continue;
                }
                let inset = if game.last_spawn == Some(index) && anim_frame < MOVE_FRAMES {
                    4_usize.saturating_sub(anim_frame.min(4))
                } else {
                    0
                };
                let px = BOARD_X + GAP + col * (TILE + GAP) + inset;
                let py = BOARD_Y + GAP + row * (TILE + GAP) + inset;
                draw_tile(px, py, TILE.saturating_sub(inset * 2), value);
            }
        }
        if let Some(step) = moving {
            let frame = anim_frame.min(MOVE_FRAMES);
            let merge_is_visible = frame >= MOVE_FRAMES / 2;
            for (travel_index, travel) in step.travel[..step.travel_len as usize].iter().enumerate()
            {
                let merge_index = step.merged_at[..step.merge_len as usize]
                    .iter()
                    .position(|&target| target == travel.to);
                if let Some(merge_index) = merge_index {
                    if merge_is_visible {
                        continue;
                    }
                    let target = step.merged_at[merge_index];
                    let leader = step.travel[..step.travel_len as usize]
                        .iter()
                        .position(|item| item.to == target && item.from != target)
                        .or_else(|| {
                            step.travel[..step.travel_len as usize]
                                .iter()
                                .position(|item| item.to == target)
                        });
                    if leader != Some(travel_index) {
                        continue;
                    }
                }
                let from = travel.from as usize;
                let to = travel.to as usize;
                let from_x = BOARD_X + GAP + from % SIZE * (TILE + GAP);
                let from_y = BOARD_Y + GAP + from / SIZE * (TILE + GAP);
                let to_x = BOARD_X + GAP + to % SIZE * (TILE + GAP);
                let to_y = BOARD_Y + GAP + to / SIZE * (TILE + GAP);
                let px = from_x as isize
                    + (to_x as isize - from_x as isize) * frame as isize / MOVE_FRAMES as isize;
                let py = from_y as isize
                    + (to_y as isize - from_y as isize) * frame as isize / MOVE_FRAMES as isize;
                draw_tile(px.max(0) as usize, py.max(0) as usize, TILE, before[from]);
            }
            if merge_is_visible {
                let inset = MOVE_FRAMES.saturating_sub(frame).saturating_sub(1);
                for &merged_at in &step.merged_at[..step.merge_len as usize] {
                    let index = merged_at as usize;
                    let x = BOARD_X + GAP + index % SIZE * (TILE + GAP) + inset;
                    let y = BOARD_Y + GAP + index / SIZE * (TILE + GAP) + inset;
                    draw_tile(x, y, TILE.saturating_sub(inset * 2), game.board[index]);
                }
            }
        }
    }

    fn draw_board_scaled(
        game: &Game,
        origin_x: usize,
        origin_y: usize,
        tile: usize,
        gap: usize,
        anim_frame: usize,
    ) {
        let pixels = SIZE * tile + (SIZE + 1) * gap;
        draw_panel(origin_x, origin_y, pixels, pixels);

        for row in 0..SIZE {
            for col in 0..SIZE {
                let idx = row * SIZE + col;
                let value = game.board[idx];

                let mut inset = 0_usize;
                if game.last_spawn == Some(idx) && anim_frame < MOVE_FRAMES {
                    inset = 4_usize.saturating_sub(anim_frame.min(4));
                }

                let px = origin_x + gap + col * (tile + gap) + inset;
                let py = origin_y + gap + row * (tile + gap) + inset;
                let size = tile.saturating_sub(inset * 2);

                draw_tile(px, py, size, value);
            }
        }
    }

    fn draw_tile(x: usize, y: usize, size: usize, value: u32) {
        draw_rect(x, y, size, size, tile_color(value));

        if value == 0 {
            return;
        }

        draw_rect(x + 4, y + 4, size.saturating_sub(8), 3, COLOR_TEXT);
        draw_rect(x + 4, y + 4, 3, size.saturating_sub(8), COLOR_TEXT);
        draw_rect(
            x + 4,
            y + size.saturating_sub(7),
            size.saturating_sub(8),
            3,
            COLOR_PANEL_EDGE,
        );
        draw_rect(
            x + size.saturating_sub(7),
            y + 4,
            3,
            size.saturating_sub(8),
            COLOR_PANEL_EDGE,
        );

        let number = tile_number(value);
        let digits = count_digits(number);
        let preferred_scale = if digits <= 2 {
            4
        } else if digits <= 3 {
            3
        } else {
            2
        };
        let available_scale = size.saturating_sub(8) / (digits * 6);
        let scale = preferred_scale.min(available_scale.max(1));
        let text_w = digits * 6 * scale;
        let tx = x + size.saturating_sub(text_w) / 2;
        let ty = y + size / 2 - 4 * scale;

        let number_color = if value <= 8 { COLOR_BG } else { COLOR_TEXT };
        draw_number(tx, ty, scale, number, number_color);
    }

    fn count_digits(mut value: usize) -> usize {
        if value == 0 {
            return 1;
        }

        let mut count = 0;
        while value > 0 {
            count += 1;
            value /= 10;
        }
        count
    }

    fn draw_text(mut x: usize, y: usize, scale: usize, text: &[u8], color: u32) {
        for &ch in text {
            if ch == b' ' {
                x += 6 * scale;
            } else {
                draw_char(x, y, scale, ch, color);
                x += 6 * scale;
            }
        }
    }

    fn draw_number(mut x: usize, y: usize, scale: usize, mut value: usize, color: u32) {
        let mut digits = [0_u8; 10];
        let mut count = 0_usize;

        if value == 0 {
            digits[0] = b'0';
            count = 1;
        } else {
            while value > 0 && count < digits.len() {
                digits[count] = b'0' + (value % 10) as u8;
                value /= 10;
                count += 1;
            }
        }

        for i in (0..count).rev() {
            draw_char(x, y, scale, digits[i], color);
            x += 6 * scale;
        }
    }

    fn draw_number_right(x: usize, y: usize, scale: usize, value: usize, width: usize, color: u32) {
        let text_width = count_digits(value) * 6 * scale;
        draw_number(x + width.saturating_sub(text_width), y, scale, value, color);
    }

    fn draw_u64(x: usize, y: usize, value: u64, width: usize, color: u32) {
        let mut digits = [0_u8; 20];
        let mut remaining = value;
        let mut count = 0;
        if remaining == 0 {
            digits[0] = b'0';
            count = 1;
        } else {
            while remaining != 0 {
                digits[count] = b'0' + (remaining % 10) as u8;
                remaining /= 10;
                count += 1;
            }
        }
        let scale = if count <= 5 { 2 } else { 1 };
        let text_width = count * 6 * scale;
        let mut position = x + width.saturating_sub(text_width);
        for digit in digits[..count].iter().rev() {
            draw_char(position, y, scale, *digit, color);
            position += 6 * scale;
        }
    }

    fn draw_char(x: usize, y: usize, scale: usize, ch: u8, color: u32) {
        let glyph = glyph_rows(ch);

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    let pixel_x = x + col * scale;
                    let pixel_y = y + row * scale;
                    if scale.is_multiple_of(2) && pixel_x.is_multiple_of(2) {
                        draw_rect(pixel_x, pixel_y, scale, scale, color);
                    } else {
                        draw_luma_rect(pixel_x, pixel_y, scale, scale, color);
                    }
                }
            }
        }
    }

    fn draw_luma_rect(x: usize, y: usize, width: usize, height: usize, color: u32) {
        unsafe {
            let mode = DISPLAY_MODE;
            if mode.is_null() || DRAW_FRAMEBUFFER.is_null() {
                return;
            }
            let fb_width = (*mode).fb_width as usize;
            let xfb_height = (*mode).xfb_height as usize;
            if x >= fb_width || y >= LOGICAL_HEIGHT || width == 0 || height == 0 {
                return;
            }

            let logical_bottom = y.saturating_add(height).min(LOGICAL_HEIGHT);
            let y0 = y.saturating_mul(xfb_height) / LOGICAL_HEIGHT;
            let y1 = logical_bottom.saturating_mul(xfb_height) / LOGICAL_HEIGHT;
            let x1 = x.saturating_add(width).min(fb_width);
            if y0 >= y1 || x >= x1 {
                return;
            }

            let luma = fade_luma(color >> 24);
            let uncached = mem_k0_to_k1(DRAW_FRAMEBUFFER as usize) as *mut u32;
            let pair_width = fb_width / 2;
            for yy in y0..y1 {
                let row = yy * pair_width;
                for xx in x..x1 {
                    let pixel = uncached.add(row + xx / 2);
                    let current = *pixel;
                    *pixel = if xx % 2 == 0 {
                        (current & 0x00FF_FFFF) | (luma << 24)
                    } else {
                        (current & 0xFFFF_00FF) | (luma << 8)
                    };
                }
            }
        }
    }

    fn fade_luma(luma: u32) -> u32 {
        let brightness = unsafe { UI_BRIGHTNESS as u32 };
        16 + luma.saturating_sub(16) * brightness / u8::MAX as u32
    }

    fn glyph_rows(ch: u8) -> [u8; 7] {
        shared_glyph_rows(ch as char)
    }

    fn draw_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
        unsafe {
            let rmode = DISPLAY_MODE;
            if rmode.is_null() || DRAW_FRAMEBUFFER.is_null() {
                return;
            }

            let fb_width = (*rmode).fb_width as usize;
            let xfb_height = (*rmode).xfb_height as usize;

            if fb_width == 0 || xfb_height == 0 {
                return;
            }

            let x0 = x.min(fb_width.saturating_sub(1)) & !1;
            let scaled_y = y.saturating_mul(xfb_height) / LOGICAL_HEIGHT;
            let scaled_bottom = y.saturating_add(h).saturating_mul(xfb_height) / LOGICAL_HEIGHT;
            let y0 = scaled_y.min(xfb_height.saturating_sub(1));
            let x1 = x.saturating_add(w).min(fb_width) & !1;
            let y1 = scaled_bottom.min(xfb_height);

            if x1 <= x0 || y1 <= y0 {
                return;
            }

            let color = (fade_luma(color >> 24) << 24)
                | (color & 0x00FF_0000)
                | (fade_luma((color >> 8) & 0xFF) << 8)
                | (color & 0x0000_00FF);

            let uncached = mem_k0_to_k1(DRAW_FRAMEBUFFER as usize) as *mut u32;
            let pair_width = fb_width / 2;

            for yy in y0..y1 {
                let row = yy * pair_width;
                let mut xx = x0;

                while xx < x1 {
                    *uncached.add(row + xx / 2) = color;
                    xx += 2;
                }
            }
        }
    }

    fn init_display() -> *mut video::GxrModeObj {
        unsafe {
            video::init();

            let rmode = video::get_preferred_mode(null_mut());
            if rmode.is_null() {
                return null_mut();
            }

            FRAMEBUFFER_A = system::allocate_framebuffer(rmode);
            FRAMEBUFFER_B = system::allocate_framebuffer(rmode);
            if FRAMEBUFFER_A.is_null() || FRAMEBUFFER_B.is_null() {
                return null_mut();
            }

            DISPLAY_FRAMEBUFFER = FRAMEBUFFER_A;
            DRAW_FRAMEBUFFER = FRAMEBUFFER_B;
            DISPLAY_MODE = rmode;

            clear_draw_framebuffer(rmode, COLOR_BG);

            video::configure(rmode);
            video::set_next_framebuffer(DISPLAY_FRAMEBUFFER);
            video::set_black(false);
            video::flush();
            video::wait_vsync();
            video::wait_vsync();

            rmode
        }
    }

    fn clear_draw_framebuffer(rmode: *mut video::GxrModeObj, color: u32) {
        unsafe {
            if rmode.is_null() || DRAW_FRAMEBUFFER.is_null() {
                return;
            }

            video::clear_frame_buffer(rmode, DRAW_FRAMEBUFFER, color);
        }
    }

    fn swap_framebuffers() {
        unsafe {
            if DRAW_FRAMEBUFFER.is_null() || DISPLAY_FRAMEBUFFER.is_null() {
                return;
            }

            let next_display = DRAW_FRAMEBUFFER;
            let next_draw = DISPLAY_FRAMEBUFFER;
            video::set_next_framebuffer(next_display);
            video::flush();
            video::wait_vsync();
            DISPLAY_FRAMEBUFFER = next_display;
            DRAW_FRAMEBUFFER = next_draw;
        }
    }

    fn sfx_move() {
        play_tone(430, 520);
    }

    fn sfx_confirm() {
        play_tone(560, 880);
    }

    fn sfx_back() {
        play_tone(520, 360);
    }

    fn sfx_win() {
        play_tone(740, 1_480);
    }

    fn sfx_game_over() {
        play_tone(780, 210);
    }

    fn sfx_merge(score: u64) {
        let rank = (64 - score.max(1).leading_zeros()) as i32;
        let start = 620 + rank * 42;
        play_tone(start.min(1_400), (start + 260).min(1_900));
    }

    fn play_tone(start_hz: i32, end_hz: i32) {
        let player = unsafe { &*core::ptr::addr_of!(SOUND_PLAYER) };
        let volume = unsafe { EFFECT_VOLUME.min(255) };
        // SAFETY: the player lives for the full process and this single main
        // thread is the only submitter. ASND owns the selected buffer until its
        // voice completes, as enforced by Player::play.
        let result = unsafe {
            player.play(SAMPLE_COUNT, volume, |samples| {
                let sample_count = samples.len();
                let mut phase = 0_u32;
                for (index, sample) in samples.iter_mut().enumerate() {
                    let frequency =
                        start_hz + (end_hz - start_hz) * index as i32 / sample_count as i32;
                    phase = phase.wrapping_add(frequency.max(1) as u32);
                    let fade_in = (index * 128 / 256).min(128) as i32;
                    let fade_out = ((sample_count - index) * 128 / 512).min(128) as i32;
                    let envelope = fade_in.min(fade_out);
                    let amplitude = 28 * envelope / 128;
                    *sample = if phase % 44_100 < 22_050 {
                        (128 + amplitude) as u8
                    } else {
                        (128 - amplitude) as u8
                    };
                }
            })
        };
        if let Err(error) = result {
            system::report(format_args!("2048: sound playback failed: {error}"));
        }
    }

    fn start_music() {
        let voice = asnd::get_first_unused_voice();
        if !(0..asnd::MAX_SND_VOICES as i32).contains(&voice) {
            system::report(format_args!(
                "2048: no audio voice is available for music ({voice})"
            ));
            return;
        }
        let samples = unsafe {
            let buffer = &mut *core::ptr::addr_of_mut!(MUSIC_SAMPLES);
            let notes = [523_u32, 659, 784, 1_047, 784, 659, 587, 785];
            let samples_per_note = MUSIC_SAMPLE_COUNT / notes.len();
            let mut phase = 0_u32;
            for (index, sample) in buffer.0[..MUSIC_SAMPLE_COUNT].iter_mut().enumerate() {
                let note = notes[index / samples_per_note];
                phase = phase.wrapping_add(note);
                let cycle = phase % 44_100;
                let wave = if cycle < 22_050 {
                    (cycle * 2 / 22_050) as i32 - 1
                } else {
                    3 - (cycle * 2 / 22_050) as i32
                };
                let within_note = index % samples_per_note;
                let fade_in = (within_note * 128 / 220).min(128) as i32;
                let fade_out = ((samples_per_note - within_note) * 128 / 220).min(128) as i32;
                let envelope = fade_in.min(fade_out);
                let amplitude = 24 * envelope / 128;
                *sample = (128 + wave * amplitude) as u8;
            }
            buffer.0.as_mut_ptr()
        };
        let config = asnd::InfiniteVoiceConfig {
            voice,
            format: asnd::VOICE_MONO_8BIT_U,
            pitch: asnd::F44100HZ_PITCH,
            delay: 0,
            snd: samples.cast(),
            size_snd: MUSIC_SAMPLE_COUNT as i32,
            volume_l: unsafe { MUSIC_VOLUME },
            volume_r: unsafe { MUSIC_VOLUME },
        };
        // SAFETY: the aligned static sample buffer remains live for the entire
        // game process and is not changed after submission.
        let result = unsafe { asnd::set_infinite_voice(config) };
        if result == asnd::SND_OK {
            unsafe {
                MUSIC_VOICE = voice;
            }
        } else {
            system::report(format_args!(
                "2048: background music start failed ({result})"
            ));
        }
    }

    fn set_music_volume(volume: i32) {
        let voice = unsafe { MUSIC_VOICE };
        if voice < 0 {
            return;
        }
        let result = asnd::change_volume_voice(voice, volume, volume);
        if result != asnd::SND_OK {
            system::report(format_args!("2048: music volume update failed ({result})"));
        }
    }

    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
        unsafe {
            clear_draw_framebuffer(DISPLAY_MODE, COLOR_GAME_OVER_BG);
            swap_framebuffers();
        }

        loop {
            core::hint::spin_loop();
        }
    }
}
