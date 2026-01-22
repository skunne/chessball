//! Blocking-move search: find a move that stops the opponent having an immediate winning reply.

use crate::board::{ChessBallBoard, Player};
use crate::moves::{MoveInfo, possible_moves};
use crate::winning_moves::winning_moves;

/// Find and return a move for `player` such that after this move the opponent does not have any immediate winning moves.
///
/// Returns None if no such blocking move exists.
pub fn find_blocking_move(position: &ChessBallBoard, player: Player) -> Option<MoveInfo> {
    let opponent = match player {
        Player::White => Player::Black,
        Player::Black => Player::White,
        Player::Neutral => Player::Neutral,
    };
    for (mv, board_after) in possible_moves(position, player) {
        let opponent_win_moves = winning_moves(&board_after, opponent);
        if opponent_win_moves.is_empty() {
            return Some(mv);
        }
    }
    None
}
