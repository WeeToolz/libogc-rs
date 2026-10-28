#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[used]
static ALLOCATOR_LINK: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;

#[cfg(target_arch = "powerpc")]
#[path = "controller_input.rs"]
mod controller_input;
#[cfg(target_arch = "powerpc")]
#[path = "controller_scan.rs"]
mod controller_scan;
#[cfg(feature = "wii-controllers")]
#[path = "half_controller_input.rs"]
mod half_controller_input;

#[cfg(target_arch = "powerpc")]
mod snake_gc {
    use core::ptr::{addr_of_mut, null_mut};

    use super::{controller_input, controller_scan};
    use asnd_sys as asnd;
    use ogc_controller::{Buttons, ControllerManager, ControllerSnapshot};
    use ogc_core::mem_k0_to_k1;
    use ogc_system as system;
    use ogc_video as video;

    const SCREEN_W: usize = 640;
    const SCREEN_H: usize = 480;

    const GRID_W: i32 = 32;
    const GRID_H: i32 = 24;
    const CELL: i32 = 16;
    const BOARD_X: i32 = 64;
    const BOARD_Y: i32 = 48;

    const MAX_SNAKE: usize = 256;
    const START_TIMEOUT_FRAMES: u32 = 600;
    const STEP_FRAMES: u32 = 8;

    const fn xfb_color(y: u8, cb: u8, cr: u8) -> u32 {
        ((y as u32) << 24) | ((cb as u32) << 16) | ((y as u32) << 8) | cr as u32
    }

    const BLACK: u32 = xfb_color(16, 128, 128);
    const WHITE: u32 = xfb_color(235, 128, 128);
    const GREEN: u32 = xfb_color(145, 54, 34);
    const DARK_GREEN: u32 = xfb_color(95, 80, 60);
    const RED: u32 = xfb_color(82, 90, 240);
    const BLUE: u32 = xfb_color(41, 240, 110);
    const YELLOW: u32 = xfb_color(210, 16, 146);
    const BROWN: u32 = xfb_color(88, 97, 153);

    static mut FRAMEBUFFER_A: *mut u32 = null_mut();
    static mut FRAMEBUFFER_B: *mut u32 = null_mut();
    static mut DISPLAY_FRAMEBUFFER: *mut u32 = null_mut();
    static mut DRAW_FRAMEBUFFER: *mut u32 = null_mut();

    static mut DISPLAY_WIDTH: usize = SCREEN_W;
    static mut DISPLAY_HEIGHT: usize = SCREEN_H;
    static mut DISPLAY_STRIDE_WIDTH: usize = SCREEN_W;

