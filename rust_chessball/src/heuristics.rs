use crate::board::{ChessBallBoard, Player, PieceType};
use crate::moves::possible_moves;
use crate::winning_moves::winning_moves;
use crate::win_avoidability::is_win_avoidable_by_opponent;
use std::collections::HashMap;

pub fn ball_pos(board: &ChessBallBoard) -> Option<(usize, usize)> {
    board.find_ball()
}

pub fn count_adjacent_pushers(board: &ChessBallBoard, player: Player) -> usize {
    if let Some((br, bc)) = board.find_ball() {
        let mut count = 0usize;
        for &(dr, dc) in crate::board::DIRECTIONS.iter() {
            let pr = br as isize - dr;
            let pc = bc as isize - dc;
            let dest_r = br as isize + dr;
            let dest_c = bc as isize + dc;
            if pr < 0 || pc < 0 || dest_r < 0 || dest_c < 0 { continue; }
            let (pr_u, pc_u, dest_r_u, dest_c_u) = (pr as usize, pc as usize, dest_r as usize, dest_c as usize);
            if pr_u >= board.rows || pc_u >= board.cols || dest_r_u >= board.rows || dest_c_u >= board.cols { continue; }
            if board.is_forbidden_col(dest_c_u) { continue; }
            if let Some(pusher) = board.get_piece(pr_u, pc_u) {
                if pusher.player == player && board.get_piece(dest_r_u, dest_c_u).is_none() {
                    count += 1;
                }
            }
        }
        return count;
    }
    0
}

pub fn count_control_around_ball(board: &ChessBallBoard, player: Player) -> (usize, usize) {
    if let Some((br, bc)) = board.find_ball() {
        let mut friendly = 0usize;
        let mut enemy = 0usize;
        for &(dr, dc) in crate::board::DIRECTIONS.iter() {
            let r = br as isize + dr;
            let c = bc as isize + dc;
            if r < 0 || c < 0 { continue; }
            let (r_u, c_u) = (r as usize, c as usize);
            if r_u >= board.rows || c_u >= board.cols { continue; }
            if let Some(p) = board.get_piece(r_u, c_u) {
                if p.player == player { friendly += 1; } else { enemy += 1; }
            }
        }
        return (friendly, enemy);
    }
    (0, 0)
}

pub fn mobility(board: &ChessBallBoard, player: Player) -> usize {
    possible_moves(board, player).len()
}

pub fn vulnerable_pieces_count(board: &ChessBallBoard, player: Player) -> usize {
    let opponent = match player { Player::White => Player::Black, Player::Black => Player::White, Player::Neutral => Player::Neutral };
    let mut vuln = 0usize;
    for r in 0..board.rows {
        for c in 0..board.cols {
            if let Some(p) = board.get_piece(r, c) {
                if p.player != player { continue; }
                for &(dr, dc) in crate::board::DIRECTIONS.iter() {
                    let opp_r = r as isize + dr;
                    let opp_c = c as isize + dc;
                    let beyond_r = r as isize + 2*dr;
                    let beyond_c = c as isize + 2*dc;
                    if opp_r < 0 || opp_c < 0 || beyond_r < 0 || beyond_c < 0 { continue; }
                    let (opp_r_u, opp_c_u, beyond_r_u, beyond_c_u) = (opp_r as usize, opp_c as usize, beyond_r as usize, beyond_c as usize);
                    if opp_r_u >= board.rows || opp_c_u >= board.cols || beyond_r_u >= board.rows || beyond_c_u >= board.cols { continue; }
                    if let Some(opp_piece) = board.get_piece(opp_r_u, opp_c_u) {
                        if opp_piece.player == opponent && opp_piece.piece_type == PieceType::Defender {
                            if let Some(beyond) = board.get_piece(beyond_r_u, beyond_c_u) { /* not empty */ } else {
                                if p.piece_type != PieceType::Ball {
                                    vuln += 1;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    vuln
}

pub fn approx_push_distance(board: &ChessBallBoard, player: Player) -> f64 {
    if let Some((br, bc)) = board.find_ball() {
        let dist = match player {
            Player::White => (board.rows - 1) as isize - br as isize,
            Player::Black => br as isize - 0,
            Player::Neutral => (board.rows - 1) as isize - br as isize,
        } as f64;
        let max_dist = (board.rows - 1) as f64;
        if max_dist == 0.0 { return 1.0; }
        // small bonus if friendly pusher directly behind
        let forward_dr = if player == Player::White { 1isize } else { -1isize };
        let behind_r = br as isize - forward_dr;
        let behind_c = bc as isize;
        let mut bonus = 0.0;
        if behind_r >= 0 && behind_c >= 0 && (behind_r as usize) < board.rows && (behind_c as usize) < board.cols {
            if let Some(p) = board.get_piece(behind_r as usize, behind_c as usize) {
                if p.player == player {
                    let dest_r = br as isize + forward_dr;
                    let dest_c = bc as isize;
                    if dest_r >= 0 && dest_c >= 0 && (dest_r as usize) < board.rows && (dest_c as usize) < board.cols {
                        if board.get_piece(dest_r as usize, dest_c as usize).is_none() && !board.is_forbidden_col(dest_c as usize) {
                            bonus = 0.5;
                        }
                    }
                }
            }
        }
        let eff = (dist - bonus).max(0.0);
        let mut norm = 1.0 - (eff / max_dist);
        if norm < 0.0 { norm = 0.0; }
        return norm;
    }
    0.0
}

pub fn ball_row_for_player(board: &ChessBallBoard, player: Player) -> f64 {
    if let Some((row, _)) = board.find_ball() {
        let val = match player {
            Player::White => row as f64,
            Player::Black => (board.rows - 1 - row) as f64,
            Player::Neutral => row as f64,
        };
        return val / ((board.rows - 1) as f64);
    }
    -1.0
}

pub fn count_opponent_pieces_between_ball_and_goal(board: &ChessBallBoard, player: Player) -> usize {
    if let Some((ball_row, _)) = board.find_ball() {
        if player == Player::Neutral { return 0; }
        let goal_row = if player == Player::White { board.rows - 1 } else { 0usize };
        let start = ball_row.min(goal_row);
        let end = ball_row.max(goal_row);
        if end - start <= 1 { return 0; }
        let mut count = 0usize;
        for r in (start + 1)..end {
            for c in 0..board.cols {
                if let Some(p) = board.get_piece(r, c) {
                    if p.player != player && p.piece_type != PieceType::Ball {
                        count += 1;
                    }
                }
            }
        }
        return count;
    }
    0
}

pub fn feature_vector(board: &ChessBallBoard, player: Player) -> HashMap<String, f64> {
    let opponent = match player { Player::White => Player::Black, Player::Black => Player::White, Player::Neutral => Player::Neutral };

    let player_wins = !winning_moves(board, player).is_empty();
    let opp_wins = !winning_moves(board, opponent).is_empty();

    let (ball_row_feature, ball_in_forbidden) = if let Some((br, bc)) = board.find_ball() {
        let dist_rows = if player == Player::White { (board.rows - 1) as isize - br as isize } else { br as isize - 0 };
        let ball_row_feature = 1.0 - (dist_rows as f64 / ((board.rows - 1) as f64));
        let ball_in_forbidden = if board.is_forbidden_col(bc) { 1.0 } else { 0.0 };
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
    if player_wins {
        if !is_win_avoidable_by_opponent(board, player) {
            unavoidable = 1.0;
        }
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