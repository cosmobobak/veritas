use kn_cuda_eval::{executor::CudaExecutor, CudaDevice};
use kn_graph::{
    dtype::{DTensor, Tensor},
    graph::Graph,
    ndarray::s,
};

use crate::game::GameImpl;

const EXECUTOR_BATCH_SIZE: usize = 1024;

pub struct ExecutorHandle<G: GameImpl> {
    pub sender: crossbeam::channel::Sender<G>,
    pub receiver: crossbeam::channel::Receiver<(Vec<f32>, f32)>,
}

pub struct EvalPipe<G: GameImpl> {
    pub sender: crossbeam::channel::Sender<(Vec<f32>, f32)>,
    pub receiver: crossbeam::channel::Receiver<G>,
}

pub struct Executor<G: GameImpl> {
    internal: Option<CudaExecutor>,
    eval_pipes: Vec<EvalPipe<G>>,
    in_waiting: Vec<(usize, G)>,
    batch_size: usize,
}

impl<G: GameImpl> Executor<G> {
    pub fn new(
        cuda_device: Option<CudaDevice>,
        num_pipes: usize,
        graph: &Graph,
    ) -> (Self, Vec<ExecutorHandle<G>>) {
        let batch_size = EXECUTOR_BATCH_SIZE.min(num_pipes);
        let internal = cuda_device.map(|cd| CudaExecutor::new(cd, graph, batch_size));
        let mut eval_pipes = Vec::new();
        let mut handles = Vec::new();
        for _ in 0..num_pipes {
            let (board_sender, board_receiver) = crossbeam::channel::bounded(1);
            let (eval_sender, eval_receiver) = crossbeam::channel::bounded(1);
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
        let mut input = Tensor::zeros(G::tensor_dims(self.batch_size));
        for (batch_index, (pipe_index, board)) in
            self.in_waiting.drain(..self.batch_size).enumerate()
        {
            // fill the slice with the feature map
            board.fill_feature_map(|index| {
                input[[batch_index, index]] = 1.0;
            });
            indices.push(pipe_index);
        }
        let inputs = [DTensor::F32(input)];
        let tensors = self.internal.as_mut().expect("no CUDA executor exists.").evaluate(&inputs);

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
pub fn executor<G: GameImpl>(graph: &Graph, batch_size: usize) -> anyhow::Result<Vec<ExecutorHandle<G>>> {
    #[cfg(feature = "pure-mcts")]
    let cuda_device = None;
    #[cfg(not(feature = "pure-mcts"))]
    let cuda_device = {
        let cd = CudaDevice::new(0).map_err(|_| anyhow::anyhow!("No cuda device available"))?;
        log::info!("Using device: {}", cd.name());
        Some(cd)
    };
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
    Ok(handles)
}
