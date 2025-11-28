//! Simple CLI example to play ChessBall against the AI or watch two AIs play.
//!
//! Commands:
//! - Enter: advance with AI choosing the move for the current player
//! - h r_from c_from r_to c_to  : make a manual (human) move if legal (indices 0-based)
//! - q : quit
//!
//! This is intentionally small and focuses on demonstrating the Rust port and the minimax function.

use std::io::{self, Write};
use chessball::board::{ChessBallBoard, Piece, PieceType, Player};
use chessball::moves::possible_moves;
use chessball::minimax::choose_best_move;

fn print_help() {
    println!("Commands:");
    println!("  <enter> : let AI choose a move for the current player");
    println!("  h r_from c_from r_to c_to : human move (0-based indices)");
    println!("  q : quit");
    println!("Example: h 5 2 4 2");
}

fn try_apply_human_move(b: &mut ChessBallBoard, player: Player, r1: usize, c1: usize, r2: usize, c2: usize) -> bool {
    for (mv, nb) in possible_moves(b, player) {
        if mv.from == (r1, c1) && mv.to == (r2, c2) {
            *b = nb;
            return true;
        }
    }
    false
}

fn main() {
    // START_BOARD from original minimax.py example (adapted to 7x6)
    let start = "\
-- -- BD BD BD --\n\
-- -- BA BA -- --\n\
-- -- -- -- -- --\n\
-- -- -- NB -- --\n\
-- -- -- -- -- --\n\
-- -- WA WA -- --\n\
-- -- WD WD WD --\n";
    let mut board = ChessBallBoard::from_repr(start).expect("failed to parse start board");
    let mut current = Player::White;

    println!("Welcome to ChessBall (Rust port example).");
    print_help();
    loop {
        println!("\nCurrent player: {:?}", current);
        println!("{}", board);
        print!("cmd> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            // let AI choose for current player
            let (mv, nb, score) = choose_best_move(&board, current, 2);
            match mv {
                Some(m) => {
                    println!("AI chooses move: from {:?} to {:?} (score {:.2})", m.from, m.to, score);
                    if let Some(nb) = nb {
                        board = nb;
                    } else {
                        println!("(no board after move available)");
                    }
                }
                None => {
                    println!("No move found for player {:?}", current);
                }
            }
        } else if line == "q" {
            println!("Goodbye.");
            break;
        } else if line.starts_with("h ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 5 {
                println!("Invalid human command; expected 4 integers");
                continue;
            }
            let r1: usize = match parts[1].parse() { Ok(v) => v, Err(_) => { println!("bad int"); continue; } };
            let c1: usize = match parts[2].parse() { Ok(v) => v, Err(_) => { println!("bad int"); continue; } };
            let r2: usize = match parts[3].parse() { Ok(v) => v, Err(_) => { println!("bad int"); continue; } };
            let c2: usize = match parts[4].parse() { Ok(v) => v, Err(_) => { println!("bad int"); continue; } };
            if try_apply_human_move(&mut board, current, r1, c1, r2, c2) {
                println!("Applied human move.");
            } else {
                println!("Move not legal.");
                continue;
            }
        } else if line == "help" || line == "?" {
            print_help();
            continue;
        } else {
            println!("Unknown command. Type Enter for AI move, 'h ...' for human move, or 'q' to quit.");
            continue;
        }

        // swap player
        current = match current {
            Player::White => Player::Black,
            Player::Black => Player::White,
            Player::Neutral => Player::Neutral,
        };
    }
}