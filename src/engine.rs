// use gomokugen::board::{Board, Move, Player};
use log::{debug, trace};
// use std::io::Write;
use std::{sync::atomic::Ordering, time::Instant};

use crate::{
    arena::Handle,
    batching::ExecutorHandle,
    game::{GameImpl, Player},
    node::Node,
    params::Params,
    timemgmt::Limits,
    ugi,
};

pub struct SearchResults<G: GameImpl> {
    /// The best move found.
    pub best_move: G::Move,
    /// The root rollout distribution.
    pub root_dist: Vec<u64>,
}

/// The MCTS engine's state.
pub struct Engine<'a, G: GameImpl> {
    /// Parameters of the search - exploration factor, c-PUCT, etc.
    params: Params<'a>,
    /// Limits on the search - time, nodes, etc.
    limits: Limits,
    /// The storage for the search tree.
    tree: Vec<Node<G>>,
    /// The root position.
    root: G,
    /// Interface to the CUDA executor.
    eval_pipe: ExecutorHandle<G>,
}

enum SelectionResult<G: GameImpl> {
    NonTerminal {
        node_index: usize,
        edge_index: usize,
        board_state: G,
    },
    Terminal {
        node_index: usize,
        board_state: G,
    },
}

impl<'a, G: GameImpl> Engine<'a, G> {
    /// Creates a new engine.
    pub const fn new(
        params: Params<'a>,
        limits: Limits,
        root: &G,
        eval_pipe: ExecutorHandle<G>,
    ) -> Self {
        Self {
            params,
            limits,
            tree: Vec::new(),
            root: *root,
            eval_pipe,
        }
    }

    pub const fn root(&self) -> G {
        self.root
    }

