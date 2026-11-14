//! Plain data types of the simulation: classes, services, departments, tax rates, civic
//! buildings, map layers and the dated event log.

use us_gen::{LotKind, Pt};

/// Who pays which tax. Shops include offices and roadside business; industry is workshops and
/// warehouses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    Homes,
    Shops,
    Industry,
}

impl Class {
    pub const ALL: [Class; 3] = [Class::Homes, Class::Shops, Class::Industry];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Class::Homes => "Homes",
            Class::Shops => "Shops",
            Class::Industry => "Industry",
        }
    }
}

/// The tax class of a building kind, or `None` for lots that pay no tax.
pub fn class_of(k: LotKind) -> Option<Class> {
    match k {
        LotKind::House | LotKind::Row | LotKind::Apartments => Some(Class::Homes),
        LotKind::Shop
        | LotKind::Gas
        | LotKind::Strip
        | LotKind::BigBox
        | LotKind::FastFood
        | LotKind::Office => Some(Class::Shops),
        LotKind::Workshop | LotKind::Warehouse => Some(Class::Industry),
        _ => None,
    }
}

/// One value per tax class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ByClass<T> {
    pub homes: T,
    pub shops: T,
    pub industry: T,
}

impl<T: Copy> ByClass<T> {
    pub fn all(v: T) -> Self {
        ByClass {
            homes: v,
            shops: v,
            industry: v,
        }
    }

    pub fn get(&self, c: Class) -> T {
        match c {
            Class::Homes => self.homes,
            Class::Shops => self.shops,
            Class::Industry => self.industry,
        }
    }

    pub fn set(&mut self, c: Class, v: T) {
        match c {
            Class::Homes => self.homes = v,
            Class::Shops => self.shops = v,
            Class::Industry => self.industry = v,
        }
    }
}

/// Budget lines the mayor can fund between 0 and 150 percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dept {
    Police,
    Fire,
    Schools,
    Health,
    Parks,
    Roads,
}

impl Dept {
    pub const ALL: [Dept; 6] = [
        Dept::Police,
        Dept::Fire,
        Dept::Schools,
        Dept::Health,
        Dept::Parks,
        Dept::Roads,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Dept::Police => "Police",
            Dept::Fire => "Fire",
            Dept::Schools => "Schools",
            Dept::Health => "Health",
            Dept::Parks => "Parks",
            Dept::Roads => "Road upkeep",
        }
    }

    /// The coverage this line pays for, if it has one.
    pub fn service(self) -> Option<Service> {
        match self {
            Dept::Police => Some(Service::Police),
            Dept::Fire => Some(Service::Fire),
            Dept::Schools => Some(Service::Schools),
            Dept::Health => Some(Service::Health),
            Dept::Parks => Some(Service::Parks),
            Dept::Roads => None,
        }
    }

    /// Baseline cost per taxed building per month at 100 percent (general staffing; the part of
    /// the budget that is not tied to a building). Roads have none: their cost is the road
    /// network.
    pub(crate) fn staff_cost(self) -> i64 {
        match self {
            Dept::Police => 90,
            Dept::Fire => 70,
            Dept::Schools => 90,
            Dept::Health => 50,
            Dept::Parks => 0,
            Dept::Roads => 0,
        }
    }
}

pub const BUDGET_MIN_PCT: u32 = 0;
pub const BUDGET_MAX_PCT: u32 = 150;
pub const BUDGET_DEFAULT_PCT: u32 = 100;

/// What a building covers. Water towers are not a budget line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Service {
    Police,
    Fire,
    Schools,
    Health,
    Parks,
    Water,
}

