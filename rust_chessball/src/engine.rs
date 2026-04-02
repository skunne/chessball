use std::fmt;

pub const ROWS: usize = 6;
pub const COLS: usize = 7;
pub const NUM_SQUARES: usize = ROWS * COLS;

const DIRECTIONS: [(i8, i8); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
];

const MAX_WHITE_ATTACKERS: usize = 2;
const MAX_WHITE_DEFENDERS: usize = 3;
const MAX_BLACK_ATTACKERS: usize = 2;
const MAX_BLACK_DEFENDERS: usize = 3;
const MAX_ATTACKER_MOVES_PER_DIRECTION: usize = 2;
const MAX_DEFENDER_MOVES_PER_DIRECTION: usize = 1;
pub const MAX_MOVES_PER_POSITION: usize =
    MAX_WHITE_ATTACKERS * DIRECTIONS.len() * MAX_ATTACKER_MOVES_PER_DIRECTION
        + MAX_WHITE_DEFENDERS * DIRECTIONS.len() * MAX_DEFENDER_MOVES_PER_DIRECTION;
const PACKED_SQUARE_BITS: u32 = 6;
const PACKED_SQUARE_MASK: u128 = (1u128 << PACKED_SQUARE_BITS) - 1;
const PACKED_EMPTY_SQUARE: u8 = 0b11_1111;
const WHITE_ATTACKER_OFFSET: u32 = 0;
const WHITE_DEFENDER_OFFSET: u32 =
    WHITE_ATTACKER_OFFSET + (MAX_WHITE_ATTACKERS as u32 * PACKED_SQUARE_BITS);
const BLACK_ATTACKER_OFFSET: u32 =
    WHITE_DEFENDER_OFFSET + (MAX_WHITE_DEFENDERS as u32 * PACKED_SQUARE_BITS);
const BLACK_DEFENDER_OFFSET: u32 =
    BLACK_ATTACKER_OFFSET + (MAX_BLACK_ATTACKERS as u32 * PACKED_SQUARE_BITS);
const BALL_OFFSET: u32 = BLACK_DEFENDER_OFFSET + (MAX_BLACK_DEFENDERS as u32 * PACKED_SQUARE_BITS);
const TO_MOVE_OFFSET: u32 = BALL_OFFSET + PACKED_SQUARE_BITS;
const LAST_TACKLE_DEFENDER_OFFSET: u32 = TO_MOVE_OFFSET + 1;
const LAST_TACKLE_VICTIM_OFFSET: u32 = LAST_TACKLE_DEFENDER_OFFSET + PACKED_SQUARE_BITS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Player {
    White,
    Black,
}

