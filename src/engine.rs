use std::{sync::atomic::Ordering, time::Instant};
use std::io::Write;
use gomokugen::board::{Board, Move, Player};
// use kn_cuda_eval::{executor::CudaExecutor, CudaDevice};
use kn_graph::{
    dtype::{DTensor, Tensor},
    graph::Graph,
    ndarray::IxDyn,
};
use log::{trace, debug};

use crate::{arena::Handle, node::Node, params::Params, timemgmt::Limits, ugi, BOARD_SIZE};

pub struct SearchResults {
    /// The best move found.
    pub best_move: Move<BOARD_SIZE>,
    /// The root rollout distribution.
    pub root_dist: Vec<u64>,
}

/// The MCTS engine's state.
pub struct Engine<'a> {
    /// Parameters of the search - exploration factor, c-PUCT, etc.
    params: Params<'a>,
    /// Limits on the search - time, nodes, etc.
    limits: Limits,
    /// The storage for the search tree.
    tree: Vec<Node>,
    /// The root position.
    root: Board<BOARD_SIZE>,
    /// The policy network.
    nn_policy: &'a Graph,
    /// A CUDA executor.
    #[cfg(feature = "cuda")]
    cuda_executor: Option<&'a CudaExecutor>,
}

enum SelectionResult {
    NonTerminal {
        node_index: usize,
        edge_index: usize,
        board_state: Board<BOARD_SIZE>,
    },
    Terminal {
        node_index: usize,
        board_state: Board<BOARD_SIZE>,
    },
}

impl<'a> Engine<'a> {
    /// Creates a new engine.
    pub const fn new(
        params: Params<'a>,
        limits: Limits,
        root: &Board<BOARD_SIZE>,
        nn_policy: &'a Graph,
    ) -> Self {
        Self {
            params,
            limits,
            tree: Vec::new(),
            root: *root,
            nn_policy,
        }
    }

