//! What stands in the water and on the shore of a coastal town: stone breakwaters, piers, moored
//! boats, a lighthouse, sea stacks and an arch. All of it is fixed, so it is built once into the
//! ground mesh.

use crate::land::Land;
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb, Sit};
use us_gen::{RockCoast, Town};

/// Outline ids of these parts start here (above every chunk's ids, below the bridge ids).
const ID_BASE: u32 = 0x00D0_0000;

const STONE: Rgb = [146, 144, 140];
const STONE_TOP: Rgb = [176, 172, 164];
const STONE_FOOT: Rgb = [112, 100, 92];
const WOOD: Rgb = [150, 112, 76];
const WOOD_DARK: Rgb = [104, 76, 54];
const BANDS: [Rgb; 5] = [
    [152, 124, 98],
    [180, 150, 112],
    [122, 102, 86],
    [194, 166, 126],
    [142, 116, 94],
];
const HULLS: [Rgb; 6] = [
    [236, 236, 232],
    [196, 58, 52],
    [40, 92, 160],
    [226, 190, 60],
    [60, 120, 96],
    [232, 232, 226],
];

fn hash(a: u64, b: u64) -> u64 {
    crate::rng::Rng::new(a, b).next_u64()
}

/// Shape of one pillar of rock.
#[derive(Clone, Copy)]
pub struct Pillar {
    pub centre: [f32; 2],
    /// Mean radius at the foot of the wall, metres.
    pub r: f32,
    pub h: f32,
    pub z0: f32,
    pub phase: f32,
    pub tint: usize,
    /// How far the top leans from the foot, metres, in x and y.
    pub lean: [f32; 2],
    /// Share of the radius lost between foot and top (0 sheer, 0.5 a spike).
    pub taper: f32,
    /// Strength of the lobes of the outline.
    pub lobes: f32,
    /// Strength of the eroded waists and bulges up the shaft.
    pub waist: f32,
    /// Radius of the talus skirt at the foot as a multiple of `r` (1 for none).
    pub skirt: f32,
    /// Squash of the plan: 1 round, below 1 a long ridge.
    pub squash: f32,
    /// Thickness of the thinnest band of rock, metres.
    pub band: f32,
}

const RING: usize = 28;

/// A ring of the pillar at height fraction `u` (0 foot, 1 top).
fn pillar_ring(p: &Pillar, u: f32, scale: f32) -> Vec<[f32; 2]> {
    let lean = u * u * (3.0 - 2.0 * u);
    // an eroded shaft: a waist or two and a bulge, differing between pillars
    let shaft = 1.0 - p.taper * u.powf(0.8)
        + p.waist
            * (0.55 * libm::sinf(5.0 * u + p.phase) + 0.3 * libm::sinf(11.0 * u + 2.0 * p.phase));
    let c = [
        p.centre[0] + p.lean[0] * lean,
        p.centre[1] + p.lean[1] * lean,
    ];
    (0..RING)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / RING as f32;
            let k = 1.0
                + p.lobes
                    * (libm::sinf(2.0 * a + p.phase + 0.7 * u)
                        + 0.6 * libm::sinf(3.0 * a + 2.0 * p.phase - 1.1 * u)
                        + 0.35 * libm::sinf(5.0 * a + 3.0 * p.phase + 2.0 * u));
            let rr = p.r * scale * shaft.max(0.25) * k;
            [
                c[0] + rr * libm::cosf(a),
                c[1] + rr * p.squash * libm::sinf(a),
            ]
        })
        .collect()
}

fn darker(c: Rgb, k: f32) -> Rgb {
    [
        (c[0] as f32 * k) as u8,
        (c[1] as f32 * k) as u8,
        (c[2] as f32 * k) as u8,
    ]
}

