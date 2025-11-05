from typing import List
from chessball_board import ChessBallBoard, Piece, PieceType, Player

def generate_white_win_positions() -> List[ChessBallBoard]:
    """
    Generates all board positions in which White wins (ball is in row 5),
    with 2 White Attackers, 3 White Defenders, 2 Black Attackers, 3 Black Defenders, 1 Ball on row 5,
    each in an empty square, no overlapping pieces.
    Returns a list of ChessBallBoard objects.
    """
    # Board size
    ROWS, COLS = ChessBallBoard.ROWS, ChessBallBoard.COLS
    ball_row = ROWS - 1  # row 5; top row is 0

    from itertools import combinations, product

    total_pieces = [
        (PieceType.ATTACKER, Player.WHITE)]*2 + \
        [(PieceType.DEFENDER, Player.WHITE)]*3 + \
        [(PieceType.ATTACKER, Player.BLACK)]*2 + \
        [(PieceType.DEFENDER, Player.BLACK)]*3

    piece_count = len(total_pieces)  # 10

    positions = []
    # All possible locations for 10 non-overlapping pieces not in (ball_row, ball_col)
    all_squares = [(r, c) for r in range(ROWS) for c in range(COLS)]
    for ball_col in range(COLS):
        ball_square = (ball_row, ball_col)
        empties = all_squares.copy()
        empties.remove(ball_square)
        for piece_squares in combinations(empties, piece_count):
            # Assign pieces to squares in every possible ordering
            for permuted_pieces in set(product(*[[piece] for piece in total_pieces])):
                # product(*[[piece] for piece in total_pieces]) is equivalent to a single permutation, 
                # so we can use only one arrangement (since all attackers/defenders of a color are indistinguishable)
                board = ChessBallBoard()
                board.place_piece(ball_row, ball_col, Piece(PieceType.BALL, Player.NEUTRAL))
                for (square, (ptype, pcolor)) in zip(piece_squares, permuted_pieces):
                    board.place_piece(square[0], square[1], Piece(ptype, pcolor))
                positions.append(board)
                break  # Only one permutation needed (pieces indistinguishable)
    return positions