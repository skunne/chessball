use std::collections::HashMap;

use crate::{
    agent::{Agent, EngineDecision},
    engine::{Player, Position},
    record::{GameOutcome, GameRecord, MoveSource, PlyRecord, Termination},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchConfig {
    pub max_plies: usize,
    pub opening_random_plies: usize,
    pub seed: u64,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            max_plies: 200,
            opening_random_plies: 0,
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchStats {
    pub games: usize,
    pub white_wins: usize,
    pub black_wins: usize,
    pub draws: usize,
    pub total_plies: usize,
    pub total_nodes: u64,
}

impl MatchStats {
    #[must_use]
    pub fn average_plies(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.total_plies as f64 / self.games as f64
        }
    }

    #[must_use]
    pub fn average_nodes(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.total_nodes as f64 / self.games as f64
        }
    }

    #[must_use]
    pub fn summary_text(&self) -> String {
        format!(
            "games={}, white_wins={}, black_wins={}, draws={}, avg_plies={:.2}, avg_nodes={:.2}",
            self.games,
            self.white_wins,
            self.black_wins,
            self.draws,
            self.average_plies(),
            self.average_nodes()
        )
    }

    pub fn absorb(&mut self, record: &GameRecord) {
        self.games += 1;
        self.total_plies += record.moves.len();
        self.total_nodes += record.moves.iter().filter_map(|ply| ply.nodes).sum::<u64>();
        match record.outcome {
            GameOutcome::WhiteWin => self.white_wins += 1,
            GameOutcome::BlackWin => self.black_wins += 1,
            GameOutcome::Draw => self.draws += 1,
        }
    }
}

pub fn play_game(white: &mut dyn Agent, black: &mut dyn Agent, config: MatchConfig) -> GameRecord {
    let mut rng = XorShift64::new(config.seed);
    let mut position = Position::new_game();
    let initial_position = position;
    let white_label = white.label();
    let black_label = black.label();
    white.begin_game(Player::White);
    black.begin_game(Player::Black);

    let mut repetitions = HashMap::new();
    repetitions.insert(position, 1u8);
    let mut moves = Vec::new();
    let mut plies = 0usize;

    loop {
        if let Some(winner) = position.winner() {
            return finish_game(
                white,
                black,
                GameRecord {
                    initial_position,
                    white_label,
                    black_label,
                    moves,
                    outcome: winner_to_outcome(winner),
                    termination: Termination::GoalRow,
                    final_position: position,
                },
            );
        }
        if plies >= config.max_plies {
            return finish_game(
                white,
                black,
                GameRecord {
                    initial_position,
                    white_label,
                    black_label,
                    moves,
                    outcome: GameOutcome::Draw,
                    termination: Termination::PlyLimit,
                    final_position: position,
                },
            );
        }

        let legal = position.legal_moves();
        if legal.is_empty() {
            return finish_game(
                white,
                black,
                GameRecord {
                    initial_position,
                    white_label,
                    black_label,
                    moves,
                    outcome: GameOutcome::Draw,
                    termination: Termination::NoLegalMoves,
                    final_position: position,
                },
            );
        }

        let player = position.to_move;
        let decision = if plies < config.opening_random_plies {
            EngineDecision {
                mv: legal[rng.choose_index(legal.len())],
                source: MoveSource::Opening,
                score: None,
                nodes: None,
            }
        } else {
            let agent = current_agent(white, black, player);
            let decision = agent
                .select_move(&position)
                .expect("agent returned no move despite legal moves");
            assert!(
                legal.contains(&decision.mv),
                "agent produced illegal move: {:?}",
                decision.mv
            );
            decision
        };

        position = position.apply(decision.mv);
        plies += 1;
        moves.push(PlyRecord {
            player,
            mv: decision.mv,
            source: decision.source,
            score: decision.score,
            nodes: decision.nodes,
        });

        let seen = repetitions.entry(position).or_insert(0);
        *seen += 1;
        if *seen >= 3 {
            return finish_game(
                white,
                black,
                GameRecord {
                    initial_position,
                    white_label,
                    black_label,
                    moves,
                    outcome: GameOutcome::Draw,
                    termination: Termination::ThreefoldRepetition,
                    final_position: position,
                },
            );
        }
    }
}

fn finish_game(white: &mut dyn Agent, black: &mut dyn Agent, record: GameRecord) -> GameRecord {
    white.end_game(record.outcome);
    black.end_game(record.outcome);
    record
}

fn current_agent<'a>(
    white: &'a mut dyn Agent,
    black: &'a mut dyn Agent,
    player: Player,
) -> &'a mut dyn Agent {
    match player {
        Player::White => white,
        Player::Black => black,
    }
}

#[must_use]
fn winner_to_outcome(winner: Player) -> GameOutcome {
    match winner {
        Player::White => GameOutcome::WhiteWin,
        Player::Black => GameOutcome::BlackWin,
    }
}

#[derive(Debug, Clone, Copy)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    #[must_use]
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0xA5A5_A5A5_A5A5_A5A5
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn choose_index(&mut self, len: usize) -> usize {
        assert!(len > 0);
        (self.next_u64() as usize) % len
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        agent::{ClassicalConfig, ClassicalEngine},
        alphazero::{AlphaZeroConfig, AlphaZeroEngine},
    };

    use super::{MatchConfig, play_game};

    #[test]
    fn classical_vs_alphazero_game_is_replayable() {
        let mut white = ClassicalEngine::new(ClassicalConfig { depth: 2 });
        let mut black = AlphaZeroEngine::new(AlphaZeroConfig {
            simulations: 12,
            ..AlphaZeroConfig::default()
        });

        let record = play_game(
            &mut white,
            &mut black,
            MatchConfig {
                max_plies: 40,
                opening_random_plies: 2,
                seed: 7,
            },
        );
        let replayed = record.replay_positions().unwrap();
        assert_eq!(replayed.len(), record.moves.len() + 1);
    }
}
