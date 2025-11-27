use crate::board::{ChessBallBoard, Player};
use crate::moves::{possible_moves, MoveInfo};
use crate::winning_moves::winning_moves;

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