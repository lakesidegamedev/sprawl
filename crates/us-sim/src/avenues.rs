//! Player-built avenues and bridges, and street upgrades.
//!
//! The mayor draws a new arterial or collector from an open road. It is checked point by point
//! (ground, water, railway, buildings), paid for when it starts, built through the same stages as
//! other streets (graded, paved, open) and then joins the traffic network.

use crate::save::{R, Res, W};
use crate::streets::{Street, StreetState, polylines_meet};
use crate::*;

/// Sample spacing along a drawn road, metres.
const STEP: f64 = 4.0;
/// A control point this near an open road snaps onto it, metres.
const SNAP_M: f64 = 10.0;
const MIN_LEN: f64 = 40.0;
const MIN_SEG: f64 = 12.0;
const MAX_LEN: f64 = 1500.0;
const MAX_POINTS: usize = 24;
/// Longest bridge span, metres.
const MAX_BRIDGE_M: f64 = 160.0;
/// Bridge cost per metre is this many times the road cost.
const BRIDGE_FACTOR: f64 = 4.0;
/// Steepest ground by class (rise over run).
const MAX_SLOPE_ARTERIAL: f64 = 0.12;
const MAX_SLOPE_COLLECTOR: f64 = 0.15;
/// Smallest angle to the railway at a crossing, as the sine of the angle.
const MIN_RAIL_SIN: f64 = 0.7;
/// Land value reach of the access gain and of the noise, metres.
const ACCESS_REACH: f64 = 150.0;
const NOISE_REACH: f64 = 35.0;

/// A road the mayor built.
#[derive(Clone, Debug)]
pub(crate) struct Avenue {
    pub street: u32,
    pub class: RoadClass,
    pub len: f64,
    /// Upkeep per month once open.
    pub cost: i64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Avenues {
    pub list: Vec<Avenue>,
    /// Ask for a traffic run soon, because a road opened.
    pub traffic_kick: bool,
}

/// What a road would cost and take, for the ghost and for the build call.
#[derive(Clone, Debug)]
pub struct RoadPlan {
    pub class: RoadClass,
    /// The path with the start (and an end on a road) snapped onto the roads.
    pub points: Vec<Pt>,
    pub length: f64,
    pub bridge_m: f64,
    /// Stretches that are bridges.
    pub bridges: Vec<(Pt, Pt)>,
    pub cost: i64,
    /// Upkeep per month once open.
    pub upkeep: i64,
    /// Empty lots the road takes.
    pub lots_taken: usize,
    give_up: Vec<usize>,
}

/// Why a road cannot go there, with the spot for the ghost to mark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoadError {
    NeedsPoints,
    /// The first point is not on an open road.
    StartNotOnRoad {
        at: Pt,
    },
    OutOfBounds {
        at: Pt,
    },
    TooShort {
        at: Pt,
    },
    TooLong {
        at: Pt,
    },
    TooSharp {
        at: Pt,
    },
    TooSteep {
        at: Pt,
    },
    InWater {
        at: Pt,
    },
    BridgeTooLong {
        at: Pt,
    },
    /// A building, site or the hall stands there.
    Occupied {
        at: Pt,
    },
    BadRailAngle {
        at: Pt,
    },
    NearRail {
        at: Pt,
    },
    CrossesHighway {
        at: Pt,
    },
    CrossesWorks {
        at: Pt,
    },
    AlongRoad {
        at: Pt,
    },
    TooPoor {
        cost: i64,
    },
    /// Upgrade: no street there, or nothing bigger to upgrade to.
    NoStreet,
    NotUpgradable,
    NotOpen,
}

