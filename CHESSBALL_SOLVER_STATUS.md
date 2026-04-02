# ChessBall Solver Status

## Scope

This note documents the current state of the Rust ChessBall implementation and solver tooling in this repository.

It reflects the code currently under:

- [rust_chessball](rust_chessball)

and the current official rules document:

- [CHESSBALL_RULES_OFFICIAL_SPEC.md](CHESSBALL_RULES_OFFICIAL_SPEC.md)

## Current Bottom Line

- The game is **not weakly solved yet**.
- The current proof model used by the exact solver is:
  - goal on the opponent goal line
  - Blue/White starts from the top side
  - infinite play is treated as `Draw`
- Large exact runs currently end with:
  - `partial_start_outcome=Draw`
  - `certified_start_outcome=<none>`
- The solver is already proving many local `WhiteWin` and `BlackWin` positions.
- It is **not yet proving any draw positions** in the large explored graphs.

## What Has Been Implemented

### Rules Engine

Canonical rules implementation in:

- [engine.rs](rust_chessball/src/engine.rs)

Implemented features:

- official `7 x 6` board
- official initial setup
- attackers, defenders, ball
- simple moves, pushes, attacker jumps, defender tackles
- touch-zone restriction for ball pushes
- tackle-memory rule
- goal detection
- packed position encoding for solver use

### Search / Solver Binaries

Available binaries:

- [solve_start.rs](rust_chessball/src/bin/solve_start.rs)
  Exact graph-based weak-solve attempt from the initial position
- [partial_tablebase.rs](rust_chessball/src/bin/partial_tablebase.rs)
  Conservative proof-oriented partial tablebase export
- [selfplay.rs](rust_chessball/src/bin/selfplay.rs)
  Self-play tournament runner
- [replay.rs](rust_chessball/src/bin/replay.rs)
  Replay exported game records

Core solver modules:

- [weak_solve.rs](rust_chessball/src/weak_solve.rs)
- [partial_tablebase.rs](rust_chessball/src/partial_tablebase.rs)
- [solver.rs](rust_chessball/src/solver.rs)

### Weak-Solve Infrastructure

The exact solver in [weak_solve.rs](rust_chessball/src/weak_solve.rs) currently includes:

- graph exploration from the initial state
- horizontal symmetry reduction only
- packed state representation
- custom dense state table for visited-state deduplication
- optional disk-backed successor edge storage
- flat predecessor arrays for retrograde propagation
- exact retrograde `Win/Loss/Draw` pass on the explored graph
- stricter truncated-run certification pass
- SCC-based draw-certifier on the closed unresolved subgraph
- solve-only compact reporting mode
- checkpoint reporting for:
  - expand
  - predecessor count
  - predecessor fill
  - retrograde

### Partial Tablebase Tooling

The partial-tablebase path currently supports:

- conservative proof-certified outcomes
- proof-oriented export mode
- browser export for certified positions and paths
- proof summary and proof-child display
- optional omission of large full-graph exports via `--proof-only`

This is useful for inspecting local proof fragments, but it is **not** the main overnight weak-solve path.

## Current Exact-Solver Model

The exact solver currently reports:

- `partial_start_outcome`
  heuristic hint on a truncated graph
- `certified_start_outcome`
  certified only if the current explored closed subgraph is enough
- `exact=true`
  only when the reachable graph has been fully explored under the current cap/model

Current command-line entrypoint:

- [solve_start.rs](rust_chessball/src/bin/solve_start.rs)

Relevant options:

- `--max-states N`
- `--disk-edges`
- `--solve-only`
- `--checkpoint-states N`
- `--checkpoint-seconds N`
- `--line-plies N`

## How To Run

### Build

```bash
cd /Users/mathieuacher/SANDBOX/chessball/rust_chessball
cargo build --release --bin solve_start --bin partial_tablebase
```

### Exact Weak-Solve Attempt

Recommended overnight command:

```bash
cd /Users/mathieuacher/SANDBOX/chessball/rust_chessball

nohup caffeinate -s target/release/solve_start \
  --solve-only \
  --max-states 1400000000 \
  --disk-edges \
  --checkpoint-states 5000000 \
  --checkpoint-seconds 30 \
  > overnight_solve_14b.out 2> overnight_solve_14b.err &
```

Read progress with:

```bash
tail -f overnight_solve_14b.err
cat overnight_solve_14b.out
```

### Partial Tablebase Export

Proof-oriented export with small artifacts:

```bash
cd /Users/mathieuacher/SANDBOX/chessball/rust_chessball

target/release/partial_tablebase \
  --max-states 1000000 \
  --proof-only \
  --certified-per-outcome 20 \
  --min-proof-plies 2 \
  --prefer-long-proofs \
  --out partial_tb_proofs \
  --line-plies 0
```

