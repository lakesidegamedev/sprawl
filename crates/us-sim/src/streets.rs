//! Streets grow one at a time. The generator plans every street of a neighbourhood up front;
//! the simulation builds them outward from the roads that already stand. A street starts only
//! when the town is running short of building lots, takes a few weeks (graded, then paved, then
//! open), and its lots can be built on only once it is open.

use crate::*;
use us_grow::geom::{SegGrid, dist_pt_seg, polyline_len};

/// A lot with no street in front of it.
pub(crate) const NO_STREET: u32 = u32::MAX;
/// Streets under construction at the same time.
const MAX_BUILDING: usize = 3;
/// Two streets touch where a point of one is this near a segment of the other, metres.
const TOUCH_M: f64 = 2.5;
/// Share of the build time spent grading before paving starts.
const GRADED_SHARE: f64 = 0.45;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreetState {
    /// Planned by the generator, not started: nothing to see yet.
    Planned,
    /// Under construction.
    Building { start: i32, end: i32 },
    /// Finished and carrying traffic.
    Open,
}

/// The two looks of a street under construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadStage {
    /// Bare earth.
    Graded,
    /// Surfaced, not yet open.
    Paved,
}

/// How far along a street under construction is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadBuildState {
    pub stage: RoadStage,
    /// 0 to 1 over the whole job.
    pub progress: f32,
    /// 0 to 1 within the current stage.
    pub stage_progress: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct Street {
    pub state: StreetState,
    pub addition: Option<usize>,
    pub class: RoadClass,
    pub mid: Pt,
    pub len: f64,
    /// Upkeep per month once open.
    pub cost: i64,
    /// Streets it touches.
    pub adj: Vec<u32>,
    /// Lots that front it.
    pub lots: Vec<u32>,
}

impl Street {
    /// A road the traffic model added later: open from the start, no lots of its own.
    pub(crate) fn open_connector(class: RoadClass, a: Pt, b: Pt) -> Street {
        Street {
            state: StreetState::Open,
            addition: None,
            class,
            mid: a.lerp(b, 0.5),
            len: a.dist(b),
            cost: 0,
            adj: Vec::new(),
            lots: Vec::new(),
        }
    }
}

/// Whether segment a-b and segment c-d cross or end within [`TOUCH_M`] of each other.
pub(crate) fn polylines_meet(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    seg_cross(a, b, c, d).is_some()
        || dist_pt_seg(a, c, d) <= TOUCH_M
        || dist_pt_seg(b, c, d) <= TOUCH_M
        || dist_pt_seg(c, a, b) <= TOUCH_M
        || dist_pt_seg(d, a, b) <= TOUCH_M
}

impl Sim {
    /// Streets, who touches whom and which lots front which street, from the planned town.
    pub(crate) fn build_streets(&mut self) {
        let mut streets: Vec<Street> = self
            .runs
            .iter()
            .map(|r| {
                let len = polyline_len(&r.points);
                Street {
                    state: if r.addition.is_none() {
                        StreetState::Open
                    } else {
                        StreetState::Planned
                    },
                    addition: r.addition,
                    class: r.class,
                    mid: r.points[r.points.len() / 2],
                    len,
                    cost: (len * road_cost_per_m(r.class) * 6.0) as i64,
                    adj: Vec::new(),
                    lots: Vec::new(),
                }
            })
            .collect();
        let grid = SegGrid::from_lines(self.runs.iter().map(|r| r.points.as_slice()));
        for (s, run) in self.runs.iter().enumerate() {
            for w in run.points.windows(2) {
                for (r, seg) in grid.near(w[0], w[1], TOUCH_M) {
                    let r = r as usize;
                    if r == s {
                        continue;
                    }
                    let o = &self.runs[r].points;
                    let (c, d) = (o[seg as usize], o[seg as usize + 1]);
                    if polylines_meet(w[0], w[1], c, d) {
                        streets[s].adj.push(r as u32);
                        streets[r].adj.push(s as u32);
                    }
                }
            }
        }
        for st in &mut streets {
            st.adj.sort_unstable();
            st.adj.dedup();
        }
        self.lot_street = vec![NO_STREET; self.lots.len()];
        for (li, l) in self.lots.iter().enumerate() {
            if l.exempt {
                continue;
            }
            let mut best = (f64::MAX, NO_STREET);
            for pad in [60.0, 220.0] {
                for (r, seg) in grid.near(l.pos, l.pos, pad) {
                    let run = &self.runs[r as usize];
                    if matches!(
                        run.class,
                        RoadClass::Rail | RoadClass::Alley | RoadClass::Highway
                    ) {
                        continue;
                    }
                    let d = dist_pt_seg(
                        l.pos,
                        run.points[seg as usize],
                        run.points[seg as usize + 1],
                    );
                    if d < best.0 {
                        best = (d, r);
                    }
                }
                if best.1 != NO_STREET {
                    break;
                }
            }
            if best.1 != NO_STREET {
                self.lot_street[li] = best.1;
                streets[best.1 as usize].lots.push(li as u32);
            }
        }
        self.streets = streets;
    }

