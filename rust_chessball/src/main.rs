//! Improved CLI for ChessBall (Rust port).
//!
//! Features:
//! - Human move parsing in "e2e4" or "e2 e4" format (columns a..f, rows 1..7 where 1 is White's back row).
//! - List legal moves for the current player with indices.
//! - Play AI move with `ai` or by pressing Enter.
//! - Make a human move by algebraic (e2e4) or by selecting an indexed legal move: `m 3`.
//! - Commands: `help`, `q`, `list`, `ai`, `depth N`, `m <index>`, `h e2e4`
//!
//! Coordinate system notes:
//! - Board has 6 columns (a..f) and 7 rows (1..7).
//! - We map "a1" -> (row = rows-1, col = 0) (White's goal row is 1), so "a7" is top row (row 0).
//! - This is chosen to be similar to chess algebraic where rank 1 is White's home.

use chessball::board::{ChessBallBoard, Coord, Player};
use chessball::minimax::choose_best_move;
use chessball::moves::{
    DefenderTackle, MoveInfo, MoveSpecialInfo, possible_moves, possible_previous_moves,
};
use std::io::{self, Write};

fn coord_to_rc(token: &str, rows: usize, cols: usize) -> Option<Coord> {
    // Expect token like "e2" where 'a'..'f' map to cols 0..cols-1 and '1'..'7' map to rows bottom->top.
    let token = token.trim();
    if token.len() < 2 || token.len() > 3 {
        return None;
    }
    let mut chars = token.chars();
    let file = chars.next().unwrap();
    let file = file.to_ascii_lowercase();
    let col = (file as u8).wrapping_sub(b'a') as isize;
    if col < 0 || (col as usize) >= cols {
        return None;
    }
    let rank_str: String = chars.collect();
    let rank: usize = match rank_str.parse() {
        Ok(n) => n,
        Err(_) => return None,
    };
    if rank < 1 || rank > rows {
        return None;
    }
    // Algebraic: rank 1 is bottom (white's home) -> row_index = rows - rank
    Some(Coord {
        r: (rows - rank),
        c: col as usize,
    })
}

fn rc_to_coord(coord: &Coord, rows: usize) -> String {
    // inverse of coord_to_rc
    let file = (b'a' + (coord.c as u8)) as char;
    // rank = rows - r
    let rank = rows as isize - coord.r as isize;
    format!("{}{}", file, rank)
}

fn move_to_pretty(mi: &MoveInfo, board_rows: usize) -> String {
    let mut s = format!(
        "{}->{}",
        rc_to_coord(&mi.from, board_rows),
        rc_to_coord(&mi.to, board_rows)
    );
    let mut flags = Vec::new();
    match &mi.special {
        MoveSpecialInfo::BallPush { ball_to } => {
            flags.push(format!("push ball->{}", rc_to_coord(ball_to, board_rows)))
        }
        MoveSpecialInfo::AttackerJump { jumped_over } => {
            flags.push(format!(
                "jump over {}",
                rc_to_coord(jumped_over, board_rows)
            ));
        }
        MoveSpecialInfo::DefenderTackle(DefenderTackle {
            pushed_piece_from,
            pushed_piece_to,
        }) => {
            flags.push(format!(
                "tackle push {}->{}",
                rc_to_coord(pushed_piece_from, board_rows),
                rc_to_coord(pushed_piece_to, board_rows)
            ));
        }
        MoveSpecialInfo::SimpleMove => {}
    };
    if !flags.is_empty() {
        s.push_str(" (");
        s.push_str(&flags.join(", "));
        s.push(')');
    }
    s
}

fn apply_move_by_index(board: &mut ChessBallBoard, player: Player, index: usize) -> bool {
    let moves = possible_moves(board, player);
    if index >= moves.len() {
        return false;
    }
    let (_mv, nb) = moves.into_iter().nth(index).unwrap();
    *board = nb;
    true
}

