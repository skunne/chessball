use crate::board::ChessBallBoard;
use crate::board::Player;
use crate::moves::{possible_moves, MoveInfo};

pub fn winning_moves(position: &ChessBallBoard, player: Player) -> Vec<MoveInfo> {
    let winner_row = match player {
        Player::Black => 0usize,
        Player::White => position.rows - 1,
        Player::Neutral => position.rows - 1, // neutral not used
    };
    let mut wins = Vec::new();
    for (mv, board_after) in possible_moves(position, player) {
        if let Some((br, _bc)) = board_after.find_ball() {
            if br == winner_row {
                wins.push(mv);
            }
        }
    }
    wins
}