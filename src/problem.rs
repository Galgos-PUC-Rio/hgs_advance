#[derive(Copy, Clone)]
pub struct NodeData {
    pub start_tw: i32,
    pub end_tw: i32,
    pub service_time: i32,
    pub demand: i32,
}

#[derive(Clone)]
pub struct Problem {
    pub seed: [u8; 32],
    pub nb_nodes: usize,
    pub nb_vehicles: usize,
    pub lb_vehicles: usize,
    pub is_vrptw: bool,
    pub fixed_distance_offset: i64,
    pub max_capacity: i32,
    /// COST matrix used by the objective (= travel distance, plus `vehicle_cost`
    /// on every arc leaving the depot towards a client when a fixed cost per
    /// used vehicle is active).
    pub distance_matrix: Vec<i32>,
    /// Pure travel distance/time matrix (used for time-window propagation and
    /// for reporting the true total distance). Never contains `vehicle_cost`.
    pub travel_matrix: Vec<i32>,
    /// Fixed cost charged once per non-empty route (0 = pure distance objective).
    pub vehicle_cost: i64,
    /// Upper clamp for the adaptive penalty coefficients.
    pub penalty_max: usize,
    pub node_positions: Vec<(i32, i32)>,
    pub node_data: Vec<NodeData>,
}

impl Problem {
    /// Cost of arc (i,j) as seen by the objective function.
    #[inline(always)]
    pub fn dm(&self, i: usize, j: usize) -> i32 {
        debug_assert!(i < self.nb_nodes && j < self.nb_nodes);
        let idx = i * self.nb_nodes + j;
        debug_assert!(idx < self.distance_matrix.len());
        unsafe { *self.distance_matrix.get_unchecked(idx) }
    }

    /// Travel time / pure distance of arc (i,j).
    #[inline(always)]
    pub fn tm(&self, i: usize, j: usize) -> i32 {
        debug_assert!(i < self.nb_nodes && j < self.nb_nodes);
        let idx = i * self.nb_nodes + j;
        debug_assert!(idx < self.travel_matrix.len());
        unsafe { *self.travel_matrix.get_unchecked(idx) }
    }

    #[inline(always)]
    pub fn nd(&self, i: usize) -> &NodeData {
        debug_assert!(i < self.nb_nodes);
        unsafe { self.node_data.get_unchecked(i) }
    }

    /// Activates (or deactivates with 0) a fixed cost per used vehicle, so that
    /// cost(solution) = vehicle_cost * #routes + distance. Implemented by adding
    /// `vehicle_cost` to every depot->client arc of the COST matrix (each
    /// non-empty route uses exactly one such arc; an empty route 0->0 uses none),
    /// which keeps every incremental move evaluation exact.
    pub fn set_vehicle_cost(&mut self, vehicle_cost: i64) {
        let n = self.nb_nodes;
        self.distance_matrix = self.travel_matrix.clone();
        for j in 1..n {
            let v = self.travel_matrix[j] as i64 + vehicle_cost;
            assert!(v < i32::MAX as i64 / 4, "vehicle cost too large for i32 arithmetic");
            self.distance_matrix[j] = v as i32;
        }
        self.vehicle_cost = vehicle_cost;
    }

    /// A vehicle cost large enough to make the objective lexicographic
    /// (#routes first, distance second): it exceeds the largest possible total
    /// distance of any solution, so saving one route always beats any distance change.
    pub fn lexicographic_vehicle_cost(&self) -> i64 {
        let n = self.nb_nodes;
        let mut ub: i64 = 0;
        for i in 1..n {
            let mut mx = 0i64;
            for j in 0..n {
                mx = mx.max(self.tm(i, j) as i64);
            }
            ub += mx; // each client has exactly one outgoing arc
        }
        let mut mx0 = 0i64;
        for j in 1..n {
            mx0 = mx0.max(self.tm(0, j) as i64);
        }
        ub += mx0 * (n as i64 - 1); // at most one depot arc per client
        ub + 1
    }

    /// True (unpenalized, vehicle-cost-free) total distance of a set of routes.
    /// Routes may be given with or without depot endpoints.
    pub fn travel_distance(&self, routes: &[Vec<usize>]) -> i64 {
        let mut total = 0i64;
        for r in routes {
            let clients: Vec<usize> = r.iter().copied().filter(|&c| c != 0).collect();
            if clients.is_empty() {
                continue;
            }
            let mut prev = 0usize;
            for &c in &clients {
                total += self.tm(prev, c) as i64;
                prev = c;
            }
            total += self.tm(prev, 0) as i64;
        }
        total
    }
}
