use crate::board::{ChessBallBoard, DIRECTIONS, Piece, PieceType};
use crate::board::Player;
use std::clone::Clone;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveInfo {
    pub from: (usize, usize),
    pub to: (usize, usize),
    pub push_ball: bool,
    pub ball_to: Option<(usize, usize)>,
    pub jump: bool,
    pub jumped_over: Option<(usize, usize)>,
    pub tackle: bool,
    pub pushed_piece_from: Option<(usize, usize)>,
    pub pushed_piece_to: Option<(usize, usize)>,
}

impl MoveInfo {
    pub fn simple(from: (usize, usize), to: (usize, usize)) -> Self {
        Self {
            from,
            to,
            push_ball: false,
            ball_to: None,
            jump: false,
            jumped_over: None,
            tackle: false,
            pushed_piece_from: None,
            pushed_piece_to: None,
        }
    }
}

impl fmt::Display for MoveInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{} -> {}{}", self.from.0, self.from.1, self.to.0, self.to.1)?;
        Ok(())
    }
}

pub fn possible_moves(board: &ChessBallBoard, player: Player) -> Vec<(MoveInfo, ChessBallBoard)> {
    let mut results = Vec::new();
    for r in 0..board.rows {
        for c in 0..board.cols {
            if let Some(piece) = board.get_piece(r, c).cloned() {
                if piece.player != player {
                    continue;
                }
                for &(dr, dc) in DIRECTIONS.iter() {
                    let nr = r as isize + dr;
                    let nc = c as isize + dc;
                    if nr >= 0 && nc >= 0 && (nr as usize) < board.rows && (nc as usize) < board.cols {
                        let (nr_u, nc_u) = (nr as usize, nc as usize);
                        // normal adjacent move if empty
                        if board.get_piece(nr_u, nc_u).is_none() {
                            let mut newb = board.clone();
                            // move piece
                            newb.remove_piece(r, c);
                            newb.place_piece(nr_u, nc_u, piece.clone());
                            results.push((MoveInfo::simple((r, c), (nr_u, nc_u)), newb));
                        } else if let Some(tgt) = board.get_piece(nr_u, nc_u) {
                            if tgt.piece_type == PieceType::Ball {
                                // ball push: ball moves to br2, bc2
                                let br2r = nr + dr;
                                let br2c = nc + dc;
                                if br2r >= 0 && br2c >= 0 && (br2r as usize) < board.rows && (br2c as usize) < board.cols {
                                    let br2 = (br2r as usize, br2c as usize);
                                    // destination empty and not forbidden col
                                    if board.get_piece(br2.0, br2.1).is_none() && !board.is_forbidden_col(br2.1) {
                                        let mut newb = board.clone();
                                        newb.remove_piece(r, c);
                                        newb.place_piece(nr_u, nc_u, piece.clone());
                                        newb.place_piece(br2.0, br2.1, Piece { piece_type: PieceType::Ball, player: Player::Neutral });
                                        let mut info = MoveInfo::simple((r, c), (nr_u, nc_u));
                                        info.push_ball = true;
                                        info.ball_to = Some(br2);
                                        results.push((info, newb));
                                    }
                                }
                            }
                        }
                    }
                    // Attacker jump
                    if piece.piece_type == PieceType::Attacker {
                        let adj_r = r as isize + dr;
                        let adj_c = c as isize + dc;
                        let jump_r = r as isize + 2 * dr;
                        let jump_c = c as isize + 2 * dc;
                        if adj_r >= 0 && adj_c >= 0 && jump_r >= 0 && jump_c >= 0 &&
                            (adj_r as usize) < board.rows && (adj_c as usize) < board.cols &&
                            (jump_r as usize) < board.rows && (jump_c as usize) < board.cols {
                            let adj = board.get_piece(adj_r as usize, adj_c as usize);
                            let jtarget = board.get_piece(jump_r as usize, jump_c as usize);
                            if adj.is_some() && adj.unwrap().piece_type != PieceType::Ball && jtarget.is_none() {
                                let mut newb = board.clone();
                                newb.remove_piece(r, c);
                                newb.place_piece(jump_r as usize, jump_c as usize, piece.clone());
                                let mut info = MoveInfo::simple((r, c), (jump_r as usize, jump_c as usize));
                                info.jump = true;
                                info.jumped_over = Some((adj_r as usize, adj_c as usize));
                                results.push((info, newb));
                            }
                        }
                    } else if piece.piece_type == PieceType::Defender {
                        // Defender tackle
                        if nr >= 0 && nc >= 0 && (nr as usize) < board.rows && (nc as usize) < board.cols {
                            let beyond_r = nr + dr;
                            let beyond_c = nc + dc;
                            if beyond_r >= 0 && beyond_c >= 0 && (beyond_r as usize) < board.rows && (beyond_c as usize) < board.cols {
                                let target = board.get_piece(nr as usize, nc as usize);
                                let beyond = board.get_piece(beyond_r as usize, beyond_c as usize);
                                if let Some(tgt) = target {
                                    if tgt.player != player && tgt.piece_type != PieceType::Ball && beyond.is_none() {
                                        let mut newb = board.clone();
                                        newb.remove_piece(r, c);
                                        newb.place_piece(nr as usize, nc as usize, piece.clone());
                                        // push opponent to beyond
                                        newb.remove_piece(nr as usize, nc as usize);
                                        newb.place_piece(beyond_r as usize, beyond_c as usize, tgt.clone());
                                        let mut info = MoveInfo::simple((r, c), (nr as usize, nc as usize));
                                        info.tackle = true;
                                        info.pushed_piece_from = Some((nr as usize, nc as usize));
                                        info.pushed_piece_to = Some((beyond_r as usize, beyond_c as usize));
                                        results.push((info, newb));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    results
}

pub fn possible_previous_moves(board: &ChessBallBoard, player: Player) -> Vec<(MoveInfo, ChessBallBoard)> {
    let mut prevs = Vec::new();
    for r in 0..board.rows {
        for c in 0..board.cols {
            if let Some(piece) = board.get_piece(r, c).cloned() {
                if piece.player != player { continue; }
                for &(dr, dc) in DIRECTIONS.iter() {
                    // simple move: piece might have come from pr,pc
                    let pr = r as isize - dr;
                    let pc = c as isize - dc;
                    if pr >= 0 && pc >= 0 && (pr as usize) < board.rows && (pc as usize) < board.cols {
                        if board.get_piece(pr as usize, pc as usize).is_none() {
                            let mut prev_board = board.clone();
                            prev_board.remove_piece(r, c);
                            prev_board.place_piece(pr as usize, pc as usize, piece.clone());
                            prevs.push((MoveInfo::simple((pr as usize, pc as usize), (r, c)), prev_board));
                        }
                    }

                    // ball-push reverse:
                    // if ball currently at (r,c) and it could have been pushed from (r-dr, c-dc)
                    if let Some(ball_pos) = board.find_ball() {
                        let (br, bc) = ball_pos;
                        if br == r && bc == c {
                            let ball_src_r = (r as isize - dr) as isize;
                            let ball_src_c = (c as isize - dc) as isize;
                            let pr = (r as isize - dr) as isize;
                            let pc = (c as isize - dc) as isize;
                            if ball_src_r >= 0 && ball_src_c >= 0 &&
                               pr >= 0 && pc >= 0 &&
                               (ball_src_c as usize) < board.cols &&
                               (ball_src_r as usize) < board.rows &&
                               (pr as usize) < board.rows && (pc as usize) < board.cols {
                                // source column must not be forbidden
                                if !board.is_forbidden_col(ball_src_c as usize) {
                                    let dest_piece = board.get_piece(br, bc);
                                    if let Some(dest) = dest_piece {
                                        if dest.piece_type == PieceType::Ball &&
                                            board.get_piece(pr as usize, pc as usize).is_none() {
                                            // candidate prev: piece at pr,pc pushed ball from ball_src to ball_dest (r,c)
                                            let mut prev_board = board.clone();
                                            prev_board.remove_piece(r, c);
                                            prev_board.place_piece(pr as usize, pc as usize, piece.clone());
                                            prev_board.remove_piece(br, bc);
                                            prev_board.place_piece(ball_src_r as usize, ball_src_c as usize, Piece { piece_type: PieceType::Ball, player: Player::Neutral });
                                            let mut info = MoveInfo::simple((pr as usize, pc as usize), (r, c));
                                            info.push_ball = true;
                                            info.ball_to = Some((r, c));
                                            prevs.push((info, prev_board));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Attacker jump reverse
                if piece.piece_type == PieceType::Attacker {
                    for &(dr, dc) in DIRECTIONS.iter() {
                        let adj_r = r as isize - dr;
                        let adj_c = c as isize - dc;
                        let prev_r = r as isize - 2*dr;
                        let prev_c = c as isize - 2*dc;
                        if adj_r >=0 && adj_c >= 0 && prev_r >=0 && prev_c >=0 &&
                           (adj_r as usize) < board.rows && (adj_c as usize) < board.cols &&
                           (prev_r as usize) < board.rows && (prev_c as usize) < board.cols {
                            let adj_piece = board.get_piece(adj_r as usize, adj_c as usize);
                            let prev_square = board.get_piece(prev_r as usize, prev_c as usize);
                            if adj_piece.is_some() && adj_piece.unwrap().piece_type != PieceType::Ball && prev_square.is_none() {
                                let mut prev_board = board.clone();
                                prev_board.remove_piece(r, c);
                                prev_board.place_piece(prev_r as usize, prev_c as usize, piece.clone());
                                let mut info = MoveInfo::simple((prev_r as usize, prev_c as usize), (r, c));
                                info.jump = true;
                                info.jumped_over = Some((adj_r as usize, adj_c as usize));
                                prevs.push((info, prev_board));
                            }
                        }
                    }
                }

                // Defender tackle retrograde
                if piece.piece_type == PieceType::Defender {
                    for &(dr, dc) in DIRECTIONS.iter() {
                        let opp_r = r as isize - dr;
                        let opp_c = c as isize - dc;
                        let pushed_r = r as isize + dr;
                        let pushed_c = c as isize + dc;
                        if opp_r >=0 && opp_c >=0 && pushed_r >=0 && pushed_c >=0 &&
                           (opp_r as usize) < board.rows && (opp_c as usize) < board.cols &&
                           (pushed_r as usize) < board.rows && (pushed_c as usize) < board.cols {
                            let opp_piece = board.get_piece(pushed_r as usize, pushed_c as usize);
                            let defender_prev_square = board.get_piece(opp_r as usize, opp_c as usize);
                            if let Some(op) = opp_piece {
                                if op.player != player && op.piece_type != PieceType::Ball && defender_prev_square.is_none() {
                                    let mut prev_board = board.clone();
                                    prev_board.remove_piece(r, c);
                                    prev_board.place_piece(opp_r as usize, opp_c as usize, piece.clone());
                                    prev_board.remove_piece(pushed_r as usize, pushed_c as usize);
                                    prev_board.place_piece(r, c, op.clone());
                                    let mut info = MoveInfo::simple((opp_r as usize, opp_c as usize), (r, c));
                                    info.tackle = true;
                                    info.pushed_piece_from = Some((r, c));
                                    info.pushed_piece_to = Some((pushed_r as usize, pushed_c as usize));
                                    prevs.push((info, prev_board));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    prevs
}

#[cfg(test)]
mod tests {
    use crate::{
        board::{ChessBallBoard, Piece, PieceType, Player},
        moves::{possible_moves, possible_previous_moves}
    };

    // fn print_two_boards(_b1: &ChessBallBoard, _b2: &ChessBallBoard) {
    //     // Omitted; tests will assert properties instead of printing.
    // }

    #[test]
    fn test_possible_moves_push_move() {
        let mut b = ChessBallBoard::new();
        b.place_piece(2, 3, Piece { piece_type: PieceType::Defender, player: Player::White });
        b.place_piece(2, 4, Piece { piece_type: PieceType::Ball, player: Player::Neutral });
        let mut found_push = false;
        for (info, _nb) in possible_moves(&b, Player::White) {
            //println!("{info}");
            if info.push_ball { found_push = true; break; }
        }
        assert!(found_push);
    }

    #[test]
    fn test_possible_moves_simple_moves() {
        let mut b = ChessBallBoard::new();
        b.place_piece(2, 3, Piece { piece_type: PieceType::Defender, player: Player::White });
        let moves = possible_moves(&b, Player::White);
        assert!(moves.len() >= 1);
    }

    #[test]
    fn test_possible_previous_moves() {
        let mut b = ChessBallBoard::new();
        b.place_piece(2, 4, Piece { piece_type: PieceType::Ball, player: Player::Neutral});
        b.place_piece(2, 3, Piece { piece_type: PieceType::Defender, player: Player::White});
        let prevs = possible_previous_moves(&b, Player::White);
        assert!(prevs.len() >= 1);
    }
}
