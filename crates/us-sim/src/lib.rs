//! Urban Sprawl simulation, headless and deterministic.
//!
//! The generator plans the whole town up front. The simulation decides when each piece of it
//! appears: additions are revealed outward from the town hall, lots in revealed additions get
//! built after a short construction phase, and the tax rate steers how fast all of that happens.
//! A rate above [`ABANDON_ABOVE_X10`] empties buildings, then clears them.
//!
//! Only integer arithmetic and IEEE `+ - * / sqrt` are used, and every random roll is a hash of
//! `(seed, lot, day)`, so the same inputs give the same state on every platform.

use us_gen::{LotKind, Pt, RoadClass, Town, Zone, pt};

/// Tax rate is stored in tenths of a percent.
pub const TAX_MIN_X10: u32 = 0;
pub const TAX_MAX_X10: u32 = 200;
pub const TAX_DEFAULT_X10: u32 = 70;
/// Above this rate buildings start to empty out (14.0 percent).
pub const ABANDON_ABOVE_X10: u32 = 140;
/// Revealed, unbuilt lots the town keeps ahead of itself.
const RESERVE: usize = 30;
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
    /// Pre-existing and exempt: farms stand from day 0 and never empty out.
    pub exempt: bool,
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
    tax_x10: u32,
    pub hall: Pt,
    lots: Vec<LotState>,
    /// Non-farm additions in reveal order, with whether each has been revealed.
    order: Vec<usize>,
    next_reveal: usize,
    revealed: Vec<bool>,
    addition_poly: Vec<Vec<Pt>>,
    runs: Vec<RoadRun>,
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
}

