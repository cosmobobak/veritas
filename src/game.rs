use gomokugen::board::{Board, Player};

use crate::BOARD_SIZE;

/// Perform a rollout from the given state, returning the reward.
pub fn rollout(state: &Board<BOARD_SIZE>) -> f32 {
    let to_move = state.turn();
    let mut state = *state;
    let mut rng = fastrand::Rng::new();

    let outcome = loop {
        if let Some(outcome) = state.outcome() {
            break outcome;
        }
        state.make_random_move(|lo, hi| rng.usize(lo..hi));
    };

    let value_x_pov = match outcome {
        Player::None => 0.5,
        Player::X => 1.0,
        Player::O => 0.0,
    };

    if to_move == Player::X {
        value_x_pov
    } else {
        1.0 - value_x_pov
    }
}