impl RoadError {
    pub fn text(self) -> String {
        match self {
            RoadError::NeedsPoints => "Click points on the map. Start on an open road.".into(),
            RoadError::StartNotOnRoad { .. } => "Start on an open road.".into(),
            RoadError::OutOfBounds { .. } => "That is off the map.".into(),
            RoadError::TooShort { .. } => "Too short. Make each leg at least 12 m.".into(),
            RoadError::TooLong { .. } => "Too long. Build it in parts.".into(),
            RoadError::TooSharp { .. } => "That turn is too sharp.".into(),
            RoadError::TooSteep { .. } => "Too steep for this road.".into(),
            RoadError::InWater { .. } => "A point of the road is in the water.".into(),
            RoadError::BridgeTooLong { .. } => "The water is too wide to bridge here.".into(),
            RoadError::Occupied { .. } => "A building or the hall is in the way.".into(),
            RoadError::BadRailAngle { .. } => "Cross the railway more squarely.".into(),
            RoadError::NearRail { .. } => "Too close to the railway.".into(),
            RoadError::CrossesHighway { .. } => "A highway cannot be crossed.".into(),
            RoadError::CrossesWorks { .. } => "A street is being built there.".into(),
            RoadError::AlongRoad { .. } => "It runs along another road.".into(),
            RoadError::TooPoor { cost } => format!("Not enough money: it costs ${cost}."),
            RoadError::NoStreet => "Click a street to upgrade.".into(),
            RoadError::NotUpgradable => {
                "That street is already the biggest class you can build.".into()
            }
            RoadError::NotOpen => "That street is not open yet.".into(),
        }
    }

    /// The spot to mark on the map.
    pub fn at(self) -> Option<Pt> {
        match self {
            RoadError::StartNotOnRoad { at }
            | RoadError::OutOfBounds { at }
            | RoadError::TooShort { at }
            | RoadError::TooLong { at }
            | RoadError::TooSharp { at }
            | RoadError::TooSteep { at }
            | RoadError::InWater { at }
            | RoadError::BridgeTooLong { at }
            | RoadError::Occupied { at }
            | RoadError::BadRailAngle { at }
            | RoadError::NearRail { at }
            | RoadError::CrossesHighway { at }
            | RoadError::CrossesWorks { at }
            | RoadError::AlongRoad { at } => Some(at),
            _ => None,
        }
    }
}

/// An upgrade of one road run to the next class.
#[derive(Clone, Debug)]
pub struct UpgradePlan {
    pub run: usize,
    pub from: RoadClass,
    pub to: RoadClass,
    pub length: f64,
    pub points: Vec<Pt>,
    pub cost: i64,
    give_up: Vec<usize>,
}

fn build_cost_per_m(c: RoadClass) -> f64 {
    match c {
        RoadClass::Arterial => 420.0,
        _ => 240.0,
    }
}

fn upgrade_cost_per_m(to: RoadClass) -> f64 {
    match to {
        RoadClass::Arterial => 260.0,
        _ => 150.0,
    }
}

fn path_len(p: &[Pt]) -> f64 {
    p.windows(2).map(|w| w[0].dist(w[1])).sum()
}

fn dist_to_path(p: Pt, path: &[Pt]) -> f64 {
    path.windows(2)
        .map(|w| dist_pt_seg(p, w[0], w[1]))
        .fold(f64::MAX, f64::min)
}

fn nearest_on(p: Pt, a: Pt, b: Pt) -> Pt {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    pt(a.x + abx * t, a.y + aby * t)
}

impl Sim {
    /// The open, drivable run segment nearest to `p` within `tol`: (run, segment, point on it).
    fn snap_open_road(&self, p: Pt, tol: f64, allow_highway: bool) -> Option<(usize, usize, Pt)> {
        let mut best: Option<(f64, usize, usize, Pt)> = None;
        for (r, run) in self.runs.iter().enumerate() {
            if matches!(run.class, RoadClass::Rail | RoadClass::Alley)
                || (!allow_highway && run.class == RoadClass::Highway)
                || !self.road_open(r)
            {
                continue;
            }
            for (s, w) in run.points.windows(2).enumerate() {
                let q = nearest_on(p, w[0], w[1]);
                let d = q.dist(p);
                if d <= tol && best.is_none_or(|b| d < b.0) {
                    best = Some((d, r, s, q));
                }
            }
        }
        best.map(|b| (b.1, b.2, b.3))
    }

