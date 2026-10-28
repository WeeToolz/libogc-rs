//! Shared screen renderer. Wii output and host layout checks use the same drawing code.
use crate::card_storage_status::CardStatus;
use crate::engine::{Game, ResultKind};
use crate::font;
use crate::session::{Screen, Session};
use crate::settings::{Mode, Settings};
pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;
pub const BG: u32 = 0x1880_1880;
pub const BAD: u32 = 0x755E_75E9;
const PANEL: u32 = 0x3680_3680;
const EDGE: u32 = 0x8C80_8C80;
const WHITE: u32 = 0xEB80_EB80;
pub const DIM: u32 = 0xB080_B080;
const ACCENT: u32 = 0xB66B_B62C;
const SAVE_STATUS_Y: usize = 441;
const SAVE_STATUS_SCALE: usize = 2;
pub trait Canvas {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32);
}
pub struct Renderer<'a, C: Canvas> {
    canvas: &'a mut C,
}
impl<'a, C: Canvas> Renderer<'a, C> {
    pub fn new(canvas: &'a mut C) -> Self {
        Self { canvas }
    }
    pub fn draw_screen(&mut self, s: &Session, remote: bool, connected: bool) {
        match s.screen {
            Screen::Title => self.draw_title(s),
            Screen::Help => self.draw_help(),
            Screen::Settings => self.draw_settings(s.data.settings, s.selection),
            Screen::Playing => self.draw_game(s, None, remote),
            Screen::Feedback => self.draw_game(s, Some(s.feedback), remote),
            Screen::Pause => self.draw_pause(s),
            Screen::Over => self.draw_over(s),
        }
        if matches!(s.screen, Screen::Playing | Screen::Feedback) {
            self.notice(
                395,
                2,
                if remote && s.data.settings.sideways {
                    b"2: NORMAL   1: BACKWARDS"
                } else {
                    b"A: NORMAL   B: BACKWARDS"
                },
                WHITE,
            );
            self.notice(
                416,
                2,
                if remote {
                    b"+: PAUSE   -: REPEAT VOICE"
                } else {
                    b"START: PAUSE   Z: REPEAT VOICE"
                },
                DIM,
            );
        }
        if !connected {
            self.panel(94, 374, 452, 47);
            self.notice(387, 2, b"CONNECT A CONTROLLER", WHITE);
        }
    }
    fn draw_title(&mut self, s: &Session) {
        self.notice(50, 5, b"LETTER FLIP", WHITE);
        self.notice(113, 2, b"LOOK, LISTEN, AND LEARN", DIM);
        self.notice(153, 3, s.data.settings.category.label(), WHITE);
        self.notice(187, 2, s.mode_label(), ACCENT);
        let (label, value) = if s.data.settings.mode == Mode::Practice {
            (b"CARDS" as &[u8], s.data.settings.cards)
        } else {
            (b"CHANCES" as &[u8], s.data.settings.chances)
        };
        self.text(246, 211, 2, label, DIM);
        self.number(356, 211, 2, u64::from(value), WHITE);
        self.panel(138, 240, 364, 130);
        for (i, label) in [b"PLAY" as &[u8], b"HOW TO PLAY", b"SETTINGS"]
            .iter()
            .enumerate()
        {
            self.menu_row(254 + i * 37, label, s.selection == i);
        }
        self.notice(380, 2, b"BEST FOR THESE GAME SETTINGS", DIM);
        self.center_number(399, 3, u64::from(s.best()), ACCENT);
        if s.data.legacy_best > 0 {
            self.text(116, 422, 2, b"PREVIOUS GAME RECORD", DIM);
            self.number(400, 422, 2, s.data.legacy_best, DIM);
        }
    }
    fn menu_row(&mut self, y: usize, label: &[u8], selected: bool) {
        if selected {
            self.canvas.rect(158, y - 5, 324, 31, ACCENT);
        }
        self.notice(y, 3, label, if selected { BG } else { WHITE });
    }
    fn draw_help(&mut self) {
        self.notice(44, 4, b"HOW TO PLAY", WHITE);
        self.panel(54, 100, 532, 289);
        for (i, line) in [
            b"CHOOSE NORMAL OR BACKWARDS." as &[u8],
            b"PRACTICE HAS NO CHANCE LIMIT.",
            b"CHALLENGE USES YOUR CHANCE LIMIT.",
            b"AFTER A MISTAKE, SEE THE ANSWER.",
            b"O, 0, AND 8 LOOK THE SAME IN A MIRROR.",
            b"THESE SHAPES ARE ALWAYS NORMAL.",
            b"SET GAMES, SOUND, AND CONTROLS",
            b"TOGETHER IN SETTINGS.",
        ]
        .iter()
        .enumerate()
        {
            self.notice(119 + i * 31, 2, line, if i == 1 { ACCENT } else { WHITE });
        }
        self.notice(404, 2, b"SELECT OR BACK: RETURN", DIM);
    }
    fn toggle(&mut self, value: bool) -> &'static [u8] {
        if value { b"ON" } else { b"OFF" }
    }
    fn setting_value(&mut self, row: usize, s: Settings) {
        let value: Option<&[u8]> = match row {
            0 => Some(s.category.label()),
            1 => Some(if s.mode == Mode::Practice {
                b"PRACTICE"
            } else {
                b"CHALLENGE"
            }),
            4 => Some(if s.asymmetric_only {
                b"MIRROR SHAPES"
            } else {
                b"ALL SHAPES"
            }),
            6 => Some(self.toggle(s.voice)),
            8 => Some(self.toggle(s.effects)),
            10 => Some(self.toggle(s.flashes)),
            11 => Some(if s.sideways {
                b"SIDEWAYS 1/2"
            } else {
                b"UPRIGHT A/B"
            }),
            _ => None,
        };
        let y = 133 + (row - self.settings_page(row).0) * 35;
        if let Some(value) = value {
            self.text(362, y, 2, value, WHITE);
        } else if row < 12 {
            let value = match row {
                2 => u64::from(s.chances),
                3 => u64::from(s.cards),
                5 => u64::from(s.feedback_ms),
                7 => u64::from(s.voice_volume),
                9 => u64::from(s.effects_volume),
                _ => 0,
            };
            self.number(362, y, 2, value, WHITE);
            if row == 7 || row == 9 {
                self.text(409, y, 2, b"%", WHITE);
            } else if row == 5 {
                self.text(422, y, 2, b"MS", WHITE);
            }
        }
    }
    fn settings_page(&mut self, row: usize) -> (usize, usize, &'static [u8]) {
        if row < 6 {
            (0, 6, b"GAME")
        } else if row < 11 {
            (6, 11, b"SOUND AND DISPLAY")
        } else {
            (11, 15, b"CONTROLS AND SAVE")
        }
    }
    fn draw_settings(&mut self, s: Settings, selected: usize) {
        const LABELS: [&[u8]; 15] = [
            b"SYMBOLS",
            b"MODE",
            b"CHANCE LIMIT",
            b"PRACTICE CARDS",
            b"SYMBOL POOL",
            b"REVIEW TIME",
            b"VOICE",
            b"VOICE VOLUME",
            b"EFFECTS",
            b"EFFECT VOLUME",
            b"SCREEN FLASH",
            b"WII REMOTE",
            b"PREVIEW SOUND",
            b"RETRY SAVE",
            b"BACK",
        ];
        self.notice(40, 4, b"SETTINGS", WHITE);
        let (start, end, name) = self.settings_page(selected);
        self.notice(85, 2, name, ACCENT);
        self.notice(
            101,
            2,
            if start == 0 {
                b"1/3"
            } else if start == 6 {
                b"2/3"
            } else {
                b"3/3"
            },
            DIM,
        );
        self.panel(52, 118, 536, 229);
        for (row, label) in LABELS.iter().enumerate().take(end).skip(start) {
            let y = 133 + (row - start) * 35;
            if row == selected {
                self.canvas.rect(62, y - 5, 516, 30, EDGE);
            }
            self.text(76, y, 2, label, if row == selected { BG } else { DIM });
            self.setting_value(row, s);
        }
        let note: &[u8] = match selected {
            2 => b"CHALLENGE ONLY. PRACTICE DOES NOT LOSE CHANCES.",
            3 => b"PRACTICE ENDS AFTER THIS MANY CARDS.",
            4 => b"MIRROR SHAPES: ONLY SHAPES WITH A VISIBLE MIRROR.",
            5 => b"TIME TO REVIEW A MISTAKE BEFORE THE NEXT CARD.",
            10 => b"FLASHES ARE OFF BY DEFAULT.",
            11 => b"UPRIGHT: A/B. SIDEWAYS: 2/1. +: PAUSE.",
            13 => b"INSERT A CARD, THEN SELECT TO RETRY.",
            6..=9 => b"SOUND CHANGES RIGHT AWAY. USE PREVIEW TO LISTEN.",
            _ => b"GAME SETTINGS APPLY TO THE NEXT ROUND.",
        };
        self.notice(361, 2, note, DIM);
        self.notice(394, 2, b"UP/DOWN: CHOOSE", WHITE);
        self.notice(
            416,
            2,
            b"LEFT/RIGHT: CHANGE   SELECT: CHANGE   BACK: RETURN",
            DIM,
        );
    }
    fn draw_game(&mut self, s: &Session, feedback: Option<ResultKind>, remote: bool) {
        let g = &s.game;
        self.text(42, 32, 2, b"CORRECT", DIM);
        self.number(
            142,
            32,
            if g.score >= 10000 { 1 } else { 2 },
            u64::from(g.score),
            WHITE,
        );
        self.text(232, 32, 2, b"BEST", DIM);
        self.number(
            300,
            32,
            if s.best() >= 10000 { 1 } else { 2 },
            u64::from(s.best()),
            WHITE,
        );
        if g.settings.mode == Mode::Practice {
            self.text(410, 32, 2, b"CARD", DIM);
            self.number(
                475,
                32,
                2,
                u64::from(
                    (g.answered + u32::from(feedback.is_none())).min(u32::from(g.settings.cards)),
                ),
                WHITE,
            );
            self.text(510, 32, 2, b"/", DIM);
            self.number(530, 32, 2, u64::from(g.settings.cards), WHITE);
        } else {
            self.text(408, 32, 2, b"CHANCES", DIM);
            self.number(520, 32, 2, u64::from(g.chances), WHITE);
        }
        self.notice(61, 2, g.settings.category.label(), DIM);
        self.panel(108, 85, 424, 296);
        if feedback.is_some() && feedback != Some(ResultKind::Correct) {
            self.notice(104, 2, b"LET US LOOK TOGETHER", WHITE);
            self.canvas.rect(
                if g.current.is_mirrored() { 369 } else { 169 },
                144,
                102,
                125,
                ACCENT,
            );
            self.canvas.rect(
                if g.current.is_mirrored() { 372 } else { 172 },
                147,
                96,
                119,
                PANEL,
            );
            self.glyph(180, 151, 16, g.current.symbol, false, WHITE);
            self.glyph(380, 151, 16, g.current.symbol, true, WHITE);
            self.text(202, 130, 2, b"NORMAL", DIM);
            self.text(393, 130, 2, b"BACKWARDS", DIM);
            if let Some(word) = font::number_word(g.current.symbol) {
                self.notice(280, 2, word.as_bytes(), DIM);
            }
            self.notice(
                315,
                2,
                if g.current.is_mirrored() {
                    b"THIS CARD IS BACKWARDS"
                } else {
                    b"THIS CARD IS NORMAL"
                },
                ACCENT,
            );
            self.notice(350, 2, b"YOU CAN LEARN WITH EACH TRY", DIM);
        } else {
            let symbol = g.current.symbol;
            if let Some(word) = font::number_word(symbol) {
                self.glyph(
                    (WIDTH - 5 * 24) / 2,
                    105,
                    24,
                    symbol,
                    g.current.is_mirrored(),
                    WHITE,
                );
                self.number_word(word, 284, WHITE);
            } else {
                self.glyph(
                    (WIDTH - 5 * 26) / 2,
                    112,
                    26,
                    symbol,
                    g.current.is_mirrored(),
                    WHITE,
                );
            }
            if feedback == Some(ResultKind::Correct) {
                self.notice(
                    345,
                    2,
                    [
                        b"GOOD JOB! +1" as &[u8],
                        b"WELL DONE! +1",
                        b"KEEP GOING! +1",
                    ][(g.answered.saturating_sub(1) as usize) % 3],
                    ACCENT,
                );
            } else if feedback.is_none() {
                let (normal_key, backwards_key) = if remote && g.settings.sideways {
                    (b"2" as &[u8], b"1" as &[u8])
                } else {
                    (b"A" as &[u8], b"B" as &[u8])
                };
                self.answer_option(137, 330, normal_key, b"NORMAL");
                self.answer_option(325, 330, backwards_key, b"BACKWARDS");
            }
        }
    }
    fn answer_option(&mut self, x: usize, y: usize, key: &[u8], label: &[u8]) {
        self.panel(x, y, 178, 39);
        self.canvas.rect(x + 5, y + 5, 30, 29, ACCENT);
        self.text(x + 12, y + 12, 2, key, BG);
        self.text(x + 45, y + 12, 2, label, WHITE);
    }
    fn draw_pause(&mut self, s: &Session) {
        self.notice(54, 4, b"TAKE A BREAK", WHITE);
        self.notice(
            110,
            2,
            if s.owner.is_none() {
                b"SELECT WITH A CONTROLLER TO RESUME"
            } else {
                b"YOUR ROUND IS WAITING"
            },
            DIM,
        );
        self.panel(128, 160, 384, 179);
        for (i, label) in [
            b"RESUME" as &[u8],
            b"RESTART",
            b"CHANGE CONTROLLER",
            b"MENU",
        ]
        .iter()
        .enumerate()
        {
            self.menu_row(176 + i * 39, label, s.selection == i);
        }
        self.notice(395, 2, b"UP/DOWN: CHOOSE   SELECT: GO", DIM);
    }
    fn draw_over(&mut self, s: &Session) {
        let g: &Game = &s.game;
        self.notice(54, 4, b"ROUND COMPLETE", WHITE);
        self.notice(110, 2, b"THANK YOU FOR TRYING!", ACCENT);
        self.panel(124, 159, 392, 190);
        self.notice(180, 2, b"CORRECT ANSWERS", DIM);
        self.center_number(211, 4, u64::from(g.score), WHITE);
        self.notice(263, 2, b"BEST FOR THESE SETTINGS", DIM);
        self.center_number(294, 3, u64::from(s.best()), ACCENT);
        self.notice(387, 2, b"SELECT: AGAIN   BACK: MENU", WHITE);
    }
    pub fn draw_save_status(&mut self, status: CardStatus) {
        self.notice(
            SAVE_STATUS_Y,
            SAVE_STATUS_SCALE,
            save_status_label(status),
            DIM,
        );
    }
    fn panel(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.canvas.rect(x, y, w, h, EDGE);
        self.canvas.rect(x + 3, y + 3, w - 6, h - 6, PANEL);
    }
    pub fn notice(&mut self, y: usize, scale: usize, s: &[u8], color: u32) {
        self.text(
            WIDTH.saturating_sub(s.len() * 6 * scale) / 2,
            y,
            scale,
            s,
            color,
        );
    }
    fn center_number(&mut self, y: usize, scale: usize, value: u64, color: u32) {
        let mut digits = [0_u8; 20];
        let n = self.format_number(value, &mut digits);
        self.notice(y, scale, &digits[..n], color);
    }
    fn format_number(&mut self, mut value: u64, digits: &mut [u8; 20]) -> usize {
        let mut length = 0;
        loop {
            digits[length] = b'0' + (value % 10) as u8;
            value /= 10;
            length += 1;
            if value == 0 {
                break;
            }
        }
        digits[..length].reverse();
        length
    }
    fn number(&mut self, x: usize, y: usize, scale: usize, value: u64, color: u32) {
        let mut digits = [0; 20];
        let len = self.format_number(value, &mut digits);
        self.text(x, y, scale, &digits[..len], color);
    }
    fn text(&mut self, mut x: usize, y: usize, scale: usize, string: &[u8], color: u32) {
        for &c in string {
            self.glyph(x, y, scale, c, false, color);
            x += 6 * scale;
        }
    }
    fn number_word(&mut self, word: &str, y: usize, color: u32) {
        let letters = word.as_bytes();
        let scale = 5;
        let x = WIDTH.saturating_sub(letters.len() * 6 * scale) / 2;
        for (index, &letter) in letters.iter().enumerate() {
            self.glyph(x + index * 6 * scale, y, scale, letter, false, color);
        }
    }
    fn glyph(&mut self, x: usize, y: usize, scale: usize, symbol: u8, backwards: bool, color: u32) {
        for (row, bits) in font::rows(symbol).iter().copied().enumerate() {
            let pixels = if backwards {
                font::flip_row(bits)
            } else {
                bits
            };
            for col in 0..5 {
                if pixels & (1 << (4 - col)) != 0 {
                    self.canvas
                        .rect(x + col * scale, y + row * scale, scale, scale, color);
                }
            }
        }
    }
}

