//! New neighborhood and skyline models. Every family has twelve seeded plans.
use crate::obj::{Obj, Rgb};
use crate::parts::{GLASS, GLASS_D};
use crate::rng::Rng;
use us_gen::LotKind;

pub const FAMILIES: [&str; 11] = [
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
            if level == 5 && density >= 0.985 && kind == LotKind::Office && rng.below(24) == 0 {
                10
            } else {
                5 + rng.below(5)
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
    obj.extrude(&ring, base, top, wall, ROOF);
    floors(&mut obj, &ring, base, count, storey, 3 + plan % 4);
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
        [28, 32, 38, 31, 34, 69][family - 5] + if family == 5 { 0 } else { plan % 4 }
    };
    let podium = rectangle(w + 1.2, d + 1.2);
    obj.extrude(&podium, 0.0, storey * 2.0, CONCRETE, ROOF);
    floors(&mut obj, &podium, 0.0, 2, storey, 4);
    let starts = [2, count * 2 / 3, count * 5 / 6];
    let ends = [count * 2 / 3, count * 5 / 6, count];
    let mut upper = w;
    for (index, (&from, &to)) in starts.iter().zip(ends.iter()).enumerate() {
        let shrink = match family {
            5 => 1.0,
            6 | 10 => 1.0 - index as f32 * 0.23,
            7 => 1.0 - index as f32 * (0.12 + plan as f32 * 0.005),
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
            if family == 9 {
                half.min(mid) * 0.45
            } else {
                0.8
            },
        );
        let outline: &[[f32; 2]] = if family == 5 || family == 6 { &b } else { &c };
        one.extrude(outline, z, h, wall, ROOF);
        floors(
            &mut one,
            outline,
            z,
            to - from,
            storey,
            if family == 5 { 3 } else { 4 + plan % 2 },
        );
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
            obj.add_placed(&one, 0.0, 0.0, 0.0);
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
                top + if family == 10 { 31.0 } else { 11.0 },
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
    const WALLS: [Rgb; 6] = [
        [208, 199, 177],
        [163, 118, 97],
        [197, 176, 127],
        [119, 145, 158],
        [159, 165, 163],
        [131, 156, 153],
    ];
    let wall = WALLS[(family + plan * 5) % WALLS.len()];
    if family < 5 {
        small(family, plan, wall)
    } else {
        tower(family, plan, level, wall)
    }
}
