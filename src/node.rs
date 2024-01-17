use std::alloc::Layout;

use gomokugen::board::{Board, Move, Player};
use smallvec::SmallVec;

use crate::{arena::Handle, BOARD_SIZE};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Edge {
    // Move corresponding to this node. From the point of view of a player.
    pov_move: Move<BOARD_SIZE>,
    // Probability that this move will be made, from the policy head of the neural
    // network. TODO: leela compresses this into a short.
    probability: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Terminal {
    /// This node is not terminal.
    NonTerminal,
    /// This node is terminal.
    Terminal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    pub const fn get_move(self, as_opponent: bool) -> Move<BOARD_SIZE> {
        if as_opponent {
            todo!()
        } else {
            self.pov_move
        }
    }

    pub const fn probability(self) -> f64 {
        self.probability as f64
    }
}

#[derive(Debug)]
pub struct Node {
    /// Average value (from value head of neural network) of all visited nodes in
    /// subtree. For terminal nodes, eval is stored. This is from the perspective
    /// of the player who "just" moved to reach this position, rather than from the
    /// perspective of the player-to-move for the position.
    /// WL stands for "W minus L". Is equal to Q if draw score is 0.
    wl: f64,
    /// Array of edges from this node.
    /// TODO: store the allocation length out-of-line, as it should fit in a u8.
    edges: Option<Box<[Edge]>>,
    /// Index of the parent node in the tree.
    parent: Handle,
    /// Index to a first child. Null for a leaf node.
    child: Handle,
    /// Index to a next sibling. Null if there are no more siblings.
    sibling: Handle,
    /// Averaged draw probability. Not flipped.
    // draw_probability: f32,
    /// Estimated remaining plies until the end of the game.
    // remaining: f32,
    /// Number of completed visits to this node.
    visits: u32,
    /// How many threads are currently visiting this node.
    // num_in_flight: u32,
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

impl Node {
    /// Creates a new node.
    pub fn new(parent: Handle, edge_index: usize) -> Self {
        let index = edge_index
            .try_into()
            .unwrap_or_else(|_| panic!("edge index {edge_index} too large"));
        Self {
            wl: 0.0,
            edges: None,
            parent,
            child: Handle::null(),
            sibling: Handle::null(),
            // draw_probability: 0.0,
            // remaining: 0.0,
            visits: 0,
            // num_in_flight: 0,
            index,
            terminal_type: Terminal::NonTerminal,
            upper_bound: GameResult::Ongoing,
            lower_bound: GameResult::Ongoing,
        }
    }

    /// Returns the move with the most visits.
    pub fn best_move(&self, tree: &[Self]) -> Move<BOARD_SIZE> {
        log::trace!("Node::best_move(self, tree) (self.index = {})", self.index);

        let mut best_move = None;
        let mut best_visits = -1;
        let mut edge = self.child;
        while !edge.is_null() {
            let visits = tree[edge.index()].visits;
            // log::trace!("  edge = {edge:?}, visits = {visits}");
            if i64::from(visits) > best_visits {
                // we have the index of the node in the tree - we want to get the move.
                // the move is stored in our edge list, but we don't know which edge in the
                // edge list that this node corresponds to, so we
                // 1. look up the node in the tree using the index
                // 2. get the index of the node's inbound edge in our edge list
                // 3. look up that index in our edge list.
                best_move =
                    Some(self.edges().unwrap()[tree[edge.index()].edge_index()].get_move(false));
                best_visits = i64::from(visits);
            }
            edge = tree[edge.index()].sibling;
        }
        best_move.expect("no moves in node")
    }

    /// Returns the distribution of visits to the children of this node.
    pub fn dist(&self, tree: &[Self]) -> Vec<u64> {
        let mut dist = vec![0; BOARD_SIZE * BOARD_SIZE];
        let mut edge = self.child;
        while !edge.is_null() {
            let move_index = self.edges.as_ref().unwrap()[tree[edge.index()].edge_index()]
                .get_move(false)
                .index();
            let visits = u64::from(tree[edge.index()].visits);
            dist[move_index] = visits;
            edge = tree[edge.index()].sibling;
        }
        dist
    }

    /// Returns the number of visits to this node.
    pub const fn visits(&self) -> u32 {
        self.visits
    }

    /// Returns the winrate of this node.
    pub fn winrate(&self) -> f64 {
        self.wl / f64::from(self.visits)
    }

    /// Add a visit to this node.
    pub fn add_visit(&mut self, value: f64) {
        self.wl += value;
        self.visits += 1;
    }

    /// Returns a reference to the edges of this node.
    pub fn edges(&self) -> Option<&[Edge]> {
        self.edges.as_deref()
    }

    /// Returns the first child of this node.
    pub const fn first_child(&self) -> Handle {
        self.child
    }

    /// Returns a mutable reference to the first child of this node.
    pub fn first_child_mut(&mut self) -> &mut Handle {
        &mut self.child
    }

    /// Returns the index of this node in the parent's edge list.
    pub const fn edge_index(&self) -> usize {
        self.index as usize
    }

    /// Returns the next sibling of this node.
    pub const fn sibling(&self) -> Handle {
        self.sibling
    }

    /// Returns a mutable reference to the next sibling of this node.
    pub fn sibling_mut(&mut self) -> &mut Handle {
        &mut self.sibling
    }

    /// Returns the parent of the node.
    pub const fn non_null_parent(&self, _tree: &[Self]) -> Option<Handle> {
        if self.parent.is_null() {
            None
        } else {
            Some(self.parent)
        }
    }

    /// Expands this node, adding the legal moves and their policies.
    pub fn expand(&mut self, &pos: &Board<BOARD_SIZE>, policy: &[f32]) {
        let mut moves = SmallVec::<[Edge; BOARD_SIZE * BOARD_SIZE]>::new();
        let mut max_logit = -1000.0;
        pos.generate_moves(|m| {
            let logit = policy[m.index()];
            if logit > max_logit {
                max_logit = logit;
            }
            moves.push(Edge {
                pov_move: m,
                probability: logit,
            });
            false
        });
        // normalize the probabilities
        // subtract the maximum probability from all probabilities
        // and exponentiate them, summing them as we go.
        let mut total = 0.0;
        for edge in &mut moves {
            edge.probability = (edge.probability - max_logit).exp();
            total += edge.probability;
        }
        // divide each probability by the total to normalize them
        for edge in &mut moves {
            edge.probability /= total;
            assert!(
                (0.0..=1.0).contains(&edge.probability),
                "got an illegal move probability - p({}) = {} but should be in [0, 1]!",
                edge.pov_move,
                edge.probability
            );
        }

        // allocate the edge list and copy the moves into it
        unsafe {
            let layout = Layout::array::<Edge>(moves.len()).unwrap();
            // cast_ptr_alignment is fine because we're allocating using the Edge layout
            #[allow(clippy::cast_ptr_alignment)]
            let ptr = std::alloc::alloc(layout).cast::<Edge>();
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            // copy the moves into the edge list
            ptr.copy_from_nonoverlapping(moves.as_ptr(), moves.len());
            let boxed_slice = Box::from_raw(std::slice::from_raw_parts_mut(ptr, moves.len()));
            self.edges = Some(boxed_slice);
        }

        if let Some(result) = pos.outcome() {
            self.terminal_type = Terminal::Terminal;
            let game_result = match result {
                Player::None => GameResult::Draw,
                Player::X => GameResult::FirstPlayerWin,
                Player::O => GameResult::SecondPlayerWin,
            };
            self.upper_bound = game_result;
            self.lower_bound = game_result;
        }
    }

    /// Whether this node is terminal.
    pub fn is_terminal(&self) -> bool {
        self.terminal_type == Terminal::Terminal
    }
}