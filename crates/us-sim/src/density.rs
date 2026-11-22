//! Where the town grows tall and how it fills in. Each lot carries a density from the town
//! generator (how central it is: downtown, the station blocks, big crossroads). Builders fill a
//! street from the buildings already standing, central lots start bigger once the town is large
//! enough, and buildings beside taller neighbours rebuild sooner, so towers come in groups
//! downtown while the suburbs stay low.

use crate::growth::max_level;
use crate::*;

/// Lots closer than this (edge to edge, metres) count as neighbours.
const NEIGHBOUR_GAP: f64 = 14.0;

/// Neighbours of every lot of the town, by index, nearest first.
pub(crate) fn neighbours(lots: &[LotState]) -> Vec<Vec<u32>> {
    const CELL: f64 = 64.0;
    let key = |p: Pt| ((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64);
    let mut cells: std::collections::BTreeMap<(i64, i64), Vec<u32>> =
        std::collections::BTreeMap::new();
    for (i, l) in lots.iter().enumerate() {
        cells.entry(key(l.pos)).or_default().push(i as u32);
    }
    lots.iter()
        .enumerate()
        .map(|(i, l)| {
            let (cx, cy) = key(l.pos);
            let mut near: Vec<(i64, u32)> = Vec::new();
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let Some(v) = cells.get(&(cx + dx, cy + dy)) else {
                        continue;
                    };
                    for &j in v {
                        if j as usize == i {
                            continue;
                        }
                        let m = &lots[j as usize];
                        let d = m.pos.dist(l.pos);
                        if d < l.radius + m.radius + NEIGHBOUR_GAP && d < CELL {
                            near.push(((d * 100.0) as i64, j));
                        }
                    }
                }
            }
            near.sort();
            near.into_iter().take(8).map(|(_, j)| j).collect()
        })
        .collect()
}

/// Highest level a building may reach at a density: houses in the outer suburbs stay houses or
/// row houses, only the middle of town takes the tallest towers.
pub fn max_level_at(kind: LotKind, density: f64) -> u8 {
    let cap = if density < 0.15 {
        2
    } else if density < 0.35 {
        3
    } else if density < 0.6 {
        4
    } else {
        5
    };
    max_level(kind).min(cap)
}

impl Sim {
    /// Density 0 to 1 of a lot (0 for buildings the mayor placed).
    pub fn lot_density(&self, i: usize) -> f64 {
        let l = &self.lots[i];
        if l.civic.is_some() {
            return 0.0;
        }
        if self.is_planned_lot(i) {
            return match l.kind {
                LotKind::Apartments | LotKind::Office => 1.0,
                LotKind::Shop | LotKind::Workshop => 0.45,
                _ => 0.10,
            };
        }
        self.town.lots.get(l.src).map_or(0.0, |t| t.density)
    }

    /// Neighbours standing or going up.
    pub(crate) fn built_neighbours(&self, i: usize) -> usize {
        self.near.get(i).map_or(0, |v| {
            v.iter()
                .filter(|&&j| {
                    matches!(
                        self.lots[j as usize].phase,
                        Phase::Built { .. } | Phase::Building { .. }
                    )
                })
                .count()
        })
    }

    /// Neighbours already standing taller than `level`.
    pub(crate) fn taller_neighbours(&self, i: usize, level: u8) -> usize {
        self.near.get(i).map_or(0, |v| {
            v.iter()
                .filter(|&&j| {
                    let m = &self.lots[j as usize];
                    matches!(m.phase, Phase::Built { .. })
                        && m.level > level
                        && m.civic.is_none()
                        && !m.exempt
                        && class_of(m.kind).is_some()
                })
                .count()
        })
    }

    /// New shops in the town centre make better use of a big roadside plot.
    /// Existing buildings and the mayor's own parcels keep their uses.
    pub(crate) fn infill_kind(&self, i: usize) -> LotKind {
        let lot = &self.lots[i];
        if self.rules.city_planning
            || lot.exempt
            || lot.civic.is_some()
            || !matches!(lot.phase, Phase::Vacant)
            || !matches!(
                lot.kind,
                LotKind::BigBox | LotKind::Strip | LotKind::Parking
            )
            || !self.town.central_blocks(lot.pos)
            || self.heritage_at(i)
        {
            return lot.kind;
        }
        match lot.kind {
            LotKind::Parking if self.built_neighbours(i) < 2 => LotKind::Parking,
            _ => LotKind::Shop,
        }
    }

    /// How much a builder likes a vacant lot beyond its value: beside buildings already
    /// standing (a street fills from its built end) and in the middle of things. In metres of
    /// distance from the hall it is worth.
    pub(crate) fn infill_bonus(&self, i: usize) -> f64 {
        let nb = self.built_neighbours(i).min(3) as f64;
        let gap = if nb >= 2.0 && self.town.central_blocks(self.lots[i].pos) {
            105.0
        } else {
            0.0
        };
        80.0 * nb + 90.0 * self.lot_density(i) + gap
    }

    /// The level a new building starts at: the middle of a big enough town builds bigger from
    /// the start, when arrivals cover it.
    pub(crate) fn start_level(&self, i: usize) -> u8 {
        let l = &self.lots[i];
        let Some(class) = class_of(l.kind) else {
            return 1;
        };
        let d = self.lot_density(i);
        let pop = self.population();
        let mut lv = 1u8;
        if d >= 0.7 && pop >= 2500 {
            lv += 1;
        }
        if d >= 0.85 && pop >= 8000 {
            lv += 1;
        }
        let lv = lv.min(self.lot_cap(i, pop)).max(1);
        let lv = if self.heritage_at(i) { 1 } else { lv };
        if lv > 1 && self.queue.get(class) < Sim::units_at(l.kind, lv) as f64 {
            return 1;
        }
        lv
    }

    /// Multiplier on the monthly chance to rebuild bigger: central lots and lots beside taller
    /// buildings go first.
    pub(crate) fn upgrade_pull(&self, i: usize) -> f64 {
        let l = &self.lots[i];
        let d = self.lot_density(i);
        let taller = self.taller_neighbours(i, l.level).min(3) as f64;
        (0.45 + 1.1 * d) * (1.0 + 0.45 * taller) * self.ord_upgrade_pull(i)
    }
}
