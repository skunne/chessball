# Is ChessBall a draw?

Stephan Kunne, Mathieu Acher, maybe Theo Matricon wants in too

## Introduction

Board games are a sandbox for AI and algorithms, blablabla

## Presenting ChessBall

Rules of ChessBall. A small chessboard, 2 attackers and 2 defenders per player, one ball at the centre of the board.

## Proving that ChessBall is a draw

### Intuition

The complexity is not that much. (Calculation of breadth and depth of the tree). Defending seems easier than attacking. If both teams push the ball towards the centre, it's just going to get stuck there!

### Method 1: reasoning from the end

Method :
* List all positions in which Opponent can win with one move (thereafter "opponent-win positions")
* For each opponent-win position, list all possible last move played by Player (thereafter "dangerous positions")
* For each dangerous position, show that Player has a move which doesn't result in an opponent-win position.

Failure of the method:
* There are way too many (number) opponent-win positions.
* However, most of those opponent-win positions are hopeless unreasonable position in which Player had already lost many moves ago and Opponent was just prolongating the agony. For example, positions in which all of Player's pieces are far from the ball in a useless corner of the board.

### Method 2: reasoning from the start

Method:
* Focus on a relatively simple defensive strategy that tries to block the ball without doing anything fancy
* Prove that this strategy cannot be beaten.
* Use a simple invariant such as "row number of the ball" and use this invariant both as a heuristic for the strategy and as an invariant in the proof. 

### Implementation in Rust

See github repo.

* Implementation of ChessBallBoard, MoveInfo
* Functions possible_moves and possible_previous_moves to generate all moves from a given position
* First attempts at a tree search with heuristics.

### Conclusion

### Acknowledgements

* Github Copilot was used to help write some of the Rust code
* Thank you to whoever gifted the ChessBall set to Mathieu

### Bibliography