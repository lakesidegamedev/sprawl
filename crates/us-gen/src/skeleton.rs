//! Stage one: the roads and rails that exist before the town does.

use crate::geom::{
    Pt, clip_line, dedup, dist_pt_line, dist_ring_line, interpolate, length, project, pt,
    split_line_at,
};
use crate::rng::Rng;
use crate::terrain::Terrain;
use crate::world::{HALL, HWY_Y, Lake, River, S, TAU, ln_angle, map_ring, ring_road, wander_line};
use crate::{Preset, RoadClass, Settings};

pub struct Skeleton {
    pub roads: Vec<(Vec<Pt>, RoadClass)>,
    pub rails: Vec<Vec<Pt>>,
    pub rail_angle: f64,
}

pub fn highway() -> Vec<Pt> {
    vec![pt(0.0, HWY_Y), pt(S, HWY_Y)]
}

fn within_map(line: &[Pt]) -> Vec<Vec<Pt>> {
    clip_line(line, &map_ring(), true)
        .into_iter()
        .map(|l| dedup(&l))
        .filter(|l| l.len() >= 2)
        .collect()
}

fn without_zone(lines: Vec<Vec<Pt>>, zone: &[Pt]) -> Vec<Vec<Pt>> {
    lines
        .into_iter()
        .flat_map(|l| clip_line(&l, zone, false))
        .collect()
}

