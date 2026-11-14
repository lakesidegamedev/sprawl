//! A town that does things on its own: fires where fire cover is weak, a rival town that sets
//! its own taxes and draws people and firms away, and seasons that sway demand.

use crate::*;

/// Days a fire burns before the lot is cleared and rebuilt.
pub const FIRE_DAYS: i32 = 6;
const RIVAL_NAMES: [&str; 4] = ["Ashford", "Bellmont", "Corvale", "Dunmore"];

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

    /// How hard the rival pulls on a class, 0 to 0.3: grows with how much worse our rate is.
    /// Negative (down to -0.15) when our rate is the better one.
    pub(crate) fn rival_pull(&self, c: Class) -> f64 {
        let diff = self.tax.get(c) as f64 - self.rival.tax.get(c) as f64;
        if diff >= 20.0 {
            (diff / 200.0).min(0.3)
        } else if diff <= -20.0 {
            (diff / 200.0).max(-0.15)
        } else {
            0.0
        }
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

    /// Monthly: the rival moves its taxes and draws people and firms from a town that taxes
    /// more.
    pub(crate) fn step_rival(&mut self) {
        self.month_index += 1;
        let m = self.month_index as u64;
        for c in Class::ALL {
            let r = roll(self.seed ^ 0x71a1, m, c.index() as u64);
            if r < 0.10 {
                let step = ((roll(self.seed ^ 0x71a2, m, c.index() as u64) * 4.0) as i32 - 2) * 10;
                let step = if step == 0 { -10 } else { step };
                let old = self.rival.tax.get(c);
                let new = (old as i32 + step).clamp(40, 110) as u32;
                if new != old {
                    self.rival.tax.set(c, new);
                    let verb = if new < old { "cut" } else { "raised" };
                    let name = self.rival.name;
                    self.log(
                        EventKind::Rival,
                        format!(
                            "{name} {verb} its {} tax to {:.1}%.",
                            c.label().to_lowercase(),
                            new as f64 / 10.0
                        ),
                        None,
                    );
                }
            }
            let ours = self.tax.get(c) as i32;
            let theirs = self.rival.tax.get(c) as i32;
            let diff = ours - theirs;
            if diff >= 20 && self.population() >= 60 {
                let n = 1 + diff / 60;
                for _ in 0..n {
                    if roll(self.seed ^ 0x71a3, m * 8 + c.index() as u64, 7)
                        < (diff as f64 / 100.0).min(0.7)
                    {
                        self.send_away(c, ours, theirs);
                    }
                }
            }
        }
    }

    /// The least valued building of a class empties as its people or firm leave.
    fn send_away(&mut self, c: Class, ours: i32, theirs: i32) {
        let day = self.day;
        let mut best: Option<(f64, usize)> = None;
        for (i, l) in self.lots.iter().enumerate() {
            if !matches!(l.phase, Phase::Built { .. })
                || l.exempt
                || l.civic.is_some()
                || l.fire.is_some()
                || class_of(l.kind) != Some(c)
            {
                continue;
            }
            let lv = self.lot_value.get(i).copied().unwrap_or(0.5) as f64;
            let score = lv + roll(self.seed ^ 0x71a4, i as u64, day as u64) * 0.3;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, i));
            }
        }
        let Some((_, i)) = best else { return };
        self.lots[i].phase = Phase::Empty { since: day };
        self.changed_lots.push(i);
        self.departures += 1;
        let who = match c {
            Class::Homes => "A household",
            Class::Shops => "A shop",
            Class::Industry => "A firm",
        };
        let name = self.rival.name;
        let at = self.lots[i].pos;
        self.log(
            EventKind::Rival,
            format!(
                "{who} left for {name}, which taxes {} at {:.1}% against your {:.1}%.",
                c.label().to_lowercase(),
                theirs as f64 / 10.0,
                ours as f64 / 10.0
            ),
            Some(at),
        );
    }
}
