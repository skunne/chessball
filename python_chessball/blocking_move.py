from typing import Optional, Dict
from chessball_board import ChessBallBoard, Player, possible_moves
from winning_moves import winning_moves

def find_blocking_move(position: ChessBallBoard, player: Player) -> Optional[Dict]:
    """
    Finds and returns a move (dict) for `player` such that, after this move,
    the opponent does not have any immediate winning moves.
    If no such blocking move exists, returns None.
    """
    opponent = Player.BLACK if player == Player.WHITE else Player.WHITE
    for move, board_after in possible_moves(position, player):
        opponent_win_moves = winning_moves(board_after, opponent)
        if not opponent_win_moves:
            return move
    return None