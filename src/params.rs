use std::sync::{mpsc, Mutex};

use gomokugen::board::Board;

use crate::{game, BOARD_SIZE};

pub struct Params<'a> {
    pub c_puct: f64,
    pub valuator: Box<dyn Fn(&Board<BOARD_SIZE>) -> f64>,
    /// A handle to a receiver for stdin.
    pub stdin_rx: Option<&'a Mutex<mpsc::Receiver<String>>>,
    /// Whether to print search info.
    pub do_stdout: bool,
}

impl Default for Params<'_> {
    fn default() -> Self {
        Self {
            c_puct: 2.50,
            valuator: Box::new(|b| game::rollout(b).into()),
            stdin_rx: None,
            do_stdout: false,
        }
    }
}

impl<'a> Params<'a> {
    pub fn with_stdin_rx(self, stdin_rx: &'a Mutex<mpsc::Receiver<String>>) -> Self {
        Self {
            stdin_rx: Some(stdin_rx),
            ..self
        }
    }

    pub fn with_stdout(self, do_stdout: bool) -> Self {
        Self { do_stdout, ..self }
    }
}
