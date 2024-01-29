use std::{io::{Write, BufWriter}, fs::File};

use gomokugen::board::{Board, Move, Player};
use kn_graph::optimizer::OptimizerSettings;

use crate::{params::Params, engine::{Engine, SearchResults}, BOARD_SIZE};

struct GameRecord {
    root: Board<BOARD_SIZE>,
    move_list: Vec<(Move<BOARD_SIZE>, Vec<u64>)>,
    outcome: Option<Player>,
}

fn thread_fn(time_allocated_millis: u128, save_folder: &str, thread_id: usize) {
    // Load an onnx file into a Graph.
    let raw_graph = kn_graph::onnx::load_graph_from_onnx_path("./new-model.onnx", false).unwrap();
    // Optimise the graph.
    let graph = kn_graph::optimizer::optimize_graph(&raw_graph, OptimizerSettings::default());
    // Deallocate the raw graph.
    std::mem::drop(raw_graph);

    let default_params = Params::default();
    let default_limits = "nodes 800".parse().unwrap();
    let starting_position = Board::new();
    let mut engine = Engine::new(default_params, default_limits, &starting_position, &graph);

    let start_time = std::time::Instant::now();

    let mut positions = BufWriter::new(File::create(format!("{save_folder}/positions-{thread_id}.csv")).unwrap());
    let mut policy_tgt = BufWriter::new(File::create(format!("{save_folder}/policy-target-{thread_id}.csv")).unwrap());
    let mut value_tgt = BufWriter::new(File::create(format!("{save_folder}/value-target-{thread_id}.csv")).unwrap());

    let mut iterations = 0;
    while start_time.elapsed().as_millis() < time_allocated_millis {
        iterations += 1;

        if iterations % 128 == 0 {
            println!("Thread {thread_id} has completed {iterations} iterations");
        }

        let mut board = Board::new();
        let mut game = GameRecord {
            root: board,
            move_list: Vec::new(),
            outcome: None,
        };

        while board.outcome().is_none() {
            engine.set_position(&board);
            let SearchResults { best_move, root_dist } = engine.go();
            assert_eq!(root_dist.len(), BOARD_SIZE * BOARD_SIZE);
            board.make_move(best_move);
            game.move_list.push((best_move, root_dist));
        }

        game.outcome = board.outcome();

        let mut board = game.root;
        for (best_move, root_dist) in game.move_list {
            let mut feature_map = vec![0; 2 * BOARD_SIZE * BOARD_SIZE];
            let to_move = board.turn();
            board.feature_map(|index, side| {
                let index = index + if side == to_move { 0 } else { BOARD_SIZE * BOARD_SIZE };
                feature_map[index] = 1;
            });
            // write out the position
            for (i, f) in feature_map.iter().enumerate() {
                write!(positions, "{}", *f).unwrap();
                if i < feature_map.len() - 1 {
                    write!(positions, ",").unwrap();
                }
            }
            writeln!(positions).unwrap();
            // write out the policy target
            for (i, p) in root_dist.iter().enumerate() {
                write!(policy_tgt, "{:.3}", *p).unwrap();
                if i < root_dist.len() - 1 {
                    write!(policy_tgt, ",").unwrap();
                }
            }
            writeln!(policy_tgt).unwrap();
            // write out the value target
            let value_target = match game.outcome {
                Some(Player::None) => 0.5,
                Some(player) => {
                    if player == to_move {
                        1.0
                    } else {
                        0.0
                    }
                }
                None => unreachable!(),
            };
            writeln!(value_tgt, "{value_target}").unwrap();
            board.make_move(best_move);
        }

        positions.flush().unwrap();
        policy_tgt.flush().unwrap();
        value_tgt.flush().unwrap();
    }
}

pub fn run_data_generation(time_allocated_millis: u128) {
    let date = chrono::Local::now().format("%Y-%m-%d-%H-%M-%S");
    let save_folder = format!("data/{date}");
    std::fs::create_dir_all(&save_folder).unwrap();

    let num_threads = std::thread::available_parallelism().unwrap().get();
    println!("Running data generation with {num_threads} threads");
    let mut threads = Vec::new();

    for thread_id in 0..num_threads {
        let save_folder = save_folder.clone();
        threads.push(std::thread::spawn(move || {
            thread_fn(time_allocated_millis, &save_folder, thread_id);
        }));
    }

    for thread in threads {
        thread.join().unwrap();
    }
}