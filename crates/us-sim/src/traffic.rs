//! Traffic shapes the roads. Every quarter the town is snapshotted and a worker thread assigns
//! the peak hour trips and picks road upgrades and connectors (`us-grow`). The result is applied
//! on a fixed later day, so the outcome is the same however long the thread takes.

use crate::*;
use std::thread::JoinHandle;
use us_gen::{Lot, Road};
use us_grow::geom::{SegGrid, dist_pt_seg, polyline_len};
use us_grow::{Change, End, GrowParams, RuleProposer, TrafficScorer, plan_round};

/// Days between traffic runs.
pub const TRAFFIC_EVERY_DAYS: i32 = 90;
/// Days a run may take before its result is applied.
const TRAFFIC_LAG_DAYS: i32 = 8;
/// First run happens on this day of the cycle.
const TRAFFIC_OFFSET: i32 = 40;
const MIN_BUILT_FOR_TRAFFIC: usize = 40;
const CELL: f64 = 30.0;
/// Trips per dwelling at the peak hour, relative to the traffic crate's base: a sprawling town
/// where everyone drives.
const TRAFFIC_SCALE: f64 = 2.5;

/// What the last traffic run found on one road run.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoadTraffic {
    /// Vehicles per peak hour, both directions together.
    pub volume: f32,
    /// Volume over capacity at the equilibrium.
    pub vc: f32,
}

impl RoadTraffic {
    /// 0 (empty) to 1 (jammed): volume over capacity, capped at 1.5, scaled.
    pub fn congestion(&self) -> f32 {
        self.vc.clamp(0.0, 1.5) / 1.5
    }
}

enum Op {
    Upgrade {
        pts: Vec<Pt>,
        to: RoadClass,
        lots: Vec<u32>,
    },
    Connector {
        pa: Pt,
        pb: Pt,
        class: RoadClass,
        lots: Vec<u32>,
    },
}

struct Done {
    run_traffic: Vec<RoadTraffic>,
    avg_cong: f32,
    ops: Vec<Op>,
}

pub(crate) struct Pending {
    handle: JoinHandle<Done>,
    join_day: i32,
    /// Snapshot lot index to `Sim::lots` index.
    lot_map: Vec<usize>,
}

/// The kind a building counts as for trips, by level.
fn demand_kind(l: &LotState) -> LotKind {
    match (l.kind, l.level) {
        (LotKind::House, 2) => LotKind::Row,
        (LotKind::House | LotKind::Row, 3..) => LotKind::Apartments,
        (LotKind::Shop, 3..) => LotKind::Office,
        (k, _) => k,
    }
}

fn grow_params() -> GrowParams {
    GrowParams {
        upgrade_budget_m: 500.0,
        max_connectors: 2,
        ..GrowParams::default()
    }
}

impl Sim {
    pub(crate) fn run_visible_idx(&self, i: usize) -> bool {
        self.runs[i].addition.is_none_or(|a| self.revealed[a])
    }

    /// Bumped whenever a road is widened or added, so a renderer knows to redraw the roads.
    pub fn roads_version(&self) -> u32 {
        self.roads_version
    }

    /// Result of the latest traffic run, one entry per `road_runs()` entry (zeros before the first).
    pub fn road_traffic(&self) -> &[RoadTraffic] {
        &self.run_traffic
    }

    pub(crate) fn step_traffic(&mut self) {
        if let Some(p) = &self.pending
            && self.day >= p.join_day
        {
            let p = self.pending.take().unwrap();
            if let Ok(done) = p.handle.join() {
                self.apply_traffic(done, &p.lot_map);
            }
            return;
        }
        if self.pending.is_none()
            && self.day % TRAFFIC_EVERY_DAYS == TRAFFIC_OFFSET
            && self.built_count() >= MIN_BUILT_FOR_TRAFFIC
        {
            self.start_traffic();
        }
    }

