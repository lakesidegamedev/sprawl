//! Population milestones: civic buildings that unlock as the town grows, and what each of the
//! new ones does to the systems around it (land value, traffic, demand, arrivals, approval,
//! capacities, leavers).

use crate::*;

/// How far a library lifts land value, metres.
const LIBRARY_REACH: f64 = 500.0;
/// How far a transit stop eases traffic and lifts land value, metres.
const TRANSIT_REACH: f64 = 450.0;
/// How far a stadium crowds the streets, metres.
const STADIUM_TRAFFIC_REACH: f64 = 300.0;
/// How far a stadium helps the shops around it, metres.
const STADIUM_SHOP_REACH: f64 = 350.0;

/// Seats a high school holds, in schools.
pub(crate) const HIGH_SCHOOL_SEATS: i64 = 3;
/// People a hospital serves, in clinics.
pub(crate) const HOSPITAL_CLINICS: i64 = 4;

/// What the new civic buildings standing near a point do to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Near {
    /// Added to land value (0 to 1 scale).
    pub value: f64,
    /// Share of traffic taken away, 0 to 1.
    pub traffic_relief: f64,
    /// Congestion added, 0 to 1 scale.
    pub traffic_add: f64,
    /// Plain lines for the why panel.
    pub notes: Vec<String>,
}

/// A number with thousands separators.
pub(crate) fn group(v: i64) -> String {
    let s = v.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let out: String = out.chars().rev().collect();
    if v < 0 { format!("-{out}") } else { out }
}

fn bit(k: CivicKind) -> u32 {
    let i = CivicKind::ALL.iter().position(|&c| c == k).unwrap_or(0);
    1 << i
}

impl Sim {
    /// Whether the mayor may build this kind now.
    pub fn is_unlocked(&self, kind: CivicKind) -> bool {
        kind.unlock_at() == 0 || self.unlocked & bit(kind) != 0
    }

    /// The next building to unlock and the population it needs, if any are still locked.
    pub fn next_unlock(&self) -> Option<(CivicKind, i64)> {
        CivicKind::ALL
            .into_iter()
            .filter(|&k| !self.is_unlocked(k))
            .map(|k| (k, k.unlock_at()))
            .min_by_key(|&(_, n)| n)
    }

    /// Unlock every kind the population has reached. Returns the new ones, in order.
    fn unlock_reached(&mut self) -> Vec<CivicKind> {
        let pop = self.population().max(self.peak_population);
        let mut out = Vec::new();
        for k in CivicKind::ALL {
            if k.unlock_at() > 0 && !self.is_unlocked(k) && pop >= k.unlock_at() {
                self.unlocked |= bit(k);
                out.push(k);
            }
        }
        out
    }

    /// Monthly: unlock what the town has grown into and say so.
    pub(crate) fn step_unlocks(&mut self) {
        for k in self.unlock_reached() {
            self.log(
                EventKind::Milestone,
                format!(
                    "The town passed {} people. You can now build a {} (Build window).",
                    group(k.unlock_at()),
                    k.label().to_lowercase()
                ),
                None,
            );
        }
    }

    /// Every unlocked kind at the town's size, with no news. For a save from before unlocks.
    pub(crate) fn unlock_silently(&mut self) {
        self.unlock_reached();
    }

    /// Unlock every building at once, with no news. For screenshots and tests of the interface.
    pub fn unlock_all(&mut self) {
        for k in CivicKind::ALL {
            self.unlocked |= bit(k);
        }
    }

    pub(crate) fn save_mile(&self, w: &mut crate::save::W) {
        w.u32(self.unlocked);
    }

    pub(crate) fn load_mile(&mut self, r: &mut crate::save::R) -> crate::save::Res<()> {
        self.unlocked = r.u32()?;
        Ok(())
    }

