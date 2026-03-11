use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

use chessball::{
    agent::{Agent, ClassicalConfig, ClassicalEngine},
    alphazero::{AlphaZeroConfig, AlphaZeroEngine},
    arena::{MatchConfig, MatchStats, play_game},
};

#[derive(Debug, Clone)]
struct CliConfig {
    games: usize,
    white: String,
    black: String,
    classical_depth: u8,
    az_simulations: usize,
    az_cpuct: f32,
    az_temperature: f32,
    az_root_noise: f32,
    az_train_games: usize,
    az_train_iterations: usize,
    az_train_max_plies: usize,
    az_replay_capacity: usize,
    az_temperature_drop_ply: usize,
    az_post_game_self_play_games: usize,
    white_checkpoint_in: Option<PathBuf>,
    white_checkpoint_out: Option<PathBuf>,
    black_checkpoint_in: Option<PathBuf>,
    black_checkpoint_out: Option<PathBuf>,
    max_plies: usize,
    opening_random_plies: usize,
    seed: u64,
    out_dir: PathBuf,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            games: 20,
            white: "classical".to_string(),
            black: "alphazero".to_string(),
            classical_depth: 4,
            az_simulations: 96,
            az_cpuct: 1.35,
            az_temperature: 1.0,
            az_root_noise: 0.25,
            az_train_games: 32,
            az_train_iterations: 2,
            az_train_max_plies: 160,
            az_replay_capacity: 4096,
            az_temperature_drop_ply: 12,
            az_post_game_self_play_games: 0,
            white_checkpoint_in: None,
            white_checkpoint_out: None,
            black_checkpoint_in: None,
            black_checkpoint_out: None,
            max_plies: 200,
            opening_random_plies: 0,
            seed: 1,
            out_dir: PathBuf::from("arena_out"),
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = parse_args()?;
    fs::create_dir_all(&config.out_dir)
        .map_err(|err| format!("failed to create {}: {err}", config.out_dir.display()))?;

    let white_checkpoints = resolve_checkpoint_paths(
        "white",
        &config.white,
        config.white_checkpoint_in.clone(),
        config.white_checkpoint_out.clone(),
        &config.out_dir,
    )?;
    let black_checkpoints = resolve_checkpoint_paths(
        "black",
        &config.black,
        config.black_checkpoint_in.clone(),
        config.black_checkpoint_out.clone(),
        &config.out_dir,
    )?;

    let mut white = build_agent(
        &config.white,
        &config,
        config.seed ^ 0x1357_2468_ACE0_FDB9,
        white_checkpoints.load.as_deref(),
        white_checkpoints.load_is_explicit,
    )?;
    let mut black = build_agent(
        &config.black,
        &config,
        config.seed ^ 0xCAFEBABE_D15EA5E5,
        black_checkpoints.load.as_deref(),
        black_checkpoints.load_is_explicit,
    )?;
    let white_label = white.label();
    let black_label = black.label();

    let mut stats = MatchStats::default();
    let mut summary_csv = String::from("game,result,termination,plies,total_nodes\n");

