import unittest
from chessball_board import ChessBallBoard, Piece, PieceType, Player

def boards_equal(b1: ChessBallBoard, b2: ChessBallBoard) -> bool:
    for r in range(ChessBallBoard.ROWS):
        for c in range(ChessBallBoard.COLS):
            p1 = b1.get_piece(r, c)
            p2 = b2.get_piece(r, c)
            if p1 is None and p2 is None:
                continue
            if (p1 is None) != (p2 is None):
                return False
            if p1.player != p2.player or p1.piece_type != p2.piece_type:
                return False
    return True

class TestReprFromStr(unittest.TestCase):
    def expected_board_str(self, token_grid):
        """
        token_grid: list of ROWS lists, each a list of COLS tokens like "--" or "WA"
        returns the exact string produced by __repr__: each row joined by single spaces, newline at end of each row.
        """
        lines = [" ".join(row) for row in token_grid]
        return "\n".join(lines) + "\n"

    def test_repr_empty_board(self):
        board = ChessBallBoard()
        empty_row = " ".join(["--"] * ChessBallBoard.COLS)
        expected = "\n".join([empty_row] * ChessBallBoard.ROWS) + "\n"
        self.assertEqual(repr(board), expected)

    def test_repr_and_from_str_single_piece(self):
        board = ChessBallBoard()
        board.place_piece(2, 3, Piece(PieceType.DEFENDER, Player.WHITE))  # WD
        # Build expected string
        token_grid = [["--"] * ChessBallBoard.COLS for _ in range(ChessBallBoard.ROWS)]
        token_grid[2][3] = "WD"
        expected = self.expected_board_str(token_grid)
        self.assertEqual(repr(board), expected)

        # Parse back
        parsed = ChessBallBoard.from_str(expected)
        self.assertTrue(boards_equal(board, parsed))
        p = parsed.get_piece(2, 3)
        self.assertIsNotNone(p)
        self.assertEqual(p.player, Player.WHITE)
        self.assertEqual(p.piece_type, PieceType.DEFENDER)

    def test_repr_and_from_str_mixed_pieces(self):
        board = ChessBallBoard()
        board.place_piece(0, 0, Piece(PieceType.ATTACKER, Player.WHITE))  # WA
        board.place_piece(3, 3, Piece(PieceType.BALL, Player.NEUTRAL))     # NB
        board.place_piece(5, 6, Piece(PieceType.DEFENDER, Player.BLACK))  # BD

        token_grid = [["--"] * ChessBallBoard.COLS for _ in range(ChessBallBoard.ROWS)]
        token_grid[0][0] = "WA"
        token_grid[3][3] = "NB"
        token_grid[5][6] = "BD"
        expected = self.expected_board_str(token_grid)
        self.assertEqual(repr(board), expected)

        parsed = ChessBallBoard.from_str(expected)
        self.assertTrue(boards_equal(board, parsed))
        # Check specific pieces
        p00 = parsed.get_piece(0, 0)
        self.assertIsNotNone(p00)
        self.assertEqual(p00.player, Player.WHITE)
        self.assertEqual(p00.piece_type, PieceType.ATTACKER)
        p33 = parsed.get_piece(3, 3)
        self.assertIsNotNone(p33)
        self.assertEqual(p33.player, Player.NEUTRAL)
        self.assertEqual(p33.piece_type, PieceType.BALL)
        p56 = parsed.get_piece(5, 6)
        self.assertIsNotNone(p56)
        self.assertEqual(p56.player, Player.BLACK)
        self.assertEqual(p56.piece_type, PieceType.DEFENDER)

    def test_round_trip_repr_from_str_idempotent(self):
        # Create several boards and assert round-trip equality
        boards = []

        # empty
        boards.append(ChessBallBoard())

        # one piece
        b1 = ChessBallBoard()
        b1.place_piece(2, 3, Piece(PieceType.DEFENDER, Player.WHITE))
        boards.append(b1)

        # mixed
        b2 = ChessBallBoard()
        b2.place_piece(0, 0, Piece(PieceType.ATTACKER, Player.WHITE))
        b2.place_piece(3, 3, Piece(PieceType.BALL, Player.NEUTRAL))
        b2.place_piece(5, 6, Piece(PieceType.DEFENDER, Player.BLACK))
        boards.append(b2)

        for original in boards:
            s = repr(original)
            parsed = ChessBallBoard.from_str(s)
            rs = repr(parsed)
            # repr strings should match exactly
            self.assertEqual(s, rs)
            # boards should be semantically equal
            self.assertTrue(boards_equal(original, parsed))

if __name__ == "__main__":
    unittest.main()