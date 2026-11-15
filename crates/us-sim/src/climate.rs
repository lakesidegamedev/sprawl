//! The weather. Each day's weather is a pure function of the seed, the town's style and the
//! date, so tomorrow can be forecast and nothing about it needs saving. What the weather leaves
//! behind does: dryness, snow on the ground and roads, how soaked the ground is, flood water,
//! storm damage and what it cost. That state is the `WTHR` save section.
//!
//! The rest of the simulation reads the weather through `wind`, `dryness`, `rain`, `snowfall`,
//! `snow_cover`, `storm` and `flooded`, and through the consequence hooks further down
//! (fire risk, congestion, demand, approval, land value, capacity).

use crate::save::{R, Res, W};
use crate::*;
use us_gen::Preset;

/// What the sky is doing today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky {
    Clear,
    Cloudy,
    Rain,
    Snow,
    Storm,
    HeatWave,
    Blizzard,
}

impl Sky {
    pub fn label(self) -> &'static str {
        match self {
            Sky::Clear => "Clear",
            Sky::Cloudy => "Cloudy",
            Sky::Rain => "Rain",
            Sky::Snow => "Snow",
            Sky::Storm => "Thunderstorm",
            Sky::HeatWave => "Heat wave",
            Sky::Blizzard => "Blizzard",
        }
    }
}

/// One day of weather.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    pub sky: Sky,
    /// Mean temperature for the day, degrees Celsius.
    pub temp_c: f32,
    /// Wind in metres per second (east is +x, north is +y).
    pub wind: Pt,
    /// How hard it rains or snows, 0 to 1 (0 on dry days).
    pub precip: f32,
}

impl Weather {
    pub fn wind_speed(&self) -> f32 {
        self.wind.len() as f32
    }

    /// The direction the wind blows from, as a compass word.
    pub fn wind_from(&self) -> &'static str {
        const NAMES: [&str; 8] = [
            "west",
            "south west",
            "south",
            "south east",
            "east",
            "north east",
            "north",
            "north west",
        ];
        // a wind blowing toward +x comes from the west
        let ang = libm::atan2(self.wind.y, self.wind.x);
        let k = libm::floor(ang / (std::f64::consts::TAU / 8.0) + 0.5).rem_euclid(8.0) as usize;
        NAMES[k]
    }
}

/// Money the weather took, by month.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeatherCosts {
    pub storm_repairs: i64,
    pub flood_repairs: i64,
    pub plowing: i64,
}

impl WeatherCosts {
    pub fn total(&self) -> i64 {
        self.storm_repairs + self.flood_repairs + self.plowing
    }
}

/// Flood water above the river's level at which the town's lowest lots go under, per unit of
/// soak above `FLOOD_AT`.
const FLOOD_PER_SOAK: f64 = 2.4;
/// Water already this high when the river first leaves its bed, metres.
const FLOOD_BASE: f64 = 1.0;
/// Soak (days of heavy rain, fading) above which the river leaves its bed.
const FLOOD_AT: f64 = 3.0;
/// Lots farther than this from open water never flood.
const FLOOD_REACH_M: f64 = 260.0;
/// How long a lot or a road remembers the weather that hit it, days.
const MEMORY_DAYS: i32 = 120;

/// What a kind of town does to the weather.
struct Style {
    /// Yearly mean temperature and the swing between midwinter and midsummer, Celsius.
    mean: f64,
    amp: f64,
    /// How often and how hard it rains.
    wet: f64,
    /// How readily rain turns into thunderstorms in the warm months.
    storm: f64,
    /// How windy.
    wind: f64,
    /// Where the wind mostly blows toward, radians.
    wind_dir: f64,
    /// How far rain can lift the water (0: none).
    flood: f64,
}

fn style(p: Preset) -> Style {
    match p {
        // prairie: windy, dry, big summer storms
        Preset::Grid => Style {
            mean: 10.0,
            amp: 15.5,
            wet: 0.9,
            storm: 1.2,
            wind: 1.3,
            wind_dir: 0.3,
            flood: 0.5,
        },
        // river valley: wet, calm
        Preset::Winding => Style {
            mean: 11.0,
            amp: 12.5,
            wet: 1.2,
            storm: 1.0,
            wind: 0.8,
            wind_dir: 2.4,
            flood: 1.0,
        },
        // desert suburbs: hot, dry, brief storms
        Preset::Sunbelt => Style {
            mean: 21.0,
            amp: 10.5,
            wet: 0.3,
            storm: 0.9,
            wind: 0.9,
            wind_dir: 1.2,
            flood: 0.0,
        },
        Preset::Crossroads => Style {
            mean: 9.5,
            amp: 14.0,
            wet: 1.0,
            storm: 1.0,
            wind: 1.0,
            wind_dir: 0.0,
            flood: 0.5,
        },
        // lakeshore: cool, snowy, onshore wind
        Preset::LakeTown => Style {
            mean: 8.0,
            amp: 13.0,
            wet: 1.1,
            storm: 0.8,
            wind: 1.1,
            wind_dir: 4.2,
            flood: 0.6,
        },
    }
}

