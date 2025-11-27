use crate::board::{ChessBallBoard, Player};
use crate::moves::possible_moves;
use crate::heuristics::{feature_vector};
use crate::winning_moves::winning_moves;
use std::f64;

pub fn has_immediate_win(board: &ChessBallBoard, player: Player) -> Option<(crate::moves::MoveInfo, ChessBallBoard)> {
    let wins = winning_moves(board, player);
    if wins.is_empty() {
        return None;
    }
    let mut pm = possible_moves(board, player);
    // find first move that is in wins set by comparing from/to
    for (mv, b2) in pm.drain(..) {
        // find if this move results in a goal row (we can check the board)
        if let Some((br, _)) = b2.find_ball() {
            let winner_row = if player == Player::Black { 0usize } else { b2.rows - 1 };
            if br == winner_row {
                return Some((mv, b2));
            }
        }
    }
    None
}

pub fn choose_best_move(board: &ChessBallBoard, player: Player, depth: usize) -> (Option<crate::moves::MoveInfo>, Option<ChessBallBoard>, f64) {
    let opponent = match player { Player::White => Player::Black, Player::Black => Player::White, Player::Neutral => Player::Neutral };
    if let Some((mv, b2)) = has_immediate_win(board, player) {
        return (Some(mv), Some(b2), f64::INFINITY);
    }
    if let Some((_mv, _b2)) = has_immediate_win(board, opponent) {
        return (None, None, f64::NEG_INFINITY);
    }

    fn minimax(node_board: &ChessBallBoard, to_move: Player, ply: usize, maximizing: bool, root_player: Player) -> (f64, Option<crate::moves::MoveInfo>, Option<ChessBallBoard>) {
        // immediate win check
        if let Some((mv, board_after)) = has_immediate_win(node_board, to_move) {
            let score = if maximizing { f64::INFINITY } else { f64::NEG_INFINITY };
            return (score, Some(mv), Some(board_after));
        }
        let other = match to_move { Player::White => Player::Black, Player::Black => Player::White, Player::Neutral => Player::Neutral };
        if has_immediate_win(node_board, other).is_some() {
            let score = if maximizing { f64::NEG_INFINITY } else { f64::INFINITY };
            return (score, None, None);
        }
        if ply == 0 {
            // static evaluation using heuristic features (simple linear combination not provided here)
            // Use simple heuristic: evaluate feature sum as proxy
            let feats = feature_vector(node_board, root_player);
            let s: f64 = feats.values().sum();
            return (s, None, None);
        }
        let moves = possible_moves(node_board, to_move);
        if moves.is_empty() {
            let feats = feature_vector(node_board, root_player);
            let s: f64 = feats.values().sum();
            return (s, None, None);
        }
        if maximizing {
            let mut best = f64::NEG_INFINITY;
            let mut best_move = None;
            let mut best_board = None;
            for (mv, b_after) in moves {
                let (score, _, _) = minimax(&b_after, other, ply - 1, false, root_player);
                if score > best {
                    best = score;
                    best_move = Some(mv);
                    best_board = Some(b_after);
                }
            }
            (best, best_move, best_board)
        } else {
            let mut best = f64::INFINITY;
            let mut best_move = None;
            let mut best_board = None;
            for (mv, b_after) in moves {
                let (score, _, _) = minimax(&b_after, other, ply - 1, true, root_player);
                if score < best {
                    best = score;
                    best_move = Some(mv);
                    best_board = Some(b_after);
                }
            }
            (best, best_move, best_board)
        }
    }

    let (score, best_move, best_board) = minimax(board, player, depth, true, player);
    (best_move, best_board, score)
}