from chessball_board import ChessBallBoard, Player, possible_previous_moves
from blocking_move import find_blocking_move

def is_win_avoidable_by_opponent(position: ChessBallBoard, player: Player) -> bool:
    """
    Given a position in which `player` has a winning move,
    checks whether in ALL possible previous positions (and moves for the opponent)
    there exists a blocking move for the opponent that prevents the win.
    In other words: returns True if the opponent could have always blocked this win,
    and False if some previous position made the win inevitable.
    """
    opponent = Player.BLACK if player == Player.WHITE else Player.WHITE
    previous_positions = possible_previous_moves(position, opponent)
    if not previous_positions:  # No prior position exists
        return False
    for move, prev_board in previous_positions:
        blocking = find_blocking_move(prev_board, opponent)
        if blocking is None:
            # At least one prior position did NOT allow a blocking move: win is inevitable
            return False
    # All previous positions allowed a blocking move for the opponent
    return True