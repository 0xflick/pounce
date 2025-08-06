use crate::chess::{self, movegen::utils::between};

// intial values are just the MG values for each piece
const SEE_VALUES: [i32; chess::Role::NUM] = [126, 781, 825, 1276, 2538, 0];

/// Does a static exchange evaluation (SEE) for a given move.
/// Returns true if the move is favorable (i.e., the material gained is greater than the
/// threshold).
pub fn see(pos: &chess::Position, mv: chess::Move, threshold: i32) -> bool {
    let from = mv.from();
    let to = mv.to();

    // the intial value of the move is the value of the piece being captured
    let mut value = see_best_case(pos, mv) - threshold;

    // if the best case is already below the threshold, we can stop early
    // the best being we just capture the piece we are attacking without any further exchanges
    if value < 0 {
        return false; // If the value is already less than the threshold, we can stop early
    }

    // next victim is the piece we attacked with, because the opponent will capture it (if they
    // can)
    let mut next_victim = if mv.promotion().is_some() {
        mv.promotion().unwrap()
    } else {
        pos.role_at(from)
            .expect("Invalid move: from square does not have a piece")
    };

    // the worst case is then losing the piece we moved, without being able to capture the piece
    value -= SEE_VALUES[next_victim as usize];

    // if the worse case is above the threshold, we can stop early
    if value >= 0 {
        return true;
    }

    let orth_sliders = pos.by_role[chess::Role::Rook] | pos.by_role[chess::Role::Queen];
    let diag_sliders = pos.by_role[chess::Role::Bishop] | pos.by_role[chess::Role::Queen];

    // "make" the move by updating the occupancy bitboard
    let mut occ = pos.occupancy ^ from | to;
    if mv.move_type(next_victim, pos.ep_square) == chess::chessmove::MoveType::EnPassant {
        occ ^= pos.ep_square.unwrap();
    }

    // side is the other side, since we just made a move
    let mut side = pos.side.opponent();

    // these are flipped because we just made a move
    let stm_pinned = pos.history.last().unwrap().pinned;
    let nstm_pinned = pos.pinned;

    let stm_king = chess::Square::from(pos.king_of(side));
    let nstm_king = chess::Square::from(pos.king_of(side.opponent()));

    let stm_king_between = between(to, stm_king);
    let nstm_king_between = between(to, nstm_king);

    // allowed moves are either anything that is not pinned, or if the piece is pinned, only moves
    // that are between the piece and the king
    let allowed = !(stm_pinned | nstm_pinned)
        | (stm_pinned & stm_king_between)
        | (nstm_pinned & nstm_king_between);

    let mut attackers = all_attackers(to, pos) & allowed;

    loop {
        let stm_attackers = attackers & pos.by_color[side];
        if stm_attackers.none() {
            break; // No more attackers, we can stop
        }

        // attack with the least valuable attacker
        for victim in chess::Role::ALL {
            next_victim = victim;
            if (stm_attackers & pos.by_role[victim]).any() {
                break; // Found a valid attacker
            }
        }

        // update occupancy
        occ ^= (stm_attackers & pos.by_role[next_victim]).lsb().unwrap();

        // if we moved diagonally, we need to update the diagonal sliders
        if next_victim == chess::Role::Bishop
            || next_victim == chess::Role::Queen
            || next_victim == chess::Role::Pawn
        {
            attackers |= chess::movegen::utils::get_bishop_moves(to, occ) & diag_sliders;
        }

        // if we moved orthogonally, we need to update the orthogonal sliders
        if next_victim == chess::Role::Rook || next_victim == chess::Role::Queen {
            attackers |= chess::movegen::utils::get_rook_moves(to, occ) & orth_sliders;
        }

        // update attackers based on new occupancy (we just removed the piece we attack with)
        attackers &= occ;

        // flip side
        side = side.opponent();

        // adjust balance
        value = -value - 1 - SEE_VALUES[next_victim as usize];

        if value > 0 {
            if next_victim == chess::Role::King && (attackers & pos.by_color[side]).any() {
                // if we are attacking with the king, and the opponent has a piece that can
                // attack, we just made an illegal move so we can stop
                side = side.opponent();
            }
            break;
        }
    }

    // the loser is the side that cannot capture the piece
    pos.side != side
}

fn see_best_case(pos: &chess::Position, mv: chess::Move) -> i32 {
    let to = mv.to();

    // inital value of the move is the value of the piece being captured
    let mut value = SEE_VALUES[pos.role_at(to).unwrap() as usize];

    // now adjust the value based on the move type
    match mv.move_type(pos.role_at(to).unwrap(), pos.ep_square) {
        chess::MoveType::Promotion => {
            value += SEE_VALUES[mv.promotion().unwrap() as usize]
                - SEE_VALUES[chess::Role::Pawn as usize];
        }
        chess::MoveType::EnPassant => {
            // En passant captures are worth the same as a pawn capture
            value = SEE_VALUES[chess::Role::Pawn as usize];
        }
        _ => {}
    }

    value
}

fn all_attackers(to: chess::Square, pos: &chess::Position) -> chess::bitboard::Bitboard {
    let mut attackers = chess::bitboard::Bitboard::EMPTY;

    // Check for pawn attacks
    attackers |= chess::movegen::utils::get_pawn_attacks(to, chess::Color::White)
        & pos.by_color_role(chess::Color::White, chess::Role::Pawn);
    attackers |= chess::movegen::utils::get_pawn_attacks(to, chess::Color::Black)
        & pos.by_color_role(chess::Color::Black, chess::Role::Pawn);

    // Check for knight attacks
    attackers |= chess::movegen::utils::get_knight_moves(to) & pos.by_role[chess::Role::Knight];

    // Check for king attacks
    attackers |= chess::movegen::utils::get_king_moves(to) & pos.by_role[chess::Role::King];

    // Check for rook attacks
    attackers |= chess::movegen::utils::get_rook_moves(to, pos.occupancy)
        & (pos.by_role[chess::Role::Rook] | pos.by_role[chess::Role::Queen]);

    // Check for bishop attacks
    attackers |= chess::movegen::utils::get_bishop_moves(to, pos.occupancy)
        & (pos.by_role[chess::Role::Bishop] | pos.by_role[chess::Role::Queen]);

    attackers
}

#[cfg(test)]
mod test {
    use crate::init;

    #[test]
    fn test_see() {
        init();
    }
}
