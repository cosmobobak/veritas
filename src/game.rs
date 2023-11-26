use gomokugen::board::{Board, Move, Player};

/// Perform a rollout from the given state, returning the reward.
pub fn rollout(state: Board<15>) -> f32 {
    let to_move = state.turn();
    let mut state = state;
    let mut rng = fastrand::Rng::new();

    let outcome = loop {
        if let Some(outcome) = state.outcome() {
            break outcome;
        }
        let mut move_buffer = [Move::null(); 225];
        let mut n_moves = 0;
        state.generate_moves(|mv| {
            move_buffer[n_moves] = mv;
            n_moves += 1;
            false
        });
        let mv = move_buffer[rng.usize(..n_moves)];
        state.make_move(mv);
    };

    match outcome {
        Player::None => 0.5,
        Player::X => 1.0,
        Player::O => 0.0,
    }
}