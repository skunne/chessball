use std::path::Path;

use crate::{
    engine::{Move, Player, Position},
    record::{GameOutcome, MoveSource},
    solver::Searcher,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineDecision {
    pub mv: Move,
    pub source: MoveSource,
    pub score: Option<i32>,
    pub nodes: Option<u64>,
}

pub trait Agent {
    fn label(&self) -> String;

    fn begin_game(&mut self, _side: Player) {}

    fn select_move(&mut self, position: &Position) -> Option<EngineDecision>;

    fn end_game(&mut self, _outcome: GameOutcome) {}

    fn save_checkpoint(&self, _path: &Path) -> Result<(), String> {
        Err("checkpointing is not supported for this agent".to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassicalConfig {
    pub depth: u8,
}

impl Default for ClassicalConfig {
    fn default() -> Self {
        Self { depth: 4 }
    }
}

#[derive(Debug, Default)]
pub struct ClassicalEngine {
    config: ClassicalConfig,
    searcher: Searcher,
}

impl ClassicalEngine {
    #[must_use]
    pub fn new(config: ClassicalConfig) -> Self {
        Self {
            config,
            searcher: Searcher::new(),
        }
    }
}

impl Agent for ClassicalEngine {
    fn label(&self) -> String {
        format!("classical(depth={})", self.config.depth)
    }

    fn begin_game(&mut self, _side: Player) {
        self.searcher.clear();
    }

    fn select_move(&mut self, position: &Position) -> Option<EngineDecision> {
        let result = self.searcher.best_move(position, self.config.depth);
        result.best_move.map(|mv| EngineDecision {
            mv,
            source: MoveSource::Search,
            score: Some(result.score),
            nodes: Some(result.nodes),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{Move, MoveKind, Piece, PieceKind, Player, Position, square};

    use super::{Agent, ClassicalConfig, ClassicalEngine};

    #[test]
    fn classical_engine_finds_immediate_winning_push() {
        let mut position = Position::empty(square(4, 3), Player::White);
        position.put_piece(
            square(3, 3),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let winning = Move {
            from: square(3, 3),
            to: square(4, 3),
            kind: MoveKind::Push {
                ball_to: square(5, 3),
            },
        };

        let mut engine = ClassicalEngine::new(ClassicalConfig { depth: 3 });
        let decision = engine.select_move(&position).unwrap();
        assert_eq!(decision.mv, winning);
    }
}