    fn start_traffic(&mut self) {
        let mut t = self.town.clone();
        t.roads = (0..self.runs.len())
            .filter(|&i| self.run_visible_idx(i))
            .map(|i| Road {
                class: self.runs[i].class,
                points: self.runs[i].points.clone(),
            })
            .collect();
        t.lots.clear();
        let mut lot_map = Vec::new();
        for (i, l) in self.lots.iter().enumerate() {
            if !matches!(l.phase, Phase::Built { .. }) {
                continue;
            }
            let src = self.town.lots.get(l.src);
            t.lots.push(Lot {
                centre: l.pos,
                radius: l.radius,
                kind: demand_kind(l),
                facing: l.facing,
                zone: src.map_or(Zone::Inner, |s| s.zone),
                variant: src.map_or(0.5, |s| s.variant),
            });
            lot_map.push(i);
        }
        let runs: Vec<(RoadClass, Vec<Pt>)> = self
            .runs
            .iter()
            .map(|r| (r.class, r.points.clone()))
            .collect();
        let visible: Vec<bool> = (0..self.runs.len())
            .map(|i| self.run_visible_idx(i))
            .collect();
        let handle = std::thread::Builder::new()
            .name("us-traffic".into())
            .spawn(move || run_traffic(t, runs, visible))
            .expect("start traffic thread");
        self.pending = Some(Pending {
            handle,
            join_day: self.day + TRAFFIC_LAG_DAYS,
            lot_map,
        });
    }

    fn apply_traffic(&mut self, done: Done, lot_map: &[usize]) {
        if done.run_traffic.len() == self.runs.len() {
            self.run_traffic = done.run_traffic;
        }
        self.avg_cong = done.avg_cong;
        let (mut widened, mut widened_m, mut built, mut built_m, mut gone) = (0, 0.0, 0, 0.0, 0);
        let mut first: Option<Pt> = None;
        for op in done.ops {
            let lots = match &op {
                Op::Upgrade { lots, .. } | Op::Connector { lots, .. } => lots.clone(),
            };
            let changed = match op {
                Op::Upgrade { pts, to, .. } => self.apply_upgrade(&pts, to).map(|len| {
                    widened += 1;
                    widened_m += len;
                    pts[pts.len() / 2]
                }),
                Op::Connector { pa, pb, class, .. } => {
                    self.apply_connector(pa, pb, class);
                    built += 1;
                    built_m += pa.dist(pb);
                    Some(pa.lerp(pb, 0.5))
                }
            };
            let Some(at) = changed else { continue };
            first.get_or_insert(at);
            for li in lots {
                let Some(&i) = lot_map.get(li as usize) else {
                    continue;
                };
                if matches!(self.lots[i].phase, Phase::Built { .. }) && !self.lots[i].exempt {
                    self.retire_lot(i);
                    gone += 1;
                }
            }
        }
        if widened + built > 0 {
            self.roads_version += 1;
            self.net_dirty = true;
            if widened > 0 {
                self.log(
                    EventKind::Road,
                    format!(
                        "{widened} street{} widened for traffic ({:.0} m).",
                        if widened == 1 { "" } else { "s" },
                        widened_m
                    ),
                    first,
                );
            }
            if built > 0 {
                self.log(
                    EventKind::Road,
                    format!(
                        "{built} new connecting road{} opened ({:.0} m).",
                        if built == 1 { "" } else { "s" },
                        built_m
                    ),
                    first,
                );
            }
            if gone > 0 {
                self.log(
                    EventKind::Road,
                    format!(
                        "{gone} building{} cleared for new road.",
                        if gone == 1 { "" } else { "s" }
                    ),
                    first,
                );
            }
        }
        self.rebuild_traffic_cells();
        self.recompute_values();
    }

    /// Take a lot out of play for good (cleared for a road or a bigger building).
    pub(crate) fn retire_lot(&mut self, i: usize) {
        self.lots[i].phase = Phase::Hidden;
        self.lots[i].addition = usize::MAX;
        self.vacant.retain(|&v| v != i);
        self.changed_lots.push(i);
    }

