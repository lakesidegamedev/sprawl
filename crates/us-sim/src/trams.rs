//! Tram lines on the transit stops.
//!
//! The mayor links two or more built transit stops into a line. The line runs along the shortest
//! path of open arterials and collectors between the stops, is paid for per metre when it
//! starts, is built in stages (track, then wires) and then runs both ways. Open lines move a
//! share of the trips off the streets they follow, lift land value near their stops, bring a few
//! more people to town, and cost upkeep every month.
//!
//! State is saved in the `TRAM` section. Old saves have none and start with no lines.

use crate::save::{LoadError, R, Res, W};
use crate::*;
use us_metrics::graph;

/// Most lines a town can have.
pub const MAX_LINES: usize = 4;
/// Most stops on one line.
pub const MAX_STOPS: usize = 6;
/// Dollars per metre of track and wire.
pub const COST_PER_M: f64 = 520.0;
/// Dollars per metre per month once open.
pub const UPKEEP_PER_M: f64 = 2.2;
const MIN_LEN: f64 = 80.0;
const MAX_LEN: f64 = 4000.0;
/// Two stops of a line must be at least this far apart, metres.
const MIN_STOP_GAP: f64 = 60.0;
/// A stop must be within this of an open arterial or collector, metres.
const SNAP_M: f64 = 80.0;
/// Share of the build time spent laying track; the rest is wires.
const TRACK_SHARE: f64 = 0.55;
/// Land value of a lot at a stop of an open line, fading to nothing at `VALUE_REACH_M`.
pub const W_TRAM: f64 = 0.02;
pub const VALUE_REACH_M: f64 = 350.0;
/// Streets within this of the track count as the line's street, metres.
const ALONG_M: f64 = 14.0;

/// How far a line is built.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TramState {
    /// Laying track.
    Track,
    /// Hanging wire.
    Wires,
    Open,
}

#[derive(Clone, Debug)]
pub(crate) struct TramLine {
    /// Lot indices of the transit stops, in order.
    pub stops: Vec<u32>,
    pub points: Vec<Pt>,
    /// Index into `points` of each stop's place on the track.
    pub stop_vert: Vec<u32>,
    pub length: f64,
    pub cost: i64,
    pub upkeep: i64,
    pub start: i32,
    pub open: i32,
    /// The "opened" news has been written.
    pub announced: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Trams {
    pub lines: Vec<TramLine>,
    /// Bumped when a line is added or removed or changes stage, so a renderer redraws.
    pub version: u32,
    /// Sum of the stages of all lines at the last day step (not saved).
    pub stage_sum: u32,
}

/// A line as the app draws and lists it.
#[derive(Clone, Debug)]
pub struct TramLineView {
    pub name: String,
    /// The track, from the first stop to the last.
    pub points: Vec<Pt>,
    /// Where each stop is on the track.
    pub stops: Vec<Pt>,
    /// Distance along the track of each stop, metres.
    pub stop_at: Vec<f64>,
    pub length: f64,
    pub cost: i64,
    pub upkeep: i64,
    pub state: TramState,
    /// 0 to 1 within the current stage.
    pub progress: f32,
    pub open_day: i32,
}

/// What a line would cost and where it would run, for the window and the ghost.
#[derive(Clone, Debug)]
pub struct TramPlan {
    pub points: Vec<Pt>,
    pub stops: Vec<Pt>,
    pub length: f64,
    pub cost: i64,
    pub upkeep: i64,
    /// Days to build.
    pub days: i32,
    stop_vert: Vec<u32>,
}

/// Why a line cannot be built.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TramError {
    NeedsStops,
    TooManyStops,
    TooManyLines,
    /// The lot is not a transit stop that is finished.
    NotAStop,
    /// The stop is not beside an open arterial or collector.
    NotOnRoad,
    TooClose,
    /// No path of arterials and collectors joins two stops.
    NoRoute,
    TooShort,
    TooLong,
    TooPoor {
        cost: i64,
    },
    NoLine,
}