fn try_apply_algebraic_move(
    board: &mut ChessBallBoard,
    player: Player,
    src: &str,
    dst: &str,
) -> Result<(), String> {
    let rows = board.rows;
    let cols = board.cols;
    let from = coord_to_rc(src, rows, cols)
        .ok_or_else(|| format!("Invalid source coordinate '{}'", src))?;
    let to =
        coord_to_rc(dst, rows, cols).ok_or_else(|| format!("Invalid dest coordinate '{}'", dst))?;
    // Find a legal move matching these coords
    for (mv, nb) in possible_moves(board, player) {
        if mv.from == from && mv.to == to {
            *board = nb;
            return Ok(());
        }
    }
    // If not found, provide helpful diagnostics
    // 1) Check whether there is a piece of player's color at 'from'
    if let Some(p) = board.get_piece(from) {
        if p.player != player {
            return Err(format!(
                "Piece at {} belongs to {:?}, not {:?}",
                src, p.player, player
            ));
        }
    } else {
        return Err(format!("No piece at source square {}", src));
    }
    // 2) Check whether any move from that source exists
    let mut legal_from = false;
    for (mv, _nb) in possible_moves(board, player) {
        if mv.from == from {
            legal_from = true;
            break;
        }
    }
    if !legal_from {
        return Err(format!("Piece at {} has no legal moves right now", src));
    }
    // 3) Destination is simply illegal for that piece
    Err(format!(
        "Destination {} is not a legal move from {}",
        dst, src
    ))
}

fn print_legal_moves(board: &ChessBallBoard, player: Player) {
    let moves = possible_moves(board, player);
    if moves.is_empty() {
        println!("No legal moves for {:?}", player);
        return;
    }
    println!("Legal moves for {:?} (index: move):", player);
    for (i, (mv, _nb)) in moves.iter().enumerate() {
        println!("  {:>3}: {}", i, move_to_pretty(mv, board.rows));
    }
}

fn print_possible_prev_moves(board: &ChessBallBoard, player: Player) {
    let prevs = possible_previous_moves(board, player);
    if prevs.is_empty() {
        println!("No previous positions found for {:?}", player);
        return;
    }
    println!("{} possible previous moves for {:?}", prevs.len(), player);
    for (i, (mv, prevb)) in prevs.into_iter().enumerate().take(10) {
        println!(
            "Prev {}: {} =>\n{}",
            i,
            move_to_pretty(&mv, board.rows),
            prevb
        );
    }
}

fn print_help() {
    println!("Commands:");
    println!("  <enter>              : AI chooses a move for the current player (depth default)");
    println!("  ai                   : same as Enter");
    println!("  ai <n>               : AI chooses moves for both players for the next n plies");
    println!(
        "  h e2e4               : human move in algebraic form (columns a..f, rows 1..7). Example: h b2b3"
    );
    println!("  h e2 e4              : also accepted");
    println!("  m <index>            : apply the legal move with given index (see 'list')");
    println!("  list                 : list legal moves for current player");
    println!("  prev                 : show some possible previous moves (diagnostic)");
    println!("  depth <n>            : set AI search depth (default 2)");
    println!("  q                    : quit");
    println!("  help                 : print this message");
    println!();
    println!(
        "Note: columns a..f map to board columns left->right, ranks 1..7 map bottom (White's goal) -> top."
    );
}

