//! Fires that grow and spread, and the engines, police cars and ambulances that answer them.
//!
//! A fire starts at a moment of the day and grows through three stages (smoke, flames, fully
//! involved). The nearest fire station sends an engine along the real road network; how long it
//! takes (turnout, distance, traffic, the fire budget, other fires the station is busy with)
//! decides the stage the fire has reached when it arrives, and so whether the building is saved,
//! how long the fire burns and how many neighbours catch. A fire nobody answers burns out.
//!
//! The town is stepped a day at a time, so a day's incidents are worked out when the day starts,
//! each with its own second of the day. The vehicles are a pure function of the clock: the
//! viewer asks where every vehicle is at a moment and draws it.
//!
//! Saved in the `FIRE` section. A save without it has no fire in progress.

use crate::dynamic::FIRE_DAYS;
use crate::roadnet::RoadNet;
use crate::save::{R, Res, W};
use crate::*;

pub const DAY_S: i64 = 86_400;
/// A fire reached before this many minutes is still smoke.
const SMOKE_MIN: f64 = 8.0;
/// Before this many minutes it is flames; after, the building is fully involved.
const FLAMES_MIN: f64 = 18.0;
/// Fires spread to buildings this close, metres.
const SPREAD_M: f64 = 34.0;
/// Most neighbours one fire sets alight. One fire never lights more than one other, and the
/// more fires burn at once the less each one spreads (neighbours turn out with buckets and
/// hoses, fire breaks hold), so a single storm cannot raze a town.
const MAX_SPREAD: u8 = 1;
/// How far an engine is willing to drive, seconds.
const DRIVE_LIMIT_S: f64 = 420.0;
/// Minutes the crew works at the scene after arriving, by stage reached.
const ON_SCENE_MIN: [f64; 3] = [25.0, 90.0, 240.0];
/// Days a fire nobody answers burns.
const UNANSWERED_DAYS: i64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VehicleKind {
    FireEngine,
    PoliceCar,
    Ambulance,
}

impl VehicleKind {
    fn code(self) -> u8 {
        self as u8
    }
    fn from_code(c: u8) -> VehicleKind {
        match c {
            1 => VehicleKind::PoliceCar,
            2 => VehicleKind::Ambulance,
            _ => VehicleKind::FireEngine,
        }
    }
}

/// A vehicle on the road at one moment, for drawing.
#[derive(Clone, Copy, Debug)]
pub struct Vehicle {
    pub kind: VehicleKind,
    pub pos: Pt,
    /// Direction of travel, radians.
    pub heading: f64,
    /// Driving with lights on (false while parked at the scene).
    pub moving: bool,
}

/// A fire burning now, for drawing.
#[derive(Clone, Copy, Debug)]
pub struct BlazeView {
    pub lot: usize,
    pub pos: Pt,
    /// 0 smoke, 1 flames, 2 fully involved.
    pub stage: u8,
}

/// One trip of an emergency vehicle: out along `route`, a stay at the end, and back.
#[derive(Clone, Debug)]
pub(crate) struct Rig {
    pub(crate) kind: VehicleKind,
    pub(crate) depart_s: i64,
    pub(crate) travel_s: f32,
    stay_until_s: i64,
    route: Vec<Pt>,
}

