#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[used]
static ALLOCATOR_LINK: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;

#[cfg(target_arch = "powerpc")]
#[path = "controller_activity.rs"]
mod controller_activity;
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
#[cfg(any(feature = "action-trace", test))]
#[path = "sudoku_action.rs"]
mod sudoku_action;

#[cfg(target_arch = "powerpc")]
mod wii {
    use core::ffi::c_void;
    use core::ptr::null_mut;

    use ogc_core::mem_k0_to_k1;
    use ogc_system::{self as system, time};
    use ogc_video as video;

    #[cfg(feature = "action-trace")]
    use crate::sudoku_action;
    use crate::{controller_activity, controller_global_input, controller_input};
    use ogc_controller::Buttons;

    static mut FRAMEBUFFER_A: *mut c_void = null_mut();
    static mut FRAMEBUFFER_B: *mut c_void = null_mut();
    static mut DRAW_FRAMEBUFFER: *mut c_void = null_mut();
    static mut DISPLAY_FRAMEBUFFER: *mut c_void = null_mut();
    static mut DISPLAY_MODE: *mut video::GxrModeObj = null_mut();

    const N: usize = 9;
    const CELLS: usize = 81;

    const BOARD_X: usize = 76;
    const BOARD_Y: usize = 34;
    const CELL: usize = 44;
    const BOARD_PIXELS: usize = N * CELL;

    const STEP_FRAMES: usize = 4;
    const SOLVED_FRAMES: usize = 180;

    const COLOR_BG: u32 = 0x0080_0080;
    const COLOR_BOARD: u32 = 0x1080_1080;
    const COLOR_CELL: u32 = 0x4040_4040;
    const COLOR_GRID: u32 = 0x8080_8080;
    const COLOR_GRID_THICK: u32 = 0xEB80_EB80;
    const COLOR_GIVEN: u32 = 0xE080_E080;
    const COLOR_TRIAL: u32 = 0xA0F0_A0F0;
    const COLOR_BACKTRACK: u32 = 0x4C55_4CFF;
    const COLOR_TEXT: u32 = 0xEB80_EB80;
    const COLOR_TEXT_DIM: u32 = 0xA080_A080;
    const COLOR_SOLVED_BG: u32 = 0x962C_9615;
    const COLOR_PANIC: u32 = 0x4C55_4CFF;

