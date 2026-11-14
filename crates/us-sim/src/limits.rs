//! What limits growth. People and firms arrive from outside at a rate that follows game time,
//! not the player's speed setting, and how many come depends on jobs, taxes, the rival town and
//! how well the town is run. Water, power, school seats and clinics have a capacity: when one is
//! full the kinds of building that need it stop being started, with a news line that says what
//! to build. New streets and utility connections are paid from the treasury when they start,
//! long before the taxes they bring.

use crate::*;

/// People arriving in a month in the first year of a Normal game, with every other factor at 1.
const BASE_PEOPLE_PER_MONTH: f64 = 46.0;
/// How fast the region's pool of movers grows each year.
const POOL_GROWTH_PER_YEAR: f64 = 0.06;
/// Share of the people of a town who work.
const WORKER_SHARE: f64 = 0.45;
/// Households that cannot find work start leaving below this jobs to workers ratio.
const LEAVE_BELOW_RATIO: f64 = 0.6;
/// Warn when a capacity passes this share of its supply.
const WARN_AT: f64 = 0.88;

/// Water a tower carries, in people served (a worker's workplace counts half).
const WATER_PER_TOWER: i64 = 2000;
const WATER_FREE: i64 = 900;
const POWER_PER_PLANT: i64 = 6000;
const POWER_FREE: i64 = 1500;
const SEATS_PER_SCHOOL: i64 = 450;
const SEATS_FREE: i64 = 150;
/// Share of the people who are pupils.
const PUPIL_SHARE_PCT: i64 = 18;
const CLINIC_SERVES: i64 = 2500;
const HEALTH_FREE: i64 = 700;

impl Sim {
    pub fn difficulty(&self) -> Difficulty {
        self.rules.difficulty
    }

    /// What one building adds to a capacity, scaled by the difficulty.
    fn scaled(&self, v: i64) -> i64 {
        v * self.rules.difficulty.capacity_pct() / 100
    }

    /// Demand and supply of one capacity right now.
    pub fn capacity(&self, c: Capacity) -> CapacityStatus {
        let pop = self.population();
        let (mut water, mut power, mut seats, mut clinics) = (0i64, 0i64, 0i64, 0i64);
        for l in &self.lots {
            if !matches!(l.phase, Phase::Built { .. }) {
                continue;
            }
            match l.civic {
                Some(CivicKind::Water) => water += 1,
                Some(CivicKind::Power) => power += 1,
                Some(CivicKind::School) => seats += 1,
                Some(CivicKind::Clinic) => clinics += 1,
                Some(_) => {}
                None => match l.kind {
                    LotKind::Water => water += 1,
                    LotKind::School => seats += 1,
                    _ => {}
                },
            }
        }
        let (demand, supply) = match c {
            Capacity::Water => (
                pop + self.jobs() / 2,
                self.scaled(WATER_FREE + WATER_PER_TOWER * water),
            ),
            Capacity::Power => (
                pop + self.jobs_of(Class::Shops) + 3 * self.jobs_of(Class::Industry),
                self.scaled(POWER_FREE + POWER_PER_PLANT * power),
            ),
            Capacity::Schools => {
                let f = self.budget[Dept::Schools.index()].min(130) as i64;
                (
                    pop * PUPIL_SHARE_PCT / 100,
                    SEATS_FREE + self.scaled(SEATS_PER_SCHOOL * seats) * f / 100,
                )
            }
            Capacity::Health => {
                let f = self.budget[Dept::Health.index()].min(130) as i64;
                (
                    pop,
                    HEALTH_FREE + self.scaled(CLINIC_SERVES * clinics) * f / 100,
                )
            }
        };
        CapacityStatus {
            capacity: c,
            demand,
            supply,
        }
    }

    /// All four capacities.
    pub fn capacities(&self) -> [CapacityStatus; 4] {
        Capacity::ALL.map(|c| self.capacity(c))
    }

    /// The capacities that are full now and so hold growth back (as of the last monthly check).
    pub fn growth_stalls(&self) -> Vec<Capacity> {
        Capacity::ALL
            .into_iter()
            .filter(|c| self.stalled[c.index()])
            .collect()
    }

