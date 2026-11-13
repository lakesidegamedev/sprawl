//! Independent checks on generated towns. Nothing here reuses generator internals.

use std::collections::HashMap;
use us_gen::{Preset, Pt, Road, RoadClass, Town, Zone, generate};

const SEEDS: [u64; 4] = [1, 2, 3, 7];
const PRESETS: [Preset; 2] = [Preset::Grid, Preset::Winding];

fn towns() -> Vec<Town> {
    let mut v = Vec::new();
    for p in PRESETS {
        for s in SEEDS {
            v.push(generate(p, s));
        }
    }
    v
}

fn seg_closest(p: Pt, a: Pt, b: Pt) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 < 1e-18 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (a.x + abx * t, a.y + aby * t);
    ((p.x - qx).powi(2) + (p.y - qy).powi(2)).sqrt()
}

/// Grid of road segments for fast "distance to any road except..." queries.
struct RoadGrid<'a> {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<(usize, usize)>>,
    roads: &'a [Road],
}

impl<'a> RoadGrid<'a> {
    fn new(roads: &'a [Road]) -> Self {
        let cell = 50.0;
        let mut cells: HashMap<(i64, i64), Vec<(usize, usize)>> = HashMap::new();
        for (ri, r) in roads.iter().enumerate() {
            for (si, w) in r.points.windows(2).enumerate() {
                let n = ((w[0].x - w[1].x).hypot(w[0].y - w[1].y) / 10.0)
                    .ceil()
                    .max(1.0) as usize;
                let mut last = (i64::MIN, i64::MIN);
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let (x, y) = (
                        w[0].x + (w[1].x - w[0].x) * t,
                        w[0].y + (w[1].y - w[0].y) * t,
                    );
                    let c = ((x / cell).floor() as i64, (y / cell).floor() as i64);
                    if c != last {
                        cells.entry(c).or_default().push((ri, si));
                        last = c;
                    }
                }
            }
        }
        RoadGrid { cell, cells, roads }
    }

    /// All (road index, distance) pairs within `r` of `p`, smallest distance per road.
    fn near(&self, p: Pt, r: f64) -> Vec<(usize, f64)> {
        let mut best: HashMap<usize, f64> = HashMap::new();
        let (x0, x1) = (
            ((p.x - r) / self.cell).floor() as i64 - 1,
            ((p.x + r) / self.cell).floor() as i64 + 1,
        );
        let (y0, y1) = (
            ((p.y - r) / self.cell).floor() as i64 - 1,
            ((p.y + r) / self.cell).floor() as i64 + 1,
        );
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                if let Some(v) = self.cells.get(&(cx, cy)) {
                    for &(ri, si) in v {
                        let w = &self.roads[ri].points;
                        let d = seg_closest(p, w[si], w[si + 1]);
                        if d <= r {
                            let e = best.entry(ri).or_insert(f64::INFINITY);
                            if d < *e {
                                *e = d;
                            }
                        }
                    }
                }
            }
        }
        let mut out: Vec<(usize, f64)> = best.into_iter().collect();
        out.sort_by_key(|a| a.0);
        out
    }
}

fn is_street(c: RoadClass) -> bool {
    matches!(
        c,
        RoadClass::Local | RoadClass::Alley | RoadClass::Collector
    )
}

fn d(a: Pt, b: Pt) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

