from typing import Optional, Tuple, Dict
import math

from chessball_board import ChessBallBoard, Player, possible_moves
from heuristics import evaluate, _DEFAULT_WEIGHTS, feature_vector
from winning_moves import winning_moves

INF = 10**9


def _has_immediate_win(board: ChessBallBoard, player: Player) -> Optional[Tuple[Dict, ChessBallBoard]]:
    """
    Return the first immediate winning move and resulting board for `player` if any,
    otherwise None.
    """
    wm = winning_moves(board, player)
    # winning_moves yields (move, board_after) pairs — return the first if present
    for move_and_board in wm:
        # move_and_board may be a tuple (move, board_after) or similar
        try:
            move, board_after = move_and_board
            return move, board_after
        except Exception:
            # If winning_moves returns a different structure, skip gracefully
            continue
    return None


def choose_best_move(
    board: ChessBallBoard,
    player: Player,
    depth: int = 3,
    weights: Optional[Dict[str, float]] = None,
    use_alpha_beta: bool = True
) -> Tuple[Optional[Dict], Optional[ChessBallBoard], float]:
    """
    Choose the best move for `player` from `board` using minimax (with optional alpha-beta)
    and the heuristics.evaluate function as the static evaluator at leaf nodes.

    Returns (best_move, best_board_after_move, score). If no move is available returns (None, None, -INF).

    Parameters
    - board: current position
    - player: player to move
    - depth: search depth (ply). depth=0 evaluates current position without searching moves.
    - weights: optional weights passed to heuristics.evaluate
    - use_alpha_beta: whether to use alpha-beta pruning

    Notes:
    - The function will short-circuit and return immediately if an immediate winning move is found.
    - Heuristic evaluation is used at leaf nodes and for positions with no legal moves.
    """
    if weights is None:
        weights = _DEFAULT_WEIGHTS

    opponent = Player.BLACK if player == Player.WHITE else Player.WHITE

    # Quick immediate-win check for the root player
    win_pair = _has_immediate_win(board, player)
    if win_pair is not None:
        move, board_after = win_pair
        return move, board_after, float("inf")

    # Also if opponent already has an immediate win from this position, that's very bad
    opp_win = _has_immediate_win(board, opponent)
    if opp_win is not None:
        return None, None, float("-inf")

    def minimax(node_board: ChessBallBoard, to_move: Player, ply: int,
                alpha: float, beta: float, maximizing: bool) -> Tuple[float, Optional[Dict], Optional[ChessBallBoard]]:
        """
        Returns (score, best_move, best_board) from node_board with to_move to play.
        If best_move is None, no legal move was found.
        """
        # Terminal checks
        # If the side to move has an immediate win, return terminal score
        iw = _has_immediate_win(node_board, to_move)
        if iw is not None:
            # If the player to move can win immediately, it's very good for them.
            score = float("inf") if maximizing else float("-inf")
            # Return the move that wins immediately if maximizing at this node (useful at root)
            move, board_after = iw
            return (score, move, board_after)

        # If opponent to_move has immediate win, this node is losing
        other = Player.BLACK if to_move == Player.WHITE else Player.WHITE
        if _has_immediate_win(node_board, other) is not None:
            score = float("-inf") if maximizing else float("inf")
            return (score, None, None)

        # Depth 0: evaluate statically
        if ply == 0:
            # evaluate from perspective of root player: positive means good for `player`
            val = evaluate(node_board, player, weights=weights)
            return val, None, None

        # Generate moves
        moves = list(possible_moves(node_board, to_move))
        if not moves:
            # No legal moves: evaluate statically (could be stalemate-ish)
            val = evaluate(node_board, player, weights=weights)
            return val, None, None

        best_move = None
        best_board_after = None

        if maximizing:
            value = float("-inf")
            for move, board_after in moves:
                # Recurse with opponent to move
                score, _, _ = minimax(board_after, other, ply - 1, alpha, beta, False)
                if score is None:
                    continue
                if score > value:
                    value = score
                    best_move = move
                    best_board_after = board_after
                if use_alpha_beta:
                    alpha = max(alpha, value)
                    if alpha >= beta:
                        break
            return value, best_move, best_board_after
        else:
            value = float("inf")
            for move, board_after in moves:
                score, _, _ = minimax(board_after, other, ply - 1, alpha, beta, True)
                if score is None:
                    continue
                if score < value:
                    value = score
                    best_move = move
                    best_board_after = board_after
                if use_alpha_beta:
                    beta = min(beta, value)
                    if beta <= alpha:
                        break
            return value, best_move, best_board_after

    # At root we are maximizing for `player`
    score, best_move, best_board_after = minimax(board, player, depth, -INF, INF, True)

    # If best_move is None but there are legal moves, pick the highest-scoring child (fallback)
    if best_move is None:
        moves = list(possible_moves(board, player))
        if moves:
            best_score = float("-inf")
            for move, board_after in moves:
                s = evaluate(board_after, player, weights=weights)
                if s > best_score:
                    best_score = s
                    best_move = move
                    best_board_after = board_after
            score = best_score

    return best_move, best_board_after, float(score if score is not None else float("-inf"))


START_BOARD = '''-- -- BD BD BD --
-- -- BA BA -- --
-- -- -- -- -- --
-- -- -- NB -- --
-- -- -- -- -- --
-- -- WA WA -- --
-- -- WD WD WD --'''
if __name__ == "__main__":
    # Simple usage example (not a full unit test) — construct a tiny board and pick a move.
    import time
    from chessball_board import Piece, PieceType
    b = ChessBallBoard.from_str(START_BOARD)
    player = Player.WHITE
    for _ in range(20):
        move, board_after, sc = choose_best_move(b, player, depth=2)
        print("Chosen move:", move)
        print("Score:", sc)
        if board_after:
            print("Resulting board:\n", board_after)
        time.sleep(.5)
        player = Player.BLACK if player == Player.WHITE else Player.WHITE