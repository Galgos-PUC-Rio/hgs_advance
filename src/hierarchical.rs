//! Hierarchical objective for VRPTW: minimize (1) the number of vehicles/routes,
//! then (2) the total distance.
//!
//! Two strategies are provided:
//!
//! * `KSearch` (option 1): the GA works with a FIXED fleet of k vehicles and
//!   minimizes distance only. An outer loop chooses k by binary search: a probe
//!   at k that does not find a feasible solution within `probe_time` seconds is
//!   declared infeasible (so only larger k are tried afterwards); a probe that
//!   succeeds with r <= k routes makes r the new upper bound. The remaining time
//!   is then spent minimizing distance with the smallest feasible k.
//!
//! * `Lexico` (option 2): a single search over both criteria at once, using
//!   cost = M * #routes + distance, where M exceeds any possible total distance,
//!   so the order is exactly lexicographic.
//!
//! `FixedK(k)` runs a single fixed-fleet optimization (useful for experiments).

use super::genetic::Genetic;
use super::individual::Individual;
use super::params::Params;
use super::problem::Problem;
use crate::solution::Solution;
use anyhow::Result;
use rand::rngs::SmallRng;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HierMode {
    KSearch,
    Lexico,
    FixedK(usize),
}

#[derive(Clone, Debug)]
pub struct Found {
    pub routes: Vec<Vec<usize>>,
    pub nb_routes: usize,
    pub distance: i64,
    pub time_found: f64,
}

impl Found {
    fn key(&self) -> (usize, i64) {
        (self.nb_routes, self.distance)
    }
}

/// Checks feasibility (capacity + time windows, on the true travel times) and
/// returns the hierarchical evaluation of a solution.
pub fn evaluate(data: &Problem, params: &Params, routes: &[Vec<usize>], t: f64) -> Option<Found> {
    let routes: Vec<Vec<usize>> = routes.iter().filter(|r| r.iter().any(|&c| c != 0)).cloned().collect();
    let ind = Individual::new_from_routes(data, params, routes.clone());
    if ind.load_excess != 0 || ind.tw_violation != 0 {
        return None;
    }
    let mut seen = vec![false; data.nb_nodes];
    for r in &routes {
        for &c in r.iter().filter(|&&c| c != 0) {
            if seen[c] {
                return None;
            }
            seen[c] = true;
        }
    }
    if (1..data.nb_nodes).any(|c| !seen[c]) {
        return None;
    }
    Some(Found { nb_routes: routes.len(), distance: data.travel_distance(&routes), routes, time_found: t })
}

pub struct HierSolver<'a> {
    pub base: Problem,
    pub params: Params,
    pub t0: Instant,
    pub time_limit: f64,
    pub probe_time: f64,
    pub verbose: bool,
    pub on_improve: Option<&'a dyn Fn(&Found) -> Result<()>>,
    pub best: Option<Found>,
}

impl<'a> HierSolver<'a> {
    fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    fn offer(&mut self, f: &Found) {
        let improved = self.best.as_ref().map_or(true, |b| f.key() < b.key());
        if improved {
            if self.verbose {
                println!("[{:8.2}s] new best: {} routes, distance {}", f.time_found, f.nb_routes, f.distance);
            }
            if let Some(cb) = self.on_improve {
                let _ = cb(f);
            }
            self.best = Some(f.clone());
        }
    }