    /// Sets the limits on the search.
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    /// Get access to the parameters of the search.
    pub fn params_mut(&mut self) -> &mut Params<'a> {
        &mut self.params
    }

    /// Sets the position to search from.
    /// This clears the search tree, but could in future be altered to retain some subtree.
    pub fn set_position(&mut self, root: &G) {
        self.root = *root;
        self.tree.clear();
    }

    /// Runs the engine.
    pub fn go(&mut self) -> SearchResults<G> {
        trace!("Engine::go()");

        Self::search(
            &self.eval_pipe,
            &self.root,
            &mut self.tree,
            &self.params,
            &self.limits,
        );

        let best_move = self.tree[0].best_move(&self.tree);

        let root_dist = self.tree[0].dist(&self.tree);

        SearchResults {
            best_move,
            root_dist,
        }
    }

    /// Repeat the search loop until the time limit is reached.
    fn search(
        executor: &ExecutorHandle<G>,
        root: &G,
        tree: &mut Vec<Node<G>>,
        params: &Params,
        limits: &Limits,
    ) {
        #![allow(clippy::cast_precision_loss)]
        trace!("Engine::search(root, tree, params, limits)");

        let start_time = Instant::now();
        let mut nodes_searched = 0;
        let mut elapsed = 0;

        if tree.is_empty() {
            // create the root node
            tree.push(Node::new(Handle::null(), 0));
            // send the root to the executor
            executor
                .sender
                .send(*root)
                .expect("failed to send board to executor");
            // wait for the result
            let (policy, _value) = executor
                .receiver
                .recv()
                .expect("failed to receive value from executor");
            tree[0].expand(*root, &policy);
        }

        // let mut log = std::io::BufWriter::new(std::fs::File::create("log.txt").unwrap());

        let mut stopped_by_stdin = false;
        while !limits.is_out_of_time(nodes_searched, elapsed) && !stopped_by_stdin {
            // perform one iteration of selection, expansion, simulation, and backpropagation
            Self::do_sesb(executor, root, tree, params);

            // update elapsed time and print stats
            if nodes_searched % 1024 == 0 {
                if params.do_stdout {
                    print!(
                        "info nodes {} time {} nps {:.0} score q {:.1} pv",
                        nodes_searched,
                        elapsed,
                        nodes_searched as f64 / (elapsed as f64 / 1000.0),
                        tree[0].winrate() * 100.0
                    );
                    Self::print_pv(root, tree);
                }
                elapsed =
                    u64::try_from(start_time.elapsed().as_millis()).expect("elapsed time overflow");
                stopped_by_stdin =
                    if let Some(Ok(cmd)) = params.stdin_rx.map(|m| m.lock().unwrap().try_recv()) {
                        let cmd = cmd.trim();
                        if cmd == "quit" {
                            ugi::QUIT.store(true, Ordering::SeqCst);
                        }
                        debug!("received command: {}", cmd);
                        true
                    } else {
                        false
                    };
                // write the root rollout distribution to log.txt
                // let root_dist = tree[0].dist(tree);
                // for visit_count in root_dist {
                //     write!(log, "{visit_count},").unwrap();
                // }
                // writeln!(log).unwrap();
            }
            // update nodes searched
            nodes_searched += 1;
        }

        trace!(
            "Engine::search: finished search loop with {} entries in tree.",
            tree.len()
        );
    }

    /// Performs one iteration of selection, expansion, simulation, and backpropagation.
    fn do_sesb(executor: &ExecutorHandle<G>, root: &G, tree: &mut Vec<Node<G>>, params: &Params) {
        trace!("Engine::do_sesb(root, tree, params)");

        // select
        let selection = Self::select(root, tree, params, 0);

        match selection {
            SelectionResult::NonTerminal {
                node_index: best_node,
                edge_index: edge_to_expand,
                mut board_state,
            } => {
                // expand
                let new_node = Self::expand(tree, params, best_node, edge_to_expand);

                // make the move
                let edge = &tree[best_node].edges().unwrap()[edge_to_expand];
                let mv = edge.get_move(false);
                board_state.make_move(mv);

                // simulate
                // send the board to the executor
                executor
                    .sender
                    .send(board_state)
                    .expect("failed to send board to executor");
                // wait for the result
                let (policy, value) = executor
                    .receiver
                    .recv()
                    .expect("failed to receive value from executor");

                // expand this node
                tree[new_node.index()].expand(board_state, &policy);

                // backpropagate
                Self::backpropagate(tree, new_node, 1.0 - f64::from(value));
            }
            SelectionResult::Terminal {
                node_index: best_node,
                board_state,
            } => {
                // if the node is terminal, we don't need to expand it.
                // we just need to backpropagate the result.
                let value = match board_state.outcome() {
                    None => unreachable!("terminal node has no outcome"),
                    Some(Player::None) => 0.5, // draw
                    Some(p) => {
                        if p == board_state.to_move() {
                            0.0
                        } else {
                            1.0
                        }
                    }
                };
                let node = Handle::from_index(best_node, tree);
                Self::backpropagate(tree, node, value);
            }
        };
    }

    /// Descends the tree, selecting the best node at each step.
    /// Returns the index of a node, and the index of the edge to be expanded.
    fn select(
        root: &G,
        tree: &mut [Node<G>],
        params: &Params,
        mut node_idx: usize,
    ) -> SelectionResult<G> {
        trace!("Engine::select(root, tree, params, node_idx = {node_idx})");

        let mut pos = *root;
        loop {
            // if the node has had a single visit, expand it
            // here, "expand" means adding all the legal moves to the node
            // with corresponding policy probabilities.
            if tree[node_idx].visits() == 1 {
                tree[node_idx].check_game_over(&pos);
            }

            // if the node is terminal, return it
            if tree[node_idx].is_terminal() {
                trace!(
                    "Engine::select: terminal node reached: index {node_idx}, position {}",
                    pos.fen()
                );
                return SelectionResult::Terminal {
                    node_index: node_idx,
                    board_state: pos,
                };
            }

            let (edge_idx, child_idx) = Self::uct_best(tree, params, node_idx);
            // if the node has no children, return it, because we can't descend any further.
            if child_idx.is_null() {
                return SelectionResult::NonTerminal {
                    node_index: node_idx,
                    edge_index: edge_idx,
                    board_state: pos,
                };
            }

            // it's *not* unexpanded, so we can descend
            trace!("Engine::select: descending to child {}", child_idx.index());
            let edge = &tree[node_idx].edges().unwrap()[edge_idx];
            let mv = edge.get_move(false);
            pos.make_move(mv);

            // descend
            node_idx = child_idx.index();
        }
    }

    /// Prints out the current line of best play.
    pub fn print_pv(root: &G, tree: &[Node<G>]) {
        let mut node_idx = Handle::from_index(0, tree);
        let mut pos = *root;
        while !node_idx.is_null() {
            if tree[node_idx.index()].edges().is_none() {
                break;
            }
            let (edge_idx, child_idx) = Self::rollouts_best(tree, node_idx.index());
            let Some(edge) = tree[node_idx.index()]
                .edges()
                .expect("node has no edges")
                .get(edge_idx)
            else {
                break;
            };
            let best_move = edge.get_move(false);
            print!(" {best_move}");
            pos.make_move(best_move);
            node_idx = child_idx;
        }
        println!();
    }

    /// Selects the best immediate edge of a node according to UCT.
    /// Returns the index of the edge, and a nullable handle to the child.
    fn uct_best(tree: &[Node<G>], params: &Params, node_idx: usize) -> (usize, Handle) {
        trace!("Engine::uct_best(tree, params, node_idx = {node_idx})");

        let node = &tree[node_idx];

        let exploration_factor = params.c_puct * f64::from(node.visits() + 1).sqrt();
        trace!(" [uct_best] exploration_factor = {exploration_factor}");

        let first_play_urgency = if node.visits() > 0 {
            1.0 - node.winrate()
        } else {
            0.5
        };

        let mut best_idx = 0;
        let mut best_value = f64::NEG_INFINITY;
        let mut best_child = Handle::null();

        let edges = node.edges().unwrap_or_else(|| {
            panic!("attempted to select the best edge of an unexpanded node. node = {node:?}");
        });
        let mut child = node.first_child();

        // This is slightly problematic because we have to do linked list stuff where
        // only some of the edges have corresponding nodes.
        // The simplest solution is just to have an array that we fill in.
        let mut values = vec![None; G::POLICY_DIM];
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
                trace!(" [expanded] edge = {idx}, value = {value}");
                if value > best_value {
                    best_idx = idx;
                    best_value = value;
                    best_child = handle;
                }
            } else {
                let value =
                    exploration_factor.mul_add(edges[idx].probability(), first_play_urgency);
                trace!(" [dangling] edge = {idx}, value = {value}, fpu = {first_play_urgency}, p(edge) = {}", edges[idx].probability());
                if value > best_value {
                    best_idx = idx;
                    best_value = value;
                    best_child = Handle::null();
                }
            }
        }

        (best_idx, best_child)
    }

    /// Selects the best immediate edge of a node according to rollout count.
    /// Returns the index of the edge, and a nullable handle to the child.
    fn rollouts_best(tree: &[Node<G>], node_idx: usize) -> (usize, Handle) {
        trace!("Engine::rollouts_best(tree, params, node_idx = {node_idx})");

        let node = &tree[node_idx];

        let mut best_idx = 0;
        let mut best_value = f64::NEG_INFINITY;
        let mut best_child = Handle::null();

        let edges = node.edges().unwrap_or_else(|| {
            panic!("attempted to select the best edge of an unexpanded node. node = {node:?}");
        });
        let mut child = node.first_child();

        // This is slightly problematic because we have to do linked list stuff where
        // only some of the edges have corresponding nodes.
        // The simplest solution is just to have an array that we fill in.
        let mut values = vec![None; G::POLICY_DIM];
        while !child.is_null() {
            let node = &tree[child.index()];
            let r = node.visits();
            values[node.edge_index()] = Some((child, f64::from(r)));
            child = node.sibling();
        }
        for (idx, value) in values.into_iter().take(edges.len()).enumerate() {
            if let Some((handle, value)) = value {
                trace!(" [expanded] edge = {idx}, value = {value}");
                if value > best_value {
                    best_idx = idx;
                    best_value = value;
                    best_child = handle;
                }
            } else {
                let value = edges[idx].probability();
                trace!(
                    " [dangling] edge = {idx}, value = {value}, p(edge) = {}",
                    edges[idx].probability()
                );
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
    fn expand(
        tree: &mut Vec<Node<G>>,
        _params: &Params,
        node_idx: usize,
        edge_index: usize,
    ) -> Handle {
        trace!("Engine::expand(tree, params, node_idx = {node_idx}, edge_idx = {edge_index})");

        let last_child_of_expanding_node = {
            // get a reference to the last expanded child of the node
            // TODO: rearchitect this without the break and with a guard.
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
        let new_node = Node::new(parent_handle, edge_index);

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

        assert!(
            memory_to_write_to.is_null(),
            "attempted to overwrite a non-null handle."
        );
        *memory_to_write_to = handle;

        handle
    }

    /// Backpropagates the value up the tree.
    fn backpropagate(tree: &mut [Node<G>], mut node: Handle, mut value: f64) {
        trace!("Engine::backpropagate(tree, node, value)");

        // backpropagate the value up the tree
        tree[node.index()].add_visit(value);
        while let Some(parent) = tree[node.index()].non_null_parent(tree) {
            value = 1.0 - value;
            tree[parent.index()].add_visit(value);
            node = parent;
        }
    }
}
