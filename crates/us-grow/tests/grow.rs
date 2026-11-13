use us_gen::{Preset, Road, RoadClass, Town, generate};
use us_grow::geom::seg_intersect;
use us_grow::{Change, Context, GrowParams, Proposer, RuleProposer, TrafficScorer, grow};
use us_metrics::graph::build;

fn params(rounds: usize) -> GrowParams {
    GrowParams {
        rounds,
        ..GrowParams::default()
    }
}

fn grown(preset: Preset, seed: u64, rounds: usize) -> (Town, Town) {
    let t = generate(preset, seed);
    let (g, _) = grow(&t, &params(rounds), &RuleProposer, &TrafficScorer);
    (t, g)
}

/// Pairs of road segments that cross in the middle of both without sharing a vertex there.
fn unnoded_crossings(roads: &[Road], skip: &[RoadClass]) -> usize {
    let segs: Vec<(usize, us_gen::Pt, us_gen::Pt)> = roads
        .iter()
        .enumerate()
        .filter(|(_, r)| !skip.contains(&r.class))
        .flat_map(|(i, r)| r.points.windows(2).map(move |w| (i, w[0], w[1])))
        .collect();
    let mut n = 0;
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            if segs[i].0 == segs[j].0 {
                continue;
            }
            let (a, b, c, d) = (segs[i].1, segs[i].2, segs[j].1, segs[j].2);
            // cheap reject
            if a.x.max(b.x) < c.x.min(d.x) || c.x.max(d.x) < a.x.min(b.x) {
                continue;
            }
            if a.y.max(b.y) < c.y.min(d.y) || c.y.max(d.y) < a.y.min(b.y) {
                continue;
            }
            if let Some((t, u)) = seg_intersect(a, b, c, d) {
                let eps = 1e-6;
                if t > eps && t < 1.0 - eps && u > eps && u < 1.0 - eps {
                    n += 1;
                }
            }
        }
    }
    n
}

fn components(t: &Town) -> usize {
    let lines: Vec<_> = t
        .roads
        .iter()
        .map(|r| (r.class, r.points.clone()))
        .collect();
    let g = build(&lines, 0.5, true).contract();
    let (_, sizes) = g.components();
    sizes.len()
}

/// Length of a class in the noded graph: roads drawn twice over the same ground count once.
fn length(t: &Town, c: RoadClass) -> f64 {
    us_metrics::town_graph(t)
        .edges
        .iter()
        .filter(|e| e.class == c)
        .map(|e| e.len)
        .sum()
}

#[test]
fn growth_never_creates_a_crossing_without_a_junction() {
    for (preset, seed) in [(Preset::Grid, 1), (Preset::Winding, 2)] {
        let (before, after) = grown(preset, seed, 12);
        // streets are rewritten from the noded graph: no crossing is left without a vertex
        let skip = [RoadClass::Rail, RoadClass::Alley];
        assert_eq!(
            unnoded_crossings(&after.roads, &skip),
            0,
            "{} streets",
            preset.name()
        );
        // and connectors do not cross alleys either: count no worse than at the start
        let skip_rail = [RoadClass::Rail];
        assert!(
            unnoded_crossings(&after.roads, &skip_rail)
                <= unnoded_crossings(&before.roads, &skip_rail),
            "{}: new crossings with alleys",
            preset.name()
        );
    }
}

#[test]
fn growth_keeps_one_connected_network_and_never_drops_roads() {
    for (preset, seed) in [(Preset::Grid, 1), (Preset::Winding, 2), (Preset::Grid, 3)] {
        let (before, after) = grown(preset, seed, 12);
        assert_eq!(
            components(&before),
            components(&after),
            "{} {seed}",
            preset.name()
        );
        // total street length never shrinks, highway is untouched, nothing is upgraded past arterial
        let total = |t: &Town| -> f64 {
            [
                RoadClass::Highway,
                RoadClass::Arterial,
                RoadClass::Collector,
                RoadClass::Local,
            ]
            .iter()
            .map(|&c| length(t, c))
            .sum()
        };
        assert!(total(&after) >= total(&before) - 1.0);
        assert!(
            (length(&after, RoadClass::Highway) - length(&before, RoadClass::Highway)).abs() < 1.0
        );
        assert_eq!(
            after
                .roads
                .iter()
                .filter(|r| r.class == RoadClass::Rail)
                .count(),
            before
                .roads
                .iter()
                .filter(|r| r.class == RoadClass::Rail)
                .count()
        );
        let (la, lb) = (
            length(&after, RoadClass::Alley),
            length(&before, RoadClass::Alley),
        );
        assert!(
            (la - lb).abs() <= 0.01 * lb + 1e-9,
            "alley length {lb} -> {la}"
        );
        assert!(after.lots.len() <= before.lots.len());
    }
}

#[test]
fn growth_is_deterministic_by_seed() {
    let (_, a) = grown(Preset::Winding, 5, 6);
    let (_, b) = grown(Preset::Winding, 5, 6);
    assert_eq!(a.fingerprint(), b.fingerprint());
    let (_, c) = grown(Preset::Winding, 6, 6);
    assert_ne!(a.fingerprint(), c.fingerprint());
}

#[test]
fn growth_adds_connectors_and_upgrades_and_cuts_delay() {
    let t = generate(Preset::Winding, 2);
    let p = params(20);
    let (g, logs) = grow(&t, &p, &RuleProposer, &TrafficScorer);
    assert!(logs.iter().map(|l| l.upgrades).sum::<usize>() > 0);
    assert!(length(&g, RoadClass::Collector) > length(&t, RoadClass::Collector));
    // same year-20 demand on both towns: the grown one takes less time
    let scale = (1.0 + p.demand_growth).powi(20);
    let b = us_grow::evaluate(&t, &p, scale).assignment.vht;
    let a = us_grow::evaluate(&g, &p, scale).assignment.vht;
    assert!(a < b, "vehicle hours {a} after vs {b} before");
}

#[test]
fn connectors_obey_the_rules() {
    let t = generate(Preset::Grid, 1);
    let (g, logs) = grow(&t, &params(20), &RuleProposer, &TrafficScorer);
    let p = params(20);
    for l in &logs {
        for (a, b) in &l.links {
            let len = a.dist(*b);
            assert!(
                len >= p.link_min - 1e-6 && len <= p.link_max + 1e-6,
                "length {len}"
            );
            for ring in &g.water {
                assert!(!us_grow::geom::point_in_ring(*a, ring));
                assert!(!us_grow::geom::point_in_ring(a.lerp(*b, 0.5), ring));
            }
            // never through a building
            for lot in &g.lots {
                let d = us_grow::geom::dist_pt_seg(lot.centre, *a, *b);
                assert!(
                    d >= 5.0 + 0.55 * lot.radius - 1e-6,
                    "lot at {d} m from a connector"
                );
            }
        }
    }
}

struct Nothing;
impl Proposer for Nothing {
    fn propose(&self, _: &Context) -> Vec<Change> {
        Vec::new()
    }
}

#[test]
fn proposer_is_a_replaceable_part() {
    let t = generate(Preset::Grid, 1);
    let (g, logs) = grow(&t, &params(2), &Nothing, &TrafficScorer);
    assert_eq!(
        logs.iter()
            .map(|l| l.upgrades + l.connectors)
            .sum::<usize>(),
        0
    );
    for c in [
        RoadClass::Highway,
        RoadClass::Arterial,
        RoadClass::Collector,
        RoadClass::Local,
    ] {
        assert!((length(&g, c) - length(&t, c)).abs() < 1.0);
    }
}
