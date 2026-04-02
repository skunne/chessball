use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs, io,
    path::Path,
};

use crate::{
    engine::{MAX_MOVES_PER_POSITION, Move, PackedPosition, Player, Position, Symmetry},
    record::move_to_notation,
};

pub type StateId = u32;
type LossCounter = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    WhiteWin,
    BlackWin,
    Draw,
    Unknown,
}

impl Outcome {
    #[must_use]
    pub const fn for_player(player: Player) -> Self {
        match player {
            Player::White => Self::WhiteWin,
            Player::Black => Self::BlackWin,
        }
    }

    #[must_use]
    pub const fn apply_symmetry(self, symmetry: Symmetry) -> Self {
        if symmetry.swaps_colors() {
            match self {
                Self::WhiteWin => Self::BlackWin,
                Self::BlackWin => Self::WhiteWin,
                Self::Draw => Self::Draw,
                Self::Unknown => Self::Unknown,
            }
        } else {
            self
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WhiteWin => "WhiteWin",
            Self::BlackWin => "BlackWin",
            Self::Draw => "Draw",
            Self::Unknown => "Unknown",
        }
    }

    #[must_use]
    pub const fn is_proven(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofRule {
    TerminalGoal,
    SinkDraw,
    WinningMove,
    ForcedLoss,
    DrawRegion,
}

impl ProofRule {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TerminalGoal => "TerminalGoal",
            Self::SinkDraw => "SinkDraw",
            Self::WinningMove => "WinningMove",
            Self::ForcedLoss => "ForcedLoss",
            Self::DrawRegion => "DrawRegion",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub mv: Move,
    pub to: StateId,
    pub child_symmetry: Symmetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentLink {
    pub from: StateId,
    pub mv: Move,
    pub child_symmetry: Symmetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PartialTablebaseConfig {
    pub max_states: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportConfig {
    pub dot_max_nodes: usize,
    pub dot_max_depth: usize,
    pub certified_per_outcome: usize,
    pub min_proof_plies: usize,
    pub prefer_long_proofs: bool,
    pub export_full_graph: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            dot_max_nodes: 200,
            dot_max_depth: 6,
            certified_per_outcome: 5,
            min_proof_plies: 0,
            prefer_long_proofs: false,
            export_full_graph: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathSelectionConfig {
    pub limit_per_outcome: usize,
    pub min_proof_plies: usize,
    pub prefer_long_proofs: bool,
}

impl Default for PathSelectionConfig {
    fn default() -> Self {
        Self {
            limit_per_outcome: 5,
            min_proof_plies: 0,
            prefer_long_proofs: false,
        }
    }
}

#[derive(Debug)]
pub struct PartialTablebase {
    pub start: StateId,
    pub start_symmetry: Symmetry,
    pub state_keys: Vec<PackedPosition>,
    pub to_move: Vec<Player>,
    pub winners: Vec<Option<Player>>,
    pub parent: Vec<Option<ParentLink>>,
    pub path_symmetry: Vec<Symmetry>,
    pub closed: Vec<bool>,
    pub depths: Vec<u32>,
    pub succs: Vec<Vec<Edge>>,
    pub preds: Vec<Vec<StateId>>,
    pub max_successors_per_state: LossCounter,
}

impl PartialTablebase {
    #[must_use]
    pub fn state_count(&self) -> usize {
        self.state_keys.len()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.succs.iter().map(Vec::len).sum()
    }

    #[must_use]
    pub fn canonical_position(&self, state: StateId) -> Position {
        self.state_keys[idx(state)].unpack()
    }

    #[must_use]
    pub fn actual_position(&self, state: StateId) -> Position {
        self.canonical_position(state)
            .apply_symmetry(self.path_symmetry[idx(state)])
    }

    #[must_use]
    pub fn successors(&self, state: StateId) -> &[Edge] {
        &self.succs[idx(state)]
    }

    #[must_use]
    pub fn predecessors(&self, state: StateId) -> &[StateId] {
        &self.preds[idx(state)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PartialStats {
    pub states: usize,
    pub closed_states: usize,
    pub open_states: usize,
    pub edges: usize,
    pub max_successors_per_state: LossCounter,
    pub proven_white_wins: usize,
    pub proven_black_wins: usize,
    pub proven_draws: usize,
    pub unknown_states: usize,
}

#[derive(Debug)]
pub struct PartialTablebaseResult {
    pub graph: PartialTablebase,
    pub outcomes: Vec<Outcome>,
    pub stats: PartialStats,
    pub exact: bool,
}

#[derive(Debug, Clone)]
pub struct CertifiedPath {
    pub target_state: StateId,
    pub depth: u32,
    pub proof_plies: usize,
    pub outcome: Outcome,
    pub states: Vec<StateId>,
    pub line: Vec<(Position, Outcome, Option<Move>)>,
}

impl CertifiedPath {
    #[must_use]
    pub fn move_sequence(&self) -> Vec<Move> {
        self.line.iter().filter_map(|(_, _, mv)| *mv).collect()
    }
}

impl PartialTablebaseResult {
    #[must_use]
    pub fn start_outcome(&self) -> Outcome {
        self.outcomes[idx(self.graph.start)].apply_symmetry(self.graph.start_symmetry)
    }

    #[must_use]
    pub fn actual_outcome(&self, state: StateId) -> Outcome {
        self.outcomes[idx(state)].apply_symmetry(self.graph.path_symmetry[idx(state)])
    }

    #[must_use]
    pub fn recommended_move(&self, state: StateId) -> Option<Move> {
        self.recommended_edge(state).map(|edge| edge.mv)
    }

    #[must_use]
    pub fn proof_rule(&self, state: StateId) -> Option<ProofRule> {
        let state_index = idx(state);
        let outcome = self.outcomes[state_index];
        if !outcome.is_proven() {
            return None;
        }
        if self.graph.winners[state_index].is_some() {
            return Some(ProofRule::TerminalGoal);
        }
        if self.graph.succs[state_index].is_empty() {
            return Some(ProofRule::SinkDraw);
        }

        let mover = self.graph.to_move[state_index];
        let mover_win = Outcome::for_player(mover);
        let opponent_win = Outcome::for_player(mover.opponent());
        if outcome == mover_win {
            Some(ProofRule::WinningMove)
        } else if outcome == opponent_win {
            Some(ProofRule::ForcedLoss)
        } else {
            Some(ProofRule::DrawRegion)
        }
    }

    #[must_use]
    pub fn proof_summary(&self, state: StateId) -> Option<String> {
        let state_index = idx(state);
        let outcome = self.actual_outcome(state);
        let rule = self.proof_rule(state)?;
        let mover = self.graph.to_move[state_index];
        let mover_name = player_name(mover);
        let winner_name = match outcome {
            Outcome::WhiteWin => player_name(Player::White),
            Outcome::BlackWin => player_name(Player::Black),
            Outcome::Draw | Outcome::Unknown => "",
        };

        Some(match rule {
            ProofRule::TerminalGoal => format!(
                "Terminal state: the ball is already on {}'s goal row.",
                winner_name
            ),
            ProofRule::SinkDraw => String::from(
                "Terminal draw in the analysis model: no legal moves and no goal has been scored.",
            ),
            ProofRule::WinningMove => {
                let witness = self
                    .recommended_edge(state)
                    .expect("winning states must have a witness move");
                let actual_move = move_to_notation(
                    witness
                        .mv
                        .apply_symmetry(self.graph.path_symmetry[state_index]),
                );
                format!(
                    "{} to move has a proof-winning move: {} leads to state #{} which is also {}.",
                    mover_name,
                    actual_move,
                    witness.to,
                    outcome.as_str()
                )
            }
            ProofRule::ForcedLoss => format!(
                "{} to move, and every legal move leads to {}.",
                mover_name,
                outcome.as_str()
            ),
            ProofRule::DrawRegion => {
                let draw_moves = self
                    .draw_preserving_moves(state)
                    .into_iter()
                    .map(|(mv, to)| format!("{} -> #{}", move_to_notation(mv), to))
                    .collect::<Vec<_>>();
                format!(
                    "{} to move cannot force a win from this closed region, and at least one move preserves Draw: {}.",
                    mover_name,
                    draw_moves.join(", ")
                )
            }
        })
    }

    #[must_use]
    pub fn proof_children(&self, state: StateId) -> Vec<(Move, StateId, Outcome)> {
        let orientation = self.graph.path_symmetry[idx(state)];
        self.graph
            .successors(state)
            .iter()
            .map(|edge| {
                (
                    edge.mv.apply_symmetry(orientation),
                    edge.to,
                    self.actual_outcome(edge.to),
                )
            })
            .collect()
    }

    #[must_use]
    pub fn line_from_start(&self, max_plies: usize) -> Vec<(Position, Outcome, Option<Move>)> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut state = self.graph.start;
        let mut orientation = self.graph.start_symmetry;

        for _ in 0..max_plies {
            let canonical_position = self.graph.canonical_position(state);
            let actual_position = canonical_position.apply_symmetry(orientation);
            let outcome = self.outcomes[idx(state)].apply_symmetry(orientation);
            let edge = self.recommended_edge(state);
            let actual_move = edge.map(|edge| edge.mv.apply_symmetry(orientation));

            out.push((actual_position, outcome, actual_move));

            let Some(edge) = edge else {
                break;
            };

            let next_state = edge.to;
            let next_orientation = orientation.combine(edge.child_symmetry);
            if !seen.insert((state, orientation)) {
                break;
            }
            state = next_state;
            orientation = next_orientation;
        }

        out
    }

    #[must_use]
    pub fn path_from_start_to(&self, target: StateId) -> Vec<(Position, Outcome, Option<Move>)> {
        let states = self.state_chain_to(target);
        let mut out = Vec::with_capacity(states.len());

        for (index, &state) in states.iter().enumerate() {
            let position = self.graph.actual_position(state);
            let outcome = self.actual_outcome(state);
            let actual_move = states
                .get(index + 1)
                .and_then(|&next| self.graph.parent[idx(next)])
                .map(|parent| {
                    parent
                        .mv
                        .apply_symmetry(self.graph.path_symmetry[idx(state)])
                });
            out.push((position, outcome, actual_move));
        }

        out
    }

    #[must_use]
    pub fn proven_paths(&self, limit_per_outcome: usize) -> Vec<CertifiedPath> {
        self.proven_paths_with_config(PathSelectionConfig {
            limit_per_outcome,
            ..PathSelectionConfig::default()
        })
    }

    #[must_use]
    pub fn certified_entry_paths(&self, limit_per_outcome: usize) -> Vec<CertifiedPath> {
        self.certified_entry_paths_with_config(PathSelectionConfig {
            limit_per_outcome,
            ..PathSelectionConfig::default()
        })
    }

    #[must_use]
    pub fn proven_paths_with_config(&self, config: PathSelectionConfig) -> Vec<CertifiedPath> {
        self.collect_paths(config, false)
    }

    #[must_use]
    pub fn certified_entry_paths_with_config(
        &self,
        config: PathSelectionConfig,
    ) -> Vec<CertifiedPath> {
        self.collect_paths(config, true)
    }

    #[must_use]
    fn collect_paths(&self, config: PathSelectionConfig, entry_only: bool) -> Vec<CertifiedPath> {
        let mut white = Vec::new();
        let mut black = Vec::new();
        let mut draws = Vec::new();

        for state in 0..self.graph.state_count() {
            let state_id = as_state_id(state);
            let outcome = self.actual_outcome(state_id);
            if !outcome.is_proven() {
                continue;
            }

            let parent_is_proven = self.graph.parent[state]
                .map(|parent| self.actual_outcome(parent.from).is_proven())
                .unwrap_or(false);
            if entry_only && parent_is_proven {
                continue;
            }

            let (states, line, proof_plies) = self.proof_path_from_start_to(state_id);
            if proof_plies < config.min_proof_plies {
                continue;
            }

            let path = CertifiedPath {
                target_state: state_id,
                depth: self.graph.depths[state],
                proof_plies,
                outcome,
                states,
                line,
            };

            match outcome {
                Outcome::WhiteWin => white.push(path),
                Outcome::BlackWin => black.push(path),
                Outcome::Draw => draws.push(path),
                Outcome::Unknown => {}
            }
        }

        for group in [&mut white, &mut black, &mut draws] {
            if config.prefer_long_proofs {
                group.sort_by_key(|path| {
                    (
                        std::cmp::Reverse(path.proof_plies),
                        path.depth,
                        path.target_state,
                    )
                });
            } else {
                group.sort_by_key(|path| (path.depth, path.target_state));
            }
            group.truncate(config.limit_per_outcome);
        }

        let mut out = Vec::with_capacity(white.len() + black.len() + draws.len());
        out.extend(white);
        out.extend(black);
        out.extend(draws);
        out
    }

    #[must_use]
    fn recommended_edge(&self, state: StateId) -> Option<Edge> {
        let state_index = idx(state);
        let mover = self.graph.to_move[state_index];
        let mover_win = Outcome::for_player(mover);
        let opponent_win = Outcome::for_player(mover.opponent());
        let state_outcome = self.outcomes[state_index];
        let edges = self.graph.successors(state);

        match state_outcome {
            outcome if outcome == mover_win => edges
                .iter()
                .copied()
                .find(|edge| self.outcomes[idx(edge.to)] == mover_win),
            Outcome::Draw => edges
                .iter()
                .copied()
                .find(|edge| self.outcomes[idx(edge.to)] == Outcome::Draw),
            outcome if outcome == opponent_win => edges
                .iter()
                .copied()
                .find(|edge| self.outcomes[idx(edge.to)] == opponent_win)
                .or_else(|| edges.first().copied()),
            Outcome::Unknown => None,
            _ => None,
        }
    }

    #[must_use]
    fn draw_preserving_moves(&self, state: StateId) -> Vec<(Move, StateId)> {
        let orientation = self.graph.path_symmetry[idx(state)];
        self.graph
            .successors(state)
            .iter()
            .filter(|edge| self.outcomes[idx(edge.to)] == Outcome::Draw)
            .map(|edge| (edge.mv.apply_symmetry(orientation), edge.to))
            .collect()
    }

    #[must_use]
    fn state_chain_to(&self, target: StateId) -> Vec<StateId> {
        let mut states = vec![target];
        let mut cursor = target;
        while let Some(parent) = self.graph.parent[idx(cursor)] {
            states.push(parent.from);
            cursor = parent.from;
        }
        states.reverse();
        states
    }

    #[must_use]
    fn proof_path_from_start_to(
        &self,
        target: StateId,
    ) -> (Vec<StateId>, Vec<(Position, Outcome, Option<Move>)>, usize) {
        let mut prefix_states = self.state_chain_to(target);
        let mut prefix_line = self.path_from_start_to(target);
        let (mut proof_states, mut proof_line) = self.proof_trace_from(target);
        let proof_plies = proof_line.iter().filter(|(_, _, mv)| mv.is_some()).count();

        if !prefix_states.is_empty() {
            prefix_states.pop();
        }
        if !prefix_line.is_empty() {
            prefix_line.pop();
        }

        prefix_states.append(&mut proof_states);
        prefix_line.append(&mut proof_line);
        (prefix_states, prefix_line, proof_plies)
    }

    #[must_use]
    fn proof_trace_from(
        &self,
        start: StateId,
    ) -> (Vec<StateId>, Vec<(Position, Outcome, Option<Move>)>) {
        let mut states = Vec::new();
        let mut line = Vec::new();
        let mut seen = HashSet::new();
        let mut state = start;
        let mut orientation = self.graph.path_symmetry[idx(start)];

        loop {
            let canonical_position = self.graph.canonical_position(state);
            let actual_position = canonical_position.apply_symmetry(orientation);
            let outcome = self.outcomes[idx(state)].apply_symmetry(orientation);

            if !seen.insert((state, orientation)) {
                states.push(state);
                line.push((actual_position, outcome, None));
                break;
            }

            let edge = self.recommended_edge(state);
            let actual_move = edge.map(|edge| edge.mv.apply_symmetry(orientation));
            states.push(state);
            line.push((actual_position, outcome, actual_move));

            let Some(edge) = edge else {
                break;
            };

            state = edge.to;
            orientation = orientation.combine(edge.child_symmetry);
        }

        (states, line)
    }
}

#[must_use]
pub fn build_start(config: PartialTablebaseConfig) -> PartialTablebaseResult {
    build_position(Position::new_game(), config)
}

#[must_use]
pub fn build_position(start: Position, config: PartialTablebaseConfig) -> PartialTablebaseResult {
    let (start_position, start_symmetry) = start.canonical_color_preserving();
    let start_key = start_position.pack();

    let mut ids = HashMap::new();
    ids.insert(start_key, 0u32);

    let mut graph = PartialTablebase {
        start: 0,
        start_symmetry,
        state_keys: vec![start_key],
        to_move: vec![start_position.to_move],
        winners: vec![start_position.winner()],
        parent: vec![None],
        path_symmetry: vec![start_symmetry],
        closed: vec![false],
        depths: vec![0],
        succs: vec![Vec::new()],
        preds: vec![Vec::new()],
        max_successors_per_state: 0,
    };

    let mut cursor = 0usize;
    while cursor < graph.state_keys.len() {
        try_expand_state(&mut graph, &mut ids, cursor, config.max_states);
        cursor += 1;
    }

    close_frontier_states(&mut graph, &ids);

    let outcomes = prove_outcomes(&graph);
    let exact = graph.closed.iter().all(|&closed| closed);
    let stats = compute_stats(&graph, &outcomes);

    PartialTablebaseResult {
        graph,
        outcomes,
        stats,
        exact,
    }
}

pub fn export_to_dir(
    result: &PartialTablebaseResult,
    out_dir: &Path,
    config: ExportConfig,
) -> io::Result<()> {
    let certified_paths = result.proven_paths_with_config(PathSelectionConfig {
        limit_per_outcome: config.certified_per_outcome,
        min_proof_plies: config.min_proof_plies,
        prefer_long_proofs: config.prefer_long_proofs,
    });
    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join("summary.txt"), summary_text(result))?;
    if config.export_full_graph {
        fs::write(out_dir.join("states.csv"), states_csv(result))?;
        fs::write(out_dir.join("edges.csv"), edges_csv(result))?;
        fs::write(out_dir.join("graph.dot"), dot_graph(result, config))?;
    }
    fs::write(
        out_dir.join("certified_paths.txt"),
        certified_paths_text(&certified_paths),
    )?;
    fs::write(
        out_dir.join("certified_paths.csv"),
        certified_paths_csv(&certified_paths),
    )?;
    fs::write(
        out_dir.join("proof_positions.txt"),
        proof_positions_text(result, &certified_paths),
    )?;
    fs::write(
        out_dir.join("proof_positions.csv"),
        proof_positions_csv(result, &certified_paths),
    )?;
    fs::write(
        out_dir.join("proof_paths.dot"),
        proof_paths_dot(&certified_paths),
    )?;
    fs::write(
        out_dir.join("browser.html"),
        proof_browser_html(result, &certified_paths),
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct PendingState {
    key: PackedPosition,
    to_move: Player,
    winner: Option<Player>,
    depth: u32,
    parent: ParentLink,
    path_symmetry: Symmetry,
}

fn try_expand_state(
    graph: &mut PartialTablebase,
    ids: &mut HashMap<PackedPosition, StateId>,
    state: usize,
    max_states: Option<usize>,
) {
    if graph.closed[state] {
        return;
    }
    if graph.winners[state].is_some() {
        graph.closed[state] = true;
        return;
    }

    let state_id = as_state_id(state);
    let position = graph.state_keys[state].unpack();
    let mut temp_edges = Vec::new();
    let mut pending = Vec::<PendingState>::new();

    for mv in position.legal_moves() {
        let child = position.apply(mv);
        let (child, child_symmetry) = child.canonical_color_preserving();
        let child_key = child.pack();

        let child_id = if let Some(&existing) = ids.get(&child_key) {
            existing
        } else if let Some(local_index) = pending.iter().position(|entry| entry.key == child_key) {
            as_state_id(graph.state_keys.len() + local_index)
        } else if let Some(limit) = max_states {
            if graph.state_keys.len() + pending.len() >= limit {
                return;
            }
            pending.push(PendingState {
                key: child_key,
                to_move: child.to_move,
                winner: child.winner(),
                depth: graph.depths[state] + 1,
                parent: ParentLink {
                    from: state_id,
                    mv,
                    child_symmetry,
                },
                path_symmetry: graph.path_symmetry[state].combine(child_symmetry),
            });
            as_state_id(graph.state_keys.len() + pending.len() - 1)
        } else {
            pending.push(PendingState {
                key: child_key,
                to_move: child.to_move,
                winner: child.winner(),
                depth: graph.depths[state] + 1,
                parent: ParentLink {
                    from: state_id,
                    mv,
                    child_symmetry,
                },
                path_symmetry: graph.path_symmetry[state].combine(child_symmetry),
            });
            as_state_id(graph.state_keys.len() + pending.len() - 1)
        };

        temp_edges.push(Edge {
            mv,
            to: child_id,
            child_symmetry,
        });
    }

    for entry in pending {
        let next_id = as_state_id(graph.state_keys.len());
        ids.insert(entry.key, next_id);
        graph.state_keys.push(entry.key);
        graph.to_move.push(entry.to_move);
        graph.winners.push(entry.winner);
        graph.parent.push(Some(entry.parent));
        graph.path_symmetry.push(entry.path_symmetry);
        graph.closed.push(false);
        graph.depths.push(entry.depth);
        graph.succs.push(Vec::new());
        graph.preds.push(Vec::new());
    }

    commit_edges(graph, state_id, temp_edges);
}

fn close_frontier_states(graph: &mut PartialTablebase, ids: &HashMap<PackedPosition, StateId>) {
    for state in 0..graph.state_keys.len() {
        if graph.closed[state] {
            continue;
        }

        let position = graph.state_keys[state].unpack();
        let mut temp_edges = Vec::new();
        let mut complete = true;
        for mv in position.legal_moves() {
            let child = position.apply(mv);
            let (child, child_symmetry) = child.canonical_color_preserving();
            let child_key = child.pack();
            let Some(&child_id) = ids.get(&child_key) else {
                complete = false;
                break;
            };
            temp_edges.push(Edge {
                mv,
                to: child_id,
                child_symmetry,
            });
        }

        if complete {
            commit_edges(graph, as_state_id(state), temp_edges);
        }
    }
}

fn commit_edges(graph: &mut PartialTablebase, state_id: StateId, edges: Vec<Edge>) {
    graph.max_successors_per_state = graph
        .max_successors_per_state
        .max(as_loss_counter(edges.len()));
    for edge in &edges {
        graph.preds[idx(edge.to)].push(state_id);
    }
    graph.succs[idx(state_id)] = edges;
    graph.closed[idx(state_id)] = true;
}

fn prove_outcomes(graph: &PartialTablebase) -> Vec<Outcome> {
    let mut outcomes = vec![Outcome::Unknown; graph.state_count()];
    let mut remaining_for_loss = vec![0u8; graph.state_count()];
    let mut queue = VecDeque::new();

    for state in 0..graph.state_count() {
        if !graph.closed[state] {
            continue;
        }

        remaining_for_loss[state] = as_loss_counter(graph.succs[state].len());

        if let Some(winner) = graph.winners[state] {
            outcomes[state] = Outcome::for_player(winner);
            queue.push_back(as_state_id(state));
        } else if graph.succs[state].is_empty() {
            outcomes[state] = Outcome::Draw;
            queue.push_back(as_state_id(state));
        }
    }

    while let Some(child) = queue.pop_front() {
        let child_outcome = outcomes[idx(child)];
        for &pred in graph.predecessors(child) {
            let pred_index = idx(pred);
            if !graph.closed[pred_index] || outcomes[pred_index].is_proven() {
                continue;
            }

            let mover = graph.to_move[pred_index];
            let mover_win = Outcome::for_player(mover);
            let opponent_win = Outcome::for_player(mover.opponent());

            if child_outcome == mover_win {
                outcomes[pred_index] = mover_win;
                queue.push_back(pred);
            } else if child_outcome == opponent_win {
                remaining_for_loss[pred_index] -= 1;
                if remaining_for_loss[pred_index] == 0 {
                    outcomes[pred_index] = opponent_win;
                    queue.push_back(pred);
                }
            }
        }
    }

    let mut draw_candidates = vec![false; graph.state_count()];
    for state in 0..graph.state_count() {
        draw_candidates[state] = graph.closed[state] && outcomes[state] == Outcome::Unknown;
    }

    let mut changed = true;
    while changed {
        changed = false;
        for state in 0..graph.state_count() {
            if !draw_candidates[state] {
                continue;
            }

            let mover = graph.to_move[state];
            let mover_win = Outcome::for_player(mover);
            let opponent_win = Outcome::for_player(mover.opponent());
            let mut has_draw_successor = false;
            let mut valid = true;

            for edge in &graph.succs[state] {
                let child = idx(edge.to);
                if draw_candidates[child] || outcomes[child] == Outcome::Draw {
                    has_draw_successor = true;
                } else if outcomes[child] == opponent_win {
                    // The mover can avoid this losing branch if a draw-preserving move exists.
                } else if outcomes[child] == mover_win {
                    valid = false;
                    break;
                } else {
                    // Unknown or open children can still hide a winning deviation for the mover.
                    valid = false;
                    break;
                }
            }

            if !valid || !has_draw_successor {
                draw_candidates[state] = false;
                changed = true;
            }
        }
    }

    for state in 0..graph.state_count() {
        if draw_candidates[state] {
            outcomes[state] = Outcome::Draw;
        }
    }

    outcomes
}

fn compute_stats(graph: &PartialTablebase, outcomes: &[Outcome]) -> PartialStats {
    let mut stats = PartialStats {
        states: graph.state_count(),
        edges: graph.edge_count(),
        max_successors_per_state: graph.max_successors_per_state,
        ..PartialStats::default()
    };

    for (state, &outcome) in outcomes.iter().enumerate() {
        if graph.closed[state] {
            stats.closed_states += 1;
        } else {
            stats.open_states += 1;
        }

        match outcome {
            Outcome::WhiteWin => stats.proven_white_wins += 1,
            Outcome::BlackWin => stats.proven_black_wins += 1,
            Outcome::Draw => stats.proven_draws += 1,
            Outcome::Unknown => stats.unknown_states += 1,
        }
    }

    stats
}

fn summary_text(result: &PartialTablebaseResult) -> String {
    format!(
        "exact={}\nstart_outcome={}\nstates={}\nclosed_states={}\nopen_states={}\nedges={}\nmax_successors_per_state={}\nproven_white_wins={}\nproven_black_wins={}\nproven_draws={}\nunknown_states={}\nproof_model=infinite_play_is_draw\n",
        result.exact,
        result.start_outcome().as_str(),
        result.stats.states,
        result.stats.closed_states,
        result.stats.open_states,
        result.stats.edges,
        result.stats.max_successors_per_state,
        result.stats.proven_white_wins,
        result.stats.proven_black_wins,
        result.stats.proven_draws,
        result.stats.unknown_states
    )
}

fn states_csv(result: &PartialTablebaseResult) -> String {
    let mut out = String::from(
        "id,depth,to_move,closed,canonical_outcome,actual_outcome,proof_rule,proof_summary,parent,parent_move,terminal_winner,successors,predecessors,canonical_board,actual_board\n",
    );
    for state in 0..result.graph.state_count() {
        let state_id = as_state_id(state);
        let position = result.graph.canonical_position(state_id);
        let actual_position = result.graph.actual_position(state_id);
        let proof_rule = result
            .proof_rule(state_id)
            .map(ProofRule::as_str)
            .unwrap_or_default();
        let proof_summary = result.proof_summary(state_id).unwrap_or_default();
        let winner = result.graph.winners[state]
            .map(|player| player.to_char().to_string())
            .unwrap_or_default();
        let parent = result.graph.parent[state]
            .map(|parent| parent.from.to_string())
            .unwrap_or_default();
        let parent_move = result.graph.parent[state]
            .map(|parent| {
                move_to_notation(
                    parent
                        .mv
                        .apply_symmetry(result.graph.path_symmetry[idx(parent.from)]),
                )
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            state,
            result.graph.depths[state],
            result.graph.to_move[state].to_char(),
            result.graph.closed[state],
            result.outcomes[state].as_str(),
            result.actual_outcome(state_id).as_str(),
            proof_rule,
            csv_escape(&proof_summary),
            parent,
            csv_escape(&parent_move),
            winner,
            result.graph.succs[state].len(),
            result.graph.preds[state].len(),
            csv_escape(&board_one_line(position)),
            csv_escape(&board_one_line(actual_position))
        ));
    }
    out
}

fn edges_csv(result: &PartialTablebaseResult) -> String {
    let mut out = String::from("from,to,move,child_symmetry\n");
    for state in 0..result.graph.state_count() {
        for edge in &result.graph.succs[state] {
            out.push_str(&format!(
                "{},{},{},{}\n",
                state,
                edge.to,
                csv_escape(&move_to_notation(edge.mv)),
                symmetry_name(edge.child_symmetry)
            ));
        }
    }
    out
}

fn dot_graph(result: &PartialTablebaseResult, config: ExportConfig) -> String {
    let states = select_dot_states(result, config.dot_max_nodes, config.dot_max_depth);
    let included = states.iter().copied().collect::<HashSet<_>>();

    let mut out = String::new();
    out.push_str("digraph partial_tablebase {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box fontname=\"Menlo\"];\n");

    for state in &states {
        let index = idx(*state);
        let position = result.graph.canonical_position(*state);
        let outcome = result.outcomes[index];
        let fill = outcome_fill(outcome);
        let line_style = if result.graph.closed[index] {
            "rounded,filled"
        } else {
            "rounded,filled,dashed"
        };
        let label = format!(
            "#{} | {} | {} | to_move={} | depth={}\\l{}\\l",
            state,
            outcome.as_str(),
            if result.graph.closed[index] {
                "closed"
            } else {
                "open"
            },
            result.graph.to_move[index].to_char(),
            result.graph.depths[index],
            dot_board_label(position)
        );
        out.push_str(&format!(
            "  n{} [label=\"{}\" style=\"{}\" fillcolor=\"{}\"{}];\n",
            state,
            label,
            line_style,
            fill,
            if *state == result.graph.start {
                " penwidth=2"
            } else {
                ""
            }
        ));
    }

    for state in &states {
        for edge in result.graph.successors(*state) {
            if !included.contains(&edge.to) {
                continue;
            }
            out.push_str(&format!(
                "  n{} -> n{} [label=\"{}\"];\n",
                state,
                edge.to,
                move_to_notation(edge.mv)
            ));
        }
    }

    out.push_str("}\n");
    out
}

fn certified_paths_text(paths: &[CertifiedPath]) -> String {
    let mut out = String::new();
    let mut current_outcome = None;

    for path in paths {
        if current_outcome != Some(path.outcome) {
            if !out.is_empty() {
                out.push('\n');
            }
            current_outcome = Some(path.outcome);
            out.push_str(&format!("[{}]\n", path.outcome.as_str()));
        }

        out.push_str(&format!(
            "state={} depth={} proof_plies={} total_plies={}\n",
            path.target_state,
            path.depth,
            path.proof_plies,
            path.move_sequence().len()
        ));
        let moves = path
            .move_sequence()
            .into_iter()
            .map(move_to_notation)
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            "sequence={}\n",
            if moves.is_empty() { "<none>" } else { &moves }
        ));
        if let Some((position, _, _)) = path.line.last() {
            out.push_str(&format!("{position}\n"));
        }
    }

    if out.is_empty() {
        out.push_str("[none]\n");
    }

    out
}

fn certified_paths_csv(paths: &[CertifiedPath]) -> String {
    let mut out =
        String::from("state,depth,outcome,proof_plies,total_plies,sequence,final_board\n");
    for path in paths {
        let sequence = path
            .move_sequence()
            .into_iter()
            .map(move_to_notation)
            .collect::<Vec<_>>()
            .join(" ");
        let final_board = path
            .line
            .last()
            .map(|(position, _, _)| board_one_line(*position))
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            path.target_state,
            path.depth,
            path.outcome.as_str(),
            path.proof_plies,
            path.move_sequence().len(),
            csv_escape(&sequence),
            csv_escape(&final_board)
        ));
    }
    out
}

fn proof_positions_text(result: &PartialTablebaseResult, paths: &[CertifiedPath]) -> String {
    if paths.is_empty() {
        return String::from("[none]\n");
    }

    let mut out = String::new();
    for path in paths {
        let state = path.target_state;
        let outcome = result.actual_outcome(state);
        let rule = result
            .proof_rule(state)
            .map(ProofRule::as_str)
            .unwrap_or("Unknown");
        let summary = result
            .proof_summary(state)
            .unwrap_or_else(|| String::from("No proof available."));
        out.push_str(&format!(
            "state={} outcome={} rule={}\n{}\n",
            state,
            outcome.as_str(),
            rule,
            summary
        ));
        for (mv, to, child_outcome) in result.proof_children(state) {
            out.push_str(&format!(
                "  {} -> state #{} ({})\n",
                move_to_notation(mv),
                to,
                child_outcome.as_str()
            ));
        }
        out.push('\n');
    }
    out
}

fn proof_positions_csv(result: &PartialTablebaseResult, paths: &[CertifiedPath]) -> String {
    let mut out = String::from("state,outcome,proof_rule,proof_summary,children\n");
    for path in paths {
        let state = path.target_state;
        let summary = result.proof_summary(state).unwrap_or_default();
        let children = result
            .proof_children(state)
            .into_iter()
            .map(|(mv, to, outcome)| {
                format!("{} -> #{} ({})", move_to_notation(mv), to, outcome.as_str())
            })
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            state,
            result.actual_outcome(state).as_str(),
            result
                .proof_rule(state)
                .map(ProofRule::as_str)
                .unwrap_or("Unknown"),
            csv_escape(&summary),
            csv_escape(&children)
        ));
    }
    out
}

fn proof_paths_dot(paths: &[CertifiedPath]) -> String {
    let mut nodes = HashMap::<StateId, (Position, Outcome, bool)>::new();
    let mut edges = Vec::new();

    for path in paths {
        for (index, &state) in path.states.iter().enumerate() {
            let (position, outcome, _) = path.line[index];
            let is_target = state == path.target_state;
            nodes
                .entry(state)
                .and_modify(|entry| entry.2 |= is_target)
                .or_insert((position, outcome, is_target));
            if let Some((_, _, Some(mv))) = path.line.get(index) {
                if let Some(&next) = path.states.get(index + 1) {
                    edges.push((state, next, *mv));
                }
            }
        }
    }

    let mut ordered_states = nodes.keys().copied().collect::<Vec<_>>();
    ordered_states.sort_unstable();

    let mut out = String::new();
    out.push_str("digraph certified_paths {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box fontname=\"Menlo\"];\n");

    for state in ordered_states {
        let (position, outcome, is_target) = nodes[&state];
        out.push_str(&format!(
            "  n{} [label=\"#{} | {}\\l{}\\l\" style=\"rounded,filled\" fillcolor=\"{}\"{}];\n",
            state,
            state,
            outcome.as_str(),
            dot_board_label(position),
            outcome_fill(outcome),
            if is_target { " penwidth=2" } else { "" }
        ));
    }

    edges.sort_by_key(|(from, to, _)| (*from, *to));
    edges.dedup_by_key(|(from, to, _)| (*from, *to));
    for (from, to, mv) in edges {
        out.push_str(&format!(
            "  n{} -> n{} [label=\"{}\"];\n",
            from,
            to,
            move_to_notation(mv)
        ));
    }

    out.push_str("}\n");
    out
}

fn proof_browser_html(result: &PartialTablebaseResult, paths: &[CertifiedPath]) -> String {
    let data = format!(
        "[{}]",
        paths
            .iter()
            .map(|path| js_path(result, path))
            .collect::<Vec<_>>()
            .join(",\n")
    );

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ChessBall Partial Tablebase Browser</title>
    <style>
      :root {{
        --paper: #f7f1e3;
        --ink: #182126;
        --muted: #55656f;
        --card: rgba(255, 253, 247, 0.92);
        --accent: #c4683f;
        --board-dark: #d7b88d;
        --board-light: #efe1c4;
        --goal-white: #cfe8da;
        --goal-black: #eed1d6;
        --win: #d9fbe3;
        --loss: #f8d7da;
        --draw: #dbeafe;
        --unknown: #eceff1;
        --line: rgba(24, 33, 38, 0.12);
        --shadow: 0 18px 45px rgba(80, 53, 33, 0.16);
      }}

      * {{
        box-sizing: border-box;
      }}

      body {{
        margin: 0;
        min-height: 100vh;
        font-family: "Avenir Next", "Segoe UI", sans-serif;
        color: var(--ink);
        background:
          radial-gradient(circle at top right, rgba(196, 104, 63, 0.18), transparent 34%),
          radial-gradient(circle at bottom left, rgba(88, 124, 143, 0.18), transparent 38%),
          linear-gradient(160deg, #f5ead1 0%, #f9f5eb 54%, #f0e4d0 100%);
      }}

      .shell {{
        width: min(1320px, calc(100vw - 32px));
        margin: 20px auto;
        display: grid;
        gap: 18px;
      }}

      .hero,
      .panel {{
        background: var(--card);
        border: 1px solid var(--line);
        border-radius: 24px;
        box-shadow: var(--shadow);
      }}

      .hero {{
        padding: 22px 26px;
      }}

      .eyebrow {{
        margin: 0 0 6px;
        font-size: 12px;
        font-weight: 700;
        letter-spacing: 0.18em;
        color: var(--accent);
      }}

      h1 {{
        margin: 0;
        font-size: clamp(28px, 4vw, 44px);
        line-height: 1.02;
        font-family: "Iowan Old Style", "Palatino Linotype", serif;
      }}

      .hero p:last-child {{
        margin: 12px 0 0;
        max-width: 78ch;
        color: var(--muted);
      }}

      .layout {{
        display: grid;
        grid-template-columns: minmax(280px, 360px) minmax(0, 1fr);
        gap: 18px;
      }}

      .sidebar,
      .main {{
        display: grid;
        gap: 18px;
      }}

      .panel {{
        padding: 18px;
      }}

      .panel h2,
      .panel h3 {{
        margin: 0 0 12px;
        font-size: 16px;
      }}

      .summary-grid {{
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 10px;
      }}

      .summary-card {{
        padding: 12px;
        border-radius: 16px;
        background: rgba(255, 255, 255, 0.75);
        border: 1px solid var(--line);
      }}

      .summary-card strong {{
        display: block;
        font-size: 22px;
        margin-bottom: 4px;
      }}

      .summary-card span {{
        color: var(--muted);
        font-size: 13px;
      }}

      .filter-row {{
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
        margin-bottom: 14px;
      }}

      .filter-row button,
      .control-row button,
      .move-chip {{
        border: 0;
        border-radius: 999px;
        background: #fff;
        color: var(--ink);
        padding: 10px 14px;
        font: inherit;
        cursor: pointer;
        transition: transform 120ms ease, background 120ms ease, box-shadow 120ms ease;
        box-shadow: 0 2px 0 rgba(24, 33, 38, 0.07);
      }}

      .filter-row button.active,
      .control-row button:hover,
      .move-chip.active {{
        background: var(--accent);
        color: #fff;
      }}

      .filter-row button:hover,
      .move-chip:hover {{
        transform: translateY(-1px);
      }}

      .path-list {{
        display: grid;
        gap: 10px;
        max-height: 58vh;
        overflow: auto;
        padding-right: 4px;
      }}

      .path-card {{
        padding: 14px;
        border-radius: 18px;
        border: 1px solid var(--line);
        background: rgba(255, 255, 255, 0.72);
        cursor: pointer;
      }}

      .path-card.active {{
        border-color: rgba(196, 104, 63, 0.55);
        box-shadow: inset 0 0 0 1px rgba(196, 104, 63, 0.28);
      }}

      .path-head {{
        display: flex;
        justify-content: space-between;
        gap: 12px;
        align-items: baseline;
        margin-bottom: 8px;
      }}

      .pill {{
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 4px 10px;
        border-radius: 999px;
        font-size: 12px;
        font-weight: 700;
      }}

      .pill.WhiteWin {{
        background: var(--win);
      }}

      .pill.BlackWin {{
        background: var(--loss);
      }}

      .pill.Draw {{
        background: var(--draw);
      }}

      .path-meta,
      .muted {{
        color: var(--muted);
        font-size: 13px;
      }}

      .path-sequence {{
        margin-top: 8px;
        font-family: "IBM Plex Mono", Menlo, monospace;
        font-size: 12px;
        line-height: 1.45;
      }}

      .viewer-top {{
        display: flex;
        justify-content: space-between;
        gap: 16px;
        align-items: center;
        margin-bottom: 12px;
      }}

      .viewer-top h2 {{
        font-size: 26px;
        margin: 0;
        font-family: "Iowan Old Style", "Palatino Linotype", serif;
      }}

      .board-shell {{
        display: grid;
        gap: 14px;
      }}

      .proof-panel {{
        display: grid;
        gap: 10px;
        padding: 16px;
        border-radius: 20px;
        border: 1px solid var(--line);
        background: rgba(255, 255, 255, 0.72);
      }}

      .proof-grid {{
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 10px;
      }}

      .proof-card {{
        padding: 12px;
        border-radius: 16px;
        border: 1px solid var(--line);
        background: rgba(255, 255, 255, 0.8);
      }}

      .proof-card strong {{
        display: block;
        margin-bottom: 4px;
        font-size: 12px;
        letter-spacing: 0.08em;
        text-transform: uppercase;
        color: var(--muted);
      }}

      .proof-card span {{
        display: block;
        font-size: 15px;
      }}

      .proof-summary {{
        padding: 14px;
        border-radius: 16px;
        background: rgba(255, 255, 255, 0.85);
        border: 1px solid var(--line);
        line-height: 1.5;
      }}

      .proof-children {{
        display: grid;
        gap: 8px;
      }}

      .proof-child {{
        display: flex;
        justify-content: space-between;
        gap: 12px;
        align-items: baseline;
        padding: 12px 14px;
        border-radius: 14px;
        border: 1px solid var(--line);
        background: rgba(255, 255, 255, 0.8);
      }}

      .proof-child code {{
        font-family: "IBM Plex Mono", Menlo, monospace;
        font-size: 12px;
      }}

      .board-caption {{
        display: grid;
        gap: 8px;
      }}

      .board-frame {{
        border-radius: 26px;
        background: #fff7ea;
        border: 1px solid var(--line);
        padding: 16px;
      }}

      .goal-strip {{
        display: grid;
        grid-template-columns: 70px 1fr;
        gap: 10px;
        align-items: center;
        font-size: 12px;
        color: var(--muted);
      }}

      .goal-chip {{
        height: 10px;
        border-radius: 999px;
      }}

      .goal-chip.black {{
        background: var(--goal-black);
      }}

      .goal-chip.white {{
        background: var(--goal-white);
      }}

      .board-wrap {{
        display: grid;
        grid-template-columns: 24px minmax(0, 1fr);
        gap: 10px;
        margin: 12px 0;
        align-items: stretch;
      }}

      .ranks,
      .files {{
        display: grid;
        font-size: 12px;
        color: var(--muted);
        font-weight: 600;
      }}

      .ranks {{
        grid-template-rows: repeat(6, 1fr);
        gap: 4px;
        align-items: center;
      }}

      .files {{
        grid-template-columns: repeat(7, 1fr);
        gap: 4px;
        padding-left: 24px;
      }}

      .board-grid {{
        display: grid;
        grid-template-columns: repeat(7, minmax(44px, 1fr));
        gap: 4px;
        border-radius: 18px;
        overflow: hidden;
      }}

      .cell {{
        aspect-ratio: 1 / 1;
        display: grid;
        place-items: center;
        font-weight: 800;
        position: relative;
      }}

      .cell.light {{
        background: var(--board-light);
      }}

      .cell.dark {{
        background: var(--board-dark);
      }}

      .token {{
        width: 72%;
        aspect-ratio: 1 / 1;
        border-radius: 50%;
        display: grid;
        place-items: center;
        font-size: 17px;
        box-shadow: inset 0 -2px 0 rgba(0, 0, 0, 0.12);
      }}

      .token.WA,
      .token.WD {{
        background: #fff;
        color: #274257;
      }}

      .token.BA,
      .token.BD {{
        background: #27303a;
        color: #f6f0e7;
      }}

      .token.NB {{
        background: #d36a34;
        color: #fff;
      }}

      .cell-label {{
        position: absolute;
        right: 6px;
        bottom: 5px;
        font-size: 10px;
        color: rgba(24, 33, 38, 0.5);
      }}

      .step-strip {{
        display: flex;
        justify-content: space-between;
        gap: 12px;
        align-items: center;
      }}

      .control-row {{
        display: flex;
        gap: 8px;
      }}

      .control-row button:disabled {{
        opacity: 0.35;
        cursor: not-allowed;
      }}

      input[type="range"] {{
        width: 100%;
        accent-color: var(--accent);
      }}

      .sequence-strip {{
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }}

      .move-chip {{
        font-family: "IBM Plex Mono", Menlo, monospace;
        font-size: 12px;
      }}

      .empty-state {{
        padding: 24px;
        border-radius: 18px;
        background: rgba(255, 255, 255, 0.7);
        border: 1px dashed var(--line);
        color: var(--muted);
      }}

      @media (max-width: 980px) {{
        .layout {{
          grid-template-columns: 1fr;
        }}

        .proof-grid {{
          grid-template-columns: 1fr;
        }}
      }}
    </style>
  </head>
  <body>
    <main class="shell">
      <section class="hero">
        <p class="eyebrow">CHESSBALL</p>
        <h1>Partial Tablebase Browser</h1>
        <p>
          Browse certified lines from the start position to a proved state and
          along its forcing continuation to a finite proof witness.
        </p>
        <p>
          Internal labels remain White and Black for compatibility; they map to
          Blue and Red in the official rules.
        </p>
      </section>

      <section class="layout">
        <aside class="sidebar">
          <section class="panel">
            <h2>Summary</h2>
            <div class="summary-grid" id="summary-grid"></div>
          </section>

          <section class="panel">
            <h2>Proven Paths</h2>
            <div class="filter-row" id="filter-row"></div>
            <div class="path-list" id="path-list"></div>
          </section>
        </aside>

        <section class="main">
          <section class="panel">
            <div class="viewer-top">
              <div>
                <p class="eyebrow">Selected Path</p>
                <h2 id="path-title">No proven path</h2>
              </div>
              <div class="pill" id="outcome-pill">-</div>
            </div>

            <div id="viewer-empty" class="empty-state" hidden>
              No proven paths are available in this export.
            </div>

            <div id="viewer-body" class="board-shell" hidden>
              <div class="board-caption">
                <div class="step-strip">
                  <div>
                    <strong id="step-label">Step 0</strong>
                    <div class="muted" id="step-meta"></div>
                  </div>
                  <div class="control-row">
                    <button id="first-btn" type="button">First</button>
                    <button id="prev-btn" type="button">Prev</button>
                    <button id="next-btn" type="button">Next</button>
                    <button id="last-btn" type="button">Last</button>
                  </div>
                </div>
                <input id="step-range" type="range" min="0" max="0" value="0" />
                <div class="sequence-strip" id="sequence-strip"></div>
              </div>

              <div class="board-frame">
                <div class="goal-strip">
                  <div class="goal-chip black"></div>
                  <div>Black (Red) scores on the top row.</div>
                </div>
                <div class="board-wrap">
                  <div class="ranks" id="rank-labels"></div>
                  <div>
                    <div class="files" id="files-top"></div>
                    <div class="board-grid" id="board-grid"></div>
                    <div class="files" id="files-bottom"></div>
                  </div>
                </div>
                <div class="goal-strip">
                  <div class="goal-chip white"></div>
                  <div>White (Blue) scores on the bottom row.</div>
                </div>
              </div>

              <section class="proof-panel">
                <div class="step-strip">
                  <div>
                    <strong>Local Proof</strong>
                    <div class="muted">Why the selected certified position has a forced verdict.</div>
                  </div>
                </div>
                <div class="proof-grid">
                  <div class="proof-card">
                    <strong>Target state</strong>
                    <span id="proof-target-state">-</span>
                  </div>
                  <div class="proof-card">
                    <strong>Proof rule</strong>
                    <span id="proof-rule">-</span>
                  </div>
                </div>
                <div class="proof-summary" id="proof-summary">-</div>
                <div>
                  <strong>Resolved children</strong>
                  <div class="muted">All legal moves from the certified target position as seen by the proof pass.</div>
                </div>
                <div class="proof-children" id="proof-children"></div>
              </section>
            </div>
          </section>
        </section>
      </section>
    </main>

    <script>
      const PATHS = {data};
      const FILES = ["a", "b", "c", "d", "e", "f", "g"];
      const RANKS = ["6", "5", "4", "3", "2", "1"];
      const FILTERS = ["all", "WhiteWin", "BlackWin", "Draw"];

      const state = {{
        filter: "all",
        selectedKey: PATHS[0]?.key ?? null,
        step: 0,
      }};

      const summaryGrid = document.querySelector("#summary-grid");
      const filterRow = document.querySelector("#filter-row");
      const pathList = document.querySelector("#path-list");
      const viewerEmpty = document.querySelector("#viewer-empty");
      const viewerBody = document.querySelector("#viewer-body");
      const pathTitle = document.querySelector("#path-title");
      const outcomePill = document.querySelector("#outcome-pill");
      const stepLabel = document.querySelector("#step-label");
      const stepMeta = document.querySelector("#step-meta");
      const stepRange = document.querySelector("#step-range");
      const sequenceStrip = document.querySelector("#sequence-strip");
      const boardGrid = document.querySelector("#board-grid");
      const rankLabels = document.querySelector("#rank-labels");
      const filesTop = document.querySelector("#files-top");
      const filesBottom = document.querySelector("#files-bottom");
      const firstBtn = document.querySelector("#first-btn");
      const prevBtn = document.querySelector("#prev-btn");
      const nextBtn = document.querySelector("#next-btn");
      const lastBtn = document.querySelector("#last-btn");
      const proofTargetState = document.querySelector("#proof-target-state");
      const proofRule = document.querySelector("#proof-rule");
      const proofSummary = document.querySelector("#proof-summary");
      const proofChildren = document.querySelector("#proof-children");

      boot();

      function boot() {{
        rankLabels.innerHTML = RANKS.map((rank) => `<span>${{rank}}</span>`).join("");
        const filesHtml = FILES.map((file) => `<span>${{file}}</span>`).join("");
        filesTop.innerHTML = filesHtml;
        filesBottom.innerHTML = filesHtml;

        firstBtn.addEventListener("click", () => setStep(0));
        prevBtn.addEventListener("click", () => setStep(state.step - 1));
        nextBtn.addEventListener("click", () => setStep(state.step + 1));
        lastBtn.addEventListener("click", () => {{
          const path = currentPath();
          if (path) {{
            setStep(path.line.length - 1);
          }}
        }});
        stepRange.addEventListener("input", () => setStep(Number(stepRange.value)));
        window.addEventListener("keydown", (event) => {{
          if (event.key === "ArrowLeft") setStep(state.step - 1);
          if (event.key === "ArrowRight") setStep(state.step + 1);
        }});

        render();
      }}

      function filteredPaths() {{
        return PATHS.filter((path) => state.filter === "all" || path.outcome === state.filter);
      }}

      function currentPath() {{
        const filtered = filteredPaths();
        if (filtered.length === 0) {{
          return null;
        }}
        return filtered.find((path) => path.key === state.selectedKey) ?? filtered[0];
      }}

      function counts() {{
        const tally = {{ all: PATHS.length, WhiteWin: 0, BlackWin: 0, Draw: 0 }};
        for (const path of PATHS) {{
          tally[path.outcome] += 1;
        }}
        return tally;
      }}

      function setFilter(filter) {{
        state.filter = filter;
        const filtered = filteredPaths();
        state.selectedKey = filtered[0]?.key ?? null;
        state.step = 0;
        render();
      }}

      function setPath(key) {{
        state.selectedKey = key;
        state.step = 0;
        render();
      }}

      function setStep(step) {{
        const path = currentPath();
        if (!path) return;
        const max = path.line.length - 1;
        state.step = Math.max(0, Math.min(max, step));
        renderViewer();
      }}

      function render() {{
        renderSummary();
        renderFilters();
        renderPathList();
        renderViewer();
      }}

      function renderSummary() {{
        const tally = counts();
        const cards = [
          ["All proven paths", tally.all],
          ["White wins", tally.WhiteWin],
          ["Black wins", tally.BlackWin],
          ["Draws", tally.Draw],
        ];
        summaryGrid.innerHTML = cards
          .map(([label, value]) => `<div class="summary-card"><strong>${{value}}</strong><span>${{label}}</span></div>`)
          .join("");
      }}

      function renderFilters() {{
        const tally = counts();
        filterRow.innerHTML = FILTERS.map((filter) => {{
          const label = filter === "all" ? "All" : filter.replace("Win", " win");
          const active = filter === state.filter ? "active" : "";
          return `<button class="${{active}}" type="button" data-filter="${{filter}}">${{label}} · ${{tally[filter]}}</button>`;
        }}).join("");
        for (const button of filterRow.querySelectorAll("button")) {{
          button.addEventListener("click", () => setFilter(button.dataset.filter));
        }}
      }}

      function renderPathList() {{
        const filtered = filteredPaths();
        if (filtered.length === 0) {{
          pathList.innerHTML = '<div class="empty-state">No paths for this outcome.</div>';
          return;
        }}

        const current = currentPath();
        if (current && current.key !== state.selectedKey) {{
          state.selectedKey = current.key;
        }}

        pathList.innerHTML = filtered.map((path) => {{
          const active = current?.key === path.key ? "active" : "";
          return `
            <article class="path-card ${{active}}" data-key="${{path.key}}">
              <div class="path-head">
                <strong>State #${{path.targetState}}</strong>
                <span class="pill ${{path.outcome}}">${{path.outcome}}</span>
              </div>
              <div class="path-meta">Depth ${{path.depth}} · proof ${{path.proofPlies}} plies · total ${{path.line.length - 1}} plies</div>
              <div class="path-sequence">${{path.sequence || "&lt;none&gt;"}}</div>
            </article>
          `;
        }}).join("");

        for (const card of pathList.querySelectorAll(".path-card")) {{
          card.addEventListener("click", () => setPath(card.dataset.key));
        }}
      }}

      function renderViewer() {{
        const path = currentPath();
        if (!path) {{
          viewerEmpty.hidden = false;
          viewerBody.hidden = true;
          return;
        }}

        viewerEmpty.hidden = true;
        viewerBody.hidden = false;
        state.selectedKey = path.key;
        if (state.step >= path.line.length) {{
          state.step = path.line.length - 1;
        }}

        const frame = path.line[state.step];
        pathTitle.textContent = `State #${{path.targetState}} · depth ${{path.depth}} · proof ${{path.proofPlies}} plies`;
        outcomePill.textContent = path.outcome;
        outcomePill.className = `pill ${{path.outcome}}`;
        stepLabel.textContent = `Step ${{state.step}} / ${{path.line.length - 1}}`;
        const stage = state.step < path.depth ? "prefix" : state.step === path.depth ? "solved state" : "forcing continuation";
        stepMeta.textContent = `State #${{frame.state}} · verdict ${{frame.outcome}} · ${{stage}}`;
        stepRange.max = String(path.line.length - 1);
        stepRange.value = String(state.step);

        firstBtn.disabled = state.step === 0;
        prevBtn.disabled = state.step === 0;
        nextBtn.disabled = state.step === path.line.length - 1;
        lastBtn.disabled = state.step === path.line.length - 1;

        boardGrid.innerHTML = frame.board
          .flatMap((row, rowIndex) =>
            row.map((cell, colIndex) => {{
              const tone = (rowIndex + colIndex) % 2 === 0 ? "light" : "dark";
              const token = renderToken(cell);
              const square = `${{FILES[colIndex]}}${{RANKS[rowIndex]}}`;
              return `<div class="cell ${{tone}}">${{token}}<span class="cell-label">${{square}}</span></div>`;
            }})
          )
          .join("");

        sequenceStrip.innerHTML = path.moves.length === 0
          ? '<span class="muted">No moves: the start position is already certified.</span>'
          : path.moves.map((move, index) => {{
              const active = state.step === index + 1 ? "active" : "";
              return `<button class="move-chip ${{active}}" type="button" data-step="${{index + 1}}">${{index + 1}}. ${{move}}</button>`;
            }}).join("");

        for (const chip of sequenceStrip.querySelectorAll(".move-chip")) {{
          chip.addEventListener("click", () => setStep(Number(chip.dataset.step)));
        }}

        renderProof(path);
      }}

      function renderProof(path) {{
        proofTargetState.textContent = `#${{path.targetState}}`;
        proofRule.textContent = path.proof.rule;
        proofSummary.textContent = path.proof.summary;
        if (path.proof.children.length === 0) {{
          proofChildren.innerHTML = '<div class="empty-state">No child moves: terminal proof state.</div>';
          return;
        }}
        proofChildren.innerHTML = path.proof.children.map((child) => `
          <div class="proof-child">
            <code>${{child.move}}</code>
            <span>state #${{child.to}} · ${{child.outcome}}</span>
          </div>
        `).join("");
      }}

      function renderToken(cell) {{
        if (!cell || cell === "--") {{
          return "";
        }}
        const glyph = cell === "NB" ? "●" : cell[1];
        return `<span class="token ${{cell}}">${{glyph}}</span>`;
      }}
    </script>
  </body>
</html>
"##
    )
}

fn js_path(result: &PartialTablebaseResult, path: &CertifiedPath) -> String {
    let moves = path
        .move_sequence()
        .into_iter()
        .map(|mv| js_string(&move_to_notation(mv)))
        .collect::<Vec<_>>()
        .join(", ");
    let line = path
        .line
        .iter()
        .enumerate()
        .map(|(index, (position, outcome, mv))| {
            format!(
                "{{state:{}, outcome:{}, move:{}, board:{}}}",
                path.states[index],
                js_string(outcome.as_str()),
                js_optional_string(mv.map(move_to_notation)),
                js_board(*position)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let key = format!("{}:{}", path.outcome.as_str(), path.target_state);
    let sequence = path
        .move_sequence()
        .into_iter()
        .map(move_to_notation)
        .collect::<Vec<_>>()
        .join(" ");
    let proof_children = result
        .proof_children(path.target_state)
        .into_iter()
        .map(|(mv, to, outcome)| {
            format!(
                "{{move:{}, to:{}, outcome:{}}}",
                js_string(&move_to_notation(mv)),
                to,
                js_string(outcome.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let proof = format!(
        "{{rule:{}, summary:{}, children:[{}]}}",
        js_string(
            result
                .proof_rule(path.target_state)
                .map(ProofRule::as_str)
                .unwrap_or("Unknown")
        ),
        js_string(
            &result
                .proof_summary(path.target_state)
                .unwrap_or_else(|| String::from("No proof summary available."))
        ),
        proof_children
    );

    format!(
        "{{key:{}, targetState:{}, depth:{}, proofPlies:{}, outcome:{}, sequence:{}, moves:[{}], line:[{}], proof:{}}}",
        js_string(&key),
        path.target_state,
        path.depth,
        path.proof_plies,
        js_string(path.outcome.as_str()),
        js_string(&sequence),
        moves,
        line,
        proof
    )
}

fn js_board(position: Position) -> String {
    let rows = format!("{position}")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let cells = line
                .split_whitespace()
                .map(js_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", cells)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", rows)
}

fn js_optional_string(value: Option<String>) -> String {
    value
        .map(|value| js_string(&value))
        .unwrap_or_else(|| "null".to_string())
}

fn js_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{}\"", escaped)
}

fn select_dot_states(
    result: &PartialTablebaseResult,
    max_nodes: usize,
    max_depth: usize,
) -> Vec<StateId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(result.graph.start, 0usize)]);

    while let Some((state, depth)) = queue.pop_front() {
        if !seen.insert(state) {
            continue;
        }
        out.push(state);
        if out.len() >= max_nodes || depth >= max_depth {
            continue;
        }
        for edge in result.graph.successors(state) {
            queue.push_back((edge.to, depth + 1));
        }
    }

    out
}

fn outcome_fill(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::WhiteWin => "#d9fbe3",
        Outcome::BlackWin => "#f8d7da",
        Outcome::Draw => "#dbeafe",
        Outcome::Unknown => "#f3f4f6",
    }
}

fn board_one_line(position: Position) -> String {
    format!("{position}")
        .trim_end()
        .replace('\n', " / ")
        .replace('"', "\"\"")
}

fn dot_board_label(position: Position) -> String {
    format!("{position}").trim_end().replace('\n', "\\l")
}

fn csv_escape(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn symmetry_name(symmetry: Symmetry) -> &'static str {
    match symmetry {
        Symmetry::Identity => "Identity",
        Symmetry::MirrorHorizontal => "MirrorHorizontal",
        Symmetry::Rotate180SwapColors => "Rotate180SwapColors",
        Symmetry::MirrorVerticalSwapColors => "MirrorVerticalSwapColors",
    }
}

fn player_name(player: Player) -> &'static str {
    match player {
        Player::White => "White",
        Player::Black => "Black",
    }
}

#[must_use]
fn idx(state: StateId) -> usize {
    state as usize
}

#[must_use]
fn as_state_id(state: usize) -> StateId {
    u32::try_from(state).expect("partial tablebase exceeded u32::MAX states")
}

#[must_use]
fn as_loss_counter(successor_len: usize) -> LossCounter {
    debug_assert!(successor_len <= MAX_MOVES_PER_POSITION);
    LossCounter::try_from(successor_len)
        .expect("ChessBall positions should have at most MAX_MOVES_PER_POSITION legal moves")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::engine::{Move, MoveKind, Piece, PieceKind, Player, Position, Symmetry, square};

    use super::{
        Edge, ExportConfig, Outcome, ParentLink, PartialStats, PartialTablebase,
        PartialTablebaseConfig, PartialTablebaseResult, PathSelectionConfig, ProofRule,
        as_state_id, build_position, build_start, export_to_dir, prove_outcomes,
    };

    fn synthetic_result(
        to_move: Vec<Player>,
        winners: Vec<Option<Player>>,
        closed: Vec<bool>,
        succs: Vec<Vec<Edge>>,
    ) -> PartialTablebaseResult {
        let states = to_move.len();
        let mut preds = vec![Vec::new(); states];
        for (from, edges) in succs.iter().enumerate() {
            for edge in edges {
                preds[edge.to as usize].push(as_state_id(from));
            }
        }

        let graph = PartialTablebase {
            start: 0,
            start_symmetry: Symmetry::Identity,
            state_keys: (0..states)
                .map(|idx| Position::empty(square(idx % 6, idx % 7), Player::White).pack())
                .collect(),
            to_move,
            winners,
            parent: vec![None::<ParentLink>; states],
            path_symmetry: vec![Symmetry::Identity; states],
            closed,
            depths: vec![0; states],
            succs,
            preds,
            max_successors_per_state: 2,
        };
        let outcomes = prove_outcomes(&graph);
        let stats = PartialStats::default();
        PartialTablebaseResult {
            exact: graph.closed.iter().all(|&value| value),
            graph,
            outcomes,
            stats,
        }
    }

    #[test]
    fn immediate_winning_position_is_certified() {
        let mut position = Position::empty(square(4, 2), Player::White);
        position.put_piece(
            square(3, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let result = build_position(position, PartialTablebaseConfig::default());
        assert!(result.exact);
        assert_eq!(result.start_outcome(), Outcome::WhiteWin);
        assert_eq!(result.line_from_start(2).len(), 2);
        let certified = result.certified_entry_paths(5);
        assert_eq!(certified.len(), 1);
        assert_eq!(certified[0].target_state, result.graph.start);
        assert_eq!(certified[0].outcome, Outcome::WhiteWin);
        assert_eq!(certified[0].proof_plies, 1);
        assert_eq!(certified[0].line.len(), 2);
    }

    #[test]
    fn min_proof_plies_can_filter_immediate_wins() {
        let mut position = Position::empty(square(4, 2), Player::White);
        position.put_piece(
            square(3, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let result = build_position(position, PartialTablebaseConfig::default());
        let filtered = result.proven_paths_with_config(PathSelectionConfig {
            limit_per_outcome: 5,
            min_proof_plies: 2,
            prefer_long_proofs: false,
        });
        assert!(filtered.is_empty());
    }

    #[test]
    fn draw_is_certified_when_only_draw_and_losing_exits_exist() {
        let draw_move = Move {
            from: square(0, 0),
            to: square(0, 1),
            kind: MoveKind::Simple,
        };
        let losing_move = Move {
            from: square(0, 0),
            to: square(1, 0),
            kind: MoveKind::Simple,
        };
        let result = synthetic_result(
            vec![Player::White, Player::Black, Player::Black],
            vec![None, None, Some(Player::Black)],
            vec![true, true, true],
            vec![
                vec![
                    Edge {
                        mv: draw_move,
                        to: 1,
                        child_symmetry: Symmetry::Identity,
                    },
                    Edge {
                        mv: losing_move,
                        to: 2,
                        child_symmetry: Symmetry::Identity,
                    },
                ],
                vec![],
                vec![],
            ],
        );

        assert_eq!(result.start_outcome(), Outcome::Draw);
        assert_eq!(
            result.proof_rule(result.graph.start),
            Some(ProofRule::DrawRegion)
        );
        assert!(
            result
                .proof_summary(result.graph.start)
                .unwrap_or_default()
                .contains("preserves Draw")
        );
    }

    #[test]
    fn draw_is_not_certified_when_an_unknown_exit_remains() {
        let draw_move = Move {
            from: square(0, 0),
            to: square(0, 1),
            kind: MoveKind::Simple,
        };
        let unknown_move = Move {
            from: square(0, 0),
            to: square(1, 0),
            kind: MoveKind::Simple,
        };
        let result = synthetic_result(
            vec![Player::White, Player::Black, Player::Black],
            vec![None, None, None],
            vec![true, true, false],
            vec![
                vec![
                    Edge {
                        mv: draw_move,
                        to: 1,
                        child_symmetry: Symmetry::Identity,
                    },
                    Edge {
                        mv: unknown_move,
                        to: 2,
                        child_symmetry: Symmetry::Identity,
                    },
                ],
                vec![],
                vec![],
            ],
        );

        assert_eq!(result.start_outcome(), Outcome::Unknown);
    }

    #[test]
    fn capped_start_state_stays_unknown_instead_of_fake_draw() {
        let result = build_start(PartialTablebaseConfig {
            max_states: Some(1),
        });
        assert!(!result.exact);
        assert_eq!(result.start_outcome(), Outcome::Unknown);
        assert_eq!(result.stats.unknown_states, result.stats.states);
        assert!(result.certified_entry_paths(5).is_empty());
    }

    #[test]
    fn sink_without_goal_row_is_certified_draw() {
        let result = build_position(
            Position::empty(square(2, 3), Player::White),
            PartialTablebaseConfig::default(),
        );
        assert!(result.exact);
        assert_eq!(result.start_outcome(), Outcome::Draw);
    }

    #[test]
    fn exporter_writes_summary_csv_and_dot() {
        let result = build_position(
            Position::empty(square(2, 3), Player::White),
            PartialTablebaseConfig::default(),
        );
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let out_dir = std::env::temp_dir().join(format!("chessball_partial_tb_{timestamp}"));

        export_to_dir(&result, &out_dir, ExportConfig::default()).unwrap();

        assert!(out_dir.join("summary.txt").exists());
        assert!(out_dir.join("states.csv").exists());
        assert!(out_dir.join("edges.csv").exists());
        assert!(out_dir.join("graph.dot").exists());
        assert!(out_dir.join("certified_paths.txt").exists());
        assert!(out_dir.join("certified_paths.csv").exists());
        assert!(out_dir.join("proof_positions.txt").exists());
        assert!(out_dir.join("proof_positions.csv").exists());
        assert!(out_dir.join("proof_paths.dot").exists());
        assert!(out_dir.join("browser.html").exists());

        std::fs::remove_dir_all(out_dir).unwrap();
    }

    #[test]
    fn proof_only_export_skips_full_graph_files() {
        let result = build_position(
            Position::empty(square(2, 3), Player::White),
            PartialTablebaseConfig::default(),
        );
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let out_dir =
            std::env::temp_dir().join(format!("chessball_partial_tb_proof_only_{timestamp}"));

        export_to_dir(
            &result,
            &out_dir,
            ExportConfig {
                export_full_graph: false,
                ..ExportConfig::default()
            },
        )
        .unwrap();

        assert!(out_dir.join("summary.txt").exists());
        assert!(!out_dir.join("states.csv").exists());
        assert!(!out_dir.join("edges.csv").exists());
        assert!(!out_dir.join("graph.dot").exists());
        assert!(out_dir.join("certified_paths.txt").exists());
        assert!(out_dir.join("certified_paths.csv").exists());
        assert!(out_dir.join("proof_positions.txt").exists());
        assert!(out_dir.join("proof_positions.csv").exists());
        assert!(out_dir.join("proof_paths.dot").exists());
        assert!(out_dir.join("browser.html").exists());

        std::fs::remove_dir_all(out_dir).unwrap();
    }
}
