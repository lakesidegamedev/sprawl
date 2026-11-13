//! Service coverage, land value, demand and the map layers built from them.

use crate::roadnet::RoadNet;
use crate::*;

const CELL: f64 = 80.0;

/// Weights of the land value terms; they add up to a little under 1 so a perfect lot is near 1.
const BASE: f64 = 0.18;
const W_ACCESS: f64 = 0.30;
const W_JOBS: f64 = 0.12;
const W_SERVICES: f64 = 0.14;
const W_PARKS: f64 = 0.10;
const W_NEIGHBOURS: f64 = 0.10;
const W_INDUSTRY: f64 = 0.10;
const W_TRAFFIC: f64 = 0.15;
/// Each percentage point of tax above 7 percent takes this much off a lot's value.
const TAX_BURDEN_PER_PCT: f64 = 0.02;

struct Grid {
    cells: Vec<Vec<u32>>,
    n: i64,
}

impl Grid {
    fn new(sim: &Sim) -> Grid {
        let n = (sim.town.size / CELL).ceil() as i64 + 1;
        let mut cells = vec![Vec::new(); (n * n) as usize];
        for (i, l) in sim.lots.iter().enumerate() {
            if l.phase == Phase::Hidden {
                continue;
            }
            let (cx, cy) = Grid::key(n, l.pos);
            cells[(cy * n + cx) as usize].push(i as u32);
        }
        Grid { cells, n }
    }

    fn key(n: i64, p: Pt) -> (i64, i64) {
        (
            ((p.x / CELL).floor() as i64).clamp(0, n - 1),
            ((p.y / CELL).floor() as i64).clamp(0, n - 1),
        )
    }

    /// Lots within `r` metres of `p` (r up to a few cells), in index order per cell.
    fn near(&self, p: Pt, r: f64, mut f: impl FnMut(usize, f64)) {
        let (cx, cy) = Grid::key(self.n, p);
        let k = (r / CELL).ceil() as i64;
        for y in (cy - k).max(0)..=(cy + k).min(self.n - 1) {
            for x in (cx - k).max(0)..=(cx + k).min(self.n - 1) {
                for &i in &self.cells[(y * self.n + x) as usize] {
                    f(i as usize, 0.0);
                }
            }
        }
    }
}

impl Sim {
    fn run_visible(&self, r: &RoadRun) -> bool {
        r.addition.is_none_or(|a| self.revealed[a])
    }

    /// Rebuild the road graph if roads changed, then coverage, land value and demand.
    pub(crate) fn recompute_values(&mut self) {
        self.values_stale = false;
        if self.net_dirty || self.roadnet.is_none() {
            let roads: Vec<(RoadClass, Vec<Pt>)> = self
                .runs
                .iter()
                .filter(|r| self.run_visible(r))
                .map(|r| (r.class, r.points.clone()))
                .collect();
            self.roadnet = Some(RoadNet::build(&roads));
            self.lot_node.clear();
            self.net_dirty = false;
        }
        let rn = self.roadnet.take().unwrap();
        while self.lot_node.len() < self.lots.len() {
            let p = self.lots[self.lot_node.len()].pos;
            self.lot_node.push(rn.attach(p));
        }
        self.compute_coverage(&rn);
        self.roadnet = Some(rn);
        self.compute_demand();
        self.compute_land_value();
    }

    fn compute_coverage(&mut self, rn: &RoadNet) {
        let n = self.lots.len();
        for s in Service::ALL {
            let mut cov = vec![0.0f32; n];
            let f = s
                .dept()
                .map_or(1.0, |d| self.budget[d.index()] as f64 / 100.0);
            let strength = f.min(1.0);
            let reach = s.reach_m() * (0.4 + 0.6 * f);
            if strength > 0.0 {
                for i in 0..n {
                    let l = &self.lots[i];
                    if !matches!(l.phase, Phase::Built { .. }) {
                        continue;
                    }
                    let svc = l.civic.map(CivicKind::service).or(service_of_kind(l.kind));
                    if svc != Some(s) {
                        continue;
                    }
                    let Some((src, off)) = self.lot_node[i] else {
                        continue;
                    };
                    let dist = rn.distances(src, reach);
                    for (j, c) in cov.iter_mut().enumerate() {
                        let Some((nd, o2)) = self.lot_node[j] else {
                            continue;
                        };
                        let dn = dist[nd as usize];
                        if dn == f32::MAX {
                            continue;
                        }
                        let d = dn as f64 + off + o2;
                        if d < reach {
                            let v = (strength * (1.0 - d / reach)) as f32;
                            if v > *c {
                                *c = v;
                            }
                        }
                    }
                }
            }
            self.coverage[s.index()] = cov;
        }
    }

