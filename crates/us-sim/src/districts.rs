//! Districts and ordinances.
//!
//! The mayor groups neighbourhoods (generator additions) into named districts and switches
//! ordinances on for the whole town or for single districts. Every ordinance is a real hook in
//! the simulation (see the `Sim` methods below and the places that call them): the density cap,
//! upgrades, land value, demand, coverage, fires, tax income and approval. The state is saved in
//! its own section (`DIST`); a game that never used it has no section and nothing here changes
//! any number.

use crate::density::max_level_at;
use crate::growth::max_level;
use crate::save::{R, Res, W};
use crate::*;

pub const MAX_DISTRICTS: usize = 12;
/// Dollars a month the snow plowing ordinance costs per metre of road it covers.
const PLOW_FEE_PER_M: f64 = 0.5;
const NO_DISTRICT: u8 = u8::MAX;

/// Names given to new districts, in order, skipping the ones in use.
const NAMES: [&str; 16] = [
    "Millbrook",
    "Harrow Hill",
    "Cobble Row",
    "Ferry Reach",
    "Tanners Yard",
    "Linden Park",
    "Saltmarsh",
    "Quarry Hill",
    "Alder Court",
    "Brickfield",
    "Copper Lane",
    "Westmere",
    "Old Foundry",
    "Sunnyside",
    "Granary Row",
    "Kiln Hill",
];

/// Twelve colours that stay apart on the map.
pub const DISTRICT_COLOURS: [[u8; 3]; MAX_DISTRICTS] = [
    [230, 159, 0],
    [86, 180, 233],
    [0, 158, 115],
    [204, 121, 167],
    [213, 94, 0],
    [240, 228, 66],
    [120, 94, 240],
    [140, 200, 90],
    [220, 80, 80],
    [60, 120, 200],
    [170, 170, 170],
    [100, 220, 210],
];

/// Where an ordinance can be switched on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrdScope {
    Town,
    District,
    Both,
}

impl OrdScope {
    pub fn text(self) -> &'static str {
        match self {
            OrdScope::Town => "Town",
            OrdScope::District => "District",
            OrdScope::Both => "Town or district",
        }
    }
}

/// A policy. Codes in saves are positions in `ALL`: only ever add to the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ordinance {
    HeightLimit,
    Upzoning,
    Heritage,
    SmokeAlarms,
    Watch,
    BusinessBreak,
    Tourism,
    CleanIndustry,
    TrafficCalming,
    Recycling,
    StormDrains,
    BuildingCodes,
    SnowPlowing,
}

impl Ordinance {
    pub const ALL: [Ordinance; 13] = [
        Ordinance::HeightLimit,
        Ordinance::Upzoning,
        Ordinance::Heritage,
        Ordinance::SmokeAlarms,
        Ordinance::Watch,
        Ordinance::BusinessBreak,
        Ordinance::Tourism,
        Ordinance::CleanIndustry,
        Ordinance::TrafficCalming,
        Ordinance::Recycling,
        Ordinance::StormDrains,
        Ordinance::BuildingCodes,
        Ordinance::SnowPlowing,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn bit(self) -> u16 {
        1 << self.index()
    }

    pub fn label(self) -> &'static str {
        match self {
            Ordinance::HeightLimit => "Height limit",
            Ordinance::Upzoning => "Downtown upzoning",
            Ordinance::Heritage => "Heritage protection",
            Ordinance::SmokeAlarms => "Smoke alarm programme",
            Ordinance::Watch => "Neighbourhood watch",
            Ordinance::BusinessBreak => "New business tax break",
            Ordinance::Tourism => "Tourism campaign",
            Ordinance::CleanIndustry => "Clean industry rules",
            Ordinance::TrafficCalming => "Traffic calming",
            Ordinance::Recycling => "Recycling programme",
            Ordinance::StormDrains => "Storm drains",
            Ordinance::BuildingCodes => "Building codes",
            Ordinance::SnowPlowing => "Snow plowing",
        }
    }

