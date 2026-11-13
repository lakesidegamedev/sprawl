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
                let ring = ring_road(r0, &mut Rng::new(seed, salt));
                for arc in without_zone(within_map(&ring), &drive_zone) {
                    for q in split_line_at(&arc, &radials) {
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
            for p in within_map(&river.line(-120.0)) {
                roads.push((p, RoadClass::Arterial));
            }
        }
    }
    let _ = TAU;
    Skeleton {
        roads,
        rails,
        rail_angle,
    }
}
