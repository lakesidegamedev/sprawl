//! Urban Sprawl simulation, headless and deterministic.
//!
//! The generator plans the whole town up front. The simulation decides when each piece of it
//! appears: additions are revealed outward from the town hall, lots in revealed additions get
//! built after a short construction phase, and the tax rate steers how fast all of that happens.
//! A rate above [`ABANDON_ABOVE_X10`] empties buildings, then clears them.
//!
//! The mayor's levers are three tax rates (homes, shops, industry), six department budgets, bonds
//! and civic buildings (see [`model`]). Each lot has a land value recomputed monthly from its
//! surroundings and the town's services (see `Sim::lot_values`), a monthly [`Sample`] of the
//! whole game is kept in `Sim::history`, and notable things are queued as [`Event`]s.
//!
//! Only integer arithmetic and IEEE `+ - * / sqrt` are used, and every random roll is a hash of
//! `(seed, lot, day)`, so the same inputs give the same state on every platform.

mod civic;
mod dynamic;
mod econ;
mod growth;
mod ledger_view;
mod model;
mod pressure;
mod roadnet;
mod save;
mod streets;
mod traffic;
mod value;

pub use dynamic::Rival;
pub use ledger_view::LedgerNow;
pub use model::*;
pub use pressure::*;
pub use save::{AUTOSAVE_SLOTS, LoadError, SAVE_VERSION, autosave_path};
pub use streets::{RoadBuildState, RoadStage, StreetState};
pub use traffic::RoadTraffic;

use us_gen::{LotKind, Pt, RoadClass, Town, Zone, pt};

/// Tax rate is stored in tenths of a percent.
pub const TAX_MIN_X10: u32 = 0;
pub const TAX_MAX_X10: u32 = 200;
pub const TAX_DEFAULT_X10: u32 = 70;
/// Above this rate buildings start to empty out (14.0 percent).
pub const ABANDON_ABOVE_X10: u32 = 140;
/// Revealed, unbuilt lots the town keeps ahead of itself.
/// Empty lots kept ready ahead of the builders. Towns are about twice as dense as they were, so
/// the reserve is twice as big.
const RESERVE: usize = 60;
/// Builders start this many times as many buildings a day as before the density change, because
/// each neighbourhood now holds about twice as many lots.
const PACE: f64 = 2.0;
const EMPTY_DAYS_BEFORE_CLEARING: i32 = 150;
pub const START_TREASURY: i64 = 600_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Its addition has not been reached yet.
    Hidden,
    /// Revealed and waiting for a builder.
    Vacant,
    Building {
        start: i32,
        end: i32,
    },
    Built {
        since: i32,
        /// Tax rate (tenths) when it was started, for the "why" line.
        tax_x10: u32,
    },
    /// Abandoned: grey, no tax paid.
    Empty {
        since: i32,
    },
    /// Demolished after standing empty; the land can be built again.
    Cleared {
        since: i32,
    },
}

#[derive(Clone, Debug)]
pub struct LotState {
    pub phase: Phase,
    pub kind: LotKind,
    pub pos: Pt,
    pub addition: usize,
    /// Collision radius in metres.
    pub radius: f64,
    /// Direction the front faces, radians.
    pub facing: f64,
    /// Index into `Town::lots`, a stable seed for the look of the building.
    pub src: usize,
    /// Pre-existing and exempt: farms stand from day 0 and never empty out.
    pub exempt: bool,
    /// Set for buildings the mayor placed; `kind` is then the look it borrows.
    pub civic: Option<CivicKind>,
    /// Building level 1 to 5 (always 1 for civic buildings and farms). A level 2 house is row
    /// houses, level 3 and up apartments; shops grow into bigger shops and offices. The kind
    /// never changes, so the renderer picks the model from kind and level together.
    pub level: u8,
    /// While the lot is being rebuilt bigger (`Phase::Building` on a lot that had a level), the
    /// level it will have when done; 0 otherwise.
    pub next_level: u8,
    /// Day a fire started here. It burns for a few days, then the lot is rebuilt from level 1.
    pub fire: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    pub fn from_days(days: i32) -> Date {
        // days since 2026-01-01; Howard Hinnant's civil_from_days on the shifted epoch
        let z = days as i64 + 20454 + 719_468; // 2026-01-01 is day 20454 after 1970-01-01
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        Date {
            year: (y + (m <= 2) as i64) as i32,
            month: m,
            day: d,
        }
    }

    pub fn text(&self) -> String {
        const M: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        format!("{} {} {}", self.day, M[self.month as usize - 1], self.year)
    }
}