## Interpreting `solve_start`

Important output fields:

- `exact`
  `true` means the start position is solved under the current proof model
- `partial_start_outcome`
  hint only on truncated runs
- `certified_start_outcome`
  strongest partial proof currently available for the start
- `certified_white_wins`, `certified_black_wins`, `certified_draws`
  rigorously certified positions in the explored graph
- `draw_candidate_seed_states`
  closed unresolved states before draw pruning
- `draw_candidate_states`
  states still surviving draw pruning
- `cyclic_draw_candidate_sccs`
  cyclic SCCs available to seed certified draws
- `revisited_child_edges`
  generated edges that hit an already-known canonical position

## Current Large-Run Results

### 1.0B discovered states

Observed result:

- `exact=false`
- `states=1000000000`
- `expanded_states=167381068`
- `closed_states=171101864`
- `certified_states=5921225`
- `certified_white_wins=3262894`
- `certified_black_wins=2658331`
- `certified_draws=0`
- `certified_start_outcome=<none>`
- `partial_start_outcome=Draw`

Interpretation:

- many local wins/losses are proved
- no certified draw region
- no certified verdict yet for the start

### 1.2B discovered states

Observed result:

- `exact=false`
- `states=1200000000`
- `expanded_states=206180134`
- `closed_states=211493177`
- `certified_states=9287436`
- `certified_white_wins=6477030`
- `certified_black_wins=2810406`
- `certified_draws=0`
- `certified_start_outcome=<none>`
- `partial_start_outcome=Draw`

Interpretation:

- more local wins/losses are proved
- still no certified draws
- start still unresolved

### 1.4B discovered states

Observed result:

- `exact=false`
- `states=1400000000`
- `expanded_states=247047221`
- `closed_states=253563504`
- `certified_states=10904478`
- `certified_white_wins=6694132`
- `certified_black_wins=4210346`
- `certified_draws=0`
- `certified_start_outcome=<none>`
- `partial_start_outcome=Draw`
- `draw_candidate_states=0`
- `draw_candidate_sccs=0`
- `cyclic_draw_candidate_sccs=0`
- `revisited_child_edges=5356136600`
- `edges=6756136572`

Interpretation:

- the exact solver is stable at this scale
- transpositions/repeated positions are extremely common
- the current blocker for draw proof is not tactical winning exits but unresolved/open successors
- the draw certifier sees no surviving draw candidate region after pruning

## Complexity Notes

### Theoretical Upper Bound

A loose combinatorial upper bound on raw states, including side to move and rough tackle-memory multiplicity, is on the order of:

- about `2.3e15` positions

This is only an upper bound:

- many positions are unreachable
- the solver canonicalizes horizontal symmetry
- move-history legality reduces the actual reachable set

### Empirical Branching

From the large exact runs:

- average branching on expanded states is about `27.3`
- observed max branching so far is `40`
- theoretical hard move bound in the engine is `56`

### Transpositions / Repetitions

Repeated positions are already merged by the state table during graph construction.

Empirical signal:

- at `1.4B`, `revisited_child_edges=5356136600`
- with `edges=6756136572`

So about `79%` of generated child edges hit an already-known canonical state.

This means:

- the game graph is much smaller than the naive game tree
- repetitions/transpositions are a dominant structural property of the game

## What The Current Diagnostics Show

Recent diagnostic counters show:

- large numbers of initial draw seeds exist before pruning
- they are then eliminated almost entirely because of `open_or_unknown_exit`
- not because of:
  - `mover_win_exit`
  - `no_draw_successor`

This suggests the main current obstacle is:

- too much frontier/open graph around candidate draw regions

not:

- a lack of cycles
- a tactical proof that all unresolved regions are actually wins/losses

## Current Limitations

- no full weak solve yet
- no certified draw for the start
- no certified draw positions in the large runs explored so far
- certification after retrograde is still silent in the logs
- no resumable exact graph checkpoints yet
- very large runs are RAM- and graph-closure-limited

## Practical Recommendation

For overnight experiments, use `solve_start` with `--solve-only`.

Use `partial_tablebase` only when you want inspectable proof fragments or browser exports.

If larger runs still show:

- `certified_draws=0`
- `draw_candidate_states=0`
- `draw_prune_removed_open_or_unknown_exit` dominating

then the next useful work is not just a larger cap, but further algorithmic work on graph closure / certification.

## Verification

Most recently re-verified locally:

```bash
cd /Users/mathieuacher/SANDBOX/chessball/rust_chessball
cargo test weak_solve:: -- --nocapture
cargo build --release --bin solve_start --bin partial_tablebase
target/release/solve_start --max-states 5000 --disk-edges --solve-only
target/release/solve_start --max-states 1000000 --disk-edges --solve-only
```

The status above is based on those checks plus the recorded large overnight runs.
