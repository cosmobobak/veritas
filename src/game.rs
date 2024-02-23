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