    const PUZZLES: [[u8; CELLS]; 5] = [
        [
            5, 3, 0, 0, 7, 0, 0, 0, 0, 6, 0, 0, 1, 9, 5, 0, 0, 0, 0, 9, 8, 0, 0, 0, 0, 6, 0, 8, 0,
            0, 0, 6, 0, 0, 0, 3, 4, 0, 0, 8, 0, 3, 0, 0, 1, 7, 0, 0, 0, 2, 0, 0, 0, 6, 0, 6, 0, 0,
            0, 0, 2, 8, 0, 0, 0, 0, 4, 1, 9, 0, 0, 5, 0, 0, 0, 0, 8, 0, 0, 7, 9,
        ],
        [
            0, 0, 0, 2, 6, 0, 7, 0, 1, 6, 8, 0, 0, 7, 0, 0, 9, 0, 1, 9, 0, 0, 0, 4, 5, 0, 0, 8, 2,
            0, 1, 0, 0, 0, 4, 0, 0, 0, 4, 6, 0, 2, 9, 0, 0, 0, 5, 0, 0, 0, 3, 0, 2, 8, 0, 0, 9, 3,
            0, 0, 0, 7, 4, 0, 4, 0, 0, 5, 0, 0, 3, 6, 7, 0, 3, 0, 1, 8, 0, 0, 0,
        ],
        [
            2, 0, 0, 0, 8, 0, 3, 0, 0, 0, 6, 0, 0, 7, 0, 0, 8, 4, 0, 3, 0, 5, 0, 0, 2, 0, 9, 0, 0,
            0, 1, 0, 5, 4, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 2, 7, 0, 6, 0, 0, 0, 3, 0, 1, 0,
            0, 7, 0, 4, 0, 7, 2, 0, 0, 4, 0, 0, 6, 0, 0, 0, 4, 0, 1, 0, 0, 0, 3,
        ],
        [
            0, 0, 5, 3, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 2, 0, 0, 7, 0, 0, 1, 0, 5, 0, 0, 4, 0,
            0, 0, 0, 5, 3, 0, 0, 0, 1, 0, 0, 7, 0, 0, 0, 6, 0, 0, 3, 2, 0, 0, 0, 8, 0, 0, 6, 0, 5,
            0, 0, 0, 0, 9, 0, 0, 4, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 9, 7, 0, 0,
        ],
        [
            0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 9, 5, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    ];

    #[derive(Copy, Clone)]
    struct Game {
        board: [u8; CELLS],
        fixed: [bool; CELLS],
        cursor: usize,
        puzzle_index: usize,
        steps: u32,
        backtracks: u32,
        last_cell: usize,
        last_was_backtrack: bool,
        solved_count: u32,
        rng: u32,
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
        let rmode = init_display();

        for color in [0x515A_51F0, 0x29F0_296E, COLOR_BG] {
            clear_draw_framebuffer(rmode, color);
            swap_framebuffers();
            video::wait_vsync();
            video::wait_vsync();
        }

        controller_global_input::init(true, cfg!(feature = "wii-controllers"));
        run_sudoku_visualizer()
    }

    fn run_sudoku_visualizer() -> ! {
        let mut game = Game {
            board: [0; CELLS],
            fixed: [false; CELLS],
            cursor: 0,
            puzzle_index: 0,
            steps: 0,
            backtracks: 0,
            last_cell: CELLS,
            last_was_backtrack: false,
            solved_count: 0,
            rng: time() as u32 ^ 0x500D_7137,
        };

        load_puzzle(&mut game, 0);
        let mut last_input_frame = u32::MAX;
        let mut was_human = false;
        let mut frame_counter = 0_u32;

        loop {
            let mut human = false;
            for frame in 0..STEP_FRAMES {
                frame_counter = frame_counter.wrapping_add(1);
                #[cfg(feature = "wii-controllers")]
                let input = controller_global_input::poll_with_profile(
                    crate::half_controller_input::HalfControllerProfile::new(
                        Buttons::A,
                        Buttons::B,
                        Buttons::empty(),
                    ),
                );
                #[cfg(not(feature = "wii-controllers"))]
                let input = controller_global_input::poll();
                human |= controller_activity::active(input, &mut last_input_frame, frame_counter);
                if human {
                    #[cfg(feature = "action-trace")]
                    let previous_cursor = game.cursor;
                    #[cfg(feature = "action-trace")]
                    let previous_board = game.board;
                    manual_input(&mut game, input);
                    #[cfg(feature = "action-trace")]
                    {
                        if game.cursor != previous_cursor {
                            controller_input::record_action(
                                "sudoku",
                                "manual",
                                "cursor_move",
                                frame_counter,
                                input,
                            );
                        }
                        if sudoku_action::board_changed(&previous_board, &game.board) {
                            controller_input::record_action(
                                "sudoku",
                                "manual",
                                "cell_edit",
                                frame_counter,
                                input,
                            );
                        }
                    }
                }
                draw_game(&game, frame);
                video::wait_vsync();
            }

            if was_human && !human {
                let puzzle_index = game.puzzle_index;
                load_puzzle(&mut game, puzzle_index);
            }
            was_human = human;
            if human {
                continue;
            }

            match solve_one_step(&mut game) {
                0 => {}
                1 => {
                    game.solved_count = game.solved_count.wrapping_add(1);
                    show_solved(&game);

                    game.rng = game.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let next =
                        ((game.puzzle_index + 1 + ((game.rng >> 24) as usize % PUZZLES.len()))
                            % PUZZLES.len()) as usize;
                    load_puzzle(&mut game, next);
                }
                _ => {
                    show_unsolved(&game);

                    let next = (game.puzzle_index + 1) % PUZZLES.len();
                    load_puzzle(&mut game, next);
                }
            }
        }
    }

    fn manual_input(game: &mut Game, input: controller_input::Input) {
        let row = game.cursor / N;
        let col = game.cursor % N;
        if input.down.contains(Buttons::LEFT) {
            game.cursor = row * N + col.saturating_sub(1);
        } else if input.down.contains(Buttons::RIGHT) {
            game.cursor = row * N + (col + 1).min(N - 1);
        } else if input.down.contains(Buttons::UP) {
            game.cursor = row.saturating_sub(1) * N + col;
        } else if input.down.contains(Buttons::DOWN) {
            game.cursor = (row + 1).min(N - 1) * N + col;
        }

        if game.fixed[game.cursor] {
            return;
        }
        if input.down.contains(Buttons::A) {
            game.board[game.cursor] = (game.board[game.cursor] + 1) % 10;
        } else if input.down.contains(Buttons::B) {
            game.board[game.cursor] = 0;
        }
    }

    fn load_puzzle(game: &mut Game, index: usize) {
        game.puzzle_index = index % PUZZLES.len();
        game.board = PUZZLES[game.puzzle_index];
        game.fixed = [false; CELLS];
        game.cursor = 0;
        game.steps = 0;
        game.backtracks = 0;
        game.last_cell = CELLS;
        game.last_was_backtrack = false;

        for i in 0..CELLS {
            game.fixed[i] = game.board[i] != 0;
        }
    }

    fn solve_one_step(game: &mut Game) -> u8 {
        while game.cursor < CELLS && game.fixed[game.cursor] {
            game.cursor += 1;
        }

        if game.cursor >= CELLS {
            return 1;
        }

        let idx = game.cursor;
        let start = game.board[idx].saturating_add(1);

        for n in start..=9 {
            game.board[idx] = n;
            if is_valid_cell(&game.board, idx) {
                game.steps = game.steps.wrapping_add(1);
                game.last_cell = idx;
                game.last_was_backtrack = false;
                game.cursor += 1;
                sfx_place();
                return 0;
            }
        }

        game.board[idx] = 0;
        game.last_cell = idx;
        game.last_was_backtrack = true;
        game.backtracks = game.backtracks.wrapping_add(1);
        sfx_backtrack();

        loop {
            if game.cursor == 0 {
                return 2;
            }

            game.cursor -= 1;

            if !game.fixed[game.cursor] {
                game.last_cell = game.cursor;
                game.last_was_backtrack = true;
                return 0;
            }
        }
    }

    fn is_valid_cell(board: &[u8; CELLS], idx: usize) -> bool {
        let value = board[idx];
        if value == 0 {
            return true;
        }

        let row = idx / N;
        let col = idx % N;

        for x in 0..N {
            let other = row * N + x;
            if other != idx && board[other] == value {
                return false;
            }
        }

        for y in 0..N {
            let other = y * N + col;
            if other != idx && board[other] == value {
                return false;
            }
        }

        let box_x = (col / 3) * 3;
        let box_y = (row / 3) * 3;

        for dy in 0..3 {
            for dx in 0..3 {
                let other = (box_y + dy) * N + (box_x + dx);
                if other != idx && board[other] == value {
                    return false;
                }
            }
        }

        true
    }

    fn draw_game(game: &Game, anim_frame: usize) {
        let rmode = unsafe { DISPLAY_MODE };
        if rmode.is_null() {
            return;
        }

        unsafe {
            draw_rect(
                0,
                0,
                (*rmode).fb_width as usize,
                (*rmode).xfb_height as usize,
                COLOR_BG,
            );
        }

        draw_text(496, 48, 3, b"SUDOKU", COLOR_TEXT);
        draw_text(496, 88, 2, b"SOLVER", COLOR_TEXT_DIM);

        draw_text(496, 152, 2, b"PUZZLE", COLOR_TEXT_DIM);
        draw_number(496, 178, 3, game.puzzle_index + 1, COLOR_TEXT);

        draw_text(496, 230, 2, b"STEPS", COLOR_TEXT_DIM);
        draw_number(496, 256, 3, game.steps as usize, COLOR_TEXT);

        draw_text(496, 308, 2, b"BACK", COLOR_TEXT_DIM);
        draw_number(496, 334, 3, game.backtracks as usize, COLOR_TEXT);

        draw_text(496, 386, 2, b"SOLVED", COLOR_TEXT_DIM);
        draw_number(496, 412, 3, game.solved_count as usize, COLOR_TEXT);

        draw_board(game, anim_frame);

        swap_framebuffers();
    }

    fn draw_board(game: &Game, anim_frame: usize) {
        draw_rect(
            BOARD_X - 8,
            BOARD_Y - 8,
            BOARD_PIXELS + 16,
            BOARD_PIXELS + 16,
            COLOR_GRID_THICK,
        );
        draw_rect(BOARD_X, BOARD_Y, BOARD_PIXELS, BOARD_PIXELS, COLOR_BOARD);

        for row in 0..N {
            for col in 0..N {
                let idx = row * N + col;
                let x = BOARD_X + col * CELL;
                let y = BOARD_Y + row * CELL;
                let value = game.board[idx];

                let mut color = COLOR_CELL;
                if idx == game.last_cell {
                    color = if game.last_was_backtrack {
                        COLOR_BACKTRACK
                    } else {
                        COLOR_TRIAL
                    };

                    if anim_frame < 2 {
                        color = COLOR_TEXT;
                    }
                }

                draw_rect(x + 2, y + 2, CELL - 4, CELL - 4, color);

                if value != 0 {
                    let digit_color = if game.fixed[idx] {
                        COLOR_GIVEN
                    } else {
                        COLOR_TEXT
                    };
                    draw_digit_centered(x, y, value, digit_color);
                }
            }
        }

        for i in 0..=N {
            let thick = i % 3 == 0;
            let line_color = if thick { COLOR_GRID_THICK } else { COLOR_GRID };
            let line_w = if thick { 4 } else { 2 };

            draw_rect(
                BOARD_X + i * CELL,
                BOARD_Y,
                line_w,
                BOARD_PIXELS + 2,
                line_color,
            );
            draw_rect(
                BOARD_X,
                BOARD_Y + i * CELL,
                BOARD_PIXELS + 2,
                line_w,
                line_color,
            );
        }
    }

    fn draw_digit_centered(cell_x: usize, cell_y: usize, value: u8, color: u32) {
        let scale = 4;
        let x = cell_x + CELL / 2 - 10;
        let y = cell_y + CELL / 2 - 14;
        draw_char(x, y, scale, b'0' + value, color);
    }

    fn show_solved(game: &Game) {
        unsafe {
            let rmode = DISPLAY_MODE;
            if rmode.is_null() {
                return;
            }

            draw_rect(
                0,
                0,
                (*rmode).fb_width as usize,
                (*rmode).xfb_height as usize,
                COLOR_SOLVED_BG,
            );

            draw_text(110, 120, 5, b"SOLVED", COLOR_TEXT);

            draw_text(148, 240, 3, b"STEPS", COLOR_TEXT_DIM);
            draw_number(358, 240, 3, game.steps as usize, COLOR_TEXT);

            draw_text(148, 282, 3, b"BACK", COLOR_TEXT_DIM);
            draw_number(358, 282, 3, game.backtracks as usize, COLOR_TEXT);

            swap_framebuffers();

            for _ in 0..SOLVED_FRAMES {
                video::wait_vsync();
            }
        }
    }

    fn show_unsolved(game: &Game) {
        unsafe {
            let rmode = DISPLAY_MODE;
            if rmode.is_null() {
                return;
            }

            draw_rect(
                0,
                0,
                (*rmode).fb_width as usize,
                (*rmode).xfb_height as usize,
                COLOR_PANIC,
            );

            draw_text(96, 128, 5, b"NO", COLOR_TEXT);
            draw_text(96, 172, 5, b"SOLVE", COLOR_TEXT);

            draw_text(148, 280, 3, b"PUZZLE", COLOR_TEXT_DIM);
            draw_number(378, 280, 3, game.puzzle_index + 1, COLOR_TEXT);

            swap_framebuffers();

            for _ in 0..SOLVED_FRAMES {
                video::wait_vsync();
            }
        }
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

    fn draw_char(x: usize, y: usize, scale: usize, ch: u8, color: u32) {
        let glyph = glyph_rows(ch);

        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    draw_rect(x + col * scale, y + row * scale, scale, scale, color);
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
            b'K' => [
                0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
            ],
            b'L' => [
                0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
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
            b'Z' => [
                0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
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
            let y0 = y.min(xfb_height.saturating_sub(1));
            let x1 = x.saturating_add(w).min(fb_width) & !1;
            let y1 = y.saturating_add(h).min(xfb_height);

            if x1 <= x0 || y1 <= y0 {
                return;
            }

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

            core::ptr::swap(
                core::ptr::addr_of_mut!(DRAW_FRAMEBUFFER),
                core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER),
            );

            video::wait_vsync();
            video::set_next_framebuffer(DISPLAY_FRAMEBUFFER);
            video::flush();
        }
    }

    fn sfx_place() {}
    fn sfx_backtrack() {}

    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
        unsafe {
            clear_draw_framebuffer(DISPLAY_MODE, COLOR_PANIC);
            swap_framebuffers();
        }

        loop {
            core::hint::spin_loop();
        }
    }
}
