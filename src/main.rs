#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

mod ugi;
mod timemgmt;
mod engine;
mod node;
mod arena;
mod params;
mod game;
mod debug;

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

    unimplemented!();
}