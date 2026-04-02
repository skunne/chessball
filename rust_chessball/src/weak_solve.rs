use std::{
    collections::{HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    mem::size_of,
    path::PathBuf,
    process,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::engine::{
    COLS, MAX_MOVES_PER_POSITION, Move, MoveKind, NUM_SQUARES, PackedPosition, Player, Position,
    Square, Symmetry,
};

pub type StateId = u32;
type LossCounter = u8;
type EdgeOffset = u64;

const MOVE_KIND_SHIFT: u32 = 12;
const EXTRA1_SHIFT: u32 = 14;
const EXTRA2_SHIFT: u32 = 20;
const MOVE_KIND_MASK: u32 = 0b11;
const SQUARE_MASK: u32 = 0b11_1111;
const EDGE_MOVE_SHIFT: u64 = 32;
const EDGE_SYMMETRY_SHIFT: u64 = 58;
const TIME_PROBE_STRIDE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    WhiteWin,
    BlackWin,
    Draw,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeStorageMode {
    #[default]
    Memory,
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeakSolveConfig {
    pub max_states: Option<usize>,
    pub edge_storage: EdgeStorageMode,
    pub checkpoint_states: Option<usize>,
    pub checkpoint_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Expand,
    PredCount,
    PredFill,
    Retrograde,
}

impl Phase {
    #[must_use]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Expand => "expand",
            Self::PredCount => "pred_count",
            Self::PredFill => "pred_fill",
            Self::Retrograde => "retrograde",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ProgressSnapshot {
    states: usize,
    expanded_states: usize,
    edges: usize,
    resident_bytes: usize,
    disk_edge_bytes: usize,
    state_table_peak_bytes: usize,
    aux_bytes: usize,
    solved_states: Option<usize>,
}

#[derive(Debug)]
struct ProgressReporter {
    checkpoint_states: Option<usize>,
    checkpoint_seconds: Option<Duration>,
    overall_started: Instant,
    current_phase: Option<PhaseTracker>,
}

#[derive(Debug)]
struct PhaseTracker {
    phase: Phase,
    started: Instant,
    last_reported: Instant,
    next_state_checkpoint: Option<usize>,
    next_time_probe: usize,
}

impl ProgressReporter {
    #[must_use]
    fn new(config: WeakSolveConfig) -> Option<Self> {
        if config.checkpoint_states.is_none() && config.checkpoint_seconds.is_none() {
            return None;
        }

        Some(Self {
            checkpoint_states: config.checkpoint_states,
            checkpoint_seconds: config.checkpoint_seconds.map(Duration::from_secs),
            overall_started: Instant::now(),
            current_phase: None,
        })
    }

    fn start_phase(&mut self, phase: Phase) {
        let now = Instant::now();
        self.current_phase = Some(PhaseTracker {
            phase,
            started: now,
            last_reported: now,
            next_state_checkpoint: self.checkpoint_states,
            next_time_probe: TIME_PROBE_STRIDE,
        });
    }

    fn maybe_report<F>(&mut self, processed: usize, snapshot: F)
    where
        F: FnOnce() -> ProgressSnapshot,
    {
        let Some(phase) = self.current_phase.as_mut() else {
            return;
        };

        let mut should_report = false;
        if let Some(interval) = self.checkpoint_states {
            if let Some(next) = phase.next_state_checkpoint
                && processed >= next
            {
                should_report = true;
                let mut updated = next;
                while processed >= updated {
                    updated = updated.saturating_add(interval);
                }
                phase.next_state_checkpoint = Some(updated);
            }
        }

        if !should_report
            && let Some(interval) = self.checkpoint_seconds
            && processed >= phase.next_time_probe
        {
            phase.next_time_probe = processed.saturating_add(TIME_PROBE_STRIDE);
            let now = Instant::now();
            if now.duration_since(phase.last_reported) >= interval {
                should_report = true;
                phase.last_reported = now;
            }
        }

        if should_report {
            let snapshot = snapshot();
            self.emit("checkpoint", processed, snapshot);
        }
    }

    fn finish_phase<F>(&mut self, processed: usize, snapshot: F)
    where
        F: FnOnce() -> ProgressSnapshot,
    {
        if self.current_phase.is_none() {
            return;
        }
        let snapshot = snapshot();
        self.emit("summary", processed, snapshot);
        self.current_phase = None;
    }

    fn emit(&mut self, kind: &str, processed: usize, snapshot: ProgressSnapshot) {
        let now = Instant::now();
        let Some(phase) = self.current_phase.as_mut() else {
            return;
        };
        phase.last_reported = now;
        let phase_elapsed = now.duration_since(phase.started).as_secs_f64();
        let total_elapsed = now.duration_since(self.overall_started).as_secs_f64();

        match snapshot.solved_states {
            Some(solved_states) => eprintln!(
                "{kind} phase={} processed={} solved_states={} phase_elapsed_s={:.1} total_elapsed_s={:.1} states={} expanded_states={} edges={} resident_bytes={} disk_edge_bytes={} state_table_peak_bytes={} aux_bytes={}",
                phase.phase.as_str(),
                processed,
                solved_states,
                phase_elapsed,
                total_elapsed,
                snapshot.states,
                snapshot.expanded_states,
                snapshot.edges,
                snapshot.resident_bytes,
                snapshot.disk_edge_bytes,
                snapshot.state_table_peak_bytes,
                snapshot.aux_bytes
            ),
            None => eprintln!(
                "{kind} phase={} processed={} phase_elapsed_s={:.1} total_elapsed_s={:.1} states={} expanded_states={} edges={} resident_bytes={} disk_edge_bytes={} state_table_peak_bytes={} aux_bytes={}",
                phase.phase.as_str(),
                processed,
                phase_elapsed,
                total_elapsed,
                snapshot.states,
                snapshot.expanded_states,
                snapshot.edges,
                snapshot.resident_bytes,
                snapshot.disk_edge_bytes,
                snapshot.state_table_peak_bytes,
                snapshot.aux_bytes
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub mv: Move,
    pub to: StateId,
    pub child_symmetry: Symmetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedMove(u32);

impl PackedMove {
    #[must_use]
    pub const fn new(mv: Move) -> Self {
        let from = mv.from.index() as u32;
        let to = mv.to.index() as u32;
        let (kind, extra1, extra2) = match mv.kind {
            MoveKind::Simple => (0, 0, 0),
            MoveKind::Push { ball_to } => (1, ball_to.index() as u32, 0),
            MoveKind::Jump { jumped } => (2, jumped.index() as u32, 0),
            MoveKind::Tackle {
                pushed_from,
                pushed_to,
            } => (3, pushed_from.index() as u32, pushed_to.index() as u32),
        };

        Self(
            from | (to << 6)
                | (kind << MOVE_KIND_SHIFT)
                | (extra1 << EXTRA1_SHIFT)
                | (extra2 << EXTRA2_SHIFT),
        )
    }

    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn unpack(self) -> Move {
        let raw = self.0;
        let from = square_from_index((raw & SQUARE_MASK) as u8);
        let to = square_from_index(((raw >> 6) & SQUARE_MASK) as u8);
        let extra1 = square_from_index(((raw >> EXTRA1_SHIFT) & SQUARE_MASK) as u8);
        let extra2 = square_from_index(((raw >> EXTRA2_SHIFT) & SQUARE_MASK) as u8);
        let kind = match (raw >> MOVE_KIND_SHIFT) & MOVE_KIND_MASK {
            0 => MoveKind::Simple,
            1 => MoveKind::Push { ball_to: extra1 },
            2 => MoveKind::Jump { jumped: extra1 },
            _ => MoveKind::Tackle {
                pushed_from: extra1,
                pushed_to: extra2,
            },
        };
        Move { from, to, kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedEdge(u64);

impl PackedEdge {
    const BYTES: usize = size_of::<u64>();

    #[must_use]
    pub const fn new(to: StateId, mv: Move, child_symmetry: Symmetry) -> Self {
        Self(
            to as u64
                | ((PackedMove::new(mv).raw() as u64) << EDGE_MOVE_SHIFT)
                | ((child_symmetry as u64) << EDGE_SYMMETRY_SHIFT),
        )
    }

    #[must_use]
    pub const fn to(self) -> StateId {
        self.0 as StateId
    }

    #[must_use]
    pub const fn child_symmetry(self) -> Symmetry {
        Symmetry::from_bits(((self.0 >> EDGE_SYMMETRY_SHIFT) & 0b11) as u8)
    }

    #[must_use]
    pub fn mv(self) -> Move {
        PackedMove(((self.0 >> EDGE_MOVE_SHIFT) & 0x03ff_ffff) as u32).unpack()
    }

    #[must_use]
    pub fn unpack(self) -> Edge {
        Edge {
            mv: self.mv(),
            to: self.to(),
            child_symmetry: self.child_symmetry(),
        }
    }

    #[must_use]
    fn to_le_bytes(self) -> [u8; Self::BYTES] {
        self.0.to_le_bytes()
    }

    #[must_use]
    fn from_le_bytes(bytes: [u8; Self::BYTES]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }
}

#[derive(Debug)]
struct DiskEdgeStore {
    path: PathBuf,
    edge_count: usize,
}

impl DiskEdgeStore {
    #[must_use]
    fn disk_bytes(&self) -> usize {
        self.edge_count * PackedEdge::BYTES
    }

    fn for_each_in_range(&self, start: usize, end: usize, mut callback: impl FnMut(PackedEdge)) {
        let count = end.saturating_sub(start);
        if count == 0 {
            return;
        }

        let mut file = File::open(&self.path).expect("failed to reopen spilled edge file");
        file.seek(SeekFrom::Start((start * PackedEdge::BYTES) as u64))
            .expect("failed to seek spilled edge file");

        let mut bytes = vec![0u8; count * PackedEdge::BYTES];
        file.read_exact(&mut bytes)
            .expect("failed to read spilled edge file");

        for chunk in bytes.chunks_exact(PackedEdge::BYTES) {
            let mut raw = [0u8; PackedEdge::BYTES];
            raw.copy_from_slice(chunk);
            callback(PackedEdge::from_le_bytes(raw));
        }
    }
}

impl Drop for DiskEdgeStore {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug)]
enum SuccessorStore {
    Memory(Vec<PackedEdge>),
    Disk(DiskEdgeStore),
}

impl SuccessorStore {
    #[must_use]
    fn edge_count(&self) -> usize {
        match self {
            Self::Memory(edges) => edges.len(),
            Self::Disk(store) => store.edge_count,
        }
    }

    #[must_use]
    fn resident_bytes(&self) -> usize {
        match self {
            Self::Memory(edges) => edges.len() * size_of::<PackedEdge>(),
            Self::Disk(_) => 0,
        }
    }

    #[must_use]
    fn disk_bytes(&self) -> usize {
        match self {
            Self::Memory(_) => 0,
            Self::Disk(store) => store.disk_bytes(),
        }
    }

    fn for_each_in_range(&self, start: usize, end: usize, mut callback: impl FnMut(PackedEdge)) {
        match self {
            Self::Memory(edges) => {
                for &edge in &edges[start..end] {
                    callback(edge);
                }
            }
            Self::Disk(store) => store.for_each_in_range(start, end, callback),
        }
    }
}

enum SuccessorWriter {
    Memory(Vec<PackedEdge>),
    Disk {
        path: PathBuf,
        writer: BufWriter<File>,
        edge_count: usize,
    },
}

impl SuccessorWriter {
    fn new(mode: EdgeStorageMode) -> Self {
        match mode {
            EdgeStorageMode::Memory => Self::Memory(Vec::new()),
            EdgeStorageMode::Disk => {
                let path = create_spill_path();
                let file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&path)
                    .expect("failed to create spilled edge file");
                Self::Disk {
                    path,
                    writer: BufWriter::new(file),
                    edge_count: 0,
                }
            }
        }
    }

    fn push(&mut self, edge: PackedEdge) {
        match self {
            Self::Memory(edges) => edges.push(edge),
            Self::Disk {
                writer, edge_count, ..
            } => {
                writer
                    .write_all(&edge.to_le_bytes())
                    .expect("failed to write spilled edge");
                *edge_count += 1;
            }
        }
    }

    #[must_use]
    fn len(&self) -> usize {
        match self {
            Self::Memory(edges) => edges.len(),
            Self::Disk { edge_count, .. } => *edge_count,
        }
    }

    #[must_use]
    fn resident_bytes(&self) -> usize {
        match self {
            Self::Memory(edges) => edges.len() * size_of::<PackedEdge>(),
            Self::Disk { .. } => 0,
        }
    }

    #[must_use]
    fn disk_bytes(&self) -> usize {
        match self {
            Self::Memory(_) => 0,
            Self::Disk { edge_count, .. } => edge_count * PackedEdge::BYTES,
        }
    }

    fn finish(self) -> SuccessorStore {
        match self {
            Self::Memory(edges) => SuccessorStore::Memory(edges),
            Self::Disk {
                path,
                mut writer,
                edge_count,
            } => {
                writer.flush().expect("failed to flush spilled edge file");
                drop(writer);
                SuccessorStore::Disk(DiskEdgeStore { path, edge_count })
            }
        }
    }
}

#[derive(Debug)]
struct StateIdTable {
    keys: Vec<u128>,
    values: Vec<StateId>,
    len: usize,
    peak_bytes: usize,
}

impl StateIdTable {
    const EMPTY: u128 = u128::MAX;

    fn with_capacity(items: usize) -> Self {
        let slots = items.saturating_mul(2).next_power_of_two().max(16);
        let mut table = Self {
            keys: vec![Self::EMPTY; slots],
            values: vec![0; slots],
            len: 0,
            peak_bytes: 0,
        };
        table.peak_bytes = table.bytes();
        table
    }

    #[must_use]
    fn bytes(&self) -> usize {
        self.keys.len() * size_of::<u128>() + self.values.len() * size_of::<StateId>()
    }

    #[must_use]
    fn peak_bytes(&self) -> usize {
        self.peak_bytes
    }

    #[must_use]
    fn get(&self, key: PackedPosition) -> Option<StateId> {
        let raw = key.raw();
        debug_assert_ne!(raw, Self::EMPTY);
        let mask = self.keys.len() - 1;
        let mut index = hash_packed_position(raw) & mask;
        loop {
            let existing = self.keys[index];
            if existing == Self::EMPTY {
                return None;
            }
            if existing == raw {
                return Some(self.values[index]);
            }
            index = (index + 1) & mask;
        }
    }

    fn insert(&mut self, key: PackedPosition, value: StateId) {
        if (self.len + 1) * 10 > self.keys.len() * 7 {
            self.resize(self.keys.len() * 2);
        }

        let raw = key.raw();
        debug_assert_ne!(raw, Self::EMPTY);
        let mask = self.keys.len() - 1;
        let mut index = hash_packed_position(raw) & mask;
        loop {
            let existing = self.keys[index];
            if existing == Self::EMPTY {
                self.keys[index] = raw;
                self.values[index] = value;
                self.len += 1;
                return;
            }
            if existing == raw {
                self.values[index] = value;
                return;
            }
            index = (index + 1) & mask;
        }
    }

    fn resize(&mut self, new_slots: usize) {
        let old_keys = std::mem::replace(&mut self.keys, vec![Self::EMPTY; new_slots]);
        let old_values = std::mem::replace(&mut self.values, vec![0; new_slots]);
        self.len = 0;
        self.peak_bytes = self.peak_bytes.max(self.bytes());

        for (key, value) in old_keys.into_iter().zip(old_values.into_iter()) {
            if key != Self::EMPTY {
                self.insert_raw(key, value);
            }
        }
    }

    fn insert_raw(&mut self, raw: u128, value: StateId) {
        let mask = self.keys.len() - 1;
        let mut index = hash_packed_position(raw) & mask;
        loop {
            if self.keys[index] == Self::EMPTY {
                self.keys[index] = raw;
                self.values[index] = value;
                self.len += 1;
                return;
            }
            index = (index + 1) & mask;
        }
    }
}

#[derive(Debug)]
pub struct StateGraph {
    pub start: StateId,
    pub start_symmetry: Symmetry,
    pub state_keys: Vec<PackedPosition>,
    pub to_move: Vec<Player>,
    pub winners: Vec<Option<Player>>,
    pub closed: Vec<bool>,
    pub max_successors_per_state: LossCounter,
    pub succ_offsets: Vec<EdgeOffset>,
    succ_store: SuccessorStore,
    pub pred_offsets: Vec<EdgeOffset>,
    pub pred_ids: Vec<StateId>,
    pub expanded_states: usize,
    pub state_table_peak_bytes: usize,
    pub revisited_child_edges: usize,
    pub self_loop_edges: usize,
    pub truncated: bool,
}

impl StateGraph {
    #[must_use]
    pub fn successor_len(&self, state: StateId) -> usize {
        let (start, end) = self.successor_bounds(state);
        end - start
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.succ_store.edge_count()
    }

    #[must_use]
    pub fn edge_storage_mode(&self) -> EdgeStorageMode {
        match self.succ_store {
            SuccessorStore::Memory(_) => EdgeStorageMode::Memory,
            SuccessorStore::Disk(_) => EdgeStorageMode::Disk,
        }
    }

    #[must_use]
    pub fn successors(&self, state: StateId) -> Vec<Edge> {
        let (start, end) = self.successor_bounds(state);
        let mut edges = Vec::with_capacity(end - start);
        self.succ_store
            .for_each_in_range(start, end, |edge| edges.push(edge.unpack()));
        edges
    }

    #[must_use]
    pub fn predecessors(&self, state: StateId) -> &[StateId] {
        let index = idx(state);
        let start = self.pred_offsets[index] as usize;
        let end = self.pred_offsets[index + 1] as usize;
        &self.pred_ids[start..end]
    }

    #[must_use]
    pub fn canonical_position(&self, state: StateId) -> Position {
        self.state_keys[idx(state)].unpack()
    }

    #[must_use]
    pub fn resident_storage_bytes(&self) -> usize {
        estimate_resident_bytes(
            self.state_keys.len(),
            self.succ_offsets.len(),
            self.succ_store.resident_bytes(),
            self.pred_offsets.len(),
            self.pred_ids.len(),
        )
    }

    #[must_use]
    pub fn disk_edge_bytes(&self) -> usize {
        self.succ_store.disk_bytes()
    }

    #[must_use]
    fn successor_bounds(&self, state: StateId) -> (usize, usize) {
        let index = idx(state);
        (
            self.succ_offsets[index] as usize,
            self.succ_offsets[index + 1] as usize,
        )
    }
}

#[must_use]
fn estimate_resident_bytes(
    states: usize,
    succ_offsets_len: usize,
    succ_resident_bytes: usize,
    pred_offsets_len: usize,
    pred_ids_len: usize,
) -> usize {
    states * size_of::<PackedPosition>()
        + states * size_of::<Player>()
        + states * size_of::<Option<Player>>()
        + states * size_of::<bool>()
        + succ_offsets_len * size_of::<EdgeOffset>()
        + succ_resident_bytes
        + pred_offsets_len * size_of::<EdgeOffset>()
        + pred_ids_len * size_of::<StateId>()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphStats {
    pub states: usize,
    pub expanded_states: usize,
    pub closed_states: usize,
    pub edges: usize,
    pub revisited_child_edges: usize,
    pub self_loop_edges: usize,
    pub max_successors_per_state: LossCounter,
    pub terminal_white_wins: usize,
    pub terminal_black_wins: usize,
    pub sink_draws: usize,
    pub certified_states: usize,
    pub certified_white_wins: usize,
    pub certified_black_wins: usize,
    pub certified_draws: usize,
    pub certified_unknown_states: usize,
    pub draw_candidate_seed_states: usize,
    pub draw_candidate_states: usize,
    pub draw_candidate_sccs: usize,
    pub cyclic_draw_candidate_sccs: usize,
    pub cyclic_draw_candidate_states: usize,
    pub largest_draw_candidate_scc: usize,
    pub largest_cyclic_draw_scc: usize,
    pub draw_prune_iterations: usize,
    pub draw_prune_removed_mover_win_exit: usize,
    pub draw_prune_removed_open_or_unknown_exit: usize,
    pub draw_prune_removed_no_draw_successor: usize,
    pub resident_storage_bytes: usize,
    pub disk_edge_bytes: usize,
    pub state_table_peak_bytes: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct CertificationStats {
    draw_candidate_seed_states: usize,
    draw_candidate_states: usize,
    draw_candidate_sccs: usize,
    cyclic_draw_candidate_sccs: usize,
    cyclic_draw_candidate_states: usize,
    largest_draw_candidate_scc: usize,
    largest_cyclic_draw_scc: usize,
    draw_prune_iterations: usize,
    draw_prune_removed_mover_win_exit: usize,
    draw_prune_removed_open_or_unknown_exit: usize,
    draw_prune_removed_no_draw_successor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawPruneRemovalReason {
    MoverWinExit,
    OpenOrUnknownExit,
}

#[derive(Debug)]
struct CertifiedOutcomeResult {
    outcomes: Vec<Option<Outcome>>,
    stats: CertificationStats,
}

#[derive(Debug)]
pub struct SolveResult {
    pub graph: StateGraph,
    pub outcomes: Vec<Outcome>,
    certified_outcomes: Option<Vec<Option<Outcome>>>,
    pub exact: bool,
    pub stats: GraphStats,
}

impl SolveResult {
    #[must_use]
    pub fn start_outcome(&self) -> Outcome {
        self.hinted_start_outcome()
    }

    #[must_use]
    pub fn hinted_start_outcome(&self) -> Outcome {
        self.outcomes[idx(self.graph.start)].apply_symmetry(self.graph.start_symmetry)
    }

    #[must_use]
    pub fn recommended_move(&self, state: StateId) -> Option<Move> {
        self.recommended_edge(state, Symmetry::Identity)
            .map(|edge| edge.mv)
    }

    #[must_use]
    pub fn certified_start_outcome(&self) -> Option<Outcome> {
        let outcome = if self.exact {
            Some(self.outcomes[idx(self.graph.start)])
        } else {
            self.certified_outcomes
                .as_ref()
                .and_then(|outcomes| outcomes[idx(self.graph.start)])
        }?;
        Some(outcome.apply_symmetry(self.graph.start_symmetry))
    }

    #[must_use]
    pub fn certified_line_from_start(
        &self,
        max_plies: usize,
    ) -> Option<Vec<(Position, Outcome, Option<Move>)>> {
        let certified = if self.exact {
            None
        } else {
            self.certified_outcomes.as_deref()
        };
        let _ = self.certified_start_outcome()?;
        Some(self.line_from_start_with_outcomes(max_plies, certified))
    }

    #[must_use]
    pub fn line_from_start(&self, max_plies: usize) -> Vec<(Position, Outcome, Option<Move>)> {
        self.line_from_start_with_outcomes(max_plies, None)
    }

    #[must_use]
    fn line_from_start_with_outcomes(
        &self,
        max_plies: usize,
        certified: Option<&[Option<Outcome>]>,
    ) -> Vec<(Position, Outcome, Option<Move>)> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut state = self.graph.start;
        let mut orientation = self.graph.start_symmetry;

        for _ in 0..max_plies {
            let canonical_position = self.graph.canonical_position(state);
            let actual_position = canonical_position.apply_symmetry(orientation);
            let canonical_outcome = match certified {
                Some(outcomes) => match outcomes[idx(state)] {
                    Some(outcome) => outcome,
                    None => break,
                },
                None => self.outcomes[idx(state)],
            };
            let outcome = canonical_outcome.apply_symmetry(orientation);
            let edge = match certified {
                Some(outcomes) => self.recommended_certified_edge(state, orientation, outcomes),
                None => self.recommended_edge(state, orientation),
            };
            let actual_move = edge.map(|edge| edge.mv.apply_symmetry(orientation));

            out.push((actual_position, outcome, actual_move));

            let Some(edge) = edge else {
                break;
            };

            let next_state = edge.to;
            let next_orientation = orientation.combine(edge.child_symmetry);
            if let Some(actual_move) = actual_move {
                debug_assert_eq!(
                    actual_position.apply(actual_move),
                    self.graph
                        .canonical_position(next_state)
                        .apply_symmetry(next_orientation)
                );
            }

            if !seen.insert((state, orientation)) {
                break;
            }
            state = next_state;
            orientation = next_orientation;
        }

        out
    }

    #[must_use]
    fn recommended_edge(&self, state: StateId, orientation: Symmetry) -> Option<Edge> {
        let state_index = idx(state);
        let mover = orientation.apply_player(self.graph.to_move[state_index]);
        let state_outcome = self.outcomes[state_index].apply_symmetry(orientation);
        let mover_win = Outcome::for_player(mover);
        let opponent_win = Outcome::for_player(mover.opponent());
        let edges = self.graph.successors(state);

        match state_outcome {
            outcome if outcome == mover_win => edges.into_iter().find(|edge| {
                self.outcomes[idx(edge.to)].apply_symmetry(orientation.combine(edge.child_symmetry))
                    == mover_win
            }),
            Outcome::Draw => {
                let mut fallback = None;
                for edge in edges {
                    let child_outcome = self.outcomes[idx(edge.to)]
                        .apply_symmetry(orientation.combine(edge.child_symmetry));
                    if child_outcome != opponent_win {
                        return Some(edge);
                    }
                    fallback.get_or_insert(edge);
                }
                fallback
            }
            _ => edges.into_iter().find(|edge| {
                self.outcomes[idx(edge.to)].apply_symmetry(orientation.combine(edge.child_symmetry))
                    == opponent_win
            }),
        }
    }

    #[must_use]
    fn recommended_certified_edge(
        &self,
        state: StateId,
        orientation: Symmetry,
        outcomes: &[Option<Outcome>],
    ) -> Option<Edge> {
        let state_index = idx(state);
        let mover = orientation.apply_player(self.graph.to_move[state_index]);
        let state_outcome = outcomes[state_index]?.apply_symmetry(orientation);
        let mover_win = Outcome::for_player(mover);
        let opponent_win = Outcome::for_player(mover.opponent());
        let edges = self.graph.successors(state);

        match state_outcome {
            outcome if outcome == mover_win => edges.into_iter().find(|edge| {
                outcomes[idx(edge.to)]
                    .map(|child| child.apply_symmetry(orientation.combine(edge.child_symmetry)))
                    == Some(mover_win)
            }),
            Outcome::Draw => {
                let mut fallback = None;
                for edge in edges {
                    match outcomes[idx(edge.to)]
                        .map(|child| child.apply_symmetry(orientation.combine(edge.child_symmetry)))
                    {
                        Some(outcome) if outcome != opponent_win => return Some(edge),
                        Some(_) => {
                            fallback.get_or_insert(edge);
                        }
                        None => {}
                    }
                }
                fallback
            }
            _ => edges.into_iter().find(|edge| {
                outcomes[idx(edge.to)]
                    .map(|child| child.apply_symmetry(orientation.combine(edge.child_symmetry)))
                    == Some(opponent_win)
            }),
        }
    }
}

#[must_use]
pub fn solve_start(config: WeakSolveConfig) -> SolveResult {
    solve_position(Position::new_game(), config)
}

#[must_use]
pub fn solve_position(start: Position, config: WeakSolveConfig) -> SolveResult {
    let mut reporter = ProgressReporter::new(config);
    let graph = build_graph(start, config, reporter.as_mut());
    let outcomes = retrograde_outcomes(&graph, reporter.as_mut());
    let certified_result = if graph.truncated {
        Some(certified_outcomes(&graph))
    } else {
        None
    };
    let certified_outcomes = certified_result
        .as_ref()
        .map(|result| result.outcomes.as_slice());
    let stats = compute_stats(
        &graph,
        &outcomes,
        certified_outcomes,
        certified_result.as_ref().map(|result| result.stats),
        graph.truncated,
    );
    SolveResult {
        exact: !graph.truncated,
        graph,
        outcomes,
        certified_outcomes: certified_result.map(|result| result.outcomes),
        stats,
    }
}

#[must_use]
fn build_graph(
    start: Position,
    config: WeakSolveConfig,
    mut reporter: Option<&mut ProgressReporter>,
) -> StateGraph {
    let (start_position, start_symmetry) = start.canonical_color_preserving();
    let start_key = start_position.pack();

    let reserve = config.max_states.unwrap_or(1024).min(131_072).max(16);
    let mut state_keys = Vec::with_capacity(reserve);
    let mut to_move = Vec::with_capacity(reserve);
    let mut winners = Vec::with_capacity(reserve);
    let mut closed = Vec::with_capacity(reserve);
    let mut succ_offsets = Vec::with_capacity(reserve + 1);
    state_keys.push(start_key);
    to_move.push(start_position.to_move);
    winners.push(start_position.winner());
    closed.push(start_position.winner().is_some());
    succ_offsets.push(0u64);
    let mut succ_writer = SuccessorWriter::new(config.edge_storage);
    let mut ids = StateIdTable::with_capacity(reserve);
    ids.insert(start_key, 0u32);

    let mut cursor = 0usize;
    let mut max_successors_per_state = 0u8;
    let mut revisited_child_edges = 0usize;
    let mut self_loop_edges = 0usize;
    let mut truncated = false;
    if let Some(reporter) = reporter.as_deref_mut() {
        reporter.start_phase(Phase::Expand);
    }

    while cursor < state_keys.len() && !truncated {
        if closed[cursor] {
            succ_offsets.push(as_edge_offset(succ_writer.len()));
            cursor += 1;
            if let Some(reporter) = reporter.as_deref_mut() {
                reporter.maybe_report(cursor, || ProgressSnapshot {
                    states: state_keys.len(),
                    expanded_states: cursor,
                    edges: succ_writer.len(),
                    resident_bytes: estimate_resident_bytes(
                        state_keys.len(),
                        succ_offsets.len(),
                        succ_writer.resident_bytes(),
                        0,
                        0,
                    ),
                    disk_edge_bytes: succ_writer.disk_bytes(),
                    state_table_peak_bytes: ids.peak_bytes(),
                    aux_bytes: 0,
                    solved_states: None,
                });
            }
            continue;
        }

        let position = state_keys[cursor].unpack();
        let mut temp_edges = Vec::new();
        let mut state_complete = true;
        for mv in position.legal_moves() {
            let child = position.apply(mv);
            let (child, child_symmetry) = child.canonical_color_preserving();
            let child_key = child.pack();
            let child_id = if let Some(id) = ids.get(child_key) {
                revisited_child_edges += 1;
                if id == as_state_id(cursor) {
                    self_loop_edges += 1;
                }
                id
            } else {
                if let Some(limit) = config.max_states
                    && state_keys.len() >= limit
                {
                    truncated = true;
                    state_complete = false;
                    break;
                }
                let id = as_state_id(state_keys.len());
                ids.insert(child_key, id);
                state_keys.push(child_key);
                to_move.push(child.to_move);
                let winner = child.winner();
                winners.push(winner);
                closed.push(winner.is_some());
                id
            };
            temp_edges.push(PackedEdge::new(child_id, mv, child_symmetry));
        }

        if !state_complete {
            break;
        }

        let successor_len = temp_edges.len();
        for edge in temp_edges {
            succ_writer.push(edge);
        }
        closed[cursor] = true;
        max_successors_per_state = max_successors_per_state.max(as_loss_counter(successor_len));
        succ_offsets.push(as_edge_offset(succ_writer.len()));
        cursor += 1;
        if let Some(reporter) = reporter.as_deref_mut() {
            reporter.maybe_report(cursor, || ProgressSnapshot {
                states: state_keys.len(),
                expanded_states: cursor,
                edges: succ_writer.len(),
                resident_bytes: estimate_resident_bytes(
                    state_keys.len(),
                    succ_offsets.len(),
                    succ_writer.resident_bytes(),
                    0,
                    0,
                ),
                disk_edge_bytes: succ_writer.disk_bytes(),
                state_table_peak_bytes: ids.peak_bytes(),
                aux_bytes: 0,
                solved_states: None,
            });
        }
    }

    let final_offset = as_edge_offset(succ_writer.len());
    while succ_offsets.len() < state_keys.len() + 1 {
        succ_offsets.push(final_offset);
    }

    let state_table_peak_bytes = ids.peak_bytes();
    if let Some(reporter) = reporter.as_deref_mut() {
        reporter.finish_phase(cursor, || ProgressSnapshot {
            states: state_keys.len(),
            expanded_states: cursor,
            edges: succ_writer.len(),
            resident_bytes: estimate_resident_bytes(
                state_keys.len(),
                succ_offsets.len(),
                succ_writer.resident_bytes(),
                0,
                0,
            ),
            disk_edge_bytes: succ_writer.disk_bytes(),
            state_table_peak_bytes,
            aux_bytes: 0,
            solved_states: None,
        });
    }

    let succ_store = succ_writer.finish();
    let (pred_offsets, pred_ids) = build_predecessors(
        &succ_offsets,
        &succ_store,
        state_keys.len(),
        cursor,
        state_table_peak_bytes,
        reporter.as_deref_mut(),
    );

    StateGraph {
        start: 0,
        start_symmetry,
        state_keys,
        to_move,
        winners,
        closed,
        max_successors_per_state,
        succ_offsets,
        succ_store,
        pred_offsets,
        pred_ids,
        expanded_states: cursor,
        state_table_peak_bytes,
        revisited_child_edges,
        self_loop_edges,
        truncated,
    }
}

fn build_predecessors(
    succ_offsets: &[EdgeOffset],
    succ_store: &SuccessorStore,
    num_states: usize,
    expanded_states: usize,
    state_table_peak_bytes: usize,
    mut reporter: Option<&mut ProgressReporter>,
) -> (Vec<EdgeOffset>, Vec<StateId>) {
    if let Some(reporter) = reporter.as_deref_mut() {
        reporter.start_phase(Phase::PredCount);
    }
    let mut indegree = vec![0usize; num_states];
    for state in 0..expanded_states {
        let start = succ_offsets[state] as usize;
        let end = succ_offsets[state + 1] as usize;
        succ_store.for_each_in_range(start, end, |edge| {
            indegree[idx(edge.to())] += 1;
        });
        if let Some(reporter) = reporter.as_deref_mut() {
            reporter.maybe_report(state + 1, || ProgressSnapshot {
                states: num_states,
                expanded_states,
                edges: succ_store.edge_count(),
                resident_bytes: estimate_resident_bytes(
                    num_states,
                    succ_offsets.len(),
                    succ_store.resident_bytes(),
                    0,
                    0,
                ),
                disk_edge_bytes: succ_store.disk_bytes(),
                state_table_peak_bytes,
                aux_bytes: indegree.len() * size_of::<usize>(),
                solved_states: None,
            });
        }
    }
    if let Some(reporter) = reporter.as_deref_mut() {
        reporter.finish_phase(expanded_states, || ProgressSnapshot {
            states: num_states,
            expanded_states,
            edges: succ_store.edge_count(),
            resident_bytes: estimate_resident_bytes(
                num_states,
                succ_offsets.len(),
                succ_store.resident_bytes(),
                0,
                0,
            ),
            disk_edge_bytes: succ_store.disk_bytes(),
            state_table_peak_bytes,
            aux_bytes: indegree.len() * size_of::<usize>(),
            solved_states: None,
        });
        reporter.start_phase(Phase::PredFill);
    }

    let mut pred_offsets = vec![0u64; num_states + 1];
    for state in 0..num_states {
        pred_offsets[state + 1] = pred_offsets[state] + as_edge_offset(indegree[state]);
    }

    let mut pred_ids = vec![0u32; succ_store.edge_count()];
    let mut write_heads = pred_offsets[..num_states].to_vec();
    for state in 0..expanded_states {
        let start = succ_offsets[state] as usize;
        let end = succ_offsets[state + 1] as usize;
        succ_store.for_each_in_range(start, end, |edge| {
            let head = &mut write_heads[idx(edge.to())];
            pred_ids[*head as usize] = as_state_id(state);
            *head += 1;
        });
        if let Some(reporter) = reporter.as_deref_mut() {
            reporter.maybe_report(state + 1, || ProgressSnapshot {
                states: num_states,
                expanded_states,
                edges: succ_store.edge_count(),
                resident_bytes: estimate_resident_bytes(
                    num_states,
                    succ_offsets.len(),
                    succ_store.resident_bytes(),
                    pred_offsets.len(),
                    pred_ids.len(),
                ),
                disk_edge_bytes: succ_store.disk_bytes(),
                state_table_peak_bytes,
                aux_bytes: write_heads.len() * size_of::<u32>(),
                solved_states: None,
            });
        }
    }
    if let Some(reporter) = reporter.as_deref_mut() {
        reporter.finish_phase(expanded_states, || ProgressSnapshot {
            states: num_states,
            expanded_states,
            edges: succ_store.edge_count(),
            resident_bytes: estimate_resident_bytes(
                num_states,
                succ_offsets.len(),
                succ_store.resident_bytes(),
                pred_offsets.len(),
                pred_ids.len(),
            ),
            disk_edge_bytes: succ_store.disk_bytes(),
            state_table_peak_bytes,
            aux_bytes: write_heads.len() * size_of::<u32>(),
            solved_states: None,
        });
    }

    (pred_offsets, pred_ids)
}

fn retrograde_outcomes(
    graph: &StateGraph,
    mut reporter: Option<&mut ProgressReporter>,
) -> Vec<Outcome> {
    let mut outcomes = vec![None::<Outcome>; graph.state_keys.len()];
    let mut remaining_for_loss = (0..graph.state_keys.len())
        .map(|state| as_loss_counter(graph.successor_len(as_state_id(state))))
        .collect::<Vec<LossCounter>>();
    let mut queue = VecDeque::new();
    let mut solved_states = 0usize;
    let mut processed = 0usize;

    if let Some(reporter) = reporter.as_deref_mut() {
        reporter.start_phase(Phase::Retrograde);
    }

    for (state, winner) in graph.winners.iter().copied().enumerate() {
        if let Some(winner) = winner {
            let outcome = Outcome::for_player(winner);
            outcomes[state] = Some(outcome);
            queue.push_back(as_state_id(state));
            solved_states += 1;
        }
    }

    while let Some(child) = queue.pop_front() {
        processed += 1;
        let child_outcome = outcomes[idx(child)].expect("queued states must be solved");
        for &pred in graph.predecessors(child) {
            let pred_index = idx(pred);
            if outcomes[pred_index].is_some() {
                continue;
            }

            let mover = graph.to_move[pred_index];
            let mover_win = Outcome::for_player(mover);
            let opponent_win = Outcome::for_player(mover.opponent());

            if child_outcome == mover_win {
                outcomes[pred_index] = Some(mover_win);
                queue.push_back(pred);
                solved_states += 1;
            } else if child_outcome == opponent_win {
                remaining_for_loss[pred_index] -= 1;
                if remaining_for_loss[pred_index] == 0 {
                    outcomes[pred_index] = Some(opponent_win);
                    queue.push_back(pred);
                    solved_states += 1;
                }
            }
        }

        if let Some(reporter) = reporter.as_deref_mut() {
            reporter.maybe_report(processed, || ProgressSnapshot {
                states: graph.state_keys.len(),
                expanded_states: graph.expanded_states,
                edges: graph.edge_count(),
                resident_bytes: graph.resident_storage_bytes(),
                disk_edge_bytes: graph.disk_edge_bytes(),
                state_table_peak_bytes: graph.state_table_peak_bytes,
                aux_bytes: outcomes.len() * size_of::<Option<Outcome>>()
                    + remaining_for_loss.len() * size_of::<LossCounter>()
                    + queue.len() * size_of::<StateId>(),
                solved_states: Some(solved_states),
            });
        }
    }

    if let Some(reporter) = reporter.as_deref_mut() {
        reporter.finish_phase(processed, || ProgressSnapshot {
            states: graph.state_keys.len(),
            expanded_states: graph.expanded_states,
            edges: graph.edge_count(),
            resident_bytes: graph.resident_storage_bytes(),
            disk_edge_bytes: graph.disk_edge_bytes(),
            state_table_peak_bytes: graph.state_table_peak_bytes,
            aux_bytes: outcomes.len() * size_of::<Option<Outcome>>()
                + remaining_for_loss.len() * size_of::<LossCounter>(),
            solved_states: Some(solved_states),
        });
    }

    outcomes
        .into_iter()
        .map(|outcome| outcome.unwrap_or(Outcome::Draw))
        .collect()
}

fn certified_outcomes(graph: &StateGraph) -> CertifiedOutcomeResult {
    let mut outcomes = vec![None::<Outcome>; graph.state_keys.len()];
    let mut remaining_for_loss = vec![0u8; graph.state_keys.len()];
    let mut queue = VecDeque::new();

    for state in 0..graph.state_keys.len() {
        if !graph.closed[state] {
            continue;
        }

        remaining_for_loss[state] = as_loss_counter(graph.successor_len(as_state_id(state)));

        if let Some(winner) = graph.winners[state] {
            outcomes[state] = Some(Outcome::for_player(winner));
            queue.push_back(as_state_id(state));
        } else if graph.successor_len(as_state_id(state)) == 0 {
            outcomes[state] = Some(Outcome::Draw);
            queue.push_back(as_state_id(state));
        }
    }

    while let Some(child) = queue.pop_front() {
        let child_outcome = outcomes[idx(child)].expect("queued states must be certified");
        for &pred in graph.predecessors(child) {
            let pred_index = idx(pred);
            if !graph.closed[pred_index] || outcomes[pred_index].is_some() {
                continue;
            }

            let mover = graph.to_move[pred_index];
            let mover_win = Outcome::for_player(mover);
            let opponent_win = Outcome::for_player(mover.opponent());

            if child_outcome == mover_win {
                outcomes[pred_index] = Some(mover_win);
                queue.push_back(pred);
            } else if child_outcome == opponent_win {
                remaining_for_loss[pred_index] -= 1;
                if remaining_for_loss[pred_index] == 0 {
                    outcomes[pred_index] = Some(opponent_win);
                    queue.push_back(pred);
                }
            }
        }
    }

    let (draw_candidates, mut certification_stats) = compute_draw_candidates(graph, &outcomes);
    let (mut draw_queue, scc_stats) = seed_draw_sccs(graph, &draw_candidates, &mut outcomes);
    certification_stats.draw_candidate_states = scc_stats.draw_candidate_states;
    certification_stats.draw_candidate_sccs = scc_stats.draw_candidate_sccs;
    certification_stats.cyclic_draw_candidate_sccs = scc_stats.cyclic_draw_candidate_sccs;
    certification_stats.cyclic_draw_candidate_states = scc_stats.cyclic_draw_candidate_states;
    certification_stats.largest_draw_candidate_scc = scc_stats.largest_draw_candidate_scc;
    certification_stats.largest_cyclic_draw_scc = scc_stats.largest_cyclic_draw_scc;

    while let Some(child) = draw_queue.pop_front() {
        for &pred in graph.predecessors(child) {
            let pred_index = idx(pred);
            if !draw_candidates[pred_index] || outcomes[pred_index].is_some() {
                continue;
            }

            if has_certified_draw_successor(graph, &outcomes, pred_index) {
                outcomes[pred_index] = Some(Outcome::Draw);
                draw_queue.push_back(pred);
            }
        }
    }

    CertifiedOutcomeResult {
        outcomes,
        stats: certification_stats,
    }
}

fn compute_draw_candidates(
    graph: &StateGraph,
    outcomes: &[Option<Outcome>],
) -> (Vec<bool>, CertificationStats) {
    let mut stats = CertificationStats::default();
    let mut draw_candidates = vec![false; graph.state_keys.len()];
    for state in 0..graph.state_keys.len() {
        draw_candidates[state] = graph.closed[state] && outcomes[state].is_none();
    }
    stats.draw_candidate_seed_states = draw_candidates.iter().filter(|&&state| state).count();

    let mut changed = true;
    while changed {
        changed = false;
        stats.draw_prune_iterations += 1;
        for state in 0..graph.state_keys.len() {
            if !draw_candidates[state] {
                continue;
            }

            let mover = graph.to_move[state];
            let mover_win = Outcome::for_player(mover);
            let opponent_win = Outcome::for_player(mover.opponent());
            let mut has_draw_successor = false;
            let mut valid = true;
            let mut removal_reason = None::<DrawPruneRemovalReason>;
            let (start, end) = graph.successor_bounds(as_state_id(state));

            graph.succ_store.for_each_in_range(start, end, |edge| {
                let child = idx(edge.to());
                if draw_candidates[child] || outcomes[child] == Some(Outcome::Draw) {
                    has_draw_successor = true;
                } else if outcomes[child] == Some(opponent_win) {
                    // The mover can avoid this losing branch if a draw-preserving move exists.
                } else if outcomes[child] == Some(mover_win) {
                    valid = false;
                    removal_reason.get_or_insert(DrawPruneRemovalReason::MoverWinExit);
                } else if !graph.closed[child] {
                    valid = false;
                    removal_reason.get_or_insert(DrawPruneRemovalReason::OpenOrUnknownExit);
                } else {
                    // Another unresolved child that is not itself in the candidate region
                    // can still hide a winning deviation for the mover.
                    valid = false;
                    removal_reason.get_or_insert(DrawPruneRemovalReason::OpenOrUnknownExit);
                }
            });

            if !valid || !has_draw_successor {
                draw_candidates[state] = false;
                changed = true;
                match removal_reason {
                    Some(DrawPruneRemovalReason::MoverWinExit) => {
                        stats.draw_prune_removed_mover_win_exit += 1;
                    }
                    Some(DrawPruneRemovalReason::OpenOrUnknownExit) => {
                        stats.draw_prune_removed_open_or_unknown_exit += 1;
                    }
                    None => {
                        stats.draw_prune_removed_no_draw_successor += 1;
                    }
                }
            }
        }
    }

    (draw_candidates, stats)
}

fn seed_draw_sccs(
    graph: &StateGraph,
    draw_candidates: &[bool],
    outcomes: &mut [Option<Outcome>],
) -> (VecDeque<StateId>, CertificationStats) {
    let mut visited = vec![false; graph.state_keys.len()];
    let mut order = Vec::new();
    let mut dfs_stack = Vec::<(StateId, usize)>::new();

    for state in 0..graph.state_keys.len() {
        if !draw_candidates[state] || visited[state] {
            continue;
        }

        visited[state] = true;
        dfs_stack.push((as_state_id(state), 0));
        while !dfs_stack.is_empty() {
            let (node, mut next_pred) = *dfs_stack.last().expect("stack should be non-empty");
            let preds = graph.predecessors(node);
            let mut pushed = false;
            while next_pred < preds.len() {
                let pred = preds[next_pred];
                next_pred += 1;
                let pred_index = idx(pred);
                if !draw_candidates[pred_index] || visited[pred_index] {
                    continue;
                }
                if let Some((_, stored_next_pred)) = dfs_stack.last_mut() {
                    *stored_next_pred = next_pred;
                }
                visited[pred_index] = true;
                dfs_stack.push((pred, 0));
                pushed = true;
                break;
            }

            if !pushed {
                if let Some((_, stored_next_pred)) = dfs_stack.last_mut() {
                    *stored_next_pred = next_pred;
                }
                order.push(node);
                dfs_stack.pop();
            }
        }
    }

    let mut assigned = vec![false; graph.state_keys.len()];
    let mut component_stack = Vec::<StateId>::new();
    let mut queue = VecDeque::new();
    let mut stats = CertificationStats {
        draw_candidate_states: draw_candidates.iter().filter(|&&state| state).count(),
        ..CertificationStats::default()
    };

    for state in order.into_iter().rev() {
        let state_index = idx(state);
        if !draw_candidates[state_index] || assigned[state_index] {
            continue;
        }

        let mut component = Vec::new();
        let mut has_self_loop = false;
        assigned[state_index] = true;
        component_stack.push(state);

        while let Some(node) = component_stack.pop() {
            component.push(node);
            let (start, end) = graph.successor_bounds(node);
            graph.succ_store.for_each_in_range(start, end, |edge| {
                let child = edge.to();
                let child_index = idx(child);
                if !draw_candidates[child_index] {
                    return;
                }
                if child == node {
                    has_self_loop = true;
                }
                if !assigned[child_index] {
                    assigned[child_index] = true;
                    component_stack.push(child);
                }
            });
        }

        stats.draw_candidate_sccs += 1;
        stats.largest_draw_candidate_scc = stats.largest_draw_candidate_scc.max(component.len());
        if component.len() > 1 || has_self_loop {
            stats.cyclic_draw_candidate_sccs += 1;
            stats.cyclic_draw_candidate_states += component.len();
            stats.largest_cyclic_draw_scc = stats.largest_cyclic_draw_scc.max(component.len());
            for state in component {
                let state_index = idx(state);
                if outcomes[state_index].is_none() {
                    outcomes[state_index] = Some(Outcome::Draw);
                    queue.push_back(state);
                }
            }
        }
    }

    (queue, stats)
}

fn has_certified_draw_successor(
    graph: &StateGraph,
    outcomes: &[Option<Outcome>],
    state: usize,
) -> bool {
    let (start, end) = graph.successor_bounds(as_state_id(state));
    let mut has_draw = false;
    graph.succ_store.for_each_in_range(start, end, |edge| {
        if outcomes[idx(edge.to())] == Some(Outcome::Draw) {
            has_draw = true;
        }
    });
    has_draw
}

fn compute_stats(
    graph: &StateGraph,
    outcomes: &[Outcome],
    certified_outcomes: Option<&[Option<Outcome>]>,
    certification_stats: Option<CertificationStats>,
    truncated: bool,
) -> GraphStats {
    let mut stats = GraphStats {
        states: graph.state_keys.len(),
        expanded_states: graph.expanded_states,
        closed_states: graph.closed.iter().filter(|&&closed| closed).count(),
        edges: graph.edge_count(),
        revisited_child_edges: graph.revisited_child_edges,
        self_loop_edges: graph.self_loop_edges,
        max_successors_per_state: graph.max_successors_per_state,
        resident_storage_bytes: graph.resident_storage_bytes(),
        disk_edge_bytes: graph.disk_edge_bytes(),
        state_table_peak_bytes: graph.state_table_peak_bytes,
        truncated: graph.truncated,
        ..GraphStats::default()
    };

    for (state, winner) in graph.winners.iter().copied().enumerate() {
        match winner {
            Some(Player::White) => stats.terminal_white_wins += 1,
            Some(Player::Black) => stats.terminal_black_wins += 1,
            None if state < graph.expanded_states
                && graph.successor_len(as_state_id(state)) == 0 =>
            {
                stats.sink_draws += 1;
            }
            None => {}
        }
    }

    if truncated {
        if let Some(certified_outcomes) = certified_outcomes {
            for outcome in certified_outcomes {
                match outcome {
                    Some(Outcome::WhiteWin) => stats.certified_white_wins += 1,
                    Some(Outcome::BlackWin) => stats.certified_black_wins += 1,
                    Some(Outcome::Draw) => stats.certified_draws += 1,
                    None => {}
                }
            }
        }
    }

    if !truncated {
        for outcome in outcomes {
            match outcome {
                Outcome::WhiteWin => stats.certified_white_wins += 1,
                Outcome::BlackWin => stats.certified_black_wins += 1,
                Outcome::Draw => stats.certified_draws += 1,
            }
        }
    }

    stats.certified_states =
        stats.certified_white_wins + stats.certified_black_wins + stats.certified_draws;
    stats.certified_unknown_states = stats.states - stats.certified_states;

    if let Some(certification_stats) = certification_stats {
        stats.draw_candidate_seed_states = certification_stats.draw_candidate_seed_states;
        stats.draw_candidate_states = certification_stats.draw_candidate_states;
        stats.draw_candidate_sccs = certification_stats.draw_candidate_sccs;
        stats.cyclic_draw_candidate_sccs = certification_stats.cyclic_draw_candidate_sccs;
        stats.cyclic_draw_candidate_states = certification_stats.cyclic_draw_candidate_states;
        stats.largest_draw_candidate_scc = certification_stats.largest_draw_candidate_scc;
        stats.largest_cyclic_draw_scc = certification_stats.largest_cyclic_draw_scc;
        stats.draw_prune_iterations = certification_stats.draw_prune_iterations;
        stats.draw_prune_removed_mover_win_exit =
            certification_stats.draw_prune_removed_mover_win_exit;
        stats.draw_prune_removed_open_or_unknown_exit =
            certification_stats.draw_prune_removed_open_or_unknown_exit;
        stats.draw_prune_removed_no_draw_successor =
            certification_stats.draw_prune_removed_no_draw_successor;
    }

    stats
}

#[must_use]
fn idx(state: StateId) -> usize {
    state as usize
}

#[must_use]
fn as_state_id(state: usize) -> StateId {
    u32::try_from(state).expect("state graph exceeded u32::MAX states")
}

#[must_use]
fn as_edge_offset(offset: usize) -> EdgeOffset {
    EdgeOffset::try_from(offset).expect("edge storage exceeded u64::MAX entries")
}

#[must_use]
fn as_loss_counter(successor_len: usize) -> LossCounter {
    debug_assert!(successor_len <= MAX_MOVES_PER_POSITION);
    LossCounter::try_from(successor_len)
        .expect("ChessBall positions should have at most MAX_MOVES_PER_POSITION legal moves")
}

#[must_use]
fn hash_packed_position(raw: u128) -> usize {
    let mixed = splitmix64(raw as u64 ^ (raw >> 64) as u64);
    mixed as usize
}

#[must_use]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[must_use]
fn square_from_index(index: u8) -> Square {
    let index = index as usize;
    debug_assert!(index < NUM_SQUARES);
    Square::new(index / COLS, index % COLS).expect("packed square index must be on the board")
}

#[must_use]
fn create_spill_path() -> PathBuf {
    let base = std::env::temp_dir();
    let pid = process::id();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..64u32 {
        let path = base.join(format!(
            "chessball_succ_edges_{pid}_{timestamp}_{attempt}.bin"
        ));
        if !path.exists() {
            return path;
        }
    }
    panic!("failed to allocate a unique spill file for ChessBall edges");
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use crate::engine::{Move, MoveKind, Piece, PieceKind, Player, Position, Symmetry, square};

    use super::{
        Edge, EdgeStorageMode, GraphStats, Outcome, PackedEdge, PackedMove, SolveResult,
        StateGraph, StateId, StateIdTable, SuccessorStore, WeakSolveConfig, as_edge_offset,
        certified_outcomes, solve_position,
    };

    fn synthetic_move(state: usize, edge_index: usize) -> Move {
        let from_index = ((state * 3) + edge_index) % 42;
        let to_index = (from_index + 1) % 42;
        Move {
            from: square(from_index / 7, from_index % 7),
            to: square(to_index / 7, to_index % 7),
            kind: MoveKind::Simple,
        }
    }

    fn synthetic_graph(
        to_move: Vec<Player>,
        winners: Vec<Option<Player>>,
        closed: Vec<bool>,
        succs: Vec<Vec<StateId>>,
    ) -> StateGraph {
        let mut succ_offsets = Vec::with_capacity(succs.len() + 1);
        let mut packed_edges = Vec::new();
        succ_offsets.push(0);
        for (state, edges) in succs.iter().enumerate() {
            for (edge_index, &to) in edges.iter().enumerate() {
                packed_edges.push(PackedEdge::new(
                    to,
                    synthetic_move(state, edge_index),
                    Symmetry::Identity,
                ));
            }
            succ_offsets.push(packed_edges.len() as u64);
        }

        let mut indegree = vec![0usize; succs.len()];
        for edges in &succs {
            for &to in edges {
                indegree[to as usize] += 1;
            }
        }

        let mut pred_offsets = vec![0u64; succs.len() + 1];
        for state in 0..succs.len() {
            pred_offsets[state + 1] = pred_offsets[state] + indegree[state] as u64;
        }
        let mut pred_ids = vec![0u32; packed_edges.len()];
        let mut heads = pred_offsets[..succs.len()].to_vec();
        for (state, edges) in succs.iter().enumerate() {
            for &to in edges {
                let head = &mut heads[to as usize];
                pred_ids[*head as usize] = state as u32;
                *head += 1;
            }
        }

        let max_successors_per_state = succs.iter().map(Vec::len).max().unwrap_or(0) as u8;

        StateGraph {
            start: 0,
            start_symmetry: Symmetry::Identity,
            state_keys: (0..succs.len())
                .map(|state| Position::empty(square(state / 7, state % 7), to_move[state]).pack())
                .collect(),
            to_move,
            winners,
            closed,
            max_successors_per_state,
            succ_offsets,
            succ_store: SuccessorStore::Memory(packed_edges),
            pred_offsets,
            pred_ids,
            expanded_states: succs.len(),
            state_table_peak_bytes: 0,
            revisited_child_edges: 0,
            self_loop_edges: 0,
            truncated: true,
        }
    }

    #[test]
    fn immediate_winning_position_is_classified_as_win() {
        let mut position = Position::empty(square(4, 2), Player::White);
        position.put_piece(
            square(3, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let winning_move = Move {
            from: square(3, 2),
            to: square(4, 2),
            kind: MoveKind::Push {
                ball_to: square(5, 2),
            },
        };

        let result = solve_position(position, WeakSolveConfig::default());
        assert_eq!(result.start_outcome(), Outcome::WhiteWin);
        assert_eq!(result.certified_start_outcome(), Some(Outcome::WhiteWin));
        assert_eq!(result.line_from_start(1)[0].2, Some(winning_move));
        assert_eq!(
            result
                .certified_line_from_start(2)
                .expect("exact solves are certified")
                .len(),
            2
        );
        assert!(result.exact);
    }

    #[test]
    fn state_id_table_resizes_and_recovers_inserted_keys() {
        let mut table = StateIdTable::with_capacity(1);
        let mut inserted = Vec::new();

        let mut id = 0u32;
        for row in 0..6 {
            for col in 0..7 {
                let key = Position::empty(square(row, col), Player::White).pack();
                table.insert(key, id);
                inserted.push((key, id));
                id += 1;
            }
        }

        assert!(table.peak_bytes() >= table.bytes());
        for (key, id) in inserted {
            assert_eq!(table.get(key), Some(id));
        }
    }

    #[test]
    fn edge_offsets_support_counts_beyond_u32_max() {
        let offset = as_edge_offset(u32::MAX as usize + 123);
        assert_eq!(offset, u32::MAX as u64 + 123);
    }

    #[test]
    fn checkpoints_do_not_change_solution() {
        let result = solve_position(
            Position::new_game(),
            WeakSolveConfig {
                max_states: Some(200),
                edge_storage: EdgeStorageMode::Disk,
                checkpoint_states: Some(50),
                checkpoint_seconds: Some(1),
            },
        );

        assert!(!result.exact);
        assert_eq!(result.start_outcome(), Outcome::Draw);
        assert_eq!(result.certified_start_outcome(), None);
        assert!(result.certified_line_from_start(4).is_none());
    }

    #[test]
    fn disk_backed_successors_preserve_solution_and_replay() {
        let mut position = Position::empty(square(4, 2), Player::White);
        position.put_piece(
            square(3, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );

        let result = solve_position(
            position,
            WeakSolveConfig {
                max_states: None,
                edge_storage: EdgeStorageMode::Disk,
                checkpoint_states: None,
                checkpoint_seconds: None,
            },
        );

        assert_eq!(result.start_outcome(), Outcome::WhiteWin);
        assert_eq!(result.graph.edge_storage_mode(), EdgeStorageMode::Disk);
        assert_eq!(result.stats.disk_edge_bytes, result.stats.edges * 8);
        assert!(!result.graph.successors(result.graph.start).is_empty());
        assert_eq!(result.line_from_start(1).len(), 1);
    }

    #[test]
    fn terminal_goal_row_position_is_classified_correctly() {
        let position = Position::empty(square(0, 3), Player::White);
        let result = solve_position(position, WeakSolveConfig::default());
        assert_eq!(result.start_outcome(), Outcome::BlackWin);
    }

    #[test]
    fn sink_without_goal_row_is_draw() {
        let position = Position::empty(square(2, 3), Player::White);
        let result = solve_position(position, WeakSolveConfig::default());
        assert_eq!(result.start_outcome(), Outcome::Draw);
        assert_eq!(result.stats.sink_draws, 1);
    }

    #[test]
    fn graph_limit_marks_result_as_inexact() {
        let result = solve_position(
            Position::new_game(),
            WeakSolveConfig {
                max_states: Some(1),
                edge_storage: EdgeStorageMode::Memory,
                checkpoint_states: None,
                checkpoint_seconds: None,
            },
        );
        assert!(!result.exact);
        assert!(result.graph.truncated);
    }

    #[test]
    fn mirrored_inputs_share_the_same_canonical_start_state() {
        let mut position = Position::empty(square(4, 2), Player::White);
        position.put_piece(
            square(3, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );
        let mirrored = position.mirrored_horizontal();

        let left = solve_position(position, WeakSolveConfig::default());
        let right = solve_position(mirrored, WeakSolveConfig::default());

        assert_eq!(
            left.graph.canonical_position(left.graph.start),
            right.graph.canonical_position(right.graph.start)
        );
        assert_eq!(
            right
                .graph
                .canonical_position(right.graph.start)
                .apply_symmetry(right.graph.start_symmetry),
            mirrored
        );

        let line = right.line_from_start(2);
        assert_eq!(line[0].0, mirrored);
        assert_eq!(line[0].1, Outcome::WhiteWin);
        assert_eq!(
            line[0].2,
            Some(Move {
                from: square(3, 4),
                to: square(4, 4),
                kind: MoveKind::Push {
                    ball_to: square(5, 4),
                },
            })
        );
    }

    #[test]
    fn rotated_color_swapped_inputs_do_not_share_the_same_canonical_start_state() {
        let mut position = Position::empty(square(4, 2), Player::White);
        position.put_piece(
            square(3, 2),
            Piece {
                player: Player::White,
                kind: PieceKind::Defender,
            },
        );
        let rotated = position.apply_symmetry(Symmetry::Rotate180SwapColors);

        let left = solve_position(position, WeakSolveConfig::default());
        let right = solve_position(rotated, WeakSolveConfig::default());

        assert_ne!(
            left.graph.canonical_position(left.graph.start),
            right.graph.canonical_position(right.graph.start)
        );
        assert_eq!(
            right
                .graph
                .canonical_position(right.graph.start)
                .apply_symmetry(right.graph.start_symmetry),
            rotated
        );
        assert_eq!(right.start_outcome(), Outcome::BlackWin);

        let line = right.line_from_start(2);
        assert_eq!(line[0].0, rotated);
        assert_eq!(line[0].1, Outcome::BlackWin);
        assert_eq!(
            line[0].2,
            Some(Move {
                from: square(2, 4),
                to: square(1, 4),
                kind: MoveKind::Push {
                    ball_to: square(0, 4),
                },
            })
        );
    }

    #[test]
    fn recommended_certified_edge_respects_actual_outcomes_under_color_swapping_symmetry() {
        let misleading_move = Move {
            from: square(2, 2),
            to: square(3, 2),
            kind: MoveKind::Simple,
        };
        let winning_move = Move {
            from: square(2, 2),
            to: square(2, 3),
            kind: MoveKind::Simple,
        };

        let graph = StateGraph {
            start: 0,
            start_symmetry: Symmetry::Identity,
            state_keys: vec![
                Position::empty(square(2, 2), Player::White).pack(),
                Position::empty(square(1, 1), Player::Black).pack(),
                Position::empty(square(4, 4), Player::Black).pack(),
            ],
            to_move: vec![Player::White, Player::Black, Player::Black],
            winners: vec![None, None, None],
            closed: vec![true, true, true],
            max_successors_per_state: 2,
            succ_offsets: vec![0, 2, 2, 2],
            succ_store: SuccessorStore::Memory(vec![
                PackedEdge::new(1, misleading_move, Symmetry::Rotate180SwapColors),
                PackedEdge::new(2, winning_move, Symmetry::Identity),
            ]),
            pred_offsets: vec![0, 0, 0, 0],
            pred_ids: Vec::new(),
            expanded_states: 3,
            state_table_peak_bytes: 0,
            revisited_child_edges: 0,
            self_loop_edges: 0,
            truncated: true,
        };
        let certified_outcomes = vec![
            Some(Outcome::WhiteWin),
            Some(Outcome::WhiteWin),
            Some(Outcome::WhiteWin),
        ];
        let result = SolveResult {
            graph,
            outcomes: vec![Outcome::WhiteWin, Outcome::WhiteWin, Outcome::WhiteWin],
            certified_outcomes: Some(certified_outcomes.clone()),
            exact: false,
            stats: GraphStats {
                states: 3,
                expanded_states: 3,
                closed_states: 3,
                edges: 2,
                revisited_child_edges: 0,
                self_loop_edges: 0,
                max_successors_per_state: 2,
                terminal_white_wins: 0,
                terminal_black_wins: 0,
                sink_draws: 0,
                certified_states: 3,
                certified_white_wins: 3,
                certified_black_wins: 0,
                certified_draws: 0,
                certified_unknown_states: 0,
                truncated: true,
                ..GraphStats::default()
            },
        };

        let chosen = result
            .recommended_certified_edge(0, Symmetry::Identity, &certified_outcomes)
            .expect("winning state should produce a certified edge");
        assert_eq!(chosen.mv, winning_move);
    }

    #[test]
    fn certified_outcomes_mark_closed_cycle_as_draw() {
        let graph = synthetic_graph(
            vec![Player::White, Player::Black],
            vec![None, None],
            vec![true, true],
            vec![vec![1], vec![0]],
        );

        let certified = certified_outcomes(&graph);
        assert_eq!(
            certified.outcomes,
            vec![Some(Outcome::Draw), Some(Outcome::Draw)]
        );
        assert_eq!(certified.stats.draw_candidate_seed_states, 2);
        assert_eq!(certified.stats.draw_candidate_states, 2);
        assert_eq!(certified.stats.draw_candidate_sccs, 1);
        assert_eq!(certified.stats.cyclic_draw_candidate_sccs, 1);
        assert_eq!(certified.stats.cyclic_draw_candidate_states, 2);
        assert_eq!(certified.stats.draw_prune_removed_mover_win_exit, 0);
        assert_eq!(certified.stats.draw_prune_removed_open_or_unknown_exit, 0);
        assert_eq!(certified.stats.draw_prune_removed_no_draw_successor, 0);
    }

    #[test]
    fn certified_outcomes_allow_draw_with_avoidable_losing_exit() {
        let graph = synthetic_graph(
            vec![Player::White, Player::Black, Player::White],
            vec![None, None, Some(Player::Black)],
            vec![true, true, true],
            vec![vec![1, 2], vec![0], vec![]],
        );

        let certified = certified_outcomes(&graph);
        assert_eq!(certified.outcomes[0], Some(Outcome::Draw));
        assert_eq!(certified.outcomes[1], Some(Outcome::Draw));
        assert_eq!(certified.outcomes[2], Some(Outcome::BlackWin));
        assert_eq!(certified.stats.draw_candidate_seed_states, 2);
        assert_eq!(certified.stats.draw_prune_removed_mover_win_exit, 0);
        assert_eq!(certified.stats.draw_prune_removed_open_or_unknown_exit, 0);
    }

    #[test]
    fn certified_outcomes_reject_draw_when_open_exit_remains() {
        let graph = synthetic_graph(
            vec![Player::White, Player::Black, Player::White],
            vec![None, None, None],
            vec![true, true, false],
            vec![vec![1, 2], vec![0], vec![]],
        );

        let certified = certified_outcomes(&graph);
        assert_eq!(certified.outcomes[0], None);
        assert_eq!(certified.outcomes[1], None);
        assert_eq!(certified.outcomes[2], None);
        assert_eq!(certified.stats.draw_candidate_seed_states, 2);
        assert_eq!(certified.stats.draw_candidate_states, 0);
        assert_eq!(certified.stats.draw_prune_removed_open_or_unknown_exit, 2);
    }

    #[test]
    fn packed_move_round_trips_all_move_kinds() {
        let moves = [
            Move {
                from: square(1, 1),
                to: square(2, 2),
                kind: MoveKind::Simple,
            },
            Move {
                from: square(1, 2),
                to: square(2, 2),
                kind: MoveKind::Push {
                    ball_to: square(3, 2),
                },
            },
            Move {
                from: square(1, 1),
                to: square(3, 3),
                kind: MoveKind::Jump {
                    jumped: square(2, 2),
                },
            },
            Move {
                from: square(1, 1),
                to: square(1, 2),
                kind: MoveKind::Tackle {
                    pushed_from: square(1, 2),
                    pushed_to: square(1, 3),
                },
            },
        ];

        for mv in moves {
            assert_eq!(PackedMove::new(mv).unpack(), mv);
        }
    }

    #[test]
    fn packed_edge_round_trips_and_is_smaller_than_unpacked_edge() {
        let edge = Edge {
            mv: Move {
                from: square(1, 1),
                to: square(2, 2),
                kind: MoveKind::Jump {
                    jumped: square(1, 2),
                },
            },
            to: 1234,
            child_symmetry: Symmetry::MirrorVerticalSwapColors,
        };

        let packed = PackedEdge::new(edge.to, edge.mv, edge.child_symmetry);
        assert_eq!(packed.unpack(), edge);
        assert!(size_of::<PackedEdge>() < size_of::<Edge>());
    }
}