fn main() {
    let start = "\
-- BD -- BD -- BD --\n\
-- -- BA -- BA -- --\n\
-- -- -- NB -- -- --\n\
-- -- -- -- -- -- --\n\
-- -- WA -- WA -- --\n\
-- WD -- WD -- WD --\n";
    let mut board = ChessBallBoard::from_repr(start).expect("failed to parse start board");
    let mut current = Player::White;
    let mut depth = 2usize;

    println!("Welcome to ChessBall (Rust port enhanced CLI).");
    print_help();

    loop {
        println!("\nCurrent player: {:?}\n", current);
        println!("{}", board);
        print!("cmd> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            println!("Input error - quitting.");
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            // AI move
            let (mv, nb, score) = choose_best_move(&board, current, depth);
            match mv {
                Some(m) => {
                    println!(
                        "AI chooses move: {} (score {:.2})",
                        move_to_pretty(&m, board.rows),
                        score
                    );
                    if let Some(nb) = nb {
                        board = nb;
                        swap_player(&mut current);
                    } else {
                        println!("(no board after move available)");
                    }
                }
                None => {
                    println!("No move found for player {:?}", current);
                }
            }
        } else {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts[0] {
                "q" => {
                    println!("Goodbye.");
                    break;
                }
                "help" | "h?" | "?" => {
                    print_help();
                }
                "list" => {
                    print_legal_moves(&board, current);
                }
                "prev" => {
                    // Diagnostic: show some previous positions for opponent
                    print_possible_prev_moves(&board, current);
                }
                "ai" => {
                    let n_moves = if parts.len() < 2 {
                        1
                    } else {
                        match parts[1].parse::<usize>() {
                            Ok(d) => {
                                println!("Playing next {d} moves with AI");
                                d
                            }
                            Err(_) => {
                                println!("Invalid number of moves! Only playing one AI move");
                                1
                            }
                        }
                    };
                    for _ in 0..n_moves {
                        let (mv, nb, score) = choose_best_move(&board, current, depth);
                        match mv {
                            Some(m) => {
                                println!(
                                    "AI chooses move: {} (score {:.2})",
                                    move_to_pretty(&m, board.rows),
                                    score
                                );
                                if let Some(nb) = nb {
                                    board = nb;
                                    swap_player(&mut current);
                                }
                            }
                            None => println!("No AI move found"),
                        }
                    }
                }
                "depth" => {
                    if parts.len() >= 2 {
                        match parts[1].parse::<usize>() {
                            Ok(d) => {
                                depth = d;
                                println!("Depth set to {}", depth);
                            }
                            Err(_) => println!("Invalid depth number"),
                        }
                    } else {
                        println!("Current depth: {}", depth);
                    }
                }
                "m" => {
                    if parts.len() < 2 {
                        println!("Usage: m <index> (see 'list')");
                    } else if let Ok(idx) = parts[1].parse::<usize>() {
                        if apply_move_by_index(&mut board, current, idx) {
                            {
                                println!("Applied move index {}", idx);
                                swap_player(&mut current)
                            };
                        } else {
                            println!("Invalid move index {}", idx);
                        }
                    } else {
                        println!("Invalid index");
                    }
                }
                "h" => {
                    // human move: accept "h e2e4" or "h e2 e4"
                    if parts.len() == 2 {
                        // maybe "e2e4"
                        let token = parts[1];
                        let token = token.trim();
                        // accept 4 or 5 char string like e2e4 or e2e10 (but here rows up to 7)
                        // try splitting into two coords of equal length (2+2 or 2+2)
                        // best to split into two tokens: first two chars and remaining.
                        if token.len() >= 4 {
                            // First coordinate might be 2 or 3 chars; try 2 then rest
                            let a = &token[0..2];
                            let b = &token[2..];
                            match try_apply_algebraic_move(&mut board, current, a, b) {
                                Ok(_) => {
                                    println!("Applied move {} -> {}", a, b);
                                    swap_player(&mut current)
                                }
                                Err(e) => println!("Illegal move: {}", e),
                            }
                        } else {
                            println!(
                                "Can't parse move '{}'; expected like e2e4 or 'h e2 e4'",
                                token
                            );
                        }
                    } else if parts.len() == 3 {
                        let a = parts[1];
                        let b = parts[2];
                        match try_apply_algebraic_move(&mut board, current, a, b) {
                            Ok(_) => {
                                println!("Applied move {} -> {}", a, b);
                                swap_player(&mut current)
                            }
                            Err(e) => println!("Illegal move: {}", e),
                        }
                    } else {
                        println!("Usage: h e2e4  or  h e2 e4");
                    }
                }
                _ => {
                    println!(
                        "Unknown command '{}'. Type 'help' for available commands.",
                        parts[0]
                    );
                }
            }
        }
    }
}

fn swap_player(player: &mut Player) {
    *player = match player {
        Player::White => Player::Black,
        Player::Black => Player::White,
        Player::Neutral => Player::Neutral,
    }
}
