//! Standalone CLI for running the `hgs_advance` HGS-VRPTW/CVRP solver on
//! classical benchmark instances (Solomon/Gehring-Homberger `.txt` VRPTW files,
//! or CVRPLib/TSPLIB-style `.vrp` CVRP files).
//!
//! Example:
//!   hgs_advance --instance C101.txt --time-limit 60 --output-dir results/

mod compression;
mod constructive;
mod genetic;
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
use solver::{RunType, Solver};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn print_usage() {
    eprintln!(
        "Usage: hgs_advance --instance <path> [--type vrptw|cvrp] [--output-dir <dir>]\n\
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
         preset's own iteration budget is the only stop criterion."
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
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                print_usage();
                bail!("Unknown argument: {}", other);
            }
        }
        i += 1;
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
    let hyperparameters = if extra_params.is_empty() {
        None
    } else {
        Some(extra_params)
    };

    let result = Solver::solve_instance_file(
        run_type,
        &instance,
        &hyperparameters,
        output_dir.as_deref(),
        time_limit_secs,
        seed,
    )?;

    match result {
        Some((sol, cost, nb_routes)) => {
            let scale = if run_type == RunType::Vrptw { 10.0 } else { 1.0 };
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
