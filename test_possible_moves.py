from chessball_board import ChessBallBoard, Piece, PieceType, Player, possible_moves

def test_possible_moves_cases():
    print("Test 1: Simple moves (a single piece in center, all adjacent squares empty)\n")
    board1 = ChessBallBoard()
    board1.place_piece(2, 3, Piece(PieceType.DEFENDER, Player.WHITE))
    moves1 = possible_moves(board1, Player.WHITE)
    for info, next_board in moves1:
        print(f"Move: {info}")
        print(next_board)

    print("\nTest 2: Push move (piece is next to ball, ball can be pushed)\n")
    board2 = ChessBallBoard()
    board2.place_piece(2, 3, Piece(PieceType.DEFENDER, Player.WHITE))
    board2.place_piece(2, 4, Piece(PieceType.BALL, Player.NEUTRAL))
    moves2 = possible_moves(board2, Player.WHITE)
    for info, next_board in moves2:
        if info.get("push_ball"):
            print(f"Move: {info}")
            print(next_board)

    print("\nTest 3: Multiple push moves (piece adjacent to two balls in different directions)\n")
    board3 = ChessBallBoard()
    board3.place_piece(2, 3, Piece(PieceType.DEFENDER, Player.WHITE))
    board3.place_piece(2, 2, Piece(PieceType.BALL, Player.NEUTRAL))
    board3.place_piece(3, 3, Piece(PieceType.BALL, Player.NEUTRAL))
    moves3 = possible_moves(board3, Player.WHITE)
    for info, next_board in moves3:
        if info.get("push_ball"):
            print(f"Move: {info}")
            print(next_board)

    print("\nTest 4: Jump moves (attacker jumps over adjacent piece)\n")
    board4 = ChessBallBoard()
    board4.place_piece(2, 3, Piece(PieceType.ATTACKER, Player.WHITE))
    board4.place_piece(2, 4, Piece(PieceType.DEFENDER, Player.BLACK)) # right
    board4.place_piece(1, 3, Piece(PieceType.DEFENDER, Player.BLACK)) # up
    moves4 = possible_moves(board4, Player.WHITE)
    for info, next_board in moves4:
        if info.get("jump"):
            print(f"Move: {info}")
            print(next_board)

    print("\nTest 5: Tackle moves (defender pushes adjacent opponent piece)\n")
    board5 = ChessBallBoard()
    board5.place_piece(2, 3, Piece(PieceType.DEFENDER, Player.WHITE))
    board5.place_piece(2, 4, Piece(PieceType.ATTACKER, Player.BLACK)) # tackle right
    moves5 = possible_moves(board5, Player.WHITE)
    for info, next_board in moves5:
        if info.get("tackle"):
            print(f"Move: {info}")
            print(next_board)

if __name__ == "__main__":
    test_possible_moves_cases()