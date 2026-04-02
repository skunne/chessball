use std::fmt;

use crate::engine::{COLS, Move, MoveKind, Player, Position, ROWS, Square, square};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveSource {
    Search,
    Mcts,
    Opening,
}

impl MoveSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Mcts => "mcts",
            Self::Opening => "opening",
        }
    }

    fn parse(input: &str) -> Result<Self, String> {
        match input {
            "search" => Ok(Self::Search),
            "mcts" => Ok(Self::Mcts),
            "opening" => Ok(Self::Opening),
            other => Err(format!("unknown move source '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOutcome {
    WhiteWin,
    BlackWin,
    Draw,
}

impl GameOutcome {
    #[must_use]
    pub const fn as_result_str(self) -> &'static str {
        match self {
            Self::WhiteWin => "1-0",
            Self::BlackWin => "0-1",
            Self::Draw => "1/2-1/2",
        }
    }

    fn parse(input: &str) -> Result<Self, String> {
        match input {
            "1-0" => Ok(Self::WhiteWin),
            "0-1" => Ok(Self::BlackWin),
            "1/2-1/2" => Ok(Self::Draw),
            other => Err(format!("unknown result '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    GoalRow,
    ThreefoldRepetition,
    PlyLimit,
    NoLegalMoves,
}

impl Termination {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GoalRow => "goal-row",
            Self::ThreefoldRepetition => "threefold-repetition",
            Self::PlyLimit => "ply-limit",
            Self::NoLegalMoves => "no-legal-moves",
        }
    }

    fn parse(input: &str) -> Result<Self, String> {
        match input {
            "goal-row" => Ok(Self::GoalRow),
            "threefold-repetition" => Ok(Self::ThreefoldRepetition),
            "ply-limit" => Ok(Self::PlyLimit),
            "no-legal-moves" => Ok(Self::NoLegalMoves),
            other => Err(format!("unknown termination '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlyRecord {
    pub player: Player,
    pub mv: Move,
    pub source: MoveSource,
    pub score: Option<i32>,
    pub nodes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    pub initial_position: Position,
    pub white_label: String,
    pub black_label: String,
    pub moves: Vec<PlyRecord>,
    pub outcome: GameOutcome,
    pub termination: Termination,
    pub final_position: Position,
}

impl GameRecord {
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("CBR1\n");
        out.push_str(&format!("White: {}\n", self.white_label));
        out.push_str(&format!("Black: {}\n", self.black_label));
        out.push_str(&format!(
            "Initial-To-Move: {}\n",
            self.initial_position.to_move.to_char()
        ));
        out.push_str("Initial-Board:\n");
        out.push_str(&format!("{}", self.initial_position));
        out.push_str("Moves:\n");
        for (idx, ply) in self.moves.iter().enumerate() {
            out.push_str(&format!(
                "{}. {} {} source={}",
                idx + 1,
                ply.player.to_char(),
                move_to_notation(ply.mv),
                ply.source.as_str()
            ));
            if let Some(score) = ply.score {
                out.push_str(&format!(" score={score}"));
            }
            if let Some(nodes) = ply.nodes {
                out.push_str(&format!(" nodes={nodes}"));
            }
            out.push('\n');
        }
        out.push_str(&format!("Result: {}\n", self.outcome.as_result_str()));
        out.push_str(&format!("Termination: {}\n", self.termination.as_str()));
        out.push_str("Final-Board:\n");
        out.push_str(&format!("{}", self.final_position));
        out
    }

    pub fn from_text(input: &str) -> Result<Self, String> {
        let lines: Vec<&str> = input.lines().collect();
        let mut idx = 0usize;

        if next_line(&lines, &mut idx, "CBR1")? != "CBR1" {
            return Err("record must start with CBR1".to_string());
        }
        let white_label = parse_prefixed(next_line(&lines, &mut idx, "White:")?, "White: ")?;
        let black_label = parse_prefixed(next_line(&lines, &mut idx, "Black:")?, "Black: ")?;
        let to_move_line = parse_prefixed(
            next_line(&lines, &mut idx, "Initial-To-Move:")?,
            "Initial-To-Move: ",
        )?;
        let initial_to_move = parse_player_char(to_move_line)?;
        if next_line(&lines, &mut idx, "Initial-Board:")? != "Initial-Board:" {
            return Err("expected Initial-Board:".to_string());
        }
        let initial_board = take_board_block(&lines, &mut idx)?;
        if next_line(&lines, &mut idx, "Moves:")? != "Moves:" {
            return Err("expected Moves:".to_string());
        }

        let mut moves = Vec::new();
        while let Some(line) = lines.get(idx).copied() {
            if line.starts_with("Result: ") {
                break;
            }
            if line.trim().is_empty() {
                idx += 1;
                continue;
            }
            moves.push(parse_move_line(line)?);
            idx += 1;
        }

        let outcome = GameOutcome::parse(parse_prefixed(
            next_line(&lines, &mut idx, "Result:")?,
            "Result: ",
        )?)?;
        let termination = Termination::parse(parse_prefixed(
            next_line(&lines, &mut idx, "Termination:")?,
            "Termination: ",
        )?)?;
        if next_line(&lines, &mut idx, "Final-Board:")? != "Final-Board:" {
            return Err("expected Final-Board:".to_string());
        }
        let final_board = take_board_block(&lines, &mut idx)?;

        let initial_position = Position::from_repr(&initial_board, initial_to_move)?;
        let replayed = replay_moves(&initial_position, &moves)?;
        let expected_final = Position::from_repr(&final_board, replayed.to_move)?;
        if replayed != expected_final {
            return Err("final board does not match replayed moves".to_string());
        }
        let inferred_outcome = match replayed.winner() {
            Some(Player::White) => GameOutcome::WhiteWin,
            Some(Player::Black) => GameOutcome::BlackWin,
            None => GameOutcome::Draw,
        };
        if termination == Termination::GoalRow && inferred_outcome != outcome {
            return Err("goal-row result does not match final position".to_string());
        }

        Ok(Self {
            initial_position,
            white_label: white_label.to_string(),
            black_label: black_label.to_string(),
            moves,
            outcome,
            termination,
            final_position: replayed,
        })
    }

    pub fn replay_positions(&self) -> Result<Vec<Position>, String> {
        let mut states = vec![self.initial_position];
        let mut position = self.initial_position;
        for ply in &self.moves {
            if ply.player != position.to_move {
                return Err(format!(
                    "move player {} does not match side to move {}",
                    ply.player.to_char(),
                    position.to_move.to_char()
                ));
            }
            if !position.legal_moves().contains(&ply.mv) {
                return Err(format!(
                    "illegal move in record: {}",
                    move_to_notation(ply.mv)
                ));
            }
            position = position.apply(ply.mv);
            states.push(position);
        }
        Ok(states)
    }
}

#[must_use]
pub fn move_to_notation(mv: Move) -> String {
    let mut out = format!(
        "{}-{}",
        square_to_algebraic(mv.from),
        square_to_algebraic(mv.to)
    );
    match mv.kind {
        MoveKind::Simple => {}
        MoveKind::Push { ball_to } => {
            out.push('@');
            out.push_str(&square_to_algebraic(ball_to));
        }
        MoveKind::Jump { jumped } => {
            out.push('^');
            out.push_str(&square_to_algebraic(jumped));
        }
        MoveKind::Tackle { pushed_to, .. } => {
            out.push('!');
            out.push_str(&square_to_algebraic(pushed_to));
        }
    }
    out
}

pub fn move_from_notation(input: &str) -> Result<Move, String> {
    let (body, suffix_kind, suffix_square) = if let Some((body, suffix)) = input.split_once('@') {
        (body, Some('@'), Some(parse_square_algebraic(suffix)?))
    } else if let Some((body, suffix)) = input.split_once('^') {
        (body, Some('^'), Some(parse_square_algebraic(suffix)?))
    } else if let Some((body, suffix)) = input.split_once('!') {
        (body, Some('!'), Some(parse_square_algebraic(suffix)?))
    } else {
        (input, None, None)
    };
    let (from_raw, to_raw) = body
        .split_once('-')
        .ok_or_else(|| format!("invalid move notation '{input}'"))?;
    let from = parse_square_algebraic(from_raw)?;
    let to = parse_square_algebraic(to_raw)?;
    let kind = match (suffix_kind, suffix_square) {
        (None, None) => MoveKind::Simple,
        (Some('@'), Some(ball_to)) => MoveKind::Push { ball_to },
        (Some('^'), Some(jumped)) => MoveKind::Jump { jumped },
        (Some('!'), Some(pushed_to)) => MoveKind::Tackle {
            pushed_from: to,
            pushed_to,
        },
        _ => return Err(format!("invalid move notation '{input}'")),
    };
    Ok(Move { from, to, kind })
}

#[must_use]
pub fn square_to_algebraic(square: Square) -> String {
    let file = (b'a' + square.col() as u8) as char;
    let rank = ROWS - square.row();
    format!("{file}{rank}")
}

pub fn parse_square_algebraic(input: &str) -> Result<Square, String> {
    if input.len() != 2 {
        return Err(format!("invalid square '{input}'"));
    }
    let mut chars = input.chars();
    let file = chars.next().unwrap().to_ascii_lowercase();
    let rank = chars.next().unwrap();
    if !(('a'..='g').contains(&file)) {
        return Err(format!("invalid file in square '{input}'"));
    }
    let rank = rank
        .to_digit(10)
        .ok_or_else(|| format!("invalid rank in square '{input}'"))? as usize;
    if !(1..=ROWS).contains(&rank) {
        return Err(format!("invalid rank in square '{input}'"));
    }
    let col = (file as u8 - b'a') as usize;
    let row = ROWS - rank;
    Ok(square(row, col))
}

fn parse_prefixed<'a>(line: &'a str, prefix: &str) -> Result<&'a str, String> {
    line.strip_prefix(prefix)
        .ok_or_else(|| format!("expected prefix '{prefix}' in line '{line}'"))
}

fn next_line<'a>(lines: &[&'a str], idx: &mut usize, expected: &str) -> Result<&'a str, String> {
    let line = lines
        .get(*idx)
        .copied()
        .ok_or_else(|| format!("expected line starting with '{expected}'"))?;
    *idx += 1;
    Ok(line)
}

fn parse_player_char(input: &str) -> Result<Player, String> {
    match input {
        "W" => Ok(Player::White),
        "B" => Ok(Player::Black),
        other => Err(format!("invalid player '{other}'")),
    }
}

fn take_board_block(lines: &[&str], idx: &mut usize) -> Result<String, String> {
    let mut board = String::new();
    for _ in 0..ROWS {
        let line = lines
            .get(*idx)
            .copied()
            .ok_or_else(|| "record ended while reading board".to_string())?;
        if line.split_whitespace().count() != COLS {
            return Err(format!("invalid board row '{line}'"));
        }
        board.push_str(line);
        board.push('\n');
        *idx += 1;
    }
    Ok(board)
}

fn parse_move_line(line: &str) -> Result<PlyRecord, String> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 4 {
        return Err(format!("invalid move line '{line}'"));
    }
    if !tokens[0].ends_with('.') {
        return Err(format!("missing move index in '{line}'"));
    }
    let player = parse_player_char(tokens[1])?;
    let mv = move_from_notation(tokens[2])?;
    let mut source = None;
    let mut score = None;
    let mut nodes = None;
    for token in &tokens[3..] {
        if let Some(value) = token.strip_prefix("source=") {
            source = Some(MoveSource::parse(value)?);
        } else if let Some(value) = token.strip_prefix("score=") {
            score = Some(
                value
                    .parse::<i32>()
                    .map_err(|_| format!("invalid score token '{token}'"))?,
            );
        } else if let Some(value) = token.strip_prefix("nodes=") {
            nodes = Some(
                value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid nodes token '{token}'"))?,
            );
        }
    }
    Ok(PlyRecord {
        player,
        mv,
        source: source.ok_or_else(|| format!("missing source in '{line}'"))?,
        score,
        nodes,
    })
}

fn replay_moves(initial: &Position, moves: &[PlyRecord]) -> Result<Position, String> {
    let mut position = *initial;
    for ply in moves {
        if ply.player != position.to_move {
            return Err(format!(
                "move player {} does not match side to move {}",
                ply.player.to_char(),
                position.to_move.to_char()
            ));
        }
        if !position.legal_moves().contains(&ply.mv) {
            return Err(format!(
                "illegal move in record: {}",
                move_to_notation(ply.mv)
            ));
        }
        position = position.apply(ply.mv);
    }
    Ok(position)
}

impl fmt::Display for GameOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_result_str())
    }
}

