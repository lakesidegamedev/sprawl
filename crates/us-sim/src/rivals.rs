//! Three rival towns off the map. The first is the rival kept in the save body (a commuter
//! suburb); the other two and every rival's size and trade with the town live in the `RIVL`
//! save section. Each sets its own taxes over time, reacts to the mayor's rates, and trades
//! households, shops and firms with the town: a rival that taxes a class lower than the town
//! draws that class away, and a town that taxes lower and is well run draws it back.

use crate::dynamic::RIVAL_NAMES;
use crate::save::{LoadError, R, Res, W};
use crate::*;

pub const TOWNS: usize = 3;

const PORT_NAMES: [&str; 4] = ["Ironquay", "Saltwick", "Kilnport", "Tidemere"];
const MALL_NAMES: [&str; 4] = ["Marketon", "Mercerton", "Cornhill", "Weldmarket"];

/// People (or jobs) one building moves, per class, when the town has no better figure.
const UNIT: [f64; 3] = [4.0, 6.0, 8.0];
const MIN_TAX: i32 = 40;
const MAX_TAX: i32 = 110;

/// What a rival town is like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Personality {
    /// Low taxes on homes: a commuter suburb that pulls households.
    Suburb,
    /// Low taxes on industry: a port and factory town that pulls firms.
    Port,
    /// Low taxes on shops: a shopping and office city that pulls shops.
    Mall,
}

impl Personality {
    pub fn label(self) -> &'static str {
        match self {
            Personality::Suburb => "Commuter suburb",
            Personality::Port => "Port and factory town",
            Personality::Mall => "Shopping and office city",
        }
    }

    /// The class this kind of town is built to attract.
    pub fn pulls(self) -> Class {
        match self {
            Personality::Suburb => Class::Homes,
            Personality::Port => Class::Industry,
            Personality::Mall => Class::Shops,
        }
    }

    /// Where its taxes drift to, tenths of a percent.
    fn target(self, c: Class) -> i32 {
        if c == self.pulls() {
            return 50;
        }
        match (self, c) {
            (Personality::Suburb, _) => 80,
            (Personality::Port, Class::Shops) => 70,
            (Personality::Port, _) => 80,
            (Personality::Mall, Class::Industry) => 70,
            (Personality::Mall, _) => 80,
        }
    }

    fn base_pop(self) -> i64 {
        match self {
            Personality::Suburb => 1800,
            Personality::Port => 2600,
            Personality::Mall => 3200,
        }
    }

    fn names(self) -> &'static [&'static str; 4] {
        match self {
            Personality::Suburb => &RIVAL_NAMES,
            Personality::Port => &PORT_NAMES,
            Personality::Mall => &MALL_NAMES,
        }
    }
}

const KINDS: [Personality; TOWNS] = [Personality::Suburb, Personality::Port, Personality::Mall];

/// How strongly rival `i` pulls on a class: fully on its own, less on the rest.
fn weight(i: usize, c: Class) -> f64 {
    if KINDS[i].pulls() == c { 1.0 } else { 0.6 }
}