    /// Check a road drawn through `pts` (the first on an open road) without building it.
    pub fn check_road(&self, class: RoadClass, pts: &[Pt]) -> Result<RoadPlan, RoadError> {
        let class = if class == RoadClass::Arterial {
            class
        } else {
            RoadClass::Collector
        };
        if pts.len() < 2 {
            return Err(RoadError::NeedsPoints);
        }
        if pts.len() > MAX_POINTS {
            return Err(RoadError::TooLong {
                at: *pts.last().unwrap(),
            });
        }
        let size = self.town.size;
        for &p in pts {
            if p.x < 20.0 || p.y < 20.0 || p.x > size - 20.0 || p.y > size - 20.0 {
                return Err(RoadError::OutOfBounds { at: p });
            }
        }
        let Some((_, _, start)) = self.snap_open_road(pts[0], SNAP_M, false) else {
            return Err(RoadError::StartNotOnRoad { at: pts[0] });
        };
        let mut path: Vec<Pt> = pts.to_vec();
        path[0] = start;
        let last = path.len() - 1;
        if let Some((_, _, q)) = self.snap_open_road(path[last], SNAP_M, false) {
            path[last] = q;
        }
        let hw = class.half_width();
        // legs, turns, length
        for w in path.windows(2) {
            if w[0].dist(w[1]) < MIN_SEG {
                return Err(RoadError::TooShort { at: w[1] });
            }
        }
        for w in path.windows(3) {
            let (a, b) = (
                pt(w[1].x - w[0].x, w[1].y - w[0].y),
                pt(w[2].x - w[1].x, w[2].y - w[1].y),
            );
            let cos = (a.x * b.x + a.y * b.y) / (a.x.hypot(a.y) * b.x.hypot(b.y));
            if cos < 0.0 {
                return Err(RoadError::TooSharp { at: w[1] });
            }
        }
        let length = path_len(&path);
        if length < MIN_LEN {
            return Err(RoadError::TooShort { at: path[last] });
        }
        if length > MAX_LEN {
            return Err(RoadError::TooLong { at: path[last] });
        }
        // the hall and the stations
        for w in path.windows(2) {
            if dist_pt_seg(self.hall, w[0], w[1]) < self.town.hall_radius + hw + 4.0 {
                return Err(RoadError::Occupied { at: self.hall });
            }
            for st in &self.town.stations {
                if dist_pt_seg(st.pos, w[0], w[1]) < st.radius + hw + 2.0 {
                    return Err(RoadError::Occupied { at: st.pos });
                }
            }
        }
        // crossings with existing roads
        let mut crossings: Vec<Pt> = Vec::new();
        for (r, run) in self.runs.iter().enumerate() {
            let visible = self.road_visible(r);
            let open = self.road_open(r);
            if !visible {
                continue;
            }
            for c in run.points.windows(2) {
                for (i, w) in path.windows(2).enumerate() {
                    let Some(x) = seg_cross(w[0], w[1], c[0], c[1]) else {
                        continue;
                    };
                    match run.class {
                        RoadClass::Rail => {
                            let (d1, d2) = (
                                pt(w[1].x - w[0].x, w[1].y - w[0].y),
                                pt(c[1].x - c[0].x, c[1].y - c[0].y),
                            );
                            let sin = (d1.cross(d2) / (d1.x.hypot(d1.y) * d2.x.hypot(d2.y))).abs();
                            if sin < MIN_RAIL_SIN {
                                return Err(RoadError::BadRailAngle { at: x });
                            }
                        }
                        RoadClass::Highway => return Err(RoadError::CrossesHighway { at: x }),
                        _ if !open => return Err(RoadError::CrossesWorks { at: x }),
                        _ => {}
                    }
                    let _ = i;
                    crossings.push(x);
                }
            }
        }
        // sample the whole path
        let mut cost = 0.0f64;
        let (mut bridge_m, mut span) = (0.0f64, 0.0f64);
        let mut bridges: Vec<(Pt, Pt)> = Vec::new();
        let mut span_start: Option<Pt> = None;
        let mut prev_pt = path[0];
        let per_m = build_cost_per_m(class);
        let max_slope = if class == RoadClass::Arterial {
            MAX_SLOPE_ARTERIAL
        } else {
            MAX_SLOPE_COLLECTOR
        };
        let total = length;
        let mut walked = 0.0f64;
        for w in path.windows(2) {
            let n = (w[0].dist(w[1]) / STEP).ceil().max(1.0) as usize;
            for k in 0..=n {
                let p = w[0].lerp(w[1], k as f64 / n as f64);
                let at_end = walked < 16.0 || total - walked < 16.0;
                let near_cross = crossings.iter().any(|c| c.dist(p) < 16.0);
                let water = self.town.water.iter().any(|ring| point_in_ring(p, ring));
                if water {
                    if span_start.is_none() {
                        span_start = Some(prev_pt);
                    }
                    span += STEP;
                    if span > MAX_BRIDGE_M {
                        return Err(RoadError::BridgeTooLong { at: p });
                    }
                    if k == 0 || k == n {
                        return Err(RoadError::InWater { at: p });
                    }
                } else if let Some(a) = span_start.take() {
                    bridges.push((a, p));
                    bridge_m += span + STEP;
                    span = 0.0;
                }
                if !water {
                    if self.town.slope_at(p) > max_slope {
                        return Err(RoadError::TooSteep { at: p });
                    }
                    if !near_cross {
                        let near_rail = self.runs.iter().enumerate().any(|(ri, run)| {
                            run.class == RoadClass::Rail
                                && self.road_visible(ri)
                                && dist_to_path(p, &run.points) < hw + 5.0
                        });
                        if near_rail {
                            return Err(RoadError::NearRail { at: p });
                        }
                    }
                }
                if !at_end && !near_cross {
                    let along = self.runs.iter().enumerate().any(|(ri, run)| {
                        !matches!(run.class, RoadClass::Rail)
                            && self.road_visible(ri)
                            && dist_to_path(p, &run.points) < hw + run.class.half_width() - 1.0
                    });
                    if along {
                        return Err(RoadError::AlongRoad { at: p });
                    }
                }
                if k > 0 {
                    let m = prev_pt.dist(p);
                    let slope_f = 1.0 + 3.0 * self.town.slope_at(p).min(0.2);
                    cost += m * per_m * if water { BRIDGE_FACTOR } else { slope_f };
                }
                prev_pt = p;
            }
            walked += w[0].dist(w[1]);
        }
        if let Some(a) = span_start {
            // ended on water
            return Err(RoadError::InWater { at: a });
        }
        // buildings and lots
        let mut give_up = Vec::new();
        for (i, l) in self.lots.iter().enumerate() {
            if l.phase == Phase::Hidden && l.addition == usize::MAX {
                continue;
            }
            if dist_to_path(l.pos, &path) < hw + l.radius {
                match l.phase {
                    Phase::Hidden | Phase::Vacant | Phase::Cleared { .. } => give_up.push(i),
                    _ => return Err(RoadError::Occupied { at: l.pos }),
                }
            }
        }
        let cost = (cost as i64) * self.rules.difficulty.cost_pct() / 100;
        if !self.can_afford(cost) {
            return Err(RoadError::TooPoor { cost });
        }
        let upkeep = ((length - bridge_m) * road_cost_per_m(class) * 6.0
            + bridge_m * road_cost_per_m(class) * 6.0 * 2.0) as i64;
        Ok(RoadPlan {
            class,
            points: path,
            length,
            bridge_m,
            bridges,
            cost,
            upkeep,
            lots_taken: give_up.len(),
            give_up,
        })
    }