    pub(crate) fn street_open(&self, s: u32) -> bool {
        self.streets
            .get(s as usize)
            .is_none_or(|st| st.state == StreetState::Open)
    }

    fn street_visible(&self, s: u32) -> bool {
        self.streets
            .get(s as usize)
            .is_none_or(|st| st.state != StreetState::Planned)
    }

    /// Whether road run `i` is finished and carrying traffic.
    pub fn road_open(&self, i: usize) -> bool {
        self.runs.get(i).is_some_and(|r| self.street_open(r.street))
    }

    /// Whether road run `i` exists on the map yet, finished or under construction. Streets the
    /// town has not started are not visible.
    pub fn road_visible(&self, i: usize) -> bool {
        self.runs
            .get(i)
            .is_some_and(|r| self.street_visible(r.street))
    }

    /// Progress of road run `i` while its street is being built; `None` once it is open and
    /// while it is still only a plan (see `road_visible`).
    pub fn road_build_state(&self, i: usize) -> Option<RoadBuildState> {
        let r = self.runs.get(i)?;
        let StreetState::Building { start, end } = self.streets.get(r.street as usize)?.state
        else {
            return None;
        };
        let p = ((self.day - start) as f64 / (end - start).max(1) as f64).clamp(0.0, 1.0);
        Some(if p < GRADED_SHARE {
            RoadBuildState {
                stage: RoadStage::Graded,
                progress: p as f32,
                stage_progress: (p / GRADED_SHARE) as f32,
            }
        } else {
            RoadBuildState {
                stage: RoadStage::Paved,
                progress: p as f32,
                stage_progress: ((p - GRADED_SHARE) / (1.0 - GRADED_SHARE)) as f32,
            }
        })
    }

    /// Streets under construction now.
    pub fn streets_building(&self) -> usize {
        self.streets
            .iter()
            .filter(|s| matches!(s.state, StreetState::Building { .. }))
            .count()
    }

    /// Streets finished so far (not counting the old roads that stood from day 0).
    pub fn streets_opened(&self) -> usize {
        self.streets
            .iter()
            .filter(|s| s.addition.is_some() && s.state == StreetState::Open)
            .count()
    }

    /// Builders prefer lots on the big streets.
    pub(crate) fn frontage_bonus(&self, lot: usize) -> f64 {
        match self
            .lot_street
            .get(lot)
            .and_then(|&s| self.streets.get(s as usize))
        {
            Some(st) => match st.class {
                RoadClass::Arterial => 70.0,
                RoadClass::Collector => 35.0,
                _ => 0.0,
            },
            None => 0.0,
        }
    }

    /// A neighbourhood is open for growth once its first street is started.
    pub(crate) fn open_addition(&mut self, a: usize) {
        if self.revealed[a] {
            return;
        }
        self.revealed[a] = true;
        self.newly_revealed.push(a);
        self.net_dirty = true;
        for c in &self.town.cul_de_sacs {
            if c.addition == a {
                self.road_cost_per_month[a] += 60;
            }
        }
        if self.day > 0 {
            let c = self.addition_centre(a);
            let n = self.revealed.iter().filter(|&&r| r).count();
            self.log(
                EventKind::Neighborhood,
                format!("A new neighbourhood opened (number {n})."),
                Some(c),
            );
        }
        // lots on streets that already stand can be built at once
        for i in 0..self.lots.len() {
            if self.lots[i].addition == a && self.lots[i].phase == Phase::Hidden {
                let street = self.lot_street.get(i).copied().unwrap_or(NO_STREET);
                if street == NO_STREET || self.street_open(street) {
                    self.release_lot(i);
                }
            }
        }
    }

    fn release_lot(&mut self, i: usize) {
        self.lots[i].phase = Phase::Vacant;
        self.vacant.push(i);
        self.changed_lots.push(i);
    }

    /// Finish a street: it carries traffic, costs upkeep, and its lots become buildable.
    pub(crate) fn open_street(&mut self, s: usize) {
        self.streets[s].state = StreetState::Open;
        self.net_dirty = true;
        self.roads_version += 1;
        let slot = self.streets[s].addition.unwrap_or(self.revealed.len());
        self.road_cost_per_month[slot] += self.streets[s].cost;
        for k in 0..self.streets[s].lots.len() {
            let i = self.streets[s].lots[k] as usize;
            let l = &self.lots[i];
            if l.phase == Phase::Hidden && l.addition != usize::MAX && self.revealed[l.addition] {
                self.release_lot(i);
            }
        }
    }

    fn begin_street(&mut self, s: usize, days: i32) {
        if let Some(a) = self.streets[s].addition {
            self.open_addition(a);
        }
        self.streets[s].state = StreetState::Building {
            start: self.day,
            end: self.day + days,
        };
        self.roads_version += 1;
    }