/// The name and taxes of the second and third rival (the first lives in the save body).
#[derive(Clone, Debug)]
pub(crate) struct Extra {
    name_code: u8,
    tax: ByClass<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct State {
    pop: i64,
    pop_year_ago: i64,
    /// Buildings taken from the town this year, per class (negative: won back by the town).
    took: ByClass<i32>,
    took_last: ByClass<i32>,
    /// Buildings the town is winning back, not yet a whole one.
    carry: ByClass<f32>,
    ahead: bool,
    /// The mayor's taxes at the last month, to see a rise.
    last_ours: ByClass<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct Rivals {
    extra: [Extra; 2],
    state: [State; TOWNS],
}

impl Rivals {
    pub(crate) fn new(seed: u64) -> Rivals {
        let extra = |i: usize| Extra {
            name_code: (mix(seed, 0x6a + i as u64, 0) % 4) as u8,
            tax: ByClass::all(70),
        };
        let state = |i: usize| {
            let pop = KINDS[i].base_pop() + (mix(seed, 0x62 + i as u64, 0) % 600) as i64;
            State {
                pop,
                pop_year_ago: pop,
                took: ByClass::all(0),
                took_last: ByClass::all(0),
                carry: ByClass::all(0.0),
                ahead: true,
                last_ours: ByClass::all(70),
            }
        };
        Rivals {
            extra: [extra(1), extra(2)],
            state: [state(0), state(1), state(2)],
        }
    }

    pub(crate) fn fingerprint(&self, f: &mut dyn FnMut(u64)) {
        for e in &self.extra {
            f(e.name_code as u64);
            for c in Class::ALL {
                f(e.tax.get(c) as u64);
            }
        }
        for s in &self.state {
            f(s.pop as u64);
            f(s.pop_year_ago as u64);
            f(s.ahead as u64);
            for c in Class::ALL {
                f(s.took.get(c) as u64);
                f(s.took_last.get(c) as u64);
                f(s.carry.get(c).to_bits() as u64);
                f(s.last_ours.get(c) as u64);
            }
        }
    }
}

/// One rival town as the screens see it.
#[derive(Clone, Debug)]
pub struct RivalView {
    pub name: &'static str,
    pub personality: Personality,
    /// Tax rates in tenths of a percent.
    pub tax: ByClass<u32>,
    pub pop: i64,
    /// Change in people over the last year.
    pub trend: i64,
    /// Buildings it took from the town this year per class; negative means the town won them back.
    pub took: ByClass<i32>,
    pub took_last: ByClass<i32>,
    pub bigger_than_town: bool,
}

fn raw_pull(diff: f64) -> f64 {
    if diff >= 20.0 {
        (diff / 200.0).min(0.3)
    } else if diff <= -20.0 {
        (diff / 200.0).max(-0.15)
    } else {
        0.0
    }
}

impl Sim {
    pub fn rival_name_of(&self, i: usize) -> &'static str {
        if i == 0 {
            self.rival.name
        } else {
            KINDS[i].names()[self.rivals.extra[i - 1].name_code as usize % 4]
        }
    }

    pub fn rival_tax_of(&self, i: usize) -> ByClass<u32> {
        if i == 0 {
            self.rival.tax
        } else {
            self.rivals.extra[i - 1].tax
        }
    }

    fn set_rival_tax(&mut self, i: usize, c: Class, v: u32) {
        if i == 0 {
            self.rival.tax.set(c, v);
        } else {
            self.rivals.extra[i - 1].tax.set(c, v);
        }
    }

    /// The three rival towns, the commuter suburb first.
    pub fn rival_towns(&self) -> Vec<RivalView> {
        let pop = self.population();
        (0..TOWNS)
            .map(|i| {
                let s = &self.rivals.state[i];
                RivalView {
                    name: self.rival_name_of(i),
                    personality: KINDS[i],
                    tax: self.rival_tax_of(i),
                    pop: s.pop,
                    trend: s.pop - s.pop_year_ago,
                    took: s.took,
                    took_last: s.took_last,
                    bigger_than_town: s.pop > pop,
                }
            })
            .collect()
    }

    /// How hard the rival towns pull on a class, about -0.15 to 0.3: grows with how much worse
    /// our rate is than the best rival for that class, negative when we are cheaper than them.
    pub(crate) fn rival_pull(&self, c: Class) -> f64 {
        let ours = self.tax.get(c) as f64;
        let mut pos = 0.0f64;
        let mut neg = 0.0f64;
        for i in 0..TOWNS {
            let raw = raw_pull(ours - self.rival_tax_of(i).get(c) as f64);
            if raw > 0.0 {
                pos = pos.max(raw * weight(i, c));
            } else {
                neg += raw;
            }
        }
        pos + neg.max(-0.15)
    }

    /// The rival that took the most from the town this year and last, if any took anything.
    pub fn rival_taking_most(&self) -> Option<&'static str> {
        let mut best: Option<(i32, usize)> = None;
        for (i, s) in self.rivals.state.iter().enumerate() {
            let n: i32 = Class::ALL
                .iter()
                .map(|&c| s.took.get(c).max(0) + s.took_last.get(c).max(0))
                .sum();
            if n > 0 && best.is_none_or(|b| n > b.0) {
                best = Some((n, i));
            }
        }
        best.map(|(_, i)| self.rival_name_of(i))
    }

    /// The line for the why panel of a lot, when a rival town bears on it.
    pub(crate) fn rival_why(&self, lot: usize) -> Option<String> {
        let l = self.lots.get(lot)?;
        if l.exempt
            || l.civic.is_some()
            || !matches!(
                l.phase,
                Phase::Built { .. } | Phase::Empty { .. } | Phase::Vacant
            )
        {
            return None;
        }
        let c = class_of(l.kind)?;
        let ours = self.tax.get(c) as i32;
        let mut best: Option<(f64, usize)> = None;
        for i in 0..TOWNS {
            let d = (ours - self.rival_tax_of(i).get(c) as i32) as f64 * weight(i, c);
            if d >= 20.0 && best.is_none_or(|b| d > b.0) {
                best = Some((d, i));
            }
        }
        let (_, i) = best?;
        Some(format!(
            "{} taxes {} at {:.1}% against your {:.1}%, so {} may leave for it.",
            self.rival_name_of(i),
            c.label().to_lowercase(),
            self.rival_tax_of(i).get(c) as f64 / 10.0,
            ours as f64 / 10.0,
            match c {
                Class::Homes => "households",
                Class::Shops => "shops",
                Class::Industry => "firms",
            }
        ))
    }

