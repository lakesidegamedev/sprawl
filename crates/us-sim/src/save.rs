//! Saving and loading a whole game.
//!
//! A save is a small header (magic, format version, length, checksum) and a compact binary body.
//! The town itself is not stored: it is generated again from its preset and seed, and a hash of
//! the result is checked so a changed generator is refused rather than loaded wrongly. Everything
//! else (every lot and road, levers, bonds, history, events, rival, derived caches and the clock)
//! is stored exactly, so loading and stepping gives the same state as never having saved.

use crate::traffic::{Done, Op, Pending, Work};
use crate::*;
use std::fmt;
use std::path::{Path, PathBuf};
use us_gen::{Preset, generate};

const MAGIC: &[u8; 4] = b"USSV";
/// The save format this build writes and reads.
pub const SAVE_VERSION: u16 = 3;
const HEADER_LEN: usize = 4 + 2 + 8 + 8;
/// How many autosave files are kept.
pub const AUTOSAVE_SLOTS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Not a save file at all.
    NotASave,
    /// Made by an older format that this build can no longer read.
    TooOld { found: u16, current: u16 },
    /// Made by a newer build of the game.
    TooNew { found: u16, current: u16 },
    /// The file ends early.
    Truncated,
    /// The contents do not match their checksum.
    Corrupt,
    /// The town generator no longer produces the town this save was made on.
    TownChanged,
    /// A value that cannot be right.
    Invalid(&'static str),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotASave => write!(f, "This file is not an Urban Sprawl save."),
            LoadError::TooOld { found, current } => write!(
                f,
                "This save was made by an older version of the game (save format {found}, this version reads format {current}) and can no longer be opened."
            ),
            LoadError::TooNew { found, current } => write!(
                f,
                "This save was made by a newer version of the game (save format {found}, this version reads format {current}). Update the game to open it."
            ),
            LoadError::Truncated => write!(f, "This save is cut short and cannot be opened."),
            LoadError::Corrupt => write!(f, "This save is damaged (its checksum does not match)."),
            LoadError::TownChanged => write!(
                f,
                "The town in this save can no longer be rebuilt by this version of the game."
            ),
            LoadError::Invalid(what) => {
                write!(f, "This save holds a value that cannot be right ({what}).")
            }
        }
    }
}

impl std::error::Error for LoadError {}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ----- byte writer and reader -----

pub(crate) struct W(pub Vec<u8>);

impl W {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.0.push(v as u8);
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    pub fn usize(&mut self, v: usize) {
        self.u64(v as u64);
    }
    pub fn str(&mut self, v: &str) {
        self.usize(v.len());
        self.0.extend_from_slice(v.as_bytes());
    }
    pub fn pt(&mut self, p: Pt) {
        self.f64(p.x);
        self.f64(p.y);
    }
    pub fn pts(&mut self, ps: &[Pt]) {
        self.usize(ps.len());
        for &p in ps {
            self.pt(p);
        }
    }
    pub fn opt_pt(&mut self, p: Option<Pt>) {
        self.bool(p.is_some());
        if let Some(p) = p {
            self.pt(p);
        }
    }
    pub fn by_class_i64(&mut self, c: ByClass<i64>) {
        self.i64(c.homes);
        self.i64(c.shops);
        self.i64(c.industry);
    }
    pub fn by_class_u32(&mut self, c: ByClass<u32>) {
        self.u32(c.homes);
        self.u32(c.shops);
        self.u32(c.industry);
    }
    pub fn by_class_f32(&mut self, c: ByClass<f32>) {
        self.f32(c.homes);
        self.f32(c.shops);
        self.f32(c.industry);
    }
}

pub(crate) struct R<'a> {
    b: &'a [u8],
    at: usize,
}

