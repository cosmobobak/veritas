use kn_cuda_eval::{executor::CudaExecutor, CudaDevice};
use kn_graph::{
    dtype::{DTensor, Tensor},
    graph::Graph,
    ndarray::{s, IxDyn},
};
use gomokugen::board::Board;

use crate::BOARD_SIZE;

const EXECUTOR_BATCH_SIZE: usize = 1;

pub struct ExecutorHandle {
    pub sender: crossbeam::channel::Sender<Board<BOARD_SIZE>>,
    pub receiver: crossbeam::channel::Receiver<Vec<f32>>,
}

pub struct EvalPipe {
    pub sender: crossbeam::channel::Sender<Vec<f32>>,
    pub receiver: crossbeam::channel::Receiver<Board<BOARD_SIZE>>,
}

pub struct Executor {
    pub internal: CudaExecutor,
    pub eval_pipes: Vec<EvalPipe>,
    pub in_waiting: Vec<(usize, Board<BOARD_SIZE>)>,
}

impl Executor {
    pub fn new(cuda_device: CudaDevice, num_pipes: usize, graph: &Graph) -> (Self, Vec<ExecutorHandle>) {
        let internal = CudaExecutor::new(cuda_device, graph, EXECUTOR_BATCH_SIZE);
        let mut eval_pipes = Vec::new();
        let mut handles = Vec::new();
        for _ in 0..num_pipes {
            let (board_sender, board_receiver) = crossbeam::channel::bounded(EXECUTOR_BATCH_SIZE);
            let (eval_sender, eval_receiver) = crossbeam::channel::bounded(EXECUTOR_BATCH_SIZE);
            eval_pipes.push(EvalPipe { sender: eval_sender, receiver: board_receiver });
            handles.push(ExecutorHandle { sender: board_sender, receiver: eval_receiver });
        }
        (Self { internal, eval_pipes, in_waiting: Vec::new() }, handles)
    }

    /// Fill the `in_waiting` queue with boards from the pipes.
    /// This function will block until the queue is full.
    pub fn pull(&mut self) -> Result<(), crossbeam::channel::RecvTimeoutError> {
        let mut found_anything = true;
        while found_anything && self.in_waiting.len() < EXECUTOR_BATCH_SIZE {
            found_anything = false;
            for (pipe_index, board) in self.eval_pipes.iter().enumerate() {
                if let Ok(board) = board.receiver.try_recv() {
                    self.in_waiting.push((pipe_index, board));
                    found_anything = true;
                }
            }
        }
        // if we have enough to fill the queue, return
        if self.in_waiting.len() >= EXECUTOR_BATCH_SIZE {
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
            if self.in_waiting.len() >= EXECUTOR_BATCH_SIZE {
                break Ok(());
            }
        }
    }

    pub fn tick(&mut self) {
        // take the first EXECUTOR_BATCH_SIZE elements from in_waiting,
        // evaluate them, and send the results to the corresponding pipes
        let mut indices = Vec::new();
        let mut input = Tensor::zeros(IxDyn(&[EXECUTOR_BATCH_SIZE, 162]));
        for (batch_index, (pipe_index, board)) in self.in_waiting.drain(..EXECUTOR_BATCH_SIZE).enumerate() {
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

        let tensor = tensors[0].unwrap_f32().unwrap();
        for (batch_index, pipe_index) in indices.into_iter().enumerate() {
            let vec = tensor.slice(s![batch_index, ..]).to_vec();
            self.eval_pipes[pipe_index].sender.send(vec).unwrap();
        }
    }
}

/// Starts the executor thread and returns a list of handles to the pipes.
pub fn executor(graph: &Graph) -> Vec<ExecutorHandle> {
    let cuda_device = CudaDevice::new(0).unwrap();
    let (mut executor, handles) = Executor::new(cuda_device, 1, graph);
    std::thread::Builder::new()
        .name("executor".into())
        .spawn(move || {
            loop {
                let res = executor.pull();
                if res.is_err() {
                    break;
                }
                executor.tick();
            }
        })
        .expect("Couldn't start executor thread");
    handles
}