use std::collections::HashMap;

use crate::engine::{Move, MoveKind, Player, Position, ROWS, Square};

const INF: i32 = 1_000_000_000;
const MATE_SCORE: i32 = 10_000_000;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: i32,
    pub nodes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TableEntry {
    depth: u8,
    score: i32,
    bound: Bound,
    best_move: Option<Move>,
}

#[derive(Debug, Default)]
pub struct Searcher {
    table: HashMap<Position, TableEntry>,
    nodes: u64,
}

impl Searcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.table.clear();
        self.nodes = 0;
    }

    #[must_use]
    pub fn best_move(&mut self, position: &Position, depth: u8) -> SearchResult {
        self.nodes = 0;
        let mut alpha = -INF;
        let beta = INF;
        let hash_move = self.table.get(position).and_then(|entry| entry.best_move);
        let moves = self.ordered_moves(position, hash_move);

        if moves.is_empty() {
            return SearchResult {
                best_move: None,
                score: self.evaluate(position),
                nodes: self.nodes,
            };
        }

        let mut best_move = None;
        let mut best_score = -INF;
        for mv in moves {
            let child = position.apply(mv);
            let score = -self.negamax(&child, depth.saturating_sub(1), -beta, -alpha, 1);
            if score > best_score {
                best_score = score;
                best_move = Some(mv);
            }
            alpha = alpha.max(score);
        }

        SearchResult {
            best_move,
            score: best_score,
            nodes: self.nodes,
        }
    }

    fn negamax(
        &mut self,
        position: &Position,
        depth: u8,
        mut alpha: i32,
        beta: i32,
        ply: i32,
    ) -> i32 {
        self.nodes += 1;
        if let Some(winner) = position.winner() {
            return if winner == position.to_move {
                MATE_SCORE - ply
            } else {
                -MATE_SCORE + ply
            };
        }

        if depth == 0 {
            return self.evaluate(position);
        }

        let alpha_orig = alpha;
        let mut hash_move = None;
        if let Some(entry) = self.table.get(position).copied()
            && entry.depth >= depth
        {
            hash_move = entry.best_move;
            match entry.bound {
                Bound::Exact => return entry.score,
                Bound::Lower => alpha = alpha.max(entry.score),
                Bound::Upper => {}
            }
            if alpha >= beta {
                return entry.score;
            }
        }

        let moves = self.ordered_moves(position, hash_move);
        if moves.is_empty() {
            return self.evaluate(position);
        }

        let mut best_move = None;
        let mut best_score = -INF;
        for mv in moves {
            let child = position.apply(mv);
            let score = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1);
            if score > best_score {
                best_score = score;
                best_move = Some(mv);
            }
            alpha = alpha.max(score);
            if alpha >= beta {
                break;
            }
        }

        let bound = if best_score <= alpha_orig {
            Bound::Upper
        } else if best_score >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        self.table.insert(
            *position,
            TableEntry {
                depth,
                score: best_score,
                bound,
                best_move,
            },
        );
        best_score
    }

    fn ordered_moves(&self, position: &Position, hash_move: Option<Move>) -> Vec<Move> {
        let mut moves = position.legal_moves();
        moves.sort_by_key(|mv| self.move_order_score(position, *mv, hash_move));
        moves.reverse();
        moves
    }

    fn move_order_score(&self, position: &Position, mv: Move, hash_move: Option<Move>) -> i32 {
        if hash_move == Some(mv) {
            return 10_000_000;
        }
        match mv.kind {
            MoveKind::Push { ball_to } => {
                let mut score = 500_000 + self.ball_progress(ball_to, position.to_move) * 1_000;
                let winner = match ball_to.row() {
                    row if row == Player::White.goal_row() => Some(Player::White),
                    row if row == Player::Black.goal_row() => Some(Player::Black),
                    _ => None,
                };
                if winner == Some(position.to_move) {
                    score += 1_000_000;
                } else if winner == Some(position.to_move.opponent()) {
                    score -= 1_000_000;
                }
                score
            }
            MoveKind::Tackle { .. } => 50_000,
            MoveKind::Jump { .. } => 10_000,
            MoveKind::Simple => 0,
        }
    }

    fn evaluate(&self, position: &Position) -> i32 {
        let player = position.to_move;
        let opponent = player.opponent();

        let mut opponent_position = *position;
        opponent_position.to_move = opponent;

        let player_ball = self.ball_progress(position.ball(), player);
        let opponent_ball = self.ball_progress(position.ball(), opponent);
        let mobility = position.legal_moves().len() as i32;
        let opponent_mobility = opponent_position.legal_moves().len() as i32;
        let pushers = self.adjacent_pushers(position, player) as i32
            - self.adjacent_pushers(position, opponent) as i32;
        let control = self.control_around_ball(position, player) as i32
            - self.control_around_ball(position, opponent) as i32;

        (player_ball - opponent_ball) * 120
            + (mobility - opponent_mobility) * 12
            + pushers * 30
            + control * 8
    }

    fn adjacent_pushers(&self, position: &Position, player: Player) -> usize {
        let ball = position.ball();
        let mut count = 0;
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
        let mut count = 0;
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

#[cfg(test)]
mod tests {
    use crate::engine::{Move, MoveKind, Piece, PieceKind, Player, Position, square};

    use super::Searcher;

    #[test]
    fn search_finds_immediate_winning_push() {
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

        let mut searcher = Searcher::new();
        let result = searcher.best_move(&position, 3);
        assert_eq!(result.best_move, Some(winning));
        assert!(result.score > 1_000_000);
        assert!(result.nodes > 0);
    }

    #[test]
    fn search_returns_some_move_from_start_position() {
        let position = Position::new_game();
        let mut searcher = Searcher::new();
        let result = searcher.best_move(&position, 2);
        assert!(result.best_move.is_some());
        assert!(result.nodes > 0);
    }
}
