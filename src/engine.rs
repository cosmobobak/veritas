use std::time::Instant;

use gomokugen::board::{Move, Board};

use crate::{node::Node, params::Params, timemgmt::Limits, arena::Handle};

pub struct SearchResults {
    /// The best move found.
    pub best_move: Move,
    /// The root rollout distribution.
    pub root_dist: Vec<u64>,
}

/// The MCTS engine's state.
pub struct Engine {
    /// Parameters of the search - exploration factor, c-PUCT, etc.
    params: Params,
    /// Limits on the search - time, nodes, etc.
    limits: Limits,
    /// The storage for the search tree.
    tree: Vec<Node>,
    /// The root position.
    root: Board<15>,
}

impl Engine {
    /// Creates a new engine.
    pub const fn new(params: Params, limits: Limits, root: Board<15>) -> Self {
        Self {
            params,
            limits,
            tree: Vec::new(),
            root,
        }
    }

    /// Sets the limits on the search.
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// Runs the engine.
    pub fn go(&mut self) -> SearchResults {
        Self::search(self.root, &mut self.tree, &self.params, &self.limits);

        let best_move = self.tree[0].best_move(&self.tree);

        let root_dist = self.tree[0].dist(&self.tree);

        SearchResults {
            best_move,
            root_dist,
        }
    }

    /// Repeat the search loop until the time limit is reached.
    fn search(root: Board<15>, tree: &mut Vec<Node>, params: &Params, limits: &Limits) {
        let start_time = Instant::now();
        let mut nodes_searched = 0;
        let mut elapsed = 0;

        while !limits.is_out_of_time(nodes_searched, elapsed) {
            // perform one iteration of selection, expansion, simulation, and backpropagation
            Self::do_sesb(root, tree, params);

            // update elapsed time
            if nodes_searched % 256 == 0 {
                elapsed = u64::try_from(start_time.elapsed().as_millis()).expect("elapsed time overflow");
            }
            // update nodes searched
            nodes_searched += 1;
        }
    }

    /// Performs one iteration of selection, expansion, simulation, and backpropagation.
    fn do_sesb(root: Board<15>, tree: &mut Vec<Node>, params: &Params) {
        // select
        let (best_node, edge_to_expand, board_state) = Self::select(root, tree, params, 0);

        // expand
        let new_node = Self::expand(tree, params, best_node, edge_to_expand);

        // simulate
        let value = (params.valuator)(&board_state);

        // backpropagate
        Self::backpropagate(tree, new_node, value);
    }

    /// Descends the tree, selecting the best node at each step.
    /// Returns the index of a node, and the index of the edge to be expanded.
    fn select(root: Board<15>, tree: &mut [Node], params: &Params, mut node_idx: usize) -> (usize, usize, Board<15>) {
        let mut pos = root;
        loop {
            // if the node has had a single visit, expand it
            if tree[node_idx].visits() == 1 {
                tree[node_idx].expand(pos);
            }

            // if the node is terminal, return it
            if tree[node_idx].is_terminal() {
                return (node_idx, usize::MAX, pos);
            }

            let (edge_idx, child_idx) = Self::uct_best(tree, params, node_idx);
            // if the node has no children, return it
            if child_idx.is_null() {
                return (node_idx, edge_idx, pos);
            }

            // it's *not* unexpanded, so we can descend
            let edge = &tree[node_idx].edges().unwrap()[edge_idx];
            let mv = edge.get_move(false);
            pos.make_move(mv);

            // descend
            node_idx = child_idx.index();
        }
    }

    /// Selects the best immediate edge of a node according to UCT.
    /// Returns the index of the edge, and a nullable handle to the child.
    fn uct_best(tree: &[Node], params: &Params, node_idx: usize) -> (usize, Handle) {
        let node = &tree[node_idx];

        let exploration_factor = params.c_puct * f64::from(node.visits()).sqrt();

        let first_play_urgency = if node.visits() > 0 {
            1.0 - node.winrate()
        } else {
            0.5
        };

        let mut best_idx = 0;
        let mut best_value = f64::NEG_INFINITY;
        let mut best_child = Handle::null();

        let edges = node.edges().unwrap();
        let mut child = node.first_child();

        // This is slightly problematic because we have to do linked list stuff where
        // only some of the edges have corresponding nodes.
        // The simplest solution is just to have an array that we fill in.
        let mut values = [None; 15 * 15];
        while !child.is_null() {
            let node = &tree[child.index()];
            let edge = &edges[node.edge_index()];
            let q = node.winrate();
            let u = exploration_factor * edge.probability() / (1.0 + f64::from(node.visits()));
            values[node.edge_index()] = Some((child, q + u));
            child = node.sibling();
        }
        for (idx, value) in values.into_iter().take(edges.len()).enumerate() {
            if let Some((handle, value)) = value {
                if value > best_value {
                    best_idx = idx;
                    best_value = value;
                    best_child = handle;
                }
            } else {
                let value = exploration_factor.mul_add(edges[idx].probability(), first_play_urgency);
                if value > best_value {
                    best_idx = idx;
                    best_value = value;
                    best_child = Handle::null();
                }
            }
        }

        (best_idx, best_child)
    }

    /// Expands an edge of a given node, returning a handle to the new node.
    fn expand(tree: &mut Vec<Node>, params: &Params, node_idx: usize, edge_idx: usize) -> Handle {
        let last_child_of_expanding_node = {
            // get a reference to the last expanded child of the node
            let mut child = tree[node_idx].first_child();
            while !child.is_null() {
                let node = &tree[child.index()];
                if node.sibling().is_null() {
                    break;
                }
                child = node.sibling();
            }
            child
        };

        // allocate a new node
        let parent_handle = Handle::from_index(node_idx, tree);
        let new_node = Node::new(parent_handle, edge_idx);

        // write the new node to the tree
        tree.push(new_node);
        let handle = Handle::from_index(tree.len() - 1, tree);

        let memory_to_write_to = if last_child_of_expanding_node.is_null() {
            // there were *no* children, so we can just write to the node itself
            tree[node_idx].first_child_mut()
        } else {
            // there were children, so we have to write to the sibling of the last child
            tree[last_child_of_expanding_node.index()].sibling_mut()
        };

        assert!(memory_to_write_to.is_null(), "attempted to overwrite a non-null handle.");
        *memory_to_write_to = handle;

        handle
    }

    /// Backpropagates the value up the tree.
    fn backpropagate(tree: &mut [Node], mut node: Handle, mut value: f64) {
        while let Some(parent) = tree[node.index()].non_null_parent(tree) {
            tree[parent.index()].add_visit(value);
            node = parent;
            value = 1.0 - value;
        }
    }
}