    /// Build the road: pay now, start roadworks, join it to the roads it meets. Returns the new
    /// road's index in `road_runs()`.
    pub fn build_road(&mut self, class: RoadClass, pts: &[Pt]) -> Result<usize, RoadError> {
        let plan = self.check_road(class, pts)?;
        self.charge(plan.cost);
        let path = self.join_road(&plan.points);
        for &i in &plan.give_up {
            self.retire_lot(i);
        }
        let days = (21.0 + plan.length / 15.0 + plan.bridge_m / 4.0).min(84.0) as i32;
        let street = self.streets.len() as u32;
        self.streets.push(Street {
            state: StreetState::Building {
                start: self.day,
                end: self.day + days,
            },
            addition: None,
            class: plan.class,
            mid: path[path.len() / 2],
            len: plan.length,
            cost: plan.upkeep,
            adj: Vec::new(),
            lots: Vec::new(),
        });
        self.runs.push(RoadRun {
            class: plan.class,
            points: path,
            addition: None,
            street,
        });
        self.run_traffic.push(RoadTraffic::default());
        self.avenues.list.push(Avenue {
            street,
            class: plan.class,
            len: plan.length,
            cost: plan.upkeep,
        });
        self.link_street(street as usize);
        self.roads_version += 1;
        let at = self.runs.last().unwrap().points[0];
        let bridge = if plan.bridge_m > 0.0 {
            " with a bridge"
        } else {
            ""
        };
        self.log(
            EventKind::Road,
            format!(
                "Work started on a new {}{bridge} ({:.0} m).",
                if plan.class == RoadClass::Arterial {
                    "avenue"
                } else {
                    "collector road"
                },
                plan.length
            ),
            Some(at),
        );
        Ok(self.runs.len() - 1)
    }

