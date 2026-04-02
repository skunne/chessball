# ChessBall Rules Specification

This document formalizes the rules of ChessBall using only the material already present in this repository. It is a consolidation, not an external reconstruction.

## Scope and source basis

This specification is inferred from:

- `paper/report.md` for the intended high-level rules.
- `rust_chessball/src/board.rs`, `rust_chessball/src/moves.rs`, and `rust_chessball/src/winning_moves.rs` for the main executable rules.
- `python_chessball/chessball_board.py` and related helpers as an earlier prototype when they agree with the paper and Rust code.

When the repository disagrees with itself, this document chooses the rule supported by the paper and by the largest internally consistent subset of the codebase. The conflicting details are listed explicitly in the final section.

## 1. Game entities

ChessBall is a two-player deterministic turn-based game between White and Black.

The standard game state contains:

- 1 White team: 2 attackers and 3 defenders.
- 1 Black team: 2 attackers and 3 defenders.
- 1 neutral ball.
- 1 rectangular board with 6 rows and 7 columns.

Coordinates are written as `(r, c)` with:

- `r in {0, 1, 2, 3, 4, 5}` from top to bottom.
- `c in {0, 1, 2, 3, 4, 5, 6}` from left to right.

White's goal row is row `5`. Black's goal row is row `0`.

The ball is neutral. It is not carried by a player; it occupies its own square.

## 2. Initial position

The initial position used by the Rust implementation is:

```text
-- BD -- BD -- BD --
-- -- BA -- BA -- --
-- -- -- NB -- -- --
-- -- -- -- -- -- --
-- -- WA -- WA -- --
-- WD -- WD -- WD --
```

Meaning:

- Black defenders start at `(0,1)`, `(0,3)`, `(0,5)`.
- Black attackers start at `(1,2)`, `(1,4)`.
- White attackers start at `(4,2)`, `(4,4)`.
- White defenders start at `(5,1)`, `(5,3)`, `(5,5)`.
- The ball starts at `(2,3)`.

This matches the paper's description that the ball starts near the center and is slightly closer to the second player.

In the runnable Rust game loop, White moves first.

## 3. Adjacency and directions

All movement rules use the 8 neighboring directions:

- orthogonal: up, down, left, right
- diagonal: up-left, up-right, down-left, down-right

Formally, the direction set is:

`Delta = {(-1,0), (1,0), (0,-1), (0,1), (-1,-1), (-1,1), (1,-1), (1,1)}`

For any square `s = (r, c)` and direction `d = (dr, dc)`, write:

- `s + d = (r + dr, c + dc)`
- `s + 2d = (r + 2dr, c + 2dc)`

A square is usable only if it remains on the board.

## 4. Turn structure

Players alternate turns.

On a turn, the player to move chooses exactly one of their own pieces and performs exactly one legal move.

Only attackers and defenders belong to players. The ball never moves by itself; it moves only when pushed.

## 5. Legal moves

Let `P` be the player to move, let `x` be one of `P`'s pieces on square `s`, and let `d in Delta`.

### 5.1 Simple move

A simple move is legal if:

- `s + d` is on the board, and
- `s + d` is empty.

Effect:

- move `x` from `s` to `s + d`.

Any player piece may make a simple move.

### 5.2 Ball push

A ball push is legal if:

- `s + d` is occupied by the ball,
- `s + 2d` is on the board,
- `s + 2d` is empty, and
- the destination column of the ball is not an outer column.

The last condition means the ball may not be pushed to column `0` or column `6`.

Effect:

- move `x` from `s` to `s + d`,
- move the ball from `s + d` to `s + 2d`.

Any player piece may push the ball.

### 5.3 Attacker jump

An attacker jump is legal only for attackers.

It is legal if:

- `s + d` is on the board,
- `s + 2d` is on the board,
- `s + d` is occupied by any non-ball piece, and
- `s + 2d` is empty.

Effect:

- move the attacker from `s` to `s + 2d`,
- leave the jumped piece unchanged on `s + d`.

No capture occurs.

### 5.4 Defender tackle

A defender tackle is legal only for defenders.

It is legal if:

- `s + d` is on the board,
- `s + 2d` is on the board,
- `s + d` is occupied by an opponent piece,
- that adjacent opponent piece is not the ball, and
- `s + 2d` is empty.

