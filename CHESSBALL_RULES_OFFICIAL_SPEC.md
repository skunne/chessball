# ChessBall Official Rules Specification

This document is the authoritative ruleset for the canonical Rust engine, solver, and browsers in this repository.

The older repository-inferred document, [CHESSBALL_RULES_SPEC.md](CHESSBALL_RULES_SPEC.md), is kept for historical reference. When the two documents differ, this file wins.

## Implementation Mapping

The code keeps the historical internal names:

- `White` = official **Blue**
- `Black` = official **Red**

That mapping is only an implementation detail. The official game is Blue vs Red.

## 1. Game Entities

- Two players: **Blue** and **Red**
- Each player has exactly 5 pieces:
  - 2 attackers
  - 3 defenders
- One neutral ball

## 2. Board and Coordinates

- The board is **7 columns x 6 rows**
- Coordinates are `(x, y)`:
  - `x in {0..6}` from left to right
  - `y in {0..5}` from top to bottom
- Blue starts on the **top** side
- Red starts on the **bottom** side

The web UI and record notation also use files `a..g` and ranks `6..1`, with rank `6` on the top row and rank `1` on the bottom row.

## 3. Goal Lines and Touch Zones

- Blue goal line: all squares with `y = 0`
- Red goal line: all squares with `y = 5`

Touch zones are the side middle squares:

- `(0,1)`, `(0,2)`, `(0,3)`, `(0,4)`
- `(6,1)`, `(6,2)`, `(6,3)`, `(6,4)`

Restrictions:

- The **ball may not be pushed into** a touch-zone square
- Player pieces **may** occupy touch-zone squares

## 4. Initial Position

Official setup:

- Blue defenders: `(1,0)`, `(3,0)`, `(5,0)`
- Blue attackers: `(2,1)`, `(4,1)`
- Red attackers: `(2,4)`, `(4,4)`
- Red defenders: `(1,5)`, `(3,5)`, `(5,5)`
- Ball: `(3,2)`
- Blue moves first

In the engine's internal `W/B` tokens, the start position is:

```text
-- WD -- WD -- WD --
-- -- WA -- WA -- --
-- -- -- NB -- -- --
-- -- -- -- -- -- --
-- -- BA -- BA -- --
-- BD -- BD -- BD --
```

Here `W = Blue`, `B = Red`, and `NB = ball`.

## 5. Turn Structure

Players alternate turns.

On each turn, the side to move performs exactly one legal action with exactly one of its own pieces.

All move directions are allowed: orthogonal and diagonal.

## 6. Legal Move Types

### 6.1 Simple Move

Attackers and defenders may move one square in any direction if the destination is on the board and empty.

### 6.2 Ball Push

Attackers and defenders may push the ball one square in any direction if:

- the ball is adjacent in that direction
- the square beyond the ball is on the board
- that destination square is empty
- that destination square is not in a touch zone

Effect:

- the piece moves into the ball's previous square
- the ball moves one square farther in the same direction

### 6.3 Defender Tackle

Only defenders may tackle.

A tackle is legal if:

- an adjacent square contains an opponent piece
- the square beyond it in the same direction is on the board
- that farther square is empty
- the move is not the forbidden immediate retaliation described in Section 7

Effect:

- the defender moves into the opponent piece's square
- the opponent piece is pushed one square farther in the same direction
- the ball does not move

### 6.4 Attacker Jump

Only attackers may jump.

A jump is legal if:

- the adjacent square is occupied by either a piece or the ball
- the square immediately beyond it in the same direction is on the board
- the landing square is empty
- the move is not the forbidden immediate retaliation described in Section 7

Effect:

- the attacker moves to the landing square
- the jumped piece or ball stays in place

## 7. Immediate Retaliation Restriction After a Tackle

The game remembers only the immediately previous tackle.

If a defender has just tackled a piece:

- the tackled attacker may not immediately jump back over that defender
- the tackled defender may not immediately tackle back through that defender

Any non-tackle move clears this memory.

## 8. Win Condition

A player wins by pushing the ball onto the **opponent's** goal line:

- Blue wins immediately when the ball reaches `y = 5`
- Red wins immediately when the ball reaches `y = 0`

Own goals are possible. The winner is determined by the goal line reached, not by which side made the move.

## 9. Undefined Cases

The official rules define goal scoring, but do not define:

- a draw rule
- a no-legal-move rule
- any repetition rule

Analysis code may adopt additional conventions, but those are solver policies, not official game rules.