    static mut CONTROLLERS: ControllerManager = ControllerManager::new();
    static mut LAST_CONTROLLER_CONNECTED: [bool; 4] = [false; 4];
    static mut LAST_CONTROLLER_HELD: [Buttons; 4] = [Buttons::empty(); 4];
    static mut LAST_CONTROLLER_DOWN: [Buttons; 4] = [Buttons::empty(); 4];

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Start,
        Human,
        Auto,
        GameOver,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Direction {
        Up,
        Down,
        Left,
        Right,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Point {
        x: i32,
        y: i32,
    }

    struct Game {
        mode: Mode,
        snake: [Point; MAX_SNAKE],
        len: usize,
        dir: Direction,
        next_dir: Direction,
        food: Point,
        score: u32,
        rng: u32,
        start_frame: u32,
        last_step_frame: u32,
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
        // Seed the arena bounds before allocating the framebuffers. application
        // uses the same startup order; the framebuffer allocator returns null
        // when system initialization has not set arena1.
        system::init();
        let rmode = init_video();
        if rmode.is_null() {
            return -1;
        }
        init_audio();

        let init = with_controllers(|controllers| {
            controller_input::start_manager(
                controllers,
                true,
                cfg!(feature = "wii-controllers"),
                |_| {},
            )
        });
        if let Err(status) = init {
            system::report(format_args!(
                "Snake: Bluetooth/Wii controller initialization failed ({status}); PAD and USB remain available\n"
            ));
        }

        for _ in 0..120 {
            let (snapshot, _) = scan_controllers();
            store_last_controller_snapshot(&snapshot);
            video::wait_vsync();
        }

        let mut frame = 0_u32;
        let mut game = Game::new(frame);

        loop {
            frame = frame.wrapping_add(1);

            let (snapshot, trace_input) = scan_controllers();
            store_last_controller_snapshot(&snapshot);
            #[cfg(feature = "action-trace")]
            let previous_mode = game.mode;

            match game.mode {
                Mode::Start => {
                    draw_start_screen(frame);

                    if pressed_start(&snapshot, trace_input) {
                        game.reset(Mode::Human, frame);
                        play_sound(SoundEffect::HumanStart);
                    } else if frame.wrapping_sub(game.start_frame) >= START_TIMEOUT_FRAMES {
                        game.reset(Mode::Auto, frame);
                        play_sound(SoundEffect::AutoStart);
                    }
                }

                Mode::Human => {
                    #[cfg(feature = "action-trace")]
                    let previous_direction = game.next_dir;
                    update_human_direction(&mut game, &snapshot, trace_input);
                    #[cfg(feature = "action-trace")]
                    if game.next_dir != previous_direction {
                        controller_input::record_action(
                            "snake-gc",
                            "human",
                            direction_action(game.next_dir),
                            frame,
                            trace_input,
                        );
                    }
                    step_game(&mut game, frame);
                    draw_game(&game);
                }

                Mode::Auto => {
                    update_auto_direction(&mut game);
                    step_game(&mut game, frame);
                    draw_game(&game);
                }

                Mode::GameOver => {
                    draw_game_over(&game);

                    if pressed_start(&snapshot, trace_input) {
                        game.reset(Mode::Human, frame);
                        play_sound(SoundEffect::HumanStart);
                    }
                }
            }
            #[cfg(feature = "action-trace")]
            if game.mode != previous_mode {
                controller_input::record_action(
                    "snake-gc",
                    mode_name(game.mode),
                    "screen_transition",
                    frame,
                    trace_input,
                );
            }

            swap_framebuffers(rmode);
        }
    }

    impl Game {
        fn new(frame: u32) -> Self {
            let mut game = Self {
                mode: Mode::Start,
                snake: [Point { x: 0, y: 0 }; MAX_SNAKE],
                len: 3,
                dir: Direction::Right,
                next_dir: Direction::Right,
                food: Point { x: 20, y: 12 },
                score: 0,
                rng: 0x1234_5678,
                start_frame: frame,
                last_step_frame: frame,
            };

            game.place_initial_snake();
            game
        }

        fn reset(&mut self, mode: Mode, frame: u32) {
            self.mode = mode;
            self.len = 3;
            self.dir = Direction::Right;
            self.next_dir = Direction::Right;
            self.score = 0;
            self.rng ^= frame.wrapping_mul(17);
            self.start_frame = frame;
            self.last_step_frame = frame;
            self.place_initial_snake();
            self.food = random_food(self);
        }

        fn place_initial_snake(&mut self) {
            self.snake[0] = Point { x: 8, y: 12 };
            self.snake[1] = Point { x: 7, y: 12 };
            self.snake[2] = Point { x: 6, y: 12 };
        }

        fn head(&self) -> Point {
            self.snake[0]
        }
    }

    fn pressed_start(snapshot: &ControllerSnapshot, input: controller_input::Input) -> bool {
        #[cfg(feature = "wii-controllers")]
        {
            let _ = snapshot;
            !((input.held | input.down) & (Buttons::A | Buttons::START)).is_empty()
        }
        #[cfg(not(feature = "wii-controllers"))]
        {
            let _ = input;
            let buttons = snapshot.player_buttons(0) | snapshot.player_buttons_down(0);
            !(buttons & (Buttons::A | Buttons::START)).is_empty()
        }
    }

    fn update_human_direction(
        game: &mut Game,
        snapshot: &ControllerSnapshot,
        input: controller_input::Input,
    ) {
        #[cfg(feature = "wii-controllers")]
        let (down_bits, held_bits) = (input.down, input.held);
        #[cfg(not(feature = "wii-controllers"))]
        let (down_bits, held_bits) = (snapshot.player_buttons_down(0), snapshot.player_buttons(0));
        #[cfg(not(feature = "wii-controllers"))]
        let _ = input;
        #[cfg(feature = "wii-controllers")]
        let _ = snapshot;
        let dpad = Buttons::LEFT | Buttons::RIGHT | Buttons::UP | Buttons::DOWN;

        let buttons = if !(down_bits & dpad).is_empty() {
            down_bits
        } else {
            held_bits
        };

        match game.dir {
            Direction::Left | Direction::Right => {
                if buttons.contains(Buttons::UP) {
                    queue_direction(game, Direction::Up);
                } else if buttons.contains(Buttons::DOWN) {
                    queue_direction(game, Direction::Down);
                }
            }
            Direction::Up | Direction::Down => {
                if buttons.contains(Buttons::LEFT) {
                    queue_direction(game, Direction::Left);
                } else if buttons.contains(Buttons::RIGHT) {
                    queue_direction(game, Direction::Right);
                }
            }
        }
    }

    fn queue_direction(game: &mut Game, dir: Direction) {
        if !directions_opposite(game.dir, dir) {
            game.next_dir = dir;
        }
    }

    fn directions_opposite(a: Direction, b: Direction) -> bool {
        matches!(
            (a, b),
            (Direction::Left, Direction::Right)
                | (Direction::Right, Direction::Left)
                | (Direction::Up, Direction::Down)
                | (Direction::Down, Direction::Up)
        )
    }

    fn update_auto_direction(game: &mut Game) {
        let head = game.head();
        let food = game.food;

        let preferred = if food.x > head.x {
            Direction::Right
        } else if food.x < head.x {
            Direction::Left
        } else if food.y > head.y {
            Direction::Down
        } else {
            Direction::Up
        };

        if direction_safe(game, preferred) {
            game.dir = preferred;
            game.next_dir = preferred;
            return;
        }

        for dir in [
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up,
        ] {
            if direction_safe(game, dir) {
                game.dir = dir;
                game.next_dir = dir;
                return;
            }
        }
    }

    fn direction_safe(game: &Game, dir: Direction) -> bool {
        let p = next_point(game.head(), dir);
        in_bounds(p) && !hits_body(game, p)
    }

    fn step_game(game: &mut Game, frame: u32) {
        if frame.wrapping_sub(game.last_step_frame) < STEP_FRAMES {
            return;
        }

        game.last_step_frame = frame;

        if !directions_opposite(game.dir, game.next_dir) {
            game.dir = game.next_dir;
        }

        let next = next_point(game.head(), game.dir);

        if !in_bounds(next) || hits_body(game, next) {
            game.mode = Mode::GameOver;
            play_sound(SoundEffect::Crash);
            return;
        }

        let ate = next == game.food;
        let new_len = if ate && game.len < MAX_SNAKE {
            game.len + 1
        } else {
            game.len
        };

        for i in (1..new_len).rev() {
            game.snake[i] = game.snake[i - 1];
        }

        game.snake[0] = next;
        game.len = new_len;

        if ate {
            game.score = game.score.wrapping_add(1);
            game.food = random_food(game);
            play_sound(SoundEffect::EatFood);
        }
    }

    fn next_point(point: Point, dir: Direction) -> Point {
        match dir {
            Direction::Up => Point {
                x: point.x,
                y: point.y - 1,
            },
            Direction::Down => Point {
                x: point.x,
                y: point.y + 1,
            },
            Direction::Left => Point {
                x: point.x - 1,
                y: point.y,
            },
            Direction::Right => Point {
                x: point.x + 1,
                y: point.y,
            },
        }
    }

    fn in_bounds(point: Point) -> bool {
        point.x >= 0 && point.y >= 0 && point.x < GRID_W && point.y < GRID_H
    }

    fn hits_body(game: &Game, point: Point) -> bool {
        for segment in game.snake.iter().take(game.len) {
            if *segment == point {
                return true;
            }
        }

        false
    }

    fn random_food(game: &mut Game) -> Point {
        loop {
            game.rng = game.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);

            let point = Point {
                x: ((game.rng >> 8) % GRID_W as u32) as i32,
                y: ((game.rng >> 16) % GRID_H as u32) as i32,
            };

            if !hits_body(game, point) {
                return point;
            }
        }
    }