    pub(crate) fn mean_demand(&self) -> f64 {
        let d = self.class_demand;
        (d.homes + d.shops + d.industry) as f64 / 3.0
    }

    fn compute_demand(&mut self) {
        let pop = self.population() as f64;
        let jobs = self.jobs() as f64;
        let workers = pop * 0.55;
        let empty_share = self.empty_count() as f64 / self.built_count().max(1) as f64;
        let land_left = !self.vacant.is_empty() || self.additions_left() > 0;
        let shop_jobs = self.jobs_of(Class::Shops) as f64;
        let ratio = |a: f64, b: f64| (a - b) / (a + b + 60.0);
        let base = |c: Class| -> f64 {
            match c {
                Class::Homes => 0.6 + 0.4 * ratio(jobs, workers),
                Class::Shops => 0.6 + 0.4 * ratio(pop * 0.35, shop_jobs),
                Class::Industry => 0.6 + 0.4 * ratio(workers, jobs),
            }
        };
        let mut out = ByClass::all(0.0f32);
        for c in Class::ALL {
            let taxf = (growth_mult(self.tax.get(c) as f64 / 10.0) / 1.6).clamp(0.0, 1.0);
            let season = self.season_pct(c) as f64 / 100.0;
            let pull = 1.0 - self.rival_pull(c);
            let d =
                base(c).clamp(0.0, 1.0) * taxf * (1.0 - 0.7 * empty_share.min(1.0)) * season * pull;
            out.set(
                c,
                if land_left {
                    d.clamp(0.0, 1.0) as f32
                } else {
                    0.0
                },
            );
        }
        self.class_demand = out;
    }

    /// 0 to 1: how strongly the town wants to grow at today's tax rates and vacancy.
    pub fn demand(&self) -> f64 {
        self.mean_demand()
    }

    /// Demand 0 to 1 for one class.
    pub fn class_demand(&self, c: Class) -> f32 {
        self.class_demand.get(c)
    }

