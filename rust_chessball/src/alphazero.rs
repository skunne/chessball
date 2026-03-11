use std::{
    collections::{HashMap, HashSet, VecDeque, hash_map::Entry},
    fs,
    path::Path,
};

use crate::{
    agent::{Agent, EngineDecision},
    engine::{Move, MoveKind, PackedPosition, Player, Position, ROWS, Square},
    record::{GameOutcome, MoveSource, move_from_notation, move_to_notation},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaZeroConfig {
    pub simulations: usize,
    pub cpuct: f32,
    pub temperature: f32,
    pub root_noise: f32,
    pub train_games: usize,
    pub train_iterations: usize,
    pub train_max_plies: usize,
    pub replay_capacity: usize,
    pub temperature_drop_ply: usize,
    pub post_game_self_play_games: usize,
    pub seed: u64,
}

impl Default for AlphaZeroConfig {
    fn default() -> Self {
        Self {
            simulations: 96,
            cpuct: 1.35,
            temperature: 1.0,
            root_noise: 0.25,
            train_games: 32,
            train_iterations: 2,
            train_max_plies: 160,
            replay_capacity: 4096,
            temperature_drop_ply: 12,
            post_game_self_play_games: 0,
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct LearnedPosition {
    visits: usize,
    value_sum: f32,
    policy_counts: HashMap<Move, i32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SearchEdge {
    mv: Move,
    prior: f32,
    visits: u32,
    value_sum: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct SearchNode {
    edges: Vec<SearchEdge>,
}

#[derive(Debug, Clone, PartialEq)]
struct SearchOutcome {
    mv: Move,
    policy: Vec<(Move, u32)>,
    value: f32,
    nodes: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct SelfPlaySample {
    position: Position,
    policy: Vec<(Move, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
struct ReplayEntry {
    key: PackedPosition,
    value: f32,
    policy_counts: Vec<(Move, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SearchMode {
    Match,
    SelfPlay {
        root_temperature: f32,
        add_root_noise: bool,
    },
}

#[derive(Debug, Clone)]
pub struct AlphaZeroEngine {
    config: AlphaZeroConfig,
    learned: HashMap<PackedPosition, LearnedPosition>,
    replay_buffer: VecDeque<ReplayEntry>,
    current_game_samples: Vec<SelfPlaySample>,
    rng: XorShift64,
}

impl AlphaZeroEngine {
    #[must_use]
    pub fn new(config: AlphaZeroConfig) -> Self {
        let mut engine = Self {
            rng: XorShift64::new(config.seed),
            config,
            learned: HashMap::new(),
            replay_buffer: VecDeque::new(),
            current_game_samples: Vec::new(),
        };
        if config.train_games > 0 && config.train_iterations > 0 {
            for _ in 0..config.train_iterations {
                engine.train_self_play(config.train_games, config.train_max_plies);
            }
        }
        engine
    }

    #[must_use]
    pub fn learned_positions(&self) -> usize {
        self.learned.len()
    }

    pub fn from_checkpoint(path: &Path, config: AlphaZeroConfig) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|err| format!("failed to read checkpoint {}: {err}", path.display()))?;
        let mut lines = content.lines();
        let header = lines
            .next()
            .ok_or_else(|| format!("checkpoint {} is empty", path.display()))?;
        if header != "AZCKPT1" {
            return Err(format!(
                "unsupported checkpoint format '{}' in {}",
                header,
                path.display()
            ));
        }

        let rng_line = lines
            .next()
            .ok_or_else(|| format!("checkpoint {} is missing rng_state", path.display()))?;
        let rng_state = rng_line
            .strip_prefix("rng_state=")
            .ok_or_else(|| format!("invalid rng_state line '{rng_line}'"))?
            .parse::<u64>()
            .map_err(|_| format!("invalid rng_state line '{rng_line}'"))?;

        let entries_line = lines
            .next()
            .ok_or_else(|| format!("checkpoint {} is missing replay_entries", path.display()))?;
        let expected_entries = entries_line
            .strip_prefix("replay_entries=")
            .ok_or_else(|| format!("invalid replay_entries line '{entries_line}'"))?
            .parse::<usize>()
            .map_err(|_| format!("invalid replay_entries line '{entries_line}'"))?;

        let mut engine = Self {
            config,
            learned: HashMap::new(),
            replay_buffer: VecDeque::new(),
            current_game_samples: Vec::new(),
            rng: XorShift64::new(rng_state),
        };

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, '|');
            let raw = parts
                .next()
                .ok_or_else(|| format!("invalid checkpoint entry '{line}'"))?
                .parse::<u128>()
                .map_err(|_| format!("invalid packed position in '{line}'"))?;
            let value_bits = parts
                .next()
                .ok_or_else(|| format!("invalid checkpoint entry '{line}'"))?
                .parse::<u32>()
                .map_err(|_| format!("invalid value bits in '{line}'"))?;
            let policies_raw = parts.next().unwrap_or_default();
            let mut policy_counts = Vec::new();
            if !policies_raw.is_empty() {
                for token in policies_raw.split(',') {
                    let (mv_raw, count_raw) = token
                        .rsplit_once(':')
                        .ok_or_else(|| format!("invalid policy token '{token}'"))?;
                    let mv = move_from_notation(mv_raw)?;
                    let count = count_raw
                        .parse::<u32>()
                        .map_err(|_| format!("invalid visit count '{count_raw}'"))?;
                    policy_counts.push((mv, count));
                }
            }

            let replay_entry = ReplayEntry {
                key: PackedPosition::from_raw(raw),
                value: f32::from_bits(value_bits),
                policy_counts,
            };
            engine.apply_replay_entry(&replay_entry, 1);
            engine.replay_buffer.push_back(replay_entry);
        }

        if engine.replay_buffer.len() != expected_entries {
            return Err(format!(
                "checkpoint {} expected {} entries but loaded {}",
                path.display(),
                expected_entries,
                engine.replay_buffer.len()
            ));
        }

        if config.replay_capacity > 0 {
            while engine.replay_buffer.len() > config.replay_capacity {
                if let Some(old) = engine.replay_buffer.pop_front() {
                    engine.apply_replay_entry(&old, -1);
                }
            }
        }

        Ok(engine)
    }

    pub fn train_self_play(&mut self, games: usize, max_plies: usize) {
        for _ in 0..games {
            let (samples, outcome) = self.play_self_play_game(max_plies);
            self.absorb_samples(&samples, outcome);
        }
    }

    pub fn save_checkpoint(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create checkpoint directory {}: {err}",
                    parent.display()
                )
            })?;
        }

        let mut out = String::from("AZCKPT1\n");
        out.push_str(&format!("rng_state={}\n", self.rng.state));
        out.push_str(&format!("replay_entries={}\n", self.replay_buffer.len()));
        for entry in &self.replay_buffer {
            out.push_str(&entry.key.raw().to_string());
            out.push('|');
            out.push_str(&entry.value.to_bits().to_string());
            out.push('|');
            for (index, (mv, count)) in entry.policy_counts.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&move_to_notation(*mv));
                out.push(':');
                out.push_str(&count.to_string());
            }
            out.push('\n');
        }

        fs::write(path, out)
            .map_err(|err| format!("failed to write checkpoint {}: {err}", path.display()))
    }

    fn play_self_play_game(&mut self, max_plies: usize) -> (Vec<SelfPlaySample>, GameOutcome) {
        let mut position = Position::new_game();
        let mut plies = 0usize;
        let mut repetitions = HashMap::new();
        let mut samples = Vec::new();
        repetitions.insert(position, 1u8);

        loop {
            if let Some(winner) = position.winner() {
                return (samples, winner_to_outcome(winner));
            }
            if plies >= max_plies {
                return (samples, GameOutcome::Draw);
            }

            let legal = position.legal_moves();
            if legal.is_empty() {
                return (samples, GameOutcome::Draw);
            }

            let root_temperature = if plies < self.config.temperature_drop_ply {
                self.config.temperature
            } else {
                0.0
            };
            let outcome = self
                .search(
                    &position,
                    SearchMode::SelfPlay {
                        root_temperature,
                        add_root_noise: true,
                    },
                )
                .expect("self-play search requires a legal move");
            samples.push(SelfPlaySample {
                position,
                policy: outcome.policy,
            });
            position = position.apply(outcome.mv);
            plies += 1;

            let seen = repetitions.entry(position).or_insert(0);
            *seen += 1;
            if *seen >= 3 {
                return (samples, GameOutcome::Draw);
            }
        }
    }

    fn absorb_samples(&mut self, samples: &[SelfPlaySample], outcome: GameOutcome) {
        for sample in samples {
            let replay_entry = self.make_replay_entry(sample, outcome);
            self.apply_replay_entry(&replay_entry, 1);
            self.replay_buffer.push_back(replay_entry);

            if self.config.replay_capacity > 0 {
                while self.replay_buffer.len() > self.config.replay_capacity {
                    if let Some(old) = self.replay_buffer.pop_front() {
                        self.apply_replay_entry(&old, -1);
                    }
                }
            }
        }
    }

    fn make_replay_entry(&self, sample: &SelfPlaySample, outcome: GameOutcome) -> ReplayEntry {
        let value = outcome_value(outcome, sample.position.to_move);
        let (canonical, symmetry) = sample.position.canonical();
        let mut policy_counts = Vec::with_capacity(sample.policy.len());
        for (mv, count) in &sample.policy {
            policy_counts.push((mv.apply_symmetry(symmetry), (*count).max(1)));
        }

        ReplayEntry {
            key: canonical.pack(),
            value,
            policy_counts,
        }
    }

    fn apply_replay_entry(&mut self, replay_entry: &ReplayEntry, sign: i32) {
        debug_assert!(sign == -1 || sign == 1);

        let mut should_remove = false;
        {
            let learned = self.learned.entry(replay_entry.key).or_default();
            if sign > 0 {
                learned.visits = learned.visits.saturating_add(1);
                learned.value_sum += replay_entry.value;
                for (mv, count) in &replay_entry.policy_counts {
                    let policy_entry = learned.policy_counts.entry(*mv).or_insert(0);
                    *policy_entry += *count as i32;
                }
            } else {
                learned.visits = learned.visits.saturating_sub(1);
                learned.value_sum -= replay_entry.value;
                let mut exhausted_moves = Vec::new();
                for (mv, count) in &replay_entry.policy_counts {
                    if let Some(policy_entry) = learned.policy_counts.get_mut(mv) {
                        *policy_entry -= *count as i32;
                        if *policy_entry <= 0 {
                            exhausted_moves.push(*mv);
                        }
                    }
                }
                for mv in exhausted_moves {
                    learned.policy_counts.remove(&mv);
                }
                should_remove = learned.visits == 0;
            }
        }

        if should_remove {
            self.learned.remove(&replay_entry.key);
        }
    }

    fn search(&mut self, position: &Position, mode: SearchMode) -> Option<SearchOutcome> {
        if position.winner().is_some() {
            return None;
        }

        let legal = position.legal_moves();
        if legal.is_empty() {
            return None;
        }

        if let Some(mv) = position.winning_moves().into_iter().next() {
            return Some(SearchOutcome {
                mv,
                policy: vec![(mv, self.config.simulations.max(1) as u32)],
                value: 1.0,
                nodes: 1,
            });
        }

        let mut tree = HashMap::new();
        let mut nodes = 0u64;
        for _ in 0..self.config.simulations.max(1) {
            let mut path = HashSet::new();
            let _ = self.simulate(*position, &mut tree, true, mode, &mut nodes, &mut path);
        }

        let root = tree.get(position)?;
        let selected_index = self.select_root_edge(root, mode);
        let selected = root.edges[selected_index];
        let mut policy: Vec<(Move, u32)> = root
            .edges
            .iter()
            .filter_map(|edge| (edge.visits > 0).then_some((edge.mv, edge.visits)))
            .collect();
        if policy.is_empty() {
            policy.push((selected.mv, 1));
        }

        let total_visits = root.edges.iter().map(|edge| edge.visits).sum::<u32>();
        let value = if total_visits == 0 {
            self.evaluate_leaf(position)
        } else {
            root.edges.iter().map(|edge| edge.value_sum).sum::<f32>() / total_visits as f32
        };

        Some(SearchOutcome {
            mv: selected.mv,
            policy,
            value,
            nodes,
        })
    }

    fn simulate(
        &mut self,
        position: Position,
        tree: &mut HashMap<Position, SearchNode>,
        is_root: bool,
        mode: SearchMode,
        nodes: &mut u64,
        path: &mut HashSet<Position>,
    ) -> f32 {
        *nodes += 1;

        if let Some(winner) = position.winner() {
            return if winner == position.to_move {
                1.0
            } else {
                -1.0
            };
        }

        let legal = position.legal_moves();
        if legal.is_empty() {
            return 0.0;
        }

        if !path.insert(position) {
            return 0.0;
        }

        let value = match tree.entry(position) {
            Entry::Vacant(entry) => {
                let add_root_noise = matches!(
                    mode,
                    SearchMode::SelfPlay {
                        add_root_noise: true,
                        ..
                    }
                );
                let priors = self.prior_distribution(&position, &legal, is_root && add_root_noise);
                let edges = legal
                    .into_iter()
                    .zip(priors)
                    .map(|(mv, prior)| SearchEdge {
                        mv,
                        prior,
                        visits: 0,
                        value_sum: 0.0,
                    })
                    .collect();
                entry.insert(SearchNode { edges });
                self.evaluate_leaf(&position)
            }
            Entry::Occupied(_) => {
                let selected_index = {
                    let node = tree
                        .get(&position)
                        .expect("node must exist before PUCT selection");
                    self.select_puct_edge(node)
                };
                let mv = tree
                    .get(&position)
                    .expect("node must exist while reading selected edge")
                    .edges[selected_index]
                    .mv;
                let child = position.apply(mv);
                let value = -self.simulate(child, tree, false, mode, nodes, path);

                let node = tree
                    .get_mut(&position)
                    .expect("node must exist while backing up value");
                let edge = &mut node.edges[selected_index];
                edge.visits = edge.visits.saturating_add(1);
                edge.value_sum += value;
                value
            }
        };

        path.remove(&position);
        value
    }

    fn select_puct_edge(&self, node: &SearchNode) -> usize {
        let total_visits = node
            .edges
            .iter()
            .map(|edge| edge.visits)
            .sum::<u32>()
            .max(1) as f32;
        let exploration = total_visits.sqrt();

        let mut best_index = 0usize;
        let mut best_score = f32::NEG_INFINITY;
        for (index, edge) in node.edges.iter().enumerate() {
            let q = if edge.visits == 0 {
                0.0
            } else {
                edge.value_sum / edge.visits as f32
            };
            let u = self.config.cpuct * edge.prior * exploration / (1.0 + edge.visits as f32);
            let score = q + u;
            if score > best_score {
                best_score = score;
                best_index = index;
            }
        }
        best_index
    }

    fn select_root_edge(&mut self, node: &SearchNode, mode: SearchMode) -> usize {
        match mode {
            SearchMode::Match => self.argmax_root_edge(node),
            SearchMode::SelfPlay {
                root_temperature, ..
            } => {
                if root_temperature <= 0.05 {
                    return self.argmax_root_edge(node);
                }

                let mut weights = Vec::with_capacity(node.edges.len());
                let exponent = 1.0f64 / root_temperature as f64;
                for edge in &node.edges {
                    let base = if edge.visits == 0 {
                        edge.prior.max(0.001) as f64
                    } else {
                        edge.visits as f64
                    };
                    weights.push(base.powf(exponent));
                }

                self.rng
                    .choose_weighted_index(&weights)
                    .unwrap_or_else(|| self.argmax_root_edge(node))
            }
        }
    }

    fn argmax_root_edge(&self, node: &SearchNode) -> usize {
        let mut best_index = 0usize;
        let mut best_visits = 0u32;
        let mut best_q = f32::NEG_INFINITY;
        let mut best_prior = f32::NEG_INFINITY;

        for (index, edge) in node.edges.iter().enumerate() {
            let q = if edge.visits == 0 {
                0.0
            } else {
                edge.value_sum / edge.visits as f32
            };
            if edge.visits > best_visits
                || (edge.visits == best_visits && q > best_q)
                || (edge.visits == best_visits
                    && (q - best_q).abs() < 1e-6
                    && edge.prior > best_prior)
            {
                best_visits = edge.visits;
                best_q = q;
                best_prior = edge.prior;
                best_index = index;
            }
        }
        best_index
    }

    fn prior_distribution(
        &self,
        position: &Position,
        legal: &[Move],
        add_root_noise: bool,
    ) -> Vec<f32> {
        let (canonical, symmetry) = position.canonical();
        let learned = self.learned.get(&canonical.pack());

        let mut weights = Vec::with_capacity(legal.len());
        let mut total = 0.0f32;
        for mv in legal {
            let canonical_mv = mv.apply_symmetry(symmetry);
            let learned_bonus = learned
                .and_then(|entry| entry.policy_counts.get(&canonical_mv))
                .copied()
                .unwrap_or(0) as f32;
            let heuristic = self.move_prior_score(position, *mv);
            let weight = 0.05 + heuristic + 1.5 * (learned_bonus + 1.0).ln();
            let normalized = weight.max(0.01);
            weights.push(normalized);
            total += normalized;
        }

        if total <= 0.0 {
            return vec![1.0 / legal.len() as f32; legal.len()];
        }

        for weight in &mut weights {
            *weight /= total;
        }

        if add_root_noise {
            let uniform = 1.0 / legal.len() as f32;
            for weight in &mut weights {
                *weight =
                    (1.0 - self.config.root_noise) * *weight + self.config.root_noise * uniform;
            }
        }

        weights
    }

    fn move_prior_score(&self, position: &Position, mv: Move) -> f32 {
        let child = position.apply(mv);
        if child.winner() == Some(position.to_move) {
            return 20.0;
        }

        let progress_delta = (self.ball_progress(child.ball(), position.to_move)
            - self.ball_progress(position.ball(), position.to_move))
            as f32;
        let control_delta = (self.control_around_ball(&child, position.to_move) as i32
            - self.control_around_ball(position, position.to_move) as i32)
            as f32;

        let move_bonus = match mv.kind {
            MoveKind::Push { .. } => 3.5,
            MoveKind::Tackle { .. } => 1.5,
            MoveKind::Jump { .. } => 1.0,
            MoveKind::Simple => 0.25,
        };

        move_bonus + progress_delta * 1.5 + control_delta * 0.2
    }

    fn evaluate_leaf(&self, position: &Position) -> f32 {
        if let Some(winner) = position.winner() {
            return if winner == position.to_move {
                1.0
            } else {
                -1.0
            };
        }

        let heuristic = self.heuristic_value(position);
        let (canonical, _) = position.canonical();
        let Some(entry) = self.learned.get(&canonical.pack()) else {
            return heuristic;
        };
        if entry.visits == 0 {
            return heuristic;
        }

        let learned_value = entry.value_sum / entry.visits as f32;
        let learned_weight = (entry.visits as f32 / 12.0).min(0.8);
        (heuristic * (1.0 - learned_weight) + learned_value * learned_weight).clamp(-1.0, 1.0)
    }

    fn heuristic_value(&self, position: &Position) -> f32 {
        let player = position.to_move;
        let opponent = player.opponent();

        let mut opponent_position = *position;
        opponent_position.to_move = opponent;

        let player_ball = self.ball_progress(position.ball(), player) as f32;
        let opponent_ball = self.ball_progress(position.ball(), opponent) as f32;
        let mobility = position.legal_moves().len() as f32;
        let opponent_mobility = opponent_position.legal_moves().len() as f32;
        let immediate_wins = position.winning_moves().len() as f32;
        let opponent_immediate_wins = opponent_position.winning_moves().len() as f32;
        let pushers = self.adjacent_pushers(position, player) as f32
            - self.adjacent_pushers(position, opponent) as f32;
        let control = self.control_around_ball(position, player) as f32
            - self.control_around_ball(position, opponent) as f32;

        ((player_ball - opponent_ball) * 0.18
            + (mobility - opponent_mobility) * 0.02
            + (immediate_wins - opponent_immediate_wins) * 0.22
            + pushers * 0.14
            + control * 0.05)
            .clamp(-0.95, 0.95)
    }

    fn adjacent_pushers(&self, position: &Position, player: Player) -> usize {
        let ball = position.ball();
        let mut count = 0usize;
        for (dr, dc) in DIRECTIONS {
            let Some(pusher) = ball.offset(-dr, -dc) else {
                continue;
            };
            let Some(ball_to) = ball.offset(dr, dc) else {
                continue;
            };
            if Position::is_forbidden_ball_destination(ball_to) {
                continue;
            }
            if position.is_empty(ball_to)
                && let Some(piece) = position.piece_at(pusher)
                && piece.player == player
            {
                count += 1;
            }
        }
        count
    }

    fn control_around_ball(&self, position: &Position, player: Player) -> usize {
        let ball = position.ball();
        let mut count = 0usize;
        for (dr, dc) in DIRECTIONS {
            if let Some(square) = ball.offset(dr, dc)
                && let Some(piece) = position.piece_at(square)
                && piece.player == player
            {
                count += 1;
            }
        }
        count
    }

    fn ball_progress(&self, square: Square, player: Player) -> i32 {
        match player {
            Player::White => square.row() as i32,
            Player::Black => (ROWS - 1 - square.row()) as i32,
        }
    }
}

impl Agent for AlphaZeroEngine {
    fn label(&self) -> String {
        format!(
            "alphazero(sims={}, train_games={}x{}, learned={})",
            self.config.simulations,
            self.config.train_games,
            self.config.train_iterations,
            self.learned_positions()
        )
    }

    fn begin_game(&mut self, _side: Player) {
        self.current_game_samples.clear();
    }

    fn select_move(&mut self, position: &Position) -> Option<EngineDecision> {
        let outcome = self.search(position, SearchMode::Match)?;
        self.current_game_samples.push(SelfPlaySample {
            position: *position,
            policy: outcome.policy.clone(),
        });
        Some(EngineDecision {
            mv: outcome.mv,
            source: MoveSource::Mcts,
            score: Some((outcome.value * 1000.0).round() as i32),
            nodes: Some(outcome.nodes),
        })
    }

    fn end_game(&mut self, outcome: GameOutcome) {
        if !self.current_game_samples.is_empty() {
            let samples = std::mem::take(&mut self.current_game_samples);
            self.absorb_samples(&samples, outcome);
        }
        if self.config.post_game_self_play_games > 0 {
            self.train_self_play(
                self.config.post_game_self_play_games,
                self.config.train_max_plies,
            );
        }
    }

    fn save_checkpoint(&self, path: &Path) -> Result<(), String> {
        Self::save_checkpoint(self, path)
    }
}

const DIRECTIONS: [(i8, i8); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (-1, 1),
    (1, -1),
    (1, 1),
];

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

    fn next_f64(&mut self) -> f64 {
        self.next_u64() as f64 / u64::MAX as f64
    }

    fn choose_weighted_index(&mut self, weights: &[f64]) -> Option<usize> {
        let total = weights.iter().copied().sum::<f64>();
        if total <= 0.0 {
            return None;
        }

        let mut target = self.next_f64() * total;
        for (index, weight) in weights.iter().copied().enumerate() {
            if weight <= 0.0 {
                continue;
            }
            if target <= weight {
                return Some(index);
            }
            target -= weight;
        }

        weights.iter().rposition(|weight| *weight > 0.0)
    }
}