fn save_status_label(status: CardStatus) -> &'static [u8] {
    match status {
        CardStatus::Ready => b"MEMORY CARD READY",
        CardStatus::NoCard => b"NO CARD - SESSION ONLY",
        CardStatus::Error => b"SAVE FAILED - SELECT RETRY SAVE",
        CardStatus::Pending => b"SAVE PENDING - SAVES AFTER ROUND",
        CardStatus::Saving => b"SAVING - KEEP CARD INSERTED",
        CardStatus::Saved => b"SETTINGS AND SCORES VERIFIED",
        CardStatus::Recovered => b"DAMAGED FILE KEPT - SAVING BLOCKED",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::Data;
    struct Raster {
        pixels: Vec<u32>,
    }
    impl Raster {
        fn new() -> Self {
            Self {
                pixels: vec![BG; WIDTH * HEIGHT],
            }
        }
        fn export(&self, name: &str) {
            let Ok(directory) = std::env::var("LETTER_FLIP_PREVIEW_DIR") else {
                return;
            };
            std::fs::create_dir_all(&directory).unwrap();
            let mut bytes = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
            for &color in &self.pixels {
                let y = ((color >> 24) & 255) as i32 - 16;
                let u = ((color >> 16) & 255) as i32 - 128;
                let v = (color & 255) as i32 - 128;
                for value in [
                    298 * y + 409 * v,
                    298 * y - 100 * u - 208 * v,
                    298 * y + 516 * u,
                ] {
                    bytes.push(((value + 128) >> 8).clamp(0, 255) as u8);
                }
            }
            std::fs::write(
                std::path::Path::new(&directory).join(format!("{name}.ppm")),
                bytes,
            )
            .unwrap();
        }
    }
    impl Canvas for Raster {
        fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
            assert!(
                x.saturating_add(w) <= WIDTH && y.saturating_add(h) <= HEIGHT,
                "rectangle outside display: {x},{y} {w}x{h}"
            );
            for row in y..y + h {
                for column in x..x + w {
                    self.pixels[row * WIDTH + column] = color;
                }
            }
        }
    }
    #[test]
    fn screens_and_all_settings_draw_within_display_bounds() {
        let mut s = Session::new(Data::default(), 0);
        for screen in [
            Screen::Title,
            Screen::Help,
            Screen::Settings,
            Screen::Playing,
            Screen::Feedback,
            Screen::Pause,
            Screen::Over,
        ] {
            s.screen = screen;
            let count = if screen == Screen::Settings {
                15
            } else if screen == Screen::Pause {
                4
            } else {
                1
            };
            for selected in 0..count {
                s.selection = selected;
                s.feedback = ResultKind::Wrong;
                s.game.answered = 1;
                let mut canvas = Raster::new();
                let mut r = Renderer::new(&mut canvas);
                r.draw_screen(&s, false, true);
                r.draw_save_status(CardStatus::Saved);
                assert!(canvas.pixels.iter().any(|&p| p != BG));
                if selected == 0 || screen == Screen::Settings && [6, 11].contains(&selected) {
                    canvas.export(&format!("{screen:?}-{selected}"));
                }
            }
        }
    }
    #[test]
    fn all_save_messages_fit_the_screen_and_flash_color_is_distinct() {
        for status in [
            CardStatus::Ready,
            CardStatus::NoCard,
            CardStatus::Error,
            CardStatus::Pending,
            CardStatus::Saving,
            CardStatus::Saved,
            CardStatus::Recovered,
        ] {
            let mut canvas = Raster::new();
            Renderer::new(&mut canvas).draw_save_status(status);
            assert!(
                canvas.pixels[SAVE_STATUS_Y * WIDTH..(SAVE_STATUS_Y + 15) * WIDTH]
                    .iter()
                    .any(|&p| p != BG)
            );
            assert!(save_status_label(status).len() * 6 * SAVE_STATUS_SCALE <= WIDTH);
        }
        assert_ne!(BAD, BG);
    }
    #[test]
    fn save_status_uses_large_glyphs_that_are_easy_to_read() {
        let mut canvas = Raster::new();
        Renderer::new(&mut canvas).draw_save_status(CardStatus::Ready);
        let has_two_by_two_glyph_pixel = (SAVE_STATUS_Y..SAVE_STATUS_Y + 7).any(|y| {
            (0..WIDTH - 1).any(|x| {
                canvas.pixels[y * WIDTH + x] == DIM
                    && canvas.pixels[y * WIDTH + x + 1] == DIM
                    && canvas.pixels[(y + 1) * WIDTH + x] == DIM
                    && canvas.pixels[(y + 1) * WIDTH + x + 1] == DIM
            })
        });
        assert!(has_two_by_two_glyph_pixel);
    }
    #[test]
    fn number_cards_and_percent_glyph_draw_real_pixels() {
        let mut s = Session::new(Data::default(), 0);
        s.screen = Screen::Playing;
        for symbol in b'0'..=b'9' {
            s.game.current.symbol = symbol;
            let mut canvas = Raster::new();
            Renderer::new(&mut canvas).draw_screen(&s, true, true);
            assert!(
                canvas.pixels[105 * WIDTH..273 * WIDTH]
                    .iter()
                    .any(|&p| p == WHITE)
            );
        }
        assert_eq!(font::rows(b'%'), [25, 25, 2, 4, 8, 19, 19]);
        assert_ne!(font::rows(b'%'), font::rows(b'?'));
    }
}