/// A run of road that appears with one addition.
#[derive(Clone, Debug)]
pub struct RoadRun {
    pub class: RoadClass,
    pub points: Vec<Pt>,
    /// `None`: part of the old skeleton, always visible.
    pub addition: Option<usize>,
    /// The street this run belongs to. A street is built as one piece; widening may later cut it
    /// into several runs that share the id.
    pub street: u32,
}

pub struct Why {
    pub title: String,
    pub status: String,
    pub built: Option<String>,
    pub reason: String,
}

fn mix(a: u64, b: u64, c: u64) -> u64 {
    let mut z = a ^ b.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ c.wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Uniform in [0, 1) from three integers.
fn roll(a: u64, b: u64, c: u64) -> f64 {
    (mix(a, b, c) >> 11) as f64 / (1u64 << 53) as f64
}

fn point_in_ring(p: Pt, ring: &[Pt]) -> bool {
    let mut inside = false;
    let mut j = ring.len().wrapping_sub(1);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_pt_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    p.dist(pt(a.x + abx * t, a.y + aby * t))
}

fn dist_pt_ring_edge(p: Pt, ring: &[Pt]) -> f64 {
    let mut best = f64::MAX;
    for i in 0..ring.len() {
        best = best.min(dist_pt_seg(p, ring[i], ring[(i + 1) % ring.len()]));
    }
    best
}

/// Growth multiplier for a tax rate in percent: about 1.0 at 7 percent, faster below, slow
/// above. Never zero, so a heavily taxed town still creeps along.
pub fn growth_mult(tax_pct: f64) -> f64 {
    (1.45 - 0.065 * tax_pct).clamp(0.05, 1.6)
}

/// Upkeep per metre per month (before the budget) by road class.
fn road_cost_per_m(c: RoadClass) -> f64 {
    match c {
        RoadClass::Highway | RoadClass::Rail => 0.0,
        RoadClass::Arterial => 1.2,
        RoadClass::Collector => 0.9,
        RoadClass::Local => 0.6,
        RoadClass::Alley => 0.25,
    }
}

fn kind_value(k: LotKind) -> i64 {
    match k {
        LotKind::House => 90_000,
        LotKind::Row => 140_000,
        LotKind::Shop => 120_000,
        LotKind::Apartments => 450_000,
        LotKind::Office => 900_000,
        LotKind::Gas => 150_000,
        LotKind::Workshop => 300_000,
        LotKind::Strip => 400_000,
        LotKind::BigBox => 800_000,
        LotKind::Warehouse => 350_000,
        LotKind::FastFood => 130_000,
        _ => 0,
    }
}

fn kind_pop(k: LotKind) -> i64 {
    match k {
        LotKind::House => 3,
        LotKind::Row => 8,
        LotKind::Apartments => 36,
        _ => 0,
    }
}

fn kind_upkeep(k: LotKind) -> i64 {
    match k {
        LotKind::School => 3000,
        LotKind::Fire => 2000,
        LotKind::Water => 800,
        LotKind::Park | LotKind::Sports => 400,
        _ => 0,
    }
}

fn kind_build_days(k: LotKind) -> i32 {
    match k {
        LotKind::House | LotKind::FastFood | LotKind::Gas => 30,
        LotKind::Row | LotKind::Shop | LotKind::Parking | LotKind::Park => 40,
        LotKind::Church | LotKind::Fire | LotKind::Water | LotKind::Sports => 55,
        LotKind::School | LotKind::Workshop | LotKind::Strip | LotKind::Warehouse => 65,
        LotKind::Apartments => 75,
        LotKind::Office | LotKind::BigBox => 90,
        LotKind::Farm => 30,
    }
}

fn kind_label(k: LotKind) -> &'static str {
    match k {
        LotKind::House => "House",
        LotKind::Row => "Row houses",
        LotKind::Shop => "Shop",
        LotKind::Apartments => "Apartment block",
        LotKind::Office => "Office tower",
        LotKind::Gas => "Gas station",
        LotKind::Workshop => "Workshop",
        LotKind::Church => "Church",
        LotKind::School => "School",
        LotKind::Fire => "Fire station",
        LotKind::Water => "Water tower",
        LotKind::Parking => "Parking lot",
        LotKind::Park => "Park",
        LotKind::Sports => "Sports field",
        LotKind::Strip => "Strip mall",
        LotKind::BigBox => "Big box store",
        LotKind::Warehouse => "Warehouse",
        LotKind::FastFood => "Fast food",
        LotKind::Farm => "Farm",
    }
}

fn abandonable(k: LotKind) -> bool {
    kind_value(k) > 0
}