    fn compute_land_value(&mut self) {
        let grid = Grid::new(self);
        let n = self.lots.len();
        let mut out = vec![0.0f32; n];
        let road_f = (self.budget[Dept::Roads.index()] as f64 / 100.0).min(1.0);
        for (i, l) in self.lots.iter().enumerate() {
            if l.phase == Phase::Hidden {
                continue;
            }
            let access = (1.0 - l.pos.dist(self.hall) / 2500.0).clamp(0.0, 1.0);
            let (mut job_w, mut built_nb, mut all_nb, mut ind_nb) =
                (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            grid.near(l.pos, 300.0, |j, _| {
                if j == i {
                    return;
                }
                let m = &self.lots[j];
                let d = m.pos.dist(l.pos);
                let built = matches!(m.phase, Phase::Built { .. });
                if d <= 300.0 && built && !m.exempt {
                    job_w += jobs_weight(m.kind) * (1.0 - d / 300.0);
                }
                if d <= 120.0 && m.radius < 40.0 && class_of(m.kind).is_some() {
                    all_nb += 1.0;
                    if built {
                        built_nb += 1.0;
                    }
                    if built && class_of(m.kind) == Some(Class::Industry) {
                        ind_nb += 1.0;
                    }
                }
            });
            let jobs_near = (job_w / 60.0).min(1.0);
            let neighbours = if all_nb > 0.0 { built_nb / all_nb } else { 0.0 };
            let c = |s: Service| self.coverage[s.index()][i] as f64;
            let services =
                (c(Service::Police) + c(Service::Fire) + c(Service::Schools) + c(Service::Health))
                    / 4.0;
            let class = class_of(l.kind);
            let industry = if class == Some(Class::Industry) {
                0.0
            } else {
                (ind_nb / 4.0).min(1.0)
            };
            let rate = class.map_or(0.0, |c| self.tax.get(c) as f64 / 10.0);
            let burden = (rate - 7.0) * TAX_BURDEN_PER_PCT;
            let traffic = self.lot_traffic(i);
            let v = BASE
                + W_ACCESS * access
                + W_JOBS * jobs_near
                + W_SERVICES * services
                + W_PARKS * c(Service::Parks)
                + W_NEIGHBOURS * neighbours
                - W_INDUSTRY * industry
                - W_TRAFFIC * traffic
                - 0.06 * (1.0 - road_f)
                - burden;
            out[i] = v.clamp(0.0, 1.0) as f32;
        }
        self.lot_value = out;
    }

    /// Mean land value over lots that exist (not hidden, not farms).
    pub fn avg_land_value(&self) -> f32 {
        let (mut sum, mut n) = (0.0f64, 0u32);
        for (l, &v) in self.lots.iter().zip(&self.lot_value) {
            if l.phase != Phase::Hidden && !l.exempt {
                sum += v as f64;
                n += 1;
            }
        }
        if n == 0 { 0.0 } else { (sum / n as f64) as f32 }
    }

    /// Land value 0 to 1 of one lot as of the last monthly recompute.
    pub fn land_value(&self, lot: usize) -> f32 {
        self.lot_value.get(lot).copied().unwrap_or(0.0)
    }

    /// One value per lot, same order as `lots()`, all 0 to 1 except `Use` which is a category id.
    ///
    /// - `LandValue`: the lot's land value (hidden lots 0).
    /// - `BuildingLevel`: level 1 to 5 over 5 while built; 0 for empty land.
    /// - `Use`: 0 nothing or not built, 1 homes, 2 shops, 3 industry, 4 civic, 5 farm.
    /// - `Coverage(s)`: how well service `s` reaches the lot by road travel distance.
    /// - `Demand(c)`: the town's demand for class `c` on lots of that class, 0 on others.
    ///
    /// `Traffic` is a road layer; use `road_values`.
    pub fn lot_values(&self, kind: MapKind) -> Vec<f32> {
        let n = self.lots.len();
        match kind {
            MapKind::LandValue => (0..n).map(|i| self.land_value(i)).collect(),
            MapKind::BuildingLevel => self
                .lots
                .iter()
                .map(|l| match l.phase {
                    Phase::Built { .. } | Phase::Building { .. } if !l.exempt => {
                        l.level as f32 / 5.0
                    }
                    _ => 0.0,
                })
                .collect(),
            MapKind::Use => self
                .lots
                .iter()
                .map(|l| {
                    if !matches!(l.phase, Phase::Built { .. } | Phase::Building { .. }) {
                        return 0.0;
                    }
                    if l.kind == LotKind::Farm {
                        return 5.0;
                    }
                    match class_of(l.kind) {
                        Some(Class::Homes) => 1.0,
                        Some(Class::Shops) => 2.0,
                        Some(Class::Industry) => 3.0,
                        None => 4.0,
                    }
                })
                .collect(),
            MapKind::Coverage(s) => {
                let v = &self.coverage[s.index()];
                (0..n).map(|i| v.get(i).copied().unwrap_or(0.0)).collect()
            }
            MapKind::Demand(c) => self
                .lots
                .iter()
                .map(|l| {
                    if class_of(l.kind) == Some(c) && l.phase != Phase::Hidden {
                        self.class_demand.get(c)
                    } else {
                        0.0
                    }
                })
                .collect(),
            MapKind::Traffic => (0..n).map(|i| self.lot_traffic(i) as f32).collect(),
        }
    }

    /// One value per entry of `road_runs()`. `Traffic`: congestion 0 to 1 (volume over capacity,
    /// capped, see `RoadTraffic::congestion`); all zero until the first traffic run. Other kinds return an empty list.
    pub fn road_values(&self, kind: MapKind) -> Vec<f32> {
        match kind {
            MapKind::Traffic => (0..self.runs.len())
                .map(|i| self.run_traffic.get(i).map_or(0.0, |t| t.congestion()))
                .collect(),
            _ => Vec::new(),
        }
    }
}

fn jobs_weight(k: LotKind) -> f64 {
    match k {
        LotKind::Shop | LotKind::Gas | LotKind::FastFood => 6.0,
        LotKind::Strip | LotKind::Workshop | LotKind::Warehouse => 15.0,
        LotKind::Office | LotKind::BigBox => 30.0,
        _ => 0.0,
    }
}
