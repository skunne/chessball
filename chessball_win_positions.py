from typing import Generator
from itertools import combinations
from chessball_board import ChessBallBoard, Piece, PieceType, Player

def generate_white_win_positions() -> Generator[ChessBallBoard, None, None]:
    """
    Yields all board positions in which White wins (ball is in row 5),
    with 2 White Attackers, 3 White Defenders, 2 Black Attackers, 3 Black Defenders, 1 Ball on row 5,
    each in an empty square, no overlapping pieces.
    """
    ROWS, COLS = ChessBallBoard.ROWS, ChessBallBoard.COLS
    ball_row = ROWS - 1  # row 5

    total_pieces = (
        [(PieceType.ATTACKER, Player.WHITE)] * 2 +
        [(PieceType.DEFENDER, Player.WHITE)] * 3 +
        [(PieceType.ATTACKER, Player.BLACK)] * 2 +
        [(PieceType.DEFENDER, Player.BLACK)] * 3
    )
    piece_count = len(total_pieces)
    all_squares = [(r, c) for r in range(ROWS) for c in range(COLS)]
    for ball_col in range(COLS):
        ball_square = (ball_row, ball_col)
        empties = all_squares.copy()
        empties.remove(ball_square)
        # Combinations of piece_count squares for pieces
        for piece_squares in combinations(empties, piece_count):
            board = ChessBallBoard()
            board.place_piece(ball_row, ball_col, Piece(PieceType.BALL, Player.NEUTRAL))
            # Place pieces; attackers/defenders indistinguishable
            for (square, (ptype, pcolor)) in zip(piece_squares, total_pieces):
                board.place_piece(square[0], square[1], Piece(ptype, pcolor))
            yield board

def generate_black_win_positions() -> Generator[ChessBallBoard, None, None]:
    """
    Yields all board positions in which Black wins (ball is in row 0),
    with 2 White Attackers, 3 White Defenders, 2 Black Attackers, 3 Black Defenders, 1 Ball on row 0,
    each in an empty square, no overlapping pieces.
    """
    ROWS, COLS = ChessBallBoard.ROWS, ChessBallBoard.COLS
    ball_row = 0

    total_pieces = (
        [(PieceType.ATTACKER, Player.WHITE)] * 2 +
        [(PieceType.DEFENDER, Player.WHITE)] * 3 +
        [(PieceType.ATTACKER, Player.BLACK)] * 2 +
        [(PieceType.DEFENDER, Player.BLACK)] * 3
    )
    piece_count = len(total_pieces)
    all_squares = [(r, c) for r in range(ROWS) for c in range(COLS)]
    for ball_col in range(COLS):
        ball_square = (ball_row, ball_col)
        empties = all_squares.copy()
        empties.remove(ball_square)
        for piece_squares in combinations(empties, piece_count):
            board = ChessBallBoard()
            board.place_piece(ball_row, ball_col, Piece(PieceType.BALL, Player.NEUTRAL))
            for (square, (ptype, pcolor)) in zip(piece_squares, total_pieces):
                board.place_piece(square[0], square[1], Piece(ptype, pcolor))
            yield board

def generate_win_positions(player: Player) -> Generator[ChessBallBoard, None, None]:
    """
    Yields all board positions where `player` has won.
    """
    if player == Player.WHITE:
        yield from generate_white_win_positions()
    elif player == Player.BLACK:
        yield from generate_black_win_positions()
    else:
        return