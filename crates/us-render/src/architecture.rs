//! New neighborhood and skyline models. Every family has twelve seeded plans.
use crate::obj::{Finish, Obj, Rgb};
use crate::parts::{GLASS, GLASS_D};
use crate::rng::Rng;
use us_gen::LotKind;

pub const FAMILIES: [&str; 13] = [
    "Single-wide homes",
    "Double-wide homes",
    "Courtyard bungalows",
    "Terraced rowhouses",
    "Warehouse lofts",
    "Office slabs",
    "Art Deco towers",
    "Stepped glass towers",
    "Twin towers",
    "Curved towers",
    "Landmark spires",
    "Offset atrium towers",
    "Terraced residential towers",
];
pub const PLANS: usize = 12;
const CONCRETE: Rgb = [176, 173, 164];
const ROOF: Rgb = [79, 91, 103];

pub(crate) fn lot(kind: LotKind, level: u8, density: f32, rng: &mut Rng) -> Option<Obj> {
    let family = match kind {
        LotKind::House if level <= 2 && density < 0.4 && rng.below(4) == 0 => {
            rng.below(if level == 1 { 2 } else { 3 })
        }
        LotKind::Row if rng.below(4) == 0 => 3,
        LotKind::Workshop | LotKind::Warehouse if rng.below(4) == 0 => 4,
        LotKind::Office | LotKind::Apartments if level >= 4 && density >= 0.78 => {
            if level == 5 && density >= 0.97 && kind == LotKind::Office && rng.below(20) == 0 {
                10
            } else if kind == LotKind::Office {
                [5, 6, 7, 9, 11][rng.below(5)]
            } else {
                [5, 7, 8, 9, 12][rng.below(5)]
            }
        }
        _ => return None,
    };
    let plan = rng.below(PLANS);
    Some(build(family, plan, level))
}

fn rectangle(w: f32, d: f32) -> [[f32; 2]; 4] {
    [[-w, -d], [w, -d], [w, d], [-w, d]]
}

fn bevel(w: f32, d: f32, c: f32) -> [[f32; 2]; 8] {
    [
        [-w + c, -d],
        [w - c, -d],
        [w, -d + c],
        [w, d - c],
        [w - c, d],
        [-w + c, d],
        [-w, d - c],
        [-w, -d + c],
    ]
}

fn floors(obj: &mut Obj, edges: &[[f32; 2]], start: f32, count: usize, storey: f32, bays: usize) {
    for (a, b) in edges.iter().zip(edges.iter().cycle().skip(1)) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let width = dx.hypot(dy);
        let n = if width < 4.0 { 1 } else { bays };
        for floor in 0..count {
            let height = start + floor as f32 * storey;
            let glass = if height < 0.01 { GLASS_D } else { GLASS };
            for bay in 0..n {
                let at = |x: f32| {
                    let t = (bay as f32 + x) / n as f32;
                    [
                        a[0] + dx * t + dy / width * 0.018,
                        a[1] + dy * t - dx / width * 0.018,
                    ]
                };
                obj.wall_quad(
                    at(0.11),
                    at(0.89),
                    height + 0.8,
                    height + storey - 0.42,
                    glass,
                );
            }
        }
    }
}

fn finish_extrusion(
    obj: &mut Obj,
    ring: &[[f32; 2]],
    base: f32,
    top: f32,
    wall: Rgb,
    finish: Finish,
) {
    let first = obj.polys.len();
    obj.extrude(ring, base, top, wall, ROOF);
    for face in &mut obj.polys[first..first + ring.len()] {
        face.finish = finish;
    }
    obj.polys[first + ring.len()].finish = Finish::Membrane;
}

/// Give broad exterior faces in the existing low-rise kit a material finish.
pub(crate) fn finish_low_rise(obj: &mut Obj, kind: LotKind) {
    let homes = matches!(kind, LotKind::House | LotKind::Row | LotKind::Apartments);
    let factory = matches!(kind, LotKind::Workshop | LotKind::Warehouse);
    if !(homes || factory || matches!(kind, LotKind::Shop | LotKind::Office)) {
        return;
    }
    for face in &mut obj.polys {
        if face.finish != Finish::Plain || face.flat || !face.shadow || face.solid.is_none() {
            continue;
        }
        let n = crate::obj::newell(&face.pts);
        let first = face.pts[0];
        let (mut lo, mut hi) = (glam::Vec3::from(first), glam::Vec3::from(first));
        for p in &face.pts[1..] {
            lo = lo.min(glam::Vec3::from(*p));
            hi = hi.max(glam::Vec3::from(*p));
        }
        let size = hi - lo;
        if n[2] > 0.45 && lo.z >= 2.1 && size.x * size.y > 7.0 {
            face.finish = if factory {
                Finish::StandingSeam
            } else if homes && (face.roof || n[2] < 0.96) {
                Finish::Shingle
            } else {
                Finish::Membrane
            };
        } else if n[2].abs() < 0.10 && size.z >= 2.3 && size.truncate().length() > 2.7 {
            let [r, g, b] = face.color;
            face.finish = if r.saturating_sub(g) > 21 && g.saturating_sub(b) > 5 {
                Finish::Brick
            } else if homes && kind == LotKind::House && r.abs_diff(g) < 36 {
                Finish::Siding
            } else {
                Finish::Stone
            };
        }
    }
}