pub struct Sim {
    seed: u64,
    pub day: i32,
    pub(crate) tax: ByClass<u32>,
    pub hall: Pt,
    pub(crate) town: Town,
    lots: Vec<LotState>,
    /// Non-farm additions in reveal order, with whether each has been revealed.
    order: Vec<usize>,
    next_reveal: usize,
    revealed: Vec<bool>,
    addition_poly: Vec<Vec<Pt>>,
    runs: Vec<RoadRun>,
    pub(crate) streets: Vec<streets::Street>,
    /// Per lot: the street it fronts (`streets::NO_STREET` for none).
    pub(crate) lot_street: Vec<u32>,
    /// Per addition: its place in the reveal order (`usize::MAX` for farms).
    pub(crate) order_pos: Vec<usize>,
    /// Revealed road upkeep basis, metres by class weight.
    road_cost_per_month: Vec<i64>,
    vacant: Vec<usize>,
    acc: f64,
    pub treasury: i64,
    pub last_income: i64,
    pub last_expense: i64,
    changed_lots: Vec<usize>,
    newly_revealed: Vec<usize>,
    pub abandoned_total: u32,
    pub(crate) budget: [u32; 6],
    pub(crate) loans: Vec<Loan>,
    pub(crate) history: Vec<Sample>,
    pub(crate) events: Vec<Event>,
    pub(crate) roadnet: Option<roadnet::RoadNet>,
    pub(crate) net_dirty: bool,
    /// Per lot: nearest junction and straight distance to it.
    pub(crate) lot_node: Vec<Option<(u32, f64)>>,
    pub(crate) lot_value: Vec<f32>,
    pub(crate) coverage: [Vec<f32>; 6],
    pub(crate) class_demand: ByClass<f32>,
    pub(crate) last_ledger: econ::Ledger,
    pub(crate) next_milestone: usize,
    pub(crate) warn_level: u8,
    pub(crate) run_traffic: Vec<RoadTraffic>,
    pub(crate) traffic_cells: Vec<f32>,
    pub(crate) avg_cong: f32,
    pub(crate) pending: Option<traffic::Pending>,
    pub(crate) roads_version: u32,
    pub(crate) rival: Rival,
    pub departures: u32,
    pub fires_total: u32,
    pub(crate) month_index: u32,
    pub(crate) values_stale: bool,
    pub(crate) rules: Rules,
    pub(crate) game_over: Option<GameOver>,
    pub(crate) broke_months: u32,
    pub(crate) next_election: i32,
    pub(crate) elections_won: u32,
    pub(crate) last_election: Option<ElectionResult>,
    pub(crate) approval: Approval,
    pub(crate) peak_population: i64,
    /// Days fires burned buildings down for lack of fire cover, for the approval rating.
    pub(crate) fire_days: Vec<i32>,
}

impl Sim {
    /// `hall` is where the player put the town hall; the town grows outward from it.
    pub fn new(town: &Town, hall: Pt, seed: u64) -> Sim {
        Sim::new_with(town, hall, seed, 2)
    }

