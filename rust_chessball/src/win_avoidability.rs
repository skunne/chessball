//! Check whether a win was avoidable by the opponent by examining possible previous positions.

use crate::blocking_move::find_blocking_move;
use crate::board::{ChessBallBoard, Player};
use crate::moves::possible_previous_moves;

/// Given a `position` in which `player` has a winning move, checks whether in ALL possible previous
/// positions there existed a blocking move for the opponent that prevented the win.
///
/// Returns `true` if the opponent could have always blocked (i.e., the win was avoidable),
/// and `false` if there exists a previous position that made the win inevitable.
pub fn is_win_avoidable_by_opponent(position: &ChessBallBoard, player: Player) -> bool {
    let opponent = match player {
        Player::White => Player::Black,
        Player::Black => Player::White,
        Player::Neutral => Player::Neutral,
    };
    let previous_positions = possible_previous_moves(position, opponent);
    if previous_positions.is_empty() {
        return false;
    }
    for (_mv, prev_board) in previous_positions {
        let blocking = find_blocking_move(&prev_board, opponent);
        if blocking.is_none() {
            return false;
        }
    }
    true
}