fn small(family: usize, plan: usize, wall: Rgb) -> Obj {
    let mut obj = Obj::new();
    let w = [5.5, 6.5, 5.9, 8.1, 9.2][family] + (plan % 4) as f32 * 0.66;
    let d = [1.9, 3.4, 4.7, 4.0, 6.1][family] + (plan / 4) as f32 * 0.74;
    let base = if family < 2 { 0.48 } else { 0.12 };
    let count = [1, 1, 1, 2, 3][family];
    let storey = if family < 2 { 2.65 } else { 3.1 };
    let top = base + count as f32 * storey;
    obj.bx(
        -w + 0.25,
        -d + 0.18,
        0.0,
        w - 0.25,
        d - 0.18,
        base,
        CONCRETE,
    );
    let ring = rectangle(w, d);
    let finish = match family {
        0 | 1 => Finish::Siding,
        3 | 4 => Finish::Brick,
        _ => Finish::Stone,
    };
    finish_extrusion(&mut obj, &ring, base, top, wall, finish);
    floors(&mut obj, &ring, base, count, storey, 3 + plan % 4);
    let roof_start = obj.polys.len();
    match plan / 4 {
        0 => {
            obj.gable(
                -w,
                -d,
                w,
                d,
                top,
                0.74 + family as f32 * 0.28,
                ROOF,
                wall,
                true,
                0.20,
            );
        }
        1 => {
            obj.hip(-w, -d, w, d, top, 0.75 + family as f32 * 0.19, ROOF, 0.28);
        }
        _ => {
            obj.bx(
                -w - 0.14,
                -d - 0.14,
                top,
                w + 0.14,
                d + 0.14,
                top + 0.32,
                ROOF,
            );
        }
    }
    for face in &mut obj.polys[roof_start..] {
        if crate::obj::newell(&face.pts)[2] > 0.42 {
            face.finish = match (family, plan / 4) {
                (_, 2) => Finish::Membrane,
                (0 | 1 | 4, _) => Finish::StandingSeam,
                _ => Finish::Shingle,
            };
        }
    }
    if family == 2 || family == 3 {
        let x = if plan.is_multiple_of(2) {
            w * 0.55
        } else {
            -w * 0.55
        };
        obj.bx(x - 1.6, -d - 1.65, 0.0, x + 1.6, -d + 0.2, top - 0.25, wall);
    }
    let entry = (plan % 4) as f32 * 1.4 - 2.1;
    obj.bx(
        entry - 1.1,
        -d - 1.67,
        0.0,
        entry + 1.1,
        -d,
        base + 0.10,
        CONCRETE,
    );
    obj.win_s(
        entry - 0.51,
        entry + 0.51,
        base,
        base + 2.05,
        -d - 0.04,
        [45, 52, 65],
    );
    obj.bx(
        entry - 1.5,
        -d - 1.9,
        base + 2.45,
        entry + 1.5,
        -d,
        base + 2.62,
        ROOF,
    );
    if family < 2 {
        for x in [-w + 1.0, w - 1.0] {
            obj.bx(
                x - 0.23,
                -d - 0.10,
                0.0,
                x + 0.23,
                -d + 0.4,
                base,
                [80, 84, 84],
            );
        }
    }
    obj
}