    /// Monthly: every rival moves its taxes, then trades with the town.
    pub(crate) fn step_rival(&mut self) {
        self.month_index += 1;
        let m = self.month_index as u64;
        let ours = self.tax;
        for i in 0..TOWNS {
            self.step_rival_taxes(i, m, ours);
        }
        self.step_rival_trade(m, ours);
        let player = self.population();
        for i in 0..TOWNS {
            let s = &mut self.rivals.state[i];
            s.last_ours = ours;
            // slow growth of its own
            s.pop = ((s.pop as f64 * 1.0020) as i64).clamp(300, 60_000);
            let now_ahead = s.pop > player;
            let was = s.ahead;
            s.ahead = now_ahead;
            if now_ahead && !was && player >= 100 {
                let (name, pop) = (self.rival_name_of(i), self.rivals.state[i].pop);
                self.log(
                    EventKind::Rival,
                    format!("{name} has overtaken your town: {pop} people against {player}."),
                    None,
                );
            }
        }
        if m.is_multiple_of(12) {
            for s in self.rivals.state.iter_mut() {
                s.took_last = s.took;
                s.took = ByClass::all(0);
                s.pop_year_ago = s.pop;
            }
        }
    }

    fn step_rival_taxes(&mut self, i: usize, m: u64, ours: ByClass<u32>) {
        let key = m + ((i as u64) << 32);
        let kind = KINDS[i];
        let growing = {
            let s = &self.rivals.state[i];
            s.pop_year_ago > 0 && s.pop * 100 >= s.pop_year_ago * 105
        };
        let walk_p = if i == 0 { 0.10 } else { 0.06 };
        for c in Class::ALL {
            let ci = c.index() as u64;
            let old = self.rival_tax_of(i).get(c);
            let mut new = old;
            let mut why = "";
            let r = roll(self.seed ^ 0x71a1, key, ci);
            if r < walk_p {
                let step = ((roll(self.seed ^ 0x71a2, key, ci) * 4.0) as i32 - 2) * 10;
                let step = if step == 0 { -10 } else { step };
                new = (old as i32 + step).clamp(MIN_TAX, MAX_TAX) as u32;
            } else if (kind.target(c) - old as i32).abs() >= 10
                && roll(self.seed ^ 0x71b1, key, ci) < 0.12
            {
                let toward = (kind.target(c) - old as i32).signum() * 10;
                new = (old as i32 + toward).clamp(MIN_TAX, MAX_TAX) as u32;
            }
            // undercut a class the mayor just taxed more
            let raised = ours.get(c) > self.rivals.state[i].last_ours.get(c);
            if raised
                && roll(self.seed ^ 0x71b2, key, ci) < if kind.pulls() == c { 0.6 } else { 0.25 }
            {
                let floor = if kind.pulls() == c { MIN_TAX } else { 50 };
                let want = (ours.get(c) as i32 - 10).clamp(floor, MAX_TAX) as u32;
                if want < new {
                    new = want;
                    why = " to undercut yours";
                }
            }
            // a growing town charges more
            if growing
                && new == old
                && roll(self.seed ^ 0x71b3, key, ci) < 0.04
                && (old as i32) < MAX_TAX
            {
                new = old + 10;
                why = " as it grows";
            }
            if new != old {
                self.set_rival_tax(i, c, new);
                let verb = if new < old { "cut" } else { "raised" };
                let name = self.rival_name_of(i);
                self.log(
                    EventKind::Rival,
                    format!(
                        "{name} {verb} its {} tax to {:.1}%{why}.",
                        c.label().to_lowercase(),
                        new as f64 / 10.0
                    ),
                    None,
                );
            }
        }
    }

