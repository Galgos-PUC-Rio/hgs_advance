use super::genetic::Genetic;
use super::loader_cvrp::CVRPLoader;
use super::loader_vrptw::{DistanceConvention, VRPTWLoader};
use super::hierarchical::{HierMode, HierSolver, Found};
use super::params::Params;
use super::problem::Problem;
use super::reverse_mode;
use crate::solution::Solution;
use anyhow::Result;
use rand::{rngs::SmallRng, SeedableRng};
use serde_json::{Map, Value};
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

pub struct Solver;

/// Instance format: classical Solomon/Gehring-Homberger VRPTW `.txt` files,
/// or CVRPLib/TSPLIB-style CVRP `.vrp` files.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunType {
    Vrptw,
    Cvrp,
}

impl Solver {
    /// Sum of consecutive-node distances over all routes. Routes are assumed
    /// feasible (as produced by `best_feasible()` / incumbent tracking), so
    /// this plain distance sum equals the true (unpenalized) cost.
    fn route_distance(data: &Problem, routes: &[Vec<usize>]) -> i32 {
        data.travel_distance(routes) as i32
    }

    fn write_solution_to_file(
        path: &Path,
        solution: &Solution,
        cost: i32,
        nb_routes: usize,
        cpu_time_secs: f64,
        scale: f64,
    ) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                create_dir_all(parent)?;
            }
        }

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let mut route_id = 1usize;
        for route in &solution.routes {
            let clients: Vec<usize> = route.iter().copied().filter(|&c| c != 0).collect();
            if clients.is_empty() {
                continue;
            }
            write!(writer, "Route #{}:", route_id)?;
            for client in clients {
                write!(writer, " {}", client)?;
            }
            writeln!(writer)?;
            route_id += 1;
        }

        // `scale` converts internal integer distances back to instance units
        // (10 for the legacy x10-truncated VRPTW convention, 1 otherwise).
        if scale == 1.0 {
            writeln!(writer, "Cost {}", cost)?;
        } else {
            writeln!(writer, "Cost {:.2}", cost as f64 / scale)?;
        }
        writeln!(writer, "NB_ROUTES: {}", nb_routes)?;
        writeln!(writer, "CPU_TIME: {:.6}", cpu_time_secs)?;
        writer.flush()?;
        Ok(())
    }

    fn solve(
        data: Arc<Problem>,
        params: Params,
        t0: &Instant,
        save_solution: Option<&dyn Fn(&Solution) -> Result<()>>,
        time_limit_secs: Option<f64>,
    ) -> Result<Option<(Solution, i32, usize)>> {
        if params.decomp_nb_phases > 0 {
            if params.display_traces {
                println!("----- DECOMPOSITION ENABLED: ACTIVATING REVERSED MODE");
            }
            return reverse_mode::solve_reversed_mode(data, params, t0, save_solution, time_limit_secs);
        }

        let mut rng = SmallRng::from_seed(data.seed);
        let mut ga = Genetic::new(data, params);
        Ok(ga
            .run(&mut rng, t0, save_solution, None, time_limit_secs)
            .map(|(routes, cost)| (Solution { routes: routes.clone() }, cost, routes.len())))
    }

    /// Loads a classical instance file, runs hgs_advance, and (optionally) writes
    /// the resulting solution to `output_dir` as `<instance-stem>.sol`.
    ///
    /// When `output_dir` is set, the solution file is also updated as a
    /// checkpoint every time the search finds a new incumbent (not just at the
    /// end) — important for long, unattended runs on large instances, where
    /// you want a usable solution on disk even if the process is interrupted
    /// before the time limit is reached.
    pub fn solve_instance_file(
        run_type: RunType,
        instance_path: &Path,
        hyperparameters: &Option<Map<String, Value>>,
        output_dir: Option<&Path>,
        time_limit_secs: Option<f64>,
        seed_byte: Option<u8>,
        convention: DistanceConvention,
    ) -> Result<Option<(Solution, i32, usize)>> {
        let t0 = Instant::now();
        let scale = if run_type == RunType::Vrptw { convention.scale() } else { 1.0 };
        let path_str = instance_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Instance path is not valid UTF-8"))?;
        let mut data = match run_type {
            RunType::Vrptw => VRPTWLoader::load(path_str, convention)?,
            RunType::Cvrp => CVRPLoader::load(path_str)?,
        };
        if let Some(seed) = seed_byte {
            data.seed = [seed; 32];
        }
        let params = Params::initialize(hyperparameters, &data);
        let data = Arc::new(data);

        let checkpoint_path: Option<PathBuf> = output_dir.map(|dir| {
            let stem = instance_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "solution".to_string());
            dir.join(format!("{}.sol", stem))
        });

        // Cheap: clones the Arc (refcount bump), not the underlying distance
        // matrix (which can be hundreds of MB on instances with ~10k nodes).
        let checkpoint_data = Arc::clone(&data);
        let checkpoint_t0 = t0;
        let save_solution = checkpoint_path.as_ref().map(|path| {
            let path = path.clone();
            move |sol: &Solution| -> Result<()> {
                let cost = Self::route_distance(&checkpoint_data, &sol.routes);
                let nb_routes = sol.routes.iter().filter(|r| r.len() > 2).count();
                Self::write_solution_to_file(
                    &path,
                    sol,
                    cost,
                    nb_routes,
                    checkpoint_t0.elapsed().as_secs_f64(),
                    scale,
                )
            }
        });
        let save_solution_ref = save_solution
            .as_ref()
            .map(|f| f as &dyn Fn(&Solution) -> Result<()>);

        match Self::solve(data, params, &t0, save_solution_ref, time_limit_secs) {
            Ok(Some((solution, cost, routes))) => {
                if let Some(path) = &checkpoint_path {
                    let cpu_time_secs = t0.elapsed().as_secs_f64();
                    Self::write_solution_to_file(path, &solution, cost, routes, cpu_time_secs, scale)?;
                }
                Ok(Some((solution, cost, routes)))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                eprintln!("Error: {}", e);
                Ok(None)
            }
        }
    }

    /// Hierarchical objective (#vehicles first, distance second) for VRPTW.
    /// Returns the best solution found as (routes, true distance, #routes).
    pub fn solve_hierarchical_file(
        instance_path: &Path,
        mode: HierMode,
        hyperparameters: &Option<Map<String, Value>>,
        output_dir: Option<&Path>,
        time_limit_secs: f64,
        probe_time_secs: f64,
        seed_byte: Option<u8>,
        convention: DistanceConvention,
        verbose: bool,
    ) -> Result<Option<Found>> {
        let t0 = Instant::now();
        let scale = convention.scale();
        let path_str = instance_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Instance path is not valid UTF-8"))?;
        let mut data = VRPTWLoader::load(path_str, convention)?;
        if let Some(seed) = seed_byte {
            data.seed = [seed; 32];
        }
        let mut params = Params::initialize(hyperparameters, &data);
        // The decomposition ("reverse") mode assumes a free fleet and a feasible
        // constructive start; the hierarchical drivers use the plain HGS instead.
        params.decomp_nb_phases = 0;
        let mut rng = SmallRng::from_seed(data.seed);

        let checkpoint_path: Option<PathBuf> = output_dir.map(|dir| {
            let stem = instance_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "solution".to_string());
            dir.join(format!("{}.sol", stem))
        });
        let save = |f: &Found| -> Result<()> {
            if let Some(path) = &checkpoint_path {
                Self::write_solution_to_file(
                    path,
                    &Solution { routes: f.routes.clone() },
                    f.distance as i32,
                    f.nb_routes,
                    f.time_found,
                    scale,
                )?;
            }
            Ok(())
        };
        let mut hs = HierSolver {
            base: data,
            params,
            t0,
            time_limit: time_limit_secs,
            probe_time: probe_time_secs,
            verbose,
            on_improve: Some(&save),
            best: None,
        };
        Ok(hs.solve(mode, &mut rng))
    }
}