    fn init_video() -> *mut video::GxrModeObj {
        video::init();

        // SAFETY: null requests the library-owned preferred mode object.
        let rmode = unsafe { video::get_preferred_mode(null_mut()) };
        if rmode.is_null() {
            return null_mut();
        }

        unsafe {
            FRAMEBUFFER_A = system::allocate_framebuffer(rmode).cast();
            FRAMEBUFFER_B = system::allocate_framebuffer(rmode).cast();

            if FRAMEBUFFER_A.is_null() || FRAMEBUFFER_B.is_null() {
                return null_mut();
            }

            DISPLAY_FRAMEBUFFER = FRAMEBUFFER_A;
            DRAW_FRAMEBUFFER = FRAMEBUFFER_A;

            DISPLAY_WIDTH = (*rmode).fb_width as usize;
            DISPLAY_HEIGHT = (*rmode).xfb_height as usize;
            DISPLAY_STRIDE_WIDTH = (((*rmode).fb_width as usize) + 15) & !15;
        }

        clear(BLACK);

        unsafe {
            DRAW_FRAMEBUFFER = FRAMEBUFFER_B;
        }

        clear(BLACK);

        // SAFETY: `rmode` is the live mode object returned above.
        unsafe { video::configure(rmode) };

        unsafe {
            let display_uncached =
                mem_k0_to_k1(DISPLAY_FRAMEBUFFER as usize) as *mut core::ffi::c_void;
            video::set_next_framebuffer(display_uncached);
        }

        video::set_black(false);
        video::flush();
        video::wait_vsync();

        unsafe {
            if ((*rmode).vi_tv_mode & 1) != 0 {
                video::wait_vsync();
            }
        }

        rmode
    }