Effect:

- move the defender from `s` to `s + d`,
- push the opponent piece from `s + d` to `s + 2d`.

No capture occurs.

## 6. Anti-immediate-revenge rule after a tackle

The paper adds two special restrictions:

- an attacker cannot jump over a defender that has just tackled it
- a defender cannot tackle back over a defender that has just tackled it

The Rust implementation models this with one-step tackle memory.

Define `last_tackle = (a, b)` to mean:

- on the immediately preceding move, a defender moved into square `a`
- the tackled opposing piece was pushed from `a` to `b`

Then, on the very next move only:

- an attacker on `b` may not jump in the direction that would jump over `a`
- a defender on `b` may not tackle in the direction that would move into `a`

After any non-tackle move, this memory is cleared. After a new tackle, it is overwritten by the new tackle.

## 7. Winning condition

A player wins immediately after making a move that leaves the ball on that player's goal row.

Therefore:

- White wins if, after White's move, the ball is on row `5`.
- Black wins if, after Black's move, the ball is on row `0`.

The repository does not define any separate win, loss, or draw rule for "no legal moves." The only explicit terminal condition implemented in both Python and Rust is reaching the mover's goal row with the ball.

## 8. Invariants of legal play

Under the consolidated rules above, legal play preserves the following:

- each side keeps exactly 2 attackers and 3 defenders
- there is no capture
- there is exactly one ball
- the ball remains neutral
- the ball is never moved onto the outer columns

The move rules are local: every move acts along one of the 8 directions and changes only the directly involved squares.

## 9. Consolidated formal game definition

A normal ChessBall game can therefore be defined by:

- the initial state in Section 2
- White to move first
- alternating turns
- legal moves exactly as in Section 5 plus Section 6
- immediate win on reaching the mover's goal row with the ball

This is the cleanest ruleset supported by the repository as a whole.

## 10. Repository inconsistencies and gaps

The repository does not currently present a single perfectly consistent implementation. The main issues are:

### 10.1 Python board dimensions are transposed

`python_chessball/chessball_board.py` defines:

- `ROWS = 7`
- `COLS = 6`

This conflicts with:

- the paper, which says "6 rows 7 files"
- the Rust board, which uses `6 x 7`
- the Rust initial position string, which has 6 rows and 7 columns

The Python tests also show this mismatch: some tests try to place pieces at column `6`, which is invalid on the Python board.

### 10.2 Rust CLI comments still describe the wrong geometry

`rust_chessball/src/main.rs` still says the board has 6 columns and 7 rows, and refers to files `a..f` and ranks `1..7`.

That comment is inconsistent with the actual Rust board implementation and the actual start position, which are `7` columns and `6` rows.

### 10.3 Forbidden ball-destination columns are intended, but Rust move generation does not enforce them

Evidence for forbidden outer columns appears in multiple places:

- Python `possible_moves` explicitly forbids ball destinations on the outer columns.
- Rust `ChessBallBoard` defines `is_forbidden_col`.
- Rust and Python heuristics assume those columns are forbidden for ball pushes.

However, `rust_chessball/src/moves.rs` currently allows a ball push whenever the destination square is on the board and empty; it does not check whether that destination is an outer column.

So the consolidated specification treats the outer columns as forbidden for ball destinations, but the current Rust move generator is more permissive than that rule.

### 10.4 The tackle-memory rule exists in the paper and Rust, but not in Python

The paper states the "just tackled" restrictions explicitly.

Rust implements them through `prev_tackle`.

The Python prototype does not track tackle memory, so it does not enforce those restrictions.

### 10.5 Rust retrograde move generation is not implemented

`rust_chessball/src/moves.rs` currently returns an empty vector for `possible_previous_moves`, while the Python prototype contains a real retrograde generator and the Rust tests still expect non-empty behavior.

This does not change the forward rules of the game, but it means the repo is incomplete as an analysis tool.

### 10.6 Non-standard positions are accepted by the tooling

The parsers and board constructors can represent arbitrary board states, including states that are not reachable from the standard initial position.

That is useful for analysis, but it means not every position the code can load should be treated as a legal game position under the consolidated rules above.