impl Rig {
    fn end_s(&self) -> i64 {
        self.stay_until_s + self.travel_s as i64 + 1
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Blaze {
    pub(crate) lot: u32,
    start_s: i64,
    /// When the first engine got there; -1 when none came.
    pub(crate) arrive_s: i64,
    pub(crate) out_s: i64,
    /// The worst stage it reaches, 0 to 2.
    peak: u8,
    pub(crate) lost: bool,
    spread: u8,
    /// Day the spread rolls were last made.
    rolled: i32,
    /// The station that answered, a lot index, or `u32::MAX`.
    station: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FireState {
    pub(crate) blazes: Vec<Blaze>,
    pub(crate) rigs: Vec<Rig>,
    /// Dollars the town has lost to fire and crime, and relief it was paid, over the game.
    pub(crate) lost_total: i64,
    pub(crate) relief_total: i64,
    pub(crate) saved: u32,
    pub(crate) hurt: u32,
    pub(crate) calls: u32,
}

const NAMES: [&str; 16] = [
    "Oak", "Maple", "Cedar", "Elm", "Pine", "Birch", "Willow", "Mill", "Church", "Park", "Lake",
    "Hill", "River", "Station", "Market", "Spring",
];

fn stage_of_age(min: f64) -> u8 {
    if min < SMOKE_MIN {
        0
    } else if min < FLAMES_MIN {
        1
    } else {
        2
    }
}

impl Sim {
    /// A street name for news lines. Streets have no names of their own, so the name is made
    /// from the street's number and the town's seed.
    pub fn street_name_of_lot(&self, lot: usize) -> String {
        let s = self.lot_street.get(lot).copied().unwrap_or(u32::MAX);
        if s == crate::streets::NO_STREET || s as usize >= self.streets.len() {
            return "the edge of town".into();
        }
        let n = NAMES[(mix(self.seed, 0x57ee, s as u64) % NAMES.len() as u64) as usize];
        let suffix = match self.streets[s as usize].class {
            RoadClass::Arterial | RoadClass::Highway => "Avenue",
            RoadClass::Collector => "Road",
            _ => "Street",
        };
        format!("{n} {suffix}")
    }

    /// Lots that give `svc` and are open now.
    fn stations_of(&self, svc: Service) -> Vec<usize> {
        (0..self.lots.len())
            .filter(|&i| {
                let l = &self.lots[i];
                if !matches!(l.phase, Phase::Built { .. }) {
                    return false;
                }
                let s = match l.civic {
                    Some(c) => c.covers(),
                    None => service_of_kind(l.kind),
                };
                s == Some(svc)
            })
            .collect()
    }

    /// Plan the trip of the vehicle that gets to `lot` first from a station of `svc`, leaving
    /// `delay_s` after `from_s`. `None` when no station can be reached.
    fn plan_rig(
        &self,
        kind: VehicleKind,
        svc: Service,
        lot: usize,
        from_s: i64,
        delay_s: f64,
        busy: &dyn Fn(usize) -> f64,
    ) -> Option<(Rig, usize, f64)> {
        let rn = self.roadnet.as_ref()?;
        let (src, off_fire) = self.lot_node.get(lot).copied().flatten()?;
        let tree = rn.tree(src, DRIVE_LIMIT_S);
        let cong = 1.0 + 0.9 * self.avg_cong as f64;
        let mut best: Option<(f64, usize, u32, f64)> = None;
        for st in self.stations_of(svc) {
            if st == lot {
                continue;
            }
            let Some((nd, off)) = self.lot_node.get(st).copied().flatten() else {
                continue;
            };
            let secs = tree.secs[nd as usize];
            if secs == f32::MAX {
                continue;
            }
            let drive = secs as f64 * cong + (off + off_fire) / 9.0;
            let total = drive + busy(st);
            if best.is_none_or(|b| total < b.0) {
                best = Some((total, st, nd, drive));
            }
        }
        let (_, st, nd, drive) = best?;
        let mut route = vec![self.lots[st].pos];
        route.extend(rn.route_to_source(&tree, nd));
        route.push(self.lots[lot].pos);
        let depart_s = from_s + delay_s as i64;
        Some((
            Rig {
                kind,
                depart_s,
                travel_s: drive as f32,
                stay_until_s: depart_s + drive as i64,
                route,
            },
            st,
            drive,
        ))
    }

    /// Turnout delay of the crews, seconds: faster with a better funded department.
    fn turnout_s(&self, dept: Dept) -> f64 {
        let f = (self.budget[dept.index()] as f64 / 100.0).clamp(0.0, 1.5);
        (3.0 + 7.0 * (1.0 - f).max(-0.3)) * 60.0
    }

    /// A fire starts on lot `i` at `start_s` seconds into the game. The old call, kept for the
    /// tests and tools: `_cov` is no longer needed because the response is worked out.
    #[cfg(test)]
    pub(crate) fn ignite(&mut self, i: usize, _cov: f64) {
        let s = self.day as i64 * DAY_S
            + (roll(self.seed ^ 0xf1de, i as u64, self.day as u64) * 80_000.0) as i64;
        self.start_blaze(i, s);
    }

    /// A fire the weather started: `lightning` adds a news line saying a bolt did it.
    pub(crate) fn ignite_by(&mut self, i: usize, _cov: f64, lightning: bool) {
        let s = self.day as i64 * DAY_S
            + (roll(self.seed ^ 0x1b07, i as u64, self.day as u64) * 80_000.0) as i64;
        if lightning {
            let name = kind_label(self.lots[i].kind).to_lowercase();
            let at = self.lots[i].pos;
            self.log(
                EventKind::Fire,
                format!("Lightning hit a {name}."),
                Some(at),
            );
        }
        self.start_blaze(i, s);
    }

    pub(crate) fn start_blaze(&mut self, i: usize, start_s: i64) {
        let day = self.day;
        let at = self.lots[i].pos;
        self.lots[i].fire = Some(day);
        self.fires_total += 1;
        self.changed_lots.push(i);
        let level = self.lots[i].level.max(1) as i64;
        let class = class_of(self.lots[i].kind);

        // the engine
        let turnout = self.turnout_s(Dept::Fire);
        let active: Vec<(u32, i64)> = self
            .fx
            .blazes
            .iter()
            .filter(|b| b.station != u32::MAX && b.out_s > start_s)
            .map(|b| (b.station, b.out_s))
            .collect();
        // a station already out on another fire sends a second crew later
        let busy =
            |st: usize| 8.0 * 60.0 * active.iter().filter(|a| a.0 as usize == st).count() as f64;
        let engine = self.plan_rig(
            VehicleKind::FireEngine,
            Service::Fire,
            i,
            start_s,
            turnout,
            &busy,
        );
        let (peak, arrive_s, out_s, station, resp_min) = match &engine {
            Some((rig, st, drive)) => {
                let wait = busy(*st);
                let r = (turnout + wait + *drive) / 60.0;
                let peak = stage_of_age(r);
                let arrive = start_s + (r * 60.0) as i64;
                let out = arrive + (ON_SCENE_MIN[peak as usize] * 60.0) as i64;
                let _ = rig;
                (peak, arrive, out, *st as u32, r)
            }
            None => (2, -1, start_s + UNANSWERED_DAYS * DAY_S, u32::MAX, f64::MAX),
        };
        let mut rigs: Vec<Rig> = Vec::new();
        if let Some((mut rig, _, _)) = engine {
            rig.depart_s += (busy(station as usize)) as i64;
            rig.stay_until_s = out_s;
            rigs.push(rig);
        }
        // police to the fire
        let pol = self.plan_rig(
            VehicleKind::PoliceCar,
            Service::Police,
            i,
            start_s,
            self.turnout_s(Dept::Police) * 0.6,
            &|_| 0.0,
        );
        if let Some((mut rig, _, _)) = pol {
            rig.stay_until_s = out_s.min(rig.stay_until_s + 3600);
            rigs.push(rig);
        }
        // people inside
        let hour = ((start_s - day as i64 * DAY_S) / 3600) as i32;
        let night = !(7..21).contains(&hour);
        let p_inside = match class {
            Some(Class::Homes) => {
                if night {
                    0.7
                } else {
                    0.3
                }
            }
            Some(_) => {
                if night {
                    0.0
                } else {
                    0.4
                }
            }
            None => 0.0,
        };
        let inside = roll(self.seed ^ 0x1451de, i as u64, start_s as u64) < p_inside;
        let mut ambulance_min = f64::MAX;
        if inside {
            let amb = self.plan_rig(
                VehicleKind::Ambulance,
                Service::Health,
                i,
                start_s,
                self.turnout_s(Dept::Health) * 0.8,
                &|_| 0.0,
            );
            if let Some((mut rig, _, drive)) = amb {
                ambulance_min = (self.turnout_s(Dept::Health) * 0.8 + drive) / 60.0;
                rig.stay_until_s = rig.depart_s + drive as i64 + 1800;
                rigs.push(rig);
            }
        }
        self.fx.rigs.extend(rigs);

        // what it costs
        let lost = peak >= 1;
        let attended = arrive_s >= 0;
        let cost = if lost { 2500 * level } else { 500 * level };
        let relief = if lost && attended { cost / 2 } else { 0 };
        self.treasury -= cost - relief;
        self.fx.lost_total += cost;
        self.fx.relief_total += relief;
        if !lost {
            self.fx.saved += 1;
        } else {
            self.fire_days.push(day);
        }
        let mut hurt_line = String::new();
        if inside {
            if ambulance_min > 15.0 {
                self.fx.hurt += 1;
                self.fire_days.push(day);
                hurt_line = if ambulance_min == f64::MAX {
                    " People were hurt and no ambulance could reach them.".into()
                } else {
                    format!(
                        " People were hurt; the ambulance took {} minutes.",
                        ambulance_min.round() as i64
                    )
                };
            } else {
                hurt_line = format!(
                    " People were inside; the ambulance took {} minutes.",
                    ambulance_min.round().max(1.0) as i64
                );
            }
        }
        self.fx.blazes.push(Blaze {
            lot: i as u32,
            start_s,
            arrive_s,
            out_s,
            peak,
            lost,
            spread: 0,
            rolled: day - 1,
            station,
        });

        let name = kind_label(self.lots[i].kind).to_lowercase();
        let street = self.street_name_of_lot(i);
        if self.fires_total == 1 && self.stations_of(Service::Fire).is_empty() {
            self.log(
                EventKind::Fire,
                "No fire station: fires will spread. Build a fire station.".into(),
                Some(at),
            );
        }
        let text = if !attended {
            format!(
                "A {name} on {street} is burning. No fire station can reach it, so it will burn out.{hurt_line}"
            )
        } else if !lost {
            format!(
                "A {name} on {street} caught fire. The engine took {} minutes and put it out in the smoke stage.{hurt_line}",
                resp_min.round().max(1.0) as i64
            )
        } else {
            format!(
                "A {name} on {street} caught fire. The engine took {} minutes and the building was lost. Relief paid ${relief}.{hurt_line}",
                resp_min.round() as i64
            )
        };
        self.log(EventKind::Fire, text, Some(at));
    }

    /// Daily: fires end, start and spread; vehicles finish; small incidents happen.
    pub(crate) fn step_fires(&mut self) {
        let day = self.day;
        let day_start = day as i64 * DAY_S;
        // fires that ended before today
        let mut k = 0;
        while k < self.fx.blazes.len() {
            if self.fx.blazes[k].out_s < day_start {
                let b = self.fx.blazes.remove(k);
                let l = b.lot as usize;
                if !b.lost {
                    // saved, but smoke and water damage keep it empty while it is repaired
                    let days = 30 + (roll(self.seed ^ 0x4e9a, l as u64, day as u64) * 30.0) as i32;
                    let lot = &mut self.lots[l];
                    lot.fire = None;
                    if matches!(lot.phase, Phase::Built { .. }) {
                        lot.next_level = 0;
                        lot.phase = Phase::Building {
                            start: day,
                            end: day + days,
                        };
                    }
                }
                self.changed_lots.push(l);
            } else {
                k += 1;
            }
        }
        self.fx.rigs.retain(|r| r.end_s() >= day_start);

        // Lots burned out have their replacement capacity reserved.
        for i in 0..self.lots.len() {
            if let Some(start) = self.lots[i].fire
                && day - start >= FIRE_DAYS
                && !self.fx.blazes.iter().any(|b| b.lot as usize == i)
            {
                let days = 60 + (roll(self.seed ^ 0xf1e, i as u64, day as u64) * 40.0) as i32;
                let l = &mut self.lots[i];
                l.fire = None;
                l.level = 1;
                l.next_level = 0;
                l.phase = Phase::Building {
                    start: day,
                    end: day + days,
                };
                self.changed_lots.push(i);
            }
        }

        // new fires
        for i in 0..self.lots.len() {
            let l = &self.lots[i];
            if l.fire.is_some()
                || !matches!(l.phase, Phase::Built { .. })
                || l.exempt
                || l.civic.is_some()
                || class_of(l.kind).is_none()
            {
                continue;
            }
            let cov = self.coverage[Service::Fire.index()]
                .get(i)
                .copied()
                .unwrap_or(0.0) as f64;
            // dry, windy days are worse; rain and lying snow damp fires down
            let wx = self.fire_weather_factor();
            let p = (0.000_01 + 0.000_4 * (1.0 - cov) * (1.0 - cov) / 30.0)
                * self.ord_fire_factor(i)
                * wx;
            if roll(self.seed ^ 0xf17e, i as u64, day as u64) < p {
                let s =
                    day_start + (roll(self.seed ^ 0xf1de, i as u64, day as u64) * 80_000.0) as i64;
                self.start_blaze(i, s);
            }
        }

        self.spread_fires();
        self.small_incidents();
    }

    /// Stage of a fire at a moment, `None` when it is not burning then.
    fn stage_at(b: &Blaze, t: i64) -> Option<u8> {
        if t < b.start_s || t >= b.out_s {
            return None;
        }
        let age = (t - b.start_s) as f64 / 60.0;
        Some(stage_of_age(age).min(b.peak))
    }

    /// Hours of flames and of full fire inside `[from, to)`.
    fn exposure(b: &Blaze, from: i64, to: i64) -> f64 {
        let fl = b.start_s + (SMOKE_MIN * 60.0) as i64;
        let full = b.start_s + (FLAMES_MIN * 60.0) as i64;
        let span = |a: i64, z: i64| -> f64 {
            let (a, z) = (a.max(from).max(b.start_s), z.min(to).min(b.out_s));
            ((z - a).max(0)) as f64 / 3600.0
        };
        match b.peak {
            0 => 0.0,
            1 => span(fl, i64::MAX),
            _ => span(fl, full) + 3.0 * span(full, i64::MAX),
        }
    }

    fn spread_fires(&mut self) {
        let day = self.day;
        let from = day as i64 * DAY_S;
        let to = from + DAY_S;
        let n = self.fx.blazes.len();
        let wind = self.wind();
        let wind_speed = (wind.x * wind.x + wind.y * wind.y).sqrt();
        let dry = 0.5 + self.dryness();
        let burning = self
            .fx
            .blazes
            .iter()
            .filter(|b| b.out_s > from && b.start_s < to)
            .count() as f64;
        let crowd = 1.0 / (1.0 + burning);
        for k in 0..n {
            let b = self.fx.blazes[k].clone();
            if b.rolled >= day || b.spread >= MAX_SPREAD {
                continue;
            }
            self.fx.blazes[k].rolled = day;
            let e = Self::exposure(&b, from, to);
            if e <= 0.0 {
                continue;
            }
            let at = self.lots[b.lot as usize].pos;
            let mut caught = 0u8;
            for j in 0..self.lots.len() {
                if b.spread + caught >= MAX_SPREAD {
                    break;
                }
                let m = &self.lots[j];
                if j == b.lot as usize
                    || m.fire.is_some()
                    || !matches!(m.phase, Phase::Built { .. })
                    || m.exempt
                    || m.civic.is_some()
                {
                    continue;
                }
                let Some(class) = class_of(m.kind) else {
                    continue;
                };
                let d = m.pos.dist(at);
                if d >= SPREAD_M {
                    continue;
                }
                let flam = match class {
                    Class::Homes if m.level >= 3 => 0.8,
                    Class::Homes => 1.0,
                    Class::Shops => 0.9,
                    Class::Industry => 1.2,
                };
                let align = if wind_speed > 0.01 && d > 0.01 {
                    ((m.pos.x - at.x) * wind.x + (m.pos.y - at.y) * wind.y) / (d * wind_speed)
                } else {
                    0.0
                };
                let windf = (1.0 + 0.12 * wind_speed * align).clamp(0.3, 2.5);
                let prox = 1.0 - d / SPREAD_M;
                let lam = 0.02 * e * prox * flam * windf * dry * crowd * self.ord_fire_factor(j);
                let p = 1.0 - libm::exp(-lam);
                if roll(
                    self.seed ^ 0x5b7ead,
                    j as u64,
                    day as u64 ^ (b.start_s as u64) << 8,
                ) < p
                {
                    caught += 1;
                    let lo = from.max(b.start_s + (SMOKE_MIN * 60.0) as i64);
                    let t = lo
                        + (roll(self.seed ^ 0xc4a7, j as u64, day as u64)
                            * (to - lo - 1).max(1) as f64) as i64;
                    self.start_blaze(j, t.min(to - 1));
                }
            }
            if let Some(b) = self.fx.blazes.get_mut(k) {
                b.spread += caught;
            }
        }
    }

    /// A day's small incidents: weak police cover means more of them and slower answers.
    fn small_incidents(&mut self) {
        let day = self.day;
        if self.built_count() < 25 {
            return;
        }
        let cover = {
            let c = &self.coverage[Service::Police.index()];
            let (mut sum, mut n) = (0.0f64, 0usize);
            for (i, l) in self.lots.iter().enumerate() {
                if matches!(l.phase, Phase::Built { .. }) && class_of(l.kind).is_some() {
                    sum += c.get(i).copied().unwrap_or(0.0) as f64;
                    n += 1;
                }
            }
            sum / n.max(1) as f64
        };
        let p = 0.04 + 0.22 * (1.0 - cover);
        if roll(self.seed ^ 0x7011ce, day as u64, 1) >= p {
            return;
        }
        let homes: Vec<usize> = (0..self.lots.len())
            .filter(|&i| {
                let l = &self.lots[i];
                matches!(l.phase, Phase::Built { .. })
                    && l.civic.is_none()
                    && class_of(l.kind).is_some()
            })
            .collect();
        if homes.is_empty() {
            return;
        }
        let i = homes[(mix(self.seed, 0x7011, day as u64) % homes.len() as u64) as usize];
        let start_s =
            day as i64 * DAY_S + (roll(self.seed ^ 0x7012, day as u64, 2) * 80_000.0) as i64;
        self.fx.calls += 1;
        let rig = self.plan_rig(
            VehicleKind::PoliceCar,
            Service::Police,
            i,
            start_s,
            self.turnout_s(Dept::Police),
            &|_| 0.0,
        );
        let street = self.street_name_of_lot(i);
        let level = self.lots[i].level.max(1) as i64;
        let at = self.lots[i].pos;
        match rig {
            Some((mut r, _, drive)) => {
                let mins = (self.turnout_s(Dept::Police) + drive) / 60.0;
                r.stay_until_s = r.depart_s + drive as i64 + 1200;
                self.fx.rigs.push(r);
                if mins > 12.0 {
                    let cost = 200 * level;
                    self.treasury -= cost;
                    self.fx.lost_total += cost;
                    self.log(
                        EventKind::Fire,
                        format!(
                            "A break-in on {street}: the police took {} minutes and the thieves were gone. Loss ${cost}.",
                            mins.round() as i64
                        ),
                        Some(at),
                    );
                }
            }
            None => {
                let cost = 300 * level;
                self.treasury -= cost;
                self.fx.lost_total += cost;
                self.log(
                    EventKind::Fire,
                    format!(
                        "A break-in on {street}. No police station can reach it. Loss ${cost}."
                    ),
                    Some(at),
                );
            }
        }
    }

    // ----- what the viewer reads -----

    /// Fires on today's map. The viewer's clock runs on its own, so a fire that has not started
    /// yet at `now_s` shows at its first stage rather than not at all; one that has burned out
    /// for the day shows at its last.
    pub fn blazes_at(&self, now_s: f64) -> Vec<BlazeView> {
        let day_start = self.day as i64 * DAY_S;
        self.fx
            .blazes
            .iter()
            .filter(|b| b.out_s >= day_start)
            .filter_map(|b| {
                let t = (now_s as i64).clamp(b.start_s, (b.out_s - 1).max(b.start_s));
                Self::stage_at(b, t).map(|stage| BlazeView {
                    lot: b.lot as usize,
                    pos: self.lots[b.lot as usize].pos,
                    stage,
                })
            })
            .collect()
    }

    /// Whether lot `i` has a fire that is still being fought or burning today.
    pub fn is_burning(&self, i: usize) -> bool {
        let day_start = self.day as i64 * DAY_S;
        self.fx
            .blazes
            .iter()
            .any(|b| b.lot as usize == i && b.out_s >= day_start)
    }

    /// Where every emergency vehicle is at `now_s` seconds into the game.
    pub fn vehicles_at(&self, now_s: f64) -> Vec<Vehicle> {
        let mut out = Vec::new();
        for r in &self.fx.rigs {
            let t = now_s - r.depart_s as f64;
            if t < 0.0 || r.route.len() < 2 {
                continue;
            }
            let travel = (r.travel_s as f64).max(1.0);
            let stay = (r.stay_until_s - r.depart_s) as f64;
            let (frac, rev, moving) = if t < travel {
                (t / travel, false, true)
            } else if t < stay.max(travel) {
                (1.0, false, false)
            } else if t < stay.max(travel) + travel {
                (1.0 - (t - stay.max(travel)) / travel, true, true)
            } else {
                continue;
            };
            let (pos, heading) = along(&r.route, frac, rev);
            out.push(Vehicle {
                kind: r.kind,
                pos,
                heading,
                moving,
            });
        }
        out
    }

    /// A line for the why panel when lot `i` is burning or was lately.
    pub(crate) fn fire_why(&self, i: usize) -> Option<String> {
        let b = self.fx.blazes.iter().find(|b| b.lot as usize == i)?;
        let street = self.street_name_of_lot(i);
        Some(if b.arrive_s < 0 {
            format!("Burning with no fire station in reach. It will burn out on {street}.")
        } else {
            let mins = ((b.arrive_s - b.start_s) as f64 / 60.0).round() as i64;
            if b.lost {
                format!(
                    "A fire here was reached after {mins} minutes, too late to save the building."
                )
            } else {
                format!("A fire here was reached after {mins} minutes and put out early.")
            }
        })
    }

    pub fn fire_losses(&self) -> (i64, i64) {
        (self.fx.lost_total, self.fx.relief_total)
    }

    // ----- save -----

    pub(crate) fn save_fires(&self, w: &mut W) {
        w.usize(self.fx.blazes.len());
        for b in &self.fx.blazes {
            w.u32(b.lot);
            w.i64(b.start_s);
            w.i64(b.arrive_s);
            w.i64(b.out_s);
            w.u8(b.peak);
            w.bool(b.lost);
            w.u8(b.spread);
            w.i32(b.rolled);
            w.u32(b.station);
        }
        w.usize(self.fx.rigs.len());
        for r in &self.fx.rigs {
            w.u8(r.kind.code());
            w.i64(r.depart_s);
            w.f32(r.travel_s);
            w.i64(r.stay_until_s);
            w.pts(&r.route);
        }
        w.i64(self.fx.lost_total);
        w.i64(self.fx.relief_total);
        w.u32(self.fx.saved);
        w.u32(self.fx.hurt);
        w.u32(self.fx.calls);
    }

    pub(crate) fn load_fires(&mut self, r: &mut R) -> Res<()> {
        let n = r.len(34)?;
        let mut blazes = Vec::with_capacity(n);
        for _ in 0..n {
            let lot = r.u32()?;
            if lot as usize >= self.lots.len() {
                return Err(crate::save::LoadError::Invalid("fire lot"));
            }
            blazes.push(Blaze {
                lot,
                start_s: r.i64()?,
                arrive_s: r.i64()?,
                out_s: r.i64()?,
                peak: r.u8()?.min(2),
                lost: r.bool()?,
                spread: r.u8()?,
                rolled: r.i32()?,
                station: r.u32()?,
            });
        }
        let n = r.len(30)?;
        let mut rigs = Vec::with_capacity(n);
        for _ in 0..n {
            rigs.push(Rig {
                kind: VehicleKind::from_code(r.u8()?),
                depart_s: r.i64()?,
                travel_s: r.f32()?,
                stay_until_s: r.i64()?,
                route: r.pts()?,
            });
        }
        self.fx = FireState {
            blazes,
            rigs,
            lost_total: r.i64()?,
            relief_total: r.i64()?,
            saved: r.u32()?,
            hurt: r.u32()?,
            calls: r.u32()?,
        };
        Ok(())
    }

    pub(crate) fn fire_hash(&self, f: &mut impl FnMut(u64)) {
        for b in &self.fx.blazes {
            f(b.lot as u64);
            f(b.start_s as u64);
            f(b.out_s as u64);
            f(b.peak as u64 | (b.lost as u64) << 8 | (b.spread as u64) << 16);
        }
        for r in &self.fx.rigs {
            f(r.kind.code() as u64);
            f(r.depart_s as u64);
            f(r.stay_until_s as u64);
            f(r.travel_s.to_bits() as u64);
            f(r.route.len() as u64);
        }
        f(self.fx.lost_total as u64);
        f(self.fx.relief_total as u64);
        f(self.fx.saved as u64 | (self.fx.hurt as u64) << 20 | (self.fx.calls as u64) << 40);
    }

    pub(crate) fn fire_is_default(&self) -> bool {
        self.fx.blazes.is_empty()
            && self.fx.rigs.is_empty()
            && self.fx.lost_total == 0
            && self.fx.calls == 0
            && self.fx.saved == 0
    }
}

/// A point `frac` of the way along a polyline, and the heading there (reversed if `rev`).
fn along(route: &[Pt], frac: f64, rev: bool) -> (Pt, f64) {
    let total: f64 = route.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut left = frac.clamp(0.0, 1.0) * total;
    let mut last = (route[0], 0.0);
    for w in route.windows(2) {
        let d = w[0].dist(w[1]);
        let h = libm::atan2(w[1].y - w[0].y, w[1].x - w[0].x);
        last = (w[1], h);
        if d < 1e-6 {
            continue;
        }
        if left <= d {
            let p = w[0].lerp(w[1], left / d);
            return (p, if rev { h + std::f64::consts::PI } else { h });
        }
        left -= d;
    }
    (
        last.0,
        if rev {
            last.1 + std::f64::consts::PI
        } else {
            last.1
        },
    )
}

#[allow(dead_code)]
fn _uses(_: &RoadNet) {}