impl TramError {
    pub fn text(self) -> String {
        match self {
            TramError::NeedsStops => "Pick at least two transit stops.".into(),
            TramError::TooManyStops => format!("A line has at most {MAX_STOPS} stops."),
            TramError::TooManyLines => format!("A town can have {MAX_LINES} tram lines."),
            TramError::NotAStop => "Only finished transit stops can be on a line.".into(),
            TramError::NotOnRoad => "A stop is not beside an open arterial or collector.".into(),
            TramError::TooClose => "Two stops in a row are too close together.".into(),
            TramError::NoRoute => {
                "No arterials or collectors join those stops. Build a road between them.".into()
            }
            TramError::TooShort => "The line is too short.".into(),
            TramError::TooLong => "The line is too long. Build it in two lines.".into(),
            TramError::TooPoor { cost } => format!(
                "Not enough money: it costs ${}.",
                crate::milestones::group(cost)
            ),
            TramError::NoLine => "There is no such line.".into(),
        }
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

fn project(p: Pt, a: Pt, b: Pt) -> Pt {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    pt(a.x + abx * t, a.y + aby * t)
}

/// Trams per game hour in each direction for a town of `pop` people with the roads budget at
/// `budget_pct` percent. A few in a small town, up to eight in a big one; a starved budget cuts
/// it, and a budget above 100 does not raise it.
pub fn frequency_for(pop: i64, budget_pct: i64) -> f32 {
    let base = (2.0 + pop.max(0) as f32 / 2500.0).min(8.0);
    base * (budget_pct as f32 / 100.0).clamp(0.4, 1.0)
}

/// Share of trips on a line's own streets that move to the tram at `freq` trams per hour.
pub fn share_for(freq: f32) -> f64 {
    0.05 + 0.20 * (freq / 8.0).clamp(0.0, 1.0) as f64
}

/// The street graph the tracks follow: open arterials and collectors.
struct Net {
    g: graph::Graph,
    adj: Vec<Vec<(u32, f64)>>,
}

impl Net {
    /// Nearest fine edge to `p`: (edge index, point on it, distance).
    fn snap(&self, p: Pt) -> Option<(usize, Pt, f64)> {
        let mut best: Option<(usize, Pt, f64)> = None;
        for (i, e) in self.g.edges.iter().enumerate() {
            let (a, b) = (self.g.nodes[e.a as usize], self.g.nodes[e.b as usize]);
            let q = project(p, a, b);
            let d = q.dist(p);
            if best.is_none_or(|x| d < x.2) {
                best = Some((i, q, d));
            }
        }
        best
    }

    /// Shortest path of points from `pa` (on edge `ea`) to `pb` (on edge `eb`).
    fn route(&self, ea: usize, pa: Pt, eb: usize, pb: Pt) -> Option<Vec<Pt>> {
        if ea == eb {
            return Some(vec![pa, pb]);
        }
        let n = self.g.nodes.len();
        let mut dist = vec![f64::MAX; n];
        let mut prev = vec![u32::MAX; n];
        let mut heap = std::collections::BinaryHeap::new();
        let key = |d: f64| std::cmp::Reverse((d * 100.0) as u64);
        let (ea_e, eb_e) = (self.g.edges[ea], self.g.edges[eb]);
        for s in [ea_e.a, ea_e.b] {
            let d = self.g.nodes[s as usize].dist(pa);
            if d < dist[s as usize] {
                dist[s as usize] = d;
                heap.push((key(d), std::cmp::Reverse(s)));
            }
        }
        while let Some((k, std::cmp::Reverse(u))) = heap.pop() {
            let d = k.0 as f64 / 100.0;
            if d > dist[u as usize] + 0.011 {
                continue;
            }
            for &(v, len) in &self.adj[u as usize] {
                let nd = dist[u as usize] + len;
                if nd < dist[v as usize] {
                    dist[v as usize] = nd;
                    prev[v as usize] = u;
                    heap.push((key(nd), std::cmp::Reverse(v)));
                }
            }
        }
        let mut best: Option<(f64, u32)> = None;
        for t in [eb_e.a, eb_e.b] {
            if dist[t as usize] == f64::MAX {
                continue;
            }
            let total = dist[t as usize] + self.g.nodes[t as usize].dist(pb);
            if best.is_none_or(|b| total < b.0) {
                best = Some((total, t));
            }
        }
        let (_, t) = best?;
        let mut nodes = vec![t];
        let mut cur = t;
        while prev[cur as usize] != u32::MAX {
            cur = prev[cur as usize];
            nodes.push(cur);
        }
        nodes.reverse();
        let mut out = vec![pa];
        out.extend(nodes.iter().map(|&i| self.g.nodes[i as usize]));
        out.push(pb);
        Some(out)
    }
}

fn push_dedup(out: &mut Vec<Pt>, p: Pt) {
    if out.last().is_none_or(|l| l.dist(p) > 0.05) {
        out.push(p);
    }
}

impl Sim {
    fn tram_net(&self) -> Net {
        let lines: Vec<(RoadClass, Vec<Pt>)> = self
            .runs
            .iter()
            .enumerate()
            .filter(|(i, r)| {
                matches!(r.class, RoadClass::Arterial | RoadClass::Collector) && self.road_open(*i)
            })
            .map(|(_, r)| (r.class, r.points.clone()))
            .collect();
        let g = graph::build(&lines, 0.5, true);
        let mut adj = vec![Vec::new(); g.nodes.len()];
        for e in &g.edges {
            adj[e.a as usize].push((e.b, e.len));
            adj[e.b as usize].push((e.a, e.len));
        }
        Net { g, adj }
    }

    /// Finished transit stops: (lot index, position).
    pub fn tram_stops(&self) -> Vec<(usize, Pt)> {
        self.lots
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                l.civic == Some(CivicKind::Transit) && matches!(l.phase, Phase::Built { .. })
            })
            .map(|(i, l)| (i, l.pos))
            .collect()
    }

