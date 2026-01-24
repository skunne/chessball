//! Heuristic feature extraction and simple evaluation helpers.
//!
//! Provides many of the same diagnostic features as the Python version.

use crate::board::{ChessBallBoard, Coord, DIRECTIONS, Piece, PieceType, Player};
use crate::moves::possible_moves;
use crate::win_avoidability::is_win_avoidable_by_opponent;
use crate::winning_moves::winning_moves;
use std::collections::HashMap;

/// Return the ball position if present.
pub fn ball_pos(board: &ChessBallBoard) -> Option<Coord> {
    board.find_ball()
}

/// Count how many of `player`'s pieces are adjacent to the ball and could push it (destination is in-bounds, empty and not a forbidden column).
pub fn count_adjacent_pushers(board: &ChessBallBoard, player: Player) -> usize {
    if let Some(ball_coord) = board.find_ball() {
        let mut count = 0usize;
        for delta in DIRECTIONS.iter() {
            let pusher_coord = ball_coord.clone() - delta.clone();
            let ball_destination = ball_coord.clone() + delta.clone();
            if !board.is_on_board(&pusher_coord)
                || !board.is_on_board(&ball_destination)
                || board.is_forbidden_col(&ball_destination)
            {
                continue;
            }
            if let Some(pusher) = board.get_piece(&pusher_coord)
                && pusher.player == player
                && board.get_piece(&ball_destination).is_none()
            {
                count += 1;
            }
        }
        return count;
    }
    0
}

/// Count friendly and enemy adjacent pieces around the ball.
pub fn count_control_around_ball(board: &ChessBallBoard, player: Player) -> (usize, usize) {
    if let Some(ball_coord) = board.find_ball() {
        let mut friendly = 0usize;
        let mut enemy = 0usize;
        for delta in DIRECTIONS.iter() {
            let piece_coord = ball_coord.clone() + delta.clone();
            if !board.is_on_board(&piece_coord) {
                continue;
            }
            match board.get_piece(&piece_coord) {
                None => {}
                Some(Piece {
                    piece_type: PieceType::Ball,
                    player: _,
                }) => unreachable!("Two balls on board"),
                Some(Piece {
                    piece_type: _,
                    player: Player::Neutral,
                }) => unreachable!("Two balls on board"),
                Some(Piece {
                    piece_type: _,
                    player: piece_player,
                }) => {
                    if *piece_player == player {
                        friendly += 1
                    } else {
                        enemy += 1
                    }
                }
            }
        }
        return (friendly, enemy);
    }
    (0, 0)
}

/// Number of legal moves for player.
pub fn mobility(board: &ChessBallBoard, player: Player) -> usize {
    possible_moves(board, player).len()
}

/// Count pieces of `player` that are vulnerable to being tackled.
pub fn vulnerable_pieces_count(board: &ChessBallBoard, player: Player) -> usize {
    let opponent = match player {
        Player::White => Player::Black,
        Player::Black => Player::White,
        Player::Neutral => Player::Neutral,
    };
    let mut vuln = 0usize;
    for coord in board.iter_coords() {
        if let Some(p) = board.get_piece(&coord) {
            if p.player != player {
                continue;
            }
            for delta in crate::board::DIRECTIONS.iter() {
                let opp_coord = coord.clone() + delta.clone();
                let destination = coord.clone() - delta.clone();
                if !board.is_on_board(&opp_coord) || !board.is_on_board(&destination) {
                    continue;
                }
                if board.get_piece(&destination).is_some() {
                    continue;
                }
                match board.get_piece(&opp_coord) {
                    None => {}
                    Some(&Piece {
                        piece_type: PieceType::Defender,
                        player: p,
                    }) => {
                        if p == opponent {
                            vuln += 1;
                        }
                    }
                    Some(&Piece {
                        piece_type: _,
                        player: _,
                    }) => {}
                }
            }
        }
    }
    vuln
}

/// Cheap approximation for push distance to goal (normalized).
pub fn approx_push_distance(board: &ChessBallBoard, player: Player) -> f64 {
    if let Some(ball_coord) = board.find_ball() {
        let dist = match player {
            Player::White => (board.rows - 1) as isize - ball_coord.r,
            Player::Black => ball_coord.r,
            Player::Neutral => (board.rows - 1) as isize - ball_coord.r,
        } as f64;
        let max_dist = (board.rows - 1) as f64;
        if max_dist == 0.0 {
            return 1.0;
        }
        // small bonus if friendly pusher directly behind
        let forward_delta = if player == Player::White {
            Coord { r: 1, c: 0 }
        } else {
            Coord { r: -1, c: 0 }
        };
        let behind = ball_coord.clone() - forward_delta.clone();
        let mut bonus = 0.0;
        if board.is_on_board(&behind)
            && let Some(p) = board.get_piece(&behind)
            && p.player == player
        {
            let dest = ball_coord + forward_delta;
            if board.is_on_board(&dest)
                && board.get_piece(&dest).is_none()
                && !board.is_forbidden_col(&dest)
            {
                bonus = 0.5;
            }
        }
        let eff = (dist - bonus).max(0.0);
        let mut norm = 1.0 - (eff / max_dist);
        if norm < 0.0 {
            norm = 0.0;
        }
        return norm;
    }
    0.0
}

