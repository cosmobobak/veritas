use std::sync::{mpsc, Mutex};

pub struct Params<'a> {
    pub c_puct: f64,
    /// A handle to a receiver for stdin.
    pub stdin_rx: Option<&'a Mutex<mpsc::Receiver<String>>>,
    /// Whether to print search info.
    pub do_stdout: bool,
}

impl Default for Params<'_> {
    fn default() -> Self {
        Self {
            c_puct: 2.50,
            stdin_rx: None,
            do_stdout: false,
        }
    }
}

impl<'a> Params<'a> {
    pub const fn with_stdin_rx(self, stdin_rx: &'a Mutex<mpsc::Receiver<String>>) -> Self {
        Self {
            stdin_rx: Some(stdin_rx),
            ..self
        }
    }

    pub const fn with_stdout(self, do_stdout: bool) -> Self {
        Self { do_stdout, ..self }
    }
}
