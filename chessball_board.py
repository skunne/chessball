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
    ROWS = 7
    COLS = 6

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
    
    @staticmethod
    def from_str(s: str) -> "ChessBallBoard":
        """
        Construct a ChessBallBoard from the string format produced by __repr__.

        Expected input format: ROWS lines, each with COLS tokens separated by whitespace.
        Each token is either "--" for an empty cell or two uppercase letters:
            <PlayerInitial><PieceTypeInitial>
        where PlayerInitial is one of:
            W = white, B = black, N = neutral
        and PieceTypeInitial is one of:
            A = attacker, D = defender, B = ball

        Example token: "WA" = White Attacker, "NB" = Neutral Ball, "--" = empty cell.

        Raises ValueError on malformed input.
        """
        lines = [line for line in s.strip().splitlines() if line.strip() != ""]
        if len(lines) != ChessBallBoard.ROWS:
            raise ValueError(f"Expected {ChessBallBoard.ROWS} rows, got {len(lines)}")

        # Helper maps
        player_map = {
            "W": Player.WHITE,
            "B": Player.BLACK,
            "N": Player.NEUTRAL
        }
        piece_map = {
            "A": PieceType.ATTACKER,
            "D": PieceType.DEFENDER,
            "B": PieceType.BALL
        }

        board = ChessBallBoard()
        for r, line in enumerate(lines):
            tokens = line.split()
            if len(tokens) != ChessBallBoard.COLS:
                raise ValueError(f"Expected {ChessBallBoard.COLS} columns on row {r}, got {len(tokens)}")
            for c, tok in enumerate(tokens):
                if tok == "--":
                    continue
                if len(tok) != 2:
                    raise ValueError(f"Invalid token '{tok}' at row {r}, col {c}")
                p_char, t_char = tok[0], tok[1]
                if p_char not in player_map:
                    raise ValueError(f"Unknown player initial '{p_char}' in token '{tok}' at {r},{c}")
                if t_char not in piece_map:
                    raise ValueError(f"Unknown piece initial '{t_char}' in token '{tok}' at {r},{c}")
                player = player_map[p_char]
                ptype = piece_map[t_char]
                board.place_piece(r, c, Piece(ptype, player))
        return board

# Directions: 8 adjacent directions (orthogonal + diagonal)
DIRECTIONS = [
    (-1, 0), (1, 0), (0, -1), (0, 1),
    (-1, -1), (-1, 1), (1, -1), (1, 1)
]

def possible_moves(board: ChessBallBoard, player: Player):
    moves_and_results = []
    for r in range(board.ROWS):
        for c in range(board.COLS):
            piece = board.get_piece(r, c)
            if piece and piece.player == player:
                for dr, dc in DIRECTIONS:
                    nr, nc = r + dr, c + dc
                    # Normal adjacent move
                    if 0 <= nr < board.ROWS and 0 <= nc < board.COLS:
                        target = board.get_piece(nr, nc)
                        if target is None:
                            # Normal move
                            new_board = deepcopy(board)
                            new_board.place_piece(nr, nc, piece)
                            new_board.remove_piece(r, c)
                            moves_and_results.append((
                                {'from': (r, c), 'to': (nr, nc), 'push_ball': False, 'jump': False},
                                new_board
                            ))
                        elif target and target.piece_type == PieceType.BALL:
                            # Ball push logic (with forbidden columns)
                            br, bc = nr, nc
                            br2, bc2 = br + dr, bc + dc
                            if (0 <= br2 < board.ROWS
                                and 0 <= bc2 < board.COLS
                                and board.get_piece(br2, bc2) is None
                                and bc2 != 0 and bc2 != board.COLS - 1):
                                new_board = deepcopy(board)
                                new_board.remove_piece(r, c)
                                new_board.place_piece(br, bc, piece)
                                new_board.place_piece(br2, bc2, Piece(PieceType.BALL, Player.NEUTRAL))
                                moves_and_results.append((
                                    {'from': (r, c), 'to': (br, bc), 'push_ball': True, 'ball_to': (br2, bc2), 'jump': False},
                                    new_board
                                ))
                    # Attacker jump move
                    if piece.piece_type == PieceType.ATTACKER:
                        adj_r, adj_c = r + dr, c + dc
                        jump_r, jump_c = r + 2*dr, c + 2*dc
                        # Check bounds for jump
                        if (0 <= adj_r < board.ROWS and 0 <= adj_c < board.COLS and
                            0 <= jump_r < board.ROWS and 0 <= jump_c < board.COLS):
                            adj_piece = board.get_piece(adj_r, adj_c)
                            jump_target = board.get_piece(jump_r, jump_c)
                            if (adj_piece is not None and adj_piece.piece_type != PieceType.BALL and
                                jump_target is None):
                                new_board = deepcopy(board)
                                new_board.place_piece(jump_r, jump_c, piece)
                                new_board.remove_piece(r, c)
                                moves_and_results.append((
                                    {'from': (r, c), 'to': (jump_r, jump_c), 'jump': True, 'jumped_over': (adj_r, adj_c), 'push_ball': False},
                                    new_board
                                ))
                    # Defender tackle move
                    elif piece.piece_type == PieceType.DEFENDER:
                        # Tackle is allowed against adjacent opponent (other than ball)
                        beyond_r, beyond_c = nr + dr, nc + dc
                        if ((0 <= beyond_r < board.ROWS and 0 <= beyond_c < board.COLS
                                and target is not None)
                            and target.player != player
                            and target.piece_type != PieceType.BALL
                            and board.get_piece(beyond_r, beyond_c) is None):
                            new_board = deepcopy(board)
                            # Defender moves into opponent's square
                            new_board.place_piece(nr, nc, piece)
                            new_board.remove_piece(r, c)
                            # Opponent is pushed to free square
                            new_board.place_piece(beyond_r, beyond_c, target)
                            new_board.remove_piece(nr, nc)
                            moves_and_results.append((
                                {'from': (r, c), 'to': (nr, nc), 'tackle': True,
                                 'pushed_piece_from': (nr, nc), 'pushed_piece_to': (beyond_r, beyond_c)},
                                new_board
                            ))
    return moves_and_results