    /// Work out a line through the transit stops `stops` (lot indices, in order) without
    /// building it.
    pub fn check_tram(&self, stops: &[usize]) -> Result<TramPlan, TramError> {
        if stops.len() < 2 {
            return Err(TramError::NeedsStops);
        }
        if stops.len() > MAX_STOPS {
            return Err(TramError::TooManyStops);
        }
        let mut at: Vec<Pt> = Vec::new();
        for &s in stops {
            let ok = self.lots.get(s).is_some_and(|l| {
                l.civic == Some(CivicKind::Transit) && matches!(l.phase, Phase::Built { .. })
            });
            if !ok {
                return Err(TramError::NotAStop);
            }
            at.push(self.lots[s].pos);
        }
        for w in at.windows(2) {
            if w[0].dist(w[1]) < MIN_STOP_GAP {
                return Err(TramError::TooClose);
            }
        }
        let net = self.tram_net();
        let mut snaps = Vec::new();
        for &p in &at {
            match net.snap(p) {
                Some(s) if s.2 <= SNAP_M => snaps.push(s),
                _ => return Err(TramError::NotOnRoad),
            }
        }
        let mut points: Vec<Pt> = Vec::new();
        let mut stop_vert: Vec<u32> = Vec::new();
        for (k, s) in snaps.iter().enumerate() {
            if k == 0 {
                push_dedup(&mut points, s.1);
                stop_vert.push(0);
                continue;
            }
            let p = &snaps[k - 1];
            let leg = net.route(p.0, p.1, s.0, s.1).ok_or(TramError::NoRoute)?;
            for q in leg {
                push_dedup(&mut points, q);
            }
            stop_vert.push(points.len() as u32 - 1);
        }
        let length = path_len(&points);
        if length < MIN_LEN {
            return Err(TramError::TooShort);
        }
        if length > MAX_LEN {
            return Err(TramError::TooLong);
        }
        let cost = (length * COST_PER_M) as i64;
        if self.trams.lines.len() >= MAX_LINES {
            return Err(TramError::TooManyLines);
        }
        if !self.can_afford(cost) {
            return Err(TramError::TooPoor { cost });
        }
        Ok(TramPlan {
            stops: stop_vert.iter().map(|&v| points[v as usize]).collect(),
            points,
            length,
            cost,
            upkeep: (length * UPKEEP_PER_M) as i64,
            days: 30 + (length / 25.0) as i32,
            stop_vert,
        })
    }

    /// Build a line through the transit stops `stops`. Returns the line's index.
    pub fn build_tram(&mut self, stops: &[usize]) -> Result<usize, TramError> {
        let plan = self.check_tram(stops)?;
        self.charge(plan.cost);
        let line = TramLine {
            stops: stops.iter().map(|&s| s as u32).collect(),
            points: plan.points,
            stop_vert: plan.stop_vert,
            length: plan.length,
            cost: plan.cost,
            upkeep: plan.upkeep,
            start: self.day,
            open: self.day + plan.days,
            announced: false,
        };
        let at = line.points[0];
        let n = self.trams.lines.len() + 1;
        self.trams.lines.push(line);
        self.trams.version += 1;
        self.log(
            EventKind::Road,
            format!("Work started on tram line {n}."),
            Some(at),
        );
        Ok(n - 1)
    }