/// Player-oriented ball row in [0,1].
pub fn ball_row_for_player(board: &ChessBallBoard, player: Player) -> f64 {
    if let Some(ball_coord) = board.find_ball() {
        let val = match player {
            Player::White => ball_coord.r as f64,
            Player::Black => (board.rows as isize - 1 - ball_coord.r) as f64,
            Player::Neutral => ball_coord.r as f64,
        };
        return val / ((board.rows - 1) as f64);
    }
    -1.0
}

/// Count opponent pieces strictly between ball row and goal row.
pub fn count_opponent_pieces_between_ball_and_goal(
    board: &ChessBallBoard,
    player: Player,
) -> usize {
    if let Some(ball_coord) = board.find_ball() {
        if player == Player::Neutral {
            return 0;
        }
        let goal_row = if player == Player::White {
            board.rows as isize - 1
        } else {
            0isize
        };
        let start = ball_coord.r.min(goal_row);
        let end = ball_coord.r.max(goal_row);
        if end - start <= 1 {
            return 0;
        }
        let mut count = 0usize;
        for coord in board.iter_coords() {
            if coord.r > start
                && let Some(p) = board.get_piece(&coord)
                && p.player != player
                && p.piece_type != PieceType::Ball
            {
                count += 1;
            }
        }
        return count;
    }
    0
}

/// Feature vector similar to the Python implementation.
/// Returns a HashMap mapping feature name to value.
pub fn feature_vector(board: &ChessBallBoard, player: Player) -> HashMap<String, f64> {
    let opponent = match player {
        Player::White => Player::Black,
        Player::Black => Player::White,
        Player::Neutral => Player::Neutral,
    };

    let player_wins = !winning_moves(board, player).is_empty();
    let opp_wins = !winning_moves(board, opponent).is_empty();

    let (ball_row_feature, ball_in_forbidden) = if let Some(ball_coord) = board.find_ball() {
        let dist_rows = if player == Player::White {
            (board.rows - 1) as isize - ball_coord.r
        } else {
            ball_coord.r
        };
        let ball_row_feature = 1.0 - (dist_rows as f64 / ((board.rows - 1) as f64));
        let ball_in_forbidden = if board.is_forbidden_col(&ball_coord) {
            1.0
        } else {
            0.0
        };
        (ball_row_feature, ball_in_forbidden)
    } else {
        (0.0, 0.0)
    };

    let adj_pushers = count_adjacent_pushers(board, player) as f64 / 8.0;
    let opp_adj_pushers = count_adjacent_pushers(board, opponent) as f64 / 8.0;
    let (control_friendly, control_enemy) = count_control_around_ball(board, player);
    let control = (control_friendly as f64 - control_enemy as f64) / 8.0;

    let mob_p = mobility(board, player) as f64;
    let mob_o = mobility(board, opponent) as f64;
    let mob_cap = 60.0;
    let mob = (mob_p - mob_o) / mob_cap;

    let max_pieces = 5.0;
    let vulnerable = vulnerable_pieces_count(board, player) as f64 / max_pieces;

    let push_dist = approx_push_distance(board, player);

    let mut unavoidable = 0.0;
    if player_wins && !is_win_avoidable_by_opponent(board, player) {
        unavoidable = 1.0;
    }

    let ball_row_value = ball_row_for_player(board, player);
    let opp_between = count_opponent_pieces_between_ball_and_goal(board, player) as f64 / 5.0;

    let mut feats = HashMap::new();
    feats.insert("win_now".to_string(), if player_wins { 1.0 } else { 0.0 });
    feats.insert("lose_now".to_string(), if opp_wins { 1.0 } else { 0.0 });
    feats.insert("ball_row".to_string(), ball_row_feature);
    feats.insert("ball_in_forbidden_col".to_string(), ball_in_forbidden);
    feats.insert("adj_pushers".to_string(), adj_pushers);
    feats.insert("opp_adj_pushers".to_string(), opp_adj_pushers);
    feats.insert("control".to_string(), control);
    feats.insert("mobility".to_string(), mob);
    feats.insert("push_distance".to_string(), push_dist);
    feats.insert("unavoidable_win".to_string(), unavoidable);
    feats.insert("vulnerable".to_string(), vulnerable);
    feats.insert("ball_row_value".to_string(), ball_row_value);
    feats.insert("opp_between_ball_and_goal".to_string(), opp_between);
    feats
}
