#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

use anyhow::Context;

mod arena;
mod batching;
mod datagen;
mod debug;
mod engine;
mod game;
mod node;
mod params;
mod timemgmt;
mod ugi;
mod pleasant;

/// The name of the engine.
pub static NAME: &str = "Veritas";
/// The version of the engine.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    std::env::set_var("RUST_BACKTRACE", "1");

    env_logger::init();

    if std::env::args_os().len() == 1 {
        // fast path to UCI:
        return ugi::main_loop::<ataxxgen::Board>(None);
    }

    let args: Vec<_> = std::env::args_os().collect();

    match args[1].to_str().unwrap() {
        "datagen" => {
            let game = args.get(2)
                .with_context(|| "did not find <GAME> argument!")?
                .to_str()
                .with_context(|| "invalid unicode!")?;
            let num_threads = args
                .get(3)
                .with_context(|| "did not find <NUM_THREADS> argument!")?
                .to_str()
                .with_context(|| "invalid unicode!")?
                .parse()
                .with_context(|| "num_threads")?;
            let time_allocated_millis = args
                .get(4)
                .with_context(|| "did not find <DATAGEN_MILLIS> argument!")?
                .to_str()
                .with_context(|| "invalid unicode!")?
                .parse()
                .with_context(|| "time_allocated_millis")?;
            let model_path = args.get(5).map(|s| s.to_str().unwrap());
            match game {
                "ataxx" => datagen::run_data_generation::<ataxxgen::Board>(
                    num_threads,
                    time_allocated_millis,
                    model_path,
                ),
                "gomoku9" => datagen::run_data_generation::<gomokugen::board::Board<9>>(
                    num_threads,
                    time_allocated_millis,
                    model_path,
                ),
                "gomoku15" => datagen::run_data_generation::<gomokugen::board::Board<15>>(
                    num_threads,
                    time_allocated_millis,
                    model_path,
                ),
                _ => panic!("unknown game"),
            }
        }
        "ugi" | "uai" | "uci" => {
            let game = args.get(2).map_or("ataxx", |s| s.to_str().unwrap());
            let model_path = args.get(3).map(|s| s.to_str().unwrap());
            match game {
                "ataxx" => ugi::main_loop::<ataxxgen::Board>(model_path),
                "gomoku9" => ugi::main_loop::<gomokugen::board::Board<9>>(model_path),
                "gomoku15" => ugi::main_loop::<gomokugen::board::Board<15>>(model_path),
                _ => panic!("unknown game"),
            }
        }
        "play" => {
            let game = args.get(2).map_or("ataxx", |s| s.to_str().unwrap());
            let model_path = args.get(3).map(|s| s.to_str().unwrap());
            match game {
                "ataxx" => pleasant::play_game_vs_user::<ataxxgen::Board>(model_path),
                "gomoku9" => pleasant::play_game_vs_user::<gomokugen::board::Board<9>>(model_path),
                "gomoku15" => pleasant::play_game_vs_user::<gomokugen::board::Board<15>>(model_path),
                _ => panic!("unknown game"),
            }
        }
        _ => panic!("unknown subcommand"),
    }
}