    /// Close a line. Nothing is paid back.
    pub fn remove_tram(&mut self, line: usize) -> Result<(), TramError> {
        if line >= self.trams.lines.len() {
            return Err(TramError::NoLine);
        }
        let was_open = self.trams.lines[line].announced;
        self.trams.lines.remove(line);
        self.trams.version += 1;
        if was_open {
            self.tram_changed();
        }
        Ok(())
    }

    fn tram_state_of(&self, l: &TramLine) -> (TramState, f32) {
        let span = (l.open - l.start).max(1) as f64;
        let p = ((self.day - l.start) as f64 / span).clamp(0.0, 1.0);
        if self.day >= l.open {
            (TramState::Open, 1.0)
        } else if p < TRACK_SHARE {
            (TramState::Track, (p / TRACK_SHARE) as f32)
        } else {
            (
                TramState::Wires,
                ((p - TRACK_SHARE) / (1.0 - TRACK_SHARE)) as f32,
            )
        }
    }

    /// The lines, for the window and the renderer.
    pub fn tram_lines(&self) -> Vec<TramLineView> {
        self.trams
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let (state, progress) = self.tram_state_of(l);
                let mut at = Vec::new();
                let mut acc = 0.0;
                let mut vert = 0;
                for (k, p) in l.points.iter().enumerate() {
                    if k > 0 {
                        acc += l.points[k - 1].dist(*p);
                    }
                    while vert < l.stop_vert.len() && l.stop_vert[vert] as usize == k {
                        at.push(acc);
                        vert += 1;
                    }
                }
                TramLineView {
                    name: format!("Line {}", i + 1),
                    points: l.points.clone(),
                    stops: l.stop_vert.iter().map(|&v| l.points[v as usize]).collect(),
                    stop_at: at,
                    length: l.length,
                    cost: l.cost,
                    upkeep: l.upkeep,
                    state,
                    progress,
                    open_day: l.open,
                }
            })
            .collect()
    }

    /// Bumped when a line is added, removed or changes stage.
    pub fn trams_version(&self) -> u32 {
        self.trams.version
    }

    fn tram_open(&self, l: &TramLine) -> bool {
        self.day >= l.open
    }

    /// Number of lines that are running.
    pub fn trams_open(&self) -> usize {
        self.trams
            .lines
            .iter()
            .filter(|l| self.tram_open(l))
            .count()
    }

    /// Trams per game hour in each direction on every open line, today.
    pub fn tram_rate(&self) -> f32 {
        frequency_for(self.population(), self.budget[Dept::Roads.index()] as i64)
    }

    /// Dollars per month for the lines that are running.
    pub(crate) fn tram_upkeep(&self) -> i64 {
        self.trams
            .lines
            .iter()
            .filter(|l| self.tram_open(l))
            .map(|l| l.upkeep)
            .sum()
    }

    /// Share of the town's congestion the running lines take away.
    pub(crate) fn tram_relief(&self) -> f64 {
        let share = share_for(self.tram_rate());
        let r: f64 = self
            .trams
            .lines
            .iter()
            .filter(|l| self.tram_open(l))
            .map(|l| share * (l.length / 1500.0).min(1.0) * 0.6)
            .sum();
        r.min(0.2)
    }

    /// Added to arrivals by running lines.
    pub(crate) fn tram_arrival_bonus(&self) -> f64 {
        (self.trams_open() as f64 * 0.02).min(0.06)
    }

    /// Added to the demand for shops by running lines.
    pub(crate) fn tram_shop_demand(&self) -> f64 {
        if self.trams_open() > 0 { 0.02 } else { 0.0 }
    }

    fn tram_stop_points(&self) -> Vec<Pt> {
        let mut out: Vec<Pt> = Vec::new();
        for l in self.trams.lines.iter().filter(|l| self.tram_open(l)) {
            for &v in &l.stop_vert {
                out.push(l.points[v as usize]);
            }
        }
        out
    }

    /// Land value from tram stops for every lot, 0 to `W_TRAM`.
    pub(crate) fn tram_bonuses(&self) -> Vec<f64> {
        let stops = self.tram_stop_points();
        if stops.is_empty() {
            return vec![0.0; self.lots.len()];
        }
        self.lots
            .iter()
            .map(|l| {
                if l.phase == Phase::Hidden {
                    return 0.0;
                }
                let d = stops.iter().map(|s| s.dist(l.pos)).fold(f64::MAX, f64::min);
                if d < VALUE_REACH_M {
                    W_TRAM * (1.0 - d / VALUE_REACH_M)
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// The line for the why panel when a tram stop is near.
    pub(crate) fn tram_note(&self, pos: Pt) -> Option<String> {
        let stops = self.tram_stop_points();
        let d = stops.iter().map(|s| s.dist(pos)).fold(f64::MAX, f64::min);
        (d < VALUE_REACH_M).then(|| {
            format!(
                "A tram stop is near, so trips by tram are easy: land value +{:.1}%.",
                W_TRAM * (1.0 - d / VALUE_REACH_M) * 100.0
            )
        })
    }

    /// Whether a point of a road is on the track of a running line.
    pub(crate) fn tram_along(&self, p: Pt) -> bool {
        self.trams
            .lines
            .iter()
            .filter(|l| self.tram_open(l))
            .any(|l| dist_to_path(p, &l.points) < ALONG_M)
    }

    /// Share of the trips on a street under a running line that go by tram, 0 when none runs.
    pub(crate) fn tram_street_share(&self) -> f64 {
        share_for(self.tram_rate())
    }

    pub(crate) fn tram_changed(&mut self) {
        self.rebuild_traffic_cells();
        self.values_stale = true;
    }

    /// Daily: finish lines and say so.
    pub(crate) fn step_trams(&mut self) {
        let day = self.day;
        let mut opened: Vec<(usize, Pt)> = Vec::new();
        for (i, l) in self.trams.lines.iter_mut().enumerate() {
            if !l.announced && day >= l.open {
                l.announced = true;
                opened.push((i, l.points[0]));
            }
        }
        // a stage change is a reason to redraw
        let stage: u32 = self
            .trams
            .lines
            .iter()
            .map(|l| match self.tram_state_of(l).0 {
                TramState::Track => 0,
                TramState::Wires => 1,
                TramState::Open => 2,
            })
            .sum();
        if stage != self.trams.stage_sum {
            self.trams.stage_sum = stage;
            self.trams.version += 1;
        }
        for (i, at) in opened {
            let l = &self.trams.lines[i];
            let text = format!(
                "Tram line {} opened: {:.0} m, {} stops.",
                i + 1,
                l.length,
                l.stops.len()
            );
            self.log(EventKind::Road, text, Some(at));
            self.tram_changed();
        }
    }

    // ----- save -----

    pub(crate) fn save_trams(&self, w: &mut W) {
        w.usize(self.trams.lines.len());
        for l in &self.trams.lines {
            w.usize(l.stops.len());
            for &s in &l.stops {
                w.u32(s);
            }
            w.pts(&l.points);
            for &v in &l.stop_vert {
                w.u32(v);
            }
            w.f64(l.length);
            w.i64(l.cost);
            w.i64(l.upkeep);
            w.i32(l.start);
            w.i32(l.open);
            w.bool(l.announced);
        }
    }

    pub(crate) fn load_trams(&mut self, r: &mut R) -> Res<()> {
        let n = r.len(8)?;
        let mut lines = Vec::with_capacity(n);
        for _ in 0..n {
            let ns = r.len(4)?;
            let mut stops = Vec::with_capacity(ns);
            for _ in 0..ns {
                let s = r.u32()?;
                if s as usize >= self.lots.len() {
                    return Err(LoadError::Invalid("tram stop"));
                }
                stops.push(s);
            }
            let points = r.pts()?;
            let mut stop_vert = Vec::with_capacity(ns);
            for _ in 0..ns {
                let v = r.u32()?;
                if v as usize >= points.len() {
                    return Err(LoadError::Invalid("tram vertex"));
                }
                stop_vert.push(v);
            }
            lines.push(TramLine {
                stops,
                points,
                stop_vert,
                length: r.f64()?,
                cost: r.i64()?,
                upkeep: r.i64()?,
                start: r.i32()?,
                open: r.i32()?,
                announced: r.bool()?,
            });
        }
        self.trams.lines = lines;
        self.trams.version += 1;
        Ok(())
    }

    pub(crate) fn tram_hash(&self, f: &mut impl FnMut(u64)) {
        f(self.trams.lines.len() as u64);
        for l in &self.trams.lines {
            f(l.points.len() as u64);
            f(l.cost as u64);
            f(l.start as u64);
            f(l.open as u64);
            f(l.announced as u64);
            f(l.length.to_bits());
        }
    }
}
