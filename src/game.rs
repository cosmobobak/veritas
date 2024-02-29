use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    None,
    First,
    Second,
}

/// Allows the extraction of the index of a move in a policy distribution.
pub trait MovePolicyIndex {
    /// The index of the move in the policy distribution.
    fn policy_index(&self) -> usize;
}

/// A wrapper around a game implementation.
/// Allows `veritas` to be generic over different game implementations.
#[allow(clippy::module_name_repetitions)]
pub trait GameImpl:
    Default + Display + Debug + Copy + Clone + FromStr + Send + Sync + 'static
{
    /// The dimensionality of the policy.
    const POLICY_DIM: usize;
    /// The associated move type.
    type Move: Copy + Eq + Display + Debug + FromStr + MovePolicyIndex;
    /// Which player is to move.
    fn to_move(&self) -> Player;
    /// The outcome of the game.
    fn outcome(&self) -> Option<Player>;
    /// Make a move.
    fn make_move(&mut self, mv: Self::Move);
    /// Generate a list of legal moves.
    fn generate_moves(&self, f: impl FnMut(Self::Move) -> bool);
    /// Return a textual representation of the game state.
    fn fen(&self) -> String;
    /// Fill the feature map with the current state.
    fn fill_feature_map(&self, index_callback: impl FnMut(usize));
    /// The dimensionality of the tensor representation of the game state.
    fn tensor_dims(batch_size: usize) -> kn_graph::ndarray::IxDyn;
    /// Make a random move.
    fn make_random_move(&mut self, mut rng: impl FnMut(usize, usize) -> usize) {
        let mut moves = Vec::new();
        self.generate_moves(|mv| {
            moves.push(mv);
            false
        });
        let mv = moves[rng(0, moves.len())];
        self.make_move(mv);
    }
    /// Perform a rollout from the given state, returning the reward.
    fn rollout(&self) -> f32 {
        let to_move = self.to_move();
        let mut state = *self;
        let mut rng = fastrand::Rng::new();

        let outcome = loop {
            if let Some(outcome) = state.outcome() {
                break outcome;
            }
            state.make_random_move(|lo, hi| rng.usize(lo..hi));
        };

        let value_x_pov = match outcome {
            Player::None => 0.5,
            Player::First => 1.0,
            Player::Second => 0.0,
        };

        if to_move == Player::First {
            value_x_pov
        } else {
            1.0 - value_x_pov
        }
    }
}

impl MovePolicyIndex for gomokugen::board::Move<9> {
    fn policy_index(&self) -> usize {
        self.index()
    }
}

impl GameImpl for gomokugen::board::Board<9> {
    const POLICY_DIM: usize = 9 * 9;
    type Move = gomokugen::board::Move<9>;
    fn to_move(&self) -> Player {
        match self.turn() {
            gomokugen::board::Player::None => Player::None,
            gomokugen::board::Player::X => Player::First,
            gomokugen::board::Player::O => Player::Second,
        }
    }
    fn outcome(&self) -> Option<Player> {
        match self.outcome() {
            None => None,
            Some(gomokugen::board::Player::None) => Some(Player::None),
            Some(gomokugen::board::Player::X) => Some(Player::First),
            Some(gomokugen::board::Player::O) => Some(Player::Second),
        }
    }
    fn make_move(&mut self, mv: Self::Move) {
        self.make_move(mv);
    }
    fn generate_moves(&self, f: impl FnMut(Self::Move) -> bool) {
        self.generate_moves(f);
    }
    fn fen(&self) -> String {
        self.fen()
    }
    fn fill_feature_map(&self, mut index_callback: impl FnMut(usize)) {
        let to_move = self.turn();
        self.feature_map(|i, c| {
            let index = i + usize::from(c != to_move) * 9 * 9;
            index_callback(index);
        });
    }
    fn tensor_dims(batch_size: usize) -> kn_graph::ndarray::IxDyn {
        kn_graph::ndarray::IxDyn(&[batch_size, 2 * 9 * 9])
    }
}

impl MovePolicyIndex for ataxxgen::Move {
    fn policy_index(&self) -> usize {
        self.policy_index()
    }
}

impl GameImpl for ataxxgen::Board {
    const POLICY_DIM: usize = 7 * 7 * 7 * 7;

    type Move = ataxxgen::Move;

    fn to_move(&self) -> Player {
        match self.turn() {
            ataxxgen::Player::White => Player::First,
            ataxxgen::Player::Black => Player::Second,
        }
    }

    fn outcome(&self) -> Option<Player> {
        match self.outcome() {
            None => None,
            Some(None) => Some(Player::None),
            Some(Some(ataxxgen::Player::White)) => Some(Player::First),
            Some(Some(ataxxgen::Player::Black)) => Some(Player::Second),
        }
    }

    fn make_move(&mut self, mv: Self::Move) {
        self.make_move(mv);
    }

    fn generate_moves(&self, f: impl FnMut(Self::Move) -> bool) {
        self.generate_moves(f);
    }

    fn fen(&self) -> String {
        self.fen()
    }

    fn fill_feature_map(&self, index_callback: impl FnMut(usize)) {
        // TODO: Implement
    }

    fn tensor_dims(batch_size: usize) -> kn_graph::ndarray::IxDyn {
        kn_graph::ndarray::IxDyn(&[batch_size, 2 * 7 * 7])
    }
}