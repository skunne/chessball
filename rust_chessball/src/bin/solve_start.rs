use std::{env, process, time::Instant};

use chessball::{
    record::move_to_notation,
    weak_solve::{EdgeStorageMode, WeakSolveConfig, solve_start},
};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut max_states = None;
    let mut line_plies = 20usize;
    let mut edge_storage = EdgeStorageMode::Memory;
    let mut checkpoint_states = None;
    let mut checkpoint_seconds = None;
    let mut solve_only = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--max-states" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --max-states".to_string())?;
                max_states = Some(
                    raw.parse::<usize>()
                        .map_err(|_| format!("invalid value '{raw}' for --max-states"))?,
                );
            }
            "--line-plies" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --line-plies".to_string())?;
                line_plies = raw
                    .parse::<usize>()
                    .map_err(|_| format!("invalid value '{raw}' for --line-plies"))?;
            }
            "--checkpoint-states" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --checkpoint-states".to_string())?;
                checkpoint_states = Some(parse_positive_usize(&raw, "--checkpoint-states")?);
            }
            "--checkpoint-seconds" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "missing value for --checkpoint-seconds".to_string())?;
                checkpoint_seconds = Some(parse_positive_u64(&raw, "--checkpoint-seconds")?);
            }
            "--disk-edges" => {
                edge_storage = EdgeStorageMode::Disk;
            }
            "--solve-only" => {
                solve_only = true;
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    let started = Instant::now();
    let result = solve_start(WeakSolveConfig {
        max_states,
        edge_storage,
        checkpoint_states,
        checkpoint_seconds,
    });
    let elapsed = started.elapsed();

    println!("exact={}", result.exact);
    println!("elapsed_ms={}", elapsed.as_millis());
    println!(
        "states={}, expanded_states={}, closed_states={}, certified_states={}, certified_white_wins={}, certified_black_wins={}, certified_draws={}, certified_unknown_states={}, draw_candidate_seed_states={}, draw_candidate_states={}, draw_candidate_sccs={}, cyclic_draw_candidate_sccs={}, cyclic_draw_candidate_states={}, largest_draw_candidate_scc={}, largest_cyclic_draw_scc={}, draw_prune_iterations={}, draw_prune_removed_mover_win_exit={}, draw_prune_removed_open_or_unknown_exit={}, draw_prune_removed_no_draw_successor={}, revisited_child_edges={}, self_loop_edges={}, edges={}, max_successors_per_state={}, terminal_white_wins={}, terminal_black_wins={}, sink_draws={}, resident_storage_bytes={}, disk_edge_bytes={}, state_table_peak_bytes={}, edge_storage={:?}, truncated={}",
        result.stats.states,
        result.stats.expanded_states,
        result.stats.closed_states,
        result.stats.certified_states,
        result.stats.certified_white_wins,
        result.stats.certified_black_wins,
        result.stats.certified_draws,
        result.stats.certified_unknown_states,
        result.stats.draw_candidate_seed_states,
        result.stats.draw_candidate_states,
        result.stats.draw_candidate_sccs,
        result.stats.cyclic_draw_candidate_sccs,
        result.stats.cyclic_draw_candidate_states,
        result.stats.largest_draw_candidate_scc,
        result.stats.largest_cyclic_draw_scc,
        result.stats.draw_prune_iterations,
        result.stats.draw_prune_removed_mover_win_exit,
        result.stats.draw_prune_removed_open_or_unknown_exit,
        result.stats.draw_prune_removed_no_draw_successor,
        result.stats.revisited_child_edges,
        result.stats.self_loop_edges,
        result.stats.edges,
        result.stats.max_successors_per_state,
        result.stats.terminal_white_wins,
        result.stats.terminal_black_wins,
        result.stats.sink_draws,
        result.stats.resident_storage_bytes,
        result.stats.disk_edge_bytes,
        result.stats.state_table_peak_bytes,
        result.graph.edge_storage_mode(),
        result.stats.truncated
    );
    println!("proof_model=infinite_play_is_draw");

    if result.exact {
        println!("start_outcome={}", result.start_outcome().as_str());
        if !solve_only && line_plies > 0 {
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
    } else {
        println!(
            "partial_start_outcome={}",
            result.hinted_start_outcome().as_str()
        );
        match result.certified_start_outcome() {
            Some(outcome) => {
                println!("certified_start_outcome={}", outcome.as_str());
                println!(
                    "graph was truncated, but the start position is certified on the explored closed subgraph"
                );
                if !solve_only
                    && line_plies > 0
                    && let Some(line) = result.certified_line_from_start(line_plies)
                {
                    println!("\ncertified_line_from_start:");
                    for (idx, (position, outcome, mv)) in line.into_iter().enumerate() {
                        println!("state {} outcome={}", idx, outcome.as_str());
                        println!("{position}");
                        if let Some(mv) = mv {
                            println!("move {}\n", move_to_notation(mv));
                        } else {
                            println!("move <none>\n");
                        }
                    }
                }
            }
            None => {
                println!("certified_start_outcome=<none>");
                println!("graph was truncated; partial_start_outcome is only a hint, not a proof");
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!("Usage: cargo run --bin solve_start -- [options]");
    println!("  --max-states N   Stop graph expansion after N discovered states");
    println!("  --line-plies N   Print up to N plies if the start position is exact or certified");
    println!("  --disk-edges     Spill successor edges to a temporary binary file");
    println!("  --solve-only     Print only compact solve statistics and start verdicts");
    println!("  --checkpoint-states N   Print checkpoint lines every N processed units per phase");
    println!("  --checkpoint-seconds N  Print checkpoint lines at least every N seconds per phase");
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

fn parse_positive_u64(raw: &str, flag: &str) -> Result<u64, String> {
    let value = raw
        .parse::<u64>()
        .map_err(|_| format!("invalid value '{raw}' for {flag}"))?;
    if value == 0 {
        return Err(format!("{flag} must be greater than 0"));
    }
    Ok(value)
}