pub fn build(
    preset: Preset,
    seed: u64,
    river: &River,
    st: &Settings,
    terrain: &Terrain,
    lake: Option<&Lake>,
) -> Skeleton {
    let tr = Some(terrain);
    let drive_zone = river.band(-116.0, 116.0);
    let mut roads: Vec<(Vec<Pt>, RoadClass)> = vec![(highway(), RoadClass::Highway)];
    let mut rails: Vec<Vec<Pt>> = Vec::new();
    let mut rng = Rng::new(seed, 0x5ce1);
    let rail_angle;
    match preset {
        Preset::Grid => {
            let bridged = if rng.chance(0.5) { 1 } else { -1 };
            for k in -3..=3i32 {
                let x = HALL.x + k as f64 * st.arterial_spacing;
                if !(120.0..=S - 120.0).contains(&x) {
                    continue;
                }
                let ln = vec![pt(x, HWY_Y), pt(x, S)];
                let pieces = if k == 0 || k == bridged {
                    vec![ln]
                } else {
                    without_zone(vec![ln], &drive_zone)
                };
                roads.extend(
                    pieces
                        .into_iter()
                        .filter(|p| length(p) > 100.0)
                        .map(|p| (p, RoadClass::Arterial)),
                );
            }
            let band150 = river.band(-150.0, 150.0);
            for j in -2..5i32 {
                let y = HALL.y + j as f64 * st.arterial_spacing;
                if !(HWY_Y + 120.0..=S - 120.0).contains(&y) {
                    continue;
                }
                let ln = vec![pt(0.0, y), pt(S, y)];
                if dist_ring_line(&band150, &ln) == 0.0 {
                    continue;
                }
                roads.push((ln, RoadClass::Arterial));
            }
            rail_angle = (32.0 + rng.uniform(-8.0, 8.0)).to_radians();
            let off = pt(
                330.0 + rng.uniform(-120.0, 120.0),
                -200.0 + rng.uniform(-100.0, 100.0),
            );
            for p in within_map(&ln_angle(rail_angle, pt(HALL.x + off.x, HALL.y + off.y))) {
                rails.push(p);
            }
            let diags = [
                wander_line(
                    HALL,
                    (138.0 + rng.uniform(-8.0, 8.0)).to_radians(),
                    &mut Rng::new(seed, 4),
                    0.12,
                    tr,
                ),
                wander_line(
                    HALL,
                    (-48.0 + rng.uniform(-8.0, 8.0)).to_radians(),
                    &mut Rng::new(seed, 6),
                    0.10,
                    tr,
                ),
            ];
            for d in diags {
                for p in without_zone(within_map(&d), &drive_zone) {
                    if length(&p) > 100.0 && dist_pt_line(HALL, &p) < 5.0 {
                        roads.push((p, RoadClass::Arterial));
                    }
                }
            }
        }
        Preset::Winding => {
            let south = wander_line(
                HALL,
                (-95.0 + rng.uniform(-6.0, 6.0)).to_radians(),
                &mut Rng::new(seed, 21),
                0.2,
                tr,
            );
            let north = wander_line(
                HALL,
                (80.0 + rng.uniform(-6.0, 6.0)).to_radians(),
                &mut Rng::new(seed, 24),
                0.3,
                tr,
            );
            let mut main: Vec<Pt> = south.iter().rev().copied().collect();
            main.extend(north.iter().skip(1).copied());
            let mains: Vec<Vec<Pt>> = within_map(&main)
                .into_iter()
                .filter(|p| dist_pt_line(HALL, p) < 5.0)
                .collect();
            for m in &mains {
                roads.push((m.clone(), RoadClass::Arterial));
            }
            let branches: [(f64, f64); 5] = [
                (-30.0, -260.0),
                (25.0, 180.0),
                (140.0, 330.0),
                (205.0, -120.0),
                (-150.0, -520.0),
            ];
            if let Some(m) = mains.first() {
                let h0 = project(m, HALL).0;
                for (i, (hd, off)) in branches.iter().enumerate() {
                    let st = interpolate(m, h0 + off).0;
                    let ln = wander_line(
                        st,
                        (hd + rng.uniform(-8.0, 8.0)).to_radians(),
                        &mut Rng::new(seed, 31 + i as u64),
                        0.5,
                        tr,
                    );
                    for p in without_zone(within_map(&ln), &drive_zone) {
                        if length(&p) > 100.0 && dist_pt_line(st, &p) < 5.0 {
                            roads.push((p, RoadClass::Arterial));
                        }
                    }
                }
            }
            let radials: Vec<Vec<Pt>> = roads.iter().skip(1).map(|(l, _)| l.clone()).collect();
            let mut keep_rng = Rng::new(seed, 9);
            for (r0, salt, keep) in [(950.0, 3u64, 0.75), (1750.0, 8u64, 0.6)] {
                let mut ring = ring_road(r0, &mut Rng::new(seed, salt));
                // start the loop at its point nearest the river so the river cut never splits
                // one stretch of ring into two pieces that meet at a bare joint
                ring.pop();
                let k = (0..ring.len())
                    .min_by(|&a, &b| {
                        let off = |p: Pt| (p.y - river.y(p.x.clamp(0.0, S))).abs();
                        off(ring[a]).total_cmp(&off(ring[b]))
                    })
                    .unwrap_or(0);
                ring.rotate_left(k);
                ring.push(ring[0]);
                for arc in without_zone(within_map(&ring), &drive_zone) {
                    let mut pieces = split_line_at(&arc, &radials);
                    // a ring that was never cut starts and ends at the same bare spot: rejoin it
                    if pieces.len() > 1 && arc[0].dist(*arc.last().unwrap()) < 1e-6 {
                        let first = pieces.remove(0);
                        let last = pieces.last_mut().unwrap();
                        last.extend(first.into_iter().skip(1));
                    }
                    for q in pieces {
                        if length(&q) > 200.0 && keep_rng.chance(keep) {
                            roads.push((q, RoadClass::Arterial));
                        }
                    }
                }
            }
            rail_angle = (12.0 + rng.uniform(-5.0, 5.0)).to_radians();
            let start = pt(0.0, 600.0 + rng.uniform(-150.0, 250.0));
            for p in within_map(&wander_line(
                start,
                rail_angle,
                &mut Rng::new(seed, 2),
                0.08,
                tr,
            )) {
                rails.push(p);
            }
        }
        Preset::Sunbelt => {
            // arterials every mile, collectors halfway between, the hall on a crossing
            let half = st.arterial_spacing / 2.0;
            let band150 = river.band(-150.0, 150.0);
            let art_x: Vec<f64> = (-6..=6i32)
                .filter(|m| m % 2 == 0)
                .map(|m| HALL.x + m as f64 * half)
                .filter(|x| (120.0..=S - 120.0).contains(x))
                .collect();
            for m in -6..=6i32 {
                let x = HALL.x + m as f64 * half;
                if !(120.0..=S - 120.0).contains(&x) {
                    continue;
                }
                let ln = vec![pt(x, HWY_Y), pt(x, S)];
                if m % 2 == 0 {
                    let pieces = if m == 0 {
                        vec![ln]
                    } else {
                        without_zone(vec![ln], &drive_zone)
                    };
                    roads.extend(
                        pieces
                            .into_iter()
                            .filter(|p| length(p) > 100.0)
                            .map(|p| (p, RoadClass::Arterial)),
                    );
                } else {
                    // collectors run from the highway to the river drive and no further
                    for p in without_zone(vec![ln], &drive_zone) {
                        if length(&p) > 100.0 && p.iter().any(|q| q.y <= HWY_Y + 1.0) {
                            roads.push((p, RoadClass::Collector));
                        }
                    }
                }
            }
            for m in -4..=6i32 {
                let y = HALL.y + m as f64 * half;
                if !(HWY_Y + 120.0..=S - 120.0).contains(&y) {
                    continue;
                }
                if m % 2 == 0 {
                    let ln = vec![pt(0.0, y), pt(S, y)];
                    if dist_ring_line(&band150, &ln) == 0.0 {
                        continue;
                    }
                    roads.push((ln, RoadClass::Arterial));
                } else if art_x.len() >= 2 {
                    // between the outermost arterials, so both ends sit on one
                    let ln = vec![pt(art_x[0], y), pt(*art_x.last().unwrap(), y)];
                    if dist_ring_line(&band150, &ln) > 0.0 {
                        roads.push((ln, RoadClass::Collector));
                    }
                }
            }
            rail_angle = (-8.0 + rng.uniform(-4.0, 4.0)).to_radians();
            let start = pt(0.0, 900.0 + rng.uniform(-100.0, 100.0));
            for p in within_map(&wander_line(
                start,
                rail_angle,
                &mut Rng::new(seed, 2),
                0.05,
                tr,
            )) {
                rails.push(p);
            }
        }
        Preset::Crossroads => {
            // two highways cross at the hall; old roads fan out from the crossing
            let two_way = |heading: f64, anchor: Pt, salt: u64, amp: f64| -> Vec<Pt> {
                let a = wander_line(
                    anchor,
                    heading + std::f64::consts::PI,
                    &mut Rng::new(seed, salt),
                    amp,
                    tr,
                );
                let b = wander_line(anchor, heading, &mut Rng::new(seed, salt + 1), amp, tr);
                let mut l: Vec<Pt> = a.iter().rev().copied().collect();
                l.extend(b.iter().skip(1).copied());
                l
            };
            let tilt = rng.uniform(-6.0, 6.0).to_radians();
            let ns = two_way(
                std::f64::consts::FRAC_PI_2 + tilt,
                pt(HALL.x - 1.3, HALL.y),
                40,
                0.05,
            );
            let ew = two_way(tilt * 0.5, pt(HALL.x, HALL.y + 1.7), 42, 0.05);
            let mut mains: Vec<Vec<Pt>> = Vec::new();
            for line in [&ns, &ew] {
                for p in without_zone(within_map(line), &drive_zone) {
                    if length(&p) > 100.0 && dist_pt_line(HALL, &p) < 8.0 {
                        mains.push(p);
                    }
                }
            }
            for m in &mains {
                roads.push((m.clone(), RoadClass::Arterial));
            }
            // radial old roads from points near the crossing
            let radial: [(f64, f64); 4] =
                [(45.0, 40.0), (135.0, 40.0), (225.0, 40.0), (315.0, 40.0)];
            if let Some(m) = mains.first() {
                let h0 = project(m, HALL).0;
                for (i, (hd, off)) in radial.iter().enumerate() {
                    let st_pt = interpolate(m, h0 + if i % 2 == 0 { *off } else { -*off }).0;
                    let ln = wander_line(
                        st_pt,
                        (hd + rng.uniform(-6.0, 6.0)).to_radians(),
                        &mut Rng::new(seed, 50 + i as u64),
                        0.18,
                        tr,
                    );
                    for p in without_zone(within_map(&ln), &drive_zone) {
                        if length(&p) > 100.0 && dist_pt_line(st_pt, &p) < 5.0 {
                            roads.push((p, RoadClass::Arterial));
                        }
                    }
                }
            }
            rail_angle = tilt * 0.5;
            let start = pt(0.0, HALL.y + 190.0 + rng.uniform(-30.0, 30.0));
            for p in within_map(&wander_line(
                start,
                rail_angle,
                &mut Rng::new(seed, 2),
                0.03,
                tr,
            )) {
                rails.push(p);
            }
        }
        Preset::LakeTown => {
            let lake = lake.expect("a lake town has a lake");
            let lake_zone = lake.ring(55.0);
            let ns_pts = |heading: f64, anchor: Pt, salt: u64, amp: f64| -> Vec<Pt> {
                let a = wander_line(
                    anchor,
                    heading + std::f64::consts::PI,
                    &mut Rng::new(seed, salt),
                    amp,
                    tr,
                );
                let b = wander_line(anchor, heading, &mut Rng::new(seed, salt + 1), amp, tr);
                let mut l: Vec<Pt> = a.iter().rev().copied().collect();
                l.extend(b.iter().skip(1).copied());
                l
            };
            let main = ns_pts(
                std::f64::consts::FRAC_PI_2 + rng.uniform(-5.0, 5.0).to_radians(),
                HALL,
                60,
                0.15,
            );
            let mains: Vec<Vec<Pt>> = without_zone(within_map(&main), &drive_zone)
                .into_iter()
                .filter(|p| length(p) > 100.0 && dist_pt_line(HALL, p) < 8.0)
                .collect();
            for m in &mains {
                roads.push((m.clone(), RoadClass::Arterial));
            }
            // an east-west road along the north bank of the lake, and one along its south bank
            for (i, dy) in [-1.0, 1.0].iter().enumerate() {
                let y0 = lake.c.y + dy * (lake.r * 1.45);
                let start = pt(0.0, y0);
                let ln = wander_line(
                    start,
                    0.0 + rng.uniform(-3.0, 3.0).to_radians(),
                    &mut Rng::new(seed, 70 + i as u64),
                    0.12,
                    tr,
                );
                for p in without_zone(without_zone(within_map(&ln), &drive_zone), &lake_zone) {
                    if length(&p) > 100.0 {
                        roads.push((p, RoadClass::Arterial));
                    }
                }
            }
            // a road on the far side of the lake
            let east_x = lake.c.x + lake.r * 1.5;
            let far = wander_line(
                pt(east_x, HWY_Y),
                std::f64::consts::FRAC_PI_2,
                &mut Rng::new(seed, 80),
                0.1,
                tr,
            );
            for p in without_zone(without_zone(within_map(&far), &drive_zone), &lake_zone) {
                if length(&p) > 100.0 {
                    roads.push((p, RoadClass::Arterial));
                }
            }
            // main street: straight from the lake's west shore to the north-south main road
            if let Some(m) = mains.first() {
                let west = lake.ring(55.0)[90];
                let target = project(m, west).2;
                if west.dist(target) > 60.0 {
                    roads.push((vec![west, target], RoadClass::Arterial));
                }
            }
            // the lakeside drive
            let mut ring = lake.ring(55.0);
            ring.pop();
            ring.push(ring[0]);
            roads.push((ring, RoadClass::Collector));
            rail_angle = (6.0 + rng.uniform(-3.0, 3.0)).to_radians();
            let start = pt(0.0, 480.0 + rng.uniform(-60.0, 100.0));
            for p in without_zone(
                within_map(&wander_line(
                    start,
                    rail_angle,
                    &mut Rng::new(seed, 2),
                    0.05,
                    tr,
                )),
                &lake_zone,
            ) {
                rails.push(p);
            }
        }
    }
    riverside_drives(&mut roads, river, preset == Preset::Winding);
    let _ = TAU;
    Skeleton {
        roads,
        rails,
        rail_angle,
    }
}

