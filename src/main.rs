#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

mod arena;
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

const BOARD_SIZE: usize = 15;

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
