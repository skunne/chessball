//! ChessBall crate root.
//!
//! Modules:
//! - agent: reusable engine/agent interface plus the classical alpha-beta engine
//! - alphazero: AlphaZero-style MCTS engine with tabular self-play training
//! - arena: engine-vs-engine match runner
//! - board: core board and piece types
//! - moves: move generation and reverse move generation
//! - winning_moves: quick detection of winning moves
//! - blocking_move: find a blocking move if available
//! - win_avoidability: check if a win was avoidable by opponent
//! - heuristics: feature extraction & evaluation
//! - minimax: simple minimax search
//! - engine: canonical ChessBall rules engine for search and solving work
//! - solver: alpha-beta search on top of the canonical engine
//! - record: replayable ChessBall game records
//! - tournament: self-play tournament utilities
//! - partial_tablebase: conservative partial proof builder with export/visualization
//! - weak_solve: exact reachable-graph and retrograde solver

// Library root: expose modules
pub mod agent;
pub mod alphazero;
pub mod arena;
pub mod blocking_move;
pub mod board;
pub mod engine;
pub mod heuristics;
pub mod minimax;
pub mod moves;
pub mod partial_tablebase;
pub mod record;
pub mod solver;
pub mod tournament;
pub mod weak_solve;
pub mod win_avoidability;
pub mod winning_moves;