    /// Whether a lot of this class may be started now.
    pub(crate) fn class_blocked(&self, class: Class) -> bool {
        self.stalled[Capacity::Water.index()]
            || self.stalled[Capacity::Power.index()]
            || (class == Class::Homes
                && (self.stalled[Capacity::Schools.index()]
                    || self.stalled[Capacity::Health.index()]))
    }

    /// People a home holds, or jobs a workplace holds, at a level.
    pub(crate) fn units_at(kind: LotKind, level: u8) -> i64 {
        match class_of(kind) {
            Some(Class::Homes) => kind_pop(kind) * econ::level_value_pct(level) / 100,
            Some(_) => econ::kind_jobs_at(kind, level),
            None => 0,
        }
    }

    /// What starting a building costs the treasury now: the utility connections, which the town
    /// pays and gets back only through taxes over the following years.
    pub(crate) fn hookup_cost(&self, kind: LotKind) -> i64 {
        (400 + kind_value(kind) / 60) * self.rules.difficulty.cost_pct() / 100
    }

    /// Arrivals per month at the pace of this point in game time and the chosen difficulty.
    fn arrival_cap(&self) -> f64 {
        let years = self.month_index as f64 / 12.0;
        BASE_PEOPLE_PER_MONTH
            * libm::pow(1.0 + POOL_GROWTH_PER_YEAR, years)
            * self.rules.difficulty.arrival_pct() as f64
            / 100.0
    }

    /// Dollars paid so far for new streets and utility connections, before the taxes they bring.
    pub fn growth_spending(&self) -> i64 {
        self.growth_spent
    }

    /// Workers' jobs to workers ratio, softened for a very small town.
    pub fn job_ratio(&self) -> f64 {
        let w = self.population() as f64 * WORKER_SHARE;
        (self.jobs() as f64 + 20.0) / (w + 20.0)
    }

    fn tax_factor(&self, c: Class) -> f64 {
        (growth_mult(self.tax.get(c) as f64 / 10.0) / growth_mult(7.0)).clamp(0.05, 1.6)
    }

