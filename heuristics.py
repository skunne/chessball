"""
Heuristic feature extraction and evaluation for ChessBall positions.

Provides:
- feature_vector(board, player) -> dict[str, float]
- evaluate(board, player, weights=None) -> float

The features are normalized to roughly comparable ranges so a linear
combination with the default weights gives a reasonable baseline evaluation.

You can tune the weights dict or pass your own to `evaluate`.
"""
from typing import Dict, Optional

from chessball_board import ChessBallBoard, Player, PieceType, possible_moves
from winning_moves import winning_moves
from win_avoidability import is_win_avoidable_by_opponent

## USELESS HEURISTIC IN CHESSBALL: THERE IS NO CAPTURE, NUMBER OF PIECES IS CONSTANT 
# def count_material(board: ChessBallBoard, player: Player):
#     """Return (n_attackers, n_defenders) for the given player."""
#     nA = nD = 0
#     for r in range(board.ROWS):
#         for c in range(board.COLS):
#             p = board.get_piece(r, c)
#             if p and p.player == player:
#                 if p.piece_type == PieceType.ATTACKER:
#                     nA += 1
#                 elif p.piece_type == PieceType.DEFENDER:
#                     nD += 1
#     return nA, nD


def ball_pos(board: ChessBallBoard) -> tuple[int, int]:
    pos = board.find_ball()
    assert pos is not None
    return pos


def is_forbidden_col(col: int, board: ChessBallBoard):
    return col == 0 or col == board.COLS - 1


def count_adjacent_pushers(board: ChessBallBoard, player: Player) -> int:
    """
    Count how many of `player`'s pieces are adjacent to the ball and *can* push it
    (i.e., adjacent to the ball, and the destination cell for the ball after push
    is inside board, empty, and not a forbidden column).
    """
    ball = ball_pos(board)
    if not ball:
        return 0
    br, bc = ball
    dirs = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)]
    count = 0
    for dr, dc in dirs:
        pr, pc = br - dr, bc - dc  # pusher would be here before pushing into ball square
        dest_r, dest_c = br + dr, bc + dc  # ball would be pushed here
        if not (0 <= pr < board.ROWS and 0 <= pc < board.COLS):
            continue
        if not (0 <= dest_r < board.ROWS and 0 <= dest_c < board.COLS):
            continue
        if is_forbidden_col(dest_c, board):
            continue
        pusher = board.get_piece(pr, pc)
        if pusher and pusher.player == player:
            # destination must be empty for a legal push
            if board.get_piece(dest_r, dest_c) is None:
                count += 1
    return count


def count_control_around_ball(board: ChessBallBoard, player: Player) -> tuple[int, int]:
    """
    Return (friendly_control, enemy_control) counts: number of adjacent squares to the ball
    that are occupied by friendly/opponent pieces (simple adjacency control).
    """
    ball = ball_pos(board)
    if not ball:
        return 0, 0
    br, bc = ball
    dirs = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)]
    friendly = 0
    enemy = 0
    for dr, dc in dirs:
        r, c = br + dr, bc + dc
        if not (0 <= r < board.ROWS and 0 <= c < board.COLS):
            continue
        p = board.get_piece(r, c)
        if p is None:
            continue
        if p.player == player:
            friendly += 1
        else:
            enemy += 1
    return friendly, enemy


def mobility(board: ChessBallBoard, player: Player) -> int:
    """
    Number of legal moves for player.
    """
    moves = possible_moves(board, player)
    return len(moves)


def vulnerable_pieces_count(board: ChessBallBoard, player: Player) -> int:
    """
    Count the number of `player`'s pieces that are immediately vulnerable to being tackled
    by an adjacent opponent Defender (i.e., opponent defender adjacent and the square beyond is empty).
    This is a conservative, cheap approximation of 'vulnerable' pieces.
    """
    vuln = 0
    opponent = Player.BLACK if player == Player.WHITE else Player.WHITE
    for r in range(board.ROWS):
        for c in range(board.COLS):
            p = board.get_piece(r, c)
            if p is None or p.player != player:
                continue
            # check adjacent opponent defenders that can push this piece
            dirs = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)]
            for dr, dc in dirs:
                opp_r, opp_c = r + dr, c + dc
                beyond_r, beyond_c = r + 2*dr, c + 2*dc
                if not (0 <= opp_r < board.ROWS and 0 <= opp_c < board.COLS):
                    continue
                if not (0 <= beyond_r < board.ROWS and 0 <= beyond_c < board.COLS):
                    continue
                opp_piece = board.get_piece(opp_r, opp_c)
                beyond = board.get_piece(beyond_r, beyond_c)
                # Opponent must be a defender, pushed piece cannot be ball, and beyond must be empty
                if (opp_piece is not None and opp_piece.player == opponent and
                        opp_piece.piece_type == PieceType.DEFENDER and
                        p.piece_type != PieceType.BALL and beyond is None):
                    vuln += 1
                    break  # count each vulnerable piece only once
    return vuln