    /// Standing buildings that act on their surroundings, rebuilt with the land values.
    pub(crate) fn rebuild_fx_sites(&mut self) {
        self.fx_sites = self
            .lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Built { .. }))
            .filter_map(|l| {
                l.civic
                    .filter(|c| {
                        matches!(
                            c,
                            CivicKind::Library
                                | CivicKind::HighSchool
                                | CivicKind::Hospital
                                | CivicKind::Transit
                                | CivicKind::Stadium
                                | CivicKind::University
                        )
                    })
                    .map(|c| (c, l.pos))
            })
            .collect();
    }

    /// How many of this kind stand and work.
    pub(crate) fn fx_count(&self, kind: CivicKind) -> usize {
        self.fx_sites.iter().filter(|s| s.0 == kind).count()
    }

    /// What the new civic buildings do at `p`; `shop` says the lot is a shop or office.
    pub(crate) fn civic_near(&self, p: Pt, shop: bool) -> Near {
        let mut n = Near::default();
        let mut library = 0.0f64;
        let mut transit = 0.0f64;
        let mut stadium_traffic = 0.0f64;
        let mut stadium_shop = 0.0f64;
        let mut univ = false;
        for &(k, at) in &self.fx_sites {
            let d = p.dist(at);
            match k {
                CivicKind::Library if d < LIBRARY_REACH => {
                    library += 0.05 * (1.0 - d / LIBRARY_REACH);
                }
                CivicKind::Transit if d < TRANSIT_REACH => {
                    transit = transit.max(1.0 - d / TRANSIT_REACH);
                }
                CivicKind::Stadium if d < STADIUM_TRAFFIC_REACH => {
                    stadium_traffic += 0.3 * (1.0 - d / STADIUM_TRAFFIC_REACH);
                }
                CivicKind::University => univ = true,
                _ => {}
            }
            if k == CivicKind::Stadium && shop && d < STADIUM_SHOP_REACH {
                stadium_shop = stadium_shop.max(1.0 - d / STADIUM_SHOP_REACH);
            }
        }
        if library > 0.0 {
            let v = library.min(0.08);
            n.value += v;
            n.notes.push(format!(
                "A library nearby adds {:.0} points of land value.",
                v * 100.0
            ));
        }
        if transit > 0.0 {
            let v = 0.06 * transit;
            n.value += v;
            n.traffic_relief = 0.5 * transit;
            n.notes.push(format!(
                "A transit stop nearby cuts traffic by {:.0}% and adds {:.0} points of land value.",
                n.traffic_relief * 100.0,
                v * 100.0
            ));
        }
        if stadium_traffic > 0.0 {
            n.traffic_add = stadium_traffic.min(0.5);
            n.notes
                .push("A stadium nearby adds traffic on the streets around it.".into());
        }
        if stadium_shop > 0.0 {
            let v = 0.06 * stadium_shop;
            n.value += v;
            n.notes.push(format!(
                "A stadium nearby brings customers: {:.0} points of land value.",
                v * 100.0
            ));
        }
        if univ {
            n.value += 0.04;
            n.notes
                .push("The university adds 4 points of land value across town.".into());
        }
        n
    }

    /// The lines for the why panel about the new civic buildings near a lot.
    pub fn civic_notes(&self, lot: usize) -> Vec<String> {
        let Some(l) = self.lots.get(lot) else {
            return Vec::new();
        };
        if l.phase == Phase::Hidden {
            return Vec::new();
        }
        let shop = class_of(l.kind) == Some(Class::Shops);
        self.civic_near(l.pos, shop).notes
    }

    /// Extra demand for shops and offices from the high schools, stadiums and universities.
    pub(crate) fn shop_demand_bonus(&self) -> f64 {
        let hs = (self.fx_count(CivicKind::HighSchool) as f64 * 0.025).min(0.075);
        let st = (self.fx_count(CivicKind::Stadium) as f64 * 0.02).min(0.06);
        let un = (self.fx_count(CivicKind::University) as f64 * 0.04).min(0.08);
        (hs + st + un).min(0.15)
    }

    /// Share more people arrive: transit stops and universities draw them.
    pub(crate) fn arrival_bonus(&self) -> f64 {
        let t = (self.fx_count(CivicKind::Transit) as f64 * 0.02).min(0.08);
        let u = (self.fx_count(CivicKind::University) as f64 * 0.06).min(0.12);
        1.0 + t + u
    }

    /// Share of the families that would leave who stay because there is a hospital.
    pub(crate) fn leaver_factor(&self) -> f64 {
        (1.0 - 0.25 * self.fx_count(CivicKind::Hospital) as f64).max(0.5)
    }

    /// Approval points from stadiums, added to the services part.
    pub(crate) fn stadium_approval(&self) -> f64 {
        (self.fx_count(CivicKind::Stadium) as f64 * 2.0).min(4.0)
    }

    /// Congestion town-wide after transit stops ease it and stadiums add to it.
    pub(crate) fn civic_adjusted_congestion(&self, avg: f64) -> f64 {
        let relief = (self.fx_count(CivicKind::Transit) as f64 * 0.05).min(0.2);
        let add = self.fx_count(CivicKind::Stadium) as f64 * 0.03;
        (avg * (1.0 - relief) + add + self.weather_congestion()).clamp(0.0, 1.0)
    }

    /// A line to add to the news when a capacity is full and a bigger building is unlocked.
    pub(crate) fn bigger_hint(&self, c: Capacity) -> &'static str {
        match c {
            Capacity::Schools if self.is_unlocked(CivicKind::HighSchool) => {
                " A high school holds three schools' worth of pupils."
            }
            Capacity::Health if self.is_unlocked(CivicKind::Hospital) => {
                " A hospital serves four clinics' worth of people."
            }
            _ => "",
        }
    }
}