    /// Put vertices into the roads the new road meets, and into the new road, so they share
    /// points. Returns the new road's points.
    fn join_road(&mut self, path: &[Pt]) -> Vec<Pt> {
        // (run, segment, point)
        let mut hits: Vec<(usize, usize, Pt)> = Vec::new();
        let mut along: Vec<(usize, f64, Pt)> = Vec::new();
        for (r, run) in self.runs.iter().enumerate() {
            if run.class == RoadClass::Rail || !self.road_visible(r) {
                continue;
            }
            for (s, c) in run.points.windows(2).enumerate() {
                for (i, w) in path.windows(2).enumerate() {
                    if let Some(x) = seg_cross(w[0], w[1], c[0], c[1]) {
                        hits.push((r, s, x));
                        along.push((i, w[0].dist(x), x));
                    }
                }
            }
        }
        // the ends: snapped onto roads already, add their vertices too
        for (idx, p) in [(0usize, path[0]), (path.len() - 2, *path.last().unwrap())] {
            if let Some((r, s, q)) = self.snap_open_road(p, 1.0, false) {
                hits.push((r, s, q));
                along.push((idx, path[idx].dist(q), q));
            }
        }
        // insert from the far end of each run and segment, so earlier indexes stay valid
        hits.sort_by(|a, b| {
            a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then_with(|| {
                let da = self.runs[a.0].points[a.1].dist(a.2);
                let db = self.runs[b.0].points[b.1].dist(b.2);
                db.total_cmp(&da)
            })
        });
        for (r, s, p) in hits {
            let (a, b) = (self.runs[r].points[s], self.runs[r].points[s + 1]);
            if a.dist(p) > 0.5 && b.dist(p) > 0.5 {
                self.runs[r].points.insert(s + 1, p);
            }
        }
        along.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut out = vec![path[0]];
        for i in 0..path.len() - 1 {
            for &(seg, _, x) in &along {
                if seg == i
                    && out.last().is_some_and(|l| l.dist(x) > 0.5)
                    && x.dist(path[i + 1]) > 0.5
                {
                    out.push(x);
                }
            }
            if out.last().is_some_and(|l| l.dist(path[i + 1]) > 0.5) {
                out.push(path[i + 1]);
            }
        }
        out
    }

