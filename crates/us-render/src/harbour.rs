//! What stands in the water and on the shore of a coastal town: stone breakwaters, piers, moored
//! boats, a lighthouse, sea stacks and an arch. All of it is fixed, so it is built once into the
//! ground mesh.

use crate::land::Land;
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb, Sit};
use us_gen::{Relief, Town};

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

/// A tower of rock: a broken foot, then layers one on another, each a touch narrower, and a cap.
/// `foot_h` is the height of the foot above the base of the model at `z0`.
#[allow(clippy::too_many_arguments)]
pub fn rock_tower(
    o: &mut Obj,
    centre: [f32; 2],
    r: f32,
    h: f32,
    z0: f32,
    phase: f32,
    tint: usize,
    bands: &[Rgb; 5],
    foot: Rgb,
    cap: Rgb,
    taper: f32,
    lobes: f32,
) {
    let sides = 14;
    let outline = |scale: f32| -> Vec<[f32; 2]> {
        (0..sides)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / sides as f32;
                let k = 1.0
                    + lobes * libm::sinf(2.0 * a + phase)
                    + 0.6 * lobes * libm::sinf(3.0 * a + 2.0 * phase);
                [
                    centre[0] + r * scale * k * libm::cosf(a),
                    centre[1] + r * scale * k * libm::sinf(a),
                ]
            })
            .collect()
    };
    let foot_h = (h * 0.2).clamp(4.0, 16.0);
    o.extrude(&outline(1.3), z0, z0 + foot_h, foot, foot);
    let mut z = z0 + foot_h;
    let mut k = 0;
    while z < z0 + h - 1.0 {
        let band = (6.0 + (hash(tint as u64, k as u64) % 5) as f32).min(z0 + h - z);
        let scale = 1.05 - taper * ((z - z0) / h);
        let c = bands[(k + tint) % bands.len()];
        o.extrude(&outline(scale), z, z + band, c, c);
        z += band;
        k += 1;
    }
    o.extrude(&outline(0.93), z0 + h - 1.2, z0 + h, cap, cap);
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
    let Relief::Coast(field) = t.terrain.relief() else {
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
    // sea stacks and the arch
    for (i, s) in field.stacks.iter().enumerate() {
        let mut o = Obj::new();
        let phase = (hash(t.seed, 900 + i as u64) % 628) as f32 / 100.0;
        if s.arch {
            let r = s.r as f32;
            let pr = r * 0.46;
            for k in [-1.0f32, 1.0] {
                rock_tower(
                    &mut o,
                    [0.0, k * r * 0.62],
                    pr,
                    s.h as f32,
                    -3.0,
                    phase,
                    i,
                    &BANDS,
                    STONE_FOOT,
                    [104, 142, 88],
                    0.3,
                    0.12,
                );
            }
            // the span across the top of the arch
            let (y0, y1) = (-r * 0.62, r * 0.62);
            o.bx_top(
                -pr * 0.9,
                y0,
                s.h as f32 * 0.74,
                pr * 0.8,
                y1,
                s.h as f32,
                BANDS[i % BANDS.len()],
                [104, 142, 88],
            );
        } else {
            rock_tower(
                &mut o,
                [0.0, 0.0],
                s.r as f32,
                s.h as f32,
                -3.0,
                phase,
                i,
                &BANDS,
                STONE_FOOT,
                [104, 142, 88],
                0.3,
                0.12,
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
        rock_tower(
            &mut o,
            [0.0, 0.0],
            r,
            hgt,
            0.0,
            phase,
            placed.len(),
            &bands,
            foot,
            cap,
            0.1,
            0.3,
        );
        let base = land.height(x, y);
        o.emit_on(mesh, &mut next, x, y, 0.0, 1.0, Some(Sit { land, z: base }));
    }
}