    /// GA restarts on `data` until `deadline` (seconds since t0). Returns the best
    /// feasible solution found in these runs. With `stop_first`, returns at the
    /// first feasible solution.
    fn run_restarts(&mut self, data: Problem, rng: &mut SmallRng, deadline: f64, stop_first: bool) -> Option<Found> {
        let data = Arc::new(data);
        let local_best: RefCell<Option<Found>> = RefCell::new(None);
        let t0 = self.t0;
        let params = self.params;
        let capture = |sol: &Solution| -> Result<()> {
            if let Some(f) = evaluate(&data, &params, &sol.routes, t0.elapsed().as_secs_f64()) {
                let mut lb = local_best.borrow_mut();
                if lb.as_ref().map_or(true, |b| f.key() < b.key()) {
                    *lb = Some(f);
                }
            }
            Ok(())
        };
        let mut restarts = 0usize;
        while self.now() < deadline {
            let mut ga = Genetic::new(Arc::clone(&data), params);
            ga.stop_at_first_feasible = stop_first;
            let res = ga.run(rng, &t0, Some(&capture), None, Some(deadline));
            if let Some((routes, _)) = res {
                let _ = capture(&Solution { routes });
            }
            restarts += 1;
            if let Some(f) = local_best.borrow().clone() {
                self.offer(&f);
                if stop_first {
                    break;
                }
            }
        }
        if self.verbose && !stop_first {
            println!("[{:8.2}s]   ({} GA run(s) in this phase)", self.now(), restarts);
        }
        local_best.into_inner()
    }

    fn fleet_problem(&self, k: usize) -> Problem {
        let mut d = self.base.clone();
        d.nb_vehicles = k;
        d.set_vehicle_cost(0);
        d
    }

    pub fn solve(&mut self, mode: HierMode, rng: &mut SmallRng) -> Option<Found> {
        let deadline = self.time_limit;
        match mode {
            HierMode::FixedK(k) => {
                let k = k.max(self.base.lb_vehicles);
                let d = self.fleet_problem(k);
                self.run_restarts(d, rng, deadline, false);
            }
            HierMode::Lexico => {
                let mut d = self.base.clone();
                let m = d.lexicographic_vehicle_cost();
                d.set_vehicle_cost(m);
                d.penalty_max = d.penalty_max.max(m as usize);
                if self.verbose {
                    println!("[lexico] cost = {} * #routes + distance (fleet limit {})", m, d.nb_vehicles);
                }
                self.run_restarts(d, rng, deadline, false);
            }
            HierMode::KSearch => self.k_search(rng, deadline),
        }
        self.best.clone()
    }

    fn k_search(&mut self, rng: &mut SmallRng, deadline: f64) {
        let fleet = self.base.nb_vehicles;
        let mut lo = self.base.lb_vehicles.max(1);
        if self.verbose {
            println!("[k-search] fleet limit {}, capacity lower bound {}, probe budget {:.1}s", fleet, lo, self.probe_time);
        }
        // Initial upper bound: first feasible solution with the full fleet.
        let d = self.fleet_problem(fleet);
        let first = self.run_restarts(d, rng, deadline, true);
        let Some(first) = first else {
            if self.verbose { println!("[k-search] no feasible solution found with the full fleet"); }
            return;
        };
        let mut hi = first.nb_routes;
        let mut best_k_sol = first;
        if self.verbose {
            println!("[{:8.2}s] [k-search] initial upper bound k = {}", self.now(), hi);
        }
        while lo < hi && self.now() < deadline {
            let mid = (lo + hi) / 2;
            let probe_deadline = (self.now() + self.probe_time).min(deadline);
            let t_start = self.now();
            let d = self.fleet_problem(mid);
            match self.run_restarts(d, rng, probe_deadline, true) {
                Some(f) => {
                    if self.verbose {
                        println!("[{:8.2}s] [k-search] probe k = {:3}: FEASIBLE ({} routes) after {:.2}s",
                                 self.now(), mid, f.nb_routes, self.now() - t_start);
                    }
                    hi = f.nb_routes.min(mid);
                    best_k_sol = f;
                }
                None => {
                    if self.verbose {
                        println!("[{:8.2}s] [k-search] probe k = {:3}: not found in {:.2}s -> assumed infeasible",
                                 self.now(), mid, self.now() - t_start);
                    }
                    lo = mid + 1;
                }
            }
        }
        if self.verbose {
            println!("[{:8.2}s] [k-search] chosen k = {} (searched interval closed at [{}, {}]); optimizing distance with the remaining time",
                     self.now(), hi, lo, hi);
        }
        let _ = best_k_sol;
        let d = self.fleet_problem(hi);
        self.run_restarts(d, rng, deadline, false);
    }
}
