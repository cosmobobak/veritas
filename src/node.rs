use gomokugen::board::{Move, Board};

use crate::{arena::Handle, BOARD_SIZE};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Edge {
    // Move corresponding to this node. From the point of view of a player.
    pov_move: Move<BOARD_SIZE>,
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
    pub fn from_movelist(moves: &[Move<BOARD_SIZE>]) -> Box<[Self]> {
        #![allow(clippy::cast_precision_loss)]
        let mut edges = Vec::new();
        edges.reserve_exact(moves.len());
        for &m in moves {
            edges.push(Self {
                pov_move: m,
                probability: 1.0 / moves.len() as f32,
            });
        }
        edges.into_boxed_slice()
    }

    // Returns move from the point of view of the player making it (if as_opponent
    // is false) or as opponent (if as_opponent is true).
    pub const fn get_move(&self, as_opponent: bool) -> Move<BOARD_SIZE> {
        if as_opponent {
            todo!()
        } else {
            self.pov_move
        }
    }

    pub const fn probability(&self) -> f64 {
        self.probability as f64
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
    edges: Option<Box<[Edge]>>,
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

impl Node {
    /// Creates a new node.
    pub fn new(parent: Handle, edge_index: usize) -> Self {
        let index = edge_index.try_into().expect("too many edges");
        Self {
            wl: 0.0,
            edges: None,
            parent,
            child: Handle::null(),
            sibling: Handle::null(),
            draw_probability: 0.0,
            remaining: 0.0,
            visits: 0,
            num_in_flight: 0,
            index,
            terminal_type: Terminal::NonTerminal,
            upper_bound: GameResult::Ongoing,
            lower_bound: GameResult::Ongoing,
        }
    }

    /// Returns the move with the most visits.
    pub fn best_move(&self, tree: &[Self]) -> Move<BOARD_SIZE> {
        eprintln!("[CALL] Node::best_move(self, tree) (self.index = {})", self.index);

        let mut best_move = None;
        let mut best_visits = -1;
        let mut edge = self.child;
        while !edge.is_null() {
            let visits = tree[edge.index()].visits;
            eprintln!("  edge = {edge:?}, visits = {visits}");
            if i64::from(visits) > best_visits {
                // we have the index of the node in the tree - we want to get the move.
                // the move is stored in our edge list, but we don't know which edge in the
                // edge list that this node corresponds to, so we
                // 1. look up the node in the tree using the index
                // 2. get the index of the node's inbound edge in our edge list
                // 3. look up that index in our edge list.
                best_move = Some(self.edges().unwrap()[tree[edge.index()].edge_index()].get_move(false));
                best_visits = i64::from(visits);
            }
            edge = tree[edge.index()].sibling;
        }
        best_move.expect("no moves in node")
    }

    /// Returns the distribution of visits to the children of this node.
    pub fn dist(&self, tree: &[Self]) -> Vec<u64> {
        let mut dist = Vec::with_capacity(self.edges.as_ref().unwrap().len());
        let mut edge = self.child;
        while !edge.is_null() {
            dist.push(u64::from(tree[edge.index()].visits));
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
    pub const fn non_null_parent(&self, tree: &[Self]) -> Option<Handle> {
        if self.parent.is_null() {
            None
        } else {
            Some(self.parent)
        }
    }

    /// Expands this node, adding the legal moves and their policies.
    pub fn expand(&mut self, pos: Board<BOARD_SIZE>) {
        fn policy(_m: Move<BOARD_SIZE>) -> f32 {
            1.0 / (BOARD_SIZE.pow(2)) as f32
        }
        let mut moves = Vec::with_capacity(BOARD_SIZE * BOARD_SIZE);
        pos.generate_moves(|m| {
            let p = policy(m);
            moves.push(Edge {
                pov_move: m,
                probability: p,
            });
            false
        });
        self.edges = Some(moves.into_boxed_slice());
    }

    /// Whether this node is terminal.
    pub fn is_terminal(&self) -> bool {
        self.terminal_type == Terminal::Terminal
    }
}

pub fn print_tree(root: usize, tree: &[Node]) {
    let mut stack = vec![Handle::from_index(root, tree)];
    while let Some(node) = stack.pop() {
        let node = &tree[node.index()];
        println!(
            "Node from move {} (parent {:?}, child {:?}, sibling {:?})",
            node.index, node.parent, node.child, node.sibling
        );
        if let Some(edges) = node.edges() {
            print!("  Edges: [");
            for edge in edges {
                print!("{edge:?}, ");
            }
            println!("]");
        }
        if !node.child.is_null() {
            stack.push(node.child);
        }
        if !node.sibling.is_null() {
            stack.push(node.sibling);
        }
    }
}