#[test]
fn street_ends_land_on_another_road_or_are_cul_de_sacs() {
    for t in towns() {
        let g = RoadGrid::new(&t.roads);
        let mut bad = Vec::new();
        for (i, r) in t
            .roads
            .iter()
            .enumerate()
            .filter(|(_, r)| is_street(r.class))
        {
            let (first, last) = (r.points[0], *r.points.last().unwrap());
            if d(first, last) < 1e-6 {
                continue;
            }
            for e in [first, last] {
                let on_other = g.near(e, 1.5).iter().any(|(ri, _)| *ri != i);
                let is_cul = t.cul_de_sacs.iter().any(|c| d(c.pos, e) < 1e-6);
                if !on_other && !is_cul {
                    bad.push((i, r.class, e));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} seed {}: {} dangling ends, first few {:?}",
            t.preset.name(),
            t.seed,
            bad.len(),
            &bad[..bad.len().min(5)]
        );
    }
}

#[test]
fn every_cul_de_sac_is_a_street_end_with_a_bulb() {
    for t in towns() {
        assert!(
            !t.cul_de_sacs.is_empty(),
            "{} seed {} has no cul-de-sacs",
            t.preset.name(),
            t.seed
        );
        for c in &t.cul_de_sacs {
            assert!(
                t.roads
                    .iter()
                    .any(|r| r.class == RoadClass::Local
                        && d(*r.points.last().unwrap(), c.pos) < 1e-6)
            );
            assert!(c.addition < t.additions.len());
        }
    }
}

#[test]
fn only_highway_arterial_and_rail_touch_the_map_edge() {
    for t in towns() {
        for r in &t.roads {
            let touches = r
                .points
                .iter()
                .any(|p| p.x < 1.0 || p.x > t.size - 1.0 || p.y > t.size - 1.0 || p.y < 1.0);
            if touches {
                assert!(
                    matches!(
                        r.class,
                        RoadClass::Highway | RoadClass::Arterial | RoadClass::Rail
                    ),
                    "{} seed {}: {:?} reaches the map edge",
                    t.preset.name(),
                    t.seed,
                    r.class
                );
            }
            for p in &r.points {
                assert!(
                    p.x > -1e-6
                        && p.x < t.size + 1e-6
                        && p.y > t.highway_y - 1e-6
                        && p.y < t.size + 1e-6,
                    "point outside map: {p:?}"
                );
            }
        }
    }
}

#[test]
fn lots_do_not_overlap_each_other_roads_or_water() {
    for t in towns() {
        let g = RoadGrid::new(&t.roads);
        // lot vs lot, using a grid
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (i, l) in t.lots.iter().enumerate() {
            cells
                .entry((
                    (l.centre.x / 50.0).floor() as i64,
                    (l.centre.y / 50.0).floor() as i64,
                ))
                .or_default()
                .push(i);
        }
        for (i, l) in t.lots.iter().enumerate() {
            let (cx, cy) = (
                (l.centre.x / 50.0).floor() as i64,
                (l.centre.y / 50.0).floor() as i64,
            );
            for x in cx - 2..=cx + 2 {
                for y in cy - 2..=cy + 2 {
                    for &j in cells.get(&(x, y)).map_or(&[][..], |v| v.as_slice()) {
                        if j > i {
                            let o = &t.lots[j];
                            assert!(
                                d(l.centre, o.centre) >= l.radius + o.radius - 1e-9,
                                "{} seed {}: lots {i} and {j} overlap",
                                t.preset.name(),
                                t.seed
                            );
                        }
                    }
                }
            }
            for (ri, dist) in g.near(l.centre, l.radius + 14.0) {
                let hw = t.roads[ri].class.half_width();
                assert!(
                    dist >= l.radius + hw - 1e-6,
                    "{} seed {}: lot {i} ({:?}) is {dist:.1} m from a {:?}, needs {:.1}",
                    t.preset.name(),
                    t.seed,
                    l.kind,
                    t.roads[ri].class,
                    l.radius + hw
                );
            }
            for c in &t.cul_de_sacs {
                assert!(
                    d(l.centre, c.pos) >= l.radius + 9.0 - 1e-6,
                    "lot {i} sits on a turning bulb"
                );
            }
            for w in &t.water {
                let inside = point_in(l.centre, w);
                let edge = ring_dist(l.centre, w);
                assert!(
                    !inside && edge >= l.radius,
                    "{} seed {}: lot {i} touches water",
                    t.preset.name(),
                    t.seed
                );
            }
            assert!(
                d(l.centre, t.hall) >= l.radius + t.hall_radius - 1e-6,
                "lot {i} on the town hall"
            );
            assert!(
                l.centre.x >= l.radius
                    && l.centre.x <= t.size - l.radius
                    && l.centre.y >= t.highway_y + l.radius
                    && l.centre.y <= t.size - l.radius
            );
        }
    }
}

fn point_in(p: Pt, ring: &[Pt]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn ring_dist(p: Pt, ring: &[Pt]) -> f64 {
    (0..ring.len())
        .map(|i| seg_closest(p, ring[i], ring[(i + 1) % ring.len()]))
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn town_hall_is_clear_of_roads_and_water() {
    for t in towns() {
        let g = RoadGrid::new(&t.roads);
        for (ri, dist) in g.near(t.hall, t.hall_radius + 14.0) {
            assert!(
                dist >= t.hall_radius + t.roads[ri].class.half_width(),
                "{} seed {}: hall on a {:?}",
                t.preset.name(),
                t.seed,
                t.roads[ri].class
            );
        }
        assert!(!point_in(t.hall, &t.water[0]));
    }
}

#[test]
fn same_seed_same_bytes_different_seed_different_town() {
    for p in PRESETS {
        let a = generate(p, 42);
        let b = generate(p, 42);
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        assert_eq!(a.fingerprint(), b.fingerprint());
        let c = generate(p, 43);
        assert_ne!(a.fingerprint(), c.fingerprint());
    }
    assert_ne!(
        generate(Preset::Grid, 5).fingerprint(),
        generate(Preset::Winding, 5).fingerprint()
    );
}

#[test]
fn pods_have_an_entrance_and_two_cul_de_sacs() {
    let mut pods_seen = 0;
    for p in PRESETS {
        for s in 1..=12u64 {
            let t = generate(p, s);
            for (ai, a) in t
                .additions
                .iter()
                .enumerate()
                .filter(|(_, a)| a.pattern == us_gen::Pattern::Pods)
            {
                pods_seen += 1;
                assert!(
                    !a.entrances.is_empty(),
                    "{} seed {s}: pod {ai} has no entrance",
                    p.name()
                );
                for e in &a.entrances {
                    // entrance sits on a highway or arterial
                    assert!(
                        t.roads
                            .iter()
                            .filter(|r| matches!(r.class, RoadClass::Highway | RoadClass::Arterial))
                            .any(|r| r
                                .points
                                .windows(2)
                                .any(|w| seg_closest(*e, w[0], w[1]) < 1.5)),
                        "entrance not on a big road"
                    );
                }
                let culs = t.cul_de_sacs.iter().filter(|c| c.addition == ai).count();
                assert!(
                    culs >= 2,
                    "{} seed {s}: pod {ai} has {culs} cul-de-sacs",
                    p.name()
                );
            }
        }
    }
    assert!(pods_seen >= 6, "only {pods_seen} pods across 24 towns");
}

#[test]
fn long_addition_edges_become_collectors() {
    for t in towns() {
        let n = t
            .roads
            .iter()
            .filter(|r| r.class == RoadClass::Collector)
            .count();
        assert!(
            n >= 3,
            "{} seed {} has {n} collectors",
            t.preset.name(),
            t.seed
        );
    }
}

#[test]
fn downtown_has_a_handful_of_towers_near_the_hall() {
    for t in towns() {
        let offices: Vec<_> = t
            .lots
            .iter()
            .filter(|l| l.kind == us_gen::LotKind::Office)
            .collect();
        assert!(
            offices.len() <= 6,
            "{} seed {}: {} towers",
            t.preset.name(),
            t.seed,
            offices.len()
        );
        for o in offices {
            assert!(
                d(o.centre, t.hall) < 252.0,
                "office {:.0} m from the hall",
                d(o.centre, t.hall)
            );
        }
    }
}

#[test]
fn streets_do_not_run_along_each_other() {
    for t in towns() {
        let g = RoadGrid::new(&t.roads);
        for (i, r) in t
            .roads
            .iter()
            .enumerate()
            .filter(|(_, r)| is_street(r.class))
        {
            let mut run = 0usize;
            let mut worst = 0usize;
            let mut acc = 0.0;
            for w in r.points.windows(2) {
                let n = (d(w[0], w[1]) / 4.0).ceil().max(1.0) as usize;
                for k in 1..n {
                    let q = Pt {
                        x: w[0].x + (w[1].x - w[0].x) * k as f64 / n as f64,
                        y: w[0].y + (w[1].y - w[0].y) * k as f64 / n as f64,
                    };
                    acc += 1.0;
                    if g.near(q, 0.8).iter().any(|(ri, _)| *ri != i) {
                        run += 1;
                        worst = worst.max(run);
                    } else {
                        run = 0;
                    }
                }
            }
            let _ = acc;
            assert!(
                worst <= 5,
                "{} seed {}: road {i} ({:?}) runs {} samples along another road",
                t.preset.name(),
                t.seed,
                r.class,
                worst
            );
        }
    }
}

#[test]
fn zones_and_lots_are_consistent() {
    for t in towns() {
        assert!(
            t.lots.len() > 2000,
            "{} seed {}: only {} lots",
            t.preset.name(),
            t.seed,
            t.lots.len()
        );
        assert!(t.additions.iter().any(|a| a.zone == Zone::Core));
        assert!(t.additions.iter().any(|a| a.zone == Zone::Farm));
    }
}

#[test]
#[ignore = "timing test, run with --release -- --ignored"]
fn four_km_town_generates_in_under_three_seconds() {
    for p in PRESETS {
        let start = std::time::Instant::now();
        let t = generate(p, 11);
        let el = start.elapsed().as_secs_f64();
        assert!(el < 3.0, "{} took {el:.2}s", p.name());
        assert!(!t.lots.is_empty());
    }
}

fn seg_seg_dist(a: Pt, b: Pt, c: Pt, e: Pt) -> f64 {
    let o = |p: Pt, q: Pt, r: Pt| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    let (d1, d2, d3, d4) = (o(c, e, a), o(c, e, b), o(a, b, c), o(a, b, e));
    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        return 0.0;
    }
    seg_closest(a, c, e)
        .min(seg_closest(b, c, e))
        .min(seg_closest(c, a, b))
        .min(seg_closest(e, a, b))
}

/// Components of the road network (everything but rail), joined where roads cross or touch.
fn components(t: &Town) -> Vec<Vec<usize>> {
    let ids: Vec<usize> = (0..t.roads.len())
        .filter(|&i| t.roads[i].class != RoadClass::Rail)
        .collect();
    let cell = 64.0;
    let mut cells: HashMap<(i64, i64), Vec<(usize, usize)>> = HashMap::new();
    for &ri in &ids {
        for (si, w) in t.roads[ri].points.windows(2).enumerate() {
            let (x0, x1) = (w[0].x.min(w[1].x), w[0].x.max(w[1].x));
            let (y0, y1) = (w[0].y.min(w[1].y), w[0].y.max(w[1].y));
            for cx in ((x0 - 2.0) / cell).floor() as i64..=((x1 + 2.0) / cell).floor() as i64 {
                for cy in ((y0 - 2.0) / cell).floor() as i64..=((y1 + 2.0) / cell).floor() as i64 {
                    cells.entry((cx, cy)).or_default().push((ri, si));
                }
            }
        }
    }
    let mut parent: Vec<usize> = (0..t.roads.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut keys: Vec<_> = cells.keys().copied().collect();
    keys.sort();
    for k in keys {
        let v = &cells[&k];
        for i in 0..v.len() {
            for j in i + 1..v.len() {
                let ((ra, sa), (rb, sb)) = (v[i], v[j]);
                if ra == rb {
                    continue;
                }
                let (pa, pb) = (&t.roads[ra].points, &t.roads[rb].points);
                if seg_seg_dist(pa[sa], pa[sa + 1], pb[sb], pb[sb + 1]) <= 1.5 {
                    let (x, y) = (find(&mut parent, ra), find(&mut parent, rb));
                    if x != y {
                        parent[x] = y;
                    }
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for &i in &ids {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort_by_key(|g| std::cmp::Reverse(g.len()));
    out
}

#[test]
fn whole_road_network_is_one_connected_piece() {
    for p in PRESETS {
        for s in 1..=12u64 {
            let t = generate(p, s);
            let comps = components(&t);
            assert_eq!(
                comps.len(),
                1,
                "{} seed {s}: {} components, small ones: {:?}",
                p.name(),
                comps.len(),
                comps[1..]
                    .iter()
                    .take(3)
                    .map(|g| (g.len(), t.roads[g[0]].points[0]))
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn no_arterial_ends_in_open_land() {
    for p in PRESETS {
        for s in 1..=12u64 {
            let t = generate(p, s);
            let g = RoadGrid::new(&t.roads);
            for (i, r) in t
                .roads
                .iter()
                .enumerate()
                .filter(|(_, r)| r.class == RoadClass::Arterial)
            {
                for e in [r.points[0], *r.points.last().unwrap()] {
                    let on_edge = e.x < 1.0
                        || e.x > t.size - 1.0
                        || e.y > t.size - 1.0
                        || e.y < t.highway_y + 1.0;
                    let on_road = g
                        .near(e, 14.0)
                        .iter()
                        .any(|&(ri, dist)| ri != i && dist <= t.roads[ri].class.half_width() + 1.0);
                    assert!(
                        on_edge || on_road,
                        "{} seed {s}: arterial {i} ends in open land at ({:.0}, {:.0})",
                        p.name(),
                        e.x,
                        e.y
                    );
                }
            }
        }
    }
}
