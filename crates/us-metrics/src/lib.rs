//! Realism metrics for road networks, plus the shared graph builder and the real-town corpus loader.

pub mod corpus;
pub mod graph;
pub mod metrics;
pub mod score;

use us_gen::{Pt, RoadClass, Town};

/// Fine road graph of a generated town (crossings and touching ends are noded).
pub fn town_graph(t: &Town) -> graph::Graph {
    roads_graph(t.roads.iter().map(|r| (r.class, r.points.as_slice())))
}

/// Fine road graph from classed polylines of a generated town.
pub fn roads_graph<'a>(roads: impl Iterator<Item = (RoadClass, &'a [Pt])>) -> graph::Graph {
    let lines: Vec<(RoadClass, Vec<Pt>)> = roads.map(|(c, p)| (c, p.to_vec())).collect();
    graph::build(&lines, 0.5, true)
}

/// Metrics of a generated town.
pub fn town_metrics(t: &Town) -> metrics::Metrics {
    metrics::compute(&town_graph(t))
}
