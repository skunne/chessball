from enum import Enum
from typing import List, Optional, Tuple
from copy import deepcopy

class Player(Enum):
    WHITE = "white"
    BLACK = "black"
    NEUTRAL = "neutral"

class PieceType(Enum):
    ATTACKER = "attacker"
    DEFENDER = "defender"
    BALL = "ball"

class Piece:
    def __init__(self, piece_type: PieceType, player: Player):
        self.piece_type = piece_type
        self.player = player

    def __repr__(self):
        return f"{self.player.value[0].upper()}{self.piece_type.value[0].upper()}"

class ChessBallBoard:
    ROWS = 6
    COLS = 7

    def __init__(self):
        # Initialize an empty board: each cell is either None or a Piece
        self.board: List[List[Optional[Piece]]] = [
            [None for _ in range(self.COLS)] for _ in range(self.ROWS)
        ]

    def place_piece(self, row: int, col: int, piece: Piece):
        if not (0 <= row < self.ROWS and 0 <= col < self.COLS):
            raise ValueError("Invalid board coordinates.")
        self.board[row][col] = piece

    def remove_piece(self, row: int, col: int):
        if not (0 <= row < self.ROWS and 0 <= col < self.COLS):
            raise ValueError("Invalid board coordinates.")
        self.board[row][col] = None

    def get_piece(self, row: int, col: int) -> Optional[Piece]:
        if not (0 <= row < self.ROWS and 0 <= col < self.COLS):
            raise ValueError("Invalid board coordinates.")
        return self.board[row][col]

    def find_ball(self) -> Optional[Tuple[int, int]]:
        for r in range(self.ROWS):
            for c in range(self.COLS):
                piece = self.board[r][c]
                if piece and piece.piece_type == PieceType.BALL:
                    return (r, c)
        return None

    def __repr__(self):
        def cell_repr(cell):
            return str(cell) if cell else "--"
        board_str = ""
        for row in self.board:
            board_str += " ".join(cell_repr(cell) for cell in row) + "\n"
        return board_str

# Directions: 8 adjacent directions (orthogonal + diagonal)
DIRECTIONS = [
    (-1, 0), (1, 0), (0, -1), (0, 1),
    (-1, -1), (-1, 1), (1, -1), (1, 1)
]

def possible_moves(board: ChessBallBoard, player: Player):
    """Generate all possible moves (with resulting boards) for the player.
    When pushing the ball, forbid pushing onto first or last column.
    """
    moves_and_results = []
    for r in range(board.ROWS):
        for c in range(board.COLS):
            piece = board.get_piece(r, c)
            if piece and piece.player == player:
                for dr, dc in DIRECTIONS:
                    nr, nc = r + dr, c + dc
                    if 0 <= nr < board.ROWS and 0 <= nc < board.COLS:
                        target = board.get_piece(nr, nc)
                        # Free adjacent square
                        if target is None:
                            new_board = deepcopy(board)
                            new_board.place_piece(nr, nc, piece)
                            new_board.remove_piece(r, c)
                            moves_and_results.append((
                                {'from': (r, c), 'to': (nr, nc), 'push_ball': False},
                                new_board
                            ))
                        # Ball-push move
                        elif target and target.piece_type == PieceType.BALL:
                            br, bc = nr, nc
                            br2, bc2 = br + dr, bc + dc
                            # Check if new ball position is on board and empty
                            if (
                                0 <= br2 < board.ROWS and
                                0 <= bc2 < board.COLS and
                                board.get_piece(br2, bc2) is None and
                                bc2 != 0 and bc2 != board.COLS - 1  # forbid first/last col
                            ):
                                new_board = deepcopy(board)
                                # Move piece into ball's square
                                new_board.remove_piece(r, c)
                                new_board.place_piece(br, bc, piece)
                                # Move ball
                                new_board.place_piece(br2, bc2, Piece(PieceType.BALL, Player.NEUTRAL))
                                moves_and_results.append((
                                    {'from': (r, c), 'to': (br, bc), 'push_ball': True, 'ball_to': (br2, bc2)},
                                    new_board
                                ))
    return moves_and_results