def possible_previous_moves(board: ChessBallBoard, player: Player):
    """
    For a given board and player, generate all possible moves and boards that could have led to the current position.
    Includes Attacker jump moves: if a player Attacker is on (r,c) and there is an adjacent piece (not the ball)
    at (adj_r,adj_c) and the square at (prev_r,prev_c) is empty, then the attacker could have jumped from there.
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
                                {'from': (pr, pc), 'to': (r, c), 'push_ball': False, 'jump': False},
                                prev_board
                            ))
                    # 2. Ball-push move: did the ball just get pushed to (r, c)?
                    ball_pos = board.find_ball()
                    br, bc = r, c  # Ball's current pos
                    br_prev, bc_prev = r - dr, c - dc
                    pr, pc = r - dr, c - dc  # Piece was at pr,pc, moved to r,c, pushed ball
                    ball_dest_r, ball_dest_c = r, c
                    ball_src_r, ball_src_c = br_prev, bc_prev
                    if (
                        0 <= ball_src_r < board.ROWS and
                        0 <= ball_src_c < board.COLS and
                        0 <= pr < board.ROWS and
                        0 <= pc < board.COLS and
                        ball_src_c != 0 and ball_src_c != board.COLS - 1
                    ):
                        dest_piece = board.get_piece(ball_dest_r, ball_dest_c)
                        if (
                            dest_piece and dest_piece.piece_type == PieceType.BALL and
                            board.get_piece(pr, pc) is None and
                            (
                                (source_piece := board.get_piece(ball_src_r, ball_src_c)) is None or
                                source_piece.piece_type != PieceType.BALL
                            )
                        ):
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
                                    'ball_to': (ball_dest_r, ball_dest_c),
                                    'jump': False
                                },
                                prev_board
                            ))
                # Attacker jump (reverse): piece at (r, c) may have arrived via jump from (prev_r, prev_c)
                if piece.piece_type == PieceType.ATTACKER:
                    for dr, dc in DIRECTIONS:
                        adj_r, adj_c = r - dr, c - dc
                        prev_r, prev_c = r - 2*dr, c - 2*dc
                        if (0 <= adj_r < board.ROWS and 0 <= adj_c < board.COLS and
                            0 <= prev_r < board.ROWS and 0 <= prev_c < board.COLS):
                            adj_piece = board.get_piece(adj_r, adj_c)
                            prev_square = board.get_piece(prev_r, prev_c)
                            # Attacker could have jumped from prev_r, prev_c, jumped over a non-ball piece at adj_r, adj_c (any player)
                            if (adj_piece is not None and adj_piece.piece_type != PieceType.BALL and
                                prev_square is None):
                                prev_board = deepcopy(board)
                                prev_board.remove_piece(r, c)
                                prev_board.place_piece(prev_r, prev_c, piece)
                                prev_moves_and_positions.append((
                                    {'from': (prev_r, prev_c), 'to': (r, c), 'jump': True, 'jumped_over': (adj_r, adj_c), 'push_ball': False},
                                    prev_board
                                ))
                # Defender tackle (retrograde): attacker at (r, c) may have just arrived by tackling
                elif piece.piece_type == PieceType.DEFENDER:
                    for dr, dc in DIRECTIONS:
                        opp_r, opp_c = r - dr, c - dc
                        pushed_r, pushed_c = r + dr, c + dc
                        if (0 <= opp_r < board.ROWS and 0 <= opp_c < board.COLS and
                            0 <= pushed_r < board.ROWS and 0 <= pushed_c < board.COLS):
                            opp_piece = board.get_piece(pushed_r, pushed_c)
                            defender_prev_square = board.get_piece(opp_r, opp_c)
                            if (opp_piece is not None
                                and opp_piece.player != player
                                and opp_piece.piece_type != PieceType.BALL
                                and defender_prev_square is None):
                                prev_board = deepcopy(board)
                                # Move defender back
                                prev_board.remove_piece(r, c)
                                prev_board.place_piece(opp_r, opp_c, piece)
                                # Move opponent piece back
                                prev_board.remove_piece(pushed_r, pushed_c)
                                prev_board.place_piece(r, c, opp_piece)
                                prev_moves_and_positions.append((
                                    {'from': (opp_r, opp_c), 'to': (r, c), 'tackle': True,
                                     'pushed_piece_from': (r, c), 'pushed_piece_to': (pushed_r, pushed_c)},
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