    /// `initial_reveals` additions nearest the hall are revealed at once (0 for a placement
    /// preview that shows only the old roads and farms).
    pub fn new_with(town: &Town, hall: Pt, seed: u64, initial_reveals: usize) -> Sim {
        let n_add = town.additions.len();
        let addition_poly: Vec<Vec<Pt>> =
            town.additions.iter().map(|a| a.polygon.clone()).collect();
        let centroid = |ring: &[Pt]| -> Pt {
            let n = ring.len().max(1) as f64;
            pt(
                ring.iter().map(|p| p.x).sum::<f64>() / n,
                ring.iter().map(|p| p.y).sum::<f64>() / n,
            )
        };
        let farm: Vec<bool> = town
            .additions
            .iter()
            .map(|a| a.zone == Zone::Farm)
            .collect();
        let mut sim = Sim {
            seed,
            day: 0,
            tax: ByClass::all(TAX_DEFAULT_X10),
            hall,
            town: town.clone(),
            lots: Vec::new(),
            order: Vec::new(),
            next_reveal: 0,
            revealed: vec![false; n_add],
            addition_poly,
            runs: Vec::new(),
            streets: Vec::new(),
            lot_street: Vec::new(),
            order_pos: vec![usize::MAX; n_add],
            road_cost_per_month: vec![0; n_add + 1],
            vacant: Vec::new(),
            acc: 0.0,
            treasury: START_TREASURY,
            last_income: 0,
            last_expense: 0,
            changed_lots: Vec::new(),
            newly_revealed: Vec::new(),
            abandoned_total: 0,
            budget: [BUDGET_DEFAULT_PCT; 6],
            loans: Vec::new(),
            history: Vec::new(),
            events: Vec::new(),
            roadnet: None,
            net_dirty: true,
            lot_node: Vec::new(),
            lot_value: Vec::new(),
            coverage: Default::default(),
            class_demand: ByClass::all(0.5),
            last_ledger: econ::Ledger::default(),
            next_milestone: 0,
            warn_level: 0,
            run_traffic: Vec::new(),
            traffic_cells: Vec::new(),
            avg_cong: 0.0,
            pending: None,
            roads_version: 0,
            rival: Rival::new(seed),
            departures: 0,
            fires_total: 0,
            month_index: 0,
            values_stale: false,
            rules: Rules::standard(),
            game_over: None,
            broke_months: 0,
            next_election: ELECTION_PERIOD_DAYS,
            elections_won: 0,
            last_election: None,
            approval: Approval {
                total: APPROVAL_BASE,
                taxes: 0,
                services: 0,
                jobs: 0,
                traffic: 0,
                debt: 0,
                fires: 0,
                worst: Issue::Taxes,
            },
            peak_population: 0,
            fire_days: Vec::new(),
        };
        // reveal order: nearest to the hall first
        let mut ord: Vec<(i64, usize)> = (0..n_add)
            .filter(|&i| !farm[i])
            .map(|i| {
                (
                    (centroid(&sim.addition_poly[i]).dist(hall) * 100.0) as i64,
                    i,
                )
            })
            .collect();
        ord.sort();
        sim.order = ord.into_iter().map(|(_, i)| i).collect();
        for (pos, &a) in sim.order.iter().enumerate() {
            sim.order_pos[a] = pos;
        }
        for (i, &is_farm) in farm.iter().enumerate() {
            if is_farm {
                sim.revealed[i] = true;
            }
        }
        // lots
        let clear_r = town.hall_radius + 14.0;
        for (src, l) in town.lots.iter().enumerate() {
            if l.centre.dist(hall) < clear_r + l.radius {
                continue;
            }
            let add = sim.addition_of(l.centre).unwrap_or(0);
            let pre = l.kind == LotKind::Farm;
            sim.lots.push(LotState {
                phase: if pre {
                    Phase::Built {
                        since: -3000,
                        tax_x10: 0,
                    }
                } else {
                    Phase::Hidden
                },
                kind: l.kind,
                pos: l.centre,
                addition: add,
                radius: l.radius,
                facing: l.facing,
                src,
                exempt: pre,
                civic: None,
                level: 1,
                next_level: 0,
                fire: None,
            });
        }
        sim.build_runs(town, &farm);
        sim.build_streets();
        // reveal the first two additions at once so the hall has a street to stand on
        for _ in 0..initial_reveals {
            sim.reveal_next();
        }
        sim.run_traffic = vec![RoadTraffic::default(); sim.runs.len()];
        sim.recompute_values();
        sim
    }

    /// The addition whose polygon contains `p`, or the one whose edge is nearest within 40 m.
    pub fn addition_of(&self, p: Pt) -> Option<usize> {
        let mut found: Option<usize> = None;
        for (i, poly) in self.addition_poly.iter().enumerate() {
            if poly.len() >= 3 && point_in_ring(p, poly) {
                // the smallest-index non-farm addition wins; farms only if nothing else
                match found {
                    Some(f) if !self.is_farm_idx(f) => {}
                    _ => found = Some(i),
                }
                if !self.is_farm_idx(i) {
                    return Some(i);
                }
            }
        }
        if found.is_some() {
            return found;
        }
        let mut best: Option<(f64, usize)> = None;
        for (i, poly) in self.addition_poly.iter().enumerate() {
            if poly.len() < 3 {
                continue;
            }
            let d = dist_pt_ring_edge(p, poly);
            if d < 40.0 && best.is_none_or(|b| d < b.0) {
                best = Some((d, i));
            }
        }
        best.map(|b| b.1)
    }

    fn is_farm_idx(&self, i: usize) -> bool {
        // farms are the additions never put in the reveal order
        !self.order.contains(&i) && !self.order.is_empty()
    }