type Res<T> = Result<T, LoadError>;

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or(LoadError::Truncated)?;
        let s = self.b.get(self.at..end).ok_or(LoadError::Truncated)?;
        self.at = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Res<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(LoadError::Invalid("flag")),
        }
    }
    pub fn u32(&mut self) -> Res<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Res<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Res<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Res<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Res<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn f64(&mut self) -> Res<f64> {
        Ok(f64::from_bits(self.u64()?))
    }
    pub fn usize(&mut self) -> Res<usize> {
        usize::try_from(self.u64()?).map_err(|_| LoadError::Invalid("size"))
    }
    /// A length that the rest of the file could plausibly hold at `each` bytes per item.
    pub fn len(&mut self, each: usize) -> Res<usize> {
        let n = self.usize()?;
        if n.saturating_mul(each.max(1)) > self.b.len() - self.at {
            return Err(LoadError::Truncated);
        }
        Ok(n)
    }
    pub fn str(&mut self) -> Res<String> {
        let n = self.len(1)?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| LoadError::Invalid("text"))
    }
    pub fn pt(&mut self) -> Res<Pt> {
        Ok(pt(self.f64()?, self.f64()?))
    }
    pub fn pts(&mut self) -> Res<Vec<Pt>> {
        let n = self.len(16)?;
        (0..n).map(|_| self.pt()).collect()
    }
    pub fn opt_pt(&mut self) -> Res<Option<Pt>> {
        Ok(if self.bool()? { Some(self.pt()?) } else { None })
    }
    pub fn by_class_i64(&mut self) -> Res<ByClass<i64>> {
        Ok(ByClass {
            homes: self.i64()?,
            shops: self.i64()?,
            industry: self.i64()?,
        })
    }
    pub fn by_class_u32(&mut self) -> Res<ByClass<u32>> {
        Ok(ByClass {
            homes: self.u32()?,
            shops: self.u32()?,
            industry: self.u32()?,
        })
    }
    pub fn by_class_f32(&mut self) -> Res<ByClass<f32>> {
        Ok(ByClass {
            homes: self.f32()?,
            shops: self.f32()?,
            industry: self.f32()?,
        })
    }
}

// ----- enum codes -----

const LOT_KINDS: [LotKind; 19] = [
    LotKind::House,
    LotKind::Row,
    LotKind::Shop,
    LotKind::Apartments,
    LotKind::Office,
    LotKind::Gas,
    LotKind::Workshop,
    LotKind::Church,
    LotKind::School,
    LotKind::Fire,
    LotKind::Water,
    LotKind::Parking,
    LotKind::Park,
    LotKind::Sports,
    LotKind::Strip,
    LotKind::BigBox,
    LotKind::Warehouse,
    LotKind::FastFood,
    LotKind::Farm,
];
const ROAD_CLASSES: [RoadClass; 6] = [
    RoadClass::Highway,
    RoadClass::Arterial,
    RoadClass::Collector,
    RoadClass::Local,
    RoadClass::Alley,
    RoadClass::Rail,
];
const EVENT_KINDS: [EventKind; 13] = [
    EventKind::Neighborhood,
    EventKind::Milestone,
    EventKind::Budget,
    EventKind::Civic,
    EventKind::Loan,
    EventKind::Upgrade,
    EventKind::Road,
    EventKind::Fire,
    EventKind::Rival,
    EventKind::Decline,
    EventKind::Election,
    EventKind::GameOver,
    EventKind::Limit,
];

fn code<T: PartialEq + Copy>(all: &[T], v: T) -> u8 {
    all.iter().position(|&x| x == v).expect("known variant") as u8
}

fn decode<T: Copy>(all: &[T], c: u8, what: &'static str) -> Res<T> {
    all.get(c as usize).copied().ok_or(LoadError::Invalid(what))
}

fn put_issue(w: &mut W, i: Issue) {
    match i {
        Issue::Taxes => w.u8(0),
        Issue::Services(s) => {
            w.u8(1);
            w.u8(s.index() as u8);
        }
        Issue::Jobs => w.u8(2),
        Issue::Traffic => w.u8(3),
        Issue::Debt => w.u8(4),
        Issue::Fires => w.u8(5),
    }
}

fn get_issue(r: &mut R) -> Res<Issue> {
    Ok(match r.u8()? {
        0 => Issue::Taxes,
        1 => Issue::Services(decode(&Service::ALL, r.u8()?, "service")?),
        2 => Issue::Jobs,
        3 => Issue::Traffic,
        4 => Issue::Debt,
        5 => Issue::Fires,
        _ => return Err(LoadError::Invalid("issue")),
    })
}