impl Player {
    #[must_use]
    pub const fn opponent(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    #[must_use]
    pub const fn goal_row(self) -> usize {
        match self {
            Self::White => ROWS - 1,
            Self::Black => 0,
        }
    }

    #[must_use]
    pub const fn to_char(self) -> char {
        match self {
            Self::White => 'W',
            Self::Black => 'B',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PieceKind {
    Attacker,
    Defender,
}

impl PieceKind {
    #[must_use]
    pub const fn to_char(self) -> char {
        match self {
            Self::Attacker => 'A',
            Self::Defender => 'D',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Piece {
    pub player: Player,
    pub kind: PieceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackedPosition(u128);

impl PackedPosition {
    #[must_use]
    pub const fn raw(self) -> u128 {
        self.0
    }

    #[must_use]
    pub const fn from_raw(raw: u128) -> Self {
        Self(raw)
    }

    #[must_use]
    pub fn unpack(self) -> Position {
        Position::from_packed(self)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Symmetry {
    Identity = 0,
    MirrorHorizontal = 1,
    Rotate180SwapColors = 2,
    MirrorVerticalSwapColors = 3,
}

impl Symmetry {
    pub const ALL: [Self; 4] = [
        Self::Identity,
        Self::MirrorHorizontal,
        Self::Rotate180SwapColors,
        Self::MirrorVerticalSwapColors,
    ];

    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Identity,
            1 => Self::MirrorHorizontal,
            2 => Self::Rotate180SwapColors,
            _ => Self::MirrorVerticalSwapColors,
        }
    }

    #[must_use]
    pub const fn combine(self, other: Self) -> Self {
        Self::from_bits(self as u8 ^ other as u8)
    }

    #[must_use]
    pub const fn flips_horizontal(self) -> bool {
        matches!(self, Self::MirrorHorizontal | Self::Rotate180SwapColors)
    }

    #[must_use]
    pub const fn flips_vertical(self) -> bool {
        matches!(
            self,
            Self::Rotate180SwapColors | Self::MirrorVerticalSwapColors
        )
    }

    #[must_use]
    pub const fn swaps_colors(self) -> bool {
        matches!(
            self,
            Self::Rotate180SwapColors | Self::MirrorVerticalSwapColors
        )
    }

    #[must_use]
    pub const fn apply_player(self, player: Player) -> Player {
        if self.swaps_colors() {
            player.opponent()
        } else {
            player
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    #[must_use]
    pub fn new(row: usize, col: usize) -> Option<Self> {
        if row < ROWS && col < COLS {
            Some(Self((row * COLS + col) as u8))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[must_use]
    pub const fn row(self) -> usize {
        self.index() / COLS
    }

    #[must_use]
    pub const fn col(self) -> usize {
        self.index() % COLS
    }

    #[must_use]
    pub fn offset(self, dr: i8, dc: i8) -> Option<Self> {
        let row = self.row() as i16 + dr as i16;
        let col = self.col() as i16 + dc as i16;
        if row < 0 || col < 0 {
            return None;
        }
        Self::new(row as usize, col as usize)
    }

    #[must_use]
    pub const fn mirrored_horizontal(self) -> Self {
        square(self.row(), COLS - 1 - self.col())
    }

    #[must_use]
    pub const fn mirrored_vertical(self) -> Self {
        square(ROWS - 1 - self.row(), self.col())
    }

    #[must_use]
    pub const fn apply_symmetry(self, symmetry: Symmetry) -> Self {
        let row = if symmetry.flips_vertical() {
            ROWS - 1 - self.row()
        } else {
            self.row()
        };
        let col = if symmetry.flips_horizontal() {
            COLS - 1 - self.col()
        } else {
            self.col()
        };
        square(row, col)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.row(), self.col())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TackleMemory {
    pub defender_to: Square,
    pub victim_to: Square,
}

impl TackleMemory {
    #[must_use]
    pub const fn apply_symmetry(self, symmetry: Symmetry) -> Self {
        Self {
            defender_to: self.defender_to.apply_symmetry(symmetry),
            victim_to: self.victim_to.apply_symmetry(symmetry),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MoveKind {
    Simple,
    Push {
        ball_to: Square,
    },
    Jump {
        jumped: Square,
    },
    Tackle {
        pushed_from: Square,
        pushed_to: Square,
    },
}

impl MoveKind {
    #[must_use]
    pub const fn apply_symmetry(self, symmetry: Symmetry) -> Self {
        match self {
            Self::Simple => Self::Simple,
            Self::Push { ball_to } => Self::Push {
                ball_to: ball_to.apply_symmetry(symmetry),
            },
            Self::Jump { jumped } => Self::Jump {
                jumped: jumped.apply_symmetry(symmetry),
            },
            Self::Tackle {
                pushed_from,
                pushed_to,
            } => Self::Tackle {
                pushed_from: pushed_from.apply_symmetry(symmetry),
                pushed_to: pushed_to.apply_symmetry(symmetry),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub kind: MoveKind,
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            MoveKind::Simple => write!(f, "{} -> {}", self.from, self.to),
            MoveKind::Push { ball_to } => {
                write!(f, "{} -> {} push {}", self.from, self.to, ball_to)
            }
            MoveKind::Jump { jumped } => {
                write!(f, "{} -> {} jump {}", self.from, self.to, jumped)
            }
            MoveKind::Tackle {
                pushed_from,
                pushed_to,
            } => write!(
                f,
                "{} -> {} tackle {} -> {}",
                self.from, self.to, pushed_from, pushed_to
            ),
        }
    }
}

impl Move {
    #[must_use]
    pub const fn apply_symmetry(self, symmetry: Symmetry) -> Self {
        Self {
            from: self.from.apply_symmetry(symmetry),
            to: self.to.apply_symmetry(symmetry),
            kind: self.kind.apply_symmetry(symmetry),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    white_attackers: u64,
    white_defenders: u64,
    black_attackers: u64,
    black_defenders: u64,
    ball: Square,
    pub to_move: Player,
    pub last_tackle: Option<TackleMemory>,
}

impl Position {
    #[must_use]
    pub fn empty(ball: Square, to_move: Player) -> Self {
        Self {
            white_attackers: 0,
            white_defenders: 0,
            black_attackers: 0,
            black_defenders: 0,
            ball,
            to_move,
            last_tackle: None,
        }
    }

    #[must_use]
    pub fn new_game() -> Self {
        let mut position = Self::empty(square(2, 3), Player::White);
        for col in [1, 3, 5] {
            position.put_piece(
                square(0, col),
                Piece {
                    player: Player::White,
                    kind: PieceKind::Defender,
                },
            );
            position.put_piece(
                square(5, col),
                Piece {
                    player: Player::Black,
                    kind: PieceKind::Defender,
                },
            );
        }
        for col in [2, 4] {
            position.put_piece(
                square(1, col),
                Piece {
                    player: Player::White,
                    kind: PieceKind::Attacker,
                },
            );
            position.put_piece(
                square(4, col),
                Piece {
                    player: Player::Black,
                    kind: PieceKind::Attacker,
                },
            );
        }
        position
    }

    pub fn from_repr(input: &str, to_move: Player) -> Result<Self, String> {
        let lines: Vec<&str> = input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        if lines.len() != ROWS {
            return Err(format!("expected {ROWS} rows, got {}", lines.len()));
        }
        let mut ball = None;
        let mut position = Self::empty(square(0, 0), to_move);
        for (row, line) in lines.into_iter().enumerate() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() != COLS {
                return Err(format!(
                    "expected {COLS} columns on row {row}, got {}",
                    tokens.len()
                ));
            }
            for (col, token) in tokens.into_iter().enumerate() {
                let at = square(row, col);
                match token {
                    "--" => {}
                    "NB" => {
                        if ball.replace(at).is_some() {
                            return Err("board contains more than one ball".to_string());
                        }
                    }
                    _ => {
                        if token.len() != 2 {
                            return Err(format!("invalid token '{token}' at ({row}, {col})"));
                        }
                        let mut chars = token.chars();
                        let player = match chars.next().unwrap() {
                            'W' => Player::White,
                            'B' => Player::Black,
                            other => {
                                return Err(format!(
                                    "invalid player token '{other}' at ({row}, {col})"
                                ));
                            }
                        };
                        let kind = match chars.next().unwrap() {
                            'A' => PieceKind::Attacker,
                            'D' => PieceKind::Defender,
                            other => {
                                return Err(format!(
                                    "invalid piece token '{other}' at ({row}, {col})"
                                ));
                            }
                        };
                        position.put_piece(at, Piece { player, kind });
                    }
                }
            }
        }
        position.ball = ball.ok_or_else(|| "board does not contain a ball".to_string())?;
        if position.has_piece(position.ball) {
            return Err("ball overlaps with a piece".to_string());
        }
        Ok(position)
    }

    #[must_use]
    pub fn pack(self) -> PackedPosition {
        let mut raw = 0u128;
        pack_piece_slots(
            &mut raw,
            self.white_attackers,
            WHITE_ATTACKER_OFFSET,
            MAX_WHITE_ATTACKERS,
        );
        pack_piece_slots(
            &mut raw,
            self.white_defenders,
            WHITE_DEFENDER_OFFSET,
            MAX_WHITE_DEFENDERS,
        );
        pack_piece_slots(
            &mut raw,
            self.black_attackers,
            BLACK_ATTACKER_OFFSET,
            MAX_BLACK_ATTACKERS,
        );
        pack_piece_slots(
            &mut raw,
            self.black_defenders,
            BLACK_DEFENDER_OFFSET,
            MAX_BLACK_DEFENDERS,
        );
        store_square_bits(&mut raw, BALL_OFFSET, self.ball.0);
        if self.to_move == Player::Black {
            raw |= 1u128 << TO_MOVE_OFFSET;
        }
        let (defender_to, victim_to) = match self.last_tackle {
            Some(memory) => (memory.defender_to.0, memory.victim_to.0),
            None => (PACKED_EMPTY_SQUARE, PACKED_EMPTY_SQUARE),
        };
        store_square_bits(&mut raw, LAST_TACKLE_DEFENDER_OFFSET, defender_to);
        store_square_bits(&mut raw, LAST_TACKLE_VICTIM_OFFSET, victim_to);
        PackedPosition(raw)
    }

    #[must_use]
    pub fn from_packed(packed: PackedPosition) -> Self {
        let raw = packed.raw();
        let ball = decode_required_square(load_square_bits(raw, BALL_OFFSET));
        let to_move = if ((raw >> TO_MOVE_OFFSET) & 1) == 0 {
            Player::White
        } else {
            Player::Black
        };
        let mut position = Self::empty(ball, to_move);

        unpack_piece_slots(
            &mut position,
            raw,
            WHITE_ATTACKER_OFFSET,
            MAX_WHITE_ATTACKERS,
            Player::White,
            PieceKind::Attacker,
        );
        unpack_piece_slots(
            &mut position,
            raw,
            WHITE_DEFENDER_OFFSET,
            MAX_WHITE_DEFENDERS,
            Player::White,
            PieceKind::Defender,
        );
        unpack_piece_slots(
            &mut position,
            raw,
            BLACK_ATTACKER_OFFSET,
            MAX_BLACK_ATTACKERS,
            Player::Black,
            PieceKind::Attacker,
        );
        unpack_piece_slots(
            &mut position,
            raw,
            BLACK_DEFENDER_OFFSET,
            MAX_BLACK_DEFENDERS,
            Player::Black,
            PieceKind::Defender,
        );

        let defender_to =
            decode_optional_square(load_square_bits(raw, LAST_TACKLE_DEFENDER_OFFSET));
        let victim_to = decode_optional_square(load_square_bits(raw, LAST_TACKLE_VICTIM_OFFSET));
        position.last_tackle = match (defender_to, victim_to) {
            (None, None) => None,
            (Some(defender_to), Some(victim_to)) => Some(TackleMemory {
                defender_to,
                victim_to,
            }),
            _ => panic!("invalid packed tackle memory"),
        };

        position
    }

    #[must_use]
    pub const fn ball(&self) -> Square {
        self.ball
    }

    #[must_use]
    pub fn winner(&self) -> Option<Player> {
        match self.ball.row() {
            row if row == Player::Black.goal_row() => Some(Player::Black),
            row if row == Player::White.goal_row() => Some(Player::White),
            _ => None,
        }
    }

    #[must_use]
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        let bit = bit(square);
        if self.white_attackers & bit != 0 {
            return Some(Piece {
                player: Player::White,
                kind: PieceKind::Attacker,
            });
        }
        if self.white_defenders & bit != 0 {
            return Some(Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            });
        }
        if self.black_attackers & bit != 0 {
            return Some(Piece {
                player: Player::Black,
                kind: PieceKind::Attacker,
            });
        }
        if self.black_defenders & bit != 0 {
            return Some(Piece {
                player: Player::Black,
                kind: PieceKind::Defender,
            });
        }
        None
    }

    #[must_use]
    pub fn is_ball_at(&self, square: Square) -> bool {
        self.ball == square
    }

    #[must_use]
    pub fn has_piece(&self, square: Square) -> bool {
        self.piece_at(square).is_some()
    }

    #[must_use]
    pub fn is_empty(&self, square: Square) -> bool {
        !self.has_piece(square) && !self.is_ball_at(square)
    }

    #[must_use]
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        self.generate_piece_moves(self.to_move, PieceKind::Attacker, &mut moves);
        self.generate_piece_moves(self.to_move, PieceKind::Defender, &mut moves);
        moves
    }

    #[must_use]
    pub fn winning_moves(&self) -> Vec<Move> {
        let player = self.to_move;
        self.legal_moves()
            .into_iter()
            .filter(|mv| self.apply(*mv).winner() == Some(player))
            .collect()
    }

    #[must_use]
    pub fn apply(&self, mv: Move) -> Self {
        let moving_piece = self
            .piece_at(mv.from)
            .expect("move source does not contain a piece");
        assert_eq!(moving_piece.player, self.to_move, "move from wrong side");

        let mut next = *self;
        next.last_tackle = None;
        next.remove_piece(mv.from);

        match mv.kind {
            MoveKind::Simple => {
                assert!(
                    self.is_empty(mv.to),
                    "simple move destination must be empty"
                );
                next.put_piece(mv.to, moving_piece);
            }
            MoveKind::Push { ball_to } => {
                assert!(
                    self.is_ball_at(mv.to),
                    "push must move into the ball square"
                );
                assert!(self.is_empty(ball_to), "ball destination must be empty");
                assert!(
                    !Self::is_forbidden_ball_destination(ball_to),
                    "ball destination may not be inside a touch zone"
                );
                next.ball = ball_to;
                next.put_piece(mv.to, moving_piece);
            }
            MoveKind::Jump { jumped } => {
                assert!(
                    moving_piece.kind == PieceKind::Attacker,
                    "only attackers may jump"
                );
                assert!(
                    !self.is_empty(jumped),
                    "jumped square must contain a piece or the ball"
                );
                assert!(self.is_empty(mv.to), "jump destination must be empty");
                next.put_piece(mv.to, moving_piece);
            }
            MoveKind::Tackle {
                pushed_from,
                pushed_to,
            } => {
                assert!(
                    moving_piece.kind == PieceKind::Defender,
                    "only defenders may tackle"
                );
                let pushed_piece = self
                    .piece_at(pushed_from)
                    .expect("tackle target must contain a piece");
                assert_ne!(
                    pushed_piece.player, moving_piece.player,
                    "tackle target must be an opponent piece"
                );
                assert!(
                    self.is_empty(pushed_to),
                    "tackle landing square must be empty"
                );
                next.remove_piece(pushed_from);
                next.put_piece(pushed_to, pushed_piece);
                next.put_piece(mv.to, moving_piece);
                next.last_tackle = Some(TackleMemory {
                    defender_to: mv.to,
                    victim_to: pushed_to,
                });
            }
        }

        next.to_move = self.to_move.opponent();
        next
    }

    pub fn put_piece(&mut self, square: Square, piece: Piece) {
        assert_ne!(square, self.ball, "piece may not overlap with the ball");
        self.remove_piece(square);
        let slot = match (piece.player, piece.kind) {
            (Player::White, PieceKind::Attacker) => &mut self.white_attackers,
            (Player::White, PieceKind::Defender) => &mut self.white_defenders,
            (Player::Black, PieceKind::Attacker) => &mut self.black_attackers,
            (Player::Black, PieceKind::Defender) => &mut self.black_defenders,
        };
        *slot |= bit(square);
    }

    pub fn remove_piece(&mut self, square: Square) {
        let mask = !bit(square);
        self.white_attackers &= mask;
        self.white_defenders &= mask;
        self.black_attackers &= mask;
        self.black_defenders &= mask;
    }

    #[must_use]
    pub fn count_pieces(&self, player: Player, kind: PieceKind) -> u32 {
        self.piece_bits(player, kind).count_ones()
    }

    #[must_use]
    pub fn mirrored_horizontal(self) -> Self {
        let mut mirrored = Self::empty(self.ball.mirrored_horizontal(), self.to_move);
        for row in 0..ROWS {
            for col in 0..COLS {
                let at = square(row, col);
                if let Some(piece) = self.piece_at(at) {
                    mirrored.put_piece(at.mirrored_horizontal(), piece);
                }
            }
        }
        mirrored.last_tackle = self.last_tackle.map(|memory| TackleMemory {
            defender_to: memory.defender_to.mirrored_horizontal(),
            victim_to: memory.victim_to.mirrored_horizontal(),
        });
        mirrored
    }

    #[must_use]
    pub fn apply_symmetry(self, symmetry: Symmetry) -> Self {
        let mut transformed = Self::empty(
            self.ball.apply_symmetry(symmetry),
            symmetry.apply_player(self.to_move),
        );
        for row in 0..ROWS {
            for col in 0..COLS {
                let at = square(row, col);
                if let Some(mut piece) = self.piece_at(at) {
                    piece.player = symmetry.apply_player(piece.player);
                    transformed.put_piece(at.apply_symmetry(symmetry), piece);
                }
            }
        }
        transformed.last_tackle = self
            .last_tackle
            .map(|memory| memory.apply_symmetry(symmetry));
        transformed
    }

    #[must_use]
    pub fn canonical_horizontal(self) -> (Self, Symmetry) {
        let mirrored = self.mirrored_horizontal();
        if mirrored < self {
            (mirrored, Symmetry::MirrorHorizontal)
        } else {
            (self, Symmetry::Identity)
        }
    }

    #[must_use]
    pub fn canonical_color_preserving(self) -> (Self, Symmetry) {
        self.canonical_horizontal()
    }

    #[must_use]
    pub fn canonical(self) -> (Self, Symmetry) {
        let mut best = self;
        let mut best_symmetry = Symmetry::Identity;
        for symmetry in Symmetry::ALL {
            let transformed = self.apply_symmetry(symmetry);
            if transformed < best {
                best = transformed;
                best_symmetry = symmetry;
            }
        }
        (best, best_symmetry)
    }

    fn generate_piece_moves(&self, player: Player, kind: PieceKind, moves: &mut Vec<Move>) {
        let mut bits = self.piece_bits(player, kind);
        while bits != 0 {
            let index = bits.trailing_zeros() as u8;
            bits &= bits - 1;
            let from = Square(index);
            for (dr, dc) in DIRECTIONS {
                if let Some(to) = from.offset(dr, dc) {
                    if self.is_empty(to) {
                        moves.push(Move {
                            from,
                            to,
                            kind: MoveKind::Simple,
                        });
                        continue;
                    }

                    if self.is_ball_at(to) {
                        if let Some(ball_to) = to.offset(dr, dc)
                            && self.is_empty(ball_to)
                            && !Self::is_forbidden_ball_destination(ball_to)
                        {
                            moves.push(Move {
                                from,
                                to,
                                kind: MoveKind::Push { ball_to },
                            });
                        }
                        if kind == PieceKind::Attacker
                            && let Some(landing) = to.offset(dr, dc)
                            && self.is_empty(landing)
                            && !self.is_forbidden_jump_reply(from, to)
                        {
                            moves.push(Move {
                                from,
                                to: landing,
                                kind: MoveKind::Jump { jumped: to },
                            });
                        }
                        continue;
                    }

                    match kind {
                        PieceKind::Attacker => {
                            if let Some(landing) = to.offset(dr, dc)
                                && self.is_empty(landing)
                                && !self.is_forbidden_jump_reply(from, to)
                            {
                                moves.push(Move {
                                    from,
                                    to: landing,
                                    kind: MoveKind::Jump { jumped: to },
                                });
                            }
                        }
                        PieceKind::Defender => {
                            if let Some(target_piece) = self.piece_at(to)
                                && target_piece.player != player
                                && let Some(pushed_to) = to.offset(dr, dc)
                                && self.is_empty(pushed_to)
                                && !self.is_forbidden_tackle_reply(from, to)
                            {
                                moves.push(Move {
                                    from,
                                    to,
                                    kind: MoveKind::Tackle {
                                        pushed_from: to,
                                        pushed_to,
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    #[must_use]
    fn piece_bits(&self, player: Player, kind: PieceKind) -> u64 {
        match (player, kind) {
            (Player::White, PieceKind::Attacker) => self.white_attackers,
            (Player::White, PieceKind::Defender) => self.white_defenders,
            (Player::Black, PieceKind::Attacker) => self.black_attackers,
            (Player::Black, PieceKind::Defender) => self.black_defenders,
        }
    }

    #[must_use]
    fn is_forbidden_jump_reply(&self, from: Square, jumped: Square) -> bool {
        matches!(
            self.last_tackle,
            Some(TackleMemory {
                defender_to,
                victim_to,
            }) if defender_to == jumped && victim_to == from
        )
    }

    #[must_use]
    fn is_forbidden_tackle_reply(&self, from: Square, to: Square) -> bool {
        matches!(
            self.last_tackle,
            Some(TackleMemory {
                defender_to,
                victim_to,
            }) if defender_to == to && victim_to == from
        )
    }

    #[must_use]
    pub(crate) const fn is_forbidden_ball_destination(square: Square) -> bool {
        (square.col() == 0 || square.col() == COLS - 1)
            && square.row() > 0
            && square.row() + 1 < ROWS
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..ROWS {
            for col in 0..COLS {
                let square = square(row, col);
                if self.ball == square {
                    write!(f, "NB")?;
                } else if let Some(piece) = self.piece_at(square) {
                    write!(f, "{}{}", piece.player.to_char(), piece.kind.to_char())?;
                } else {
                    write!(f, "--")?;
                }
                if col + 1 < COLS {
                    write!(f, " ")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[must_use]
pub const fn square(row: usize, col: usize) -> Square {
    assert!(row < ROWS && col < COLS);
    Square((row * COLS + col) as u8)
}

#[must_use]
const fn bit(square: Square) -> u64 {
    1u64 << square.0
}

fn pack_piece_slots(raw: &mut u128, bits: u64, offset: u32, slots: usize) {
    let mut bits = bits;
    let mut slot = 0usize;
    while bits != 0 {
        assert!(
            slot < slots,
            "position exceeds canonical ChessBall piece counts"
        );
        let index = bits.trailing_zeros() as u8;
        store_square_bits(raw, offset + slot as u32 * PACKED_SQUARE_BITS, index);
        bits &= bits - 1;
        slot += 1;
    }
    while slot < slots {
        store_square_bits(
            raw,
            offset + slot as u32 * PACKED_SQUARE_BITS,
            PACKED_EMPTY_SQUARE,
        );
        slot += 1;
    }
}

fn unpack_piece_slots(
    position: &mut Position,
    raw: u128,
    offset: u32,
    slots: usize,
    player: Player,
    kind: PieceKind,
) {
    for slot in 0..slots {
        let bits = load_square_bits(raw, offset + slot as u32 * PACKED_SQUARE_BITS);
        if let Some(square) = decode_optional_square(bits) {
            position.put_piece(square, Piece { player, kind });
        }
    }
}

fn store_square_bits(raw: &mut u128, offset: u32, square: u8) {
    *raw |= (square as u128) << offset;
}

#[must_use]
fn load_square_bits(raw: u128, offset: u32) -> u8 {
    ((raw >> offset) & PACKED_SQUARE_MASK) as u8
}

#[must_use]
fn decode_optional_square(bits: u8) -> Option<Square> {
    if bits == PACKED_EMPTY_SQUARE {
        None
    } else {
        Some(decode_required_square(bits))
    }
}

#[must_use]
fn decode_required_square(bits: u8) -> Square {
    assert!(
        bits < NUM_SQUARES as u8,
        "packed square index must be on the board"
    );
    Square(bits)
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::{
        COLS, MAX_MOVES_PER_POSITION, Move, MoveKind, PackedPosition, Piece, PieceKind, Player,
        Position, ROWS, Symmetry, TackleMemory, square,
    };

    #[test]
    fn new_game_matches_canonical_layout() {
        let position = Position::new_game();
        let expected = "\
-- WD -- WD -- WD --\n\
-- -- WA -- WA -- --\n\
-- -- -- NB -- -- --\n\
-- -- -- -- -- -- --\n\
-- -- BA -- BA -- --\n\
-- BD -- BD -- BD --\n";

        assert_eq!(format!("{position}"), expected);
        assert_eq!(position.ball(), square(2, 3));
        assert_eq!(position.to_move, Player::White);
        assert_eq!(position.count_pieces(Player::White, PieceKind::Attacker), 2);
        assert_eq!(position.count_pieces(Player::White, PieceKind::Defender), 3);
        assert_eq!(position.count_pieces(Player::Black, PieceKind::Attacker), 2);
        assert_eq!(position.count_pieces(Player::Black, PieceKind::Defender), 3);
    }

    #[test]
    fn max_moves_per_position_matches_piece_and_direction_limits() {
        assert_eq!(MAX_MOVES_PER_POSITION, 56);
        assert!(Position::new_game().legal_moves().len() <= MAX_MOVES_PER_POSITION);
    }

    #[test]
    fn parse_and_display_round_trip() {
        let text = "\
-- WD -- WD -- WD --\n\
-- -- WA -- WA -- --\n\
-- -- -- NB -- -- --\n\
-- -- -- -- -- -- --\n\
-- -- BA -- BA -- --\n\
-- BD -- BD -- BD --\n";
        let position = Position::from_repr(text, Player::White).unwrap();
        assert_eq!(format!("{position}"), text);
    }

    #[test]
    fn ball_push_to_touch_zone_is_illegal() {
        let mut position = Position::empty(square(2, 5), Player::White);
        position.put_piece(
            square(2, 4),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let illegal_push = Move {
            from: square(2, 4),
            to: square(2, 5),
            kind: MoveKind::Push {
                ball_to: square(2, 6),
            },
        };

        assert!(!position.legal_moves().contains(&illegal_push));
    }

    #[test]
    fn ball_push_to_outer_goal_square_is_legal() {
        let mut position = Position::empty(square(4, 5), Player::White);
        position.put_piece(
            square(3, 4),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let winning_corner_push = Move {
            from: square(3, 4),
            to: square(4, 5),
            kind: MoveKind::Push {
                ball_to: square(5, 6),
            },
        };

        assert!(position.legal_moves().contains(&winning_corner_push));
        assert_eq!(
            position.apply(winning_corner_push).winner(),
            Some(Player::White)
        );
    }

    #[test]
    fn white_goal_push_is_a_winning_move() {
        let mut position = Position::empty(square(4, 3), Player::White);
        position.put_piece(
            square(3, 3),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let winning = Move {
            from: square(3, 3),
            to: square(4, 3),
            kind: MoveKind::Push {
                ball_to: square(5, 3),
            },
        };

        assert!(position.winning_moves().contains(&winning));
        assert_eq!(position.apply(winning).winner(), Some(Player::White));
    }

    #[test]
    fn attacker_can_jump_over_friendly_piece() {
        let mut position = Position::empty(square(0, 3), Player::White);
        position.put_piece(
            square(2, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Attacker,
            },
        );
        position.put_piece(
            square(2, 3),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let jump = Move {
            from: square(2, 2),
            to: square(2, 4),
            kind: MoveKind::Jump {
                jumped: square(2, 3),
            },
        };

        assert!(position.legal_moves().contains(&jump));
    }

    #[test]
    fn attacker_can_jump_over_ball() {
        let position = Position::empty(square(2, 3), Player::White);

        let jump = Move {
            from: square(2, 2),
            to: square(2, 4),
            kind: MoveKind::Jump {
                jumped: square(2, 3),
            },
        };

        let mut position = position;
        position.put_piece(
            square(2, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Attacker,
            },
        );

        assert!(position.legal_moves().contains(&jump));
        assert_eq!(position.apply(jump).ball(), square(2, 3));
    }

    #[test]
    fn defender_tackle_pushes_opponent_piece() {
        let mut position = Position::empty(square(0, 3), Player::White);
        position.put_piece(
            square(2, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );
        position.put_piece(
            square(2, 3),
            Piece {
                player: Player::Black,
                kind: PieceKind::Attacker,
            },
        );

        let tackle = Move {
            from: square(2, 2),
            to: square(2, 3),
            kind: MoveKind::Tackle {
                pushed_from: square(2, 3),
                pushed_to: square(2, 4),
            },
        };

        let next = position.apply(tackle);
        assert_eq!(
            next.piece_at(square(2, 3)),
            Some(Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            })
        );
        assert_eq!(
            next.piece_at(square(2, 4)),
            Some(Piece {
                player: Player::Black,
                kind: PieceKind::Attacker,
            })
        );
        assert_eq!(
            next.last_tackle,
            Some(TackleMemory {
                defender_to: square(2, 3),
                victim_to: square(2, 4),
            })
        );
    }

    #[test]
    fn immediate_jump_back_after_tackle_is_blocked() {
        let mut position = Position::empty(square(0, 3), Player::White);
        position.put_piece(
            square(2, 3),
            Piece {
                player: Player::Black,
                kind: PieceKind::Defender,
            },
        );
        position.put_piece(
            square(2, 4),
            Piece {
                player: Player::White,
                kind: PieceKind::Attacker,
            },
        );
        position.last_tackle = Some(TackleMemory {
            defender_to: square(2, 3),
            victim_to: square(2, 4),
        });

        let illegal = Move {
            from: square(2, 4),
            to: square(2, 2),
            kind: MoveKind::Jump {
                jumped: square(2, 3),
            },
        };

        assert!(!position.legal_moves().contains(&illegal));
    }

    #[test]
    fn immediate_tackle_back_after_tackle_is_blocked() {
        let mut position = Position::empty(square(0, 3), Player::White);
        position.put_piece(
            square(2, 3),
            Piece {
                player: Player::Black,
                kind: PieceKind::Defender,
            },
        );
        position.put_piece(
            square(2, 4),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );
        position.last_tackle = Some(TackleMemory {
            defender_to: square(2, 3),
            victim_to: square(2, 4),
        });

        let illegal = Move {
            from: square(2, 4),
            to: square(2, 3),
            kind: MoveKind::Tackle {
                pushed_from: square(2, 3),
                pushed_to: square(2, 2),
            },
        };

        assert!(!position.legal_moves().contains(&illegal));
    }

    #[test]
    fn non_tackle_move_clears_tackle_memory() {
        let mut position = Position::empty(square(0, 3), Player::White);
        position.put_piece(
            square(2, 3),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );
        position.last_tackle = Some(TackleMemory {
            defender_to: square(1, 1),
            victim_to: square(1, 2),
        });

        let next = position.apply(Move {
            from: square(2, 3),
            to: square(3, 3),
            kind: MoveKind::Simple,
        });

        assert_eq!(next.last_tackle, None);
    }

    #[test]
    fn packed_position_round_trips_and_is_smaller_than_position() {
        let mut position = Position::new_game();
        position.last_tackle = Some(TackleMemory {
            defender_to: square(2, 3),
            victim_to: square(2, 4),
        });

        let packed = position.pack();
        assert_eq!(packed.unpack(), position);
        assert!(size_of::<PackedPosition>() < size_of::<Position>());
    }

    #[test]
    fn rotate_180_swap_colors_swaps_side_and_piece_colors() {
        let mut position = Position::empty(square(1, 2), Player::White);
        position.put_piece(
            square(2, 1),
            Piece {
                player: Player::White,
                kind: PieceKind::Attacker,
            },
        );
        position.put_piece(
            square(4, 5),
            Piece {
                player: Player::Black,
                kind: PieceKind::Defender,
            },
        );
        position.last_tackle = Some(TackleMemory {
            defender_to: square(2, 1),
            victim_to: square(2, 2),
        });

        let transformed = position.apply_symmetry(Symmetry::Rotate180SwapColors);

        assert_eq!(transformed.to_move, Player::Black);
        assert_eq!(transformed.ball(), square(4, 4));
        assert_eq!(
            transformed.piece_at(square(3, 5)),
            Some(Piece {
                player: Player::Black,
                kind: PieceKind::Attacker,
            })
        );
        assert_eq!(
            transformed.piece_at(square(1, 1)),
            Some(Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            })
        );
        assert_eq!(
            transformed.last_tackle,
            Some(TackleMemory {
                defender_to: square(3, 5),
                victim_to: square(3, 4),
            })
        );
    }

    #[test]
    fn symmetries_are_involutions_on_positions() {
        let mut position = Position::new_game();
        position.last_tackle = Some(TackleMemory {
            defender_to: square(2, 3),
            victim_to: square(2, 4),
        });

        for symmetry in Symmetry::ALL {
            assert_eq!(
                position.apply_symmetry(symmetry).apply_symmetry(symmetry),
                position
            );
        }
    }

    #[test]
    fn geometry_constants_match_canonical_board() {
        assert_eq!(ROWS, 6);
        assert_eq!(COLS, 7);
        assert_eq!(super::NUM_SQUARES, 42);
    }
}
