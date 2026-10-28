//! Five-by-seven ASCII alphabet and numeral font. Lowercase is not upcased.
//! The same pixels are used for rendering and deciding whether a mirror is visible.

pub const fn rows(ch: u8) -> [u8; 7] {
    match ch {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'a' => [0, 0, 14, 1, 15, 17, 15],
        b'b' => [16, 16, 22, 25, 17, 17, 30],
        b'c' => [0, 0, 14, 17, 16, 17, 14],
        b'd' => [1, 1, 13, 19, 17, 17, 15],
        b'e' => [0, 0, 14, 17, 31, 16, 14],
        b'f' => [6, 9, 8, 28, 8, 8, 8],
        b'g' => [0, 15, 17, 17, 15, 1, 14],
        b'h' => [16, 16, 22, 25, 17, 17, 17],
        b'i' => [4, 0, 12, 4, 4, 4, 14],
        b'j' => [2, 0, 6, 2, 2, 18, 12],
        b'k' => [16, 16, 18, 20, 24, 20, 18],
        b'l' => [12, 4, 4, 4, 4, 4, 14],
        b'm' => [0, 0, 26, 21, 21, 21, 21],
        b'n' => [0, 0, 22, 25, 17, 17, 17],
        b'o' => [0, 0, 14, 17, 17, 17, 14],
        b'p' => [0, 0, 30, 17, 30, 16, 16],
        b'q' => [0, 0, 15, 17, 15, 1, 1],
        b'r' => [0, 0, 22, 25, 16, 16, 16],
        b's' => [0, 0, 15, 16, 14, 1, 30],
        b't' => [8, 8, 28, 8, 8, 9, 6],
        b'u' => [0, 0, 17, 17, 17, 19, 13],
        b'v' => [0, 0, 17, 17, 17, 10, 4],
        b'w' => [0, 0, 17, 17, 21, 21, 10],
        b'x' => [0, 0, 17, 10, 4, 10, 17],
        b'y' => [0, 0, 17, 17, 15, 1, 14],
        b'z' => [0, 0, 31, 2, 4, 8, 31],
        // The number word distinguishes zero from O; the glyph stays symmetric.
        b'0' => [14, 17, 17, 17, 17, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b' ' => [0; 7],
        b',' => [0, 0, 0, 0, 0, 4, 8],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b':' => [0, 12, 12, 0, 12, 12, 0],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'/' => [1, 1, 2, 4, 8, 16, 16],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b'%' => [25, 25, 2, 4, 8, 19, 19],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'=' => [0, 31, 0, 31, 0, 0, 0],
        b'<' => [2, 4, 8, 16, 8, 4, 2],
        b'>' => [8, 4, 2, 1, 2, 4, 8],
        b'[' => [14, 8, 8, 8, 8, 8, 14],
        b']' => [14, 2, 2, 2, 2, 2, 14],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}

pub const fn flip_row(row: u8) -> u8 {
    ((row & 1) << 4) | ((row & 2) << 2) | (row & 4) | ((row & 8) >> 2) | ((row & 16) >> 4)
}

pub fn visibly_mirrored(ch: u8) -> bool {
    rows(ch).iter().any(|&row| row != flip_row(row))
}

/// Voice asset index and name. Letter case shares one clip.
const NUMBER_WORDS: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

pub const fn number_word(ch: u8) -> Option<&'static str> {
    if ch >= b'0' && ch <= b'9' {
        Some(NUMBER_WORDS[(ch - b'0') as usize])
    } else {
        None
    }
}

pub const fn voice_asset(ch: u8) -> Option<(usize, &'static str)> {
    match ch {
        b'A' | b'a' => Some((0, "a")),
        b'B' | b'b' => Some((1, "b")),
        b'C' | b'c' => Some((2, "c")),
        b'D' | b'd' => Some((3, "d")),
        b'E' | b'e' => Some((4, "e")),
        b'F' | b'f' => Some((5, "f")),
        b'G' | b'g' => Some((6, "g")),
        b'H' | b'h' => Some((7, "h")),
        b'I' | b'i' => Some((8, "i")),
        b'J' | b'j' => Some((9, "j")),
        b'K' | b'k' => Some((10, "k")),
        b'L' | b'l' => Some((11, "l")),
        b'M' | b'm' => Some((12, "m")),
        b'N' | b'n' => Some((13, "n")),
        b'O' | b'o' => Some((14, "o")),
        b'P' | b'p' => Some((15, "p")),
        b'Q' | b'q' => Some((16, "q")),
        b'R' | b'r' => Some((17, "r")),
        b'S' | b's' => Some((18, "s")),
        b'T' | b't' => Some((19, "t")),
        b'U' | b'u' => Some((20, "u")),
        b'V' | b'v' => Some((21, "v")),
        b'W' | b'w' => Some((22, "w")),
        b'X' | b'x' => Some((23, "x")),
        b'Y' | b'y' => Some((24, "y")),
        b'Z' | b'z' => Some((25, "z")),
        b'0'..=b'9' => {
            let digit = (ch - b'0') as usize;
            Some((26 + digit, NUMBER_WORDS[digit]))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_symbols_have_printable_glyphs() {
        for ch in b'A'..=b'Z' {
            assert!(rows(ch).iter().any(|&v| v != 0));
        }
        for ch in b'a'..=b'z' {
            assert!(rows(ch).iter().any(|&v| v != 0));
        }
        for ch in b'0'..=b'9' {
            assert!(rows(ch).iter().any(|&v| v != 0));
        }
        assert_ne!(rows(b'A'), rows(b'a'));
        assert_ne!(rows(b'B'), rows(b'b'));
        assert_eq!(rows(b',')[..5], [0; 5]);
        assert_ne!(rows(b','), rows(b'?'));
    }
    #[test]
    fn mirrored_symmetric_shapes_are_not_incorrect_questions() {
        assert!(!visibly_mirrored(b'O'));
        assert!(!visibly_mirrored(b'8'));
        assert!(!visibly_mirrored(b'0'));
        assert!(visibly_mirrored(b'R'));
        assert_eq!(flip_row(0b10010), 0b01001);
        for ch in b'A'..=b'Z' {
            for &row in &rows(ch) {
                assert_eq!(flip_row(flip_row(row)), row);
            }
        }
    }

    #[test]
    fn number_words_match_their_voice_asset_names() {
        for digit in b'0'..=b'9' {
            let word = number_word(digit).expect("every digit has a number word");
            assert_eq!(voice_asset(digit).map(|(_, name)| name), Some(word));
        }
        assert_eq!(number_word(b'A'), None);
    }
}