/// A pillar of rock: a broken skirt of talus at the foot, then a wall in thick bands of different
/// tone, each ending in a thin dark ledge, with an irregular outline that tapers, bulges and may
/// lean. The top is a rounded cap of the cap colour.
pub fn rock_pillar(o: &mut Obj, p: &Pillar, bands: &[Rgb; 5], foot: Rgb, cap: Rgb) {
    let z0 = p.z0;
    let top = z0 + p.h;
    // talus skirt
    let skirt_h = (p.h * 0.16).clamp(3.0, 22.0);
    if p.skirt > 1.0 {
        let lo = pillar_ring(p, 0.0, p.skirt);
        let hi = pillar_ring(p, skirt_h / p.h, 1.0);
        o.loft(&lo, z0, &hi, z0 + skirt_h, foot, None);
    }
    let mut z = z0 + skirt_h * if p.skirt > 1.0 { 1.0 } else { 0.0 };
    let mut k = 0u64;
    while z < top - 2.0 {
        let step = (p.band * (1.0 + 0.15 * (hash(p.tint as u64 + 31, k) % 7) as f32)).min(top - z);
        let (u0, u1) = ((z - z0) / p.h, (z + step - z0) / p.h);
        let c = bands[(hash(p.tint as u64, k + 5) % bands.len() as u64) as usize];
        // a stratum, then the dark ledge where it ends. Hard and soft layers weather back by
        // different amounts, so the width changes from one stratum to the next
        let jit = |k: u64| {
            1.0 + p.waist * 0.9 * ((hash(p.tint as u64 + 77, k) % 1000) as f32 / 500.0 - 1.0)
        };
        let lo = pillar_ring(p, u0, jit(k));
        let hi = pillar_ring(p, u1, jit(k + 1));
        o.loft(&lo, z, &hi, z + step - 0.7, c, None);
        let lip = pillar_ring(p, u1, 1.03 * jit(k + 1));
        o.loft(&hi, z + step - 0.7, &lip, z + step, darker(c, 0.62), None);
        z += step;
        k += 1;
    }
    // the cap: a rounded top
    let jt = 1.0 + p.waist * 0.9 * ((hash(p.tint as u64 + 77, k) % 1000) as f32 / 500.0 - 1.0);
    let rim = pillar_ring(p, 1.0, jt);
    let crown = pillar_ring(p, 1.0, 0.62 * jt);
    let dome = (p.r * 0.25).min(p.h * 0.1);
    o.loft(&rim, top - 0.5, &crown, top + dome, cap, Some(cap));
}

