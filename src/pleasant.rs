use kn_graph::optimizer::OptimizerSettings;

use crate::{
    batching,
    engine::SearchResults,
    game::{GameImpl, Player},
    params::Params,
    timemgmt::Limits,
};

pub fn play_game_vs_user<G: GameImpl>(net_path: Option<&str>) -> anyhow::Result<()> {
    // Load an onnx file into a Graph.
    let raw_graph = kn_graph::onnx::load_graph_from_onnx_path(net_path.unwrap_or("./model.onnx"), false).unwrap();
    // Optimise the graph.
    let graph = kn_graph::optimizer::optimize_graph(&raw_graph, OptimizerSettings::default());
    // Deallocate the raw graph.
    std::mem::drop(raw_graph);

    let starting_position = G::default();
    // clear the screen
    print!("\x1B[2J\x1B[1;1H");
    println!("{starting_position}");

    let mut response = String::new();
    println!("Would you like to move first? (y/n)");
    std::io::stdin().read_line(&mut response).unwrap();
    let user_goes_first = response.trim().to_lowercase() == "y";
    let mut user_to_move = user_goes_first;

    let params = Params::default();
    let limits = Limits::movetime(1000);
    let executor = batching::executor(&graph, 1)?;
    let mut engine =
        crate::engine::Engine::new(params, limits, &starting_position, executor.into_iter().next().unwrap());
    let mut board = starting_position;

    loop {
        if user_to_move {
            println!("Your move:");
            let mut user_move = String::new();
            std::io::stdin().read_line(&mut user_move).unwrap();
            let user_move = user_move.trim();
            if user_move == "quit" {
                return Ok(());
            }
            if let Ok(m) = user_move.parse() {
                let mut legal = false;
                board.generate_moves(|l| {
                    if l == m {
                        legal = true;
                    }
                    legal
                });
                if legal {
                    board.make_move(m);
                    engine.set_position(&board);
                    // clear the screen
                    print!("\x1B[2J\x1B[1;1H");
                    println!("{}", engine.root());
                    user_to_move = false;
                } else {
                    println!("Illegal move: {user_move}");
                }
            } else {
                println!("Invalid move: {user_move}");
            }
        } else {
            let SearchResults { best_move, .. } = engine.go()?;
            board.make_move(best_move);
            engine.set_position(&board);
            // clear the screen
            print!("\x1B[2J\x1B[1;1H");
            println!("{}", engine.root());
            user_to_move = true;
        }

        if engine.root().outcome().is_some() {
            break;
        }
    }

    let outcome = engine.root().outcome().unwrap();
    let outcome = if user_goes_first { outcome } else { outcome.opposite() };

    match outcome {
        Player::First => println!("You win!"),
        Player::Second => println!("You lose!"),
        Player::None => println!("Draw!"),
    }

    Ok(())
}
