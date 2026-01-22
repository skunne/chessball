//! Board representation and basic utilities for ChessBall.
//!
//! The board is a fixed-size rows x cols grid stored row-major in a Vec<Option<Piece>>.
//! Public API provides placing/removing pieces, reading pieces, finding the ball,
//! and parsing/printing the textual representation used in the original Python code.

use std::fmt;

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
/// ChessBallBoard holds the board matrix. Defaults to 7 rows x 6 cols (same as Python port).
pub struct ChessBallBoard {
    // row-major storage
    /// number of rows
    pub rows: usize,
    /// number of cols
    pub cols: usize,
    cells: Vec<Option<Piece>>,
}

impl Default for ChessBallBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl ChessBallBoard {
    /// Default board rows used in the Python codebase.
    pub const DEFAULT_ROWS: usize = 7;
    /// Default board cols used in the Python codebase.
    pub const DEFAULT_COLS: usize = 6;

    /// Create an empty board with default dimensions.
    ///
    /// Example:
    /// ```
    /// use chessball::board::ChessBallBoard;
    /// let b = ChessBallBoard::new();
    /// assert_eq!(b.rows, 7);
    /// assert_eq!(b.cols, 6);
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
        }
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
    fn idx(&self, r: usize, c: usize) -> usize {
        r * self.cols + c
    }

    /// Place a piece at (r, c). Panics on out-of-bounds coordinates.
    pub fn place_piece(&mut self, r: usize, c: usize, piece: Piece) {
        if r >= self.rows || c >= self.cols {
            panic!("Invalid board coordinates.");
        }
        let i = self.idx(r, c);
        self.cells[i] = Some(piece);
    }

    /// Place a piece at (r, c). Panics on out-of-bounds coordinates.
    pub fn place_ball(&mut self, r: usize, c: usize) {
        self.place_piece(
            r,
            c,
            Piece {
                piece_type: PieceType::Ball,
                player: Player::Neutral,
            },
        );
    }

    /// Remove the piece at (r, c). Panics on out-of-bounds coordinates.
    pub fn remove_piece(&mut self, r: usize, c: usize) {
        if r >= self.rows || c >= self.cols {
            panic!("Invalid board coordinates.");
        }
        let i = self.idx(r, c);
        self.cells[i] = None;
    }

    /// Get a reference to the piece at (r, c), or None. Panics on out-of-bounds.
    #[must_use]
    pub fn get_piece(&self, r: usize, c: usize) -> Option<&Piece> {
        if r >= self.rows || c >= self.cols {
            panic!("Invalid board coordinates.");
        }
        self.cells[self.idx(r, c)].as_ref()
    }

    // pub fn get_piece_mut(&mut self, r: usize, c: usize) -> Option<&mut Piece> {
    //     if r >= self.rows || c >= self.cols {
    //         panic!("Invalid board coordinates.");
    //     }
    //     self.cells[self.idx(r, c)].as_mut()
    // }

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
    pub fn find_ball(&self) -> Option<(usize, usize)> {
        for r in 0..self.rows {
            for c in 0..self.cols {
                if let Some(p) = &self.cells[self.idx(r, c)]
                    && p.piece_type == PieceType::Ball
                {
                    return Some((r, c));
                }
            }
        }
        None
    }

    /// Returns true if the column is forbidden for a ball destination (col 0 or last).
    #[must_use]
    pub fn is_forbidden_col(&self, col: usize) -> bool {
        col == 0 || col == self.cols - 1
    }

    /// Parse the textual repr given by Display into a ChessBallBoard.
    ///
    /// The format uses ROWS lines, each with COLS tokens separated by spaces.
    /// '--' denotes empty, otherwise two chars: <PlayerInitial><PieceInitial>, e.g. 'WA', 'NB'.
    ///
    /// Returns Err if formatting is invalid.
    pub fn from_repr(s: &str) -> Result<Self, String> {
        let mut board = ChessBallBoard::new();
        let lines: Vec<&str> = s
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
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
                    r,
                    c,
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
        for r in 0..self.rows {
            for c in 0..self.cols {
                match &self.cells[self.idx(r, c)] {
                    Some(piece) => write!(f, "{}", piece)?,
                    None => write!(f, "--")?,
                }
                if c + 1 < self.cols {
                    write!(f, " ")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// 8 directions of adjacency: orthogonal + diagonal
pub const DIRECTIONS: &[(isize, isize)] = &[
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
];

#[cfg(test)]
mod tests {
    use crate::board::{ChessBallBoard, Piece, PieceType, Player};

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
            2,
            3,
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
        let p = parsed.get_piece(2, 3).unwrap();
        assert_eq!(p.player, Player::White);
        assert_eq!(p.piece_type, PieceType::Defender);
    }

    #[test]
    fn test_display_and_from_repr_roundtrip_idempotent() {
        let mut boards = Vec::new();
        boards.push(ChessBallBoard::new());
        let mut b1 = ChessBallBoard::new();
        b1.place_piece(
            2,
            3,
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        boards.push(b1);
        let mut b2 = ChessBallBoard::new();
        b2.place_piece(
            0,
            0,
            Piece {
                piece_type: PieceType::Attacker,
                player: Player::White,
            },
        );
        b2.place_piece(
            3,
            3,
            Piece {
                piece_type: PieceType::Ball,
                player: Player::Neutral,
            },
        );
        b2.place_piece(
            5,
            5,
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
    fn test_board_place_ball() {
        let s = "-- NB -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n";
        let mut b = ChessBallBoard::from_repr(s).unwrap();
        b.place_ball(0, 1);
        let out = format!("{}", b);
        assert_eq!(out, s);
    }

    #[test]
    fn test_board_place_piece() {
        let s = "NB -- -- -- -- --\n-- -- WA -- -- --\n-- -- -- -- -- --\n-- -- -- -- BD --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- WD\n";
        let mut b = ChessBallBoard::from_repr(s).unwrap();
        b.place_piece(
            1,
            2,
            Piece {
                piece_type: PieceType::Attacker,
                player: Player::White,
            },
        );
        b.place_piece(
            6,
            5,
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        b.place_piece(
            3,
            4,
            Piece {
                piece_type: PieceType::Defender,
                player: Player::Black,
            },
        );
        b.place_piece(
            0,
            0,
            Piece {
                piece_type: PieceType::Ball,
                player: Player::Neutral,
            },
        );
        let out = format!("{}", b);
        assert_eq!(out, s);
    }
}
