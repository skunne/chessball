from typing import List, Tuple, Dict
from chessball_board import ChessBallBoard, Player, PieceType, possible_moves

def winning_moves(position: ChessBallBoard, player: Player) -> List[Dict]:
    """
    Returns the list of moves for `player` in `position` such that after the move,
    the ball is in the winning row (row 0 for black, row 5 for white).
    Each move is represented as a dict (as in possible_moves).
    """
    winner_row = 0 if player == Player.BLACK else position.ROWS - 1

    moves_and_boards = possible_moves(position, player)
    win_moves = []
    for move, board in moves_and_boards:
        ball_pos = board.find_ball()
        if ball_pos and ball_pos[0] == winner_row:
            win_moves.append(move)
    return win_moves