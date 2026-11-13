//! Stage one: the roads and rails that exist before the town does.

use crate::geom::{
    Pt, clip_line, dedup, dist_pt_line, dist_ring_line, interpolate, length, project, pt,
    split_line_at,
};
use crate::rng::Rng;
use crate::world::{HALL, HWY_Y, River, S, TAU, ln_angle, map_ring, ring_road, wander_line};
use crate::{Preset, RoadClass};

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

pub fn build(preset: Preset, seed: u64, river: &River) -> Skeleton {
    let drive_zone = river.band(-116.0, 116.0);
    let mut roads: Vec<(Vec<Pt>, RoadClass)> = vec![(highway(), RoadClass::Highway)];
    let mut rails: Vec<Vec<Pt>> = Vec::new();
    let mut rng = Rng::new(seed, 0x5ce1);
    let rail_angle;
    match preset {
        Preset::Grid => {
            let bridged = if rng.chance(0.5) { 1 } else { -1 };
            for k in -2..=2i32 {
                let x = HALL.x + k as f64 * 805.0;
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
            for j in -1..4i32 {
                let y = HALL.y + j as f64 * 805.0;
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
                ),
                wander_line(
                    HALL,
                    (-48.0 + rng.uniform(-8.0, 8.0)).to_radians(),
                    &mut Rng::new(seed, 6),
                    0.10,
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
            );
            let north = wander_line(
                HALL,
                (80.0 + rng.uniform(-6.0, 6.0)).to_radians(),
                &mut Rng::new(seed, 24),
                0.3,
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
            )) {
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
        if *c != RoadClass::Arterial {
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
        if *c != RoadClass::Arterial {
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