/// A natural arch: one banded sweep of rock that rises from the sea as a leg, bends over a rounded
/// opening and comes down as the other leg, so the span is continuous with both. The section is a
/// lumpy ellipse that is thick at the feet and thins to the crown, and the strata follow the
/// sweep with a dark ledge between them.
fn rock_arch(o: &mut Obj, r: f32, h: f32, phase: f32, tint: usize, bands: &[Rgb; 5]) {
    let foot = -3.0f32;
    let c = r * 0.66;
    let zs = foot + (h - foot) * 0.4;
    let rise = (h * 0.86 - zs - h * 0.1).max(h * 0.2);
    let leg_th = r * 0.3;
    let crown_th = (h * 0.1).clamp(r * 0.14, r * 0.26);
    let wx0 = r * 0.5;
    // the centre line, dense, then cut at equal steps
    let mut dense: Vec<[f32; 2]> = Vec::new();
    let nl = 40;
    for k in 0..=nl {
        dense.push([-c, foot + (zs - foot) * k as f32 / nl as f32]);
    }
    let na = 120;
    for k in 1..na {
        let t = std::f32::consts::PI * k as f32 / na as f32;
        dense.push([-c * libm::cosf(t), zs + rise * libm::sinf(t)]);
    }
    for k in 0..=nl {
        dense.push([c, zs + (foot - zs) * k as f32 / nl as f32]);
    }
    let mut cum = vec![0.0f32];
    for w in dense.windows(2) {
        let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        cum.push(cum.last().unwrap() + d);
    }
    let total = *cum.last().unwrap();
    let band = 8.0f32;
    let n = (total / (band / 3.0)).ceil() as usize;
    let at = |len: f32| -> ([f32; 2], [f32; 2]) {
        let len = len.clamp(0.0, total);
        let i = cum.partition_point(|&x| x < len).clamp(1, dense.len() - 1);
        let f = (len - cum[i - 1]) / (cum[i] - cum[i - 1]).max(1e-6);
        let (a, b) = (dense[i - 1], dense[i]);
        let p = [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
        let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt().max(1e-6);
        (p, [(b[0] - a[0]) / l, (b[1] - a[1]) / l])
    };
    const SIDES: usize = 14;
    let ring_at = |len: f32, swell: f32| -> Vec<[f32; 3]> {
        let (p, t) = at(len);
        let u = len / total;
        // 0 at the feet, 1 at the crown
        let mid = libm::sinf(std::f32::consts::PI * u);
        let flare = 1.0 + 0.5 * (1.0 - ((p[1] - foot) / (h * 0.16)).clamp(0.0, 1.0));
        let th = (leg_th + (crown_th - leg_th) * mid.powf(1.4)) * flare;
        let wx = (wx0 + (wx0 * 0.6 - wx0) * mid.powf(1.4)) * flare;
        let erode = 1.0
            + 0.16 * libm::sinf(u * 23.0 + phase)
            + 0.1 * libm::sinf(u * 51.0 + 2.0 * phase);
        let nrm = [-t[1], t[0]];
        (0..SIDES)
            .map(|k| {
                let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
                let lump = 1.0
                    + 0.16 * libm::sinf(2.0 * a + phase + 6.0 * u)
                    + 0.09 * libm::sinf(3.0 * a + 2.0 * phase - 9.0 * u);
                let (ca, sa) = (libm::cosf(a), libm::sinf(a));
                let (e1, e2) = (wx * ca * lump * erode * swell, th * sa * lump * erode * swell);
                [e1, p[0] + nrm[0] * e2, p[1] + nrm[1] * e2]
            })
            .collect()
    };
    let mut prev = ring_at(0.0, 1.0);
    for k in 1..=n {
        let len = total * k as f32 / n as f32;
        // a thin ledge at the end of each stratum, then the next stratum's tone
        let seg = (len / band) as u64;
        let ledge = (len % band) < band / 3.0;
        let base = bands[(hash(tint as u64 + 31, seg) % bands.len() as u64) as usize];
        let col = if ledge { darker(base, 0.62) } else { base };
        let col = if (len / total - 0.5).abs() > 0.5 - 0.06 { STONE_FOOT } else { col };
        let cur = ring_at(len, if ledge { 1.04 } else { 1.0 });
        // one solid per slice, so each face is turned away from the middle of its own slice
        let m = (prev.len() + cur.len()) as f32;
        let mid = [0, 1, 2].map(|d| prev.iter().chain(cur.iter()).map(|p| p[d]).sum::<f32>() / m);
        let sol = o.new_solid(mid);
        o.solids[sol as usize].merged = true;
        for i in 0..SIDES {
            let j = (i + 1) % SIDES;
            // ring points are (x, y, z) with x along the ridge: the pair is (e1, y, z)
            let q = |r: &[[f32; 3]], k: usize| [r[k][0], r[k][1], r[k][2]];
            o.face(sol, &[q(&prev, i), q(&prev, j), q(&cur, j), q(&cur, i)], col, false);
        }
        prev = cur;
    }
    // a little grass on the crown
    let (cp, _) = at(total * 0.5);
    let top = cp[1] + crown_th * 0.9;
    let rim: Vec<[f32; 2]> = (0..10)
        .map(|k| {
            let a = std::f32::consts::TAU * k as f32 / 10.0;
            [wx0 * 0.4 * libm::cosf(a), c * 0.34 * libm::sinf(a)]
        })
        .collect();
    let crown: Vec<[f32; 2]> = rim.iter().map(|p| [p[0] * 0.7, p[1] * 0.7]).collect();
    o.loft(&rim, top - 1.4, &crown, top + 1.2, [104, 142, 88], Some([104, 142, 88]));
}

fn boat(size: u8, seed: u64) -> Obj {
    let mut o = Obj::new();
    let hull = HULLS[(hash(seed, 1) % HULLS.len() as u64) as usize];
    let (l, w, hh) = match size {
        0 => (14.0f32, 5.0f32, 2.2f32),
        1 => (22.0, 7.4, 3.2),
        _ => (34.0, 10.0, 4.6),
    };
    let bow = l * 0.28;
    let pts = [
        [-l / 2.0, -w / 2.0],
        [l / 2.0 - bow, -w / 2.0],
        [l / 2.0, 0.0],
        [l / 2.0 - bow, w / 2.0],
        [-l / 2.0, w / 2.0],
    ];
    o.extrude(&pts, -0.4, hh, hull, [150, 112, 80]);
    // a cabin aft, and a mast on the larger ones
    o.bx_top(
        -l * 0.4,
        -w * 0.3,
        hh,
        -l * 0.05,
        w * 0.3,
        hh + 0.4 + 0.35 * size as f32 + 0.6,
        [240, 240, 236],
        [200, 200, 196],
    );
    if size >= 1 {
        o.bx(
            l * 0.12,
            -0.12,
            hh,
            l * 0.12 + 0.24,
            0.12,
            hh + 6.0 + 3.0 * size as f32,
            [210, 210, 206],
        );
    }
    o
}

/// Everything fixed in and by the water of a coastal town, into `mesh`.
pub fn add(mesh: &mut Mesh, land: &Land, t: &Town) {
    let RockCoast::Coast(field) = t.terrain.rock_coast() else {
        return;
    };
    let mut next = ID_BASE;
    let h = field.harbour();
    // breakwaters: a wide low foot of rough stone under a narrower wall with a flat top
    for line in &h.breakwaters {
        for w in line.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.dist(b) as f32;
            if len < 1.0 {
                continue;
            }
            let ang = libm::atan2f((b.y - a.y) as f32, (b.x - a.x) as f32);
            let mut o = Obj::new();
            o.bx(-5.0, -15.0, -3.0, len + 5.0, 15.0, 1.6, STONE_FOOT);
            o.bx_top(-3.0, -8.0, -3.0, len + 3.0, 8.0, 6.0, STONE, STONE_TOP);
            o.emit_on(mesh, &mut next, a.x as f32, a.y as f32, ang, 1.0, None);
        }
    }
    // piers: a deck on posts
    for (root, tip) in &h.piers {
        let len = root.dist(*tip) as f32;
        let ang = libm::atan2f((tip.y - root.y) as f32, (tip.x - root.x) as f32);
        let mut o = Obj::new();
        o.bx_top(0.0, -4.0, 1.8, len, 4.0, 2.4, WOOD, [172, 132, 92]);
        let mut x = 2.0;
        while x < len {
            for y in [-3.6f32, 3.6] {
                o.bx(x - 0.5, y - 0.5, -2.0, x + 0.5, y + 0.5, 1.8, WOOD_DARK);
            }
            x += 9.0;
        }
        o.emit_on(
            mesh,
            &mut next,
            root.x as f32,
            root.y as f32,
            ang,
            1.0,
            None,
        );
    }
    // boats
    for (i, m) in h.boats.iter().enumerate() {
        let o = boat(m.size, hash(t.seed, i as u64));
        o.emit_on(
            mesh,
            &mut next,
            m.at.x as f32,
            m.at.y as f32,
            m.heading as f32,
            1.0,
            None,
        );
    }
    // lighthouse on the headland
    let lp = h.lighthouse;
    let base = land.height(lp.x as f32, lp.y as f32);
    let mut o = Obj::new();
    o.prism(
        0.0,
        0.0,
        0.0,
        3.0,
        6.2,
        [120, 118, 116],
        12,
        Some([140, 138, 134]),
    );
    o.prism(
        0.0,
        0.0,
        3.0,
        12.0,
        4.4,
        [240, 240, 234],
        12,
        Some([240, 240, 234]),
    );
    o.prism(
        0.0,
        0.0,
        12.0,
        16.0,
        4.1,
        [196, 60, 52],
        12,
        Some([196, 60, 52]),
    );
    o.prism(
        0.0,
        0.0,
        16.0,
        22.0,
        3.7,
        [240, 240, 234],
        12,
        Some([240, 240, 234]),
    );
    o.prism(
        0.0,
        0.0,
        22.0,
        23.0,
        5.0,
        [60, 60, 66],
        12,
        Some([80, 80, 86]),
    );
    o.prism(
        0.0,
        0.0,
        23.0,
        26.0,
        2.2,
        [250, 232, 150],
        12,
        Some([250, 232, 150]),
    );
    o.pyramid(0.0, 0.0, 26.0, 3.4, 3.4, [196, 60, 52], 12, 0.0);
    o.emit_on(
        mesh,
        &mut next,
        lp.x as f32,
        lp.y as f32,
        0.0,
        1.8,
        Some(Sit { land, z: base }),
    );
    // sea stacks and the arch: pillars of different sizes, some leaning, narrowing and eroded
    for (i, s) in field.stacks.iter().enumerate() {
        let mut o = Obj::new();
        let rnd = |k: u64| (hash(t.seed, 900 + 17 * i as u64 + k) % 10_000) as f32 / 10_000.0;
        let phase = rnd(0) * std::f32::consts::TAU;
        let h = s.h as f32;
        let lean_dir = rnd(1) * std::f32::consts::TAU;
        let lean_len = h * (0.05 + 0.25 * rnd(2));
        let pillar = |centre: [f32; 2], r: f32, h: f32, lean: [f32; 2], k: usize| Pillar {
            centre,
            r,
            h,
            z0: -3.0,
            phase: phase + k as f32 * 1.9,
            tint: i * 3 + k,
            lean,
            taper: 0.3 + 0.32 * rnd(3),
            lobes: 0.2,
            waist: 0.22 + 0.14 * rnd(4),
            skirt: 1.45,
            squash: 0.55 + 0.45 * rnd(5),
            band: 5.0,
        };
        if s.arch {
            rock_arch(&mut o, s.r as f32, h, phase, i, &BANDS);
        } else {
            let lean = [
                lean_len * libm::cosf(lean_dir),
                lean_len * libm::sinf(lean_dir),
            ];
            rock_pillar(
                &mut o,
                &pillar([0.0, 0.0], s.r as f32, h, lean, 0),
                &BANDS,
                STONE_FOOT,
                [104, 142, 88],
            );
        }
        o.emit_on(mesh, &mut next, s.c.x as f32, s.c.y as f32, 0.0, 1.0, None);
    }
}

