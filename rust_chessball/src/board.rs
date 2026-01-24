//! Board representation and basic utilities for ChessBall.
//!
//! The board is a fixed-size rows x cols grid stored row-major in a Vec<Option<Piece>>.
//! Public API provides placing/removing pieces, reading pieces, finding the ball,
//! and parsing/printing the textual representation used in the original Python code.

use std::fmt;

use crate::moves::DefenderTackle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Player owning a piece.
pub enum Player {
    White,
    Black,
    Neutral,
}

impl Player {
    /// Convert single-character player initial to Player.
    ///
    /// Example:
    /// ```
    /// use chessball::board::Player;
    /// assert_eq!(Player::from_char('W'), Some(Player::White));
    /// ```
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'W' => Some(Player::White),
            'B' => Some(Player::Black),
            'N' => Some(Player::Neutral),
            _ => None,
        }
    }

    /// Convert Player to single-character player initial.
    ///
    /// Example:
    /// ```
    /// use chessball::board::Player;
    /// assert_eq!(Some(Player::White).to_char(), 'W');
    /// ```
    pub fn to_char(&self) -> char {
        match self {
            Player::White => 'W',
            Player::Black => 'B',
            Player::Neutral => 'N',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Type of piece.
pub enum PieceType {
    Attacker,
    Defender,
    Ball,
}

impl PieceType {
    /// Convert piece-letter (A/D/B) to PieceType.
    ///
    /// Example:
    /// ```
    /// use chessball::board::PieceType;
    /// assert_eq!(PieceType::from_char('A'), Some(PieceType::Attacker));
    /// ```
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'A' => Some(PieceType::Attacker),
            'D' => Some(PieceType::Defender),
            'B' => Some(PieceType::Ball),
            _ => None,
        }
    }

    /// Convert PieceType to piece-letter (A/D/B).
    ///
    /// Example:
    /// ```
    /// use chessball::board::PieceType;
    /// assert_eq!(Some(PieceType::Attacker).to_char(), 'A');
    /// ```
    pub fn to_char(&self) -> char {
        match self {
            PieceType::Attacker => 'A',
            PieceType::Defender => 'D',
            PieceType::Ball => 'B',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Board piece with a type and owner.
pub struct Piece {
    pub piece_type: PieceType,
    pub player: Player,
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.player.to_char(), self.piece_type.to_char())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coord {
    pub r: isize,
    pub c: isize,
}

impl std::ops::Add for Coord {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            r: self.r + other.r,
            c: self.c + other.c,
        }
    }
}

impl std::ops::Sub for Coord {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            r: self.r - other.r,
            c: self.c - other.c,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// ChessBallBoard holds the board matrix. Defaults to 7 rows x 6 cols (same as Python port).
pub struct ChessBallBoard {
    // row-major storage
    /// number of rows
    pub rows: usize,
    /// number of cols
    pub cols: usize,
    cells: Vec<Option<Piece>>,
    pub prev_tackle: Option<DefenderTackle>,
}

impl Default for ChessBallBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl ChessBallBoard {
    pub const DEFAULT_ROWS: usize = 6;
    pub const DEFAULT_COLS: usize = 7;

    /// Create an empty board with default dimensions.
    ///
    /// Example:
    /// ```
    /// use chessball::board::ChessBallBoard;
    /// let b = ChessBallBoard::new();
    /// assert_eq!(b.rows, 6);
    /// assert_eq!(b.cols, 7);
    /// assert_eq!(b.cells.len(), 42);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let rows = Self::DEFAULT_ROWS;
        let cols = Self::DEFAULT_COLS;
        Self {
            rows,
            cols,
            cells: vec![None; rows * cols],
            prev_tackle: None,
        }
    }
    pub fn new_game() -> Self {
        let rows = Self::DEFAULT_ROWS;
        let cols = Self::DEFAULT_COLS;
        let mut board = Self {
            rows,
            cols,
            cells: vec![None; rows * cols],
            prev_tackle: None,
        };
        let (whiterow0, blackrow0) = (Self::DEFAULT_ROWS as isize - 1, 0);
        let (whiterow1, blackrow1) = (whiterow0 - 1, 1);
        for c in [1, 3, 5] {
            board.place_piece(
                &Coord { r: blackrow0, c },
                Piece {
                    piece_type: PieceType::Defender,
                    player: Player::Black,
                },
            );
            board.place_piece(
                &Coord { r: whiterow0, c },
                Piece {
                    piece_type: PieceType::Defender,
                    player: Player::White,
                },
            );
        }
        for c in [2, 4] {
            board.place_piece(
                &Coord { r: blackrow1, c },
                Piece {
                    piece_type: PieceType::Attacker,
                    player: Player::Black,
                },
            );
            board.place_piece(
                &Coord { r: whiterow1, c },
                Piece {
                    piece_type: PieceType::Attacker,
                    player: Player::White,
                },
            );
        }
        board.place_ball(&Coord { r: 2, c: 3 });
        board
    }

    /// Convert (row, col) coordinates to vector index.
    ///
    /// Example:
    /// ```
    /// use chessball::board::ChessBallBoard;
    /// let b = ChessBallBoard::new();
    /// assert_eq!(b.idx(0, 0), 0);
    /// assert_eq!(b.idx(0, 1), 1);
    /// assert_eq!(b.idx(1, 0), b.cols);
    /// ```
    #[must_use]
    fn idx(&self, coord: &Coord) -> usize {
        (coord.r as usize) * self.cols + (coord.c as usize)
    }

    #[must_use]
    pub fn is_on_board(&self, at: &Coord) -> bool {
        0 <= at.r && at.r < self.rows as isize && 0 <= at.c && at.c < self.cols as isize
    }

    /// Place a piece at (r, c). Panics on out-of-bounds coordinates.
    pub fn place_piece(&mut self, at: &Coord, piece: Piece) {
        if !self.is_on_board(at) {
            panic!("Invalid board coordinates.");
        }
        let i = self.idx(at);
        self.cells[i] = Some(piece);
    }

    /// Place a piece at (r, c). Panics on out-of-bounds coordinates.
    pub fn place_ball(&mut self, at: &Coord) {
        self.place_piece(
            at,
            Piece {
                piece_type: PieceType::Ball,
                player: Player::Neutral,
            },
        );
    }

    /// Remove the piece at (r, c). Panics on out-of-bounds coordinates.
    pub fn remove_piece(&mut self, at: &Coord) {
        if !self.is_on_board(at) {
            panic!("Invalid board coordinates.");
        }
        let i = self.idx(at);
        self.cells[i] = None;
    }

    /// Get a reference to the piece at (r, c), or None. Panics on out-of-bounds.
    #[must_use]
    pub fn get_piece(&self, at: &Coord) -> Option<&Piece> {
        if !self.is_on_board(at) {
            panic!("Invalid board coordinates.");
        }
        self.cells[self.idx(at)].as_ref()
    }

    // pub fn get_piece_mut(&mut self, r: usize, c: usize) -> Option<&mut Piece> {
    //     if r >= self.rows || c >= self.cols {
    //         panic!("Invalid board coordinates.");
    //     }
    //     self.cells[self.idx(r, c)].as_mut()
    // }

    pub fn iter_coords(&self) -> impl std::iter::Iterator<Item = Coord> {
        (0..self.cols * self.rows).map(|i| Coord {
            r: (i / self.cols) as isize,
            c: (i % self.cols) as isize,
        })
    }

    /// Find the ball on the board. Returns (row, col) or None if no ball.
    ///
    /// Example:
    /// ```
    /// use chessball::board::{ChessBallBoard, Piece, PieceType, Player};
    /// let mut b = ChessBallBoard::new();
    /// b.place_piece(2, 4, Piece { piece_type: PieceType::Ball, player: Player::Neutral });
    /// assert_eq!(b.find_ball(), Some((2, 4)));
    /// ```
    #[must_use]
    pub fn find_ball(&self) -> Option<Coord> {
        for coord in self.iter_coords() {
            if let Some(p) = &self.cells[self.idx(&coord)]
                && p.piece_type == PieceType::Ball
            {
                return Some(coord);
            }
        }
        None
    }

    /// Returns true if the column is forbidden for a ball destination (col 0 or last).
    #[must_use]
    pub fn is_forbidden_col(&self, coord: &Coord) -> bool {
        coord.c == 0 || coord.c == (self.cols - 1) as isize
    }

    /// Parse the textual repr given by Display into a ChessBallBoard.
    ///
    /// The format uses ROWS lines, each with COLS tokens separated by spaces.
    /// '--' denotes empty, otherwise two chars: <PlayerInitial><PieceInitial>, e.g. 'WA', 'NB'.
    ///
    /// Returns Err if formatting is invalid.
    pub fn from_repr(s: &str) -> Result<Self, String> {
        let lines: Vec<&str> = s
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        let n_rows = lines.len();
        let n_cols = (lines[0].len() + 1) / 3;
        let mut board = ChessBallBoard {
            rows: n_rows,
            cols: n_cols,
            cells: vec![None; n_rows * n_cols],
            prev_tackle: None,
        };
        if lines.len() != board.rows {
            return Err(format!("Expected {} rows, got {}", board.rows, lines.len()));
        }
        for (r, line) in lines.into_iter().enumerate() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() != board.cols {
                return Err(format!(
                    "Expected {} cols at row {}, got {}",
                    board.cols,
                    r,
                    tokens.len()
                ));
            }
            for (c, tok) in tokens.into_iter().enumerate() {
                if tok == "--" {
                    continue;
                }
                if tok.len() != 2 {
                    return Err(format!("Invalid token '{}' at {},{}", tok, r, c));
                }
                let mut chars = tok.chars();
                let pch = chars.next().unwrap();
                let tch = chars.next().unwrap();
                let player = Player::from_char(pch)
                    .ok_or_else(|| format!("Unknown player '{}' at {},{}", pch, r, c))?;
                let ptype = PieceType::from_char(tch)
                    .ok_or_else(|| format!("Unknown piece '{}' at {},{}", tch, r, c))?;
                board.place_piece(
                    &Coord {
                        r: r as isize,
                        c: c as isize,
                    },
                    Piece {
                        piece_type: ptype,
                        player,
                    },
                );
            }
        }
        Ok(board)
    }
}