impl Service {
    pub const ALL: [Service; 6] = [
        Service::Police,
        Service::Fire,
        Service::Schools,
        Service::Health,
        Service::Parks,
        Service::Water,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Service::Police => "Police",
            Service::Fire => "Fire",
            Service::Schools => "Schools",
            Service::Health => "Health",
            Service::Parks => "Parks",
            Service::Water => "Water",
        }
    }

    pub fn dept(self) -> Option<Dept> {
        match self {
            Service::Police => Some(Dept::Police),
            Service::Fire => Some(Dept::Fire),
            Service::Schools => Some(Dept::Schools),
            Service::Health => Some(Dept::Health),
            Service::Parks => Some(Dept::Parks),
            Service::Water => None,
        }
    }

    /// Road travel distance in metres the service reaches at 100 percent funding.
    pub fn reach_m(self) -> f64 {
        match self {
            Service::Police => 1200.0,
            Service::Fire => 1100.0,
            Service::Schools => 900.0,
            Service::Health => 1300.0,
            Service::Parks => 450.0,
            Service::Water => 1500.0,
        }
    }
}

/// Civic buildings the mayor can place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CivicKind {
    School,
    Police,
    Fire,
    Clinic,
    Park,
    Water,
    /// Supplies power; a town cannot grow past the plants it has built. Until the renderer has a
    /// model it is drawn with the sports field look.
    Power,
}

impl CivicKind {
    pub const ALL: [CivicKind; 7] = [
        CivicKind::School,
        CivicKind::Police,
        CivicKind::Fire,
        CivicKind::Clinic,
        CivicKind::Park,
        CivicKind::Water,
        CivicKind::Power,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CivicKind::School => "School",
            CivicKind::Police => "Police station",
            CivicKind::Fire => "Fire station",
            CivicKind::Clinic => "Clinic",
            CivicKind::Park => "Park",
            CivicKind::Water => "Water tower",
            CivicKind::Power => "Power plant",
        }
    }

    /// Price in dollars, paid from the treasury when placed.
    pub fn cost(self) -> i64 {
        match self {
            CivicKind::School => 120_000,
            CivicKind::Police => 90_000,
            CivicKind::Fire => 100_000,
            CivicKind::Clinic => 80_000,
            CivicKind::Park => 30_000,
            CivicKind::Water => 60_000,
            CivicKind::Power => 250_000,
        }
    }

    /// Dollars per month at 100 percent funding.
    pub fn upkeep(self) -> i64 {
        match self {
            CivicKind::School => 3000,
            CivicKind::Police => 2500,
            CivicKind::Fire => 2000,
            CivicKind::Clinic => 2200,
            CivicKind::Park => 400,
            CivicKind::Water => 800,
            CivicKind::Power => 4000,
        }
    }

    pub fn service(self) -> Service {
        match self {
            CivicKind::School => Service::Schools,
            CivicKind::Police => Service::Police,
            CivicKind::Fire => Service::Fire,
            CivicKind::Clinic => Service::Health,
            CivicKind::Park => Service::Parks,
            // a utility: it covers nothing, only adds capacity (see `Capacity`)
            CivicKind::Water | CivicKind::Power => Service::Water,
        }
    }

    /// Footprint radius in metres.
    pub fn radius(self) -> f64 {
        match self {
            CivicKind::School => 13.0,
            CivicKind::Police => 10.0,
            CivicKind::Fire => 11.0,
            CivicKind::Clinic => 10.0,
            CivicKind::Park => 14.0,
            CivicKind::Water => 7.0,
            CivicKind::Power => 16.0,
        }
    }

    /// The generator building kind the renderer draws it with until it has its own model.
    pub fn lot_kind(self) -> LotKind {
        match self {
            CivicKind::School => LotKind::School,
            CivicKind::Police => LotKind::Fire,
            CivicKind::Fire => LotKind::Fire,
            CivicKind::Clinic => LotKind::Church,
            CivicKind::Park => LotKind::Park,
            CivicKind::Water => LotKind::Water,
            CivicKind::Power => LotKind::Sports,
        }
    }
}

