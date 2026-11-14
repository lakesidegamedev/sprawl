//! Buildings change over time: upgrade through five levels, merge lots for big buildings, and
//! decline when the land under them loses value.

use crate::*;

/// Highest level a kind of building reaches.
pub fn max_level(k: LotKind) -> u8 {
    match k {
        LotKind::House => 3,
        LotKind::Row => 4,
        LotKind::Apartments => 5,
        LotKind::Shop => 3,
        LotKind::Office => 5,
        LotKind::Strip | LotKind::BigBox | LotKind::Warehouse => 3,
        LotKind::Workshop => 4,
        LotKind::Gas | LotKind::FastFood => 2,
        _ => 1,
    }
}

/// Days a building must have stood at its level before it can change again.
const SETTLE_DAYS: i32 = 240;
/// Monthly chance that a lot which qualifies starts an upgrade.
const UPGRADE_CHANCE: f64 = 0.12;
const DECLINE_CHANCE: f64 = 0.06;

impl Sim {
    fn services_avg(&self, i: usize) -> f64 {
        let c = |s: Service| self.coverage[s.index()].get(i).copied().unwrap_or(0.0) as f64;
        (c(Service::Police) + c(Service::Fire) + c(Service::Schools) + c(Service::Health)) / 4.0
    }

    /// Land value a lot needs to move up from `level`, and the lowest it can fall to before
    /// the building declines.
    pub fn level_thresholds(level: u8) -> (f32, f32) {
        let l = level as f32;
        (0.42 + 0.07 * l, 0.16 + 0.06 * l)
    }

    /// Monthly pass: start upgrades where land value, demand and services stay high, and let
    /// buildings on poor land decline.
    pub(crate) fn evolve_buildings(&mut self) {
        let day = self.day;
        let (mut upgrades, mut declines) = (0u32, 0u32);
        let (mut up_at, mut down_at): (Option<Pt>, Option<Pt>) = (None, None);
        for i in 0..self.lots.len() {
            let l = &self.lots[i];
            let Phase::Built { since, .. } = l.phase else {
                continue;
            };
            let Some(class) = class_of(l.kind) else {
                continue;
            };
            if l.exempt || l.civic.is_some() || l.fire.is_some() || day - since < SETTLE_DAYS {
                continue;
            }
            let lv = self.lot_value.get(i).copied().unwrap_or(0.0);
            let (up_lv, down_lv) = Sim::level_thresholds(l.level);
            let roll_m = roll(self.seed ^ 0x1e7e1, i as u64, day as u64);
            if l.level < max_level(l.kind)
                && lv >= up_lv
                && self.class_demand.get(class) >= 0.2
                && self.services_avg(i) >= 0.1 * (l.level as f64 - 1.0)
                && roll_m < UPGRADE_CHANCE
                && self.can_upgrade(l.kind, l.level)
            {
                let (k, lv) = (l.kind, l.level);
                self.take_units(k, Sim::units_at(k, lv + 1) - Sim::units_at(k, lv));
                let l = &self.lots[i];
                let days = 30 + 15 * l.level as i32 + (roll_m * 100.0) as i32 % 15;
                let pos = l.pos;
                let next = l.level + 1;
                self.lots[i].next_level = next;
                self.lots[i].phase = Phase::Building {
                    start: day,
                    end: day + days,
                };
                self.changed_lots.push(i);
                if next >= 3 {
                    self.merge_neighbours(i, class);
                }
                upgrades += 1;
                up_at.get_or_insert(pos);
            } else if l.level > 1 && lv < down_lv && roll_m > 1.0 - DECLINE_CHANCE {
                self.lots[i].level -= 1;
                self.lots[i].phase = Phase::Built {
                    since: day,
                    tax_x10: self.lot_rate_x10(i),
                };
                self.changed_lots.push(i);
                declines += 1;
                down_at.get_or_insert(self.lots[i].pos);
            }
        }
        if upgrades > 0 {
            self.log(
                EventKind::Upgrade,
                format!(
                    "{upgrades} building{} being rebuilt bigger.",
                    if upgrades == 1 { " is" } else { "s are" }
                ),
                up_at,
            );
        }
        if declines > 0 {
            self.log(
                EventKind::Decline,
                format!(
                    "{declines} building{} run down on poor land.",
                    if declines == 1 { " has" } else { "s have" }
                ),
                down_at,
            );
        }
    }

    /// A big building takes up to two empty lots of its own class beside it.
    pub(crate) fn merge_neighbours(&mut self, i: usize, class: Class) {
        let pos = self.lots[i].pos;
        let mut near: Vec<(i64, usize)> = Vec::new();
        for (j, m) in self.lots.iter().enumerate() {
            if j != i
                && m.phase == Phase::Vacant
                && class_of(m.kind) == Some(class)
                && m.pos.dist(pos) < 30.0
            {
                near.push(((m.pos.dist(pos) * 100.0) as i64, j));
            }
        }
        near.sort();
        let mut merged = 0;
        for &(_, j) in near.iter().take(2) {
            self.retire_lot(j);
            merged += 1;
        }
        if merged > 0 {
            self.lots[i].radius = (self.lots[i].radius * (1.0 + 0.12 * merged as f64)).min(16.0);
            self.log(
                EventKind::Upgrade,
                format!("{} lots merged for a bigger building.", merged + 1),
                Some(pos),
            );
        }
    }
}
