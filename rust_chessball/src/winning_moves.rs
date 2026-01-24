//! Winning move detection.

use crate::board::ChessBallBoard;
use crate::board::Player;
use crate::moves::{MoveInfo, possible_moves};

/// Return the list of moves for `player` that result in the ball being in the player's goal row.
///
/// For Black the winning row is 0; for White it's rows-1.
pub fn winning_moves(position: &ChessBallBoard, player: Player) -> Vec<MoveInfo> {
    let winner_row = match player {
        Player::Black => 0usize,
        Player::White => position.rows - 1,
        Player::Neutral => position.rows - 1, // neutral not used
    };
    let mut wins = Vec::new();
    for (mv, board_after) in possible_moves(position, player) {
        if let Some(ball_coord) = board_after.find_ball()
            && ball_coord.r == winner_row as isize
        {
            wins.push(mv);
        }
    }
    wins
}