#[must_use]
fn winner_to_outcome(winner: Player) -> GameOutcome {
    match winner {
        Player::White => GameOutcome::WhiteWin,
        Player::Black => GameOutcome::BlackWin,
    }
}

#[must_use]
fn outcome_value(outcome: GameOutcome, player: Player) -> f32 {
    match outcome {
        GameOutcome::WhiteWin => {
            if player == Player::White {
                1.0
            } else {
                -1.0
            }
        }
        GameOutcome::BlackWin => {
            if player == Player::Black {
                1.0
            } else {
                -1.0
            }
        }
        GameOutcome::Draw => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        engine::{Move, MoveKind, Piece, PieceKind, Player, Position, square},
        record::GameOutcome,
    };

    use super::{Agent, AlphaZeroConfig, AlphaZeroEngine};

    #[test]
    fn alphazero_engine_returns_legal_move_from_start_position() {
        let position = Position::new_game();
        let mut engine = AlphaZeroEngine::new(AlphaZeroConfig {
            simulations: 16,
            train_games: 0,
            train_iterations: 0,
            ..AlphaZeroConfig::default()
        });

        let decision = engine.select_move(&position).unwrap();
        assert!(position.legal_moves().contains(&decision.mv));
    }

    #[test]
    fn alphazero_engine_finds_immediate_winning_push() {
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

        let mut engine = AlphaZeroEngine::new(AlphaZeroConfig {
            simulations: 8,
            train_games: 0,
            train_iterations: 0,
            ..AlphaZeroConfig::default()
        });
        let decision = engine.select_move(&position).unwrap();
        assert_eq!(decision.mv, winning);
    }

    #[test]
    fn self_play_training_populates_learning_table() {
        let engine = AlphaZeroEngine::new(AlphaZeroConfig {
            simulations: 8,
            train_games: 2,
            train_iterations: 1,
            train_max_plies: 24,
            ..AlphaZeroConfig::default()
        });
        assert!(engine.learned_positions() > 0);
    }

    #[test]
    fn deeper_training_and_search_do_not_overflow_on_repetition() {
        let position = Position::new_game();
        let mut engine = AlphaZeroEngine::new(AlphaZeroConfig {
            simulations: 64,
            train_games: 4,
            train_iterations: 1,
            train_max_plies: 40,
            ..AlphaZeroConfig::default()
        });

        let decision = engine.select_move(&position).unwrap();
        assert!(position.legal_moves().contains(&decision.mv));
    }

    #[test]
    fn completed_match_game_is_folded_back_into_learning() {
        let position = Position::new_game();
        let mut engine = AlphaZeroEngine::new(AlphaZeroConfig {
            simulations: 12,
            train_games: 0,
            train_iterations: 0,
            ..AlphaZeroConfig::default()
        });

        let learned_before = engine.learned_positions();
        engine.begin_game(Player::White);
        let _decision = engine.select_move(&position).unwrap();
        engine.end_game(GameOutcome::Draw);

        assert!(engine.learned_positions() > learned_before);
    }

    #[test]
    fn checkpoint_round_trip_restores_learning_state() {
        let position = Position::new_game();
        let mut engine = AlphaZeroEngine::new(AlphaZeroConfig {
            simulations: 12,
            train_games: 0,
            train_iterations: 0,
            ..AlphaZeroConfig::default()
        });
        engine.begin_game(Player::White);
        let _decision = engine.select_move(&position).unwrap();
        engine.end_game(GameOutcome::Draw);

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let checkpoint = env::temp_dir().join(format!(
            "chessball_az_checkpoint_{}_{}.txt",
            std::process::id(),
            unique
        ));

        engine.save_checkpoint(&checkpoint).unwrap();
        let loaded = AlphaZeroEngine::from_checkpoint(
            &checkpoint,
            AlphaZeroConfig {
                train_games: 0,
                train_iterations: 0,
                ..AlphaZeroConfig::default()
            },
        )
        .unwrap();
        fs::remove_file(&checkpoint).ok();

        assert_eq!(loaded.learned_positions(), engine.learned_positions());
        assert!(loaded.learned_positions() > 0);
    }
}
