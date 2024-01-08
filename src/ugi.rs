use gomokugen::board::Board;

use crate::{NAME, VERSION, timemgmt::Limits, engine::{Engine, SearchResults}, params::Params};

/// The main loop of the Universal Game Interface (UGI).
pub fn main_loop() {
    let (tx, rx) = std::sync::mpsc::channel();
    let stdin_reader_thread = std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut stdin = stdin.lock();
        let mut line = String::new();
        while std::io::BufRead::read_line(&mut stdin, &mut line).is_ok() {
            tx.send(line.clone()).unwrap();
            line.clear();
        }
    });

    let version_extension = if cfg!(feature = "final-release") { "" } else { "-dev" };
    println!("{NAME} {VERSION}{version_extension} by Cosmo");

    let default_params = Params::default();
    let default_limits = Limits::default();
    let starting_position = Board::new();
    let mut engine = Engine::new(default_params, default_limits, starting_position);

    loop {
        std::io::Write::flush(&mut std::io::stdout()).expect("couldn't flush stdout");
        let Ok(line) = rx.recv() else {
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
                println!("bestmove {}", best_move);
            }
            unknown => println!("info string unknown command: {unknown}"),
        }
    }

    stdin_reader_thread.join().expect("couldn't join stdin reader thread");
}