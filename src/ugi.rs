//! The Universal Game Interface (UGI) implementation.

use std::{
    ops::ControlFlow,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
};

use kn_graph::optimizer::OptimizerSettings;
use log::info;

use crate::{
    batching,
    engine::{Engine, SearchResults},
    game::{GameImpl, Player},
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
#[allow(clippy::too_many_lines)]
pub fn main_loop<G: GameImpl>(net_path: Option<&str>) -> anyhow::Result<()> {
    let stdin = Mutex::new(stdin_reader());

    let version_extension = if cfg!(feature = "final-release") {
        ""
    } else {
        "-dev"
    };
    println!("{NAME} {VERSION}{version_extension} by Cosmo");

    // Load an onnx file into a Graph.
    let raw_graph = kn_graph::onnx::load_graph_from_onnx_path(net_path.unwrap_or("./model.onnx"), false).unwrap();
    // Optimise the graph.
    let graph = kn_graph::optimizer::optimize_graph(&raw_graph, OptimizerSettings::default());
    // Deallocate the raw graph.
    std::mem::drop(raw_graph);

    let executor_handles = batching::executor(&graph, 1)?;

    let default_params = Params::default().with_stdin_rx(&stdin).with_stdout(true);
    let default_limits = Limits::default();
    let starting_position = G::default();
    let mut engine = Engine::new(
        default_params,
        default_limits,
        &starting_position,
        executor_handles.into_iter().next().unwrap(),
    );

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
            protocol @ ("ugi" | "uai" | "uci") => {
                println!("id name {NAME} {VERSION}{version_extension}");
                println!("id author Cosmo");
                println!("{protocol}ok");
            }
            "uginewgame" | "ucinewgame" | "uainewgame" => {
                engine.set_position(&G::default());
            }
            "show" => {
                println!("info string position fen {}", engine.root().fen());
                let board_string = engine.root().to_string();
                let prefixed = board_string
                    .lines()
                    .map(|line| format!("info string {line}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                println!("{prefixed}");
            }
            "stop" => {
                // engine.stop();
            }
            query if query.starts_with("query ") => match query.trim_start_matches("query ").trim()
            {
                "gameover" => {
                    println!("response {}", engine.root().outcome().is_some());
                }
                "p1turn" => {
                    println!("response {}", engine.root().to_move() == Player::First);
                }
                "result" => {
                    println!(
                        "response {}",
                        match engine.root().outcome() {
                            Some(Player::First) => "p1win",
                            Some(Player::Second) => "p2win",
                            Some(Player::None) => "draw",
                            None => "none",
                        }
                    );
                }
                _ => println!("response unknown query: {query}"),
            },
            go if go.starts_with("go") => {
                let limits_text = go.trim_start_matches("go").trim();
                let limits_text = G::player_substitute(limits_text);
                let limits: Limits = if let Ok(limits) = limits_text.parse()
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
                } = engine.go()?;
                info!("best move from search: {}", best_move);
                info!("root rollout distribution: {:?}", root_dist);
                println!("bestmove {best_move}");
            }
            play if play.starts_with("play ") => {
                if make_move_on_engine(play, &mut engine) == ControlFlow::Break(()) {
                    continue;
                }
            }
            set_position if set_position.starts_with("position ") => {
                if parse_position(set_position, &mut engine) == ControlFlow::Break(()) {
                    continue;
                }
            }
            set_option if set_option.starts_with("setoption ") => {
                let mut words = set_option
                    .trim_start_matches("setoption ")
                    .split_ascii_whitespace();
                words.next(); // "name"
                let Ok(name) = words.next().ok_or(()) else {
                    println!("info string invalid setoption command");
                    continue;
                };
                words.next(); // "value"
                let Ok(value) = words.next().ok_or(()) else {
                    println!("info string invalid setoption command");
                    continue;
                };
                match name {
                    "cpuct" => {
                        let Ok(cpuct) = value.parse() else {
                            println!("info string invalid cpuct value");
                            continue;
                        };
                        engine.params_mut().c_puct = cpuct;
                    }
                    _ => println!("info string unknown option: {name}"),
                }
            }
            unknown => println!("info string unknown command: {unknown}"),
        }

        if QUIT.load(Ordering::SeqCst) {
            break;
        }
    }

    STDIN_READER_THREAD_KEEP_RUNNING.store(false, Ordering::SeqCst);

    Ok(())
}

fn make_move_on_engine<G: GameImpl>(play: &str, engine: &mut Engine<'_, G>) -> ControlFlow<()> {
    let Ok(mv) = play.trim_start_matches("play ").trim().parse() else {
        println!("info string invalid move \"{play}\"");
        return ControlFlow::Break(());
    };
    let mut root = engine.root();
    let mut move_legal = false;
    root.generate_moves(|legal_mv| {
        if legal_mv == mv {
            move_legal = true;
        };
        move_legal
    });
    if !move_legal {
        println!("info string illegal move \"{mv}\"");
        return ControlFlow::Break(());
    }
    root.make_move(mv);
    engine.set_position(&root);
    ControlFlow::Continue(())
}

fn parse_position<G: GameImpl>(set_position: &str, engine: &mut Engine<'_, G>) -> ControlFlow<()> {
    let (board_part, moves_part) = set_position
        .trim_start_matches("position ")
        .trim()
        .split_once("moves")
        .map_or_else(
            || (set_position.trim_start_matches("position ").trim(), ""),
            |(board_part, moves_part)| (board_part.trim(), moves_part.trim()),
        );
    let mut board = match board_part {
        "startpos" => G::default(),
        fen if fen.starts_with("fen ") => {
            if let Ok(board) = fen.trim_start_matches("fen ").trim().parse() {
                board
            } else {
                println!("info string invalid fen \"{fen}\"");
                return ControlFlow::Break(());
            }
        }
        _ => {
            println!("info string invalid position command");
            return ControlFlow::Break(());
        }
    };
    for mv in moves_part.split_ascii_whitespace() {
        if let Ok(mv) = mv.parse() {
            board.make_move(mv);
        } else {
            println!("info string invalid move \"{mv}\"");
            continue;
        }
    }
    engine.set_position(&board);
    ControlFlow::Continue(())
}