fn put_phase(w: &mut W, p: Phase) {
    match p {
        Phase::Hidden => w.u8(0),
        Phase::Vacant => w.u8(1),
        Phase::Building { start, end } => {
            w.u8(2);
            w.i32(start);
            w.i32(end);
        }
        Phase::Built { since, tax_x10 } => {
            w.u8(3);
            w.i32(since);
            w.u32(tax_x10);
        }
        Phase::Empty { since } => {
            w.u8(4);
            w.i32(since);
        }
        Phase::Cleared { since } => {
            w.u8(5);
            w.i32(since);
        }
    }
}

fn get_phase(r: &mut R) -> Res<Phase> {
    Ok(match r.u8()? {
        0 => Phase::Hidden,
        1 => Phase::Vacant,
        2 => Phase::Building {
            start: r.i32()?,
            end: r.i32()?,
        },
        3 => Phase::Built {
            since: r.i32()?,
            tax_x10: r.u32()?,
        },
        4 => Phase::Empty { since: r.i32()? },
        5 => Phase::Cleared { since: r.i32()? },
        _ => return Err(LoadError::Invalid("phase")),
    })
}

fn put_summary(w: &mut W, s: &CareerSummary) {
    w.i32(s.days);
    w.i32(s.years);
    w.i64(s.population);
    w.i64(s.peak_population);
    w.u32(s.buildings);
    w.i64(s.jobs);
    w.i64(s.treasury);
    w.i64(s.debt);
    w.u32(s.elections_won);
    w.u32(s.fires);
    w.u32(s.departures);
    w.i32(s.approval);
}

fn get_summary(r: &mut R) -> Res<CareerSummary> {
    Ok(CareerSummary {
        days: r.i32()?,
        years: r.i32()?,
        population: r.i64()?,
        peak_population: r.i64()?,
        buildings: r.u32()?,
        jobs: r.i64()?,
        treasury: r.i64()?,
        debt: r.i64()?,
        elections_won: r.u32()?,
        fires: r.u32()?,
        departures: r.u32()?,
        approval: r.i32()?,
    })
}

fn put_traffic(w: &mut W, t: &RoadTraffic) {
    w.f32(t.volume);
    w.f32(t.vc);
}

fn get_traffic(r: &mut R) -> Res<RoadTraffic> {
    Ok(RoadTraffic {
        volume: r.f32()?,
        vc: r.f32()?,
    })
}

fn put_pending(w: &mut W, p: &Pending) {
    let Work::Ready(done) = &p.work else {
        unreachable!("settled before saving")
    };
    w.i32(p.join_day);
    w.usize(p.lot_map.len());
    for &i in &p.lot_map {
        w.usize(i);
    }
    w.usize(done.run_traffic.len());
    for t in &done.run_traffic {
        put_traffic(w, t);
    }
    w.f32(done.avg_cong);
    w.usize(done.ops.len());
    for op in &done.ops {
        match op {
            Op::Upgrade { pts, to, lots } => {
                w.u8(0);
                w.pts(pts);
                w.u8(code(&ROAD_CLASSES, *to));
                put_u32s(w, lots);
            }
            Op::Connector {
                pa,
                pb,
                class,
                lots,
            } => {
                w.u8(1);
                w.pt(*pa);
                w.pt(*pb);
                w.u8(code(&ROAD_CLASSES, *class));
                put_u32s(w, lots);
            }
        }
    }
}

fn put_u32s(w: &mut W, v: &[u32]) {
    w.usize(v.len());
    for &x in v {
        w.u32(x);
    }
}

fn get_u32s(r: &mut R) -> Res<Vec<u32>> {
    let n = r.len(4)?;
    (0..n).map(|_| r.u32()).collect()
}

