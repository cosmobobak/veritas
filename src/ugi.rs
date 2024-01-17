//! The Universal Game Interface (UGI) implementation.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Mutex,
};

use gomokugen::board::Board;
use kn_graph::optimizer::OptimizerSettings;
use log::info;

use crate::{
    engine::{Engine, SearchResults},
    params::Params,
    timemgmt::Limits,
    NAME, VERSION,
};

fn stdin_reader() -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("stdin-reader".into())
        .spawn(|| stdin_reader_worker(sender))
        .expect("Couldn't start stdin reader worker thread");
    receiver
}

/// Whether the stdin reader thread should keep running.
static STDIN_READER_THREAD_KEEP_RUNNING: AtomicBool = AtomicBool::new(true);
/// Whether the main thread should keep running.
pub static QUIT: AtomicBool = AtomicBool::new(false);

fn stdin_reader_worker(sender: mpsc::Sender<String>) {
    let mut linebuf = String::with_capacity(128);
    while let Ok(bytes) = std::io::stdin().read_line(&mut linebuf) {
        if bytes == 0 {
            // EOF
            sender
                .send("quit".into())
                .expect("couldn't send quit command to main thread");
            QUIT.store(true, Ordering::SeqCst);
            break;
        }
        let cmd = linebuf.trim();
        if cmd.is_empty() {
            linebuf.clear();
            continue;
        }
        if let Err(e) = sender.send(cmd.to_owned()) {
            eprintln!("info string error sending command to main thread: {e}");
            break;
        }
        if !STDIN_READER_THREAD_KEEP_RUNNING.load(Ordering::SeqCst) {
            break;
        }
        linebuf.clear();
    }
    std::mem::drop(sender);
}

/// The main loop of the Universal Game Interface (UGI).
pub fn main_loop() {
    let stdin = Mutex::new(stdin_reader());

    let version_extension = if cfg!(feature = "final-release") {
        ""
    } else {
        "-dev"
    };
    println!("{NAME} {VERSION}{version_extension} by Cosmo");

    // Load an onnx file into a Graph.
    let raw_graph = kn_graph::onnx::load_graph_from_onnx_path("./model.onnx", false).unwrap();
    // Optimise the graph.
    let graph = kn_graph::optimizer::optimize_graph(&raw_graph, OptimizerSettings::default());
    // Deallocate the raw graph.
    std::mem::drop(raw_graph);

    let default_params = Params::default().with_stdin_rx(&stdin);
    let default_limits = Limits::default();
    let starting_position = Board::new();
    let mut engine = Engine::new(default_params, default_limits, &starting_position, &graph);

    loop {
        std::io::Write::flush(&mut std::io::stdout()).expect("couldn't flush stdout");
        let Ok(line) = stdin.lock().expect("failed to take lock on stdin").recv() else {
            break;
        };
        let input = line.trim();

        match input {
            "\n" | "\r\n" | "" => continue,
            "quit" => {
                QUIT.store(true, Ordering::SeqCst);
                break;
            }
            "isready" => println!("readyok"),
            "ugi" => {
                println!("id name {NAME} {VERSION}{version_extension}");
                println!("id author Cosmo");
                println!("ugiok");
            }
            "show" => {
                println!("info string position fen {}", engine.root().fen());
            }
            go if go.starts_with("go") => {
                let limits: Limits = if let Ok(limits) = go.trim_start_matches("go").trim().parse()
                {
                    limits
                } else {
                    println!("info string invalid go command");
                    continue;
                };
                engine.set_limits(limits);
                let SearchResults {
                    best_move,
                    root_dist,
                } = engine.go();
                info!("best move from search: {}", best_move);
                info!("root rollout distribution: {:?}", root_dist);
                println!("bestmove {best_move}");
            }
            "stop" => {
                // engine.stop();
            }
            set_position if set_position.starts_with("position fen ") => {
                let fen = set_position.trim_start_matches("position fen ").trim();
                let board = match fen.parse() {
                    Ok(board) => board,
                    Err(e) => {
                        println!("info string invalid fen \"{fen}\": {e}");
                        continue;
                    }
                };
                engine.set_position(&board);
            }
            unknown => println!("info string unknown command: {unknown}"),
        }

        if QUIT.load(Ordering::SeqCst) {
            break;
        }
    }

    STDIN_READER_THREAD_KEEP_RUNNING.store(false, Ordering::SeqCst);
}
