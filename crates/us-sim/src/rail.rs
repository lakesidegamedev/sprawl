//! The railway as the town feels it: an open depot pulls commuters to the lots around it, and
//! industry beside the line ships by freight. Nothing here is saved: it is all worked out from
//! the generated town (depots, rails), the street state and the population.

use crate::*;
use us_gen::StationKind;

/// Lots this close to an open depot gain land value (straight line, metres).
pub const DEPOT_REACH_M: f64 = 400.0;
/// Industry this close to the railway gains land value (metres).
pub const FREIGHT_REACH_M: f64 = 600.0;
/// People the town needs before the depot counts as served.
pub const DEPOT_MIN_POP: i64 = 100;
/// Land value at the depot door, fading to nothing at `DEPOT_REACH_M`.
pub const W_DEPOT: f64 = 0.015;
/// Land value of an industrial lot at the track, fading to nothing at `FREIGHT_REACH_M`.
pub const W_FREIGHT: f64 = 0.02;
/// Added to the demand for shops while a depot is open.
pub const SHOP_DEMAND_DEPOT: f32 = 0.04;
/// Added to the demand for industry while industry stands beside the line.
pub const INDUSTRY_DEMAND_FREIGHT: f32 = 0.03;
/// A street within this distance of a depot is the one beside it.
const DEPOT_STREET_M: f64 = 80.0;

/// How often trains run, per game hour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainRates {
    pub passenger: f32,
    pub freight: f32,
}

/// What the railway adds to one lot, each 0 to its weight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailBonus {
    /// Land value from an open depot nearby.
    pub depot: f64,
    /// Land value of an industrial lot from the railway.
    pub freight: f64,
}

/// Trains per game hour for a town of `pop` people: a few in a small town, more in a big one.
pub fn rates_for(pop: i64) -> TrainRates {
    let p = pop.max(0) as f32;
    TrainRates {
        passenger: (1.0 + p / 1500.0).min(8.0),
        freight: (0.5 + p / 3000.0).min(4.0),
    }
}

fn dist_to_line(p: Pt, line: &[Pt]) -> f64 {
    line.windows(2)
        .map(|w| dist_pt_seg(p, w[0], w[1]))
        .fold(f64::MAX, f64::min)
}

impl Sim {
    fn rail_lines(&self) -> Vec<&[Pt]> {
        self.town
            .roads
            .iter()
            .filter(|r| r.class == RoadClass::Rail)
            .map(|r| r.points.as_slice())
            .collect()
    }

    /// Whether the depot at `station` (an index into `Town::stations`) is in service: the town has
    /// some people and the street beside the depot is open.
    pub fn depot_open(&self, station: usize) -> bool {
        let Some(st) = self.town.stations.get(station) else {
            return false;
        };
        if st.kind != StationKind::Depot || self.population() < DEPOT_MIN_POP {
            return false;
        }
        let mut best: Option<(f64, bool)> = None;
        for (i, r) in self.runs.iter().enumerate() {
            if r.class == RoadClass::Rail {
                continue;
            }
            let d = dist_to_line(st.pos, &r.points);
            if d < DEPOT_STREET_M && best.is_none_or(|b| d < b.0) {
                best = Some((d, self.road_open(i)));
            }
        }
        best.is_some_and(|b| b.1)
    }

    /// Centres of the depots that are in service.
    pub fn open_depots(&self) -> Vec<Pt> {
        (0..self.town.stations.len())
            .filter(|&i| self.depot_open(i))
            .map(|i| self.town.stations[i].pos)
            .collect()
    }

    /// Trains per game hour at today's population, raised or lowered by nothing else yet.
    pub fn train_rates(&self) -> TrainRates {
        rates_for(self.population())
    }

    /// The railway's land value bonus for lot `i`.
    pub fn rail_bonus(&self, i: usize) -> RailBonus {
        let depots = self.open_depots();
        let rails = self.rail_lines();
        self.rail_bonus_with(i, &depots, &rails)
    }

    pub(crate) fn rail_bonus_with(&self, i: usize, depots: &[Pt], rails: &[&[Pt]]) -> RailBonus {
        let l = &self.lots[i];
        if l.phase == Phase::Hidden {
            return RailBonus::default();
        }
        let class = class_of(l.kind);
        let mut b = RailBonus::default();
        if class != Some(Class::Industry) {
            let d = depots
                .iter()
                .map(|s| s.dist(l.pos))
                .fold(f64::MAX, f64::min);
            if d < DEPOT_REACH_M {
                b.depot = W_DEPOT * (1.0 - d / DEPOT_REACH_M);
            }
        } else {
            let d = rails
                .iter()
                .map(|r| dist_to_line(l.pos, r))
                .fold(f64::MAX, f64::min);
            if d < FREIGHT_REACH_M {
                b.freight = W_FREIGHT * (1.0 - d / FREIGHT_REACH_M);
            }
        }
        b
    }

    /// Per-lot bonuses for all lots at once (used by the land value pass).
    pub(crate) fn rail_bonuses(&self) -> Vec<RailBonus> {
        let depots = self.open_depots();
        let rails = self.rail_lines();
        if depots.is_empty() && rails.is_empty() {
            return vec![RailBonus::default(); self.lots.len()];
        }
        (0..self.lots.len())
            .map(|i| self.rail_bonus_with(i, &depots, &rails))
            .collect()
    }

    /// Demand added to a class by the railway: commuters for shops, freight for industry.
    pub(crate) fn rail_demand(&self, c: Class) -> f32 {
        match c {
            Class::Shops if !self.open_depots().is_empty() => SHOP_DEMAND_DEPOT,
            Class::Industry => {
                let rails = self.rail_lines();
                let served = self.lots.iter().any(|l| {
                    matches!(l.phase, Phase::Built { .. })
                        && !l.exempt
                        && class_of(l.kind) == Some(Class::Industry)
                        && rails
                            .iter()
                            .any(|r| dist_to_line(l.pos, r) < FREIGHT_REACH_M)
                });
                if served { INDUSTRY_DEMAND_FREIGHT } else { 0.0 }
            }
            _ => 0.0,
        }
    }

    /// The line the why panel adds for a lot the railway helps, if any.
    pub(crate) fn rail_note(&self, lot: usize) -> Option<String> {
        let b = self.rail_bonus(lot);
        if b.depot > 0.0 {
            Some(format!(
                " The depot is near, so commuters come by train: land value +{:.1}%.",
                b.depot * 100.0
            ))
        } else if b.freight > 0.0 {
            Some(format!(
                " The railway is near, so goods move by freight: land value +{:.1}%.",
                b.freight * 100.0
            ))
        } else {
            None
        }
    }
}