fn tower(family: usize, plan: usize, level: u8, wall: Rgb) -> Obj {
    let mut obj = Obj::new();
    let w = 8.7 + (plan % 4) as f32 * 0.85;
    let d = 7.0 + (plan / 4) as f32;
    let storey = if family == 6 { 3.9 } else { 3.5 };
    let count = if level < 5 {
        9 + plan % 4 + usize::from(family != 5)
    } else {
        [28, 36, 41, 34, 43, 76, 49, 42][family - 5] + if family == 5 { 0 } else { plan % 4 }
    };
    let base_depth = d + 1.15 + 0.30 * (plan % 3) as f32;
    let podium = rectangle(w + 1.2, base_depth);
    let street_wall = [CONCRETE, [145, 113, 99], [142, 152, 158]][plan % 3];
    finish_extrusion(
        &mut obj,
        &podium,
        0.0,
        storey * 2.0,
        street_wall,
        Finish::Loft,
    );
    let entry = (plan % 3) as f32 * 1.2 - 1.2;
    obj.bx(
        entry - 2.5,
        -base_depth - 1.2,
        storey * 0.91,
        entry + 2.5,
        -base_depth,
        storey * 0.98,
        ROOF,
    );
    obj.wall_quad(
        [entry - 1.2, -base_depth - 0.07],
        [entry + 1.2, -base_depth - 0.07],
        0.0,
        storey * 0.82,
        GLASS_D,
    );
    let facade = match family {
        5 | 11 => Finish::RibbedGlass,
        6 | 10 => Finish::Deco,
        8 | 12 => Finish::Balconies,
        _ => Finish::CurtainWall,
    };
    let starts = [2, count * 2 / 3, count * 5 / 6];
    let ends = [count * 2 / 3, count * 5 / 6, count];
    let mut upper = w;
    let mut crown_x = 0.0;
    for (index, (&from, &to)) in starts.iter().zip(ends.iter()).enumerate() {
        let shrink = match family {
            5 => 1.0,
            6 | 10 => 1.0 - index as f32 * 0.23,
            7 => 1.0 - index as f32 * (0.12 + plan as f32 * 0.005),
            11 => 1.0 - index as f32 * 0.09,
            12 => 1.0 - index as f32 * 0.21,
            _ => 1.0 - index as f32 * 0.13,
        };
        upper = w * shrink;
        let mid = d * shrink;
        let z = from as f32 * storey;
        let h = to as f32 * storey;
        let mut one = Obj::new();
        let half = if family == 8 { upper * 0.46 } else { upper };
        let b = rectangle(half, mid);
        let c = bevel(
            half,
            mid,
            if matches!(family, 9 | 11) {
                half.min(mid) * 0.45
            } else {
                0.8
            },
        );
        let outline: &[[f32; 2]] = if family == 5 || family == 6 { &b } else { &c };
        finish_extrusion(&mut one, outline, z, h, wall, facade);
        if family == 12 {
            // Intermittent real terrace outlines keep the silhouette readable
            // from a distance. Small balcony rails are part of the facade finish.
            one.bx(
                -half - 0.30,
                -mid - 0.85,
                h - 0.4,
                half + 0.30,
                -mid + 0.20,
                h,
                CONCRETE,
            );
        }
        if family == 6 || family == 10 {
            for x in [-half * 0.83, 0.0, half * 0.83] {
                one.bx(
                    x - 0.20,
                    -mid - 0.17,
                    z,
                    x + 0.20,
                    -mid + 0.06,
                    h + 0.36,
                    CONCRETE,
                );
            }
        }
        if family == 8 {
            obj.add_placed(&one, -w * 0.55, 0.0, 0.0);
            obj.add_placed(&one, w * 0.55, 0.0, 0.0);
            obj.bx(
                -w * 0.6,
                -1.1,
                z + storey,
                w * 0.6,
                1.1,
                z + storey * 1.7,
                wall,
            );
        } else {
            crown_x = if family == 11 {
                [0.0, -w * 0.17, w * 0.12][index] * if plan.is_multiple_of(2) { 1.0 } else { -1.0 }
            } else {
                0.0
            };
            obj.add_placed(&one, crown_x, 0.0, 0.0);
        }
    }
    let top = count as f32 * storey;
    match family {
        6 | 10 => {
            let crown = bevel(upper * 0.63, d * 0.43, 0.65);
            let tip = bevel(upper * 0.22, d * 0.17, 0.29);
            obj.loft(&crown, top, &tip, top + 7.0, wall, Some(ROOF));
            obj.prism(
                0.0,
                0.0,
                top + 7.0,
                top + if family == 10 {
                    39.0 + (plan % 3) as f32 * 4.0
                } else {
                    11.0
                },
                0.23,
                CONCRETE,
                6,
                Some(CONCRETE),
            );
        }
        8 => {
            for x in [-w * 0.55, w * 0.55] {
                obj.bx(x - 0.9, -1.0, top, x + 0.9, 1.0, top + 3.2, ROOF);
            }
        }
        11 => {
            let mut lantern = Obj::new();
            let ring = bevel(upper * 0.64, d * 0.50, 0.65);
            finish_extrusion(
                &mut lantern,
                &ring,
                top,
                top + 8.0,
                wall,
                Finish::CurtainWall,
            );
            obj.add_placed(&lantern, crown_x, 0.0, 0.0);
        }
        _ => {
            obj.bx(
                -upper * 0.43,
                -d * 0.28,
                top,
                upper * 0.43,
                d * 0.28,
                top + 2.3,
                ROOF,
            );
        }
    }
    obj
}

/// Create one of the measured architecture plans. Level five previews the full skyline form.
pub fn build(family: usize, plan: usize, level: u8) -> Obj {
    const WALLS: [Rgb; 9] = [
        [208, 199, 177],
        [163, 118, 97],
        [197, 176, 127],
        [119, 145, 158],
        [159, 165, 163],
        [131, 156, 153],
        [176, 129, 127],
        [161, 162, 190],
        [198, 188, 153],
    ];
    let wall = WALLS[(family + plan * 5) % WALLS.len()];
    if family < 5 {
        small(family, plan, wall)
    } else {
        tower(family, plan, level, wall)
    }
}