    fn audio_enabled() -> bool {
        false
    }

    fn init_audio() {
        if audio_enabled() {
            asnd::init();
        }
    }

    #[derive(Clone, Copy)]
    enum SoundEffect {
        HumanStart,
        AutoStart,
        EatFood,
        Crash,
    }

    fn play_sound(effect: SoundEffect) {
        match effect {
            SoundEffect::HumanStart => play_tone(0x40),
            SoundEffect::AutoStart => play_tone(0x50),
            SoundEffect::EatFood => play_tone(0x70),
            SoundEffect::Crash => play_tone(0x10),
        }
    }

    fn play_tone(value: u8) {
        if !audio_enabled() {
            let _ = value;
            return;
        }

        #[repr(C, align(32))]
        struct AlignedTone([u8; 1024]);

        static mut TONE: AlignedTone = AlignedTone([0; 1024]);

        // SAFETY: the static tone buffer is initialized before voice submission
        // and is not mutated while playback can access it.
        let tone_ptr = unsafe { addr_of_mut!(TONE.0).cast::<u8>() };

        unsafe {
            for i in 0..1024 {
                tone_ptr.add(i).write_volatile(value);
            }
        }

        // SAFETY: `TONE` is 32-byte aligned, has static storage, and is not
        // modified again while this one-shot voice may read it.
        let _ = unsafe {
            asnd::set_voice(asnd::VoiceConfig {
                voice: -1,
                format: asnd::VOICE_MONO_8BIT,
                pitch: asnd::F44100HZ_PITCH,
                delay: 0,
                snd: tone_ptr.cast(),
                size_snd: 1024,
                volume_l: asnd::MID_VOLUME,
                volume_r: asnd::MID_VOLUME,
                callback: None,
            })
        };
    }