    pub const fn root(&self) -> Board<BOARD_SIZE> {
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
    pub fn set_position(&mut self, root: &Board<BOARD_SIZE>) {
        self.root = *root;
        self.tree.clear();
    }

    /// Runs the engine.
    pub fn go(&mut self) -> SearchResults {
        trace!("Engine::go()");

        Self::search(
            &self.root,
            &mut self.tree,
            &self.params,
            &self.limits,
            self.nn_policy,
        );

        let best_move = self.tree[0].best_move(&self.tree);

        let root_dist = self.tree[0].dist(&self.tree);

        SearchResults {
            best_move,
            root_dist,
        }
    }

    /// Evaluates the policy network on a position.
    fn generate_policy(graph: &kn_graph::graph::Graph, board: &Board<BOARD_SIZE>, cpu: bool) -> Vec<f32> {
        return vec![1.0; BOARD_SIZE.pow(2)];
        // if cpu {
            Self::generate_policy_cpu(graph, board)
        // } else {
        //     Self::generate_policy_cuda(graph, board)
        // }
    }
    fn generate_policy_cpu(graph: &kn_graph::graph::Graph, board: &Board<BOARD_SIZE>) -> Vec<f32> {
        // build inputs
        let batch_size = 1;
        // inputs are a 162 1-D element vector
        let mut tensor = Tensor::zeros(IxDyn(&[batch_size, 162]));
        // set the input data from the board state
        let to_move = board.turn();
        board.feature_map(|i, c| {
            let index = i + usize::from(c != to_move) * 81;
            tensor[[0, index]] = 1.0;
        });
        let inputs = [DTensor::F32(tensor)];

        // evaluate the graph on this input
        let tensors = kn_graph::cpu::cpu_eval_graph(graph, batch_size, &inputs);
        assert_eq!(tensors.len(), 1);

        // get the output as a vector
        tensors[0]
            .unwrap_f32()
            .unwrap()
            .as_slice()
            .unwrap()
            .to_vec()
    }
    // fn generate_policy_cuda(graph: &kn_graph::graph::Graph, board: &Board<9>) -> Vec<f32> {
    //     let device = CudaDevice::new(0)
    //         .expect("failed to create CUDA device.");
    //     let mut executor = CudaExecutor::new(device, graph, 1);

    //     // inputs are a 162 1-D element vector
    //     let mut tensor = Tensor::zeros(IxDyn(&[1, 162]));
    //     // set the input data from the board state
    //     let to_move = board.turn();
    //     board.feature_map(|i, c| {
    //         let index = i + usize::from(c != to_move) * 81;
    //         tensor[[0, index]] = 1.0;
    //     });
    //     let inputs = [DTensor::F32(tensor)];

    //     // run the executor
    //     let tensors = executor.evaluate(&inputs);
    //     assert_eq!(tensors.len(), 1);

    //     // get the output as a vector
    //     tensors[0]
    //         .unwrap_f32()
    //         .unwrap()
    //         .as_slice()
    //         .unwrap()
    //         .to_vec()
    // }

    /// Repeat the search loop until the time limit is reached.
    fn search(
        root: &Board<BOARD_SIZE>,
        tree: &mut Vec<Node>,
        params: &Params,
        limits: &Limits,
        nn_policy: &Graph,
    ) {
        trace!("Engine::search(root, tree, params, limits)");

        let start_time = Instant::now();
        let mut nodes_searched = 0;
        let mut elapsed = 0;

        if tree.is_empty() {
            // create the root node
            tree.push(Node::new(Handle::null(), 0));
            let policy = Self::generate_policy(nn_policy, root, false);
            tree[0].expand(root, &policy);
        }

        // let mut log = std::io::BufWriter::new(std::fs::File::create("log.txt").unwrap());

        let mut stopped_by_stdin = false;
        while !limits.is_out_of_time(nodes_searched, elapsed) && !stopped_by_stdin {
            // perform one iteration of selection, expansion, simulation, and backpropagation
            Self::do_sesb(root, tree, params, nn_policy);

            // update elapsed time and print stats
            if nodes_searched % 1024 == 0 {
                if params.do_stdout {
                    print!(
                        "info nodes {} time {} score q {:.1} pv",
                        nodes_searched,
                        elapsed,
                        tree[0].winrate() * 100.0
                    );
                    Self::print_pv(root, tree, params);
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
    fn do_sesb(root: &Board<BOARD_SIZE>, tree: &mut Vec<Node>, params: &Params, nn_policy: &Graph) {
        trace!("Engine::do_sesb(root, tree, params)");

        // select
        let selection = Self::select(root, tree, params, 0, nn_policy);

        match selection {
            SelectionResult::NonTerminal {
                node_index: best_node,
                edge_index: edge_to_expand,
                board_state,
            } => {
                // expand
                let new_node = Self::expand(tree, params, best_node, edge_to_expand);

                // simulate
                let value = (params.valuator)(&board_state);

                // backpropagate
                Self::backpropagate(tree, new_node, value);
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
                        if p == board_state.turn() {
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
        root: &Board<BOARD_SIZE>,
        tree: &mut [Node],
        params: &Params,
        mut node_idx: usize,
        nn_policy: &Graph,
    ) -> SelectionResult {
        trace!("Engine::select(root, tree, params, node_idx = {node_idx})");

        let mut pos = *root;
        loop {
            // if the node has had a single visit, expand it
            // here, "expand" means adding all the legal moves to the node
            // with corresponding policy probabilities.
            if tree[node_idx].visits() == 1 {
                let policy = Self::generate_policy(nn_policy, &pos, false);
                tree[node_idx].expand(&pos, &policy);
            }

            // if the node is terminal, return it
            if tree[node_idx].is_terminal() {
                debug!(
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
    pub fn print_pv(root: &Board<BOARD_SIZE>, tree: &[Node], params: &Params) {
        let mut node_idx = Handle::from_index(0, tree);
        let mut pos = *root;
        while !node_idx.is_null() {
            if tree[node_idx.index()].edges().is_none() {
                break;
            }
            let (edge_idx, child_idx) = Self::uct_best(tree, params, node_idx.index());
            let edge = &tree[node_idx.index()].edges().unwrap()[edge_idx];
            let best_move = edge.get_move(false);
            print!(" {best_move}");
            pos.make_move(best_move);
            node_idx = child_idx;
        }
        println!();
    }

    /// Selects the best immediate edge of a node according to UCT.
    /// Returns the index of the edge, and a nullable handle to the child.
    fn uct_best(tree: &[Node], params: &Params, node_idx: usize) -> (usize, Handle) {
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
        let mut values = [None; BOARD_SIZE * BOARD_SIZE];
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

    /// Expands an edge of a given node, returning a handle to the new node.
    fn expand(
        tree: &mut Vec<Node>,
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
    fn backpropagate(tree: &mut [Node], mut node: Handle, mut value: f64) {
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