/// Smooth noise in 0..1 that changes over about one unit of `t`.
fn vnoise(seed: u64, chan: u64, t: f64) -> f64 {
    let i = libm::floor(t);
    let f = t - i;
    let s = f * f * (3.0 - 2.0 * f);
    let a = roll(seed ^ 0x57ea_7e5, chan, i as i64 as u64);
    let b = roll(seed ^ 0x57ea_7e5, chan, (i as i64 + 1) as u64);
    a + (b - a) * s
}

/// Pure weather for a day number, for a style of town.
pub(crate) fn weather_on(seed: u64, p: Preset, day: i32) -> Weather {
    let st = style(p);
    let d = Date::from_days(day);
    let phase = ((d.month - 1) as f64 * 30.4 + d.day as f64) / 365.0;
    // -1 in the middle of January, +1 in the middle of July
    let season = -libm::cos(std::f64::consts::TAU * (phase - 15.0 / 365.0));
    let t = day as f64;
    let temp = st.mean
        + st.amp * season
        + 5.5 * (2.0 * vnoise(seed, 1, t / 6.0) - 1.0)
        + 3.5 * (2.0 * vnoise(seed, 2, t / 19.0) - 1.0);
    let wet = 0.7 * vnoise(seed, 3, t / 3.2) + 0.3 * roll(seed ^ 0xd41e, 4, day as u64);
    let thr = (0.62 - 0.12 * (st.wet - 1.0)).clamp(0.5, 0.8);
    let mut precip = if wet > thr {
        0.25 + 0.75 * ((wet - thr) / (1.0 - thr)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let gust = vnoise(seed, 5, t / 2.5);
    let mut speed = st.wind * (2.0 + 6.0 * gust);
    let mut sky = if precip > 0.0 {
        let storm_p = st.storm * (0.06 + 0.42 * season.max(0.0));
        if temp >= 13.0 && precip >= 0.45 && roll(seed ^ 0x5702, 6, day as u64) < storm_p {
            Sky::Storm
        } else if temp <= 0.5 {
            if precip >= 0.7 && temp < -1.0 {
                Sky::Blizzard
            } else {
                Sky::Snow
            }
        } else {
            Sky::Rain
        }
    } else if wet > 0.5 {
        Sky::Cloudy
    } else {
        Sky::Clear
    };
    if precip == 0.0 && temp >= st.mean + st.amp + 3.5 {
        sky = Sky::HeatWave;
    }
    match sky {
        Sky::Storm => speed += 5.0 + 6.0 * precip,
        Sky::Blizzard => speed += 6.0,
        Sky::Snow | Sky::Rain => speed += 1.5,
        _ => {}
    }
    if matches!(sky, Sky::Clear | Sky::Cloudy | Sky::HeatWave) {
        precip = 0.0;
    }
    let ang = st.wind_dir + 1.7 * (2.0 * vnoise(seed, 7, t / 4.0) - 1.0);
    Weather {
        sky,
        temp_c: temp as f32,
        wind: pt(speed * libm::cos(ang), speed * libm::sin(ang)),
        precip: precip as f32,
    }
}

/// What the weather has done to the town, saved in the `WTHR` section.
#[derive(Clone, Debug)]
pub(crate) struct Climate {
    pub dryness: f64,
    /// Days of heavy rain, fading: how soaked the ground is.
    pub soak: f64,
    pub snow_cover: f64,
    pub road_snow: f64,
    /// Flood water above the river's level, metres.
    pub flood_m: f64,
    /// Recent run of bad days, 0 to 1, for shopping demand.
    pub gloom: f64,
    /// Days of power cuts and water shortages, newest last.
    pub shortage_days: Vec<i32>,
    /// Days a flood began, newest last.
    pub flood_days: Vec<i32>,
    /// Lots under water now, ascending.
    pub flooded: Vec<usize>,
    /// Road runs under water now, ascending.
    pub flooded_runs: Vec<usize>,
    /// The day a warning was last shown for.
    pub warned_for: i32,
    /// Costs of this month so far, and of last month.
    pub month: WeatherCosts,
    pub last_month: WeatherCosts,
    /// (lot, day, kind) of what hit a lot lately: 0 flood, 1 storm damage, 2 lightning.
    pub hits: Vec<(u32, i32, u8)>,
    /// Lots lightning struck today (not saved; the renderer flashes there).
    pub strikes: Vec<usize>,
    /// Derived from the town, rebuilt when needed, never saved.
    pub(crate) cache: Option<Cache>,
}

#[derive(Clone, Debug)]
pub(crate) struct Cache {
    /// Per lot: height above the nearest water, metres (infinite when no water is near).
    pub(crate) lot_rel: Vec<f64>,
    /// Per road run: the lowest height above water along it.
    pub(crate) run_rel: Vec<f64>,
}

impl Default for Climate {
    fn default() -> Climate {
        Climate {
            dryness: 0.5,
            soak: 0.0,
            snow_cover: 0.0,
            road_snow: 0.0,
            flood_m: 0.0,
            gloom: 0.25,
            shortage_days: Vec::new(),
            flood_days: Vec::new(),
            flooded: Vec::new(),
            flooded_runs: Vec::new(),
            warned_for: -1,
            month: WeatherCosts::default(),
            last_month: WeatherCosts::default(),
            hits: Vec::new(),
            strikes: Vec::new(),
            cache: None,
        }
    }
}

impl Climate {
    pub(crate) fn hash_into(&self, f: &mut dyn FnMut(u64)) {
        f(self.dryness.to_bits());
        f(self.soak.to_bits());
        f(self.snow_cover.to_bits());
        f(self.road_snow.to_bits());
        f(self.flood_m.to_bits());
        f(self.gloom.to_bits());
        f(self.flooded.len() as u64);
        f(self.flooded_runs.len() as u64);
        f(self.shortage_days.len() as u64);
        f(self.month.total() as u64);
        f(self.last_month.total() as u64);
        f(self.hits.len() as u64);
    }
}

fn pct_of(value: i64, pct_x10: i64) -> i64 {
    value * pct_x10 / 1000
}

impl Sim {
    // ----- the weather as the rest of the game reads it -----

    /// Wind as a vector in metres per second (east is +x, north is +y).
    pub fn wind(&self) -> Pt {
        self.weather().wind
    }

    /// How dry the town is, 0 (soaked) to 1 (drought). Fires catch and spread more when dry.
    pub fn dryness(&self) -> f64 {
        self.wx.dryness
    }

    /// Rain falling now, 0 to 1.
    pub fn rain(&self) -> f32 {
        let w = self.weather();
        if matches!(w.sky, Sky::Rain | Sky::Storm) {
            w.precip
        } else {
            0.0
        }
    }

    /// Snow falling now, 0 to 1.
    pub fn snowfall(&self) -> f32 {
        let w = self.weather();
        if matches!(w.sky, Sky::Snow | Sky::Blizzard) {
            w.precip
        } else {
            0.0
        }
    }

    /// Snow lying on the ground, 0 to 1.
    pub fn snow_cover(&self) -> f32 {
        self.wx.snow_cover as f32
    }

    /// A thunderstorm is over the town now.
    pub fn storm(&self) -> bool {
        self.weather().sky == Sky::Storm
    }

    /// Lots that are under flood water now (lot indices). Empty when there is no flood.
    pub fn flooded(&self) -> &[usize] {
        &self.wx.flooded
    }

    // ----- views for the top bar and the map -----

    /// Today's weather.
    pub fn weather(&self) -> Weather {
        weather_on(self.seed, self.town.preset, self.day)
    }

    /// Tomorrow's weather, as the forecast knows it.
    pub fn weather_tomorrow(&self) -> Weather {
        weather_on(self.seed, self.town.preset, self.day + 1)
    }

    /// Today's temperature in degrees Celsius.
    pub fn temperature_c(&self) -> f32 {
        self.weather().temp_c
    }

    /// A day-ahead warning for the top bar, if tomorrow is bad.
    pub fn weather_warning(&self) -> Option<String> {
        self.warning_for(self.day + 1)
    }

    fn warning_for(&self, day: i32) -> Option<String> {
        let today = weather_on(self.seed, self.town.preset, day - 1);
        let w = weather_on(self.seed, self.town.preset, day);
        let new = today.sky != w.sky;
        match w.sky {
            Sky::Storm if new => Some("A thunderstorm is on its way.".into()),
            Sky::Blizzard if new => Some("A blizzard is on its way.".into()),
            Sky::HeatWave if new => Some("A heat wave is on its way.".into()),
            Sky::Rain | Sky::Storm
                if self.flood_chance(w.precip as f64) && self.wx.flood_m < 0.2 =>
            {
                Some("Heavy rain is on its way. Low ground by the water may flood.".into())
            }
            _ => None,
        }
    }

    /// Whether rain of this strength tomorrow would lift the water out of its bed.
    fn flood_chance(&self, precip: f64) -> bool {
        style(self.town.preset).flood > 0.0 && self.wx.soak * 0.78 + precip > FLOOD_AT - 0.35
    }

    /// Snow lying on the roads, 0 to 1 (plowing clears it).
    pub fn road_snow(&self) -> f32 {
        self.wx.road_snow as f32
    }

    /// Flood water depth above the river, metres (0 when there is no flood).
    pub fn flood_depth(&self) -> f32 {
        self.wx.flood_m as f32
    }

    /// Road runs under water now (indices into `road_runs`), closed to traffic.
    pub fn flooded_roads(&self) -> &[usize] {
        &self.wx.flooded_runs
    }

    /// Lots lightning hit today.
    pub fn lightning_strikes(&self) -> &[usize] {
        &self.wx.strikes
    }

    /// A heat wave is on.
    pub fn heat_wave(&self) -> bool {
        self.weather().sky == Sky::HeatWave
    }

    /// Power was cut or water ran short today.
    pub fn shortage_today(&self) -> bool {
        self.wx.shortage_days.last() == Some(&self.day)
    }

    /// What weather cost the treasury last month and so far this month.
    pub fn weather_costs(&self) -> (WeatherCosts, WeatherCosts) {
        (self.wx.last_month, self.wx.month)
    }

    // ----- hooks the other systems call -----

    /// Multiplier on the chance a fire starts: dry, windy days are worse, rain and snow put fires
    /// out.
    pub(crate) fn fire_weather_factor(&self) -> f64 {
        let w = self.weather();
        let mut f = 0.55 + 0.9 * self.wx.dryness;
        f *= 0.85 + 0.03 * w.wind_speed() as f64;
        if self.rain() > 0.2 {
            f *= 0.3;
        }
        if self.wx.snow_cover > 0.3 {
            f *= 0.2;
        }
        f
    }

    /// Chance a fire jumps to a neighbour at `to`, scaled for the dry air and the wind. `from` is
    /// the burning lot's place.
    pub(crate) fn fire_spread_factor(&self, from: Pt, to: Pt) -> f64 {
        let w = self.wind();
        let dry = 0.7 + 0.6 * self.wx.dryness;
        let sp = w.len();
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let dl = (dx * dx + dy * dy).sqrt();
        if sp < 1.0 || dl < 1.0 {
            return dry;
        }
        let along = (dx * w.x + dy * w.y) / (dl * sp);
        dry * (1.0 + 0.1 * along * sp.min(12.0) / 4.0).clamp(0.3, 1.8)
    }

    /// Extra congestion from rain, snow on the roads and flooded roads, 0 to about 0.3.
    pub(crate) fn weather_congestion(&self) -> f64 {
        let mut c = 0.02 * self.rain() as f64 + 0.18 * self.wx.road_snow;
        if !self.wx.flooded_runs.is_empty() {
            let (mut all, mut shut) = (0.0f64, 0.0f64);
            for (i, r) in self.runs.iter().enumerate() {
                if !self.road_open(i) {
                    continue;
                }
                let len = run_len(&r.points);
                all += len;
                if self.wx.flooded_runs.binary_search(&i).is_ok() {
                    shut += len;
                }
            }
            if all > 0.0 {
                c += (3.0 * shut / all).min(0.25);
            }
        }
        c
    }

    /// Multiplier on a class's demand: bad stretches keep shoppers home. Averages 1.
    pub(crate) fn weather_demand_factor(&self, c: Class) -> f64 {
        if c != Class::Shops {
            return 1.0;
        }
        1.0 - (0.12 * (self.wx.gloom - 0.25)).clamp(-0.03, 0.04)
    }

    /// Approval points the weather has cost the mayor lately (zero or negative).
    pub(crate) fn weather_approval(&self) -> f64 {
        let day = self.day;
        let cuts = self
            .wx
            .shortage_days
            .iter()
            .filter(|&&d| day - d <= 90)
            .count() as f64;
        let floods = self
            .wx
            .flood_days
            .iter()
            .filter(|&&d| day - d <= 120)
            .count() as f64;
        -(1.2 * cuts).min(6.0) - (2.0 * floods).min(5.0)
    }

    /// 0 to 1: how fresh the town's trouble with the weather is, which sends people to the rival
    /// towns.
    pub(crate) fn weather_trouble(&self) -> f64 {
        let day = self.day;
        let last = self
            .wx
            .shortage_days
            .last()
            .into_iter()
            .chain(self.wx.flood_days.last())
            .map(|&d| day - d)
            .min();
        last.map_or(0.0, |a| (1.0 - a as f64 / 45.0).max(0.0))
    }

    /// Land value a lot lost to a flood it had lately.
    pub(crate) fn flood_value_penalty(&self, lot: usize) -> f64 {
        match self
            .wx
            .hits
            .binary_search_by(|h| (h.0 as usize, h.2).cmp(&(lot, 0)))
        {
            Ok(k) => {
                let age = (self.day - self.wx.hits[k].1).max(0) as f64;
                0.07 * (1.0 - age / MEMORY_DAYS as f64).max(0.0)
            }
            Err(_) => 0.0,
        }
    }

    /// Percent of normal use of water or power right now (heat and cold, storms).
    pub(crate) fn weather_use_pct(&self, c: Capacity) -> i64 {
        let w = self.weather();
        let t = w.temp_c as f64;
        match c {
            Capacity::Water => match w.sky {
                Sky::HeatWave => 125,
                _ if t > 28.0 => 108,
                _ => 100,
            },
            Capacity::Power => match w.sky {
                Sky::HeatWave => 112,
                Sky::Storm => 108 + (12.0 * w.precip as f64) as i64,
                Sky::Blizzard => 115,
                _ if t < -8.0 => 110,
                _ => 100,
            },
            _ => 100,
        }
    }

    /// Days new buildings take, longer under building codes.
    pub(crate) fn codes_build_days(&self, lot: usize, days: i32) -> i32 {
        if self.ordinance_at(lot, Ordinance::BuildingCodes) {
            days * 125 / 100
        } else {
            days
        }
    }

    // ----- the daily step -----

    pub(crate) fn ensure_cache(&mut self) {
        if let Some(c) = &self.wx.cache
            && c.lot_rel.len() == self.lots.len()
            && c.run_rel.len() == self.runs.len()
        {
            return;
        }
        let town = &self.town;
        // the surface of each water body: the river's floor, the lake's mean
        let levels: Vec<f64> = town
            .water
            .iter()
            .enumerate()
            .map(|(k, ring)| {
                let hs = ring.iter().map(|&p| town.height_at(p));
                if k == 0 {
                    hs.fold(f64::MAX, f64::min)
                } else {
                    hs.sum::<f64>() / ring.len().max(1) as f64
                }
            })
            .collect();
        let rel_at = |p: Pt| -> f64 {
            let mut best = f64::INFINITY;
            for (ring, &lv) in town.water.iter().zip(&levels) {
                if ring.len() < 3 || point_in_ring(p, ring) {
                    continue;
                }
                if dist_pt_ring_edge(p, ring) <= FLOOD_REACH_M {
                    best = best.min(town.height_at(p) - lv);
                }
            }
            best
        };
        let lot_rel = self.lots.iter().map(|l| rel_at(l.pos)).collect();
        let run_rel = self
            .runs
            .iter()
            .map(|r| {
                if matches!(r.class, RoadClass::Highway | RoadClass::Rail) {
                    return f64::INFINITY;
                }
                let mut m = f64::INFINITY;
                for w in r.points.windows(2) {
                    for k in 0..3 {
                        let p = w[0].lerp(w[1], k as f64 / 3.0);
                        // a bridge stands over the water
                        if !town.water.iter().any(|ring| point_in_ring(p, ring)) {
                            m = m.min(rel_at(p));
                        }
                    }
                }
                m
            })
            .collect();
        self.wx.cache = Some(Cache { lot_rel, run_rel });
    }

    /// Daily, before fires and traffic.
    pub(crate) fn step_weather(&mut self) {
        let day = self.day;
        let w = self.weather();
        self.wx.strikes.clear();
        let t = w.temp_c as f64;
        let precip = w.precip as f64;
        let st = style(self.town.preset);
        let wet_day = matches!(w.sky, Sky::Rain | Sky::Storm);
        let snow_day = matches!(w.sky, Sky::Snow | Sky::Blizzard);

        // dryness
        let c = &mut self.wx;
        if wet_day {
            c.dryness -= 0.13 * precip;
        } else if snow_day {
            c.dryness -= 0.03 * precip;
        } else if t > 2.0 {
            c.dryness += 0.02 + 0.012 * ((t - 15.0) / 15.0).clamp(0.0, 1.0);
            if w.sky == Sky::HeatWave {
                c.dryness += 0.02;
            }
        } else {
            c.dryness += 0.002;
        }
        c.dryness = c.dryness.clamp(0.0, 1.0);

        // snow on the ground
        if snow_day {
            let add = if w.sky == Sky::Blizzard { 0.5 } else { 0.3 };
            c.snow_cover = (c.snow_cover + add * precip).min(1.0);
            c.road_snow = (c.road_snow + 0.6 * precip).min(1.0);
        }
        let mut melt = 0.0;
        if t > 0.0 {
            melt = 0.04 + 0.012 * t.min(12.0);
            c.snow_cover = (c.snow_cover - melt).max(0.0);
            c.road_snow = (c.road_snow - 2.0 * melt).max(0.0);
        } else {
            c.snow_cover = (c.snow_cover - 0.004).max(0.0);
        }

        // plowing
        if self.wx.road_snow > 0.04 {
            self.plow_day();
        }

        // how soaked the ground is, and the flood
        let c = &mut self.wx;
        let melt_in = if t > 2.0 && c.snow_cover > 0.2 {
            6.0 * melt
        } else {
            0.0
        };
        c.soak = c.soak * 0.78 + if wet_day { precip } else { 0.0 } + melt_in;
        let level = if c.soak > FLOOD_AT {
            (FLOOD_BASE + (c.soak - FLOOD_AT) * FLOOD_PER_SOAK) * st.flood
        } else {
            0.0
        };
        let before = c.flood_m;
        c.flood_m = if level > c.flood_m {
            level
        } else {
            (c.flood_m * 0.6 - 0.05).max(0.0)
        };
        if c.flood_m < 0.4 && level < 0.4 {
            c.flood_m = 0.0;
        }
        if self.wx.flood_m > 0.0 || !self.wx.flooded.is_empty() || !self.wx.flooded_runs.is_empty()
        {
            self.step_flood(before);
        }

        // the mood of the weeks
        let bad = (precip >= 0.5
            || matches!(w.sky, Sky::HeatWave | Sky::Blizzard | Sky::Storm)
            || t < -10.0) as u8 as f64;
        self.wx.gloom = self.wx.gloom * 0.9 + 0.1 * bad;

        // storms and heat
        if w.sky == Sky::Storm {
            self.storm_day(&w);
        }
        self.shortage_check(&w);

        // the warning for tomorrow
        if self.wx.warned_for != day + 1 {
            self.wx.warned_for = day + 1;
            if let Some(text) = self.warning_for(day + 1) {
                self.log(EventKind::Weather, text, None);
            }
        }
        if w.sky == Sky::HeatWave
            && weather_on(self.seed, self.town.preset, day - 1).sky != Sky::HeatWave
        {
            self.log(
                EventKind::Weather,
                "A heat wave has started. Water use is up and the ground is drying out.".into(),
                None,
            );
        }
        // forget old trouble
        self.wx.hits.retain(|h| day - h.1 <= MEMORY_DAYS);
        self.wx.shortage_days.retain(|&d| day - d <= 400);
        self.wx.flood_days.retain(|&d| day - d <= 400);
    }

    fn plow_day(&mut self) {
        let share = self.road_share(Ordinance::SnowPlowing);
        let f = self.budget[Dept::Roads.index()].min(130) as f64 / 100.0;
        let rate = 0.22 * f * (1.0 + 1.0 * share);
        let cleared = rate.min(self.wx.road_snow);
        self.wx.road_snow -= cleared;
        // plowing costs by the metre of road, and more at a bigger budget
        let len = self.open_road_metres();
        let cost = (len * 0.035 * f * (cleared / 0.22).min(1.0)) as i64;
        self.wx.month.plowing += cost;
    }

    /// Metres of open road a plow can clear.
    fn open_road_metres(&self) -> f64 {
        let mut len = 0.0;
        for (i, r) in self.runs.iter().enumerate() {
            if !matches!(r.class, RoadClass::Highway | RoadClass::Rail) && self.road_open(i) {
                len += run_len(&r.points);
            }
        }
        len
    }

    /// Share 0 to 1 of the open plowable roads an ordinance covers.
    pub(crate) fn road_share(&self, o: Ordinance) -> f64 {
        let (mut all, mut hit) = (0.0f64, 0.0f64);
        for (i, r) in self.runs.iter().enumerate() {
            if matches!(r.class, RoadClass::Highway | RoadClass::Rail) || !self.road_open(i) {
                continue;
            }
            let len = run_len(&r.points);
            all += len;
            if self.ord_mask_addition(r.addition) & o.bit() != 0 {
                hit += len;
            }
        }
        if all > 0.0 { hit / all } else { 0.0 }
    }

    /// Metres of road that the ordinance covers in a district (`None`: the whole town, whether or
    /// not it is on), counting only what a plow can clear.
    pub(crate) fn plow_metres(&self, district: Option<usize>) -> f64 {
        let mut len = 0.0;
        for (i, r) in self.runs.iter().enumerate() {
            if matches!(r.class, RoadClass::Highway | RoadClass::Rail) || !self.road_open(i) {
                continue;
            }
            if let Some(d) = district
                && r.addition.and_then(|a| self.district_of_addition(a)) != Some(d)
            {
                continue;
            }
            len += run_len(&r.points);
        }
        len
    }

    fn step_flood(&mut self, before: f64) {
        self.ensure_cache();
        let day = self.day;
        let flood = self.wx.flood_m;
        let old: Vec<usize> = std::mem::take(&mut self.wx.flooded);
        let mut now = Vec::new();
        let mut fresh = Vec::new();
        if flood > 0.0 {
            let cache = self.wx.cache.take().unwrap();
            for (i, l) in self.lots.iter().enumerate() {
                if l.exempt || l.phase == Phase::Hidden || l.phase == Phase::Vacant {
                    continue;
                }
                let rel = cache.lot_rel[i];
                if !rel.is_finite() {
                    continue;
                }
                // storm drains carry half the water away
                let level = if self.ordinance_at(i, Ordinance::StormDrains) {
                    flood * 0.5
                } else {
                    flood
                };
                if rel < level {
                    now.push(i);
                    if old.binary_search(&i).is_err() {
                        fresh.push(i);
                    }
                }
            }
            // roads
            let mut runs = Vec::new();
            for (i, &rel) in cache.run_rel.iter().enumerate() {
                if rel.is_finite() && rel < flood && self.road_open(i) {
                    runs.push(i);
                }
            }
            let new_runs = runs
                .iter()
                .filter(|r| self.wx.flooded_runs.binary_search(r).is_err())
                .count();
            self.wx.flooded_runs = runs;
            self.wx.cache = Some(cache);
            if before <= 0.0 {
                self.wx.flood_days.push(day);
            }
            let _ = new_runs;
        } else {
            self.wx.flooded_runs.clear();
        }
        // damage to the lots the water reached for the first time
        let (mut cost, mut homes_out, mut hit) = (0i64, 0u32, 0u32);
        let mut at = None;
        for &i in &fresh {
            let l = &self.lots[i];
            if !matches!(l.phase, Phase::Built { .. }) || l.civic.is_some() {
                continue;
            }
            let value = kind_value(l.kind) * econ::level_value_pct(l.level) / 100;
            cost += pct_of(
                value,
                35 + (roll(self.seed ^ 0xf100d, i as u64, day as u64) * 30.0) as i64,
            );
            hit += 1;
            at.get_or_insert(l.pos);
            self.wx.hits.retain(|h| !(h.0 as usize == i && h.2 == 0));
            let k = self
                .wx
                .hits
                .partition_point(|h| (h.0 as usize, h.2) < (i, 0));
            self.wx.hits.insert(k, (i as u32, day, 0));
            if class_of(l.kind) == Some(Class::Homes)
                && roll(self.seed ^ 0xf100e, i as u64, day as u64) < 0.55
            {
                self.lots[i].phase = Phase::Empty { since: day };
                self.departures += 1;
                homes_out += 1;
                self.changed_lots.push(i);
            }
        }
        self.wx.flooded = now;
        self.wx.month.flood_repairs += cost;
        if hit > 0 {
            let first = before <= 0.0;
            let text = if first {
                format!(
                    "Flood: the river is in {hit} buildings{}. {} road sections are closed and traffic is taking other routes. Repairs cost ${cost}.",
                    if homes_out > 0 {
                        format!(", and {homes_out} households left")
                    } else {
                        String::new()
                    },
                    self.wx.flooded_runs.len()
                )
            } else {
                format!(
                    "The flood rose: {hit} more buildings are under water. Repairs cost ${cost}."
                )
            };
            if first || hit >= 3 {
                self.log(EventKind::Weather, text, at);
            }
        }
        if before > 0.0 && self.wx.flood_m == 0.0 && old.len() + self.wx.flooded.len() > 0 {
            self.log(
                EventKind::Weather,
                "The flood water has gone down. Closed roads are open again.".into(),
                None,
            );
        }
    }

    fn storm_day(&mut self, w: &Weather) {
        let day = self.day;
        let precip = w.precip as f64;
        // wind damage: a few buildings lose roofs and fences
        let taxed: Vec<usize> = (0..self.lots.len())
            .filter(|&i| {
                let l = &self.lots[i];
                matches!(l.phase, Phase::Built { .. })
                    && !l.exempt
                    && l.civic.is_none()
                    && class_of(l.kind).is_some()
            })
            .collect();
        if taxed.is_empty() {
            return;
        }
        let want = (taxed.len() as f64 / 250.0
            * precip
            * (0.5 + roll(self.seed ^ 0x570a, 8, day as u64))) as usize;
        let want = want.min(12);
        let (mut cost, mut n) = (0i64, 0u32);
        let mut at = None;
        for k in 0..want {
            let pick = taxed
                [(mix(self.seed ^ 0x570b, day as u64, k as u64) % taxed.len() as u64) as usize];
            let l = &self.lots[pick];
            let value = kind_value(l.kind) * econ::level_value_pct(l.level) / 100;
            let mut c = pct_of(
                value,
                15 + (roll(self.seed ^ 0x570c, pick as u64, day as u64) * 25.0) as i64,
            );
            if self.ordinance_at(pick, Ordinance::BuildingCodes) {
                c /= 2;
            }
            cost += c;
            n += 1;
            at.get_or_insert(self.lots[pick].pos);
            let key = (pick as u32, 1u8);
            self.wx.hits.retain(|h| (h.0, h.2) != key);
            let at_k = self
                .wx
                .hits
                .partition_point(|h| (h.0 as usize, h.2) < (pick, 1));
            self.wx.hits.insert(at_k, (pick as u32, day, 1));
        }
        self.wx.month.storm_repairs += cost;
        if n > 0 {
            self.log(
                EventKind::Weather,
                format!("The storm damaged {n} buildings. Repairs cost ${cost}."),
                at,
            );
        }
        // lightning
        let strikes = (1.0 + 2.0 * precip * roll(self.seed ^ 0x570d, 9, day as u64)) as usize;
        for k in 0..strikes {
            let pick = taxed
                [(mix(self.seed ^ 0x570e, day as u64, k as u64) % taxed.len() as u64) as usize];
            self.wx.strikes.push(pick);
            if self.lots[pick].fire.is_none()
                && roll(self.seed ^ 0x570f, pick as u64, day as u64) < 0.25
            {
                let cov = self.coverage[Service::Fire.index()]
                    .get(pick)
                    .copied()
                    .unwrap_or(0.0) as f64;
                self.ignite_by(pick, cov, true);
                let key = (pick as u32, 2u8);
                self.wx.hits.retain(|h| (h.0, h.2) != key);
                let at_k = self
                    .wx
                    .hits
                    .partition_point(|h| (h.0 as usize, h.2) < (pick, 2));
                self.wx.hits.insert(at_k, (pick as u32, day, 2));
            }
        }
    }

    /// Storms and heat waves load the grid and the water. When a capacity is already near its
    /// limit it gives out for the day.
    fn shortage_check(&mut self, w: &Weather) {
        let power = w.sky == Sky::Storm || w.sky == Sky::Blizzard;
        let water = w.sky == Sky::HeatWave;
        if !power && !water {
            return;
        }
        let c = if water {
            Capacity::Water
        } else {
            Capacity::Power
        };
        let st = self.capacity(c);
        if st.supply > 0 && st.demand as f64 > st.supply as f64 * 0.96 {
            self.wx.shortage_days.push(self.day);
            let text = if water {
                "Water ran short in the heat. Growth is paused for the day. A bigger water supply would prevent this."
            } else if w.sky == Sky::Storm {
                "The storm knocked out power across the town. Growth is paused for the day. More power capacity would prevent this."
            } else {
                "The blizzard overloaded the power grid. Growth is paused for the day. More power capacity would prevent this."
            };
            self.log(EventKind::Weather, text.into(), None);
        }
    }

    /// Take what the weather cost this month and start a new month.
    pub(crate) fn take_weather_costs(&mut self) -> i64 {
        let m = std::mem::take(&mut self.wx.month);
        self.wx.last_month = m;
        m.total()
    }

    /// The weather lines for a lot's why panel.
    pub(crate) fn weather_lines(&self, i: usize) -> Vec<String> {
        let mut out = Vec::new();
        let Some(l) = self.lots.get(i) else {
            return out;
        };
        if l.phase == Phase::Hidden || l.exempt {
            return out;
        }
        let day = self.day;
        if self.wx.flooded.binary_search(&i).is_ok() {
            out.push("Flood: river water is in this building. Repairs are paid from the treasury and households may leave.".into());
        } else if let Some(h) = self.wx.hits.iter().find(|h| h.0 as usize == i && h.2 == 0) {
            out.push(format!(
                "Flooded {} days ago: land value here is lower for a while.",
                day - h.1
            ));
        }
        if let Some(h) = self.wx.hits.iter().find(|h| h.0 as usize == i && h.2 == 1) {
            out.push(format!(
                "Storm damage {} days ago, repaired at the town's cost.",
                day - h.1
            ));
        }
        if let Some(h) = self.wx.hits.iter().find(|h| h.0 as usize == i && h.2 == 2) {
            out.push(format!(
                "Lightning started a fire here {} days ago.",
                day - h.1
            ));
        }
        if class_of(l.kind).is_some() && l.civic.is_none() {
            if self.wx.dryness > 0.75 && matches!(l.phase, Phase::Built { .. }) {
                out.push("Dry spell: fires start and spread more easily.".into());
            }
            if self.wx.road_snow > 0.3 {
                out.push("Snow on the roads slows traffic here until it is plowed.".into());
            }
        }
        out
    }

    // ----- save -----

    pub(crate) fn save_weather(&self, w: &mut W) {
        let c = &self.wx;
        w.u8(1);
        for v in [
            c.dryness,
            c.soak,
            c.snow_cover,
            c.road_snow,
            c.flood_m,
            c.gloom,
        ] {
            w.f64(v);
        }
        w.usize(c.shortage_days.len());
        for &d in &c.shortage_days {
            w.i32(d);
        }
        w.usize(c.flood_days.len());
        for &d in &c.flood_days {
            w.i32(d);
        }
        w.usize(c.flooded.len());
        for &i in &c.flooded {
            w.u32(i as u32);
        }
        w.usize(c.flooded_runs.len());
        for &i in &c.flooded_runs {
            w.u32(i as u32);
        }
        w.i32(c.warned_for);
        for m in [c.month, c.last_month] {
            w.i64(m.storm_repairs);
            w.i64(m.flood_repairs);
            w.i64(m.plowing);
        }
        w.usize(c.hits.len());
        for &(lot, day, kind) in &c.hits {
            w.u32(lot);
            w.i32(day);
            w.u8(kind);
        }
    }

    pub(crate) fn load_weather(&mut self, r: &mut R) -> Res<()> {
        if r.u8()? != 1 {
            return Ok(());
        }
        let mut c = Climate {
            dryness: r.f64()?.clamp(0.0, 1.0),
            soak: r.f64()?.max(0.0),
            snow_cover: r.f64()?.clamp(0.0, 1.0),
            road_snow: r.f64()?.clamp(0.0, 1.0),
            flood_m: r.f64()?.max(0.0),
            gloom: r.f64()?.clamp(0.0, 1.0),
            ..Climate::default()
        };
        let n = r.len(4)?;
        for _ in 0..n {
            c.shortage_days.push(r.i32()?);
        }
        let n = r.len(4)?;
        for _ in 0..n {
            c.flood_days.push(r.i32()?);
        }
        let lots = self.lots.len();
        let n = r.len(4)?;
        for _ in 0..n {
            let i = r.u32()? as usize;
            if i >= lots {
                return Err(crate::save::LoadError::Invalid("flooded lot"));
            }
            c.flooded.push(i);
        }
        let runs = self.runs.len();
        let n = r.len(4)?;
        for _ in 0..n {
            let i = r.u32()? as usize;
            if i >= runs {
                return Err(crate::save::LoadError::Invalid("flooded road"));
            }
            c.flooded_runs.push(i);
        }
        c.warned_for = r.i32()?;
        for slot in 0..2 {
            let m = WeatherCosts {
                storm_repairs: r.i64()?,
                flood_repairs: r.i64()?,
                plowing: r.i64()?,
            };
            if slot == 0 {
                c.month = m;
            } else {
                c.last_month = m;
            }
        }
        let n = r.len(9)?;
        for _ in 0..n {
            let lot = r.u32()?;
            let day = r.i32()?;
            let kind = r.u8()?;
            if lot as usize >= lots {
                return Err(crate::save::LoadError::Invalid("weather lot"));
            }
            c.hits.push((lot, day, kind));
        }
        c.flooded.sort_unstable();
        c.flooded_runs.sort_unstable();
        c.hits.sort_by_key(|h| (h.0, h.2));
        self.wx = c;
        Ok(())
    }
}

fn run_len(points: &[Pt]) -> f64 {
    points.windows(2).map(|w| w[0].dist(w[1])).sum()
}
