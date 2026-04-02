use std::collections::HashMap;

use crate::{
    engine::{Player, Position},
    record::{GameOutcome, GameRecord, MoveSource, PlyRecord, Termination},
    solver::Searcher,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TournamentConfig {
    pub games: usize,
    pub depth: u8,
    pub max_plies: usize,
    pub opening_random_plies: usize,
    pub seed: u64,
}

impl Default for TournamentConfig {
    fn default() -> Self {
        Self {
            games: 10,
            depth: 4,
            max_plies: 200,
            opening_random_plies: 0,
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSummary {
    pub game_index: usize,
    pub outcome: GameOutcome,
    pub termination: Termination,
    pub plies: usize,
    pub total_nodes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TournamentStats {
    pub games: usize,
    pub white_wins: usize,
    pub black_wins: usize,
    pub draws: usize,
    pub total_plies: usize,
    pub total_nodes: u64,
}

impl TournamentStats {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TournamentReport {
    pub config: TournamentConfig,
    pub stats: TournamentStats,
    pub games: Vec<GameSummary>,
}

impl TournamentReport {
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut out = String::from("game,result,termination,plies,total_nodes\n");
        for game in &self.games {
            out.push_str(&format!(
                "{},{},{},{},{}\n",
                game.game_index,
                game.outcome.as_result_str(),
                game.termination.as_str(),
                game.plies,
                game.total_nodes
            ));
        }
        out
    }
}

pub fn play_selfplay_game(
    depth: u8,
    max_plies: usize,
    opening_random_plies: usize,
    seed: u64,
    game_index: usize,
) -> GameRecord {
    let mut position = Position::new_game();
    let initial_position = position;
    let mut searcher = Searcher::new();
    let game_seed = seed ^ (game_index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut rng = XorShift64::new(game_seed);
    let mut moves = Vec::new();
    let mut total_plies = 0usize;
    let mut repetitions = HashMap::new();
    repetitions.insert(position, 1u8);

    loop {
        if let Some(winner) = position.winner() {
            return GameRecord {
                initial_position,
                white_label: format!("solver(depth={depth})"),
                black_label: format!("solver(depth={depth})"),
                moves,
                outcome: winner_to_outcome(winner),
                termination: Termination::GoalRow,
                final_position: position,
            };
        }
        if total_plies >= max_plies {
            return GameRecord {
                initial_position,
                white_label: format!("solver(depth={depth})"),
                black_label: format!("solver(depth={depth})"),
                moves,
                outcome: GameOutcome::Draw,
                termination: Termination::PlyLimit,
                final_position: position,
            };
        }

        let legal = position.legal_moves();
        if legal.is_empty() {
            return GameRecord {
                initial_position,
                white_label: format!("solver(depth={depth})"),
                black_label: format!("solver(depth={depth})"),
                moves,
                outcome: GameOutcome::Draw,
                termination: Termination::NoLegalMoves,
                final_position: position,
            };
        }

        let current_player = position.to_move;
        let ply_record = if total_plies < opening_random_plies {
            let selected = legal[rng.choose_index(legal.len())];
            PlyRecord {
                player: current_player,
                mv: selected,
                source: MoveSource::Opening,
                score: None,
                nodes: None,
            }
        } else {
            let result = searcher.best_move(&position, depth);
            let selected = result
                .best_move
                .unwrap_or_else(|| legal[rng.choose_index(legal.len())]);
            PlyRecord {
                player: current_player,
                mv: selected,
                source: MoveSource::Search,
                score: Some(result.score),
                nodes: Some(result.nodes),
            }
        };

        position = position.apply(ply_record.mv);
        total_plies += 1;
        moves.push(ply_record);

        let seen = repetitions.entry(position).or_insert(0);
        *seen += 1;
        if *seen >= 3 {
            return GameRecord {
                initial_position,
                white_label: format!("solver(depth={depth})"),
                black_label: format!("solver(depth={depth})"),
                moves,
                outcome: GameOutcome::Draw,
                termination: Termination::ThreefoldRepetition,
                final_position: position,
            };
        }
    }
}

#[must_use]
pub fn run_selfplay_tournament(config: TournamentConfig) -> (TournamentReport, Vec<GameRecord>) {
    let mut stats = TournamentStats::default();
    let mut summaries = Vec::with_capacity(config.games);
    let mut records = Vec::with_capacity(config.games);

    for game_index in 0..config.games {
        let record = play_selfplay_game(
            config.depth,
            config.max_plies,
            config.opening_random_plies,
            config.seed,
            game_index,
        );
        let total_nodes = record.moves.iter().filter_map(|ply| ply.nodes).sum::<u64>();
        let summary = GameSummary {
            game_index: game_index + 1,
            outcome: record.outcome,
            termination: record.termination,
            plies: record.moves.len(),
            total_nodes,
        };
        stats.games += 1;
        stats.total_plies += summary.plies;
        stats.total_nodes += total_nodes;
        match summary.outcome {
            GameOutcome::WhiteWin => stats.white_wins += 1,
            GameOutcome::BlackWin => stats.black_wins += 1,
            GameOutcome::Draw => stats.draws += 1,
        }
        summaries.push(summary);
        records.push(record);
    }

    (
        TournamentReport {
            config,
            stats,
            games: summaries,
        },
        records,
    )
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
        let state = if seed == 0 {
            0xA5A5_A5A5_A5A5_A5A5
        } else {
            seed
        };
        Self { state }
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
    use super::{
        GameOutcome, Termination, TournamentConfig, play_selfplay_game, run_selfplay_tournament,
    };

    #[test]
    fn selfplay_game_produces_replayable_record() {
        let record = play_selfplay_game(2, 20, 2, 7, 0);
        let positions = record.replay_positions().unwrap();
        assert_eq!(positions.len(), record.moves.len() + 1);
    }

    #[test]
    fn tournament_stats_sum_to_number_of_games() {
        let config = TournamentConfig {
            games: 4,
            depth: 1,
            max_plies: 30,
            opening_random_plies: 2,
            seed: 11,
        };
        let (report, records) = run_selfplay_tournament(config);
        assert_eq!(records.len(), 4);
        assert_eq!(report.stats.games, 4);
        assert_eq!(
            report.stats.white_wins + report.stats.black_wins + report.stats.draws,
            4
        );
        assert_eq!(report.games.len(), 4);
    }

    #[test]
    fn zero_random_opening_can_still_end_in_valid_result() {
        let record = play_selfplay_game(2, 10, 0, 1, 0);
        assert!(matches!(
            record.termination,
            Termination::GoalRow
                | Termination::ThreefoldRepetition
                | Termination::PlyLimit
                | Termination::NoLegalMoves
        ));
        assert!(matches!(
            record.outcome,
            GameOutcome::WhiteWin | GameOutcome::BlackWin | GameOutcome::Draw
        ));
    }
}