    /// Days a street takes to build.
    fn build_days(&self, s: usize) -> i32 {
        let st = &self.streets[s];
        let base = match st.class {
            RoadClass::Alley => 7,
            RoadClass::Local => 14,
            _ => 21,
        };
        let extra = (st.len / 18.0) as i32;
        let noise = (roll(self.seed ^ 0x57ee7, s as u64, 3) * 7.0) as i32;
        (base + extra + noise).min(49)
    }

    /// Daily: finish streets that are done, and start another when the town runs short of lots
    /// to build on.
    pub(crate) fn step_streets(&mut self) {
        let day = self.day;
        let (mut building, mut pending) = (0usize, 0usize);
        for s in 0..self.streets.len() {
            if let StreetState::Building { end, .. } = self.streets[s].state {
                if day >= end {
                    self.open_street(s);
                    self.open_alleys_beside(s);
                    continue;
                }
                building += 1;
                pending += self.streets[s]
                    .lots
                    .iter()
                    .filter(|&&i| {
                        let l = &self.lots[i as usize];
                        l.phase == Phase::Hidden && l.addition != usize::MAX
                    })
                    .count();
            }
        }
        if building >= MAX_BUILDING || self.vacant.len() + pending >= RESERVE {
            return;
        }
        // a neighbourhood with no streets of its own opens in its turn
        while let Some(&a) = self.order.get(self.next_reveal) {
            if self.revealed[a] {
                self.next_reveal += 1;
            } else if self.streets.iter().all(|s| s.addition != Some(a)) {
                self.open_addition(a);
                self.next_reveal += 1;
            } else {
                break;
            }
        }
        if let Some(s) = self.pick_street() {
            let days = self.build_days(s);
            self.begin_street(s, days);
        }
    }

    /// Alleys start when the street they run off is finished.
    fn open_alleys_beside(&mut self, s: usize) {
        for k in 0..self.streets[s].adj.len() {
            let n = self.streets[s].adj[k] as usize;
            let st = &self.streets[n];
            if st.class == RoadClass::Alley
                && st.state == StreetState::Planned
                && st.addition.is_some_and(|a| self.revealed[a])
            {
                let days = self.build_days(n);
                self.begin_street(n, days);
            }
        }
    }

    /// The next street to build: one that touches the roads already standing, among the
    /// neighbourhoods already started and the next one in line. Near the town hall and on the
    /// bigger streets first, with some noise so the edge stays ragged.
    fn pick_street(&mut self) -> Option<usize> {
        let touches_open = |me: &Sim, s: usize| {
            me.streets[s].adj.iter().any(|&n| {
                let o = &me.streets[n as usize];
                o.state == StreetState::Open && o.class != RoadClass::Rail
            })
        };
        let plannable = |st: &Street| {
            st.state == StreetState::Planned
                && st.class != RoadClass::Alley
                && st.addition.is_some()
        };
        let mut cands: Vec<usize> = (0..self.streets.len())
            .filter(|&s| plannable(&self.streets[s]) && touches_open(self, s))
            .collect();
        if cands.is_empty() {
            // nothing touches the network (a generator gap): take the planned street nearest
            // to a road that stands
            let mut best: Option<(f64, usize)> = None;
            for s in 0..self.streets.len() {
                if !plannable(&self.streets[s]) {
                    continue;
                }
                let a = self.streets[s].addition.unwrap();
                if !self.revealed[a] && self.order_pos[a] != self.next_unrevealed_pos() {
                    continue;
                }
                let d = self
                    .streets
                    .iter()
                    .filter(|o| o.state == StreetState::Open && o.class != RoadClass::Rail)
                    .map(|o| o.mid.dist(self.streets[s].mid))
                    .fold(f64::MAX, f64::min);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, s));
                }
            }
            return best.map(|b| b.1);
        }
        let gate = cands
            .iter()
            .filter_map(|&s| {
                let a = self.streets[s].addition?;
                (!self.revealed[a]).then_some(self.order_pos[a])
            })
            .min();
        cands.retain(|&s| {
            let a = self.streets[s].addition.unwrap();
            self.revealed[a] || Some(self.order_pos[a]) == gate
        });
        let mut best: Option<(f64, usize)> = None;
        for &s in &cands {
            let st = &self.streets[s];
            let bonus = match st.class {
                RoadClass::Collector => 110.0,
                RoadClass::Arterial => 160.0,
                _ => 0.0,
            };
            let noise = roll(self.seed ^ 0x57ee7, s as u64, 9) * 130.0;
            let score = st.mid.dist(self.hall) + noise - bonus;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, s));
            }
        }
        best.map(|b| b.1)
    }

    fn next_unrevealed_pos(&self) -> usize {
        self.order
            .iter()
            .position(|&a| !self.revealed[a])
            .unwrap_or(usize::MAX)
    }
}