def approx_push_distance(board: ChessBallBoard, player: Player) -> float:
    """
    Cheap approximation for number of pushes needed to reach goal row.
    Returns a normalized value in [0,1] where 1 means ball is on goal row (0 pushes),
    0 means farthest from goal (ROWS-1 pushes).
    Approximation: uses straight row distance with a small bonus if there's an immediate pusher
    in the 'forward' direction.
    """
    ball = ball_pos(board)
    if not ball:
        return 0.0
    br, bc = ball
    if player == Player.WHITE:
        dist = (board.ROWS - 1) - br
        forward_dr = 1
    else:
        dist = br - 0
        forward_dr = -1

    max_dist = board.ROWS - 1
    if max_dist == 0:
        return 1.0
    # bonus if a friendly pusher is directly behind the ball in forward direction
    behind_r, behind_c = br - forward_dr, bc
    bonus = 0.0
    if 0 <= behind_r < board.ROWS and 0 <= behind_c < board.COLS:
        p = board.get_piece(behind_r, behind_c)
        if p and p.player == player:
            # But ensure the destination for the ball would be legal if pushed that way
            dest_r, dest_c = br + forward_dr, bc
            if (0 <= dest_r < board.ROWS and 0 <= dest_c < board.COLS and
                    board.get_piece(dest_r, dest_c) is None and not is_forbidden_col(dest_c, board)):
                bonus = 0.5  # reduce effective distance a little
    effective_dist = max(0.0, dist - bonus)
    # Normalize: smaller distance -> higher normalized value
    norm = 1.0 - (effective_dist / max_dist)
    if norm < 0.0:
        norm = 0.0
    return norm

def ball_row_for_player(board: ChessBallBoard, player: Player) -> int:
    """
    Return a player-oriented row score derived from the ball's row:

    - For Player.WHITE: returns the raw row index (0..ROWS-1). Larger is better.
    - For Player.BLACK: returns (ROWS-1 - raw_row) so that larger is better for Black too.
    - For Player.NEUTRAL: returns the raw row.

    If there's no ball, returns -1.
    """
    pos = board.find_ball()
    assert pos is not None
    row, _ = pos
    if player == Player.WHITE:
        return row
    if player == Player.BLACK:
        return board.ROWS - 1 - row
    return row

def count_opponent_pieces_between_ball_and_goal(board: ChessBallBoard, player: Player) -> int:
    """
    Count the number of opponent pieces that lie in rows strictly between the current
    ball row and the player's goal row.

    - For Player.WHITE the goal row is ROWS-1, so we count opponent pieces in rows (ball_row+1 .. ROWS-2).
    - For Player.BLACK the goal row is 0, so we count opponent pieces in rows (1 .. ball_row-1).
    - For Player.NEUTRAL returns 0.

    Returns an integer count (0 if no ball or no opponent pieces in-between).
    """
    pos = board.find_ball()
    if pos is None:
        return 0

    if player == Player.NEUTRAL:
        return 0

    ball_row = pos[0]
    goal_row = board.ROWS - 1 if player == Player.WHITE else 0

    # Determine exclusive range of rows between ball and goal
    start = min(ball_row, goal_row)
    end = max(ball_row, goal_row)

    # If there are no rows strictly between, return 0
    if end - start <= 1:
        return 0

    count = 0
    for r in range(start + 1, end):
        for c in range(board.COLS):
            piece = board.get_piece(r, c)
            if piece is None:
                continue
            # Count only opponent pieces (exclude the ball)
            if piece.player != player and piece.piece_type != PieceType.BALL:
                count += 1
    return count

