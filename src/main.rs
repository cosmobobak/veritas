#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

mod arena;
mod debug;
mod engine;
mod game;
mod node;
mod params;
mod timemgmt;
mod ugi;
mod datagen;

/// The name of the engine.
pub static NAME: &str = "Veritas";
/// The version of the engine.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

const BOARD_SIZE: usize = 9;

fn main() {
    #[cfg(debug_assertions)]
    std::env::set_var("RUST_BACKTRACE", "1");

    env_logger::init();

    if std::env::args_os().len() == 1 {
        // fast path to UCI:
        return ugi::main_loop();
    }

    let args: Vec<_> = std::env::args_os().collect();

    match args[1].to_str().unwrap() {
        "datagen" => {
            let time_allocated_millis = args[2].to_str().unwrap().parse().unwrap();
            datagen::run_data_generation(time_allocated_millis);
        }
        _ => panic!("unknown subcommand"),
    }
}