    for game_index in 0..config.games {
        let game_seed = config.seed ^ (game_index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let record = play_game(
            &mut *white,
            &mut *black,
            MatchConfig {
                max_plies: config.max_plies,
                opening_random_plies: config.opening_random_plies,
                seed: game_seed,
            },
        );
        let total_nodes = record.moves.iter().filter_map(|ply| ply.nodes).sum::<u64>();
        summary_csv.push_str(&format!(
            "{},{},{},{},{}\n",
            game_index + 1,
            record.outcome.as_result_str(),
            record.termination.as_str(),
            record.moves.len(),
            total_nodes
        ));
        stats.absorb(&record);

        let path = config
            .out_dir
            .join(format!("game_{:04}.cbr", game_index + 1));
        fs::write(&path, record.to_text())
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }

    if let Some(path) = white_checkpoints.save.as_deref() {
        white.save_checkpoint(path)?;
    }
    if let Some(path) = black_checkpoints.save.as_deref() {
        black.save_checkpoint(path)?;
    }

    let summary_txt = format!(
        "White: {white_label}\nBlack: {black_label}\nGames: {}\nMax plies: {}\nOpening random plies: {}\nSeed: {}\n{}\n",
        config.games,
        config.max_plies,
        config.opening_random_plies,
        config.seed,
        stats.summary_text()
    );
    write_text(&config.out_dir.join("summary.txt"), &summary_txt)?;
    write_text(&config.out_dir.join("summary.csv"), &summary_csv)?;

    println!("White: {white_label}");
    println!("Black: {black_label}");
    println!("{}", stats.summary_text());
    println!("records written to {}", config.out_dir.display());
    Ok(())
}

fn parse_args() -> Result<CliConfig, String> {
    let mut config = CliConfig::default();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--games" => config.games = parse_value(&arg, args.next())?,
            "--white" => {
                config.white = args
                    .next()
                    .ok_or_else(|| "missing value for --white".to_string())?
            }
            "--black" => {
                config.black = args
                    .next()
                    .ok_or_else(|| "missing value for --black".to_string())?
            }
            "--classical-depth" => config.classical_depth = parse_value(&arg, args.next())?,
            "--az-simulations" => config.az_simulations = parse_value(&arg, args.next())?,
            "--az-cpuct" => config.az_cpuct = parse_value(&arg, args.next())?,
            "--az-temperature" => config.az_temperature = parse_value(&arg, args.next())?,
            "--az-root-noise" => config.az_root_noise = parse_value(&arg, args.next())?,
            "--az-train-games" => config.az_train_games = parse_value(&arg, args.next())?,
            "--az-train-iterations" => config.az_train_iterations = parse_value(&arg, args.next())?,
            "--az-train-max-plies" => config.az_train_max_plies = parse_value(&arg, args.next())?,
            "--az-replay-capacity" => config.az_replay_capacity = parse_value(&arg, args.next())?,
            "--az-temperature-drop-ply" => {
                config.az_temperature_drop_ply = parse_value(&arg, args.next())?
            }
            "--az-post-game-self-play-games" => {
                config.az_post_game_self_play_games = parse_value(&arg, args.next())?
            }
            "--white-checkpoint-in" => {
                config.white_checkpoint_in =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        "missing value for --white-checkpoint-in".to_string()
                    })?))
            }
            "--white-checkpoint-out" => {
                config.white_checkpoint_out =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        "missing value for --white-checkpoint-out".to_string()
                    })?))
            }
            "--black-checkpoint-in" => {
                config.black_checkpoint_in =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        "missing value for --black-checkpoint-in".to_string()
                    })?))
            }
            "--black-checkpoint-out" => {
                config.black_checkpoint_out =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        "missing value for --black-checkpoint-out".to_string()
                    })?))
            }
            "--max-plies" => config.max_plies = parse_value(&arg, args.next())?,
            "--opening-random-plies" => {
                config.opening_random_plies = parse_value(&arg, args.next())?
            }
            "--seed" => config.seed = parse_value(&arg, args.next())?,
            "--out" => {
                config.out_dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "missing value for --out".to_string())?,
                )
            }
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    Ok(config)
}

