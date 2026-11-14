//! A town park that reads as one: a looping path with spokes out to the edge, a pond, benches,
//! a playground, a small pavilion and trees in groups. Built from the park's outline and the
//! town seed alone, so it is the same on every platform.

use crate::ground::{Ground, layer};
use crate::land::Land;
use crate::obj::{Obj, Rgb, Sit};
use crate::rng::Rng;
use us_gen::{Pt, pt};

pub const PATH: Rgb = [222, 208, 176];
const POND: Rgb = [84, 150, 212];
const SHORE: Rgb = [206, 196, 150];
const PLAY_SAND: Rgb = [226, 208, 150];
const WOOD: Rgb = [146, 100, 66];
const IRON: Rgb = [58, 60, 70];
const PAV_WALL: Rgb = [238, 232, 216];
const PAV_ROOF: Rgb = [170, 82, 62];

/// First outline id handed to park furniture (above every lot chunk, below the bridge ids).
const ID_BASE: u32 = 0x00C0_0000;
const IDS_PER_PARK: u32 = 0x4000;

fn in_ring(p: Pt, ring: &[Pt]) -> bool {
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

/// Metres from `c` to the edge of the park along `angle` (stops at 400).
fn reach(c: Pt, angle: f64, ring: &[Pt]) -> f64 {
    let (s, co) = (libm::sin(angle), libm::cos(angle));
    let mut d = 0.0;
    while d < 400.0 && in_ring(pt(c.x + co * d, c.y + s * d), ring) {
        d += 2.0;
    }
    d - 2.0
}

fn at(c: Pt, angle: f64, r: f64) -> Pt {
    pt(c.x + libm::cos(angle) * r, c.y + libm::sin(angle) * r)
}

/// A middle spot of the outline that has room around it, or None for a park too small to dress.
fn heart(ring: &[Pt]) -> Option<(Pt, f64)> {
    let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
    for p in ring {
        lo = pt(lo.x.min(p.x), lo.y.min(p.y));
        hi = pt(hi.x.max(p.x), hi.y.max(p.y));
    }
    let mid = pt((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
    let mut best: Option<(Pt, f64)> = None;
    // the bounding box middle, then a coarse search around it for the roomiest point
    for iy in -4i32..=4 {
        for ix in -4i32..=4 {
            let c = pt(
                mid.x + ix as f64 * (hi.x - lo.x) / 12.0,
                mid.y + iy as f64 * (hi.y - lo.y) / 12.0,
            );
            if !in_ring(c, ring) {
                continue;
            }
            let room = (0..12)
                .map(|k| reach(c, k as f64 * std::f64::consts::TAU / 12.0, ring))
                .fold(f64::MAX, f64::min);
            // prefer room, and stay near the middle on ties
            let score = room - 0.002 * c.dist(mid);
            if best.is_none_or(|b| score > b.1) {
                best = Some((c, score));
            }
        }
    }
    best.filter(|b| b.1 > 14.0)
}

fn bench() -> Obj {
    let mut o = Obj::new();
    o.bx(-0.8, -0.25, 0.0, -0.65, 0.25, 0.4, IRON);
    o.bx(0.65, -0.25, 0.0, 0.8, 0.25, 0.4, IRON);
    o.bx(-0.9, -0.3, 0.4, 0.9, 0.3, 0.5, WOOD);
    o.bx(-0.9, 0.22, 0.5, 0.9, 0.3, 0.95, WOOD);
    o
}

fn pavilion() -> Obj {
    let mut o = Obj::new();
    o.slab(-3.2, -3.2, 0.0, 3.2, 3.2, 0.25, [196, 190, 178]);
    for (x, y) in [(-2.8, -2.8), (2.8, -2.8), (-2.8, 2.8), (2.8, 2.8)] {
        o.bx(x - 0.18, y - 0.18, 0.25, x + 0.18, y + 0.18, 3.0, PAV_WALL);
    }
    o.bx(-3.3, -3.3, 3.0, 3.3, 3.3, 3.25, PAV_WALL);
    o.pyramid(0.0, 0.0, 3.25, 4.9, 1.6, PAV_ROOF, 4, 45.0);
    o.bx(-0.9, 2.1, 0.25, 0.9, 2.6, 0.7, WOOD);
    o
}

fn playground() -> Obj {
    let mut o = Obj::new();
    // a swing frame with two seats
    let red = [200, 70, 60];
    for x in [-2.0f32, 2.0] {
        o.bx(x - 0.12, -0.9, 0.0, x + 0.12, -0.6, 2.3, red);
        o.bx(x - 0.12, 0.6, 0.0, x + 0.12, 0.9, 2.3, red);
    }
    o.bx(-2.1, -0.9, 2.3, 2.1, 0.9, 2.5, red);
    for x in [-0.9f32, 0.9] {
        o.bx(x - 0.04, -0.02, 0.5, x + 0.04, 0.02, 2.3, IRON);
        o.bx(x - 0.3, -0.15, 0.4, x + 0.3, 0.15, 0.5, WOOD);
    }
    // a slide: a tower and a sloping chute
    let blue = [70, 120, 200];
    o.bx(3.6, -0.6, 0.0, 4.6, 0.4, 1.8, [230, 190, 60]);
    o.poly(
        &[
            [4.0, 0.4, 1.8],
            [4.6, 0.4, 1.8],
            [4.6, 3.4, 0.2],
            [4.0, 3.4, 0.2],
        ],
        blue,
        true,
        true,
        false,
    );
    o
}

/// Dress a park: `ring` is its outline. Adds ground markings (path, pond, play sand) to `g` and
/// 3D furniture and trees into the same mesh. Does nothing when the park is too small.
pub fn add_park(g: &mut Ground, land: &Land, ring: &[Pt], seed: u64, index: usize) {
    let Some((c, room)) = heart(ring) else {
        return;
    };
    let mut rng = Rng::new(seed ^ 0x9a2c_41d7, index as u64);
    let mut next_id = ID_BASE + (index as u32 % 64) * IDS_PER_PARK;
    let start = next_id;
    let ring_r = (room * 0.5).clamp(9.0, 42.0);
    let a0 = rng.range(0.0, std::f32::consts::TAU) as f64;

    // the pond in the middle, with a sandy rim
    let pond_r = (ring_r * 0.32).clamp(3.0, 22.0);
    let blob = |r: f64| -> Vec<Pt> {
        (0..28)
            .map(|k| {
                let a = k as f64 * std::f64::consts::TAU / 28.0;
                let wob = 1.0 + 0.12 * libm::sin(a * 3.0 + a0) + 0.06 * libm::cos(a * 5.0);
                pt(
                    c.x + libm::cos(a) * r * wob * 1.2,
                    c.y + libm::sin(a) * r * wob * 0.85,
                )
            })
            .collect()
    };
    g.polygon(&blob(pond_r + 1.4), layer::SAND, SHORE);
    g.polygon(&blob(pond_r), layer::WATER, POND);

    // the loop path and spokes out to the edge
    let ring_line: Vec<Pt> = (0..=40)
        .map(|k| at(c, k as f64 * std::f64::consts::TAU / 40.0, ring_r))
        .collect();
    g.ribbon(&ring_line, 1.4, 0.0, layer::SAND + 0.02, PATH);
    for k in 0..4 {
        let a = a0 + k as f64 * std::f64::consts::FRAC_PI_2;
        let end = reach(c, a, ring) - 1.5;
        if end - ring_r > 6.0 {
            g.ribbon(
                &[at(c, a, ring_r), at(c, a, end)],
                1.2,
                0.0,
                layer::SAND + 0.02,
                PATH,
            );
        }
    }

    let mut taken: Vec<(Pt, f64)> = vec![(c, pond_r * 1.3 + 4.0)];
    let put = |g: &mut Ground, obj: &Obj, p: Pt, theta: f32, id: &mut u32| {
        let z = land.height(p.x as f32, p.y as f32);
        obj.emit_on(
            &mut g.mesh,
            id,
            p.x as f32,
            p.y as f32,
            theta,
            1.0,
            Some(Sit { land, z }),
        );
    };

    // benches on the inner side of the loop, facing the pond
    let b = bench();
    let n_bench = ((ring_r / 6.0) as usize).clamp(3, 10);
    for k in 0..n_bench {
        let a = a0 + (k as f64 + 0.5) * std::f64::consts::TAU / n_bench as f64;
        // leave the spokes clear
        let spoke = ((a - a0) / std::f64::consts::FRAC_PI_2).fract().abs();
        if !(0.18..=0.82).contains(&spoke) {
            continue;
        }
        let p = at(c, a, ring_r - 2.4);
        put(
            g,
            &b,
            p,
            (a - std::f64::consts::FRAC_PI_2) as f32,
            &mut next_id,
        );
        taken.push((p, 2.0));
    }

    // a pavilion on the loop and a playground beyond it, when the park is roomy
    if ring_r >= 22.0 {
        let a = a0 + std::f64::consts::FRAC_PI_4 + std::f64::consts::PI;
        let p = at(c, a, ring_r * 0.74);
        if p.dist(c) > pond_r * 1.4 + 6.0 {
            put(g, &pavilion(), p, 0.0, &mut next_id);
            taken.push((p, 6.0));
        }
    }
    let room_out = room - ring_r;
    if room_out >= 14.0 {
        let a = a0 + std::f64::consts::FRAC_PI_4;
        let p = at(c, a, ring_r + room_out * 0.5);
        if in_ring(p, ring) {
            g.rect(
                p.x as f32 - 6.0,
                p.y as f32 - 5.0,
                p.x as f32 + 8.0,
                p.y as f32 + 6.0,
                layer::SAND + 0.01,
                PLAY_SAND,
            );
            put(g, &playground(), p, 0.0, &mut next_id);
            taken.push((p, 9.0));
        }
    }

    // trees in groups, away from the path and the furniture
    let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
    for p in ring {
        lo = pt(lo.x.min(p.x), lo.y.min(p.y));
        hi = pt(hi.x.max(p.x), hi.y.max(p.y));
    }
    let area = (hi.x - lo.x) * (hi.y - lo.y);
    let groups = ((area / 1200.0) as usize).clamp(3, 70);
    let green = [crate::kit::TREE, crate::kit::TREE_D];
    let clear = |p: Pt, taken: &[(Pt, f64)]| -> bool {
        if !in_ring(p, ring) {
            return false;
        }
        let d = p.dist(c);
        // off the loop path
        if (d - ring_r).abs() < 3.4 {
            return false;
        }
        // off the spokes
        for k in 0..4 {
            let a = a0 + k as f64 * std::f64::consts::FRAC_PI_2;
            let (dx, dy) = (p.x - c.x, p.y - c.y);
            let along = dx * libm::cos(a) + dy * libm::sin(a);
            let side = (-dx * libm::sin(a) + dy * libm::cos(a)).abs();
            if along > ring_r - 2.0 && side < 3.2 {
                return false;
            }
        }
        taken.iter().all(|(q, r)| p.dist(*q) > *r)
    };
    for _ in 0..groups {
        let centre = pt(
            lo.x + rng.f32() as f64 * (hi.x - lo.x),
            lo.y + rng.f32() as f64 * (hi.y - lo.y),
        );
        let n = 4 + rng.below(5);
        for _ in 0..n {
            let p = pt(
                centre.x + rng.range(-7.0, 7.0) as f64,
                centre.y + rng.range(-7.0, 7.0) as f64,
            );
            if !clear(p, &taken) || next_id >= start + IDS_PER_PARK - 64 {
                continue;
            }
            let (rad, h) = (rng.range(1.9, 3.0), rng.range(5.5, 9.0));
            let col = crate::kit::tree_variant(rng.pick(&green), p.x as f32, p.y as f32);
            let tree = crate::kit::tree(rad, h, col);
            put(g, &tree, p, 0.0, &mut next_id);
            taken.push((p, 3.0));
        }
    }
}
