//! Move generation and retrograde move generation.
//!
//! Provides `possible_moves` and `possible_previous_moves` analogues of the Python code.
//! Moves are represented by MoveInfo; generators return Vec<(MoveInfo, ChessBallBoard)> for simplicity.

use crate::board::{ChessBallBoard, Coord, DIRECTIONS, Piece, PieceType};
use crate::board::{CoordDelta, Player};
use std::clone::Clone;
use std::fmt;

/// Struct describing a move
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveInfo {
    pub from: Coord,
    pub to: Coord,
    pub special: MoveSpecialInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveSpecialInfo {
    SimpleMove,
    BallPush { ball_to: Coord },
    AttackerJump { jumped_over: Coord },
    DefenderTackle(DefenderTackle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefenderTackle {
    pub pushed_piece_from: Coord,
    pub pushed_piece_to: Coord,
}

impl MoveInfo {
    /// Simple adjacent move info helper.
    pub fn simple(from: Coord, to: Coord) -> Self {
        Self {
            from,
            to,
            special: MoveSpecialInfo::SimpleMove,
        }
    }
}

impl fmt::Display for MoveInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{} -> {}{}",
            self.from.r, self.from.c, self.to.r, self.to.c
        )?;
        Ok(())
    }
}

/// Generate all legal moves for `player` from `board`.
///
/// Returns a Vec of (MoveInfo, resulting_board).
///
/// This version scans the board once and invokes lightweight per-piece-per-direction
/// helpers to classify and generate moves. This avoids multiple full-board scans.
pub fn possible_moves(board: &ChessBallBoard, player: Player) -> Vec<(MoveInfo, ChessBallBoard)> {
    let mut results = Vec::new();
    for coord in board.iter_coords() {
        if let Some(piece) = board.get_piece(coord).cloned() {
            if piece.player != player {
                continue;
            }
            for &delta in DIRECTIONS.iter() {
                // Always attempt simple moves and ball pushes
                gen_simple_move_for(board, player, coord, &piece, delta, &mut results);
                gen_ball_push_move_for(board, player, coord, &piece, delta, &mut results);
                // Specialized moves: attacker jump, defender tackle
                gen_attacker_jump_move_for(board, player, coord, &piece, delta, &mut results);
                gen_defender_tackle_move_for(board, player, coord, &piece, delta, &mut results);
            }
        }
    }
    results
}

/// Generate a simple adjacent move for a single piece in one direction.
///
/// This is a per-piece-per-direction helper used by `possible_moves`.
///
/// - board: current board
/// - player: moving player
/// - r,c: coordinates of the moving piece
/// - piece: the piece at (r,c) (borrowed)
/// - dr,dc: direction to attempt
/// - results: append generated moves here
fn gen_simple_move_for(
    board: &ChessBallBoard,
    _player: Player,
    from: Coord,
    piece: &Piece,
    delta: CoordDelta,
    results: &mut Vec<(MoveInfo, ChessBallBoard)>,
) {
    if let Some(to) = from + delta
        && board.get_piece(to).is_none()
    {
        let mut newb = board.clone();
        newb.prev_tackle = None;
        newb.remove_piece(from);
        newb.place_piece(to, piece.clone());
        results.push((MoveInfo::simple(from, to), newb));
    }
}

/// Generate a ball-push move for a single piece in one direction.
///
/// This is a per-piece-per-direction helper used by `possible_moves`.
fn gen_ball_push_move_for(
    board: &ChessBallBoard,
    player: Player,
    from: Coord,
    piece: &Piece,
    delta: CoordDelta,
    results: &mut Vec<(MoveInfo, ChessBallBoard)>,
) {
    assert!(piece.player == player);
    if let Some(ball_coord) = from + delta
        && let Some(ball_dest) = ball_coord + delta
        && board.get_piece(ball_coord)
            == Some(&Piece {
                piece_type: PieceType::Ball,
                player: Player::Neutral,
            })
        && board.get_piece(ball_dest).is_none()
    {
        let mut new_board = board.clone();
        new_board.prev_tackle = None;
        new_board.place_ball(ball_dest);
        new_board.place_piece(ball_coord, piece.clone());
        new_board.remove_piece(from);
        let info = MoveInfo {
            from,
            to: ball_coord,
            special: MoveSpecialInfo::BallPush { ball_to: ball_dest },
        };
        results.push((info, new_board));
    }
}