fn build_agent(
    kind: &str,
    config: &CliConfig,
    seed: u64,
    checkpoint_in: Option<&Path>,
    checkpoint_in_is_explicit: bool,
) -> Result<Box<dyn Agent>, String> {
    match kind {
        "classical" => Ok(Box::new(ClassicalEngine::new(ClassicalConfig {
            depth: config.classical_depth,
        }))),
        "alphazero" => {
            let az_config = AlphaZeroConfig {
                simulations: config.az_simulations,
                cpuct: config.az_cpuct,
                temperature: config.az_temperature,
                root_noise: config.az_root_noise,
                train_games: config.az_train_games,
                train_iterations: config.az_train_iterations,
                train_max_plies: config.az_train_max_plies,
                replay_capacity: config.az_replay_capacity,
                temperature_drop_ply: config.az_temperature_drop_ply,
                post_game_self_play_games: config.az_post_game_self_play_games,
                seed,
            };

            let engine = if let Some(path) = checkpoint_in {
                if path.exists() {
                    AlphaZeroEngine::from_checkpoint(path, az_config)?
                } else if checkpoint_in_is_explicit {
                    return Err(format!("checkpoint {} does not exist", path.display()));
                } else {
                    AlphaZeroEngine::new(az_config)
                }
            } else {
                AlphaZeroEngine::new(az_config)
            };
            Ok(Box::new(engine))
        }
        other => Err(format!(
            "unknown engine '{other}', expected 'classical' or 'alphazero'"
        )),
    }
}

#[derive(Debug, Clone)]
struct CheckpointPaths {
    load: Option<PathBuf>,
    save: Option<PathBuf>,
    load_is_explicit: bool,
}

fn resolve_checkpoint_paths(
    side: &str,
    kind: &str,
    explicit_in: Option<PathBuf>,
    explicit_out: Option<PathBuf>,
    out_dir: &Path,
) -> Result<CheckpointPaths, String> {
    if kind != "alphazero" {
        if explicit_in.is_some() || explicit_out.is_some() {
            return Err(format!(
                "{side} checkpoint paths are only valid for the alphazero engine"
            ));
        }
        return Ok(CheckpointPaths {
            load: None,
            save: None,
            load_is_explicit: false,
        });
    }

    let default_path = out_dir.join(format!("{side}_alphazero.azckpt"));
    let save = explicit_out
        .or_else(|| explicit_in.clone())
        .or(Some(default_path));
    let load_is_explicit = explicit_in.is_some();
    let load = if let Some(path) = explicit_in {
        Some(path)
    } else if let Some(path) = save.clone() {
        path.exists().then_some(path)
    } else {
        None
    };

    Ok(CheckpointPaths {
        load,
        save,
        load_is_explicit,
    })
}

fn parse_value<T: std::str::FromStr>(flag: &str, value: Option<String>) -> Result<T, String> {
    let raw = value.ok_or_else(|| format!("missing value for {flag}"))?;
    raw.parse::<T>()
        .map_err(|_| format!("invalid value '{raw}' for {flag}"))
}

fn write_text(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn print_help() {
    println!("Usage: cargo run --bin arena -- [options]");
    println!("  --games N                  Number of games to play");
    println!("  --white classical|alphazero");
    println!("  --black classical|alphazero");
    println!("  --classical-depth N        Alpha-beta depth for the classical engine");
    println!("  --az-simulations N         MCTS simulations per move");
    println!("  --az-cpuct F               PUCT exploration constant");
    println!("  --az-temperature F         Self-play sampling temperature");
    println!("  --az-root-noise F          Root prior noise mixed during self-play");
    println!("  --az-train-games N         Self-play games used to pretrain each AlphaZero engine");
    println!("  --az-train-iterations N    Number of self-play pretraining rounds");
    println!("  --az-train-max-plies N     Ply cap while pretraining");
    println!("  --az-replay-capacity N     Max retained training positions (0 = unlimited)");
    println!(
        "  --az-temperature-drop-ply N  Self-play ply after which move choice turns deterministic"
    );
    println!(
        "  --az-post-game-self-play-games N  Extra self-play games after each completed arena game"
    );
    println!("  --white-checkpoint-in FILE  Load White AlphaZero state from FILE");
    println!("  --white-checkpoint-out FILE Save White AlphaZero state to FILE");
    println!("  --black-checkpoint-in FILE  Load Black AlphaZero state from FILE");
    println!("  --black-checkpoint-out FILE Save Black AlphaZero state to FILE");
    println!("  --max-plies N              Adjudicate drawn games after N plies");
    println!("  --opening-random-plies N   Random opening plies before engines take over");
    println!("  --seed N                   Base seed");
    println!("  --out DIR                  Output directory");
}
