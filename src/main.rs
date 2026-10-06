//! Standalone CLI for running the `hgs_advance` HGS-VRPTW/CVRP solver on
//! classical benchmark instances (Solomon/Gehring-Homberger `.txt` VRPTW files,
//! or CVRPLib/TSPLIB-style `.vrp` CVRP files).
//!
//! Example:
//!   hgs_advance --instance C101.txt --time-limit 60 --output-dir results/

mod compression;
mod constructive;
mod genetic;
mod hierarchical;
mod individual;
mod local_search;
mod loader_cvrp;
mod loader_vrptw;
mod params;
mod population;
mod pred_queue;
mod problem;
mod reverse_mode;
mod sequence;
mod solution;
mod solver;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{Map, Value};
use hierarchical::HierMode;
use loader_vrptw::DistanceConvention;
use solver::{RunType, Solver};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn print_usage() {
    eprintln!(
        "Simple usage (VRPTW, vehicles first then distance; best calibrated setup):\n\
         \x20 hgs_advance <instance.txt> [time_limit_seconds=60]\n\
         \x20 -> solution written to ./<instance>.sol\n\
         \n\
         Full usage: hgs_advance --instance <path> [--type vrptw|cvrp] [--output-dir <dir>]\n\
         \x20                 [--time-limit <seconds>] [--exploration-level <0-6>]\n\
         \x20                 [--param key=value ...] [--seed <0-255>]\n\
         \n\
         If --type is omitted, it is guessed from the instance file contents:\n\
         CVRPLib TSPLIB-style files (NODE_COORD_SECTION/DIMENSION) => cvrp\n\
         Solomon/Gehring-Homberger files => vrptw\n\
         \n\
         If --output-dir is omitted, no solution file is written; the result is\n\
         only printed to stdout. The solution is written as <output-dir>/<instance-stem>.sol\n\
         \n\
         --time-limit caps wall-clock runtime; without it, the exploration-level\n\
         preset's own iteration budget is the only stop criterion.\n\
         \n\
         Objective / mode (VRPTW):\n\
         \x20 --mode distance     original hgs_advance: minimize distance only (default)\n\
         \x20 --mode k-search     OPTION 1: fixed fleet k inside HGS, binary search on k\n\
         \x20                     outside; then minimize distance with the smallest k\n\
         \x20 --mode lexico       OPTION 2: single search on M*#routes + distance\n\
         \x20 --mode fixed-k --k K  minimize distance with at most K vehicles\n\
         \x20 --probe-time <s>    k-search: time without finding a feasible solution\n\
         \x20                     after which a k is assumed infeasible (default: 10% of\n\
         \x20                     the time limit)\n\
         \x20 --distance round|trunc10  arc distance convention (default round = nint of\n\
         \x20                     the Euclidean distance; trunc10 = legacy x10 + truncation)\n\
         \x20 --quiet             hierarchical modes: only print the final line"
    );
}

