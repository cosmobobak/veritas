#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

//! Veritas, a UGI-conformant MCTS-PUCT engine.

use engine::Engine;
use gomokugen::board::Board;
use timemgmt::Limits;

mod ugi;
mod timemgmt;
mod engine;
mod node;
mod arena;
mod params;
mod game;

/// The name of the engine.
pub static NAME: &str = "Veritas";
/// The version of the engine.
pub static VERSION: &str = env!("CARGO_PKG_VERSION");

const BOARD_SIZE: usize = 15;

fn main() {
    #[cfg(debug_assertions)]
    std::env::set_var("RUST_BACKTRACE", "1");

    // if std::env::args_os().len() == 1 {
    //     // fast path to UCI:
    //     return ugi::main_loop();
    // }

    // test engine behaviour:
    let params = params::Params {
        c_puct: 1.0,
        valuator: Box::new(|b| game::rollout(*b).into())
    };

    let mut engine = Engine::new(
        params,
        Limits::infinite(),
        Board::new(),
    );

    engine.set_limits("nodes 1000".parse().unwrap());

    let results = engine.go();

    println!("best move: {}", results.best_move);
    println!("root dist: {:?}", results.root_dist);
}
