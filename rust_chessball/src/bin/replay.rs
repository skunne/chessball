use std::{env, fs, process};

use chessball::record::GameRecord;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path = match args.next() {
        Some(arg) if arg == "--help" || arg == "-h" => {
            print_help();
            return Ok(());
        }
        Some(path) => path,
        None => {
            print_help();
            return Err("missing record path".to_string());
        }
    };

    let content =
        fs::read_to_string(&path).map_err(|err| format!("failed to read {path}: {err}"))?;
    let record = GameRecord::from_text(&content)?;
    let positions = record.replay_positions()?;

    println!("White: {}", record.white_label);
    println!("Black: {}", record.black_label);
    println!("Result: {}", record.outcome);
    println!("Termination: {}", record.termination);
    println!();
    println!("Initial position:");
    println!("{}", positions[0]);
    for (idx, ply) in record.moves.iter().enumerate() {
        println!(
            "Ply {}: {} {}",
            idx + 1,
            chessball::record::move_to_notation(ply.mv),
            ply.source.as_str()
        );
        println!("{}", positions[idx + 1]);
    }
    Ok(())
}

fn print_help() {
    println!("Usage: cargo run --bin replay -- <record.cbr>");
}