    /// One line on what it does.
    pub fn effect(self) -> &'static str {
        match self {
            Ordinance::HeightLimit => {
                "No new building rises above level 2. Residents are happier. Taller buildings already standing stay."
            }
            Ordinance::Upzoning => {
                "Central lots may go one level above their usual limit and rebuild bigger sooner. More traffic, and some neighbours object."
            }
            Ordinance::Heritage => {
                "No rebuilding bigger and no merging lots. Land value is higher. Small upkeep per building."
            }
            Ordinance::SmokeAlarms => "Half as many fires. Costs a fee per resident.",
            Ordinance::Watch => {
                "Police cover is stronger, so voters feel safer. Costs a fee per resident."
            }
            Ordinance::BusinessBreak => {
                "Shops and industry pay 20 percent less tax and count the tax rate as 1 point lower. Demand for both is higher."
            }
            Ordinance::Tourism => {
                "More demand for shops. Costs a flat amount that grows with the population."
            }
            Ordinance::CleanIndustry => {
                "Industry hurts nearby land value half as much. Industry is a little less in demand. Fee per industrial building."
            }
            Ordinance::TrafficCalming => {
                "Traffic hurts home land value less and voters like it. Shops and industry lose a little value to slower streets. Fee per building."
            }
            Ordinance::Recycling => "Voters like it. Costs a fee per resident.",
            Ordinance::StormDrains => {
                "Floods reach only half as far here and do half the damage. Fee per building."
            }
            Ordinance::BuildingCodes => {
                "Storm damage costs half as much. New buildings take a quarter longer to finish. Fee per building."
            }
            Ordinance::SnowPlowing => {
                "Snow clears off the roads twice as fast, so traffic recovers sooner. Fee per metre of road."
            }
        }
    }

    pub fn scope(self) -> OrdScope {
        match self {
            Ordinance::HeightLimit
            | Ordinance::Upzoning
            | Ordinance::Heritage
            | Ordinance::TrafficCalming => OrdScope::District,
            Ordinance::BusinessBreak | Ordinance::Tourism => OrdScope::Town,
            _ => OrdScope::Both,
        }
    }
}

/// A named group of neighbourhoods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct District {
    pub name: String,
    /// Position in `DISTRICT_COLOURS`.
    pub colour: u8,
    /// The additions (neighbourhoods) in it.
    pub additions: Vec<usize>,
    ordinances: u16,
}