def feature_vector(board: ChessBallBoard, player: Player) -> Dict[str, float]:
    """
    Computes a dictionary of heuristic features for `player` on `board`.

    Features returned (values roughly in [-1,1] or [0,1] unless described):
      - win_now: 1 if player has an immediate winning move, else 0
      - lose_now: 1 if opponent has an immediate winning move, else 0
      - ball_row: closeness of ball to player's goal row in [0,1]
      - ball_in_forbidden_col: 1 if ball is in col 0 or COLS-1 else 0
      - adj_pushers: fraction of adjacent squares that are friendly push-capable (0..1)
      - control: (friendly_adjacent - enemy_adjacent)/8 in [-1,1]
      - material: weighted material diff normalized to [-1,1]
      - mobility: normalized mobility diff (approx)
      - protection: same as control (alias)
      - push_distance: approx closeness to goal in [0,1]
      - unavoidable_win: 1 if player has a winning move that was unavoidable for opponent, else 0
      - vulnerable: fraction of player's pieces that are immediately vulnerable (0..1) (negative in eval)
      - ball_row: position of the ball along direction of the goal [0,1]
      - opp_between_ball_and_goal: fraction of opponent pieces in rows behing the ball [0,1]
    """
    opponent = Player.BLACK if player == Player.WHITE else Player.WHITE

    # Immediate win/lose
    player_wins = bool(next(iter(winning_moves(board, player)), False))
    opp_wins = bool(next(iter(winning_moves(board, opponent)), False))

    # Ball proximity and forbidden column
    ball = ball_pos(board)
    if ball:
        br, bc = ball
        if player == Player.WHITE:
            dist_rows = (board.ROWS - 1) - br
        else:
            dist_rows = br - 0
        ball_row_feature = 1.0 - (dist_rows / (board.ROWS - 1)) if (board.ROWS - 1) > 0 else 1.0
        ball_in_forbidden = 1.0 if is_forbidden_col(bc, board) else 0.0
    else:
        ball_row_feature = 0.0
        ball_in_forbidden = 0.0

    # Adjacent pushers and control
    adj_pushers = count_adjacent_pushers(board, player)
    opp_adj_pushers = count_adjacent_pushers(board, opponent)
    adj_pushers_norm = adj_pushers / 8.0
    opp_adj_pushers_norm = opp_adj_pushers / 8.0
    control_friendly, control_enemy = count_control_around_ball(board, player)
    control = (control_friendly - control_enemy) / 8.0  # in [-1,1]

    # # Material
    # nA_p, nD_p = count_material(board, player)
    # nA_o, nD_o = count_material(board, opponent)
    # # Normalize by maximum possible pieces per side (5)
    max_pieces = 5.0
    # wA = 1.2
    # wD = 1.0
    # mat_diff = (wA * (nA_p - nA_o) + wD * (nD_p - nD_o)) / ( (wA + wD) * max_pieces )

    # Mobility (approx): number of moves difference normalized by an arbitrary cap
    mob_p = mobility(board, player)
    mob_o = mobility(board, opponent)
    mob_cap = 60.0  # heuristic cap to keep value small
    mob = (mob_p - mob_o) / mob_cap

    # Vulnerable pieces fraction
    vulnerable = vulnerable_pieces_count(board, player) / max_pieces

    # Approx push distance normalized
    push_dist = approx_push_distance(board, player)

    # Unavoidable win: 1 if player has a winning move and that win was NOT avoidable by opponent
    unavoidable = 0.0
    if player_wins:
        try:
            avoidable = is_win_avoidable_by_opponent(board, player)
            # is_win_avoidable_by_opponent returns True if opponent *could* always block the win.
            # So unavoidable = 1 when player has winning move AND opponent could NOT always block it.
            unavoidable = 1.0 if not avoidable else 0.0
        except Exception:
            # If the function is expensive or missing, treat as 0
            unavoidable = 0.0

    # ball row
    ball_row_value = ball_row_for_player(board, player) / (board.ROWS - 1)

    # opponent pieces between ball and goal
    opp_pieces_between_ball_and_goal = count_opponent_pieces_between_ball_and_goal(board, player) / 5

    features = {
        "win_now": 1.0 if player_wins else 0.0,
        "lose_now": 1.0 if opp_wins else 0.0,
        "ball_row": float(ball_row_feature),
        "ball_in_forbidden_col": float(ball_in_forbidden),
        "adj_pushers": float(adj_pushers_norm),
        "opp_adj_pushers": float(opp_adj_pushers_norm),
        "control": float(control),
        #"material": float(mat_diff),
        "mobility": float(mob),
        "push_distance": float(push_dist),
        "unavoidable_win": float(unavoidable),
        "vulnerable": float(vulnerable),
        "ball_row": ball_row_value,
        "opp_between_ball_and_goal": opp_pieces_between_ball_and_goal,
    }

    return features


# Default weights for linear evaluation. These are starting points — tune as needed.
_DEFAULT_WEIGHTS = {
    "win_now": 1e6,
    "lose_now": -1e6,
    "ball_row": 10.0,
    "ball_in_forbidden_col": -5.0,
    "adj_pushers": 30.0,
    "opp_adj_pushers": -30.0,
    "control": 20.0,
    #"material": 8.0,
    "mobility": 1.0,
    "push_distance": 20.0,
    "unavoidable_win": 200.0,
    "vulnerable": -15.0,
    "ball_row": 12.0,
    "opp_between_ball_and_goal": 8.0,
    # bias can be used for constant offsets
    "bias": 0.0,
}


def evaluate(board: ChessBallBoard, player: Player, weights: Optional[Dict[str, float]] = None) -> float:
    """
    Linear evaluation function combining features with weights.

    - board: ChessBallBoard
    - player: Player for whom to evaluate (positive = good for player)
    - weights: optional dict overriding defaults; any missing feature uses default weight.

    Returns a float score (higher better for `player`).
    """
    feats = feature_vector(board, player)
    if weights is None:
        weights = _DEFAULT_WEIGHTS

    score = weights.get("bias", 0.0)
    for k, v in feats.items():
        w = weights.get(k, _DEFAULT_WEIGHTS.get(k, 0.0))
        score += w * v

    return score