use super::problem::{NodeData, Problem};
use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct VRPTWLoader;

/// How Euclidean distances (and travel times) are turned into integers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DistanceConvention {
    /// d(i,j) = nint(euclid(i,j)), no scaling (default).
    Round,
    /// Legacy hgs_advance convention: coordinates and times x10, distances truncated.
    Trunc10,
}

impl DistanceConvention {
    /// Factor that converts internal distance units back to instance units.
    pub fn scale(self) -> f64 {
        match self {
            DistanceConvention::Round => 1.0,
            DistanceConvention::Trunc10 => 10.0,
        }
    }
}

impl VRPTWLoader {
    pub fn load(path: &str, convention: DistanceConvention) -> Result<Problem> {
        let file = File::open(path).map_err(|e| anyhow!("Cannot open VRPTW file '{}': {}", path, e))?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        for _ in 0..4 {
            let _ = lines.next();
        }

        let fleet_line = lines
            .next()
            .ok_or_else(|| anyhow!("Missing VRPTW fleet line"))?
            .map_err(|e| anyhow!("Cannot read VRPTW fleet line: {}", e))?;
        let fleet_parts: Vec<&str> = fleet_line.split_whitespace().collect();
        if fleet_parts.len() < 2 {
            return Err(anyhow!("Invalid VRPTW fleet line: '{}'", fleet_line));
        }
        let nb_vehicles = fleet_parts[0]
            .parse::<usize>()
            .map_err(|_| anyhow!("Invalid VRPTW vehicle count '{}'", fleet_parts[0]))?;
        let max_capacity = fleet_parts[1]
            .parse::<i32>()
            .map_err(|_| anyhow!("Invalid VRPTW capacity '{}'", fleet_parts[1]))?;

        for _ in 0..4 {
            let _ = lines.next();
        }

        let mut nodes: Vec<(usize, i32, i32, i32, i32, i32, i32)> = Vec::new();
        for line in lines {
            let raw = line.map_err(|e| anyhow!("Cannot read VRPTW line: {}", e))?;
            if raw.trim().is_empty() {
                continue;
            }
            let p: Vec<&str> = raw.split_whitespace().collect();
            if p.len() < 7 {
                continue;
            }
            nodes.push((
                p[0].parse::<usize>().map_err(|_| anyhow!("Invalid VRPTW node id '{}'", p[0]))?,
                p[1].parse::<i32>().map_err(|_| anyhow!("Invalid VRPTW x '{}'", p[1]))?,
                p[2].parse::<i32>().map_err(|_| anyhow!("Invalid VRPTW y '{}'", p[2]))?,
                p[3].parse::<i32>().map_err(|_| anyhow!("Invalid VRPTW demand '{}'", p[3]))?,
                p[4].parse::<i32>().map_err(|_| anyhow!("Invalid VRPTW ready time '{}'", p[4]))?,
                p[5].parse::<i32>().map_err(|_| anyhow!("Invalid VRPTW due time '{}'", p[5]))?,
                p[6].parse::<i32>().map_err(|_| anyhow!("Invalid VRPTW service time '{}'", p[6]))?,
            ));
        }

        if nodes.is_empty() {
            return Err(anyhow!("No VRPTW nodes found"));
        }

        let nb_nodes = nodes.len();
        let mut node_positions = vec![(0, 0); nb_nodes];
        let mut node_data = vec![
            NodeData {
                start_tw: 0,
                end_tw: 0,
                service_time: 0,
                demand: 0,
            };
            nb_nodes
        ];

        for (idx, node) in nodes.iter().enumerate() {
            if node.0 != idx {
                return Err(anyhow!("VRPTW nodes should be ordered from 0 to n-1"));
            }
            let f = match convention {
                DistanceConvention::Round => 1,
                // Legacy: multiply coordinates and times by 10, and truncate Euclidean distances
                DistanceConvention::Trunc10 => 10,
            };
            node_positions[idx] = (f * node.1, f * node.2);
            node_data[idx] = NodeData {
                start_tw: f * node.4,
                end_tw: f * node.5,
                service_time: f * node.6,
                demand: node.3,
            };
        }

        let mut distance_matrix = vec![0i32; nb_nodes * nb_nodes];
        for i in 0..nb_nodes {
            for j in 0..nb_nodes {
                let dx = (node_positions[i].0 - node_positions[j].0) as f64;
                let dy = (node_positions[i].1 - node_positions[j].1) as f64;
                let e = dx.hypot(dy);
                distance_matrix[i * nb_nodes + j] = match convention {
                    // nearest integer (halves rounded up; coordinates are integers so
                    // an exact .5 cannot actually occur for a Euclidean norm)
                    DistanceConvention::Round => e.round() as i32,
                    DistanceConvention::Trunc10 => e as i32,
                };
            }
        }

        let total_demand: i64 = node_data.iter().map(|nd| nd.demand as i64).sum();
        let lb_vehicles =
            ((total_demand + max_capacity as i64 - 1) / max_capacity as i64) as usize;
        Ok(Problem {
            seed: [0u8; 32],
            nb_nodes,
            nb_vehicles,
            lb_vehicles,
            is_vrptw: true,
            fixed_distance_offset: 0,
            max_capacity,
            travel_matrix: distance_matrix.clone(),
            distance_matrix,
            vehicle_cost: 0,
            penalty_max: 10_000,
            node_positions,
            node_data,
        })
    }
}