/// Generate an attacker jump move for a single piece in one direction.
///
/// This is a per-piece-per-direction helper used by `possible_moves`.
fn gen_attacker_jump_move_for(
    board: &ChessBallBoard,
    _player: Player,
    from: Coord,
    piece: &Piece,
    delta: CoordDelta,
    results: &mut Vec<(MoveInfo, ChessBallBoard)>,
) {
    // Only attackers can jump
    if piece.piece_type != PieceType::Attacker {
        return;
    }
    if let Some(jumped_over_coord) = from + delta
        && let Some(destination) = jumped_over_coord + delta
    {
        if let Some(prev_tackle) = &board.prev_tackle
            && (prev_tackle.pushed_piece_from, prev_tackle.pushed_piece_to)
                == (jumped_over_coord, from)
        {
            // Not allowed to jump over defender who tackled us in previous turn
            return;
        }
        if board.is_on_board(destination)
            && let Some(jumped_piece) = board.get_piece(jumped_over_coord)
            && board.get_piece(destination).is_none()
            && jumped_piece.piece_type != PieceType::Ball
        {
            let mut newb = board.clone();
            newb.prev_tackle = None;
            newb.remove_piece(from);
            newb.place_piece(destination, piece.clone());
            let info = MoveInfo {
                from,
                to: destination,
                special: MoveSpecialInfo::AttackerJump {
                    jumped_over: jumped_over_coord,
                },
            };
            results.push((info, newb));
        }
    }
}

/// Generate a defender tackle move for a single piece in one direction.
///
/// This is a per-piece-per-direction helper used by `possible_moves`.
fn gen_defender_tackle_move_for(
    board: &ChessBallBoard,
    player: Player,
    from: Coord,
    piece: &Piece,
    delta: CoordDelta,
    results: &mut Vec<(MoveInfo, ChessBallBoard)>,
) {
    // Only defenders can tackle
    if piece.piece_type != PieceType::Defender {
        return;
    }
    if let Some(to) = from + delta
        && let Some(pushed_to) = to + delta
    {
        if let Some(prev_tackle) = &board.prev_tackle
            && (prev_tackle.pushed_piece_from, prev_tackle.pushed_piece_to) == (to, from)
        {
            // Not allowed to tackle defender who tackled us in previous turn
            return;
        }
        if board.is_on_board(pushed_to)
            && let Some(pushed_piece) = board.get_piece(to)
            && board.get_piece(pushed_to).is_none()
            && pushed_piece.player != player
            && pushed_piece.piece_type != PieceType::Ball
        {
            let mut newb = board.clone();
            // push opponent to beyond
            newb.remove_piece(to);
            newb.place_piece(pushed_to, pushed_piece.clone());
            // move own piece to freed position
            newb.remove_piece(from);
            newb.place_piece(to, piece.clone());
            let tackle = DefenderTackle {
                pushed_piece_from: to,
                pushed_piece_to: pushed_to,
            };
            let info = MoveInfo {
                from,
                to,
                special: MoveSpecialInfo::DefenderTackle(tackle.clone()),
            };
            newb.prev_tackle = Some(tackle);
            results.push((info, newb));
        }
    }
}

/// Generate candidate previous moves (retrograde) for `player` that could have led to `board`
pub fn possible_previous_moves(
    board: &ChessBallBoard,
    player: Player,
) -> Vec<(MoveInfo, ChessBallBoard)> {
    // Conservative retrograde support: reconstruct simple one-step predecessor
    // positions only. This keeps the helper useful for diagnostics without
    // inventing speculative reverse push/jump/tackle states.
    let mut prevs = Vec::new();

    for to in board.iter_coords() {
        let Some(piece) = board.get_piece(to).cloned() else {
            continue;
        };
        if piece.player != player || piece.piece_type == PieceType::Ball {
            continue;
        }

        for &delta in DIRECTIONS.iter() {
            let Some(from) = to - delta else {
                continue;
            };
            if board.get_piece(from).is_some() {
                continue;
            }

            let mut prev_board = board.clone();
            prev_board.prev_tackle = None;
            prev_board.remove_piece(to);
            prev_board.place_piece(from, piece.clone());
            prevs.push((MoveInfo::simple(from, to), prev_board));
        }
    }

    prevs
}

#[cfg(test)]
mod tests {
    use crate::{
        board::{ChessBallBoard, Coord, CoordDelta, DIRECTIONS, Piece, PieceType, Player},
        moves::{DefenderTackle, MoveSpecialInfo, possible_moves, possible_previous_moves},
    };

    // Tests for the top-level behavior (unchanged semantics)

