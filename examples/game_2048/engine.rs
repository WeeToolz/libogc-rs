//! Deterministic 2048 rules shared by the player game and title demonstration.

pub const SIDE: usize = 4;
pub const CELLS: usize = SIDE * SIDE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

impl Direction {
    pub const ALL: [Self; 4] = [Self::Up, Self::Right, Self::Down, Self::Left];

    fn index(self, line: usize, pos: usize) -> usize {
        match self {
            Self::Up => pos * SIDE + line,
            Self::Right => line * SIDE + SIDE - 1 - pos,
            Self::Down => (SIDE - 1 - pos) * SIDE + line,
            Self::Left => line * SIDE + pos,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectionRepeat {
    direction: Option<Direction>,
    frames: u16,
}

impl DirectionRepeat {
    pub fn sample(
        &mut self,
        held: Option<Direction>,
        pressed: Option<Direction>,
    ) -> Option<Direction> {
        let Some(direction) = held else {
            self.direction = None;
            self.frames = 0;
            return None;
        };
        if pressed == Some(direction) || self.direction != Some(direction) {
            self.direction = Some(direction);
            self.frames = 0;
            return Some(direction);
        }
        self.frames = self.frames.saturating_add(1);
        if self.frames >= 14 {
            self.frames = 9;
            Some(direction)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TileTravel {
    pub from: u8,
    pub to: u8,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoveResult {
    pub board: [u32; CELLS],
    pub score_delta: u64,
    pub travel: [TileTravel; CELLS],
    pub travel_len: u8,
    pub merged_at: [u8; CELLS],
    pub merge_len: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Game {
    pub board: [u32; CELLS],
    pub score: u64,
    pub best_score: u64,
    pub moves: u32,
    pub rng: u32,
    pub last_spawn: Option<usize>,
    pub won: bool,
}

impl Game {
    pub fn new(seed: u32) -> Self {
        let mut game = Self {
            board: [0; CELLS],
            score: 0,
            best_score: 0,
            moves: 0,
            rng: if seed == 0 { 0x2048_7137 } else { seed },
            last_spawn: None,
            won: false,
        };
        game.reset();
        game
    }

    pub fn reset(&mut self) {
        self.board = [0; CELLS];
        self.score = 0;
        self.moves = 0;
        self.last_spawn = None;
        self.won = false;
        self.spawn();
        self.spawn();
    }

    pub fn apply(&mut self, direction: Direction) -> Option<MoveResult> {
        let mut result = calculate_move(&self.board, direction)?;
        self.board = result.board;
        self.score = self.score.saturating_add(result.score_delta);
        self.best_score = self.best_score.max(self.score);
        self.moves = self.moves.saturating_add(1);
        self.won |= self.board.contains(&2048);
        self.spawn();
        result.board = self.board;
        Some(result)
    }

    pub fn spawn(&mut self) -> Option<usize> {
        let empty_count = self.board.iter().filter(|&&value| value == 0).count();
        if empty_count == 0 {
            self.last_spawn = None;
            return None;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let target = self.rng as usize % empty_count;
        let value = if self.rng.is_multiple_of(10) { 4 } else { 2 };
        let Some(index) = self
            .board
            .iter()
            .enumerate()
            .filter(|(_, tile)| **tile == 0)
            .nth(target)
            .map(|(index, _)| index)
        else {
            self.last_spawn = None;
            return None;
        };
        self.board[index] = value;
        self.last_spawn = Some(index);
        Some(index)
    }

    pub fn can_move(&self) -> bool {
        has_legal_move(&self.board)
    }
}

pub fn calculate_move(board: &[u32; CELLS], direction: Direction) -> Option<MoveResult> {
    let mut result = MoveResult {
        board: [0; CELLS],
        score_delta: 0,
        travel: [TileTravel::default(); CELLS],
        travel_len: 0,
        merged_at: [0; CELLS],
        merge_len: 0,
    };

    for line in 0..SIDE {
        let mut values = [0_u32; SIDE];
        let mut sources = [0_u8; SIDE];
        let mut count = 0;
        for pos in 0..SIDE {
            let from = direction.index(line, pos);
            let value = board[from];
            if value != 0 {
                values[count] = value;
                sources[count] = from as u8;
                count += 1;
            }
        }

        let mut input = 0;
        let mut output = 0;
        while input < count {
            let mut value = values[input];
            let mut consumed = 1;
            if input + 1 < count && values[input] == values[input + 1] {
                if let Some(merged) = value.checked_mul(2) {
                    value = merged;
                    consumed = 2;
                    result.score_delta = result.score_delta.saturating_add(merged as u64);
                }
            }
            let to = direction.index(line, output);
            result.board[to] = value;
            for source in input..input + consumed {
                let from = sources[source];
                let travel_index = result.travel_len as usize;
                result.travel[travel_index] = TileTravel {
                    from,
                    to: to as u8,
                    value: values[source],
                };
                result.travel_len += 1;
            }
            if consumed == 2 {
                let merge_index = result.merge_len as usize;
                result.merged_at[merge_index] = to as u8;
                result.merge_len += 1;
            }
            input += consumed;
            output += 1;
        }
    }

    (result.board != *board).then_some(result)
}

pub fn has_legal_move(board: &[u32; CELLS]) -> bool {
    if board.contains(&0) {
        return true;
    }
    Direction::ALL
        .into_iter()
        .any(|direction| calculate_move(board, direction).is_some())
}

pub fn empty_count(board: &[u32; CELLS]) -> usize {
    board.iter().filter(|&&value| value == 0).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_packs_tiles_and_reports_travel() {
        let mut board = [0; CELLS];
        board[1] = 2;
        board[3] = 4;
        let movement = calculate_move(&board, Direction::Left).unwrap();
        assert_eq!(&movement.board[..4], &[2, 4, 0, 0]);
        assert_eq!(movement.travel_len, 2);
        assert_eq!(
            movement.travel[0],
            TileTravel {
                from: 1,
                to: 0,
                value: 2
            }
        );
        assert_eq!(
            movement.travel[1],
            TileTravel {
                from: 3,
                to: 1,
                value: 4
            }
        );
    }

    #[test]
    fn merges_each_tile_once_and_scores_resulting_values() {
        let mut board = [0; CELLS];
        board[..4].copy_from_slice(&[2, 2, 2, 2]);
        let movement = calculate_move(&board, Direction::Left).unwrap();
        assert_eq!(&movement.board[..4], &[4, 4, 0, 0]);
        assert_eq!(movement.score_delta, 8);
        assert_eq!(movement.merge_len, 2);
    }

    #[test]
    fn merge_does_not_chain_in_one_move() {
        let mut board = [0; CELLS];
        board[..3].copy_from_slice(&[2, 2, 4]);
        let movement = calculate_move(&board, Direction::Left).unwrap();
        assert_eq!(&movement.board[..4], &[4, 4, 0, 0]);
        assert_eq!(movement.score_delta, 4);
    }

    #[test]
    fn all_directions_compress_and_merge_toward_the_edge() {
        for (direction, indices) in [
            (Direction::Left, [0, 1, 2, 3]),
            (Direction::Right, [3, 2, 1, 0]),
            (Direction::Up, [0, 4, 8, 12]),
            (Direction::Down, [12, 8, 4, 0]),
        ] {
            let mut board = [0; CELLS];
            board[indices[1]] = 2;
            board[indices[3]] = 2;
            let moved = calculate_move(&board, direction).unwrap();
            assert_eq!(moved.board[indices[0]], 4);
            assert_eq!(moved.score_delta, 4);
        }
    }

    #[test]
    fn invalid_move_does_not_change_board_or_score() {
        let mut board = [0; CELLS];
        board[0] = 2;
        assert_eq!(calculate_move(&board, Direction::Left), None);
        assert_eq!(board[0], 2);
    }

    #[test]
    fn full_board_with_no_equal_neighbors_is_game_over() {
        let board = [2, 4, 2, 4, 4, 2, 4, 2, 2, 4, 2, 4, 4, 2, 4, 2];
        assert!(!has_legal_move(&board));
    }

    #[test]
    fn spawn_uses_an_empty_cell_and_reset_keeps_best_score() {
        let mut game = Game::new(1);
        assert_eq!(empty_count(&game.board), 14);
        game.best_score = 128;
        game.reset();
        assert_eq!(empty_count(&game.board), 14);
        assert_eq!(game.best_score, 128);
    }

    #[test]
    fn valid_move_scores_and_spawns_once_while_invalid_move_does_nothing() {
        let mut game = Game::new(0x1234);
        game.board = [0; CELLS];
        game.board[0] = 2;
        game.board[1] = 2;
        game.score = 0;
        game.moves = 0;
        let before_empty = empty_count(&game.board);
        let movement = game.apply(Direction::Left).unwrap();
        assert_eq!(game.score, 4);
        assert_eq!(game.moves, 1);
        assert_eq!(empty_count(&game.board), before_empty);
        assert_eq!(movement.score_delta, 4);
        game.board = [2, 4, 2, 4, 4, 2, 4, 2, 2, 4, 2, 4, 4, 2, 4, 2];
        let before = game;
        assert_eq!(game.apply(Direction::Left), None);
        assert_eq!(game, before);
    }

    #[test]
    fn reaching_2048_sets_win_and_can_continue() {
        let mut game = Game::new(9);
        game.board = [0; CELLS];
        game.board[0] = 1024;
        game.board[1] = 1024;
        game.apply(Direction::Left).unwrap();
        assert!(game.won);
        assert_eq!(game.board[0], 2048);
        game.board[4] = 2;
        assert!(game.apply(Direction::Down).is_some());
        assert!(game.won);
    }

    #[test]
    fn held_direction_repeats_after_delay_and_resets_on_release() {
        let mut repeat = DirectionRepeat::default();
        assert_eq!(
            repeat.sample(Some(Direction::Left), Some(Direction::Left)),
            Some(Direction::Left)
        );
        for _ in 0..13 {
            assert_eq!(repeat.sample(Some(Direction::Left), None), None);
        }
        assert_eq!(
            repeat.sample(Some(Direction::Left), None),
            Some(Direction::Left)
        );
        for _ in 0..4 {
            assert_eq!(repeat.sample(Some(Direction::Left), None), None);
        }
        assert_eq!(
            repeat.sample(Some(Direction::Left), None),
            Some(Direction::Left)
        );
        assert_eq!(repeat.sample(None, None), None);
        assert_eq!(
            repeat.sample(Some(Direction::Left), None),
            Some(Direction::Left)
        );
    }
}