    fn step_rival_trade(&mut self, m: u64, ours: ByClass<u32>) {
        let pop_ok = self.population() >= 60;
        let well_run = self.approval.total >= 55;
        for c in Class::ALL {
            let ci = c.index();
            // the best rival for this class draws households, shops or firms away
            let mut best: Option<(f64, usize)> = None;
            for i in 0..TOWNS {
                let diff = ours.get(c) as f64 - self.rival_tax_of(i).get(c) as f64;
                let d = diff * weight(i, c);
                if diff >= 20.0 && best.is_none_or(|b| d > b.0) {
                    best = Some((d, i));
                }
            }
            if let (Some((_, i)), true) = (best, pop_ok) {
                let diff = ours.get(c) as i32 - self.rival_tax_of(i).get(c) as i32;
                let n = 1 + diff / 60;
                let p = (diff as f64 / 100.0).min(0.7) * weight(i, c);
                for _ in 0..n {
                    if roll(self.seed ^ 0x71a3, m * 8 + ci as u64, 7) < p {
                        self.send_away(c, i);
                    }
                }
            }
            // a cheaper, well run town draws them back
            if !well_run {
                continue;
            }
            for i in 0..TOWNS {
                let diff = self.rival_tax_of(i).get(c) as f64 - ours.get(c) as f64;
                if diff < 20.0 {
                    continue;
                }
                let s = &self.rivals.state[i];
                let rate = (s.pop as f64 / 1500.0) * 0.12 * (diff / 20.0).min(3.0) * weight(i, c);
                let carry = s.carry.get(c) as f64 + rate;
                let whole = carry.floor();
                let s = &mut self.rivals.state[i];
                s.carry.set(c, (carry - whole) as f32);
                if whole >= 1.0 {
                    let n = whole as i32;
                    s.took.set(c, s.took.get(c) - n);
                    s.pop = (s.pop - (UNIT[ci] as i64) * n as i64).max(300);
                    if !self.class_blocked(c) {
                        let q = self.queue.get(c) + UNIT[ci] * n as f64;
                        self.queue.set(c, q);
                    }
                    if roll(self.seed ^ 0x71b4, m, ci as u64) < 0.3 {
                        let name = self.rival_name_of(i);
                        let who = match c {
                            Class::Homes => "Households",
                            Class::Shops => "Shops",
                            Class::Industry => "Firms",
                        };
                        self.log(
                            EventKind::Rival,
                            format!(
                                "{who} are moving from {name} to your town, where {} tax is lower.",
                                c.label().to_lowercase()
                            ),
                            None,
                        );
                    }
                }
            }
        }
    }

    /// The least valued building of a class empties as its people or firm leave for rival `i`.
    fn send_away(&mut self, c: Class, i: usize) {
        let day = self.day;
        let ours = self.tax.get(c) as i32;
        let theirs = self.rival_tax_of(i).get(c) as i32;
        let mut best: Option<(f64, usize)> = None;
        for (j, l) in self.lots.iter().enumerate() {
            if !matches!(l.phase, Phase::Built { .. })
                || l.exempt
                || l.civic.is_some()
                || l.fire.is_some()
                || class_of(l.kind) != Some(c)
            {
                continue;
            }
            let lv = self.lot_value.get(j).copied().unwrap_or(0.5) as f64;
            let score = lv + roll(self.seed ^ 0x71a4, j as u64, day as u64) * 0.3;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, j));
            }
        }
        let Some((_, j)) = best else { return };
        let moved = Sim::units_at(self.lots[j].kind, self.lots[j].level).max(1);
        self.lots[j].phase = Phase::Empty { since: day };
        self.changed_lots.push(j);
        self.departures += 1;
        let s = &mut self.rivals.state[i];
        s.pop += moved;
        s.took.set(c, s.took.get(c) + 1);
        let who = match c {
            Class::Homes => "A household",
            Class::Shops => "A shop",
            Class::Industry => "A firm",
        };
        let name = self.rival_name_of(i);
        let at = self.lots[j].pos;
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

    // ----- the RIVL save section -----

    pub(crate) fn save_rivals(&self, w: &mut W) {
        w.u8(1);
        for e in &self.rivals.extra {
            w.u8(e.name_code);
            w.by_class_u32(e.tax);
        }
        for s in &self.rivals.state {
            w.i64(s.pop);
            w.i64(s.pop_year_ago);
            for c in Class::ALL {
                w.i32(s.took.get(c));
            }
            for c in Class::ALL {
                w.i32(s.took_last.get(c));
            }
            for c in Class::ALL {
                w.f32(s.carry.get(c));
            }
            w.bool(s.ahead);
            w.by_class_u32(s.last_ours);
        }
    }

    pub(crate) fn load_rivals(&mut self, r: &mut R) -> Res<()> {
        if r.u8()? != 1 {
            return Err(LoadError::Invalid("rivals"));
        }
        let mut out = Rivals::new(self.seed);
        for e in out.extra.iter_mut() {
            e.name_code = r.u8()? % 4;
            e.tax = r.by_class_u32()?;
        }
        for s in out.state.iter_mut() {
            s.pop = r.i64()?.clamp(0, 1_000_000);
            s.pop_year_ago = r.i64()?.clamp(0, 1_000_000);
            for c in Class::ALL {
                s.took.set(c, r.i32()?);
            }
            for c in Class::ALL {
                s.took_last.set(c, r.i32()?);
            }
            for c in Class::ALL {
                s.carry.set(c, r.f32()?);
            }
            s.ahead = r.bool()?;
            s.last_ours = r.by_class_u32()?;
        }
        self.rivals = out;
        Ok(())
    }
}
