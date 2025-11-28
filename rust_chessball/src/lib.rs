//! ChessBall crate root.
//!
//! Modules:
//! - board: core board and piece types
//! - moves: move generation and reverse move generation
//! - winning_moves: quick detection of winning moves
//! - blocking_move: find a blocking move if available
//! - win_avoidability: check if a win was avoidable by opponent
//! - heuristics: feature extraction & evaluation
//! - minimax: simple minimax search

// Library root: expose modules
pub mod board;
pub mod moves;
pub mod winning_moves;
pub mod blocking_move;
pub mod win_avoidability;
pub mod heuristics;
pub mod minimax;