fn get_pending(r: &mut R) -> Res<Pending> {
    let join_day = r.i32()?;
    let n = r.len(8)?;
    let lot_map = (0..n).map(|_| r.usize()).collect::<Res<Vec<_>>>()?;
    let n = r.len(8)?;
    let run_traffic = (0..n).map(|_| get_traffic(r)).collect::<Res<Vec<_>>>()?;
    let avg_cong = r.f32()?;
    let n = r.len(1)?;
    let mut ops = Vec::with_capacity(n);
    for _ in 0..n {
        ops.push(match r.u8()? {
            0 => Op::Upgrade {
                pts: r.pts()?,
                to: decode(&ROAD_CLASSES, r.u8()?, "road class")?,
                lots: get_u32s(r)?,
            },
            1 => Op::Connector {
                pa: r.pt()?,
                pb: r.pt()?,
                class: decode(&ROAD_CLASSES, r.u8()?, "road class")?,
                lots: get_u32s(r)?,
            },
            _ => return Err(LoadError::Invalid("road change")),
        });
    }
    Ok(Pending {
        work: Work::Ready(Done {
            run_traffic,
            avg_cong,
            ops,
        }),
        join_day,
        lot_map,
    })
}

fn put_f32s(w: &mut W, v: &[f32]) {
    w.usize(v.len());
    for &x in v {
        w.f32(x);
    }
}

fn get_f32s(r: &mut R) -> Res<Vec<f32>> {
    let n = r.len(4)?;
    (0..n).map(|_| r.f32()).collect()
}

fn town_hash(t: &Town) -> u64 {
    fnv(&t.canonical_bytes())
}