    fn build_runs(&mut self, town: &Town, farm: &[bool]) {
        let mut runs = Vec::new();
        for r in &town.roads {
            let skeleton = matches!(
                r.class,
                RoadClass::Highway | RoadClass::Arterial | RoadClass::Rail
            );
            if skeleton {
                runs.push(RoadRun {
                    class: r.class,
                    points: r.points.clone(),
                    addition: None,
                    street: runs.len() as u32,
                });
                continue;
            }
            let mut cur: Vec<Pt> = Vec::new();
            let mut cur_add: Option<Option<usize>> = None;
            for w in r.points.windows(2) {
                let mid = pt((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0);
                let a = self
                    .addition_of(mid)
                    .filter(|&i| !farm.get(i).copied().unwrap_or(false));
                if cur_add != Some(a) {
                    if cur.len() >= 2 {
                        runs.push(RoadRun {
                            class: r.class,
                            points: std::mem::take(&mut cur),
                            addition: cur_add.unwrap(),
                            street: runs.len() as u32,
                        });
                    }
                    cur.clear();
                    cur.push(w[0]);
                    cur_add = Some(a);
                }
                cur.push(w[1]);
            }
            if cur.len() >= 2 {
                runs.push(RoadRun {
                    class: r.class,
                    points: cur,
                    addition: cur_add.unwrap_or(None),
                    street: runs.len() as u32,
                });
            }
        }
        // upkeep per addition (index n_add = skeleton): dollars per month. Only the old roads
        // count now; a street adds its upkeep when it opens.
        let n_add = self.revealed.len();
        for run in runs.iter().filter(|r| r.addition.is_none()) {
            let len: f64 = run.points.windows(2).map(|w| w[0].dist(w[1])).sum();
            let per_m = road_cost_per_m(run.class);
            self.road_cost_per_month[n_add] += (len * per_m * 6.0) as i64;
        }
        self.runs = runs;
    }

    // ----- inputs and views -----

    pub fn lots(&self) -> &[LotState] {
        &self.lots
    }

    pub fn road_runs(&self) -> &[RoadRun] {
        &self.runs
    }

    pub fn is_revealed(&self, addition: usize) -> bool {
        self.revealed.get(addition).copied().unwrap_or(true)
    }

    pub fn date(&self) -> Date {
        Date::from_days(self.day)
    }

    /// The town this game is played on (a loaded save rebuilds it from its preset and seed).
    pub fn town(&self) -> &Town {
        &self.town
    }

    /// Neighbourhoods the town has not started on yet.
    pub fn additions_left(&self) -> usize {
        self.order.iter().filter(|&&a| !self.revealed[a]).count()
    }

    /// Lots and additions that changed since the last call.
    pub fn take_changes(&mut self) -> (Vec<usize>, Vec<usize>) {
        (
            std::mem::take(&mut self.changed_lots),
            std::mem::take(&mut self.newly_revealed),
        )
    }

    pub fn population(&self) -> i64 {
        self.lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Built { .. }) && !l.exempt)
            .map(|l| kind_pop(l.kind) * econ::level_value_pct(l.level) / 100)
            .sum()
    }

    pub fn built_count(&self) -> usize {
        self.lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Built { .. }) && !l.exempt)
            .count()
    }

    pub fn empty_count(&self) -> usize {
        self.lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Empty { .. }))
            .count()
    }

    pub fn building_count(&self) -> usize {
        self.lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Building { .. }))
            .count()
    }

    /// Hash of the whole state, for determinism checks.
    pub fn fingerprint(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut f = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        f(self.day as u64);
        f(self.treasury as u64);
        f(self.debt() as u64);
        for c in Class::ALL {
            f(self.tax.get(c) as u64);
        }
        f(self.runs.len() as u64);
        for r in &self.runs {
            f(r.class as u64);
            f(r.points.len() as u64);
            f(r.street as u64);
        }
        for st in &self.streets {
            match st.state {
                StreetState::Planned => f(1),
                StreetState::Building { start, end } => {
                    f(2);
                    f(start as u64);
                    f(end as u64)
                }
                StreetState::Open => f(3),
            }
        }
        for l in &self.lots {
            f(l.level as u64 | (l.fire.is_some() as u64) << 8);
        }
        for l in &self.lots {
            match l.phase {
                Phase::Hidden => f(1),
                Phase::Vacant => f(2),
                Phase::Building { start, end } => {
                    f(3);
                    f(start as u64);
                    f(end as u64)
                }
                Phase::Built { since, tax_x10 } => {
                    f(4);
                    f(since as u64);
                    f(tax_x10 as u64)
                }
                Phase::Empty { since } => {
                    f(5);
                    f(since as u64)
                }
                Phase::Cleared { since } => {
                    f(6);
                    f(since as u64)
                }
            }
        }
        h
    }

    // ----- the clock -----

    /// Open the nearest neighbourhood in the reveal order with all its streets at once. Only
    /// the starting town is made this way; after that streets are built one by one.
    fn reveal_next(&mut self) -> bool {
        while self
            .order
            .get(self.next_reveal)
            .is_some_and(|&a| self.revealed[a])
        {
            self.next_reveal += 1;
        }
        let Some(&a) = self.order.get(self.next_reveal) else {
            return false;
        };
        self.open_addition(a);
        for s in 0..self.streets.len() {
            if self.streets[s].addition == Some(a) && self.streets[s].state == StreetState::Planned
            {
                self.open_street(s);
            }
        }
        true
    }

    fn start_one(&mut self) -> bool {
        if self.vacant.is_empty() {
            return false;
        }
        let day = self.day as u64;
        let mut best: Option<(f64, usize)> = None;
        for (vi, &li) in self.vacant.iter().enumerate() {
            let l = &self.lots[li];
            let noise = roll(self.seed, li as u64, 77) * 140.0;
            // builders prefer valuable land near the hall, in the class the town wants most
            let lv = self.lot_value.get(li).copied().unwrap_or(0.5) as f64;
            let want = class_of(l.kind).map_or(0.4, |c| self.class_demand.get(c) as f64);
            let score =
                l.pos.dist(self.hall) + noise - 260.0 * lv - 180.0 * want - self.frontage_bonus(li);
            let _ = day;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, vi));
            }
        }
        let (_, vi) = best.unwrap();
        let li = self.vacant.swap_remove(vi);
        let days =
            kind_build_days(self.lots[li].kind) + (roll(self.seed, li as u64, 5) * 10.0) as i32;
        self.lots[li].phase = Phase::Building {
            start: self.day,
            end: self.day + days,
        };
        self.changed_lots.push(li);
        true
    }

    /// Advance one game day.
    pub fn step_day(&mut self) {
        if self.game_over.is_some() {
            return;
        }
        self.day += 1;
        let day = self.day;
        let tax = self.tax_pct();

        // finish construction
        for i in 0..self.lots.len() {
            if let Phase::Building { end, .. } = self.lots[i].phase
                && day >= end
            {
                if self.lots[i].next_level > self.lots[i].level {
                    self.lots[i].level = self.lots[i].next_level;
                }
                self.lots[i].next_level = 0;
                let rate = self.lot_rate_x10(i);
                self.lots[i].phase = Phase::Built {
                    since: day,
                    tax_x10: rate,
                };
                self.changed_lots.push(i);
                let l = &self.lots[i];
                if l.civic.is_some() || service_of_kind(l.kind).is_some() {
                    self.values_stale = true;
                    if let Some(c) = l.civic {
                        let at = l.pos;
                        self.log(
                            EventKind::Civic,
                            format!("The {} opened.", c.label().to_lowercase()),
                            Some(at),
                        );
                    }
                }
            }
        }

        self.step_fires();
        self.step_traffic();
        if self.values_stale {
            self.recompute_values();
        }

        // abandonment, recovery and clearing: each lot answers to its own class's rate; the
        // poorest land goes first
        for i in 0..self.lots.len() {
            let l = &self.lots[i];
            let rate = self.lot_rate_x10(i);
            let over = rate > ABANDON_ABOVE_X10;
            match l.phase {
                Phase::Built { .. } if over && abandonable(l.kind) && !l.exempt => {
                    let lv = self.lot_value.get(i).copied().unwrap_or(0.5) as f64;
                    let p = (rate as f64 / 10.0 - 14.0) / 6.0 * 0.004 * (1.5 - lv);
                    if roll(self.seed ^ 0xabad, i as u64, day as u64) < p {
                        self.lots[i].phase = Phase::Empty { since: day };
                        self.abandoned_total += 1;
                        self.changed_lots.push(i);
                    }
                }
                Phase::Empty { since } => {
                    if !over {
                        if roll(self.seed ^ 0xbeef, i as u64, day as u64) < 0.01 {
                            self.lots[i].phase = Phase::Built {
                                since: day,
                                tax_x10: rate,
                            };
                            self.changed_lots.push(i);
                        }
                    } else if day - since >= EMPTY_DAYS_BEFORE_CLEARING {
                        self.lots[i].phase = Phase::Cleared { since: day };
                        self.changed_lots.push(i);
                    }
                }
                Phase::Cleared { .. } => {
                    self.lots[i].phase = Phase::Vacant;
                    self.vacant.push(i);
                    self.changed_lots.push(i);
                }
                _ => {}
            }
        }

        // growth
        let built = self.built_count() as f64;
        let rate = 0.8 * PACE * growth_mult(tax) * (1.0 + (built / (700.0 * PACE)).min(1.5));
        // the mood of the market: all three classes together
        let rate = rate * (0.6 + 0.8 * self.mean_demand());
        self.acc += rate;
        self.step_streets();
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            if !self.start_one() {
                self.acc = self.acc.min(2.0);
                break;
            }
        }

        // monthly ledger on the first of the month
        if self.date().day == 1 {
            self.run_month();
        }
    }

    // ----- the why panel -----

    pub fn why(&self, lot: usize) -> Why {
        let l = &self.lots[lot];
        let title = l
            .civic
            .map_or_else(|| kind_label(l.kind), CivicKind::label)
            .to_string();
        let tax = self.lot_rate_x10(lot) as f64 / 10.0;
        let date = |d: i32| Date::from_days(d).text();
        match l.phase {
            Phase::Hidden => Why {
                title: "Open land".into(),
                status: "not reached yet".into(),
                built: None,
                reason: match self.lot_street.get(lot).copied().filter(|&s| s != streets::NO_STREET)
                {
                    Some(s) if self.streets[s as usize].state != StreetState::Open => {
                        "The street to it has not been built yet. The town extends a street when it needs more room."
                            .into()
                    }
                    _ => "The town has not grown out this far yet.".into(),
                },
            },
            Phase::Vacant => Why {
                title: "Empty lot".into(),
                status: "waiting for a builder".into(),
                built: None,
                reason: if tax > 14.0 {
                    format!("Builders are scarce at {tax:.1}% tax; growth is slow.")
                } else if tax <= 5.0 {
                    "Low taxes: builders will get to it soon.".into()
                } else {
                    "Waiting its turn behind lots closer to the town hall.".into()
                },
            },
            Phase::Building { start, end } => Why {
                title,
                status: format!("under construction, done {}", date(end)),
                built: Some(format!("started {}", date(start))),
                reason: "Builders chose this lot: it is close to the hall and the street is ready."
                    .into(),
            },
            Phase::Built { since, tax_x10 } => {
                let t = tax_x10 as f64 / 10.0;
                let reason = if l.exempt {
                    "It was here before the town.".to_string()
                } else if !abandonable(l.kind) {
                    "Built as a town service; it pays no tax and costs upkeep.".to_string()
                } else if t <= 5.0 {
                    format!("Built: low taxes ({t:.1}%) made this lot cheap to develop.")
                } else if t <= 10.0 {
                    format!("Built: land near the hall and steady demand at {t:.1}% tax.")
                } else {
                    format!("Built: demand held up despite {t:.1}% tax.")
                };
                let warn = if abandonable(l.kind) && self.lot_rate_x10(lot) > ABANDON_ABOVE_X10 {
                    format!(" At {tax:.1}% tax it may empty out soon.")
                } else {
                    String::new()
                };
                Why {
                    title,
                    status: "in use".into(),
                    built: (since > -1000).then(|| format!("built {}", date(since))),
                    reason: format!("{reason}{warn}"),
                }
            }
            Phase::Empty { since } => Why {
                title,
                status: "empty".into(),
                built: Some(format!("empty since {}", date(since))),
                reason: if self.lot_rate_x10(lot) > ABANDON_ABOVE_X10 {
                    format!(
                        "Empty: taxes too high ({tax:.1}%, limit {:.1}%). It will be cleared if they stay up.",
                        ABANDON_ABOVE_X10 as f64 / 10.0
                    )
                } else {
                    "Empty: taxes were too high. Now they are fair, someone may move back in."
                        .into()
                },
            },
            Phase::Cleared { since } => Why {
                title: "Cleared lot".into(),
                status: "demolished".into(),
                built: Some(format!("cleared {}", date(since))),
                reason: "Torn down after standing empty under high taxes.".into(),
            },
        }
    }
}