/// The service a plain generated lot kind provides, if any.
pub(crate) fn service_of_kind(k: LotKind) -> Option<Service> {
    match k {
        LotKind::School => Some(Service::Schools),
        LotKind::Fire => Some(Service::Fire),
        LotKind::Water => Some(Service::Water),
        LotKind::Park | LotKind::Sports => Some(Service::Parks),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceError {
    /// Not enough money in the treasury.
    TooPoor {
        cost: i64,
    },
    /// Outside the map.
    OutOfBounds,
    /// More than 45 m from any revealed road.
    NoRoad,
    /// On the pavement of a road.
    OnRoad,
    InWater,
    /// Overlaps a building, a construction site or the town hall.
    Occupied,
}

impl PlaceError {
    pub fn text(self) -> String {
        match self {
            PlaceError::TooPoor { cost } => format!("Not enough money: it costs ${cost}."),
            PlaceError::OutOfBounds => "That is off the map.".into(),
            PlaceError::NoRoad => "Too far from a road the town has built.".into(),
            PlaceError::OnRoad => "That spot is on a road.".into(),
            PlaceError::InWater => "That spot is in the water.".into(),
            PlaceError::Occupied => "Something already stands there.".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoanError {
    /// Debt would pass the credit limit, which follows the tax base.
    LimitReached,
    NothingToRepay,
    NotEnoughCash,
}

/// A bond: fixed size, interest on the balance, equal monthly principal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loan {
    pub principal_left: i64,
    pub monthly_principal: i64,
}

pub const BOND_SIZE: i64 = 100_000;
pub const BOND_MONTHS: i64 = 48;
/// Yearly interest in tenths of a percent when the town owes nothing (8.0 percent).
pub const BOND_RATE_X10: i64 = 80;
/// Extra interest at a fully used credit limit (4.0 percent more).
pub const BOND_RATE_RISK_X10: i64 = 40;
pub const MIN_CREDIT_LIMIT: i64 = 150_000;

/// A layer for the map views. `Sim::lot_values` and `Sim::road_values` document each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapKind {
    /// Land value, 0 (worthless) to 1.
    LandValue,
    /// Building level 1 to 5 divided by 5; 0 for lots with no building.
    BuildingLevel,
    /// Category id as a float: 0 nothing, 1 homes, 2 shops, 3 industry, 4 civic, 5 farm.
    Use,
    /// How well a service covers a lot, 0 to 1.
    Coverage(Service),
    /// Demand for the lot's own class, 0 to 1 (civic lots get 0).
    Demand(Class),
    /// Roads only: congestion 0 (empty) to 1 (jammed). Needs `road_values`.
    Traffic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Neighborhood,
    Milestone,
    Budget,
    Civic,
    Loan,
    Upgrade,
    Road,
    Fire,
    Rival,
    Decline,
    Election,
    GameOver,
    /// A capacity ran short or came back (see `Capacity`).
    Limit,
}

/// A dated line for the news feed.
#[derive(Clone, Debug)]
pub struct Event {
    pub day: i32,
    pub kind: EventKind,
    pub text: String,
    /// Where on the map it happened, for a click-to-jump.
    pub place: Option<Pt>,
}

/// One sample of the whole game, taken on the first of every month.
#[derive(Clone, Debug)]
pub struct Sample {
    pub day: i32,
    pub year: i32,
    pub month: u32,
    pub population: i64,
    pub jobs: i64,
    pub treasury: i64,
    /// Tax collected this month per class.
    pub income: ByClass<i64>,
    /// Spending this month, indexed by `Dept::index()`.
    pub spending: [i64; 6],
    /// General upkeep per building, not tied to a budget line.
    pub admin: i64,
    /// Interest plus principal paid on bonds this month.
    pub debt_service: i64,
    pub debt: i64,
    pub tax_x10: ByClass<u32>,
    /// Demand 0 to 1 per class.
    pub demand: ByClass<f32>,
    pub built: u32,
    pub building: u32,
    pub empty: u32,
    pub avg_land_value: f32,
    /// Average road congestion 0 to 1; 0 until traffic runs.
    pub congestion: f32,
    /// Voter approval 0 to 100.
    pub approval: i32,
    /// Days until the next election (0 when elections are off or it is today).
    pub days_to_election: i32,
}

impl Sample {
    pub fn income_total(&self) -> i64 {
        self.income.homes + self.income.shops + self.income.industry
    }

    pub fn spending_total(&self) -> i64 {
        self.spending.iter().sum::<i64>() + self.admin + self.debt_service
    }
}

/// How hard the town is to grow, chosen at the start of a game. It scales how fast people and
/// firms arrive, how much each building adds to the town's capacities, and what new streets and
/// connections cost the treasury.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Easy => {
                "People and firms arrive faster, utilities stretch further and new streets cost less."
            }
            Difficulty::Normal => "A well run town reaches about 10,000 people in 10 to 15 years.",
            Difficulty::Hard => {
                "People arrive slowly, utilities run out sooner and growth costs the treasury more."
            }
        }
    }

    /// Speed of arrivals, percent of normal.
    pub fn arrival_pct(self) -> i64 {
        match self {
            Difficulty::Easy => 135,
            Difficulty::Normal => 100,
            Difficulty::Hard => 75,
        }
    }

    /// What one water tower, school, clinic or power plant carries, percent of normal.
    pub fn capacity_pct(self) -> i64 {
        match self {
            Difficulty::Easy => 125,
            Difficulty::Normal => 100,
            Difficulty::Hard => 85,
        }
    }

    /// What new streets and connections cost the town, percent of normal.
    pub fn cost_pct(self) -> i64 {
        match self {
            Difficulty::Easy => 60,
            Difficulty::Normal => 100,
            Difficulty::Hard => 140,
        }
    }
}