impl fmt::Display for Termination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{Move, MoveKind, Piece, PieceKind, Player, Position, square};

    use super::{
        GameOutcome, GameRecord, MoveSource, PlyRecord, Termination, move_from_notation,
        move_to_notation, parse_square_algebraic, square_to_algebraic,
    };

    #[test]
    fn square_algebraic_round_trip() {
        let sq = square(5, 0);
        assert_eq!(square_to_algebraic(sq), "a1");
        assert_eq!(parse_square_algebraic("a1").unwrap(), sq);
        assert_eq!(parse_square_algebraic("g6").unwrap(), square(0, 6));
    }

    #[test]
    fn move_notation_round_trip_for_all_move_kinds() {
        let moves = [
            Move {
                from: square(4, 2),
                to: square(3, 2),
                kind: MoveKind::Simple,
            },
            Move {
                from: square(3, 3),
                to: square(4, 3),
                kind: MoveKind::Push {
                    ball_to: square(5, 3),
                },
            },
            Move {
                from: square(2, 2),
                to: square(2, 4),
                kind: MoveKind::Jump {
                    jumped: square(2, 3),
                },
            },
            Move {
                from: square(2, 2),
                to: square(2, 3),
                kind: MoveKind::Tackle {
                    pushed_from: square(2, 3),
                    pushed_to: square(2, 4),
                },
            },
        ];

        for mv in moves {
            let notation = move_to_notation(mv);
            assert_eq!(move_from_notation(&notation).unwrap(), mv);
        }
    }

    #[test]
    fn game_record_round_trip_and_replay() {
        let mut initial = Position::empty(square(4, 3), Player::White);
        initial.put_piece(
            square(3, 3),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );
        let winning_move = Move {
            from: square(3, 3),
            to: square(4, 3),
            kind: MoveKind::Push {
                ball_to: square(5, 3),
            },
        };
        let final_position = initial.apply(winning_move);
        let record = GameRecord {
            initial_position: initial,
            white_label: "solver(depth=3)".to_string(),
            black_label: "solver(depth=3)".to_string(),
            moves: vec![PlyRecord {
                player: Player::White,
                mv: winning_move,
                source: MoveSource::Search,
                score: Some(10_000_000),
                nodes: Some(42),
            }],
            outcome: GameOutcome::WhiteWin,
            termination: Termination::GoalRow,
            final_position,
        };

        let text = record.to_text();
        let parsed = GameRecord::from_text(&text).unwrap();
        assert_eq!(parsed, record);
        let positions = parsed.replay_positions().unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(*positions.last().unwrap(), final_position);
    }
}
