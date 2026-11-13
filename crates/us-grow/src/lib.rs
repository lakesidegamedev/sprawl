//! Traffic driven road growth.
//!
//! The work is split in two behind small traits so a learned model can replace either half later:
//! a `Proposer` makes candidate changes (upgrade a street one class, add a connector street) and a
//! `Scorer` judges each one (vehicle hours saved per hour of peak traffic, and what it costs).
//! `grow` runs rounds ("years"): assign traffic, propose, score, apply the best, repeat.

mod driver;
pub mod geom;
mod propose;
mod score;

pub use driver::{Evaluation, RoundLog, evaluate, grow};
pub use propose::RuleProposer;
pub use score::TrafficScorer;

use geom::{PointGrid, SegGrid};
use us_gen::{Pt, RoadClass, Town};
use us_traffic::{Assignment, Demand, Network, Skim};

/// Tunable growth settings.
#[derive(Clone, Debug)]
pub struct GrowParams {
    pub rounds: usize,
    /// Peak demand grows by this fraction each round (a year of new homes and jobs).
    pub demand_growth: f64,
    /// Longest stretch of street that may be upgraded in one round, metres.
    pub upgrade_budget_m: f64,
    /// An upgrade is considered once volume over capacity reaches this.
    pub upgrade_vc: f64,
    pub max_connectors: usize,
    /// Shortest and longest connector, metres.
    pub link_min: f64,
    pub link_max: f64,
    /// A connector is only considered when the existing route between its ends is at least
    /// this many times its straight length.
    pub min_circuity: f64,
    /// Connector must save at least this many vehicle hours per peak hour.
    pub min_gain: f64,
    /// Connector must save at least this many vehicle hours per peak hour per kilometre built.
    pub min_gain_per_km: f64,
    /// Lots a single connector may remove.
    pub max_lots_removed: usize,
    pub demand: us_traffic::DemandParams,
    pub assign: us_traffic::AssignParams,
}

impl Default for GrowParams {
    fn default() -> Self {
        GrowParams {
            rounds: 20,
            demand_growth: 0.04,
            upgrade_budget_m: 1200.0,
            upgrade_vc: 1.0,
            max_connectors: 4,
            link_min: 70.0,
            link_max: 320.0,
            min_circuity: 2.2,
            min_gain: 0.4,
            min_gain_per_km: 2.0,
            max_lots_removed: 2,
            demand: us_traffic::DemandParams::default(),
            assign: us_traffic::AssignParams::default(),
        }
    }
}

/// Where one end of a connector lands.
#[derive(Clone, Copy, Debug)]
pub enum End {
    /// An existing junction or dead end.
    Node(u32),
    /// A new junction on a street, `at` metres of the straight line from the street's segment start.
    Street {
        street: u32,
        /// Segment of the street polyline the new junction splits.
        seg: u32,
        /// Fraction of the whole street length at the junction.
        frac: f64,
    },
}

#[derive(Clone, Debug)]
pub enum Change {
    Upgrade {
        street: u32,
        to: RoadClass,
        lots_removed: Vec<u32>,
    },
    Connector {
        a: End,
        b: End,
        pa: Pt,
        pb: Pt,
        class: RoadClass,
        lots_removed: Vec<u32>,
    },
}

/// Everything a proposer or scorer may look at.
pub struct Context<'a> {
    pub town: &'a Town,
    pub net: &'a Network,
    pub demand: &'a Demand,
    pub assignment: &'a Assignment,
    /// Travel time from every zone node to every node at the equilibrium.
    pub skim: &'a Skim,
    pub index: &'a Index,
    pub params: &'a GrowParams,
}

pub struct Index {
    /// Every road of the town (all classes, rail included): (road, segment).
    pub segs: SegGrid,
    /// Streets of the drivable network: (street, segment).
    pub street_segs: SegGrid,
    pub lots: PointGrid,
    pub nodes: PointGrid,
}

impl Index {
    pub fn new(town: &Town, net: &Network) -> Index {
        Index {
            segs: SegGrid::new(town),
            street_segs: SegGrid::from_lines(net.streets.edges.iter().map(|e| e.pts.as_slice())),
            lots: PointGrid::new(town.lots.iter().map(|l| l.centre)),
            nodes: PointGrid::new(net.streets.nodes.iter().copied()),
        }
    }
}

pub struct Score {
    /// Vehicle hours saved per peak hour.
    pub gain: f64,
    /// Metres of street built or widened, counting a lot lost as 80 m.
    pub cost: f64,
}

pub trait Proposer {
    fn propose(&self, ctx: &Context) -> Vec<Change>;
}

pub trait Scorer {
    fn score(&self, ctx: &Context, change: &Change) -> Option<Score>;
}