    /// Nearest drivable run segment to `p` within `tol`: (run, segment).
    fn nearest_run_seg(&self, p: Pt, tol: f64) -> Option<(usize, usize)> {
        let grid = SegGrid::from_lines(self.runs.iter().map(|r| r.points.as_slice()));
        let mut best: Option<(f64, usize, usize)> = None;
        for (r, s) in grid.near(p, p, tol) {
            let (r, s) = (r as usize, s as usize);
            let run = &self.runs[r];
            if matches!(run.class, RoadClass::Rail | RoadClass::Alley) || !self.run_visible_idx(r) {
                continue;
            }
            let d = dist_pt_seg(p, run.points[s], run.points[s + 1]);
            if d <= tol && best.is_none_or(|b| d < b.0) {
                best = Some((d, r, s));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Widen the stretch of road along `pts` to `to`. Returns its length in metres.
    fn apply_upgrade(&mut self, pts: &[Pt], to: RoadClass) -> Option<f64> {
        let mid = pts[pts.len() / 2];
        let (r, _) = self.nearest_run_seg(mid, 3.0)?;
        let run = self.runs[r].clone();
        if run.class == to {
            return None;
        }
        let nearest_vertex = |p: Pt, fallback: usize| -> usize {
            let (mut bi, mut bd) = (fallback, 2.5);
            for (i, q) in run.points.iter().enumerate() {
                let d = q.dist(p);
                if d < bd {
                    bd = d;
                    bi = i;
                }
            }
            bi
        };
        let last = run.points.len() - 1;
        let a = nearest_vertex(pts[0], 0);
        let b = nearest_vertex(*pts.last().unwrap(), last);
        let (i0, i1) = (a.min(b), a.max(b));
        if i0 == i1 {
            return None;
        }
        let slot = run.addition.unwrap_or(self.revealed.len());
        let len = polyline_len(&run.points[i0..=i1]);
        let delta = (len * (road_cost_per_m(to) - road_cost_per_m(run.class)) * 3.0) as i64;
        self.road_cost_per_month[slot] += delta;
        let tr = self.run_traffic.get(r).copied().unwrap_or_default();
        let mut pieces: Vec<(RoadClass, Vec<Pt>)> = Vec::new();
        if i0 > 0 {
            pieces.push((run.class, run.points[..=i0].to_vec()));
        }
        pieces.push((to, run.points[i0..=i1].to_vec()));
        if i1 < last {
            pieces.push((run.class, run.points[i1..].to_vec()));
        }
        let n = pieces.len();
        self.runs.splice(
            r..=r,
            pieces.into_iter().map(|(class, points)| RoadRun {
                class,
                points,
                addition: run.addition,
            }),
        );
        self.run_traffic.splice(r..=r, std::iter::repeat_n(tr, n));
        Some(len)
    }

    /// Add a straight connector, putting a vertex into the roads it lands on.
    fn apply_connector(&mut self, pa: Pt, pb: Pt, class: RoadClass) {
        for p in [pa, pb] {
            if let Some((r, s)) = self.nearest_run_seg(p, 2.0) {
                let (a, b) = (self.runs[r].points[s], self.runs[r].points[s + 1]);
                if a.dist(p) > 0.5 && b.dist(p) > 0.5 {
                    self.runs[r].points.insert(s + 1, p);
                }
            }
        }
        let n_add = self.revealed.len();
        self.road_cost_per_month[n_add] += (pa.dist(pb) * road_cost_per_m(class) * 3.0) as i64;
        self.runs.push(RoadRun {
            class,
            points: vec![pa, pb],
            addition: None,
        });
        self.run_traffic.push(RoadTraffic::default());
        // keep the hidden, never-built land clear of the new road
        let hw = class.half_width();
        for i in 0..self.lots.len() {
            if matches!(self.lots[i].phase, Phase::Hidden | Phase::Vacant)
                && self.lots[i].addition != usize::MAX
                && dist_pt_seg(self.lots[i].pos, pa, pb) < hw + self.lots[i].radius
            {
                self.retire_lot(i);
            }
        }
    }

    fn rebuild_traffic_cells(&mut self) {
        let n = (self.town.size / CELL).ceil() as usize + 1;
        let mut cells = vec![0.0f32; n * n];
        for (r, run) in self.runs.iter().enumerate() {
            let c = self.run_traffic.get(r).map_or(0.0, |t| t.congestion());
            if c <= 0.0 || !self.run_visible_idx(r) {
                continue;
            }
            for w in run.points.windows(2) {
                let steps = (w[0].dist(w[1]) / (CELL / 2.0)).ceil().max(1.0) as usize;
                for k in 0..=steps {
                    let p = w[0].lerp(w[1], k as f64 / steps as f64);
                    let (cx, cy) = (
                        ((p.x / CELL) as usize).min(n - 1),
                        ((p.y / CELL) as usize).min(n - 1),
                    );
                    let v = &mut cells[cy * n + cx];
                    *v = v.max(c);
                }
            }
        }
        self.traffic_cells = cells;
    }

    /// Congestion 0 to 1 of the busiest road within about 40 m of a lot.
    pub(crate) fn lot_traffic(&self, lot: usize) -> f64 {
        if self.traffic_cells.is_empty() {
            return 0.0;
        }
        let n = (self.town.size / CELL).ceil() as i64 + 1;
        let p = self.lots[lot].pos;
        let (cx, cy) = ((p.x / CELL) as i64, (p.y / CELL) as i64);
        let mut m = 0.0f32;
        for y in (cy - 1).max(0)..=(cy + 1).min(n - 1) {
            for x in (cx - 1).max(0)..=(cx + 1).min(n - 1) {
                m = m.max(self.traffic_cells[(y * n + x) as usize]);
            }
        }
        m as f64
    }

    pub(crate) fn avg_congestion(&self) -> f32 {
        self.avg_cong
    }
}

/// The worker: assign traffic, choose changes, map the results onto the road runs.
fn run_traffic(town: Town, runs: Vec<(RoadClass, Vec<Pt>)>, visible: Vec<bool>) -> Done {
    let p = grow_params();
    let plan = plan_round(&town, &p, &RuleProposer, &TrafficScorer, 0, TRAFFIC_SCALE);
    let edges = &plan.eval.net.streets.edges;
    let grid = SegGrid::from_lines(runs.iter().map(|r| r.1.as_slice()));
    let mut rt = vec![RoadTraffic::default(); runs.len()];
    let (mut wsum, mut lsum) = (0.0f64, 0.0f64);
    for (i, e) in edges.iter().enumerate() {
        let vc = plan.eval.assignment.edge_vc[i];
        let vol = plan.eval.assignment.edge_volume[i];
        wsum += e.len * (vc.clamp(0.0, 1.5) / 1.5);
        lsum += e.len;
        let mid = e.pts[e.pts.len() / 2];
        let mut best: Option<(f64, usize)> = None;
        for (r, s) in grid.near(mid, mid, 3.0) {
            let (r, s) = (r as usize, s as usize);
            if !visible[r] || matches!(runs[r].0, RoadClass::Rail | RoadClass::Alley) {
                continue;
            }
            let d = dist_pt_seg(mid, runs[r].1[s], runs[r].1[s + 1]);
            if d <= 3.0 && best.is_none_or(|b| d < b.0) {
                best = Some((d, r));
            }
        }
        if let Some((_, r)) = best {
            rt[r].vc = rt[r].vc.max(vc as f32);
            rt[r].volume = rt[r].volume.max(vol as f32);
        }
    }
    let mut ops = Vec::new();
    for c in &plan.changes {
        match c {
            Change::Upgrade {
                street,
                to,
                lots_removed,
            } => ops.push(Op::Upgrade {
                pts: edges[*street as usize].pts.clone(),
                to: *to,
                lots: lots_removed.clone(),
            }),
            Change::Connector {
                a,
                b,
                pa,
                pb,
                class,
                lots_removed,
            } => {
                let _: (&End, &End) = (a, b);
                ops.push(Op::Connector {
                    pa: *pa,
                    pb: *pb,
                    class: *class,
                    lots: lots_removed.clone(),
                })
            }
        }
    }
    Done {
        run_traffic: rt,
        avg_cong: if lsum > 0.0 {
            (wsum / lsum) as f32
        } else {
            0.0
        },
        ops,
    }
}
