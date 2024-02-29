#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

use gomokugen::board::Board;

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

fn main() {
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
            datagen::run_data_generation::<Board<9>>(num_threads, time_allocated_millis);
        }
        _ => panic!("unknown subcommand"),
    }
}