/// Old crossroads where a town hall can go: the generated hall site first, then every place two
/// arterials meet.
pub fn hall_sites(town: &Town) -> Vec<Pt> {
    let mut out = vec![town.hall];
    let arts: Vec<&[Pt]> = town
        .roads
        .iter()
        .filter(|r| r.class == RoadClass::Arterial)
        .map(|r| r.points.as_slice())
        .collect();
    let inside =
        |p: Pt| p.x > 250.0 && p.x < town.size - 250.0 && p.y > 400.0 && p.y < town.size - 250.0;
    let push = |p: Pt, out: &mut Vec<Pt>| {
        if inside(p) && out.iter().all(|q| q.dist(p) > 120.0) {
            out.push(p);
        }
    };
    for i in 0..arts.len() {
        for j in 0..arts.len() {
            if i == j {
                continue;
            }
            // endpoints of i that land on j
            for &e in [arts[i][0], *arts[i].last().unwrap()].iter() {
                for w in arts[j].windows(2) {
                    if dist_pt_seg(e, w[0], w[1]) < 6.0 {
                        push(e, &mut out);
                    }
                }
            }
            if j > i {
                for a in arts[i].windows(2) {
                    for b in arts[j].windows(2) {
                        if let Some(p) = seg_cross(a[0], a[1], b[0], b[1]) {
                            push(p, &mut out);
                        }
                    }
                }
            }
        }
    }
    out.truncate(24);
    // the hall must stand beside the crossroads, not on the pavement
    let first = out[0];
    let mut placed = vec![first];
    for &p in &out[1..] {
        let q = clear_spot_near(town, p);
        if placed.iter().all(|r| r.dist(q) > 100.0) {
            placed.push(q);
        }
    }
    placed
}