    fn draw_start_screen(frame: u32) {
        clear(BLACK);

        draw_rect(120, 78, 400, 76, WHITE);
        draw_rect(132, 90, 376, 52, BLACK);

        draw_text(182, 104, 4, b"SNAKE GC", WHITE);
        draw_text(16, 456, 2, b"DISPLAY R3", WHITE);
        draw_text(130, 210, 3, b"PRESS A START", GREEN);
        draw_text(128, 260, 2, b"AUTO IN 10 SECONDS", BLUE);
        draw_text(104, 300, 2, b"DPAD MOVE   A START", WHITE);
        draw_text(80, 360, 2, b"CTL SCAN  INPUT START", WHITE);

        let progress =
            ((frame % START_TIMEOUT_FRAMES) as usize * 360) / START_TIMEOUT_FRAMES as usize;
        draw_rect(140, 340, 360, 12, DARK_GREEN);
        draw_rect(140, 340, progress as i32, 12, YELLOW);

        draw_text(80, 380, 2, b"ANY", WHITE);

        unsafe {
            for chan in 0..4 {
                let y = 405 + chan as i32 * 16;
                let scan_seen = LAST_CONTROLLER_CONNECTED[chan];
                let held = LAST_CONTROLLER_HELD[chan];
                let down = LAST_CONTROLLER_DOWN[chan];
                let input_seen = !held.is_empty() || !down.is_empty();
                let start_seen = !((held | down) & (Buttons::A | Buttons::START)).is_empty();

                draw_rect(140, y, 40, 10, if scan_seen { GREEN } else { RED });
                draw_rect(190, y, 40, 10, if input_seen { BLUE } else { DARK_GREEN });
                draw_rect(240, y, 40, 10, if start_seen { YELLOW } else { DARK_GREEN });
            }
        }
    }

    fn scan_controllers() -> (ControllerSnapshot, controller_input::Input) {
        with_controllers(|controllers| {
            #[cfg(feature = "wii-controllers")]
            {
                controller_scan::scan_manager_wii(
                    controllers,
                    crate::half_controller_input::HalfControllerProfile::new(
                        Buttons::A,
                        Buttons::empty(),
                        Buttons::empty(),
                    ),
                )
            }
            #[cfg(not(feature = "wii-controllers"))]
            {
                controller_scan::scan_manager(controllers)
            }
        })
    }