impl Sim {
    /// The whole game as bytes. A traffic run in flight is waited for first (it is deterministic,
    /// so the game goes on exactly as it would have).
    pub fn save(&mut self) -> Vec<u8> {
        self.settle_traffic();
        let mut w = W(Vec::new());
        // the code is the place in `Preset::ALL`, so new presets must be added at the end
        w.u8(code(&Preset::ALL, self.town.preset));
        w.u64(self.town.seed);
        w.u64(town_hash(&self.town));
        w.u64(self.seed);
        w.pt(self.hall);
        w.i32(self.day);
        w.by_class_u32(self.tax);
        w.usize(self.next_reveal);
        w.usize(self.revealed.len());
        for &b in &self.revealed {
            w.bool(b);
        }
        w.usize(self.lots.len());
        for l in &self.lots {
            put_phase(&mut w, l.phase);
            w.u8(code(&LOT_KINDS, l.kind));
            w.pt(l.pos);
            w.usize(l.addition);
            w.f64(l.radius);
            w.f64(l.facing);
            w.usize(l.src);
            w.bool(l.exempt);
            w.u8(l.civic.map_or(255, |c| code(&CivicKind::ALL, c)));
            w.u8(l.level);
            w.u8(l.next_level);
            w.bool(l.fire.is_some());
            if let Some(d) = l.fire {
                w.i32(d);
            }
        }
        w.usize(self.runs.len());
        for r in &self.runs {
            w.u8(code(&ROAD_CLASSES, r.class));
            w.pts(&r.points);
            w.bool(r.addition.is_some());
            if let Some(a) = r.addition {
                w.usize(a);
            }
            w.u32(r.street);
        }
        w.usize(self.streets.len());
        for st in &self.streets {
            match st.state {
                StreetState::Planned => w.u8(0),
                StreetState::Building { start, end } => {
                    w.u8(1);
                    w.i32(start);
                    w.i32(end);
                }
                StreetState::Open => w.u8(2),
            }
        }
        w.usize(self.road_cost_per_month.len());
        for &c in &self.road_cost_per_month {
            w.i64(c);
        }
        w.usize(self.vacant.len());
        for &v in &self.vacant {
            w.usize(v);
        }
        w.f64(self.acc);
        w.i64(self.treasury);
        w.i64(self.last_income);
        w.i64(self.last_expense);
        for list in [&self.changed_lots, &self.newly_revealed] {
            w.usize(list.len());
            for &i in list {
                w.usize(i);
            }
        }
        w.u32(self.abandoned_total);
        for &b in &self.budget {
            w.u32(b);
        }
        w.usize(self.loans.len());
        for l in &self.loans {
            w.i64(l.principal_left);
            w.i64(l.monthly_principal);
        }
        w.usize(self.history.len());
        for s in &self.history {
            w.i32(s.day);
            w.i32(s.year);
            w.u32(s.month);
            w.i64(s.population);
            w.i64(s.jobs);
            w.i64(s.treasury);
            w.by_class_i64(s.income);
            for &x in &s.spending {
                w.i64(x);
            }
            w.i64(s.admin);
            w.i64(s.debt_service);
            w.i64(s.debt);
            w.by_class_u32(s.tax_x10);
            w.by_class_f32(s.demand);
            w.u32(s.built);
            w.u32(s.building);
            w.u32(s.empty);
            w.f32(s.avg_land_value);
            w.f32(s.congestion);
            w.i32(s.approval);
            w.i32(s.days_to_election);
        }
        w.usize(self.events.len());
        for e in &self.events {
            w.i32(e.day);
            w.u8(code(&EVENT_KINDS, e.kind));
            w.str(&e.text);
            w.opt_pt(e.place);
        }
        w.bool(self.net_dirty);
        w.usize(self.lot_node.len());
        for n in &self.lot_node {
            w.bool(n.is_some());
            if let Some((a, d)) = n {
                w.u32(*a);
                w.f64(*d);
            }
        }
        put_f32s(&mut w, &self.lot_value);
        for c in &self.coverage {
            put_f32s(&mut w, c);
        }
        w.by_class_f32(self.class_demand);
        let lg = &self.last_ledger;
        w.by_class_i64(lg.income);
        for &x in &lg.spending {
            w.i64(x);
        }
        w.i64(lg.admin);
        w.i64(lg.interest);
        w.i64(lg.principal);
        w.usize(self.next_milestone);
        w.u8(self.warn_level);
        w.usize(self.run_traffic.len());
        for t in &self.run_traffic {
            put_traffic(&mut w, t);
        }
        put_f32s(&mut w, &self.traffic_cells);
        w.f32(self.avg_cong);
        w.bool(self.pending.is_some());
        if let Some(p) = &self.pending {
            put_pending(&mut w, p);
        }
        w.u32(self.roads_version);
        w.u8(dynamic::rival_code(self.rival.name));
        w.by_class_u32(self.rival.tax);
        w.u32(self.departures);
        w.u32(self.fires_total);
        w.u32(self.month_index);
        w.bool(self.values_stale);
        w.bool(self.rules.bankruptcy);
        w.bool(self.rules.elections);
        w.bool(self.rules.unlimited_money);
        w.u8(code(&Difficulty::ALL, self.rules.difficulty));
        w.bool(self.game_over.is_some());
        if let Some(g) = &self.game_over {
            w.i32(g.day);
            match &g.reason {
                GameOverReason::Bankrupt => w.u8(0),
                GameOverReason::LostElection {
                    challenger,
                    issue,
                    share,
                } => {
                    w.u8(1);
                    w.str(challenger);
                    put_issue(&mut w, *issue);
                    w.i32(*share);
                }
            }
            put_summary(&mut w, &g.summary);
        }
        w.u32(self.broke_months);
        w.i32(self.next_election);
        w.u32(self.elections_won);
        w.bool(self.last_election.is_some());
        if let Some(e) = &self.last_election {
            w.i32(e.day);
            w.i32(e.approval);
            w.i32(e.share);
            w.bool(e.won);
            w.str(&e.challenger);
            put_issue(&mut w, e.issue);
        }
        let a = &self.approval;
        for v in [
            a.total, a.taxes, a.services, a.jobs, a.traffic, a.debt, a.fires,
        ] {
            w.i32(v);
        }
        put_issue(&mut w, a.worst);
        w.i64(self.peak_population);
        w.usize(self.fire_days.len());
        for &d in &self.fire_days {
            w.i32(d);
        }
        for c in Class::ALL {
            w.f64(self.queue.get(c));
        }
        for i in 0..4 {
            w.bool(self.stalled[i]);
            w.bool(self.warned[i]);
        }
        w.bool(self.jobless);
        w.i32(self.last_kind_street);
        w.i64(self.growth_spent);

        let body = w.0;
        let mut out = Vec::with_capacity(HEADER_LEN + body.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&SAVE_VERSION.to_le_bytes());
        out.extend_from_slice(&(body.len() as u64).to_le_bytes());
        out.extend_from_slice(&fnv(&body).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Read a game saved by [`Sim::save`].
    pub fn load(bytes: &[u8]) -> Result<Sim, LoadError> {
        if bytes.len() < 6 || &bytes[..4] != MAGIC {
            return Err(LoadError::NotASave);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version < SAVE_VERSION {
            return Err(LoadError::TooOld {
                found: version,
                current: SAVE_VERSION,
            });
        }
        if version > SAVE_VERSION {
            return Err(LoadError::TooNew {
                found: version,
                current: SAVE_VERSION,
            });
        }
        if bytes.len() < HEADER_LEN {
            return Err(LoadError::Truncated);
        }
        let len = u64::from_le_bytes(bytes[6..14].try_into().unwrap());
        let sum = u64::from_le_bytes(bytes[14..22].try_into().unwrap());
        let body = &bytes[HEADER_LEN..];
        if (body.len() as u64) < len {
            return Err(LoadError::Truncated);
        }
        let body = &body[..len as usize];
        if fnv(body) != sum {
            return Err(LoadError::Corrupt);
        }
        let mut r = R { b: body, at: 0 };
        let r = &mut r;
        let preset = decode(&Preset::ALL, r.u8()?, "preset")?;
        let town_seed = r.u64()?;
        let hash = r.u64()?;
        let town = generate(preset, town_seed);
        if town_hash(&town) != hash {
            return Err(LoadError::TownChanged);
        }
        let seed = r.u64()?;
        let hall = r.pt()?;
        let mut s = Sim::new_with(&town, hall, seed, 0);
        s.day = r.i32()?;
        s.tax = r.by_class_u32()?;
        s.next_reveal = r.usize()?;
        let n = r.len(1)?;
        if n != s.revealed.len() {
            return Err(LoadError::Invalid("additions"));
        }
        for i in 0..n {
            s.revealed[i] = r.bool()?;
        }
        let n = r.len(40)?;
        let mut lots = Vec::with_capacity(n);
        for _ in 0..n {
            let phase = get_phase(r)?;
            let kind = decode(&LOT_KINDS, r.u8()?, "lot kind")?;
            let pos = r.pt()?;
            let addition = r.usize()?;
            let radius = r.f64()?;
            let facing = r.f64()?;
            let src = r.usize()?;
            let exempt = r.bool()?;
            let civic = match r.u8()? {
                255 => None,
                c => Some(decode(&CivicKind::ALL, c, "civic kind")?),
            };
            let level = r.u8()?;
            let next_level = r.u8()?;
            let fire = if r.bool()? { Some(r.i32()?) } else { None };
            lots.push(LotState {
                phase,
                kind,
                pos,
                addition,
                radius,
                facing,
                src,
                exempt,
                civic,
                level,
                next_level,
                fire,
            });
        }
        s.lots = lots;
        let n = r.len(9)?;
        let mut runs = Vec::with_capacity(n);
        for _ in 0..n {
            let class = decode(&ROAD_CLASSES, r.u8()?, "road class")?;
            let points = r.pts()?;
            let addition = if r.bool()? { Some(r.usize()?) } else { None };
            if addition.is_some_and(|a| a >= s.revealed.len()) {
                return Err(LoadError::Invalid("road addition"));
            }
            let street = r.u32()?;
            runs.push(RoadRun {
                class,
                points,
                addition,
                street,
            });
        }
        s.runs = runs;
        let n = r.len(1)?;
        if n < s.streets.len() {
            return Err(LoadError::Invalid("streets"));
        }
        for i in 0..n {
            let state = match r.u8()? {
                0 => StreetState::Planned,
                1 => StreetState::Building {
                    start: r.i32()?,
                    end: r.i32()?,
                },
                2 => StreetState::Open,
                _ => return Err(LoadError::Invalid("street state")),
            };
            if i == s.streets.len() {
                // a connector the traffic model added after the town was planned
                s.streets.push(streets::Street::open_connector(
                    RoadClass::Collector,
                    s.hall,
                    s.hall,
                ));
            }
            s.streets[i].state = state;
        }
        if s.runs.iter().any(|r| r.street as usize >= s.streets.len()) {
            return Err(LoadError::Invalid("road street"));
        }
        let n = r.len(8)?;
        s.road_cost_per_month = (0..n).map(|_| r.i64()).collect::<Res<_>>()?;
        let n = r.len(8)?;
        s.vacant = (0..n).map(|_| r.usize()).collect::<Res<_>>()?;
        s.acc = r.f64()?;
        s.treasury = r.i64()?;
        s.last_income = r.i64()?;
        s.last_expense = r.i64()?;
        let n = r.len(8)?;
        s.changed_lots = (0..n).map(|_| r.usize()).collect::<Res<_>>()?;
        let n = r.len(8)?;
        s.newly_revealed = (0..n).map(|_| r.usize()).collect::<Res<_>>()?;
        s.abandoned_total = r.u32()?;
        for b in s.budget.iter_mut() {
            *b = r.u32()?;
        }
        let n = r.len(16)?;
        s.loans = (0..n)
            .map(|_| {
                Ok(Loan {
                    principal_left: r.i64()?,
                    monthly_principal: r.i64()?,
                })
            })
            .collect::<Res<_>>()?;
        let n = r.len(100)?;
        let mut history = Vec::with_capacity(n);
        for _ in 0..n {
            history.push(Sample {
                day: r.i32()?,
                year: r.i32()?,
                month: r.u32()?,
                population: r.i64()?,
                jobs: r.i64()?,
                treasury: r.i64()?,
                income: r.by_class_i64()?,
                spending: {
                    let mut a = [0i64; 6];
                    for x in a.iter_mut() {
                        *x = r.i64()?;
                    }
                    a
                },
                admin: r.i64()?,
                debt_service: r.i64()?,
                debt: r.i64()?,
                tax_x10: r.by_class_u32()?,
                demand: r.by_class_f32()?,
                built: r.u32()?,
                building: r.u32()?,
                empty: r.u32()?,
                avg_land_value: r.f32()?,
                congestion: r.f32()?,
                approval: r.i32()?,
                days_to_election: r.i32()?,
            });
        }
        s.history = history;
        let n = r.len(9)?;
        let mut events = Vec::with_capacity(n);
        for _ in 0..n {
            events.push(Event {
                day: r.i32()?,
                kind: decode(&EVENT_KINDS, r.u8()?, "event kind")?,
                text: r.str()?,
                place: r.opt_pt()?,
            });
        }
        s.events = events;
        s.net_dirty = r.bool()?;
        let n = r.len(1)?;
        s.lot_node = (0..n)
            .map(|_| {
                Ok(if r.bool()? {
                    Some((r.u32()?, r.f64()?))
                } else {
                    None
                })
            })
            .collect::<Res<_>>()?;
        s.lot_value = get_f32s(r)?;
        for k in 0..6 {
            s.coverage[k] = get_f32s(r)?;
        }
        s.class_demand = r.by_class_f32()?;
        s.last_ledger = econ::Ledger {
            income: r.by_class_i64()?,
            spending: {
                let mut a = [0i64; 6];
                for x in a.iter_mut() {
                    *x = r.i64()?;
                }
                a
            },
            admin: r.i64()?,
            interest: r.i64()?,
            principal: r.i64()?,
        };
        s.next_milestone = r.usize()?;
        s.warn_level = r.u8()?;
        let n = r.len(8)?;
        s.run_traffic = (0..n).map(|_| get_traffic(r)).collect::<Res<_>>()?;
        s.traffic_cells = get_f32s(r)?;
        s.avg_cong = r.f32()?;
        s.pending = if r.bool()? {
            Some(get_pending(r)?)
        } else {
            None
        };
        s.roads_version = r.u32()?;
        s.rival = Rival {
            name: dynamic::rival_name(r.u8()?).ok_or(LoadError::Invalid("rival"))?,
            tax: r.by_class_u32()?,
        };
        s.departures = r.u32()?;
        s.fires_total = r.u32()?;
        s.month_index = r.u32()?;
        s.values_stale = r.bool()?;
        s.rules = Rules {
            bankruptcy: r.bool()?,
            elections: r.bool()?,
            unlimited_money: r.bool()?,
            difficulty: decode(&Difficulty::ALL, r.u8()?, "difficulty")?,
        };
        s.game_over = if r.bool()? {
            let day = r.i32()?;
            let reason = match r.u8()? {
                0 => GameOverReason::Bankrupt,
                1 => GameOverReason::LostElection {
                    challenger: r.str()?,
                    issue: get_issue(r)?,
                    share: r.i32()?,
                },
                _ => return Err(LoadError::Invalid("game over")),
            };
            Some(GameOver {
                day,
                reason,
                summary: get_summary(r)?,
            })
        } else {
            None
        };
        s.broke_months = r.u32()?;
        s.next_election = r.i32()?;
        s.elections_won = r.u32()?;
        s.last_election = if r.bool()? {
            Some(ElectionResult {
                day: r.i32()?,
                approval: r.i32()?,
                share: r.i32()?,
                won: r.bool()?,
                challenger: r.str()?,
                issue: get_issue(r)?,
            })
        } else {
            None
        };
        s.approval = Approval {
            total: r.i32()?,
            taxes: r.i32()?,
            services: r.i32()?,
            jobs: r.i32()?,
            traffic: r.i32()?,
            debt: r.i32()?,
            fires: r.i32()?,
            worst: get_issue(r)?,
        };
        s.peak_population = r.i64()?;
        let n = r.len(4)?;
        s.fire_days = (0..n).map(|_| r.i32()).collect::<Res<_>>()?;
        for c in Class::ALL {
            s.queue.set(c, r.f64()?);
        }
        for i in 0..4 {
            s.stalled[i] = r.bool()?;
            s.warned[i] = r.bool()?;
        }
        s.jobless = r.bool()?;
        s.last_kind_street = r.i32()?;
        s.growth_spent = r.i64()?;

        // the sizes the rest of the code indexes by
        let nl = s.lots.len();
        if s.next_reveal > s.order.len()
            || s.lot_value.len() > nl
            || s.lot_node.len() > nl
            || s.run_traffic.len() != s.runs.len()
            || s.road_cost_per_month.len() != s.revealed.len() + 1
            || s.vacant.iter().chain(&s.changed_lots).any(|&i| i >= nl)
            || s.lots
                .iter()
                .any(|l| l.addition != usize::MAX && l.addition >= s.revealed.len())
            || s.coverage.iter().any(|c| c.len() > nl)
        {
            return Err(LoadError::Invalid("sizes"));
        }
        // the road graph is a cache of the visible roads; rebuild it unless it was already due
        s.roadnet = None;
        if !s.net_dirty {
            let roads: Vec<(RoadClass, Vec<Pt>)> = s
                .runs
                .iter()
                .filter(|r| s.street_open(r.street))
                .map(|r| (r.class, r.points.clone()))
                .collect();
            s.roadnet = Some(roadnet::RoadNet::build(&roads));
        }
        Ok(s)
    }

    /// Write an autosave into `dir`, keeping the newest [`AUTOSAVE_SLOTS`] files
    /// (`autosave-1.save` is the newest). Returns the path written.
    pub fn autosave(&mut self, dir: &Path) -> std::io::Result<PathBuf> {
        let bytes = self.save();
        std::fs::create_dir_all(dir)?;
        for n in (1..AUTOSAVE_SLOTS).rev() {
            let from = autosave_path(dir, n);
            if from.exists() {
                std::fs::rename(&from, autosave_path(dir, n + 1))?;
            }
        }
        let tmp = dir.join("autosave.tmp");
        std::fs::write(&tmp, &bytes)?;
        let to = autosave_path(dir, 1);
        std::fs::rename(&tmp, &to)?;
        Ok(to)
    }

    /// The newest autosave in `dir` that loads, if any.
    pub fn load_latest_autosave(dir: &Path) -> Option<Sim> {
        (1..=AUTOSAVE_SLOTS)
            .filter_map(|n| std::fs::read(autosave_path(dir, n)).ok())
            .find_map(|b| Sim::load(&b).ok())
    }
}

/// Path of autosave slot `n` (1 is the newest).
pub fn autosave_path(dir: &Path, n: usize) -> PathBuf {
    dir.join(format!("autosave-{n}.save"))
}
