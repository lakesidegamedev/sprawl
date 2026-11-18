//! Realism metrics for road networks, plus the shared graph builder and the real-town corpus loader.

pub mod corpus;
pub mod graph;
pub mod metrics;
pub mod score;

use us_gen::{Preset, Pt, RoadClass, Town};

/// Fine road graph of a generated town (crossings and touching ends are noded).
pub fn town_graph(t: &Town) -> graph::Graph {
    terrain_roads_graph(t, t.roads.iter().map(|r| (r.class, r.points.as_slice())))
}

/// Fine road graph from classed polylines of a generated town.
pub fn roads_graph<'a>(roads: impl Iterator<Item = (RoadClass, &'a [Pt])>) -> graph::Graph {
    let lines: Vec<(RoadClass, Vec<Pt>)> = roads.map(|(c, p)| (c, p.to_vec())).collect();
    graph::build(&lines, 0.5, true)
}

/// A generated or live network, using the Gorge bridge profile for routing.
pub fn terrain_roads_graph<'a>(
    town: &Town,
    roads: impl Iterator<Item = (RoadClass, &'a [Pt])>,
) -> graph::Graph {
    let roads: Vec<_> = roads.collect();
    let highways: Vec<_> = roads
        .iter()
        .filter_map(|(c, points)| (*c == RoadClass::Highway).then_some(*points))
        .collect();
    let profiles: Vec<_> = roads
        .into_iter()
        .filter(|(class, _)| *class != RoadClass::Rail)
        .map(|(class, line)| {
            let travel = if class == RoadClass::Collector {
                match us_gen::slip_lane_forward(line, &highways) {
                    Some(true) => graph::Travel::Forward,
                    Some(false) => graph::Travel::Reverse,
                    None => graph::Travel::Both,
                }
            } else {
                graph::Travel::Both
            };
            if town.preset != Preset::Gorge {
                return graph::GradedLine {
                    class,
                    travel,
                    points: line.iter().map(|&p| (p, 0.0)).collect(),
                };
            }
            let mut points = Vec::new();
            for w in line.windows(2) {
                let count = (w[0].dist(w[1]) / 5.0).ceil().max(1.0) as usize;
                for i in 0..count {
                    let p = w[0].lerp(w[1], i as f64 / count as f64);
                    if points.last().is_none_or(|&q: &Pt| q.dist(p) > 1e-6) {
                        points.push(p);
                    }
                }
            }
            if let Some(&p) = line.last()
                && points.last().is_none_or(|&q| q.dist(p) > 1e-6)
            {
                points.push(p);
            }
            let mut path_length = 0.0_f32;
            let mut distances = vec![0.0; points.len()];
            for (i, w) in points.windows(2).enumerate() {
                path_length += w[0].dist(w[1]) as f32;
                distances[i + 1] = path_length;
            }
            let ground: Vec<f32> = points
                .iter()
                .map(|&p| town.terrain.height_at(p) as f32)
                .collect();
            let deck = us_gen::bridges::high_decks(&ground, &distances);
            let points = if deck.weight.iter().any(|&weight| weight > 0.0) {
                points
                    .into_iter()
                    .zip(ground)
                    .zip(deck.z)
                    .zip(deck.weight)
                    .map(|(((p, h), z), w)| (p, f64::from((z - h).max(0.0) * w)))
                    .collect()
            } else {
                line.iter().map(|&p| (p, 0.0)).collect()
            };
            graph::GradedLine {
                class,
                points,
                travel,
            }
        })
        .collect();
    graph::build_graded(&profiles, 0.5)
}

/// Metrics of a generated town.
pub fn town_metrics(t: &Town) -> metrics::Metrics {
    metrics::compute(&town_graph(t))
}