impl District {
    pub fn rgb(&self) -> [u8; 3] {
        DISTRICT_COLOURS[self.colour as usize % MAX_DISTRICTS]
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DistrictState {
    list: Vec<District>,
    /// Ordinances on for the whole town, as a bit set.
    town: u16,
    /// Per addition: index into `list`, or `NO_DISTRICT`. Empty until the first assignment.
    add_dist: Vec<u8>,
    /// What the ordinances cost at the last monthly close.
    pub(crate) last_cost: i64,
}

impl DistrictState {
    pub(crate) fn is_default(&self) -> bool {
        self.list.is_empty() && self.town == 0 && self.last_cost == 0
    }

    fn rebuild(&mut self, n_add: usize) {
        self.add_dist = vec![NO_DISTRICT; n_add];
        for (k, d) in self.list.iter().enumerate() {
            for &a in &d.additions {
                if a < n_add {
                    self.add_dist[a] = k as u8;
                }
            }
        }
    }

    pub(crate) fn hash_into(&self, f: &mut dyn FnMut(u64)) {
        f(self.town as u64);
        f(self.last_cost as u64);
        f(self.list.len() as u64);
        for d in &self.list {
            f(d.colour as u64);
            f(d.ordinances as u64);
            f(d.additions.len() as u64);
            for &a in &d.additions {
                f(a as u64);
            }
        }
    }
}

/// What one district looks like today.
#[derive(Clone, Debug)]
pub struct DistrictStats {
    pub people: i64,
    pub jobs: i64,
    pub buildings: u32,
    pub avg_land_value: f32,
    /// Mean building level of homes, shops and industry; 0 with none.
    pub avg_level: f32,
    /// Tax per month at today's state.
    pub tax_income: i64,
    /// Ordinance fees per month in this district, town-wide ordinances included.
    pub ordinance_cost: i64,
    /// The thing hurting it most, in plain words.
    pub problem: String,
}

impl Sim {
    // ----- districts -----

    pub fn districts(&self) -> &[District] {
        &self.dist.list
    }

    /// Make a new empty district with the next free name and colour. `None` at the limit.
    pub fn create_district(&mut self) -> Option<usize> {
        let d = &mut self.dist;
        if d.list.len() >= MAX_DISTRICTS {
            return None;
        }
        let name = NAMES
            .iter()
            .find(|n| d.list.iter().all(|x| x.name != **n))
            .map_or_else(
                || format!("District {}", d.list.len() + 1),
                |n| n.to_string(),
            );
        let colour = (0..MAX_DISTRICTS as u8)
            .find(|c| d.list.iter().all(|x| x.colour != *c))
            .unwrap_or(0);
        d.list.push(District {
            name,
            colour,
            additions: Vec::new(),
            ordinances: 0,
        });
        let n = self.revealed.len();
        self.dist.rebuild(n);
        Some(self.dist.list.len() - 1)
    }

    pub fn rename_district(&mut self, d: usize, name: &str) {
        let name: String = name.trim().chars().take(24).collect();
        if let Some(x) = self.dist.list.get_mut(d)
            && !name.is_empty()
        {
            x.name = name;
        }
    }

    /// Delete a district; its neighbourhoods and ordinances go with it. Later districts move up
    /// one place.
    pub fn delete_district(&mut self, d: usize) {
        if d < self.dist.list.len() {
            self.dist.list.remove(d);
            let n = self.revealed.len();
            self.dist.rebuild(n);
            self.values_stale = true;
        }
    }

    /// The district a neighbourhood is in.
    pub fn district_of_addition(&self, a: usize) -> Option<usize> {
        match self.dist.add_dist.get(a) {
            Some(&k) if k != NO_DISTRICT => Some(k as usize),
            _ => None,
        }
    }

    pub fn district_of_lot(&self, i: usize) -> Option<usize> {
        self.district_of_addition(self.lots.get(i)?.addition)
    }

    /// Paint tool: put the neighbourhood in district `d`, or take it out if it is already
    /// there. A neighbourhood is in one district at a time. Returns whether it is now in `d`.
    pub fn toggle_addition(&mut self, d: usize, a: usize) -> bool {
        if d >= self.dist.list.len() || a >= self.revealed.len() || self.is_farm_idx(a) {
            return false;
        }
        let was = self.district_of_addition(a);
        if let Some(w) = was {
            self.dist.list[w].additions.retain(|&x| x != a);
        }
        let now = was != Some(d);
        if now {
            self.dist.list[d].additions.push(a);
            self.dist.list[d].additions.sort_unstable();
        }
        let n = self.revealed.len();
        self.dist.rebuild(n);
        self.values_stale = true;
        now
    }

    // ----- ordinances -----

    pub fn ordinance_town(&self, o: Ordinance) -> bool {
        self.dist.town & o.bit() != 0
    }

    pub fn ordinance_in_district(&self, d: usize, o: Ordinance) -> bool {
        self.dist
            .list
            .get(d)
            .is_some_and(|x| x.ordinances & o.bit() != 0)
    }

    /// Switch an ordinance on or off for the whole town. Ignored for district-only ones.
    pub fn set_ordinance_town(&mut self, o: Ordinance, on: bool) {
        if o.scope() == OrdScope::District {
            return;
        }
        if on {
            self.dist.town |= o.bit();
        } else {
            self.dist.town &= !o.bit();
        }
        self.values_stale = true;
    }

    /// Switch an ordinance on or off for one district. Ignored for town-only ones.
    pub fn set_ordinance_district(&mut self, d: usize, o: Ordinance, on: bool) {
        if o.scope() == OrdScope::Town {
            return;
        }
        if let Some(x) = self.dist.list.get_mut(d) {
            if on {
                x.ordinances |= o.bit();
            } else {
                x.ordinances &= !o.bit();
            }
            self.values_stale = true;
        }
    }

    fn any_ordinance(&self) -> bool {
        self.dist.town != 0 || self.dist.list.iter().any(|d| d.ordinances != 0)
    }

    /// The ordinances that apply to a lot, as a bit set.
    fn ord_mask(&self, i: usize) -> u16 {
        let d = &self.dist;
        let m = d.town;
        match self.lots.get(i).and_then(|l| d.add_dist.get(l.addition)) {
            Some(&k) if k != NO_DISTRICT => m | d.list[k as usize].ordinances,
            _ => m,
        }
    }

    /// The ordinances that apply to a road run in an addition (`None`: the old skeleton, which
    /// only town-wide ordinances cover).
    pub(crate) fn ord_mask_addition(&self, addition: Option<usize>) -> u16 {
        let d = &self.dist;
        match addition.and_then(|a| d.add_dist.get(a)) {
            Some(&k) if k != NO_DISTRICT => d.town | d.list[k as usize].ordinances,
            _ => d.town,
        }
    }

    /// Whether an ordinance applies to a lot.
    pub fn ordinance_at(&self, i: usize, o: Ordinance) -> bool {
        self.ord_mask(i) & o.bit() != 0
    }

    /// Share 0 to 1 of the lots of a class (all taxed lots for `None`) that an ordinance covers.
    fn ord_share(&self, o: Ordinance, class: Option<Class>) -> f64 {
        let (mut all, mut hit) = (0u32, 0u32);
        for (i, l) in self.lots.iter().enumerate() {
            let c = class_of(l.kind);
            if l.phase == Phase::Hidden
                || l.exempt
                || l.civic.is_some()
                || c.is_none()
                || (class.is_some() && c != class)
            {
                continue;
            }
            all += 1;
            if self.ord_mask(i) & o.bit() != 0 {
                hit += 1;
            }
        }
        if all == 0 {
            0.0
        } else {
            hit as f64 / all as f64
        }
    }

    // ----- hooks: density, upgrades -----

    /// Highest level a building on this lot may reach, with ordinances.
    pub(crate) fn lot_cap(&self, i: usize, population: i64) -> u8 {
        let l = &self.lots[i];
        let d = self.lot_density(i);
        let mut cap = max_level_at(l.kind, d);
        let m = self.ord_mask(i);
        if m & Ordinance::Upzoning.bit() != 0 && d >= 0.35 {
            cap = (cap + 1).min(max_level(l.kind));
        }
        if m & Ordinance::HeightLimit.bit() != 0 {
            cap = cap.min(2);
        }
        if cap < 4 || !matches!(l.kind, LotKind::Office | LotKind::Apartments) {
            return cap;
        }
        let value = self.lot_value.get(i).copied().unwrap_or(0.0);
        let cover = self.services_avg(i);
        let neighbors = self.taller_neighbours(i, 1);
        if value < 0.58 || cover < 0.23 || neighbors < 2 || population < 3_000 {
            return 3;
        }
        if d < 0.72
            || value < 0.69
            || cover < 0.42
            || self.taller_neighbours(i, 2) < 3
            || population < 10_000
        {
            return cap.min(4);
        }
        cap
    }

    /// Heritage lots are never rebuilt bigger or merged.
    pub(crate) fn heritage_at(&self, i: usize) -> bool {
        self.ord_mask(i) & Ordinance::Heritage.bit() != 0
    }

    /// Multiplier on the chance to rebuild bigger.
    pub(crate) fn ord_upgrade_pull(&self, i: usize) -> f64 {
        if self.ord_mask(i) & Ordinance::Upzoning.bit() != 0 && self.lot_density(i) >= 0.35 {
            1.35
        } else {
            1.0
        }
    }

    // ----- hooks: land value, traffic, coverage, demand, fires, tax -----

    /// Land value points an ordinance adds to a lot (heritage).
    pub(crate) fn ord_value_bonus(&self, i: usize) -> f64 {
        if self.ord_mask(i) & Ordinance::Heritage.bit() != 0 {
            0.05
        } else {
            0.0
        }
    }

    /// How much the industry term of a lot counts: clean industry rules halve it.
    pub(crate) fn ord_industry_factor(&self, i: usize) -> f64 {
        if self.ord_mask(i) & Ordinance::CleanIndustry.bit() != 0 {
            0.5
        } else {
            1.0
        }
    }

    /// Traffic 0 to 1 as it weighs on a lot's value.
    pub(crate) fn lot_traffic_eff(&self, i: usize) -> f64 {
        let t = self.lot_traffic(i);
        let m = self.ord_mask(i);
        if m == 0 || t == 0.0 {
            return t;
        }
        let mut f = 1.0;
        if m & Ordinance::TrafficCalming.bit() != 0 {
            f *= if class_of(self.lots[i].kind) == Some(Class::Homes) {
                0.6
            } else {
                1.15
            };
        }
        if m & Ordinance::Upzoning.bit() != 0 && self.lot_density(i) >= 0.35 {
            f *= 1.25;
        }
        (t * f).min(1.0)
    }

    /// Police cover of a lot after the neighbourhood watch.
    pub(crate) fn ord_watch_cover(&self, i: usize, cov: f32) -> f32 {
        if self.ord_mask(i) & Ordinance::Watch.bit() != 0 {
            (cov * 1.25 + 0.1).min(1.0)
        } else {
            cov
        }
    }

    /// Multiplier on a class's demand from ordinances.
    pub(crate) fn ord_demand_factor(&self, c: Class) -> f64 {
        if !self.any_ordinance() {
            return 1.0;
        }
        let mut f = 1.0;
        match c {
            Class::Homes => {}
            Class::Shops => {
                f += 0.15 * self.ord_share(Ordinance::BusinessBreak, Some(c));
                f += 0.10 * self.ord_share(Ordinance::Tourism, Some(c));
            }
            Class::Industry => {
                f += 0.15 * self.ord_share(Ordinance::BusinessBreak, Some(c));
                f -= 0.10 * self.ord_share(Ordinance::CleanIndustry, Some(c));
            }
        }
        f
    }

    /// Multiplier on the chance of a fire on a lot.
    pub(crate) fn ord_fire_factor(&self, i: usize) -> f64 {
        if self.ord_mask(i) & Ordinance::SmokeAlarms.bit() != 0 {
            0.5
        } else {
            1.0
        }
    }

    /// The tax rate a lot is judged by for abandonment, tenths of a percent.
    pub(crate) fn lot_rate_eff_x10(&self, i: usize) -> u32 {
        let r = self.lot_rate_x10(i);
        if self.ord_mask(i) & Ordinance::BusinessBreak.bit() != 0
            && class_of(self.lots[i].kind) != Some(Class::Homes)
        {
            r.saturating_sub(10)
        } else {
            r
        }
    }

    /// Tax a lot pays per month, with the business break.
    pub(crate) fn lot_tax_month(&self, i: usize) -> i64 {
        let l = &self.lots[i];
        let Some(c) = class_of(l.kind) else {
            return 0;
        };
        let value = kind_value(l.kind) * econ::level_value_pct(l.level) / 100;
        let add = value * self.tax.get(c) as i64 / 1000 / 12;
        if c != Class::Homes && self.ord_mask(i) & Ordinance::BusinessBreak.bit() != 0 {
            add * 80 / 100
        } else {
            add
        }
    }

    // ----- costs -----

    fn lot_people(l: &LotState) -> i64 {
        kind_pop(l.kind) * econ::level_value_pct(l.level) / 100
    }

    /// Fee per month of one ordinance on one standing lot, in dollars (business break is lost
    /// tax, counted by `ordinance_estimate`, not a fee).
    fn lot_fee(&self, i: usize, o: Ordinance) -> i64 {
        let l = &self.lots[i];
        if !matches!(l.phase, Phase::Built { .. }) || l.exempt || l.civic.is_some() {
            return 0;
        }
        let Some(c) = class_of(l.kind) else {
            return 0;
        };
        match o {
            Ordinance::SmokeAlarms => 3 * Sim::lot_people(l),
            Ordinance::Watch => 3 * Sim::lot_people(l),
            Ordinance::Recycling => 4 * Sim::lot_people(l),
            Ordinance::Heritage => 40,
            Ordinance::StormDrains => 8,
            Ordinance::BuildingCodes => 12,
            Ordinance::TrafficCalming => 15,
            Ordinance::CleanIndustry if c == Class::Industry => 60,
            _ => 0,
        }
    }

    /// Flat monthly cost of the tourism campaign at today's population.
    fn tourism_cost(&self) -> i64 {
        250 + self.population() / 2
    }

    /// Total ordinance fees per month at today's state.
    pub fn ordinance_costs(&self) -> i64 {
        if !self.any_ordinance() {
            return 0;
        }
        let mut sum = 0;
        for i in 0..self.lots.len() {
            let m = self.ord_mask(i);
            if m == 0 {
                continue;
            }
            for o in Ordinance::ALL {
                if m & o.bit() != 0 {
                    sum += self.lot_fee(i, o);
                }
            }
        }
        if self.ordinance_town(Ordinance::Tourism) {
            sum += self.tourism_cost();
        }
        sum + self.plow_fee_now()
    }

    /// Fee per month for plowing, per metre of road in the places that have it on.
    fn plow_fee_now(&self) -> i64 {
        let share = self.road_share(Ordinance::SnowPlowing);
        if share <= 0.0 {
            return 0;
        }
        (share * self.plow_metres(None) * PLOW_FEE_PER_M) as i64
    }

    /// What an ordinance costs per month over the town (`district` of `None`) or one district,
    /// whether or not it is on there now. Negative for fees and lost tax.
    pub fn ordinance_estimate(&self, o: Ordinance, district: Option<usize>) -> i64 {
        let mut sum = 0;
        for (i, l) in self.lots.iter().enumerate() {
            if let Some(d) = district
                && self.district_of_addition(l.addition) != Some(d)
            {
                continue;
            }
            sum += self.lot_fee(i, o);
            if o == Ordinance::BusinessBreak
                && matches!(l.phase, Phase::Built { .. })
                && !l.exempt
                && let Some(c) = class_of(l.kind)
                && c != Class::Homes
            {
                let value = kind_value(l.kind) * econ::level_value_pct(l.level) / 100;
                sum += value * self.tax.get(c) as i64 / 1000 / 12 * 20 / 100;
            }
        }
        if o == Ordinance::Tourism && district.is_none() {
            sum += self.tourism_cost();
        }
        if o == Ordinance::SnowPlowing {
            sum += (self.plow_metres(district) * PLOW_FEE_PER_M) as i64;
        }
        -sum
    }

    // ----- approval -----

    /// Approval points each active ordinance adds (or takes), rounded, nonzero only. They are
    /// already inside the services and traffic parts of `Approval`.
    pub fn ordinance_approval(&self) -> Vec<(Ordinance, i32)> {
        self.ord_points()
            .into_iter()
            .map(|(o, p)| (o, libm::floor(p + 0.5) as i32))
            .filter(|&(_, p)| p != 0)
            .collect()
    }

    fn ord_points(&self) -> Vec<(Ordinance, f64)> {
        let mut out = Vec::new();
        if !self.any_ordinance() {
            return out;
        }
        let per = |o: Ordinance| match o {
            Ordinance::HeightLimit => 2.0,
            Ordinance::Heritage => 1.0,
            Ordinance::Recycling => 2.0,
            Ordinance::TrafficCalming => 2.0,
            Ordinance::Upzoning => -1.0,
            _ => 0.0,
        };
        for o in Ordinance::ALL {
            let p = per(o);
            if p != 0.0 {
                let s = self.ord_share(o, Some(Class::Homes));
                if s > 0.0 {
                    out.push((o, p * s));
                }
            }
        }
        out
    }

    /// (services part, traffic part) of the approval that ordinances add.
    pub(crate) fn ord_approval_parts(&self) -> (f64, f64) {
        let (mut services, mut traffic) = (0.0, 0.0);
        for (o, p) in self.ord_points() {
            if o == Ordinance::TrafficCalming {
                traffic += p;
            } else {
                services += p;
            }
        }
        (services, traffic)
    }

    /// What the challenger adds on election night, if it fits the issue: promising a missing
    /// ordinance or attacking the fees.
    pub(crate) fn challenger_line(&self, issue: Issue) -> String {
        let town_on = |o| self.ordinance_town(o);
        match issue {
            Issue::Traffic if !town_on(Ordinance::TrafficCalming) => {
                " They promised traffic calming.".into()
            }
            Issue::Fires if !town_on(Ordinance::SmokeAlarms) => {
                " They promised a smoke alarm programme.".into()
            }
            Issue::Services(Service::Police) if !town_on(Ordinance::Watch) => {
                " They promised a neighbourhood watch.".into()
            }
            Issue::Debt | Issue::Taxes if self.dist.last_cost > 0 => {
                format!(
                    " They said the ordinances, ${} a month, are wasted money.",
                    self.dist.last_cost
                )
            }
            Issue::Jobs if !town_on(Ordinance::BusinessBreak) => {
                " They promised a tax break for new business.".into()
            }
            _ => String::new(),
        }
    }

    // ----- views -----

    /// Lines for the why panel: the district and every ordinance acting on this lot.
    pub fn ordinance_lines(&self, i: usize) -> Vec<String> {
        let mut out = Vec::new();
        let Some(l) = self.lots.get(i) else {
            return out;
        };
        if l.phase == Phase::Hidden {
            return out;
        }
        if let Some(d) = self.district_of_lot(i) {
            out.push(format!("In the {} district.", self.dist.list[d].name));
        }
        let m = self.ord_mask(i);
        let class = class_of(l.kind);
        if m == 0 || class.is_none() || l.exempt {
            return out;
        }
        let homes = class == Some(Class::Homes);
        let dense = self.lot_density(i) >= 0.35;
        for o in Ordinance::ALL {
            if m & o.bit() == 0 {
                continue;
            }
            let line = match o {
                Ordinance::HeightLimit => "Height limit: buildings here stop at level 2.".to_string(),
                Ordinance::Upzoning if dense => {
                    "Downtown upzoning: this lot may rise one level above its usual limit and rebuilds sooner. More traffic.".into()
                }
                Ordinance::Upzoning => continue,
                Ordinance::Heritage => {
                    "Heritage protection: no rebuilding bigger or merging. Land value is higher.".into()
                }
                Ordinance::SmokeAlarms => "Smoke alarm programme: half the fire risk.".into(),
                Ordinance::Watch => "Neighbourhood watch: police cover is stronger here.".into(),
                Ordinance::BusinessBreak if !homes => {
                    "New business tax break: this lot pays 20 percent less tax.".into()
                }
                Ordinance::BusinessBreak => continue,
                Ordinance::Tourism if class == Some(Class::Shops) => {
                    "Tourism campaign: more shoppers, so shops are in more demand.".into()
                }
                Ordinance::Tourism => continue,
                Ordinance::CleanIndustry if class == Some(Class::Industry) => {
                    "Clean industry rules: this firm pays a fee and industry is less in demand."
                        .into()
                }
                Ordinance::CleanIndustry => {
                    "Clean industry rules: nearby industry hurts this lot's value half as much."
                        .into()
                }
                Ordinance::TrafficCalming if homes => {
                    "Traffic calming: traffic takes less off this home's value.".into()
                }
                Ordinance::TrafficCalming => {
                    "Traffic calming: slower streets take a little off this lot's value.".into()
                }
                Ordinance::Recycling if homes => {
                    "Recycling programme: residents are happier. Costs a fee per resident.".into()
                }
                Ordinance::Recycling => continue,
                Ordinance::StormDrains => {
                    "Storm drains: floods reach only half as far here. Costs a fee.".into()
                }
                Ordinance::BuildingCodes => {
                    "Building codes: storm damage costs half as much here, and new buildings take a quarter longer to finish."
                        .into()
                }
                Ordinance::SnowPlowing => {
                    "Snow plowing: roads here clear twice as fast. Costs a fee per metre of road."
                        .into()
                }
            };
            out.push(line);
        }
        out
    }

    /// Numbers for one district.
    pub fn district_stats(&self, d: usize) -> Option<DistrictStats> {
        self.dist.list.get(d)?;
        let (mut people, mut jobs, mut built) = (0i64, 0i64, 0u32);
        let (mut vsum, mut vn) = (0.0f64, 0u32);
        let (mut lsum, mut ln) = (0.0f64, 0u32);
        let (mut tax, mut fees) = (0i64, 0i64);
        let (mut empty, mut taxed) = (0u32, 0u32);
        let mut cong = 0.0f64;
        let mut sums = [0.0f64; 5];
        let mut homes = 0.0f64;
        for (i, l) in self.lots.iter().enumerate() {
            if self.district_of_addition(l.addition) != Some(d) || l.phase == Phase::Hidden {
                continue;
            }
            if l.exempt {
                continue;
            }
            vsum += self.lot_value.get(i).copied().unwrap_or(0.0) as f64;
            vn += 1;
            let m = self.ord_mask(i);
            for o in Ordinance::ALL {
                if m & o.bit() != 0 {
                    fees += self.lot_fee(i, o);
                }
            }
            if matches!(l.phase, Phase::Empty { .. }) {
                empty += 1;
            }
            if !matches!(l.phase, Phase::Built { .. }) {
                continue;
            }
            built += 1;
            people += Sim::lot_people(l);
            jobs += econ::kind_jobs_at(l.kind, l.level);
            tax += self.lot_tax_month(i);
            if class_of(l.kind).is_some() {
                lsum += l.level as f64;
                ln += 1;
                taxed += 1;
                cong += self.lot_traffic(i);
            }
            if class_of(l.kind) == Some(Class::Homes) {
                homes += 1.0;
                for (k, s) in sums.iter_mut().enumerate() {
                    *s += self.coverage[k].get(i).copied().unwrap_or(0.0) as f64;
                }
            }
        }
        let problem = if built == 0 {
            "Nothing built yet".to_string()
        } else {
            // the same measures as the town's approval, taken over this district
            let mut worst: (f64, String) = (-2.0, "No big problem".into());
            let mut consider = |score: f64, text: String| {
                if score < worst.0 {
                    worst = (score, text);
                }
            };
            if homes > 0.0 {
                let means: Vec<f64> = sums.iter().map(|s| s / homes).collect();
                let mean = means.iter().sum::<f64>() / 5.0;
                let mut weakest = 0;
                for k in 1..5 {
                    if means[k] < means[weakest] {
                        weakest = k;
                    }
                }
                consider(
                    (mean - 0.45) * 40.0,
                    Issue::Services(Service::ALL[weakest]).text(),
                );
            }
            if people > 0 {
                let ratio = jobs as f64 / (people as f64 * 0.5);
                consider((ratio.min(1.3) - 0.8) * 12.0, Issue::Jobs.text());
            }
            if taxed > 0 {
                consider(-(cong / taxed as f64) * 25.0, Issue::Traffic.text());
            }
            if vn > 0 {
                consider(-(empty as f64 / vn as f64) * 40.0, "empty buildings".into());
            }
            worst.1
        };
        Some(DistrictStats {
            people,
            jobs,
            buildings: built,
            avg_land_value: if vn > 0 {
                (vsum / vn as f64) as f32
            } else {
                0.0
            },
            avg_level: if ln > 0 {
                (lsum / ln as f64) as f32
            } else {
                0.0
            },
            tax_income: tax,
            ordinance_cost: fees,
            problem,
        })
    }

    /// Per lot: the district colour index (255 for none), for the map view.
    pub fn district_lot_colours(&self) -> Vec<u8> {
        self.lots
            .iter()
            .map(|l| match self.district_of_addition(l.addition) {
                Some(d) => self.dist.list[d].colour,
                None => u8::MAX,
            })
            .collect()
    }

    // ----- saving -----

    pub(crate) fn save_districts(&self, w: &mut W) {
        let d = &self.dist;
        w.u8(1);
        w.i64(d.last_cost);
        w.u32(d.town as u32);
        w.u32(d.list.len() as u32);
        for x in &d.list {
            w.str(&x.name);
            w.u8(x.colour);
            w.u32(x.ordinances as u32);
            w.usize(x.additions.len());
            for &a in &x.additions {
                w.u32(a as u32);
            }
        }
    }

    pub(crate) fn load_districts(&mut self, r: &mut R) -> Res<()> {
        if r.u8()? != 1 {
            return Ok(());
        }
        let n_add = self.revealed.len();
        let mut st = DistrictState {
            last_cost: r.i64()?,
            town: (r.u32()? & ((1u32 << Ordinance::ALL.len()) - 1)) as u16,
            ..Default::default()
        };
        let n = r.u32()? as usize;
        if n > MAX_DISTRICTS {
            return Err(LoadError::Invalid("districts"));
        }
        for _ in 0..n {
            let name = r.str()?;
            let colour = r.u8()?;
            let ordinances = (r.u32()? & ((1u32 << Ordinance::ALL.len()) - 1)) as u16;
            let k = r.len(4)?;
            let mut additions = Vec::with_capacity(k);
            for _ in 0..k {
                let a = r.u32()? as usize;
                if a >= n_add {
                    return Err(LoadError::Invalid("district neighbourhood"));
                }
                additions.push(a);
            }
            st.list.push(District {
                name,
                colour: colour % MAX_DISTRICTS as u8,
                additions,
                ordinances,
            });
        }
        st.rebuild(n_add);
        self.dist = st;
        Ok(())
    }
}
