use gomokugen::board::Move;

use crate::arena::Handle;

struct Edge {
    // Move corresponding to this node. From the point of view of a player.
    pov_move: Move,
    // Probability that this move will be made, from the policy head of the neural
    // network. TODO: leela compresses this into a short.
    probability: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Terminal {
    /// This node is not terminal.
    NonTerminal,
    /// This node is terminal.
    Terminal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GameResult {
    /// The game is ongoing.
    Ongoing,
    /// The game is a draw.
    Draw,
    /// The game is a win for the first player.
    FirstPlayerWin,
    /// The game is a win for the second player.
    SecondPlayerWin,
}

impl Edge {


    // Returns move from the point of view of the player making it (if as_opponent
    // is false) or as opponent (if as_opponent is true).
    pub const fn get_move(&self, as_opponent: bool) -> Move {
        if as_opponent {
            todo!()
        } else {
            self.pov_move
        }
    }

    pub const fn probability(&self) -> f32 {
        self.probability
    }

    pub fn set_probability(&mut self, probability: f32) {
        // TODO: check that probability is in [0, 1].
        self.probability = probability;
    }
}

pub struct Node {
    /// Average value (from value head of neural network) of all visited nodes in
    /// subtree. For terminal nodes, eval is stored. This is from the perspective
    /// of the player who "just" moved to reach this position, rather than from the
    /// perspective of the player-to-move for the position.
    /// WL stands for "W minus L". Is equal to Q if draw score is 0.
    wl: f64,
    /// Array of edges from this node.
    /// TODO: store the allocation length out-of-line, as it should fit in a u8.
    edges: Box<[Edge]>,
    /// Index of the parent node in the tree.
    parent: Handle,
    /// Index to a first child. Null for a leaf node.
    child: Handle,
    /// Index to a next sibling. Null if there are no more siblings.
    sibling: Handle,
    /// Averaged draw probability. Not flipped.
    draw_probability: f32,
    /// Estimated remaining plies until the end of the game.
    remaining: f32,
    /// Number of completed visits to this node.
    visits: u32,
    /// How many threads are currently visiting this node.
    num_in_flight: u32,
    /// Index of this node in the parent's edge list.
    index: u16,

    // TODO: pack the next three fields into a single u8.
    /// Whether this node ends the game.
    terminal_type: Terminal,
    /// Best possible outcome for this node.
    upper_bound: GameResult,
    /// Worst possible outcome for this node.
    lower_bound: GameResult,
}