/// Prints the board in a way consistent with from_repr
impl fmt::Display for ChessBallBoard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for coord in self.iter_coords() {
            match &self.cells[self.idx(&coord)] {
                Some(piece) => write!(f, "{}", piece)?,
                None => write!(f, "--")?,
            }
            if coord.c + 1 < self.cols as isize {
                write!(f, " ")?;
            }
            if coord.c as usize == self.cols - 1 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

/// 8 directions of adjacency: orthogonal + diagonal
pub const DIRECTIONS: &[Coord] = &[
    Coord { r: -1, c: 0 },
    Coord { r: 1, c: 0 },
    Coord { r: 0, c: -1 },
    Coord { r: 0, c: 1 },
    Coord { r: -1, c: -1 },
    Coord { r: -1, c: 1 },
    Coord { r: 1, c: -1 },
    Coord { r: 1, c: 1 },
];

#[cfg(test)]
mod tests {
    use crate::board::{ChessBallBoard, Coord, Piece, PieceType, Player};

    #[test]
    fn test_board_from_repr_and_display_roundtrip() {
        let s = "-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- NB -- -- --\n-- -- -- -- -- --\n-- -- WA WA -- --\n-- -- WD WD WD --\n";
        let b = ChessBallBoard::from_repr(s).unwrap();
        let out = format!("{}", b);
        assert_eq!(out, s);
    }

