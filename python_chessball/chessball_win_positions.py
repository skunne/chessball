from typing import Generator, Tuple
from itertools import combinations
from chessball_board import ChessBallBoard, Piece, PieceType, Player
from win_avoidability import is_win_avoidable_by_opponent

def generate_unavoidable_win_positions(player: Player) -> Generator[ChessBallBoard, None, None]:
    """
    Efficiently yields all positions in which `player` has won with their last move
    and the win was NOT avoidable by the opponent.
    That is: for each position where player has a winning last move,
    only yield those for which `is_win_avoidable_by_opponent(position, player) == False`.
    """
    # Use efficient win generation logic (see previous answers)
    from chessball_win_positions import generate_win_positions

    for position in generate_win_positions(player):
        if not is_win_avoidable_by_opponent(position, player):
            yield position

def generate_win_positions(player: Player) -> Generator[ChessBallBoard, None, None]:
    """
    Efficiently generates all board positions in which `player` has won with their last move:
    - The ball is in the winning row for `player`.
    - The ball is adjacent to one of `player`'s pieces.
    - The adjacency is such that this piece could have pushed the ball into the winning row with their last move (respecting forbidden columns).
    - All other pieces (2A+3D per player) are anywhere on the board, distinct squares.
    """
    ROWS, COLS = ChessBallBoard.ROWS, ChessBallBoard.COLS
    winner_row = 0 if player == Player.BLACK else ROWS - 1

    # Piece lists for both players, attackers then defenders
    total_pieces = (
        [(PieceType.ATTACKER, Player.WHITE)] * 2 +
        [(PieceType.DEFENDER, Player.WHITE)] * 3 +
        [(PieceType.ATTACKER, Player.BLACK)] * 2 +
        [(PieceType.DEFENDER, Player.BLACK)] * 3
    )

    piece_count = len(total_pieces)  # 10
    all_squares = [(r, c) for r in range(ROWS) for c in range(COLS)]

    # All 8 possible directions; piece must be in previous row/col one away from ball
    directions = [
        (-1, 0), (1, 0), (0, -1), (0, 1),
        (-1, -1), (-1, 1), (1, -1), (1, 1)
    ]

    for ball_col in range(COLS):
        ball_pos = (winner_row, ball_col)

        # For each direction, determine where the pusher must be, and where the ball was pushed from
        for dr, dc in directions:
            pusher_r, pusher_c = winner_row - dr, ball_col - dc
            ball_prev_r, ball_prev_c = winner_row - dr, ball_col - dc

            # Check bounds for both pusher and previous ball
            if not (0 <= pusher_r < ROWS and 0 <= pusher_c < COLS): continue
            if not (0 <= ball_prev_r < ROWS and 0 <= ball_prev_c < COLS): continue
            # Cannot push the ball from column 0 or COLS-1
            if ball_prev_c == 0 or ball_prev_c == COLS - 1: continue

            # Ball must have moved from (ball_prev_r, ball_prev_c) to (winner_row, ball_col) by pusher
            occupied_squares = {ball_pos, (pusher_r, pusher_c)}

            # Generate all ways to place the remaining pieces on the free squares
            free_squares = [sq for sq in all_squares if sq not in occupied_squares]
            # Remove the one piece (pusher) from total_pieces
            # Find all ways the pusher could be assigned from the attack/defend slots of given color
            # Since all attackers/defenders are indistinguishable, just skip one piece of player from total_pieces
            # Find all possible types for candidate pusher
            for pusher_type in [PieceType.ATTACKER, PieceType.DEFENDER]:
                pusher_piece = (pusher_type, player)
                # Check there are enough pieces of that type
                player_pieces = [(pt, pc) for pt, pc in total_pieces if pc == player and pt == pusher_type]
                if player_pieces.count(pusher_piece) == 0: continue
                # Remove one occurrence of pusher_piece
                remaining_pieces = total_pieces.copy()
                remaining_pieces.remove(pusher_piece)
                # Now fill the remaining squares with remaining_pieces
                for piece_squares in combinations(free_squares, len(remaining_pieces)):
                    board = ChessBallBoard()
                    # Place ball
                    board.place_piece(ball_pos[0], ball_pos[1], Piece(PieceType.BALL, Player.NEUTRAL))
                    # Place pusher
                    board.place_piece(pusher_r, pusher_c, Piece(pusher_type, player))
                    # Place rest
                    for (square, (ptype, pcolor)) in zip(piece_squares, remaining_pieces):
                        board.place_piece(square[0], square[1], Piece(ptype, pcolor))
                    yield board

if __name__=='__main__':
    n = sum(1 for _ in generate_unavoidable_win_positions(Player.WHITE))
    # n = 0
    # for position in generate_unavoidable_win_positions(Player.WHITE):
    #     n += 1
    #     print(position)
    #     print()
    print(n, "winning White positions that Black couldn't have blocked")
