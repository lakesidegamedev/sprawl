//! Travel demand and user equilibrium traffic assignment on a generated town.
//!
//! Pipeline: `Network::from_town` (road graph with speeds and capacities), `Demand::from_town`
//! (zones from lots, gravity model, peak hour trips), `assign` (Frank-Wolfe with BPR delay).
//! Everything is deterministic: no hash order in any result, fixed chunking of origins so the
//! float sums do not depend on thread count.

mod assign;
mod demand;
mod network;

pub use assign::{AssignParams, Assignment, Skim, assign, skim};
pub use demand::{Demand, DemandParams, Zone, attraction_jobs, production_homes};
pub use network::{Arc, Network, NodeGrid, class_capacity, class_node_delay, class_speed};