/// Nearest point within 90 m of `p` that is clear of every road and the river.
fn clear_spot_near(town: &Town, p: Pt) -> Pt {
    let clearance = |q: Pt| -> f64 {
        let mut best = f64::MAX;
        for r in &town.roads {
            let hw = r.class.half_width();
            for w in r.points.windows(2) {
                if (w[0].x.min(w[1].x) - q.x).max(q.x - w[0].x.max(w[1].x)) > best + hw + 60.0 {
                    continue;
                }
                best = best.min(dist_pt_seg(q, w[0], w[1]) - hw);
            }
        }
        for ring in &town.water {
            if point_in_ring(q, ring) {
                return -1.0;
            }
            best = best.min(dist_pt_ring_edge(q, ring) - 20.0);
        }
        best
    };
    let need = town.hall_radius + 14.0;
    let mut best: Option<(f64, Pt)> = None;
    for k in 0..6 {
        let r = 28.0 + 14.0 * k as f64;
        for a in 0..24 {
            let ang = a as f64 * std::f64::consts::TAU / 24.0;
            let q = pt(p.x + r * libm::cos(ang), p.y + r * libm::sin(ang));
            if clearance(q) >= need && best.is_none_or(|b| r < b.0) {
                best = Some((r, q));
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map_or(p, |b| b.1)
}

fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<Pt> {
    let r = pt(b.x - a.x, b.y - a.y);
    let s = pt(d.x - c.x, d.y - c.y);
    let den = r.cross(s);
    if den.abs() < 1e-9 {
        return None;
    }
    let q = pt(c.x - a.x, c.y - a.y);
    let t = q.cross(s) / den;
    let u = q.cross(r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| pt(a.x + r.x * t, a.y + r.y * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    fn run(town: &Town, tax: u32, days: i32) -> Sim {
        let mut s = Sim::new_with_rules(town, hall_sites(town)[0], 7, 2, Rules::sandbox());
        s.set_tax_x10(tax);
        for _ in 0..days {
            s.step_day();
        }
        s
    }

    #[test]
    fn low_tax_grows_faster_than_high_tax() {
        let town = generate(Preset::Grid, 3);
        let low = run(&town, 30, 500);
        let high = run(&town, 120, 500);
        assert!(
            low.built_count() > high.built_count() * 3 / 2,
            "low {} high {}",
            low.built_count(),
            high.built_count()
        );
    }

    #[test]
    fn same_inputs_same_state() {
        let town = generate(Preset::Winding, 5);
        let a = run(&town, 70, 400);
        let b = run(&town, 70, 400);
        assert_eq!(a.fingerprint(), b.fingerprint());
        let c = run(&town, 80, 400);
        assert_ne!(a.fingerprint(), c.fingerprint());
    }

    #[test]
    fn abandonment_only_above_threshold() {
        let town = generate(Preset::Grid, 3);
        let at = run(&town, ABANDON_ABOVE_X10, 900);
        assert_eq!(at.abandoned_total, 0);
        let above = run(&town, 190, 900);
        // growth is slow up there, so build up first at a fair rate and then raise taxes
        let mut s = run(&town, 60, 700);
        s.set_tax_x10(190);
        for _ in 0..300 {
            s.step_day();
        }
        assert!(s.abandoned_total > 0, "no building emptied at 19 percent");
        let _ = above;
    }

    #[test]
    fn many_seeds_and_every_site_run_clean() {
        for seed in 1..=8u64 {
            for preset in [Preset::Grid, Preset::Winding] {
                let town = generate(preset, seed * 37);
                let sites = hall_sites(&town);
                assert!(!sites.is_empty());
                for &site in sites.iter().take(3) {
                    let mut s = Sim::new(&town, site, seed);
                    for _ in 0..120 {
                        s.step_day();
                    }
                    assert!(s.building_count() + s.built_count() > 0);
                }
            }
        }
    }

    #[test]
    fn date_starts_on_new_year() {
        let d = Date::from_days(0);
        assert_eq!((d.year, d.month, d.day), (2026, 1, 1));
        let d = Date::from_days(365);
        assert_eq!((d.year, d.month, d.day), (2027, 1, 1));
    }

    #[test]
    fn town_keeps_growing_and_pays_something() {
        let town = generate(Preset::Grid, 3);
        let s = run(&town, 70, 1500);
        assert!(s.built_count() > 400, "built {}", s.built_count());
        assert!(s.population() > 1000);
        eprintln!(
            "day {} built {} pop {} treasury {} flow {:?} left {}",
            s.day,
            s.built_count(),
            s.population(),
            s.treasury,
            s.monthly_flow(),
            s.additions_left()
        );
    }
}

#[cfg(test)]
mod tests_levers;

#[cfg(test)]
mod tests_world;

#[cfg(test)]
mod tests_density;

#[cfg(test)]
mod tests_balance;

#[cfg(test)]
mod tests_pressure;

#[cfg(test)]
mod tests_save;

#[cfg(test)]
mod tests_streets;
