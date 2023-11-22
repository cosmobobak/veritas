#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

mod ugi;
mod timemgmt;
mod engine;
mod node;
mod arena;
mod params;

/// The name of the engine.
pub static NAME: &str = "Veritas";
/// The version of the engine.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    #[cfg(debug_assertions)]
    std::env::set_var("RUST_BACKTRACE", "1");

    if std::env::args_os().len() == 1 {
        // fast path to UCI:
        return ugi::main_loop();
    }

    todo!()
}