    #[cfg(feature = "action-trace")]
    fn mode_name(mode: Mode) -> &'static str {
        match mode {
            Mode::Start => "start",
            Mode::Human => "human",
            Mode::Auto => "autoplay",
            Mode::GameOver => "game_over",
        }
    }

    #[cfg(feature = "action-trace")]
    fn direction_action(direction: Direction) -> &'static str {
        match direction {
            Direction::Up => "turn_up",
            Direction::Down => "turn_down",
            Direction::Left => "turn_left",
            Direction::Right => "turn_right",
        }
    }

    fn store_last_controller_snapshot(snapshot: &ControllerSnapshot) {
        unsafe {
            for idx in 0..4 {
                LAST_CONTROLLER_CONNECTED[idx] = false;
                LAST_CONTROLLER_HELD[idx] = Buttons::empty();
                LAST_CONTROLLER_DOWN[idx] = Buttons::empty();
            }

            for (idx, controller) in snapshot.controllers().iter().take(4).enumerate() {
                LAST_CONTROLLER_CONNECTED[idx] = controller.connected;
                LAST_CONTROLLER_HELD[idx] = controller.buttons;
                LAST_CONTROLLER_DOWN[idx] = controller.buttons_down;
            }
        }
    }

    fn with_controllers<R>(f: impl FnOnce(&mut ControllerManager) -> R) -> R {
        unsafe { f(&mut *addr_of_mut!(CONTROLLERS)) }
    }

    fn draw_game(game: &Game) {
        clear(BLACK);

        draw_rect(BOARD_X - 4, BOARD_Y - 4, GRID_W * CELL + 8, 4, WHITE);
        draw_rect(
            BOARD_X - 4,
            BOARD_Y + GRID_H * CELL,
            GRID_W * CELL + 8,
            4,
            WHITE,
        );
        draw_rect(BOARD_X - 4, BOARD_Y - 4, 4, GRID_H * CELL + 8, WHITE);
        draw_rect(
            BOARD_X + GRID_W * CELL,
            BOARD_Y - 4,
            4,
            GRID_H * CELL + 8,
            WHITE,
        );

        draw_apple(game.food);

        for i in (0..game.len).rev() {
            if i == 0 {
                draw_head(game.snake[i], game.dir);
            } else {
                draw_cell(game.snake[i], DARK_GREEN);
            }
        }

        draw_text(16, 16, 2, b"SCORE", WHITE);
        draw_number(120, 16, 2, game.score as usize, WHITE);

        if game.mode == Mode::Auto {
            draw_text(500, 16, 2, b"AUTO", BLUE);
        }
    }

    fn draw_game_over(game: &Game) {
        clear(BLACK);
        draw_text(170, 150, 5, b"GAME", RED);
        draw_text(170, 200, 5, b"OVER", RED);
        draw_text(160, 280, 3, b"SCORE", WHITE);
        draw_number(360, 280, 3, game.score as usize, WHITE);
        draw_text(130, 350, 2, b"PRESS A START", GREEN);
    }

    fn draw_cell(point: Point, color: u32) {
        draw_cell_rect(point, 2, 2, CELL - 4, CELL - 4, color);
    }

    fn draw_head(point: Point, direction: Direction) {
        draw_cell(point, GREEN);
        let eyes = match direction {
            Direction::Up => [(4, 3), (10, 3)],
            Direction::Down => [(4, 11), (10, 11)],
            Direction::Left => [(2, 4), (2, 10)],
            Direction::Right => [(12, 4), (12, 10)],
        };
        for (x, y) in eyes {
            draw_cell_rect(point, x, y, 2, 2, BLACK);
        }
    }

    fn draw_apple(point: Point) {
        // Use even horizontal coordinates for the two-pixel XFB color pairs.
        for (x, y, width, height, color) in [
            (8, 1, 2, 4, BROWN),
            (10, 2, 4, 2, GREEN),
            (4, 4, 4, 2, RED),
            (10, 4, 2, 2, RED),
            (2, 6, 12, 6, RED),
            (4, 12, 8, 2, RED),
            (6, 14, 4, 1, RED),
            (4, 7, 2, 2, WHITE),
        ] {
            draw_cell_rect(point, x, y, width, height, color);
        }
    }

    fn draw_cell_rect(point: Point, x: i32, y: i32, w: i32, h: i32, color: u32) {
        draw_rect(
            BOARD_X + point.x * CELL + x,
            BOARD_Y + point.y * CELL + y,
            w,
            h,
            color,
        );
    }

    fn clear(color: u32) {
        let draw_framebuffer = unsafe { DRAW_FRAMEBUFFER };

        if draw_framebuffer.is_null() {
            return;
        }

        // SAFETY: null requests the library-owned preferred mode object.
        let rmode = unsafe { video::get_preferred_mode(null_mut()) };

        if !rmode.is_null() {
            // SAFETY: the framebuffer was allocated for this live mode and is
            // uniquely written by the renderer here.
            unsafe {
                video::clear_frame_buffer(
                    rmode,
                    mem_k0_to_k1(draw_framebuffer as usize) as *mut core::ffi::c_void,
                    color,
                )
            };
        }
    }

    fn draw_rect(x: i32, y: i32, w: i32, h: i32, color: u32) {
        unsafe {
            if DRAW_FRAMEBUFFER.is_null() {
                return;
            }

            let width = DISPLAY_WIDTH;
            let height = DISPLAY_HEIGHT;
            let stride_width = DISPLAY_STRIDE_WIDTH;
            let words_per_row = stride_width / 2;

            if width == 0 || height == 0 || words_per_row == 0 {
                return;
            }

            let uncached = mem_k0_to_k1(DRAW_FRAMEBUFFER as usize) as *mut u32;

            for yy in 0..h {
                let py = y + yy;

                if !(0..height as i32).contains(&py) {
                    continue;
                }

                let x_start = x.max(0);
                let x_end = (x + w).min(width as i32);

                if x_start >= x_end {
                    continue;
                }

                let word_start = (x_start as usize) / 2;
                let word_end = (x_end as usize).div_ceil(2);

                for word_x in word_start..word_end {
                    let index = py as usize * words_per_row + word_x;
                    uncached.add(index).write_volatile(color);
                }
            }
        }
    }

    fn swap_framebuffers(_rmode: *mut video::GxrModeObj) {
        unsafe {
            if DRAW_FRAMEBUFFER.is_null() || DISPLAY_FRAMEBUFFER.is_null() {
                return;
            }

            let next_display = DRAW_FRAMEBUFFER;
            let next_draw = DISPLAY_FRAMEBUFFER;

            video::set_next_framebuffer(next_display.cast());
            video::flush();
            video::wait_vsync();

            DISPLAY_FRAMEBUFFER = next_display;
            DRAW_FRAMEBUFFER = next_draw;
        }
    }

    fn draw_text(mut x: i32, y: i32, scale: i32, text: &[u8], color: u32) {
        for &ch in text {
            if ch == b' ' {
                x += 6 * scale;
            } else {
                draw_char(x, y, scale, ch, color);
                x += 6 * scale;
            }
        }
    }

    fn draw_number(mut x: i32, y: i32, scale: i32, mut value: usize, color: u32) {
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

    fn draw_char(x: i32, y: i32, scale: i32, ch: u8, color: u32) {
        let glyph = glyph_rows(ch);

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                let mask = 1 << (4 - col);
                if bits & mask != 0 {
                    draw_rect(x + col * scale, y + row as i32 * scale, scale, scale, color);
                }
            }
        }
    }

    fn glyph_rows(ch: u8) -> [u8; 7] {
        match ch {
            b'A' => [
                0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            b'B' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
            ],
            b'C' => [
                0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
            ],
            b'D' => [
                0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
            ],
            b'E' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
            ],
            b'G' => [
                0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
            ],
            b'I' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
            ],
            b'K' => [
                0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
            ],
            b'L' => [
                0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
            ],
            b'M' => [
                0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
            ],
            b'N' => [
                0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
            ],
            b'O' => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            b'P' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
            ],
            b'R' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
            ],
            b'S' => [
                0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            b'T' => [
                0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            b'U' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            b'V' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
            ],
            b'Y' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            b'0' => [
                0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
            ],
            b'1' => [
                0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            b'2' => [
                0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
            ],
            b'3' => [
                0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
            ],
            b'4' => [
                0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
            ],
            b'5' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
            ],
            b'6' => [
                0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
            ],
            b'7' => [
                0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
            ],
            b'8' => [
                0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
            ],
            b'9' => [
                0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100,
            ],
            _ => [0; 7],
        }
    }

    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
        clear(RED);
        loop {
            core::hint::spin_loop();
        }
    }
}