fn detect_run_type(path: &Path) -> Result<RunType> {
    let content = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    if content.contains("NODE_COORD_SECTION") || content.contains("DIMENSION") {
        Ok(RunType::Cvrp)
    } else {
        Ok(RunType::Vrptw)
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut instance: Option<PathBuf> = None;
    let mut run_type: Option<String> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut time_limit_secs: Option<f64> = None;
    let mut exploration_level: Option<u64> = None;
    let mut seed: Option<u8> = None;
    let mut extra_params: Map<String, Value> = Map::new();
    let mut mode: String = "distance".to_string();
    let mut fixed_k: Option<usize> = None;
    let mut probe_time: Option<f64> = None;
    let mut convention = DistanceConvention::Round;
    let mut verbose = true;
    let mut mode_given = false;
    let mut positional: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--instance" => {
                i += 1;
                instance = args.get(i).map(PathBuf::from);
            }
            "--type" => {
                i += 1;
                run_type = args.get(i).cloned();
            }
            "--output-dir" => {
                i += 1;
                output_dir = args.get(i).map(PathBuf::from);
            }
            "--time-limit" => {
                i += 1;
                time_limit_secs = args.get(i).and_then(|s| s.parse::<f64>().ok());
            }
            "--exploration-level" => {
                i += 1;
                exploration_level = args.get(i).and_then(|s| s.parse::<u64>().ok());
            }
            "--seed" => {
                i += 1;
                seed = args.get(i).and_then(|s| s.parse::<u8>().ok());
            }
            "--param" => {
                i += 1;
                if let Some(kv) = args.get(i) {
                    if let Some((k, v)) = kv.split_once('=') {
                        let value = serde_json::from_str::<Value>(v)
                            .unwrap_or_else(|_| Value::String(v.to_string()));
                        extra_params.insert(k.to_string(), value);
                    } else {
                        bail!("--param expects key=value, got '{}'", kv);
                    }
                }
            }
            "--mode" => {
                i += 1;
                mode = args.get(i).cloned().unwrap_or_default();
                mode_given = true;
            }
            "--k" => {
                i += 1;
                fixed_k = args.get(i).and_then(|s| s.parse::<usize>().ok());
            }
            "--probe-time" => {
                i += 1;
                probe_time = args.get(i).and_then(|s| s.parse::<f64>().ok());
            }
            "--distance" => {
                i += 1;
                convention = match args.get(i).map(|s| s.as_str()) {
                    Some("round") => DistanceConvention::Round,
                    Some("trunc10") => DistanceConvention::Trunc10,
                    other => bail!("Unknown --distance {:?}, expected round|trunc10", other),
                };
            }
            "--quiet" => {
                verbose = false;
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other if !other.starts_with('-') => positional.push(other.to_string()),
            other => {
                print_usage();
                bail!("Unknown argument: {}", other);
            }
        }
        i += 1;
    }

    // Simple call:  hgs_advance <instance> [time_limit_seconds]
    // = VRPTW, hierarchical objective (#routes, then distance), option 1
    // (fixed fleet k inside HGS + binary search on k), exploration level 5 and
    // max_it_noimprov = 5000 (calibrated configuration "ks_l5_noimp5k").
    // The solution is written to ./<instance-stem>.sol unless --output-dir is given.
    let simple_call = !positional.is_empty();
    if simple_call {
        if positional.len() > 2 || (instance.is_some() && !positional.is_empty()) {
            print_usage();
            bail!("Use either `hgs_advance <instance> [time_limit]` or the --instance flags, not both");
        }
        instance = Some(PathBuf::from(&positional[0]));
        if let Some(t) = positional.get(1) {
            time_limit_secs = Some(
                t.parse::<f64>()
                    .map_err(|_| anyhow!("time limit must be a number of seconds, got '{}'", t))?,
            );
        }
        if !mode_given {
            mode = "k-search".to_string();
        }
        if run_type.is_none() {
            run_type = Some("vrptw".to_string());
        }
        if !extra_params.contains_key("max_it_noimprov") {
            extra_params.insert("max_it_noimprov".to_string(), Value::from(5000u64));
        }
        if output_dir.is_none() {
            output_dir = Some(PathBuf::from("."));
        }
    }

    let instance = match instance {
        Some(p) => p,
        None => {
            print_usage();
            return Err(anyhow!("--instance is required"));
        }
    };
    let run_type = match run_type.as_deref() {
        Some("vrptw") => RunType::Vrptw,
        Some("cvrp") => RunType::Cvrp,
        Some(other) => bail!("Unknown --type '{}', expected 'vrptw' or 'cvrp'", other),
        None => detect_run_type(&instance)?,
    };

    if let Some(dir) = &output_dir {
        fs::create_dir_all(dir).with_context(|| format!("creating output dir {}", dir.display()))?;
    }

    if let Some(level) = exploration_level {
        extra_params.insert("exploration_level".to_string(), Value::from(level));
    }
    let mut hyperparameters = if extra_params.is_empty() {
        None
    } else {
        Some(extra_params)
    };

    let hier_mode = match mode.as_str() {
        "distance" => None,
        "k-search" => Some(HierMode::KSearch),
        "lexico" => Some(HierMode::Lexico),
        "fixed-k" => Some(HierMode::FixedK(
            fixed_k.ok_or_else(|| anyhow!("--mode fixed-k requires --k <K>"))?,
        )),
        other => bail!("Unknown --mode '{}'", other),
    };
    if let Some(hm) = hier_mode {
        // Hierarchical modes use GA restarts until the time limit; level 5 gives
        // each restart enough iterations (tested: level 4 restarts too early).
        if exploration_level.is_none() {
            let mut hp = hyperparameters.clone().unwrap_or_default();
            hp.insert("exploration_level".to_string(), Value::from(5u64));
            hyperparameters = Some(hp);
        }
        if run_type != RunType::Vrptw {
            bail!("--mode {} is only implemented for VRPTW instances", mode);
        }
        let tl = time_limit_secs.unwrap_or(60.0);
        let pt = probe_time.unwrap_or((0.1 * tl).max(1.0));
        if verbose && simple_call {
            println!(
                "[config] mode {}, time limit {}s, probe time {:.1}s, exploration level {}, max_it_noimprov {}",
                mode,
                tl,
                pt,
                exploration_level.unwrap_or(5),
                hyperparameters
                    .as_ref()
                    .and_then(|h| h.get("max_it_noimprov"))
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "preset".to_string())
            );
        }
        let res = Solver::solve_hierarchical_file(
            &instance,
            hm,
            &hyperparameters,
            output_dir.as_deref(),
            tl,
            pt,
            seed,
            convention,
            verbose,
        )?;
        match res {
            Some(f) => {
                let scale = convention.scale();
                if scale == 1.0 {
                    println!(
                        "Solved '{}' [mode {}]: {} routes, total distance = {} (best found at {:.2}s)",
                        instance.display(), mode, f.nb_routes, f.distance, f.time_found
                    );
                } else {
                    println!(
                        "Solved '{}' [mode {}]: {} routes, total distance = {:.2} (best found at {:.2}s)",
                        instance.display(), mode, f.nb_routes, f.distance as f64 / scale, f.time_found
                    );
                }
            }
            None => println!("No feasible solution found for '{}'.", instance.display()),
        }
        return Ok(());
    }

    let result = Solver::solve_instance_file(
        run_type,
        &instance,
        &hyperparameters,
        output_dir.as_deref(),
        time_limit_secs,
        seed,
        convention,
    )?;

    match result {
        Some((sol, cost, nb_routes)) => {
            let scale = if run_type == RunType::Vrptw { convention.scale() } else { 1.0 };
            println!(
                "Solved '{}': {} routes, total distance = {:.2}",
                instance.display(),
                nb_routes,
                cost as f64 / scale
            );
            let _ = sol; // routes are written to the .sol file when --output-dir is set
            if let Some(dir) = &output_dir {
                let stem = instance
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "solution".to_string());
                println!("Solution written to {}", dir.join(format!("{}.sol", stem)).display());
            }
        }
        None => println!("No feasible solution found for '{}'.", instance.display()),
    }

    Ok(())
}
