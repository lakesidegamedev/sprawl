//! A town that does things on its own: fires where fire cover is weak, a rival town that sets
//! its own taxes and draws people and firms away, and seasons that sway demand.

use crate::*;

/// Days a fire burns before the lot is cleared and rebuilt.
pub const FIRE_DAYS: i32 = 6;
pub(crate) const RIVAL_NAMES: [&str; 4] = ["Ashford", "Bellmont", "Corvale", "Dunmore"];

pub(crate) fn rival_code(name: &str) -> u8 {
    RIVAL_NAMES.iter().position(|&n| n == name).unwrap_or(0) as u8
}

pub(crate) fn rival_name(code: u8) -> Option<&'static str> {
    RIVAL_NAMES.get(code as usize).copied()
}

#[derive(Clone, Debug)]
pub struct Rival {
    pub name: &'static str,
    /// Its tax rates in tenths of a percent.
    pub tax: ByClass<u32>,
}

impl Rival {
    pub(crate) fn new(seed: u64) -> Rival {
        Rival {
            name: RIVAL_NAMES[(mix(seed, 0x61, 0) % 4) as usize],
            tax: ByClass::all(70),
        }
    }
}

/// Demand swing by month, percent of normal, January first.
fn season_table(c: Class) -> [u32; 12] {
    match c {
        Class::Homes => [96, 96, 98, 101, 104, 106, 106, 105, 102, 99, 97, 96],
        Class::Shops => [92, 94, 97, 99, 101, 102, 102, 103, 100, 103, 108, 112],
        Class::Industry => [98, 99, 101, 102, 103, 102, 100, 100, 102, 103, 100, 97],
    }
}

impl Sim {
    /// The seasonal demand factor now, in percent of normal.
    pub fn season_pct(&self, c: Class) -> u32 {
        season_table(c)[self.date().month as usize - 1]
    }

    pub fn rival(&self) -> &Rival {
        &self.rival
    }

    /// Daily: fires start, burn and are rebuilt.
    pub(crate) fn step_fires(&mut self) {
        let day = self.day;
        for i in 0..self.lots.len() {
            match self.lots[i].fire {
                Some(start) if day - start >= FIRE_DAYS => {
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
                Some(_) => {}
                None => {
                    let l = &self.lots[i];
                    if !matches!(l.phase, Phase::Built { .. })
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
                    let p = 0.000_01 + 0.000_4 * (1.0 - cov) * (1.0 - cov) / 30.0;
                    if roll(self.seed ^ 0xf17e, i as u64, day as u64) < p {
                        self.ignite(i, cov);
                    }
                }
            }
        }
    }

    pub(crate) fn ignite(&mut self, i: usize, cov: f64) {
        let day = self.day;
        let at = self.lots[i].pos;
        self.lots[i].fire = Some(day);
        self.fires_total += 1;
        if cov < 0.3 {
            self.fire_days.push(day);
        }
        let mut spread = 0;
        if cov < 0.3 {
            for j in 0..self.lots.len() {
                let m = &self.lots[j];
                if j != i
                    && spread < 2
                    && m.fire.is_none()
                    && matches!(m.phase, Phase::Built { .. })
                    && !m.exempt
                    && m.civic.is_none()
                    && class_of(m.kind).is_some()
                    && m.pos.dist(at) < 28.0
                    && roll(self.seed ^ 0x5b7ead, j as u64, day as u64) < 0.4
                {
                    self.lots[j].fire = Some(day);
                    self.fires_total += 1;
                    self.fire_days.push(day);
                    spread += 1;
                }
            }
        }
        let name = kind_label(self.lots[i].kind).to_lowercase();
        let text = if cov < 0.3 {
            if spread > 0 {
                format!("A {name} fire spread to {spread} more. No fire station covers this area.")
            } else {
                format!("A {name} burned. No fire station covers this area.")
            }
        } else {
            format!("A {name} caught fire; the fire station put it out.")
        };
        self.log(EventKind::Fire, text, Some(at));
        self.changed_lots.push(i);
    }
}