    /// Monthly: who arrives, who leaves, and which capacities are full.
    pub(crate) fn step_limits(&mut self) {
        // capacities first, so a full one blocks this month's arrivals
        let was = self.stalled;
        let was_warn = self.warned;
        let mut news: Vec<(String, Option<Pt>)> = Vec::new();
        for st in self.capacities() {
            let i = st.capacity.index();
            // pause growth when full; resume with some room so it does not flicker
            self.stalled[i] = if was[i] {
                st.ratio() >= 0.97
            } else {
                st.is_full()
            };
            self.warned[i] = st.ratio() >= WARN_AT;
            if self.stalled[i] && !was[i] {
                news.push((st.capacity.stall_text(), None));
            } else if !self.stalled[i] && was[i] {
                news.push((
                    format!("{} has room again: growth can resume.", st.capacity.label()),
                    None,
                ));
            } else if self.warned[i] && !was_warn[i] && !self.stalled[i] {
                news.push((
                    format!(
                        "{} is {}% used. {}",
                        st.capacity.label(),
                        (st.ratio() * 100.0) as i64,
                        st.capacity.build_hint()
                    ),
                    None,
                ));
            }
        }
        for (text, at) in news {
            self.log(EventKind::Limit, text, at);
        }

        // arrivals
        let pop = self.population() as f64;
        let jobs = self.jobs() as f64;
        let cap = self.arrival_cap();
        let ratio = self.job_ratio();
        let mood = 0.55 + 0.9 * self.approval.total as f64 / 100.0;
        let rival = |me: &Sim, c: Class| (1.0 - 2.0 * me.rival_pull(c)).clamp(0.4, 1.15);
        let homes_attr = ratio.clamp(0.3, 1.2)
            * self.tax_factor(Class::Homes)
            * rival(self, Class::Homes)
            * mood.clamp(0.6, 1.4);
        let shops_attr = self.tax_factor(Class::Shops) * rival(self, Class::Shops);
        let ind_attr = self.tax_factor(Class::Industry) * rival(self, Class::Industry);
        // the town wants people for the jobs it has, customers for its shops, workers for its
        // firms
        let want_pop = 150.0 + 1.25 * jobs / WORKER_SHARE;
        let want_shop = 0.30 * pop + 10.0;
        let want_ind = 0.17 * pop + 6.0;
        let now = |c: Class| self.jobs_of(c) as f64;
        let inflow = [
            ((want_pop - pop - self.queue.homes).max(0.0) * 0.3).min(cap * homes_attr),
            ((want_shop - now(Class::Shops) - self.queue.shops).max(0.0) * 0.3)
                .min(cap * 0.5 * shops_attr),
            ((want_ind - now(Class::Industry) - self.queue.industry).max(0.0) * 0.3)
                .min(cap * 0.3 * ind_attr),
        ];
        for (k, c) in Class::ALL.into_iter().enumerate() {
            let q = self.queue.get(c);
            let add = if self.class_blocked(c) {
                0.0
            } else {
                inflow[k]
            };
            // newcomers who find nothing built give up and go elsewhere
            self.queue.set(c, (q * 0.9 + add).min(cap * 6.0));
        }

        // no jobs: families leave
        if pop >= 200.0 && ratio < LEAVE_BELOW_RATIO {
            // nobody moves in while there is no work
            self.queue.homes = 0.0;
            let n = (((LEAVE_BELOW_RATIO - ratio) * pop / 40.0).ceil() as usize)
                .clamp(1, 12 + pop as usize / 200);
            let mut gone = 0;
            let mut at = None;
            for _ in 0..n {
                if let Some(p) = self.send_away_unemployed() {
                    gone += 1;
                    at.get_or_insert(p);
                }
            }
            if gone > 0 && !self.jobless {
                self.log(
                    EventKind::Limit,
                    "Families are leaving: there are not enough jobs for them.".into(),
                    at,
                );
            }
        }
        self.jobless = ratio < LEAVE_BELOW_RATIO;
    }

    /// The least valued occupied home empties because its household found no work.
    fn send_away_unemployed(&mut self) -> Option<Pt> {
        let day = self.day;
        let mut best: Option<(f64, usize)> = None;
        for (i, l) in self.lots.iter().enumerate() {
            if !matches!(l.phase, Phase::Built { .. })
                || l.exempt
                || l.civic.is_some()
                || l.fire.is_some()
                || class_of(l.kind) != Some(Class::Homes)
            {
                continue;
            }
            let lv = self.lot_value.get(i).copied().unwrap_or(0.5) as f64;
            let score = lv + roll(self.seed ^ 0x70b5, i as u64, day as u64) * 0.3;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, i));
            }
        }
        let (_, i) = best?;
        self.lots[i].phase = Phase::Empty { since: day };
        self.changed_lots.push(i);
        self.departures += 1;
        Some(self.lots[i].pos)
    }

    /// Whether a lot of this kind can be started now, and if so what it takes out of the queue
    /// of arrivals. Lots that are not homes, shops or industry (schools, parks and so on) are
    /// the town's own and always can.
    pub(crate) fn can_start(&self, kind: LotKind) -> bool {
        let Some(c) = class_of(kind) else {
            return true;
        };
        !self.class_blocked(c) && self.queue.get(c) >= 0.5 * Sim::units_at(kind, 1) as f64
    }

    pub(crate) fn take_units(&mut self, kind: LotKind, units: i64) {
        if let Some(c) = class_of(kind) {
            let q = (self.queue.get(c) - units as f64).max(0.0);
            self.queue.set(c, q);
        }
    }

    /// Whether an upgrade to the next level is covered by arrivals.
    pub(crate) fn can_upgrade(&self, kind: LotKind, level: u8) -> bool {
        let Some(c) = class_of(kind) else {
            return false;
        };
        let delta = Sim::units_at(kind, level + 1) - Sim::units_at(kind, level);
        !self.class_blocked(c) && self.queue.get(c) >= delta as f64
    }
}
