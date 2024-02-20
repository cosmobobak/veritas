use gomokugen::board::Board;
use kn_cuda_eval::{executor::CudaExecutor, CudaDevice};
use kn_graph::{
    dtype::{DTensor, Tensor},
    graph::Graph,
    ndarray::{s, IxDyn},
};

use crate::BOARD_SIZE;

const EXECUTOR_BATCH_SIZE: usize = 1024;

pub struct ExecutorHandle {
    pub sender: crossbeam::channel::Sender<Board<BOARD_SIZE>>,
    pub receiver: crossbeam::channel::Receiver<(Vec<f32>, f32)>,
}

pub struct EvalPipe {
    pub sender: crossbeam::channel::Sender<(Vec<f32>, f32)>,
    pub receiver: crossbeam::channel::Receiver<Board<BOARD_SIZE>>,
}

pub struct Executor {
    internal: CudaExecutor,
    eval_pipes: Vec<EvalPipe>,
    in_waiting: Vec<(usize, Board<BOARD_SIZE>)>,
    batch_size: usize,
}

impl Executor {
    pub fn new(
        cuda_device: CudaDevice,
        num_pipes: usize,
        graph: &Graph,
    ) -> (Self, Vec<ExecutorHandle>) {
        let batch_size = EXECUTOR_BATCH_SIZE.min(num_pipes);
        let internal = CudaExecutor::new(cuda_device, graph, batch_size);
        let mut eval_pipes = Vec::new();
        let mut handles = Vec::new();
        for _ in 0..num_pipes {
            let (board_sender, board_receiver) = crossbeam::channel::bounded(batch_size);
            let (eval_sender, eval_receiver) = crossbeam::channel::bounded(batch_size);
            eval_pipes.push(EvalPipe {
                sender: eval_sender,
                receiver: board_receiver,
            });
            handles.push(ExecutorHandle {
                sender: board_sender,
                receiver: eval_receiver,
            });
        }
        (
            Self {
                internal,
                eval_pipes,
                in_waiting: Vec::new(),
                batch_size,
            },
            handles,
        )
    }

    /// Fill the `in_waiting` queue with boards from the pipes.
    /// This function will block until the queue is full.
    pub fn pull(&mut self) -> Result<(), crossbeam::channel::RecvTimeoutError> {
        let mut found_anything = true;
        while found_anything && self.in_waiting.len() < self.batch_size {
            found_anything = false;
            for (pipe_index, board) in self.eval_pipes.iter().enumerate() {
                if let Ok(board) = board.receiver.try_recv() {
                    self.in_waiting.push((pipe_index, board));
                    found_anything = true;
                }
            }
        }
        // if we have enough to fill the queue, return
        if self.in_waiting.len() >= self.batch_size {
            return Ok(());
        }
        // otherwise, block until we have enough
        let mut select = crossbeam::channel::Select::new();
        for pipe in &self.eval_pipes {
            select.recv(&pipe.receiver);
        }
        loop {
            let oper = select.select();
            let index = oper.index();
            let board = oper.recv(&self.eval_pipes[index].receiver)?;
            self.in_waiting.push((index, board));
            if self.in_waiting.len() >= self.batch_size {
                break Ok(());
            }
        }
    }

    pub fn tick(&mut self) {
        // take the first EXECUTOR_BATCH_SIZE elements from in_waiting,
        // evaluate them, and send the results to the corresponding pipes
        let mut indices = Vec::new();
        let mut input = Tensor::zeros(IxDyn(&[self.batch_size, 162]));
        for (batch_index, (pipe_index, board)) in
            self.in_waiting.drain(..self.batch_size).enumerate()
        {
            // TODO: this is really tightly coupled to the board representation
            // and should be abstracted away.
            let to_move = board.turn();
            board.feature_map(|i, c| {
                let index = i + usize::from(c != to_move) * 81;
                input[[batch_index, index]] = 1.0;
            });
            indices.push(pipe_index);
        }
        let inputs = [DTensor::F32(input)];
        let tensors = self.internal.evaluate(&inputs);

        let policy = tensors[0].unwrap_f32().unwrap();
        let value = tensors[1].unwrap_f32().unwrap();
        for (batch_index, pipe_index) in indices.into_iter().enumerate() {
            let policy_vec = policy.slice(s![batch_index, ..]).to_vec();
            let value = value[[batch_index, 0]];
            self.eval_pipes[pipe_index]
                .sender
                .send((policy_vec, value))
                .unwrap();
        }
    }
}

/// Starts the executor thread and returns a list of handles to the pipes.
pub fn executor(graph: &Graph, batch_size: usize) -> Vec<ExecutorHandle> {
    let cuda_device = CudaDevice::new(0).unwrap();
    println!("Using device: {}", cuda_device.name());
    let (mut executor, handles) = Executor::new(cuda_device, batch_size, graph);
    std::thread::Builder::new()
        .name("executor".into())
        .spawn(move || loop {
            let res = executor.pull();
            if res.is_err() {
                break;
            }
            executor.tick();
            log::debug!("Batch of evaluations completed.");
        })
        .expect("Couldn't start executor thread");
    handles
}
