// A small set of tests translated from the Python tests. Place under tests/ or run with `cargo test`.
#[cfg(test)]
mod tests {
    use crate::board::{ChessBallBoard, Piece, PieceType, Player};
    use crate::moves::{possible_moves, possible_previous_moves};

    fn print_two_boards(_b1: &ChessBallBoard, _b2: &ChessBallBoard) {
        // Omitted; tests will assert properties instead of printing.
    }

    #[test]
    fn test_simple_moves() {
        let mut b = ChessBallBoard::new();
        b.place_piece(2, 3, Piece { piece_type: PieceType::Defender, player: Player::White });
        let moves = possible_moves(&b, Player::White);
        assert!(moves.len() >= 1);
    }

    #[test]
    fn test_push_move() {
        let mut b = ChessBallBoard::new();
        b.place_piece(2, 3, Piece { piece_type: PieceType::Defender, player: Player::White });
        b.place_piece(2, 4, Piece { piece_type: PieceType::Ball, player: Player::Neutral });
        let mut found_push = false;
        for (info, _nb) in possible_moves(&b, Player::White) {
            if info.push_ball { found_push = true; break; }
        }
        assert!(found_push);
    }

    #[test]
    fn test_previous_moves() {
        let mut b = ChessBallBoard::new();
        b.place_piece(2, 4, Piece { piece_type: PieceType::Ball, player: Player::Neutral});
        b.place_piece(2, 3, Piece { piece_type: PieceType::Defender, player: Player::White});
        let prevs = possible_previous_moves(&b, Player::White);
        assert!(prevs.len() >= 1);
    }
}