def possible_previous_moves(board: ChessBallBoard, player: Player):
    """
    For a given board and player, generate all possible moves and boards that could have led to the current position.
    When pushing the ball, forbid that the previous ball position was in the first or last column.
    """
    prev_moves_and_positions = []
    for r in range(board.ROWS):
        for c in range(board.COLS):
            piece = board.get_piece(r, c)
            if piece and piece.player == player:
                for dr, dc in DIRECTIONS:
                    pr, pc = r - dr, c - dc
                    # 1. Simple move: could this piece have just arrived here from pr,pc?
                    if 0 <= pr < board.ROWS and 0 <= pc < board.COLS:
                        if board.get_piece(pr, pc) is None:
                            prev_board = deepcopy(board)
                            prev_board.remove_piece(r, c)
                            prev_board.place_piece(pr, pc, piece)
                            prev_moves_and_positions.append((
                                {'from': (pr, pc), 'to': (r, c), 'push_ball': False},
                                prev_board
                            ))
                    # 2. Ball-push move: did the ball just get pushed to (r, c)?
                    ball_pos = board.find_ball()
                    br, bc = r, c  # Ball's current pos
                    br_prev, bc_prev = r - dr, c - dc
                    pr, pc = r - dr, c - dc  # Piece was at pr,pc, moved to r,c, pushed ball
                    ball_dest_r, ball_dest_c = r, c
                    ball_src_r, ball_src_c = br_prev, bc_prev
                    # Add constraint: previous ball position can't be col 0 or col COLS-1
                    if (
                        0 <= ball_src_r < board.ROWS and
                        0 <= ball_src_c < board.COLS and
                        0 <= pr < board.ROWS and
                        0 <= pc < board.COLS and
                        ball_src_c != 0 and ball_src_c != board.COLS - 1  # Forbidden columns!
                    ):
                        if (
                            board.get_piece(ball_dest_r, ball_dest_c) and
                            board.get_piece(ball_dest_r, ball_dest_c).piece_type == PieceType.BALL and
                            board.get_piece(pr, pc) is None and
                            (
                                board.get_piece(ball_src_r, ball_src_c) is None or
                                board.get_piece(ball_src_r, ball_src_c).piece_type != PieceType.BALL
                            )
                        ):
                            # Reconstruct previous board.
                            prev_board = deepcopy(board)
                            prev_board.remove_piece(r, c)
                            prev_board.place_piece(pr, pc, piece)
                            prev_board.remove_piece(ball_dest_r, ball_dest_c)
                            prev_board.place_piece(ball_src_r, ball_src_c, Piece(PieceType.BALL, Player.NEUTRAL))
                            prev_moves_and_positions.append((
                                {
                                    'from': (pr, pc),
                                    'to': (r, c),
                                    'push_ball': True,
                                    'ball_from': (ball_src_r, ball_src_c),
                                    'ball_to': (ball_dest_r, ball_dest_c)
                                },
                                prev_board
                            ))
    return prev_moves_and_positions

# ---------------- TESTS ----------------

def test_moves_and_prev_moves():
    # Set up board:
    # White Defender at (2,2), Ball at (3,3), free surroundings
    board = ChessBallBoard()
    board.place_piece(2, 2, Piece(PieceType.DEFENDER, Player.WHITE))
    board.place_piece(3, 3, Piece(PieceType.BALL, Player.NEUTRAL))
    print("Initial Board:")
    print(board)

    # Possible moves for white:
    possible = possible_moves(board, Player.WHITE)
    print("\nPossible moves for WHITE:")
    for info, nextb in possible:
        print("Move:", info)
        print(nextb)

    # Apply a ball push (if exists)
    move_with_ball_push = None
    for info, nextb in possible:
        if info['push_ball']:
            move_with_ball_push = (info, nextb)
            break

    if move_with_ball_push:
        print("\nTesting previous moves for resulting board (after push):")
        _, pushed_board = move_with_ball_push

        prev_moves = possible_previous_moves(pushed_board, Player.WHITE)
        for info, prevb in prev_moves:
            print("Previous Move:", info)
            print("Previous Board:")
            print(prevb)

def test_diagonal_and_edge_cases():
    # Board with piece at edge (0,0), Ball at (1,1)
    board = ChessBallBoard()
    board.place_piece(0, 0, Piece(PieceType.ATTACKER, Player.BLACK))
    board.place_piece(1, 1, Piece(PieceType.BALL, Player.NEUTRAL))
    print("Edge Board:")
    print(board)
    possible = possible_moves(board, Player.BLACK)
    print("\nPossible moves for BLACK at edge:")
    for info, nextb in possible:
        print("Move:", info)
        print(nextb)

if __name__ == "__main__":
    print("TEST 1: Moves and Previous Moves\n")
    test_moves_and_prev_moves()
    print("\nTEST 2: Diagonal Moves and Edge Cases\n")
    test_diagonal_and_edge_cases()