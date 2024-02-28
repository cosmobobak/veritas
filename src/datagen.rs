use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::atomic::AtomicUsize,
};

use kn_graph::{ndarray::Dimension, optimizer::OptimizerSettings};
use rand::{seq::SliceRandom, Rng as _};

use crate::{
    batching::{self, ExecutorHandle},
    engine::{Engine, SearchResults},
    game::{GameImpl, Player},
    params::Params,
};

struct GameRecord<G: GameImpl> {
    root: G,
    move_list: Vec<(G::Move, Vec<u64>)>,
    outcome: Option<Player>,
}

static GAMES_GENERATED: AtomicUsize = AtomicUsize::new(0);
static POSITIONS_GENERATED: AtomicUsize = AtomicUsize::new(0);

#[allow(clippy::too_many_lines)]
fn thread_fn<G: GameImpl>(
    time_allocated_millis: u128,
    save_folder: &str,
    thread_id: usize,
    executor: ExecutorHandle<G>,
) {
    #![allow(clippy::cast_precision_loss)]
    let start_time = std::time::Instant::now();
    let default_params = Params::default();
    let default_limits = "nodes 800".parse().unwrap();
    let starting_position = G::default();
    let mut engine = Engine::new(default_params, default_limits, &starting_position, executor);

    let mut rng = rand::thread_rng();

    let mut positions =
        BufWriter::new(File::create(format!("{save_folder}/positions-{thread_id}.csv")).unwrap());
    let mut policy_tgt = BufWriter::new(
        File::create(format!("{save_folder}/policy-target-{thread_id}.csv")).unwrap(),
    );
    let mut value_tgt = BufWriter::new(
        File::create(format!("{save_folder}/value-target-{thread_id}.csv")).unwrap(),
    );

    while start_time.elapsed().as_millis() < time_allocated_millis {
        GAMES_GENERATED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if thread_id == 0 {
            print!(
                "\rGenerated {} games at {} pos/sec",
                GAMES_GENERATED.load(std::sync::atomic::Ordering::Relaxed),
                POSITIONS_GENERATED.load(std::sync::atomic::Ordering::Relaxed) as f64
                    / start_time.elapsed().as_secs_f64()
            );
            std::io::stdout().flush().unwrap();
        }

        let mut board = G::default();
        for _ in 0..8 + rng.gen_range(0..=1) {
            let mut moves = Vec::new();
            board.generate_moves(|mv| {
                moves.push(mv);
                false
            });
            let Some(&mv) = moves.choose(&mut rng) else {
                continue;
            };
            board.make_move(mv);
        }
        let mut game = GameRecord {
            root: board.clone(),
            move_list: Vec::new(),
            outcome: None,
        };

        while board.outcome().is_none() {
            engine.set_position(&board);
            let SearchResults {
                best_move,
                root_dist,
            } = engine.go();
            assert_eq!(root_dist.len(), G::POLICY_DIM);
            board.make_move(best_move);
            game.move_list.push((best_move, root_dist));
        }

        game.outcome = board.outcome();

        let mut board = game.root;
        for (best_move, root_dist) in game.move_list {
            let ixdyn = G::tensor_dims(1);
            let mut feature_map = vec![0; ixdyn.size()];
            let to_move = board.to_move();
            board.fill_feature_map(|index| {
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
            POSITIONS_GENERATED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        positions.flush().unwrap();
        policy_tgt.flush().unwrap();
        value_tgt.flush().unwrap();
    }
    if thread_id == 0 {
        println!();
    }
}

pub fn run_data_generation<G: GameImpl>(num_threads: usize, time_allocated_millis: u128) {
    let date = chrono::Local::now().format("%Y-%m-%d-%H-%M-%S");
    let save_folder = format!("data/{date}");
    std::fs::create_dir_all(&save_folder).unwrap();

    println!("Running data generation with {num_threads} threads");
    let mut threads = Vec::new();

    // Load an onnx file into a Graph.
    let raw_graph = kn_graph::onnx::load_graph_from_onnx_path("./model.onnx", false).unwrap();
    // Optimise the graph.
    let graph = kn_graph::optimizer::optimize_graph(&raw_graph, OptimizerSettings::default());
    // Deallocate the raw graph.
    std::mem::drop(raw_graph);

    let executor_handles = batching::executor::<G>(&graph, num_threads);

    for (thread_id, executor) in executor_handles.into_iter().enumerate() {
        let save_folder = save_folder.clone();
        threads.push(std::thread::spawn(move || {
            thread_fn(time_allocated_millis, &save_folder, thread_id, executor);
        }));
    }

    for thread in threads {
        // we don't care if the thread panicked
        let _ = thread.join();
    }

    println!("Data generation complete! (saved to {save_folder})");
    println!(
        "Generated {} games.",
        GAMES_GENERATED.load(std::sync::atomic::Ordering::Relaxed)
    );
}