impl Sim {
    /// `hall` is where the player put the town hall; the town grows outward from it.
    pub fn new(town: &Town, hall: Pt, seed: u64) -> Sim {
        let n_add = town.additions.len();
        let addition_poly: Vec<Vec<Pt>> = town.additions.iter().map(|a| a.polygon.clone()).collect();
        let centroid = |ring: &[Pt]| -> Pt {
            let n = ring.len().max(1) as f64;
            pt(
                ring.iter().map(|p| p.x).sum::<f64>() / n,
                ring.iter().map(|p| p.y).sum::<f64>() / n,
            )
        };
        let farm: Vec<bool> = town.additions.iter().map(|a| a.zone == Zone::Farm).collect();
        let mut sim = Sim {
            seed,
            day: 0,
            tax_x10: TAX_DEFAULT_X10,
            hall,
            lots: Vec::new(),
            order: Vec::new(),
            next_reveal: 0,
            revealed: vec![false; n_add],
            addition_poly,
            runs: Vec::new(),
            road_cost_per_month: vec![0; n_add + 1],
            vacant: Vec::new(),
            acc: 0.0,
            treasury: START_TREASURY,
            last_income: 0,
            last_expense: 0,
            changed_lots: Vec::new(),
            newly_revealed: Vec::new(),
            abandoned_total: 0,
        };
        // reveal order: nearest to the hall first
        let mut ord: Vec<(i64, usize)> = (0..n_add)
            .filter(|&i| !farm[i])
            .map(|i| ((centroid(&sim.addition_poly[i]).dist(hall) * 100.0) as i64, i))
            .collect();
        ord.sort();
        sim.order = ord.into_iter().map(|(_, i)| i).collect();
        for i in 0..n_add {
            if farm[i] {
                sim.revealed[i] = true;
            }
        }
        // lots
        let clear_r = town.hall_radius + 14.0;
        for l in &town.lots {
            if l.centre.dist(hall) < clear_r + l.radius {
                continue;
            }
            let add = sim.addition_of(l.centre).unwrap_or(0);
            let pre = l.kind == LotKind::Farm;
            sim.lots.push(LotState {
                phase: if pre {
                    Phase::Built { since: -3000, tax_x10: 0 }
                } else {
                    Phase::Hidden
                },
                kind: l.kind,
                pos: l.centre,
                addition: add,
                exempt: pre,
            });
        }
        sim.build_runs(town, &farm);
        // reveal the first two additions at once so the hall has a street to stand on
        for _ in 0..2 {
            sim.reveal_next();
        }
        sim.newly_revealed.clear();
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
                });
            }
        }
        // upkeep per addition (index n_add = skeleton): dollars per month
        let n_add = self.revealed.len();
        for run in &runs {
            let len: f64 = run.points.windows(2).map(|w| w[0].dist(w[1])).sum();
            let per_m = match run.class {
                RoadClass::Highway | RoadClass::Rail => 0.0,
                RoadClass::Arterial => 1.2,
                RoadClass::Collector => 0.9,
                RoadClass::Local => 0.6,
                RoadClass::Alley => 0.25,
            };
            let slot = run.addition.unwrap_or(n_add);
            self.road_cost_per_month[slot] += (len * per_m * 3.0) as i64;
        }
        // cul-de-sacs add a little
        for c in &town.cul_de_sacs {
            if c.addition < n_add {
                self.road_cost_per_month[c.addition] += 60;
            }
        }
        self.runs = runs;
    }

    // ----- inputs and views -----

    pub fn tax_x10(&self) -> u32 {
        self.tax_x10
    }

    pub fn tax_pct(&self) -> f64 {
        self.tax_x10 as f64 / 10.0
    }

    pub fn set_tax_x10(&mut self, v: u32) {
        self.tax_x10 = v.clamp(TAX_MIN_X10, TAX_MAX_X10);
    }

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

    pub fn additions_left(&self) -> usize {
        self.order.len() - self.next_reveal
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
            .map(|l| kind_pop(l.kind))
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

    /// Dollars per month at today's state: (tax income, expenses).
    pub fn monthly_flow(&self) -> (i64, i64) {
        let mut income = 0i64;
        let mut upkeep = 0i64;
        for l in &self.lots {
            if let Phase::Built { .. } = l.phase {
                income += kind_value(l.kind) * self.tax_x10 as i64 / 1000 / 12;
                upkeep += kind_upkeep(l.kind);
                if abandonable(l.kind) {
                    upkeep += 330;
                }
            }
        }
        let mut roads = self.road_cost_per_month[self.revealed.len()];
        for (i, &r) in self.revealed.iter().enumerate() {
            if r {
                roads += self.road_cost_per_month[i];
            }
        }
        (income, upkeep + roads)
    }

    pub fn monthly_net(&self) -> i64 {
        let (i, e) = self.monthly_flow();
        i - e
    }

    /// 0 to 1: how strongly the town wants to grow at today's tax rate and vacancy.
    pub fn demand(&self) -> f64 {
        let built = self.built_count().max(1) as f64;
        let empty_share = self.empty_count() as f64 / built;
        let d = growth_mult(self.tax_pct()) / 1.6 * (1.0 - 0.7 * empty_share.min(1.0));
        let land_left = !self.vacant.is_empty() || self.additions_left() > 0;
        if land_left { d.clamp(0.0, 1.0) } else { 0.0 }
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

    fn reveal_next(&mut self) -> bool {
        let Some(&a) = self.order.get(self.next_reveal) else {
            return false;
        };
        self.next_reveal += 1;
        self.revealed[a] = true;
        self.newly_revealed.push(a);
        for i in 0..self.lots.len() {
            if self.lots[i].addition == a && self.lots[i].phase == Phase::Hidden {
                self.lots[i].phase = Phase::Vacant;
                self.vacant.push(i);
                self.changed_lots.push(i);
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
            // civic lots and parks build a little later than homes and shops
            let score = l.pos.dist(self.hall) + noise;
            let _ = day;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, vi));
            }
        }
        let (_, vi) = best.unwrap();
        let li = self.vacant.swap_remove(vi);
        let days = kind_build_days(self.lots[li].kind)
            + (roll(self.seed, li as u64, 5) * 10.0) as i32;
        self.lots[li].phase = Phase::Building {
            start: self.day,
            end: self.day + days,
        };
        self.changed_lots.push(li);
        true
    }

    /// Advance one game day.
    pub fn step_day(&mut self) {
        self.day += 1;
        let day = self.day;
        let tax = self.tax_pct();

        // finish construction
        for i in 0..self.lots.len() {
            if let Phase::Building { end, .. } = self.lots[i].phase
                && day >= end
            {
                self.lots[i].phase = Phase::Built {
                    since: day,
                    tax_x10: self.tax_x10,
                };
                self.changed_lots.push(i);
            }
        }

        // abandonment, recovery and clearing
        let over = self.tax_x10 > ABANDON_ABOVE_X10;
        for i in 0..self.lots.len() {
            let l = &self.lots[i];
            match l.phase {
                Phase::Built { .. } if over && abandonable(l.kind) && !l.exempt => {
                    let p = (tax - 14.0) / 6.0 * 0.004;
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
                                tax_x10: self.tax_x10,
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
        let rate = 0.8 * growth_mult(tax) * (1.0 + (built / 700.0).min(1.5));
        self.acc += rate;
        let mut reveals = 0;
        while self.vacant.len() < RESERVE && reveals < 2 && self.reveal_next() {
            reveals += 1;
        }
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            if !self.start_one() {
                self.acc = self.acc.min(2.0);
                break;
            }
        }
        if self.vacant.len() < RESERVE && reveals == 0 {
            self.reveal_next();
        }

        // monthly ledger on the first of the month
        if self.date().day == 1 {
            let (income, expense) = self.monthly_flow();
            self.treasury += income - expense;
            self.last_income = income;
            self.last_expense = expense;
        }
    }

    // ----- the why panel -----

    pub fn why(&self, lot: usize) -> Why {
        let l = &self.lots[lot];
        let title = kind_label(l.kind).to_string();
        let tax = self.tax_pct();
        let date = |d: i32| Date::from_days(d).text();
        match l.phase {
            Phase::Hidden => Why {
                title: "Open land".into(),
                status: "not reached yet".into(),
                built: None,
                reason: "The town has not grown out this far yet.".into(),
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
                reason: "Builders chose this lot: it is close to the hall and the street is ready.".into(),
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
                let warn = if abandonable(l.kind) && self.tax_x10 > ABANDON_ABOVE_X10 {
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
                reason: if self.tax_x10 > ABANDON_ABOVE_X10 {
                    format!(
                        "Empty: taxes too high ({tax:.1}%, limit {:.1}%). It will be cleared if they stay up.",
                        ABANDON_ABOVE_X10 as f64 / 10.0
                    )
                } else {
                    "Empty: taxes were too high. Now they are fair, someone may move back in.".into()
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
    let inside = |p: Pt| p.x > 250.0 && p.x < town.size - 250.0 && p.y > 400.0 && p.y < town.size - 250.0;
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
    out
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
        let mut s = Sim::new(town, hall_sites(town)[0], 7);
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
