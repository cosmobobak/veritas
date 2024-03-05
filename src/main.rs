#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

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
        return ugi::main_loop::<ataxxgen::Board>();
    }

    let args: Vec<_> = std::env::args_os().collect();

    match args[1].to_str().unwrap() {
        "datagen" => {
            let num_threads = args[2].to_str().unwrap().parse().unwrap();
            let time_allocated_millis = args[3].to_str().unwrap().parse().unwrap();
            let game = args.get(2).map_or("ataxx", |s| s.to_str().unwrap());
            match game {
                "ataxx" => datagen::run_data_generation::<ataxxgen::Board>(num_threads, time_allocated_millis),
                "gomoku" => datagen::run_data_generation::<gomokugen::board::Board<9>>(num_threads, time_allocated_millis),
                _ => panic!("unknown game"),
            }
        }
        "uci" => {
            let game = args.get(2).map_or("ataxx", |s| s.to_str().unwrap());
            match game {
                "ataxx" => ugi::main_loop::<ataxxgen::Board>(),
                "gomoku" => ugi::main_loop::<gomokugen::board::Board<9>>(),
                _ => panic!("unknown game"),
            }
        }
        _ => panic!("unknown subcommand"),
    }
}
