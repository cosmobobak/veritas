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

fn game_record_writer_thread<G: GameImpl>(save_folder: &str, recv: std::sync::mpsc::Receiver<GameRecord<G>>) -> anyhow::Result<()> {
    let mut positions = BufWriter::new(File::create(format!("{save_folder}/positions.csv"))?);
    let mut policy_tgt = BufWriter::new(File::create(format!("{save_folder}/policy-target.csv"))?);
    let mut value_tgt = BufWriter::new(File::create(format!("{save_folder}/value-target.csv"))?);

    for game in recv {
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
                write!(positions, "{}", *f)?;
                if i < feature_map.len() - 1 {
                    write!(positions, ",")?;
                }
            }
            writeln!(positions)?;
            // write out the policy target
            assert_eq!(root_dist.len(), G::POLICY_DIM);
            for (i, p) in root_dist.iter().enumerate() {
                write!(policy_tgt, "{:.3}", *p)?;
                if i < root_dist.len() - 1 {
                    write!(policy_tgt, ",")?;
                }
            }
            writeln!(policy_tgt)?;
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
            writeln!(value_tgt, "{value_target}")?;
            board.make_move(best_move);
            POSITIONS_GENERATED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    positions.flush()?;
    policy_tgt.flush()?;
    value_tgt.flush()?;

    Ok(())
}

#[allow(clippy::too_many_lines)]
fn self_play_worker_thread<G: GameImpl>(
    time_allocated_millis: u128,
    thread_id: usize,
    executor: ExecutorHandle<G>,
    send: std::sync::mpsc::Sender<GameRecord<G>>,
) -> anyhow::Result<()> {
    #![allow(clippy::cast_precision_loss)]
    let start_time = std::time::Instant::now();
    let default_params = Params::default();
    let default_limits = "nodes 800".parse()?;
    let starting_position = G::default();
    let mut engine = Engine::new(default_params, default_limits, &starting_position, executor);

    let mut rng = rand::thread_rng();

    while start_time.elapsed().as_millis() < time_allocated_millis {
        GAMES_GENERATED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if thread_id == 0 {
            print!(
                "\rGenerated {} games at {} pos/sec",
                GAMES_GENERATED.load(std::sync::atomic::Ordering::Relaxed),
                POSITIONS_GENERATED.load(std::sync::atomic::Ordering::Relaxed) as f64
                    / start_time.elapsed().as_secs_f64()
            );
            std::io::stdout().flush()?;
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
            root: board,
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

        if let Some(outcome) = board.outcome() {
            game.outcome = Some(outcome);
        } else {
            anyhow::bail!("Game ended without outcome in position {:?}. move sequence was {:?}", board, game.move_list);
        }

        send.send(game)?;
    }

    if thread_id == 0 {
        println!();
    }

    std::mem::drop(send);

    Ok(())
}

pub fn run_data_generation<G: GameImpl>(num_threads: usize, time_allocated_millis: u128) -> anyhow::Result<()> {
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

    let executor_handles = batching::executor::<G>(&graph, num_threads)?;

    let (send, recv) = std::sync::mpsc::channel();

    let save_folder_p = save_folder.clone();
    threads.push(std::thread::Builder::new().name("game_record_writer".to_string()).spawn(move || {
        game_record_writer_thread(&save_folder_p, recv)
    })?);

    for (thread_id, executor) in executor_handles.into_iter().enumerate() {
        let send = send.clone();
        threads.push(std::thread::Builder::new().name(format!("self_play_worker_{thread_id}")).spawn(move || {
            self_play_worker_thread(time_allocated_millis, thread_id, executor, send)
        })?);
    }

    std::mem::drop(send);

    log::trace!("Waiting for threads to finish...");
    for thread in threads {
        log::trace!("Joining {}", thread.thread().name().unwrap_or("unnamed"));
        // we don't care if the thread panicked
        let _ = thread.join();
    }

    println!("Data generation complete! (saved to {save_folder})");
    println!(
        "Generated {} games.",
        GAMES_GENERATED.load(std::sync::atomic::Ordering::Relaxed)
    );

    Ok(())
}