/// Distant mesas and buttes on the horizon past the edge of a red rock town's map, so the desert
/// does not stop at the last road. They stand on the held level of the land at the edge.
pub fn add_horizon(mesh: &mut Mesh, land: &Land, t: &Town) {
    let bands: [Rgb; 5] = [
        [176, 84, 56],
        [214, 124, 64],
        [192, 98, 64],
        [226, 160, 98],
        [150, 66, 48],
    ];
    let foot: Rgb = [186, 118, 82];
    let cap: Rgb = [198, 126, 84];
    let size = t.size as f32;
    let mut next = ID_BASE + 0x8000;
    let mut rng = crate::rng::Rng::new(t.seed, 0x4012);
    let mut placed: Vec<(f32, f32)> = Vec::new();
    let mut tries = 0;
    while placed.len() < 18 && tries < 200 {
        tries += 1;
        // a point in the band 350 to 1400 m outside the edge, all round
        let side = rng.below(4);
        let along = rng.range(-900.0, size + 900.0);
        let out = rng.range(350.0, 1400.0);
        let (x, y) = match side {
            0 => (-out, along),
            1 => (size + out, along),
            2 => (along, -out),
            _ => (along, size + out),
        };
        if placed.iter().any(|p| (p.0 - x).hypot(p.1 - y) < 330.0) {
            continue;
        }
        placed.push((x, y));
        let wide = rng.chance(0.4);
        let (r, hgt) = if wide {
            (rng.range(140.0, 260.0), rng.range(70.0, 150.0))
        } else {
            (rng.range(35.0, 65.0), rng.range(120.0, 230.0))
        };
        let phase = rng.range(0.0, std::f32::consts::TAU);
        let mut o = Obj::new();
        // like the near formations: a sheer banded wall with a lobed outline on a talus skirt
        let pillar = Pillar {
            centre: [0.0, 0.0],
            r,
            h: hgt,
            z0: 0.0,
            phase,
            tint: placed.len(),
            lean: [0.0, 0.0],
            taper: if wide { 0.03 } else { 0.12 },
            lobes: if wide { 0.2 } else { 0.1 },
            waist: if wide { 0.02 } else { 0.05 },
            skirt: if wide { 1.3 } else { 1.7 },
            band: if wide { 14.0 } else { 18.0 },
            squash: if wide {
                0.55 + 0.4 * rng.range(0.0, 1.0)
            } else {
                1.0
            },
        };
        rock_pillar(&mut o, &pillar, &bands, foot, cap);
        let base = land.height(x, y);
        o.emit_on(mesh, &mut next, x, y, 0.0, 1.0, Some(Sit { land, z: base }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    #[test]
    fn the_sea_lies_at_the_foot_of_the_cliffs_and_stacks_stand_out_of_it() {
        for seed in 1..=4 {
            let t = generate(Preset::Coast, seed);
            let land = Land::from_town(&t);
            let level = land.water_level(&t.water[0]);
            assert!(level < 6.0, "seed {seed}: the sea stands {level} m up");
            let mut mesh = Mesh::default();
            add(&mut mesh, &land, &t);
            let RockCoast::Coast(c) = t.terrain.rock_coast() else {
                panic!("a coast");
            };
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.pos.iter().all(|c| c.is_finite())),
                "seed {seed}: a vertex is not finite"
            );
            for s in &c.stacks {
                let top = mesh
                    .vertices
                    .iter()
                    .filter(|v| {
                        (v.pos[0] as f64 - s.c.x).abs() < s.r * 2.0
                            && (v.pos[1] as f64 - s.c.y).abs() < s.r * 2.0
                    })
                    .map(|v| v.pos[2])
                    .fold(f32::MIN, f32::max);
                assert!(
                    top as f64 > s.h * 0.85 - 3.0,
                    "seed {seed}: stack of {} m tops out at {top}",
                    s.h
                );
                assert!(
                    top > level + 20.0,
                    "seed {seed}: a stack hardly clears the water"
                );
            }
        }
    }
}