/// Something a town cannot grow past without building more of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capacity {
    /// Water towers. Everything that grows needs it.
    Water,
    /// Power plants. Everything that grows needs it.
    Power,
    /// School seats for the town's children. Homes stop growing when they are full.
    Schools,
    /// Clinic capacity. Homes stop growing when it is full.
    Health,
}

impl Capacity {
    pub const ALL: [Capacity; 4] = [
        Capacity::Water,
        Capacity::Power,
        Capacity::Schools,
        Capacity::Health,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Capacity::Water => "Water supply",
            Capacity::Power => "Power",
            Capacity::Schools => "School seats",
            Capacity::Health => "Clinics",
        }
    }

    /// What to build when it is full.
    pub fn build_hint(self) -> &'static str {
        match self {
            Capacity::Water => "Build a water tower.",
            Capacity::Power => "Build a power plant.",
            Capacity::Schools => "Build a school.",
            Capacity::Health => "Build a clinic.",
        }
    }

    /// The civic building that adds to it.
    pub fn civic(self) -> CivicKind {
        match self {
            Capacity::Water => CivicKind::Water,
            Capacity::Power => CivicKind::Power,
            Capacity::Schools => CivicKind::School,
            Capacity::Health => CivicKind::Clinic,
        }
    }

    /// The news line when growth stalls.
    pub fn stall_text(self) -> String {
        let what = match self {
            Capacity::Water => "the water supply is full",
            Capacity::Power => "the power supply is full",
            Capacity::Schools => "the schools are full",
            Capacity::Health => "the clinics are full",
        };
        format!("Growth has stalled: {what}. {}", self.build_hint())
    }
}

/// How much of a capacity the town uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapacityStatus {
    pub capacity: Capacity,
    /// What the town needs now.
    pub demand: i64,
    /// What it has.
    pub supply: i64,
}

impl CapacityStatus {
    /// Demand over supply; above 1.0 the town is short.
    pub fn ratio(&self) -> f64 {
        self.demand as f64 / self.supply.max(1) as f64
    }

    pub fn is_full(&self) -> bool {
        self.demand >= self.supply
    }
}
