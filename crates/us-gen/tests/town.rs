//! Independent checks on generated towns. Nothing here reuses generator internals.

use std::collections::HashMap;
use us_gen::{Preset, Pt, Road, RoadClass, Town, Zone, generate, pt};

const SEEDS: [u64; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
const PRESETS: [Preset; 9] = Preset::ALL;

fn towns() -> &'static [Town] {
    static TOWNS: std::sync::OnceLock<Vec<Town>> = std::sync::OnceLock::new();
    TOWNS.get_or_init(|| {
        let mut v = Vec::new();
        for p in PRESETS {
            for s in SEEDS {
                v.push(generate(p, s));
            }
        }
        v
    })
}

fn town(p: Preset, s: u64) -> &'static Town {
    towns()
        .iter()
        .find(|t| t.preset == p && t.seed == s)
        .expect("town in cache")
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
    for t in towns().iter() {
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
    for t in towns().iter() {
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
    for t in towns().iter() {
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
    for t in towns().iter() {
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
    for t in towns().iter() {
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
            let t = town(p, s);
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
                    // entrance sits on a highway, arterial or (in sunbelt towns) collector
                    assert!(
                        t.roads
                            .iter()
                            .filter(|r| {
                                matches!(r.class, RoadClass::Highway | RoadClass::Arterial)
                                    || (p.base() == Preset::Sunbelt
                                        && r.class == RoadClass::Collector)
                            })
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
    assert!(pods_seen >= 6, "only {pods_seen} pods across 60 towns");
}

#[test]
fn long_addition_edges_become_collectors() {
    for t in towns().iter() {
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
fn downtown_has_a_cluster_of_towers_at_its_centres() {
    for t in towns().iter() {
        let offices: Vec<_> = t
            .lots
            .iter()
            .filter(|l| l.kind == us_gen::LotKind::Office)
            .collect();
        assert!(
            (1..=16).contains(&offices.len()),
            "{} seed {}: {} towers",
            t.preset.name(),
            t.seed,
            offices.len()
        );
        for o in offices {
            let c = us_gen::centrality(&t.centres, o.centre);
            assert!(
                c >= 0.55,
                "office {:.0} m from the hall has centrality {c:.2}",
                d(o.centre, t.hall)
            );
            assert!(o.density > 0.7, "office density {:.2}", o.density);
        }
    }
}

#[test]
fn version_one_towns_are_unchanged() {
    // saves from 1.0.0 preview 6 and 7 rebuild their town with generator version 1
    let known = [
        (Preset::Grid, 1, 0x5a21_c829_5a15_dac2u64),
        (Preset::Winding, 2, 0x4b2b_4134_c47f_1068),
        (Preset::Sunbelt, 3, 0xc5bd_9134_c70e_b74b),
        (Preset::Crossroads, 4, 0xc662_cfba_26fb_cb07),
        (Preset::LakeTown, 1, 0x08e6_8d74_ff49_7b24),
    ];
    for (p, s, fp) in known {
        let t = us_gen::generate_version(p, s, 1);
        assert_eq!(t.fingerprint(), fp, "{} seed {s}", p.name());
        assert!(t.stations.is_empty());
    }
}

#[test]
fn version_three_towns_of_the_five_first_styles_are_unchanged() {
    // the newer styles must never reach the code of the five 1.0 styles
    let known = [
        (Preset::Grid, 1, 0x3226_bdf6_1bd7_8561u64),
        (Preset::Grid, 2, 0x5687_d114_417a_755au64),
        (Preset::Grid, 3, 0x1c0f_6c12_41e0_d3d2u64),
        (Preset::Grid, 4, 0xfc08_ddb4_7d49_9082u64),
        (Preset::Winding, 1, 0xd3a6_774c_9a5a_64c2u64),
        (Preset::Winding, 2, 0x14c0_9d25_3ced_f9a4u64),
        (Preset::Winding, 3, 0x9b92_0a40_c5ac_6f41u64),
        (Preset::Winding, 4, 0x925e_d1e3_181f_f426u64),
        (Preset::Sunbelt, 1, 0x7924_7572_1d1c_c46bu64),
        (Preset::Sunbelt, 2, 0x1d8d_5cb0_0965_d273u64),
        (Preset::Sunbelt, 3, 0x1b62_1a2d_c6a1_c1a6u64),
        (Preset::Sunbelt, 4, 0x8488_0390_cd6d_7458u64),
        (Preset::Crossroads, 1, 0x7c3b_9f12_b203_fda1u64),
        (Preset::Crossroads, 2, 0x150f_bd21_9114_c683u64),
        (Preset::Crossroads, 3, 0x17be_ddd8_bfc3_a0e5u64),
        (Preset::Crossroads, 4, 0xcb12_31c1_3de7_de5au64),
        (Preset::LakeTown, 1, 0x2fe9_0211_b2a6_62d9u64),
        (Preset::LakeTown, 2, 0xa77f_d1b4_ddc0_2de0u64),
        (Preset::LakeTown, 3, 0x812d_7986_5c3a_7addu64),
        (Preset::LakeTown, 4, 0x5b9a_e8b9_ca1e_764du64),
    ];
    for (p, s, fp) in known {
        let t = us_gen::generate_version(p, s, 3);
        assert_eq!(t.fingerprint(), fp, "{} seed {s}", p.name());
    }
}

#[test]
fn version_two_towns_are_unchanged() {
    // saves from 1.0.0 and 1.0.1 rebuild their town with generator version 2
    let known = [
        (Preset::Grid, 1, 0xcb03_bc97_0ebb_e3f5u64),
        (Preset::Winding, 2, 0x0bc1_a38a_1424_de83),
        (Preset::Sunbelt, 3, 0xe9b8_3920_169a_345c),
        (Preset::Crossroads, 4, 0x4d60_3af9_ab41_749a),
        (Preset::LakeTown, 1, 0x8365_8859_dbbb_d245),
    ];
    for (p, s, fp) in known {
        let t = us_gen::generate_version(p, s, 2);
        assert_eq!(t.fingerprint(), fp, "{} seed {s}", p.name());
    }
}

#[test]
fn every_railway_through_town_has_a_depot_clear_of_roads_and_lots() {
    for t in towns().iter() {
        let depots: Vec<_> = t
            .stations
            .iter()
            .filter(|s| s.kind == us_gen::StationKind::Depot)
            .collect();
        assert!(
            !depots.is_empty(),
            "{} seed {}: no depot",
            t.preset.name(),
            t.seed
        );
        let rails: Vec<_> = t
            .roads
            .iter()
            .filter(|r| r.class == RoadClass::Rail)
            .collect();
        for s in &t.stations {
            let to_rail = rails
                .iter()
                .flat_map(|r| {
                    r.points
                        .windows(2)
                        .map(move |w| seg_closest(s.pos, w[0], w[1]))
                })
                .fold(f64::INFINITY, f64::min);
            assert!(to_rail < 25.0, "station {to_rail:.0} m from the track");
            for r in t.roads.iter().filter(|r| r.class != RoadClass::Rail) {
                for w in r.points.windows(2) {
                    let dd = seg_closest(s.pos, w[0], w[1]);
                    assert!(dd >= s.radius + r.class.half_width(), "station on a road");
                }
            }
            for l in &t.lots {
                assert!(
                    d(l.centre, s.pos) >= l.radius + s.radius - 1e-6,
                    "lot on a station"
                );
            }
        }
        // the depot is a centre the town gathers round
        assert!(
            t.centres
                .iter()
                .any(|c| c.kind == us_gen::CentreKind::Station),
            "{} seed {}: depot is not a centre",
            t.preset.name(),
            t.seed
        );
    }
}

#[test]
fn no_lot_stands_in_a_park() {
    for t in towns().iter() {
        for (i, l) in t.lots.iter().enumerate() {
            if l.kind == us_gen::LotKind::Farm {
                continue;
            }
            for a in t.additions.iter().filter(|a| a.zone == Zone::Park) {
                assert!(
                    !point_in(l.centre, &a.polygon)
                        && ring_dist(l.centre, &a.polygon) >= l.radius - 1e-6,
                    "{} seed {}: lot {i} ({:?}) in a park",
                    t.preset.name(),
                    t.seed,
                    l.kind
                );
            }
        }
    }
}

#[test]
fn no_street_ends_on_the_railway() {
    for t in towns().iter() {
        let culs: Vec<Pt> = t.cul_de_sacs.iter().map(|c| c.pos).collect();
        for (i, r) in t
            .roads
            .iter()
            .enumerate()
            .filter(|(_, r)| is_street(r.class))
        {
            if d(r.points[0], *r.points.last().unwrap()) < 1e-6 {
                continue;
            }
            for e in [r.points[0], *r.points.last().unwrap()] {
                if culs.iter().any(|c| d(*c, e) < 1e-6) {
                    continue;
                }
                let on_road = t.roads.iter().enumerate().any(|(j, o)| {
                    j != i
                        && o.class != RoadClass::Rail
                        && o.points
                            .windows(2)
                            .any(|w| seg_closest(e, w[0], w[1]) < 1.5)
                });
                assert!(
                    on_road,
                    "{} seed {}: road {i} ({:?}) ends at ({:.0}, {:.0}) with no road",
                    t.preset.name(),
                    t.seed,
                    r.class,
                    e.x,
                    e.y
                );
            }
        }
    }
}

#[test]
fn lots_are_densest_at_the_centres() {
    for t in towns().iter() {
        let (mut core, mut nc, mut outer, mut no) = (0.0, 0.0, 0.0, 0.0);
        for l in &t.lots {
            match l.zone {
                Zone::Core => {
                    core += l.density;
                    nc += 1.0;
                }
                Zone::Outer => {
                    outer += l.density;
                    no += 1.0;
                }
                _ => {}
            }
            assert!((0.0..=1.0).contains(&l.density));
        }
        if nc > 0.0 && no > 0.0 {
            assert!(
                core / nc > 2.0 * outer / no,
                "{} seed {}",
                t.preset.name(),
                t.seed
            );
        }
    }
}

#[test]
fn streets_do_not_run_along_each_other() {
    for t in towns().iter() {
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
    for t in towns().iter() {
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
            let t = town(p, s);
            let comps = components(t);
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
            let t = town(p, s);
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

#[test]
fn ground_is_flat_at_the_hall_and_steep_ground_is_unbuilt() {
    for t in towns() {
        assert!(
            t.slope_at(t.hall) < 0.04,
            "{} seed {}: hall on a slope",
            t.preset.name(),
            t.seed
        );
        let mut steep = 0usize;
        let mut cells = 0usize;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..64 {
            for j in 0..64 {
                let p = us_gen::pt(30.0 + i as f64 * 63.0, 130.0 + j as f64 * 61.0);
                cells += 1;
                let h = t.height_at(p);
                lo = lo.min(h);
                hi = hi.max(h);
                if t.slope_at(p) > 0.12 {
                    steep += 1;
                }
            }
        }
        assert!(
            hi - lo > 40.0,
            "{} seed {}: no relief",
            t.preset.name(),
            t.seed
        );
        // the mountain valley is mostly mountain: its steep share is the point of the style
        // (a little over half may be steep there; a quarter everywhere else)
        let limit = if t.preset == Preset::Valley {
            cells * 6 / 10
        } else {
            cells / 4
        };
        assert!(
            steep < limit,
            "{} seed {}: too much steep ground",
            t.preset.name(),
            t.seed
        );
        for l in &t.lots {
            assert!(t.slope_at(l.centre) < 0.15 + 1e-9, "lot on steep ground");
        }
    }
}

#[test]
fn terrain_is_deterministic_and_exposes_a_heightfield() {
    let a = generate(Preset::Winding, 5);
    let b = generate(Preset::Winding, 5);
    assert_eq!(a.terrain.heightfield(33), b.terrain.heightfield(33));
    assert_ne!(
        a.terrain.heightfield(33),
        generate(Preset::Winding, 6).terrain.heightfield(33)
    );
    assert_eq!(a.terrain.heightfield(33).len(), 33 * 33);
}

#[test]
fn old_roads_do_not_climb_steep_hills_head_on() {
    // along each arterial the grade stays gentle on average: roads follow the land
    for t in towns().iter().filter(|t| t.preset == Preset::Winding) {
        for r in t.roads.iter().filter(|r| r.class == RoadClass::Arterial) {
            let mut sum = 0.0;
            let mut n = 0.0;
            for w in r.points.windows(2) {
                let len = d(w[0], w[1]);
                if len > 1.0 {
                    sum += ((t.height_at(w[1]) - t.height_at(w[0])) / len).abs();
                    n += 1.0;
                }
            }
            if n > 20.0 {
                assert!(
                    sum / n < 0.07,
                    "{} seed {}: arterial climbs at {:.3}",
                    t.preset.name(),
                    t.seed,
                    sum / n
                );
            }
        }
    }
}

#[test]
fn lake_towns_have_a_lake_with_a_drive_round_it() {
    for t in towns().iter().filter(|t| t.preset == Preset::LakeTown) {
        assert_eq!(t.water.len(), 2, "river and lake");
        let lake = &t.water[1];
        // the drive: collector pieces that stay about 55 m off the shore, nearly all the way round
        let mut drive_len = 0.0;
        for r in t.roads.iter().filter(|r| r.class == RoadClass::Collector) {
            if r.points
                .iter()
                .all(|p| (ring_dist(*p, lake) - 55.0).abs() < 8.0)
            {
                drive_len += r.points.windows(2).map(|w| d(w[0], w[1])).sum::<f64>();
            }
        }
        let shore: f64 = lake.windows(2).map(|w| d(w[0], w[1])).sum();
        assert!(
            drive_len > shore * 0.9,
            "seed {}: lakeside drive covers {drive_len:.0} of {shore:.0} m",
            t.seed
        );
        assert!(
            t.lots.iter().any(|l| ring_dist(l.centre, lake) < 160.0),
            "seed {}: nothing built near the lake",
            t.seed
        );
        for r in &t.roads {
            for p in &r.points {
                assert!(
                    !point_in(*p, lake),
                    "{:?} road runs through the lake (seed {})",
                    r.class,
                    t.seed
                );
            }
        }
    }
}

#[test]
fn crossroads_towns_have_two_highways_crossing_at_the_hall() {
    for t in towns().iter().filter(|t| t.preset == Preset::Crossroads) {
        let through = t
            .roads
            .iter()
            .filter(|r| r.class == RoadClass::Arterial)
            .filter(|r| {
                r.points
                    .windows(2)
                    .any(|w| seg_closest(t.hall, w[0], w[1]) < 60.0)
            })
            .count();
        assert!(
            through >= 2,
            "{} seed {}: only {through} roads through the centre",
            t.preset.name(),
            t.seed
        );
    }
}

#[test]
fn sunbelt_towns_are_pods_and_commercial_corners_on_a_mile_grid() {
    for t in towns().iter().filter(|t| t.preset == Preset::Sunbelt) {
        let pods = t
            .additions
            .iter()
            .filter(|a| a.pattern == us_gen::Pattern::Pods)
            .count();
        let malls = t.additions.iter().filter(|a| a.zone == Zone::Mall).count();
        assert!(pods >= 3, "seed {}: {pods} pods", t.seed);
        assert!(malls >= 1, "seed {}: no commercial corners", t.seed);
        let culs = t.cul_de_sacs.len();
        assert!(culs >= 20, "seed {}: only {culs} cul-de-sacs", t.seed);
    }
}

fn ring_area(r: &[Pt]) -> f64 {
    let mut s = 0.0;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[(i + 1) % r.len()]);
        s += a.x * b.y - b.x * a.y;
    }
    s.abs() / 2.0
}

/// Towns used to be about 700 buildings per square kilometre of developed land; they must now be
/// about twice that, in every preset, without going past what the streets can hold.
#[test]
fn every_preset_is_about_twice_as_dense_as_before() {
    for p in PRESETS {
        let (mut land, mut built, mut foot) = (0.0, 0.0, 0.0);
        for s in 1..=3 {
            let t = town(p, s);
            land += t
                .additions
                .iter()
                .filter(|a| !matches!(a.zone, Zone::Farm | Zone::Park))
                .map(|a| ring_area(&a.polygon))
                .sum::<f64>();
            for l in t
                .lots
                .iter()
                .filter(|l| l.zone != Zone::Farm && !matches!(l.kind, us_gen::LotKind::Park))
            {
                built += 1.0;
                foot += 0.7 * std::f64::consts::PI * l.radius * l.radius;
            }
        }
        let per_km2 = built / (land / 1e6);
        let coverage = foot / land;
        assert!(
            (1300.0..2200.0).contains(&per_km2),
            "{}: {per_km2:.0} buildings per km2 of developed land",
            p.name()
        );
        assert!(
            (0.15..0.35).contains(&coverage),
            "{}: {:.1} per cent of developed land built over",
            p.name(),
            coverage * 100.0
        );
    }
}

fn is_ring(r: &Road) -> bool {
    r.class == RoadClass::Collector
        && r.points.len() == 33
        && r.points[0].dist(*r.points.last().unwrap()) < 1e-6
}

#[test]
fn version_three_towns_have_roundabouts_that_roads_join() {
    let mut rings = 0;
    for t in towns().iter() {
        for r in t.roads.iter().filter(|r| is_ring(r)) {
            rings += 1;
            let c = {
                let n = r.points.len() as f64;
                Pt {
                    x: r.points.iter().map(|p| p.x).sum::<f64>() / n,
                    y: r.points.iter().map(|p| p.y).sum::<f64>() / n,
                }
            };
            // arms: other roads with an end on the ring
            let arms = t
                .roads
                .iter()
                .filter(|o| !std::ptr::eq(*o, r) && o.class != RoadClass::Rail)
                .filter(|o| {
                    [o.points[0], *o.points.last().unwrap()].iter().any(|e| {
                        r.points
                            .windows(2)
                            .any(|w| seg_closest(*e, w[0], w[1]) < 1.0)
                    })
                })
                .count();
            assert!(
                arms >= 3,
                "{} seed {}: roundabout at ({:.0}, {:.0}) has {arms} roads on it",
                t.preset.name(),
                t.seed,
                c.x,
                c.y
            );
            // nothing but the arms comes within the ring's outer edge
            for o in t.roads.iter().filter(|o| !std::ptr::eq(*o, r)) {
                for p in &o.points {
                    assert!(
                        d(*p, c) > 15.0 || o.class == RoadClass::Rail,
                        "a road runs into the island"
                    );
                }
            }
        }
    }
    assert!(rings >= 20, "only {rings} roundabouts in 60 towns");
}

#[test]
fn version_three_towns_have_ramps_between_highway_and_arterial() {
    let mut ramps = 0;
    for t in towns().iter() {
        let hwy: Vec<&Road> = t
            .roads
            .iter()
            .filter(|r| r.class == RoadClass::Highway)
            .collect();
        for r in t.roads.iter().filter(|r| r.class == RoadClass::Collector) {
            if r.points.len() != 15 {
                continue;
            }
            let touches = |p: Pt, line: &Road| {
                line.points
                    .windows(2)
                    .any(|w| seg_closest(p, w[0], w[1]) < 1.0)
            };
            let on_hwy = hwy
                .iter()
                .any(|h| touches(r.points[0], h) || touches(*r.points.last().unwrap(), h));
            let on_art = t.roads.iter().any(|a| {
                a.class == RoadClass::Arterial
                    && (touches(r.points[0], a) || touches(*r.points.last().unwrap(), a))
            });
            if on_hwy && on_art {
                ramps += 1;
            }
        }
    }
    assert!(ramps >= 20, "only {ramps} ramps in 60 towns");
}

// ---- river gorge and mountain valley ----

fn style_towns(p: Preset) -> impl Iterator<Item = &'static Town> {
    towns().iter().filter(move |t| t.preset == p)
}

/// Heights along a road with the stretches that stand on a bridge left out: where the ground
/// falls well below the straight line between the ground 120 m before and 120 m after, the road
/// is carried across and does not follow the ground.
fn climbs_on_the_ground(t: &Town, r: &Road) -> Vec<f64> {
    // resampled every 10 m along the road
    let mut pts: Vec<Pt> = vec![r.points[0]];
    let mut carry = 0.0;
    for w in r.points.windows(2) {
        let l = d(w[0], w[1]);
        let mut at = 10.0 - carry;
        while at <= l {
            pts.push(w[0].lerp(w[1], at / l));
            at += 10.0;
        }
        carry = l - (at - 10.0);
    }
    let h: Vec<f64> = pts.iter().map(|p| t.height_at(*p)).collect();
    let n = pts.len();
    let reach = 80usize; // 800 m: the widest span
    let carried: Vec<bool> = (0..n)
        .map(|i| {
            let a = i.saturating_sub(reach);
            let b = (i + reach).min(n - 1);
            let top_l = h[a..=i].iter().cloned().fold(f64::MIN, f64::max);
            let top_r = h[i..=b].iter().cloned().fold(f64::MIN, f64::max);
            top_l.min(top_r) - h[i] > 12.0
        })
        .collect();
    // the abutments at both ends of a span count as part of it
    let near_span: Vec<bool> = (0..n)
        .map(|i| carried[i.saturating_sub(3)..(i + 4).min(n)].iter().any(|&c| c))
        .collect();
    let mut grades = Vec::new();
    // grade over 30 m: a hairpin may have a short steeper bend
    for i in 0..n.saturating_sub(3) {
        if !near_span[i..=i + 3].iter().any(|&c| c) {
            grades.push(((h[i + 3] - h[i]) / d(pts[i], pts[i + 3])).abs());
        }
    }
    grades
}

#[test]
fn mountain_roads_stay_within_a_grade_a_road_can_climb() {
    for p in [Preset::Gorge, Preset::Valley] {
        for t in style_towns(p) {
            for (i, r) in t.roads.iter().enumerate() {
                // streets and collectors inside a patch follow the shared street code on ground the lots allow
                if !matches!(r.class, RoadClass::Highway | RoadClass::Arterial) {
                    continue;
                }
                let g = climbs_on_the_ground(t, r);
                if g.len() < 20 {
                    continue;
                }
                let mean = g.iter().sum::<f64>() / g.len() as f64;
                let worst = g.iter().cloned().fold(0.0, f64::max);
                // a hairpin road on a gully may be steeper than a street, never past a third
                assert!(
                    mean < 0.1 && worst < 0.34,
                    "{} seed {}: road {i} ({}) climbs at mean {mean:.3}, worst {worst:.3}",
                    p.name(),
                    t.seed,
                    r.class.name()
                );
            }
        }
    }
}

#[test]
fn the_gorge_is_deep_and_the_main_road_crosses_it_on_a_bridge() {
    for t in style_towns(Preset::Gorge) {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for i in 0..80 {
            for j in 0..80 {
                let h = t.height_at(pt(i as f64 * 51.0, 300.0 + j as f64 * 45.0));
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        assert!(hi - lo > 280.0, "gorge seed {}: only {:.0} m deep", t.seed, hi - lo);
        // a road that goes down into the gorge and up again is carried over it
        let spans = t
            .roads
            .iter()
            .filter(|r| matches!(r.class, RoadClass::Arterial | RoadClass::Highway))
            .filter(|r| {
                let h: Vec<f64> = r.points.iter().map(|p| t.height_at(*p)).collect();
                let samples: Vec<f64> = {
                    let mut v = Vec::new();
                    for w in r.points.windows(2) {
                        let k = ((d(w[0], w[1]) / 10.0).ceil() as usize).max(1);
                        for i in 0..k {
                            v.push(t.height_at(w[0].lerp(w[1], i as f64 / k as f64)));
                        }
                    }
                    v
                };
                let _ = h;
                samples.windows(40).any(|w| {
                    let top = w[0].min(w[w.len() - 1]);
                    let bottom = w.iter().cloned().fold(f64::MAX, f64::min);
                    top - bottom > 120.0
                })
            })
            .count();
        assert!(spans >= 2, "gorge seed {}: {spans} roads span a gorge", t.seed);
    }
}

#[test]
fn the_gorge_has_a_flat_bench_and_a_switchback_down_to_it() {
    for t in style_towns(Preset::Gorge) {
        // lots on the bench: low ground, a few hundred metres above nothing
        let low: Vec<&us_gen::Lot> = t.lots.iter().filter(|l| t.height_at(l.centre) < 8.0).collect();
        assert!(low.len() >= 20, "gorge seed {}: {} lots on the riverside flat", t.seed, low.len());
        // the switchback is a long road with many turns that ends on the flat
        let sw = t.roads.iter().find(|r| {
            r.class == RoadClass::Arterial
                && t.height_at(*r.points.last().unwrap()) < 8.0
                && t.height_at(r.points[0]) > 150.0
        });
        let sw = sw.unwrap_or_else(|| panic!("gorge seed {}: no road down to the flat", t.seed));
        let mut turns = 0;
        for w in sw.points.windows(3) {
            let a1 = (w[1].y - w[0].y).atan2(w[1].x - w[0].x);
            let a2 = (w[2].y - w[1].y).atan2(w[2].x - w[1].x);
            if (a2 - a1).sin().abs() > 0.25 {
                turns += 1;
            }
        }
        assert!(turns >= 12, "gorge seed {}: switchback has only {turns} bends", t.seed);
    }
}

#[test]
fn floods_reach_only_the_low_flat_of_a_gorge() {
    for t in style_towns(Preset::Gorge) {
        let near_water: usize = t.lots.iter().filter(|l| t.height_at(l.centre) < 12.0).count();
        let all = t.lots.len();
        assert!(near_water * 20 < all, "gorge seed {}: {near_water} of {all} lots on the floor", t.seed);
    }
}

#[test]
fn the_valley_floor_is_the_town_and_ridges_stand_far_above() {
    for t in style_towns(Preset::Valley) {
        let mut hi = f64::MIN;
        for i in 0..80 {
            for j in 0..80 {
                hi = hi.max(t.height_at(pt(i as f64 * 51.0, 100.0 + j as f64 * 49.0)));
            }
        }
        assert!(hi > 600.0, "valley seed {}: peaks only {hi:.0} m", t.seed);
        let low = t.lots.iter().filter(|l| t.height_at(l.centre) < 90.0).count();
        assert!(low * 10 > t.lots.len() * 9, "valley seed {}: lots up the slopes", t.seed);
        assert!(t.lots.len() > 3500, "valley seed {}: only {} lots", t.seed, t.lots.len());
        // a road runs the length of the valley, and one goes up each side valley
        let long = t
            .roads
            .iter()
            .filter(|r| r.class == RoadClass::Arterial && d(r.points[0], *r.points.last().unwrap()) > 3500.0)
            .count();
        assert!(long >= 2, "valley seed {}: {long} long roads", t.seed);
    }
}