/// Arterials stop 116 m short of the river unless bridged. Every side of the river where one
/// stops gets a riverside drive, and each stopped arterial is tied into it. Winding towns always
/// have the drive on the north-facing bank.
fn riverside_drives(roads: &mut Vec<(Vec<Pt>, RoadClass)>, river: &River, always_low: bool) {
    let side_of = |e: Pt| -> Option<f64> {
        let o = e.y - river.y(e.x.clamp(0.0, S));
        if (110.0..=122.0).contains(&o) {
            Some(120.0)
        } else if (-122.0..=-110.0).contains(&o) {
            Some(-120.0)
        } else {
            None
        }
    };
    let mut sides: Vec<f64> = Vec::new();
    if always_low {
        sides.push(-120.0);
    }
    for (l, c) in roads.iter() {
        if !matches!(*c, RoadClass::Arterial | RoadClass::Collector) {
            continue;
        }
        for e in [l[0], *l.last().unwrap()] {
            if let Some(sd) = side_of(e)
                && !sides.contains(&sd)
            {
                sides.push(sd);
            }
        }
    }
    sides.sort_by(|a, b| a.total_cmp(b));
    let mut drives: Vec<(f64, Vec<Vec<Pt>>)> = Vec::new();
    for sd in sides {
        drives.push((sd, within_map(&river.line(sd))));
    }
    for (l, c) in roads.iter_mut() {
        if !matches!(*c, RoadClass::Arterial | RoadClass::Collector) {
            continue;
        }
        let n = l.len();
        for front in [true, false] {
            let e = if front { l[0] } else { l[n - 1] };
            let Some(sd) = side_of(e) else { continue };
            let Some((_, pieces)) = drives.iter().find(|(s, _)| *s == sd) else {
                continue;
            };
            let best = pieces
                .iter()
                .map(|p| project(p, e))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((_, dist, q)) = best
                && dist < 12.0
            {
                if front {
                    l.insert(0, q);
                } else {
                    l.push(q);
                }
            }
        }
    }
    for (_, pieces) in drives {
        for p in pieces {
            roads.push((p, RoadClass::Arterial));
        }
    }
}