    #[test]
    fn test_possible_moves_push_move() {
        let mut b = ChessBallBoard::new();
        b.place_piece(
            Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        b.place_ball(Coord { r: 2, c: 4 });
        let found_push =
            possible_moves(&b, Player::White)
                .iter()
                .any(|(info, _newboard)| match info.special {
                    MoveSpecialInfo::BallPush { ball_to: _ } => true,
                    _ => false,
                });
        assert!(found_push);
    }

    #[test]
    fn test_possible_moves_simple_moves() {
        let mut b = ChessBallBoard::new();
        b.place_piece(
            Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        let moves = possible_moves(&b, Player::White);
        assert!(!moves.is_empty());
    }

    #[test]
    fn test_possible_previous_moves() {
        let mut b = ChessBallBoard::new();
        b.place_ball(Coord { r: 2, c: 4 });
        b.place_piece(
            Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        let prevs = possible_previous_moves(&b, Player::White);
        assert!(!prevs.is_empty());
    }

    // Unit tests for each helper via direct invocation (helpers are in the parent module).
    // We call them through `super::` because they are private to the module.

    #[test]
    fn test_gen_simple_move_for() {
        let mut b = ChessBallBoard::new();
        b.place_piece(
            Coord { r: 2, c: 2 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        let mut results = Vec::new();
        // try all directions; expect at least one simple move
        for &delta in DIRECTIONS.iter() {
            super::gen_simple_move_for(
                &b,
                Player::White,
                Coord { r: 2, c: 2 },
                &b.get_piece(Coord { r: 2, c: 2 }).unwrap(),
                delta,
                &mut results,
            );
        }
        assert!(!results.is_empty(), "Expected at least one simple move");
        // ensure none of the generated moves have special flags
        assert!(
            results
                .iter()
                .all(|(m, _)| m.special == MoveSpecialInfo::SimpleMove)
        );
    }

    #[test]
    fn test_gen_ball_push_move_for() {
        let mut b = ChessBallBoard::new();
        b.place_piece(
            Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        b.place_ball(Coord { r: 2, c: 4 });
        // ensure (2,5) is empty and not forbidden (board.new() uses standard columns)
        let mut results = Vec::new();
        // only the rightward direction should produce a push
        super::gen_ball_push_move_for(
            &b,
            Player::White,
            Coord { r: 2, c: 3 },
            &b.get_piece(Coord { r: 2, c: 3 }).unwrap(),
            CoordDelta {
                r: 0,
                c: 1,
                rows: b.rows,
                cols: b.cols,
            },
            &mut results,
        );
        println!("{}", b);
        assert!(
            results.iter().any(|(info, _)| match info.special {
                MoveSpecialInfo::BallPush { ball_to: _ } => true,
                _ => false,
            }),
            "Expected a ball-push move"
        );
    }

    #[test]
    fn test_gen_attacker_jump_move_for() {
        let mut b = ChessBallBoard::new();
        // place attacker at (2,2), opponent piece (non-ball) at (2,3) and empty (2,4)
        b.place_piece(
            Coord { r: 2, c: 2 },
            Piece {
                piece_type: PieceType::Attacker,
                player: Player::White,
            },
        );
        b.place_piece(
            Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::Black,
            },
        );
        let mut results = Vec::new();
        super::gen_attacker_jump_move_for(
            &b,
            Player::White,
            Coord { r: 2, c: 2 },
            &b.get_piece(Coord { r: 2, c: 2 }).unwrap(),
            CoordDelta {
                r: 0,
                c: 1,
                rows: b.rows,
                cols: b.cols,
            },
            &mut results,
        );
        assert!(results.iter().any(|(info, _)| info.special
            == MoveSpecialInfo::AttackerJump {
                jumped_over: Coord { r: 2, c: 3 },
            }));
    }

    #[test]
    fn test_gen_defender_tackle_move_for() {
        let mut b = ChessBallBoard::new();
        // place defender at (2,2), opponent piece at (2,3), empty at (2,4)
        b.place_piece(
            Coord { r: 2, c: 2 },
            Piece {
                piece_type: PieceType::Defender,
                player: Player::White,
            },
        );
        b.place_piece(
            Coord { r: 2, c: 3 },
            Piece {
                piece_type: PieceType::Attacker,
                player: Player::Black,
            },
        );
        let mut results = Vec::new();
        super::gen_defender_tackle_move_for(
            &b,
            Player::White,
            Coord { r: 2, c: 2 },
            &b.get_piece(Coord { r: 2, c: 2 }).unwrap(),
            CoordDelta {
                r: 0,
                c: 1,
                rows: b.rows,
                cols: b.cols,
            },
            &mut results,
        );
        assert!(results.iter().any(|(m, _)| m.special
            == MoveSpecialInfo::DefenderTackle(DefenderTackle {
                pushed_piece_from: Coord { r: 2, c: 3 },
                pushed_piece_to: Coord { r: 2, c: 4 },
            })));
    }
}
