use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    White,
    Black,
    Neutral,
}

impl Player {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'W' => Some(Player::White),
            'B' => Some(Player::Black),
            'N' => Some(Player::Neutral),
            _ => None,
        }
    }

    pub fn to_char(&self) -> char {
        match self {
            Player::White => 'W',
            Player::Black => 'B',
            Player::Neutral => 'N',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceType {
    Attacker,
    Defender,
    Ball,
}

impl PieceType {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'A' => Some(PieceType::Attacker),
            'D' => Some(PieceType::Defender),
            'B' => Some(PieceType::Ball),
            _ => None,
        }
    }

    pub fn to_char(&self) -> char {
        match self {
            PieceType::Attacker => 'A',
            PieceType::Defender => 'D',
            PieceType::Ball => 'B',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub piece_type: PieceType,
    pub player: Player,
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.player.to_char(), self.piece_type.to_char())
    }
}

#[derive(Debug, Clone)]
pub struct ChessBallBoard {
    // row-major storage
    pub rows: usize,
    pub cols: usize,
    cells: Vec<Option<Piece>>,
}

impl ChessBallBoard {
    pub const DEFAULT_ROWS: usize = 7;
    pub const DEFAULT_COLS: usize = 6;

    pub fn new() -> Self {
        let rows = Self::DEFAULT_ROWS;
        let cols = Self::DEFAULT_COLS;
        Self {
            rows,
            cols,
            cells: vec![None; rows * cols],
        }
    }

    fn idx(&self, r: usize, c: usize) -> usize {
        r * self.cols + c
    }

    pub fn place_piece(&mut self, r: usize, c: usize, piece: Piece) {
        if r >= self.rows || c >= self.cols {
            panic!("Invalid board coordinates.");
        }
        let i = self.idx(r, c);
        self.cells[i] = Some(piece);
    }

    pub fn remove_piece(&mut self, r: usize, c: usize) {
        if r >= self.rows || c >= self.cols {
            panic!("Invalid board coordinates.");
        }
        let i = self.idx(r, c);
        self.cells[i] = None;
    }

    pub fn get_piece(&self, r: usize, c: usize) -> Option<&Piece> {
        if r >= self.rows || c >= self.cols {
            panic!("Invalid board coordinates.");
        }
        self.cells[self.idx(r, c)].as_ref()
    }

    pub fn get_piece_mut(&mut self, r: usize, c: usize) -> Option<&mut Piece> {
        if r >= self.rows || c >= self.cols {
            panic!("Invalid board coordinates.");
        }
        self.cells[self.idx(r, c)].as_mut()
    }

    pub fn find_ball(&self) -> Option<(usize, usize)> {
        for r in 0..self.rows {
            for c in 0..self.cols {
                if let Some(p) = &self.cells[self.idx(r, c)] {
                    if p.piece_type == PieceType::Ball {
                        return Some((r, c));
                    }
                }
            }
        }
        None
    }

    pub fn is_forbidden_col(&self, col: usize) -> bool {
        col == 0 || col == self.cols - 1
    }

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
                board.place_piece(r, c, Piece { piece_type: ptype, player });
            }
        }
        Ok(board)
    }
}

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

// 8 directions
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
    use super::*;

    #[test]
    fn test_from_and_display_roundtrip() {
        let s = "-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- -- -- -- --\n-- -- NB -- -- --\n-- -- -- -- -- --\n-- -- WA WA -- --\n-- -- WD WD WD --\n";
        let b = ChessBallBoard::from_repr(s).unwrap();
        let out = format!("{}", b);
        assert_eq!(out, s);
    }
}