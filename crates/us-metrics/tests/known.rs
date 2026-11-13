//! Metrics on hand-built networks whose answers are known.

use us_gen::{Pt, RoadClass, pt};
use us_metrics::graph::{build, faces};
use us_metrics::metrics::{compute, orientation_entropy};

type Lines = Vec<(RoadClass, Vec<Pt>)>;

/// n by n blocks of `s` metres, lower left corner at `o`.
fn grid(n: usize, s: f64, o: Pt) -> Lines {
    let mut v = Vec::new();
    for i in 0..=n {
        let k = i as f64 * s;
        v.push((
            RoadClass::Local,
            vec![pt(o.x, o.y + k), pt(o.x + n as f64 * s, o.y + k)],
        ));
        v.push((
            RoadClass::Local,
            vec![pt(o.x + k, o.y), pt(o.x + k, o.y + n as f64 * s)],
        ));
    }
    v
}

#[test]
fn three_by_three_junction_grid() {
    // 3 x 3 junctions = 2 x 2 blocks of 100 m. Lines cross without shared vertices, so this also
    // checks the noding.
    let g = build(&grid(2, 100.0, pt(1900.0, 1900.0)), 0.5, true);
    assert_eq!(g.nodes.len(), 9);
    assert_eq!(g.edges.len(), 12);
    let f = faces(&g);
    assert_eq!(f.len(), 4);
    for face in &f {
        assert!((face.area - 10_000.0).abs() < 1e-6);
        assert!((face.perimeter - 400.0).abs() < 1e-6);
    }
    let m = compute(&g);
    assert_eq!(m.n_blocks, 4);
    // 4 T junctions and 1 crossing; the 4 corners are bends, not junctions
    assert!((m.get("share_3way") - 0.8).abs() < 1e-9);
    assert!((m.get("share_4way") - 0.2).abs() < 1e-9);
    assert_eq!(m.get("share_dead_end"), 0.0);
    assert!((m.get("block_area_median") - 10_000.0).abs() < 1e-6);
    assert!((m.get("block_perimeter_mean") - 400.0).abs() < 1e-6);
    // two axes, both directions: four equal bins
    assert!((m.get("orientation_entropy") - 4.0_f64.ln()).abs() < 1e-9);
    // 1200 m of street in a 4096 m window
    let km2 = 4096.0 * 4096.0 / 1e6;
    assert!((m.get("street_density") - 1.2 / km2).abs() < 1e-9);
    assert!((m.get("intersection_density") - 5.0 / km2).abs() < 1e-9);
    assert!((m.get("share_local") - 1.0).abs() < 1e-12);
}

#[test]
fn big_grid_circuity_is_manhattan_over_euclid() {
    let g = build(&grid(12, 100.0, pt(1400.0, 1400.0)), 0.5, true);
    let m = compute(&g);
    let c = m.get("circuity");
    // taxicab distance over straight distance is between 1 (axis aligned) and sqrt 2 (diagonal)
    assert!(c > 1.05 && c < 1.414, "circuity {c}");
    assert_eq!(m.get("giant_component_share"), 1.0);
}

#[test]
fn cul_de_sac_star_has_only_dead_ends_and_one_junction() {
    let c = pt(2000.0, 2000.0);
    let lines: Lines = [0.0_f64, 120.0, 240.0]
        .iter()
        .map(|a| {
            let r = a.to_radians();
            (
                RoadClass::Local,
                vec![c, pt(c.x + 200.0 * r.cos(), c.y + 200.0 * r.sin())],
            )
        })
        .collect();
    let g = build(&lines, 0.5, true);
    assert_eq!(faces(&g).len(), 0);
    let m = compute(&g);
    assert_eq!(m.n_blocks, 0);
    assert!((m.get("share_dead_end") - 0.75).abs() < 1e-9);
    assert!((m.get("share_3way") - 0.25).abs() < 1e-9);
    assert_eq!(m.get("share_4way"), 0.0);
}

#[test]
fn ring_is_one_face_with_known_area() {
    let (n, r) = (12usize, 250.0_f64);
    let c = pt(2000.0, 2000.0);
    let mut pts: Vec<Pt> = (0..n)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            pt(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    pts.push(pts[0]);
    let g = build(&[(RoadClass::Collector, pts)], 0.5, true);
    let f = faces(&g);
    assert_eq!(f.len(), 1);
    let area = 0.5 * n as f64 * r * r * (std::f64::consts::TAU / n as f64).sin();
    let perim = n as f64 * 2.0 * r * (std::f64::consts::PI / n as f64).sin();
    assert!((f[0].area - area).abs() < 1e-6, "{} vs {area}", f[0].area);
    assert!((f[0].perimeter - perim).abs() < 1e-6);
    let m = compute(&g);
    assert_eq!(m.n_blocks, 1);
    assert_eq!(m.get("share_dead_end"), 0.0);
    assert!((m.get("share_collector") - 1.0).abs() < 1e-12);
}

#[test]
fn noding_splits_crossings_and_touching_ends() {
    // a cross, and a street ending exactly on the side of another
    let lines: Lines = vec![
        (
            RoadClass::Arterial,
            vec![pt(1000.0, 1000.0), pt(1200.0, 1000.0)],
        ),
        (
            RoadClass::Local,
            vec![pt(1100.0, 900.0), pt(1100.0, 1100.0)],
        ),
        (
            RoadClass::Local,
            vec![pt(1150.0, 1000.0), pt(1150.0, 1100.0)],
        ),
    ];
    let g = build(&lines, 0.5, true);
    let deg = g.degrees();
    assert_eq!(deg.iter().filter(|&&d| d == 4).count(), 1);
    assert_eq!(deg.iter().filter(|&&d| d == 3).count(), 1);
    // without noding nothing connects
    let g2 = build(&lines, 0.5, false);
    assert!(g2.degrees().iter().all(|&d| d == 1));
}

#[test]
fn orientation_entropy_of_one_street_is_ln2() {
    let g = build(
        &[(
            RoadClass::Local,
            vec![pt(1000.0, 1000.0), pt(1000.0, 1300.0)],
        )],
        0.5,
        true,
    );
    let sg = g.contract();
    assert!((orientation_entropy(&sg) - 2.0_f64.ln()).abs() < 1e-9);
}

#[test]
fn alleys_and_rail_stay_out_of_street_metrics() {
    let mut lines = grid(2, 100.0, pt(1900.0, 1900.0));
    lines.push((
        RoadClass::Alley,
        vec![pt(1950.0, 1900.0), pt(1950.0, 2000.0)],
    ));
    lines.push((
        RoadClass::Rail,
        vec![pt(1800.0, 1950.0), pt(2300.0, 1950.0)],
    ));
    let m = compute(&build(&lines, 0.5, true));
    assert_eq!(m.n_blocks, 4);
    assert!((m.get("share_3way") - 0.8).abs() < 1e-9);
    assert!((m.get("share_alley") - 100.0 / 1300.0).abs() < 1e-9);
}
