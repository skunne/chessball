use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

use chessball::tournament::{TournamentConfig, run_selfplay_tournament};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut config = TournamentConfig::default();
    let mut out_dir = PathBuf::from("tournament_out");

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--games" => config.games = parse_value(&arg, args.next())?,
            "--depth" => config.depth = parse_value(&arg, args.next())?,
            "--max-plies" => config.max_plies = parse_value(&arg, args.next())?,
            "--opening-random-plies" => {
                config.opening_random_plies = parse_value(&arg, args.next())?
            }
            "--seed" => config.seed = parse_value(&arg, args.next())?,
            "--out" => {
                out_dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "missing value for --out".to_string())?,
                )
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => {
                return Err(format!("unknown argument '{other}'"));
            }
        }
    }

    fs::create_dir_all(&out_dir)
        .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;

    let (report, records) = run_selfplay_tournament(config);
    for (idx, record) in records.iter().enumerate() {
        let path = out_dir.join(format!("game_{:04}.cbr", idx + 1));
        fs::write(&path, record.to_text())
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }

    let summary_txt = format!(
        "Tournament config: games={}, depth={}, max_plies={}, opening_random_plies={}, seed={}\n{}\n",
        report.config.games,
        report.config.depth,
        report.config.max_plies,
        report.config.opening_random_plies,
        report.config.seed,
        report.stats.summary_text()
    );
    write_text(&out_dir.join("summary.txt"), &summary_txt)?;
    write_text(&out_dir.join("summary.csv"), &report.to_csv())?;

    println!("{}", report.stats.summary_text());
    println!("records written to {}", out_dir.display());
    Ok(())
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
    println!("Usage: cargo run --bin selfplay -- [options]");
    println!("  --games N                  Number of games to generate");
    println!("  --depth N                  Search depth for both sides");
    println!("  --max-plies N              Draw by adjudication after N plies");
    println!("  --opening-random-plies N   Random opening plies before search starts");
    println!("  --seed N                   Seed for opening randomization");
    println!("  --out DIR                  Output directory");
}
