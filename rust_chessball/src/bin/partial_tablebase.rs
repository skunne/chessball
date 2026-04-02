use std::{env, path::PathBuf, process};

use chessball::{
    partial_tablebase::{
        ExportConfig, PartialTablebaseConfig, PathSelectionConfig, build_start, export_to_dir,
    },
    record::move_to_notation,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut max_states = None;
    let mut out_dir = PathBuf::from("partial_tablebase_out");
    let mut dot_max_nodes = 200usize;
    let mut dot_max_depth = 6usize;
    let mut certified_per_outcome = 5usize;
    let mut min_proof_plies = 0usize;
    let mut prefer_long_proofs = false;
    let mut line_plies = 12usize;
    let mut export_full_graph = true;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--max-states" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --max-states".to_string())?;
                max_states = Some(parse_positive_usize(&raw, "--max-states")?);
            }
            "--out" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --out".to_string())?;
                out_dir = PathBuf::from(raw);
            }
            "--dot-max-nodes" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --dot-max-nodes".to_string())?;
                dot_max_nodes = parse_positive_usize(&raw, "--dot-max-nodes")?;
            }
            "--dot-max-depth" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --dot-max-depth".to_string())?;
                dot_max_depth = parse_positive_usize(&raw, "--dot-max-depth")?;
            }
            "--certified-per-outcome" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --certified-per-outcome".to_string())?;
                certified_per_outcome = parse_positive_usize(&raw, "--certified-per-outcome")?;
            }
            "--min-proof-plies" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --min-proof-plies".to_string())?;
                min_proof_plies = raw
                    .parse::<usize>()
                    .map_err(|_| format!("invalid value '{raw}' for --min-proof-plies"))?;
            }
            "--prefer-long-proofs" => {
                prefer_long_proofs = true;
            }
            "--proof-only" => {
                export_full_graph = false;
            }
            "--line-plies" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --line-plies".to_string())?;
                line_plies = raw
                    .parse::<usize>()
                    .map_err(|_| format!("invalid value '{raw}' for --line-plies"))?;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    let result = build_start(PartialTablebaseConfig { max_states });
    export_to_dir(
        &result,
        &out_dir,
        ExportConfig {
            dot_max_nodes,
            dot_max_depth,
            certified_per_outcome,
            min_proof_plies,
            prefer_long_proofs,
            export_full_graph,
        },
    )
    .map_err(|err| format!("failed to export partial tablebase: {err}"))?;
    let path_selection = PathSelectionConfig {
        limit_per_outcome: certified_per_outcome,
        min_proof_plies,
        prefer_long_proofs,
    };
    let certified_paths = result.proven_paths_with_config(path_selection);
    let certified_entries = result.certified_entry_paths_with_config(path_selection);

    println!("exact={}", result.exact);
    println!(
        "states={}, closed_states={}, open_states={}, edges={}, max_successors_per_state={}",
        result.stats.states,
        result.stats.closed_states,
        result.stats.open_states,
        result.stats.edges,
        result.stats.max_successors_per_state
    );
    println!(
        "proven_white_wins={}, proven_black_wins={}, proven_draws={}, unknown_states={}",
        result.stats.proven_white_wins,
        result.stats.proven_black_wins,
        result.stats.proven_draws,
        result.stats.unknown_states
    );
    println!("start_outcome={}", result.start_outcome().as_str());
    println!("proven_paths={}", certified_paths.len());
    println!("certified_entry_paths={}", certified_entries.len());
    println!("export_full_graph={}", export_full_graph);
    println!("out_dir={}", out_dir.display());

    if !certified_paths.is_empty() {
        println!("\nproven_paths:");
        for path in &certified_paths {
            let moves = path
                .move_sequence()
                .into_iter()
                .map(move_to_notation)
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "state={} depth={} proof_plies={} outcome={} sequence={}",
                path.target_state,
                path.depth,
                path.proof_plies,
                path.outcome.as_str(),
                if moves.is_empty() { "<none>" } else { &moves }
            );
        }
    }

    if line_plies > 0 {
        println!("\nline_from_start:");
        for (idx, (position, outcome, mv)) in
            result.line_from_start(line_plies).into_iter().enumerate()
        {
            println!("state {} outcome={}", idx, outcome.as_str());
            println!("{position}");
            if let Some(mv) = mv {
                println!("move {}\n", move_to_notation(mv));
            } else {
                println!("move <none>\n");
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!("Usage: cargo run --bin partial_tablebase -- [options]");
    println!("  --max-states N     Discover at most N canonical states");
    println!(
        "  --out DIR          Write summary.txt, states.csv, edges.csv, graph.dot, certified_paths.*, and proof_paths.dot"
    );
    println!("  --dot-max-nodes N  Limit DOT export breadth from the start state");
    println!("  --dot-max-depth N  Limit DOT export depth from the start state");
    println!("  --certified-per-outcome N  Export up to N shortest proven paths per outcome");
    println!(
        "  --min-proof-plies N  Keep only paths whose forcing continuation from the solved state lasts at least N plies"
    );
    println!(
        "  --prefer-long-proofs  Prefer longer forcing continuations when choosing exported paths"
    );
    println!(
        "  --proof-only       Skip states.csv, edges.csv, and graph.dot; keep proof-oriented exports"
    );
    println!("  --line-plies N     Print up to N certified plies from the start");
}

fn parse_positive_usize(raw: &str, flag: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|_| format!("invalid value '{raw}' for {flag}"))?;
    if value == 0 {
        return Err(format!("{flag} must be greater than 0"));
    }
    Ok(value)
}