    /// Which streets a street touches, in both directions (so growth can branch from it).
    pub(crate) fn link_street(&mut self, s: usize) {
        let mine: Vec<Vec<Pt>> = self
            .runs
            .iter()
            .filter(|r| r.street as usize == s)
            .map(|r| r.points.clone())
            .collect();
        let mut adj: Vec<u32> = Vec::new();
        for run in &self.runs {
            let o = run.street as usize;
            if o == s || run.class == RoadClass::Rail {
                continue;
            }
            let meets = mine.iter().any(|m| {
                m.windows(2).any(|a| {
                    run.points
                        .windows(2)
                        .any(|b| polylines_meet(a[0], a[1], b[0], b[1]))
                })
            });
            if meets {
                adj.push(o as u32);
            }
        }
        adj.sort_unstable();
        adj.dedup();
        for &o in &adj {
            let st = &mut self.streets[o as usize];
            if !st.adj.contains(&(s as u32)) {
                st.adj.push(s as u32);
            }
        }
        self.streets[s].adj = adj;
    }

    /// Called when a street opens: the mayor's roads announce themselves and ask for a traffic
    /// run, so traffic reroutes onto them within days instead of at the next quarter.
    pub(crate) fn on_street_opened(&mut self, s: usize) {
        if let Some(a) = self.avenues.list.iter().find(|a| a.street as usize == s) {
            let (class, len) = (a.class, a.len);
            self.avenues.traffic_kick = true;
            self.values_stale = true;
            let at = self.streets[s].mid;
            self.log(
                EventKind::Road,
                format!(
                    "The new {} is open ({:.0} m).",
                    if class == RoadClass::Arterial {
                        "avenue"
                    } else {
                        "collector road"
                    },
                    len
                ),
                Some(at),
            );
        }
    }

    // ----- upgrades -----

    /// Check upgrading the street under `p` to the next class.
    pub fn check_upgrade(&self, p: Pt) -> Result<UpgradePlan, RoadError> {
        let Some((r, _, _)) = self.snap_open_road(p, 8.0, false) else {
            return Err(RoadError::NoStreet);
        };
        let run = &self.runs[r];
        let to = match run.class {
            RoadClass::Local => RoadClass::Collector,
            RoadClass::Collector => RoadClass::Arterial,
            _ => return Err(RoadError::NotUpgradable),
        };
        if !self.road_open(r) {
            return Err(RoadError::NotOpen);
        }
        let hw = to.half_width();
        let length = path_len(&run.points);
        let mut give_up = Vec::new();
        for (i, l) in self.lots.iter().enumerate() {
            if l.phase == Phase::Hidden && l.addition == usize::MAX {
                continue;
            }
            if dist_to_path(l.pos, &run.points) < hw + l.radius - 2.0 {
                match l.phase {
                    Phase::Hidden | Phase::Vacant | Phase::Cleared { .. } => give_up.push(i),
                    _ => return Err(RoadError::Occupied { at: l.pos }),
                }
            }
        }
        let cost =
            (length * upgrade_cost_per_m(to)) as i64 * self.rules.difficulty.cost_pct() / 100;
        if !self.can_afford(cost) {
            return Err(RoadError::TooPoor { cost });
        }
        Ok(UpgradePlan {
            run: r,
            from: run.class,
            to,
            length,
            points: run.points.clone(),
            cost,
            give_up,
        })
    }

    /// Upgrade the street under `p`. It is drawn as roadworks and keeps carrying traffic.
    pub fn upgrade_street(&mut self, p: Pt) -> Result<usize, RoadError> {
        let plan = self.check_upgrade(p)?;
        self.charge(plan.cost);
        for &i in &plan.give_up {
            self.retire_lot(i);
        }
        let r = plan.run;
        let slot = self.runs[r].addition.unwrap_or(self.revealed.len());
        let delta =
            (plan.length * (road_cost_per_m(plan.to) - road_cost_per_m(plan.from)) * 6.0) as i64;
        self.road_cost_per_month[slot] += delta;
        self.runs[r].class = plan.to;
        let st = self.runs[r].street as usize;
        if let Some(s) = self.streets.get_mut(st) {
            s.class = plan.to;
        }
        self.roads_version += 1;
        self.net_dirty = true;
        self.avenues.traffic_kick = true;
        self.values_stale = true;
        let at = plan.points[plan.points.len() / 2];
        self.log(
            EventKind::Road,
            format!(
                "A street is being widened to a {} ({:.0} m).",
                plan.to.name(),
                plan.length
            ),
            Some(at),
        );
        Ok(r)
    }