    #[test]
    fn test_display_empty_board() {
        let board = ChessBallBoard::new();
        let empty_row = (0..board.cols).map(|_| "--").collect::<Vec<_>>().join(" ");
        let expected = (0..board.rows)
            .map(|_| empty_row.clone())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(format!("{}", board), expected);
    }

    #[test]
    fn test_display_and_from_repr_single_piece() {
        let mut board = ChessBallBoard::new();
        board.place_piece(
            &Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        ); // WD
        let token_grid = {
            let mut g = vec![vec!["--"; board.cols]; board.rows];
            g[2][3] = "WD";
            g
        };
        let expected = token_grid
            .into_iter()
            .map(|row| row.join(" "))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(format!("{}", board), expected);

        let parsed = ChessBallBoard::from_repr(&expected).unwrap();
        assert_eq!(&board, &parsed);
        let p = parsed.get_piece(&Coord { r: 2, c: 3 }).unwrap();
        assert_eq!(p.player, Player::White);
        assert_eq!(p.piece_type, PieceType::Defender);
    }

    #[test]
    fn test_display_and_from_repr_roundtrip_idempotent() {
        let mut boards = Vec::new();
        boards.push(ChessBallBoard::new());
        let mut b1 = ChessBallBoard::new();
        b1.place_piece(
            &Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        boards.push(b1);
        let mut b2 = ChessBallBoard::new();
        b2.place_piece(
            &Coord { r: 0, c: 0 },
            Piece {
                piece_type: PieceType::Attacker,
                player: Player::White,
            },
        );
        b2.place_piece(
            &Coord { r: 3, c: 3 },
            Piece {
                piece_type: PieceType::Ball,
                player: Player::Neutral,
            },
        );
        b2.place_piece(
            &Coord { r: 5, c: 5 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::Black,
            },
        );
        boards.push(b2);

        for original in boards {
            let s = format!("{}", original);
            let parsed = ChessBallBoard::from_repr(&s).unwrap();
            let rs = format!("{}", parsed);
            assert_eq!(s, rs);
            assert_eq!(&original, &parsed);
        }
    }

    #[test]
    fn test_board_new_game() {
        let board_a = ChessBallBoard::new_game();
        let board_b = ChessBallBoard::from_repr(
            "\
-- BD -- BD -- BD --\n\
-- -- BA -- BA -- --\n\
-- -- -- NB -- -- --\n\
-- -- -- -- -- -- --\n\
-- -- WA -- WA -- --\n\
-- WD -- WD -- WD --\n",
        )
        .unwrap();
        assert_eq!(board_a, board_b);
    }

    #[test]
    fn test_board_place_ball() {
        let s = "-- NB -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n";
        let mut b = ChessBallBoard::from_repr(s).unwrap();
        b.place_ball(&Coord { r: 0, c: 1 });
        let out = format!("{}", b);
        assert_eq!(out, s);
    }

    #[test]
    fn test_board_place_piece() {
        let s = "NB -- -- -- -- --\n-- -- WA -- -- --\n-- -- -- -- -- --\n-- -- -- -- BD --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- WD\n";
        let mut b = ChessBallBoard::from_repr(s).unwrap();
        b.place_piece(
            &Coord { r: 1, c: 2 },
            Piece {
                piece_type: PieceType::Attacker,
                player: Player::White,
            },
        );
        b.place_piece(
            &Coord { r: 6, c: 5 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        b.place_piece(
            &Coord { r: 3, c: 4 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::Black,
            },
        );
        b.place_piece(
            &Coord { r: 0, c: 0 },
            Piece {
                piece_type: PieceType::Ball,
                player: Player::Neutral,
            },
        );
        let out = format!("{}", b);
        assert_eq!(out, s);
    }
}
