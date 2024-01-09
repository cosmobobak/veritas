use std::sync::{atomic::{AtomicBool, Ordering}, mpsc};

use gomokugen::board::Board;

use crate::{NAME, VERSION, timemgmt::Limits, engine::{Engine, SearchResults}, params::Params};

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
static QUIT: AtomicBool = AtomicBool::new(false);

fn stdin_reader_worker(sender: mpsc::Sender<String>) {
    let mut linebuf = String::with_capacity(128);
    while let Ok(bytes) = std::io::stdin().read_line(&mut linebuf) {
        if bytes == 0 {
            // EOF
            sender.send("quit".into()).expect("couldn't send quit command to main thread");
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
    let stdin = stdin_reader();

    let version_extension = if cfg!(feature = "final-release") { "" } else { "-dev" };
    println!("{NAME} {VERSION}{version_extension} by Cosmo");

    let default_params = Params::default();
    let default_limits = Limits::default();
    let starting_position = Board::new();
    let mut engine = Engine::new(default_params, default_limits, starting_position);

    loop {
        std::io::Write::flush(&mut std::io::stdout()).expect("couldn't flush stdout");
        let Ok(line) = stdin.recv() else {
            break;
        };
        let input = line.trim();

        match input {
            "\n" | "\r\n" | "" => continue,
            "quit" => break,
            "isready" => println!("readyok"),
            "ugi" => {
                println!("id name {NAME} {VERSION}{version_extension}");
                println!("id author Cosmo");
                println!("ugiok");
            }
            go if go.starts_with("go") => {
                let limits: Limits = if let Ok(limits) = go.trim_start_matches("go").trim().parse() {
                    limits
                } else {
                    println!("info string invalid go command");
                    continue;
                };
                engine.set_limits(limits);
                let SearchResults { best_move, root_dist } = engine.go();
                println!("bestmove {best_move}");
            }
            unknown => println!("info string unknown command: {unknown}"),
        }
    }

    STDIN_READER_THREAD_KEEP_RUNNING.store(false, Ordering::SeqCst);
}