    // ----- consequences on land -----

    /// Land value change at `p` from the mayor's open roads: easier to reach, less quiet.
    pub(crate) fn avenue_effect(&self, p: Pt) -> (f64, f64) {
        if self.avenues.list.is_empty() {
            return (0.0, 0.0);
        }
        let (mut access, mut noise) = (0.0f64, 0.0f64);
        for a in &self.avenues.list {
            if !self.street_open(a.street) {
                continue;
            }
            let big = if a.class == RoadClass::Arterial {
                1.0
            } else {
                0.6
            };
            for (ri, run) in self.runs.iter().enumerate() {
                if run.street != a.street {
                    continue;
                }
                let d = dist_to_path(p, &run.points);
                if d < ACCESS_REACH {
                    access = access.max(0.05 * big * (1.0 - d / ACCESS_REACH));
                }
                if d < NOISE_REACH {
                    let c = self
                        .run_traffic
                        .get(ri)
                        .map_or(0.0, |t| t.congestion() as f64);
                    noise = noise.max((0.01 + 0.05 * c) * big * (1.0 - d / NOISE_REACH));
                }
            }
        }
        (access, noise)
    }

    /// A line for the why panel when a mayor's road affects the lot.
    pub(crate) fn avenue_note(&self, p: Pt) -> Option<String> {
        let (a, n) = self.avenue_effect(p);
        match (a > 0.004, n > 0.004) {
            (true, true) => {
                Some("A road you built is close: easy to reach, but the traffic is noisy.".into())
            }
            (true, false) => Some("A road you built is near: easier to reach.".into()),
            (false, true) => Some("A road you built passes close: traffic noise.".into()),
            _ => None,
        }
    }

    // ----- save -----

    pub(crate) fn save_avenues(&self, w: &mut W) {
        w.usize(self.avenues.list.len());
        for a in &self.avenues.list {
            w.u32(a.street);
            w.u8(u8::from(a.class == RoadClass::Arterial));
            w.f64(a.len);
            w.i64(a.cost);
        }
        w.bool(self.avenues.traffic_kick);
    }

    pub(crate) fn load_avenues(&mut self, r: &mut R) -> Res<()> {
        let n = r.len(21)?;
        let mut list = Vec::with_capacity(n);
        for _ in 0..n {
            let street = r.u32()?;
            let class = if r.u8()? == 1 {
                RoadClass::Arterial
            } else {
                RoadClass::Collector
            };
            let len = r.f64()?;
            let cost = r.i64()?;
            if street as usize >= self.streets.len() {
                return Err(crate::save::LoadError::Invalid("avenue street"));
            }
            list.push(Avenue {
                street,
                class,
                len,
                cost,
            });
        }
        self.avenues.traffic_kick = r.bool()?;
        for a in &list {
            let s = a.street as usize;
            self.streets[s].class = a.class;
            self.streets[s].len = a.len;
            self.streets[s].cost = a.cost;
            if let Some(run) = self.runs.iter().find(|r| r.street == a.street) {
                self.streets[s].mid = run.points[run.points.len() / 2];
            }
            self.link_street(s);
        }
        self.avenues.list = list;
        Ok(())
    }

    pub(crate) fn avenue_hash(&self, f: &mut impl FnMut(u64)) {
        for a in &self.avenues.list {
            f(a.street as u64);
            f(a.cost as u64);
            f(a.len.to_bits());
        }
        f(self.avenues.traffic_kick as u64);
    }

    /// Roads the mayor has built (open or not).
    pub fn avenues_built(&self) -> usize {
        self.avenues.list.len()
    }
}
