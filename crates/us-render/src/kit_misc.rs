//! Parametric builders for every lot kind that is not a home, shop, flat block or office:
//! vehicles, car parks, parks, sports grounds, churches, schools, stations, industry, farms and
//! the big commercial boxes. See `kit` for the shared conventions.

use crate::kit::{CAR_COLS, TREE, TREE_D, tree};
use crate::obj::{Obj, Rgb};
use crate::parts::*;
use crate::rng::Rng;

const CAR_MORE: [Rgb; 6] = [
    [150, 156, 164],
    [120, 40, 52],
    [226, 140, 50],
    [110, 150, 190],
    [30, 34, 44],
    [200, 190, 160],
];

/// A vehicle of about four metres facing +x: sedan, hatchback, pickup or van.
pub fn car(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = if rng.chance(0.6) {
        rng.pick(&CAR_COLS)
    } else {
        rng.pick(&CAR_MORE)
    };
    let win = [70, 96, 120];
    match wpick(rng, &[(0u8, 5), (1, 2), (2, 2), (3, 1)]) {
        0 => {
            o.bx(-2.1, -0.9, 0.3, 2.1, 0.9, 1.0, col);
            o.bx_top(-1.1, -0.8, 1.0, 1.0, 0.8, 1.55, win, col);
        }
        1 => {
            o.bx(-1.9, -0.9, 0.3, 1.9, 0.9, 1.0, col);
            o.bx_top(-1.7, -0.8, 1.0, 0.9, 0.8, 1.7, win, col);
        }
        2 => {
            o.bx(-2.3, -0.95, 0.3, 2.3, 0.95, 1.0, col);
            o.bx_top(0.0, -0.85, 1.0, 1.6, 0.85, 1.6, win, col);
            o.bx(-2.3, -0.9, 1.0, -0.2, 0.9, 1.15, shade(col, 0.7));
        }
        _ => {
            o.bx_top(-2.3, -0.95, 0.3, 2.3, 0.95, 2.0, col, shade(col, 0.9));
            o.bx(1.5, -0.97, 1.1, 2.3, 0.97, 1.7, win);
        }
    }
    o
}

pub fn parking(w: f32, d: f32, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let asphalt = rng.pick(&[[92u8, 94, 102], [80, 82, 88], [112, 112, 116], [70, 72, 78]]);
    o.slab(-w / 2.0, -d / 2.0, 0.0, w / 2.0, d / 2.0, 0.12, asphalt);
    let n = (w / 3.0) as usize;
    let line = rng.pick(&[[230u8, 230, 226], [230, 230, 226], [236, 200, 60]]);
    let layout = rng.below(3); // 0 both sides, 1 south side only, 2 north side only
    let rows: Vec<(f32, f32)> = match layout {
        0 => vec![
            (-d / 2.0 + 0.6, -d / 2.0 + 5.4),
            (d / 2.0 - 5.4, d / 2.0 - 0.6),
        ],
        1 => vec![(-d / 2.0 + 0.6, -d / 2.0 + 5.4)],
        _ => vec![(d / 2.0 - 5.4, d / 2.0 - 0.6)],
    };
    for i in 0..=n {
        let x = -w / 2.0 + 1.0 + i as f32 * (w - 2.0) / n as f32;
        for &(y0, y1) in &rows {
            o.poly(
                &[
                    [x - 0.12, y0, 0.2],
                    [x + 0.12, y0, 0.2],
                    [x + 0.12, y1, 0.2],
                    [x - 0.12, y1, 0.2],
                ],
                line,
                false,
                false,
                true,
            );
        }
    }
    let fill = 0.35 + rng.f32() * 0.5;
    for i in 0..n {
        let x = -w / 2.0 + 1.0 + (i as f32 + 0.5) * (w - 2.0) / n as f32;
        for &(y0, y1) in &rows {
            if rng.chance(fill) {
                let c = car(rng);
                let a = if y0 < 0.0 {
                    std::f32::consts::FRAC_PI_2
                } else {
                    -std::f32::consts::FRAC_PI_2
                };
                o.add_placed(&c, x, (y0 + y1) / 2.0, a);
            }
        }
    }
    if rng.chance(0.5) {
        for (x, y) in [(-w / 2.0 + 0.5, 0.0f32), (w / 2.0 - 0.5, 0.0)] {
            if rng.chance(0.6) {
                o.bx(
                    x - 0.08,
                    y - 0.08,
                    0.12,
                    x + 0.08,
                    y + 0.08,
                    5.5,
                    [90, 94, 100],
                );
                o.bx(
                    x - 0.5,
                    y - 0.2,
                    5.5,
                    x + 0.5,
                    y + 0.2,
                    5.7,
                    [240, 232, 190],
                );
            }
        }
    }
    if layout > 0 && rng.chance(0.5) {
        let cy = if layout == 1 {
            d / 2.0 - 1.4
        } else {
            -d / 2.0 + 1.4
        };
        o.bx_top(
            -w / 2.0 + 1.0,
            cy - 0.8,
            0.12,
            w / 2.0 - 1.0,
            cy + 0.8,
            0.5,
            [150, 130, 100],
            GREEN,
        );
    }
    o
}

pub fn park(region: Region, w: f32, d: f32, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let green = rng.pick(&[[104u8, 168, 84], [112, 176, 90], [96, 158, 80]]);
    let green = if region == Region::Sunbelt {
        mix(green, [200, 180, 110], 0.35)
    } else {
        green
    };
    o.slab(-w / 2.0, -d / 2.0, 0.0, w / 2.0, d / 2.0, 0.15, green);
    let path = rng.pick(&[[222u8, 210, 170], [214, 206, 190], [196, 160, 130]]);
    let strip = |o: &mut Obj, a: [f32; 4]| {
        o.poly(
            &[
                [a[0], a[1], 0.2],
                [a[2], a[1], 0.2],
                [a[2], a[3], 0.2],
                [a[0], a[3], 0.2],
            ],
            path,
            false,
            false,
            true,
        );
    };
    match rng.below(3) {
        0 => {
            strip(&mut o, [-w / 2.0, -1.2, w / 2.0, 1.2]);
            strip(&mut o, [-1.2, -d / 2.0, 1.2, d / 2.0]);
        }
        1 => {
            strip(&mut o, [-w / 2.0, -1.0, w / 2.0, 1.0]);
        }
        _ => {
            strip(
                &mut o,
                [
                    -w / 2.0 + 2.0,
                    -d / 2.0 + 2.0,
                    w / 2.0 - 2.0,
                    -d / 2.0 + 3.2,
                ],
            );
            strip(&mut o, [-1.2, -d / 2.0, 1.2, d / 2.0]);
        }
    }
    let centre = region_centre(region, rng);
    match centre {
        0 => {
            o.prism(
                0.0,
                0.0,
                0.15,
                0.8,
                3.2,
                [200, 196, 186],
                10,
                Some([110, 170, 220]),
            );
            o.prism(0.0, 0.0, 0.8, 2.2, 0.4, [200, 196, 186], 6, None);
        }
        1 => {
            o.prism(
                0.0,
                0.0,
                0.15,
                0.4,
                4.6,
                [200, 196, 186],
                12,
                Some([100, 164, 214]),
            );
        }
        2 => {
            o.prism(0.0, 0.0, 0.15, 3.0, 2.6, [236, 230, 214], 8, None);
            o.pyramid(
                0.0,
                0.0,
                3.0,
                3.2,
                1.6,
                rng.pick(&[[90u8, 130, 120], [150, 70, 60], [90, 100, 140]]),
                8,
                22.5,
            );
        }
        3 => {
            o.bx(-1.0, -1.0, 0.15, 1.0, 1.0, 1.2, STONE);
            o.bx(-0.4, -0.4, 1.2, 0.4, 0.4, 3.4, shade(STONE, 0.9));
        }
        _ => {
            for (sx, sy) in [(-4.0f32, 0.0f32), (4.0, 0.0)] {
                prop(&mut o, Prop::Swing, sx, sy + 3.0, rng);
            }
            o.slab(-6.0, -3.2, 0.15, 6.0, 0.2, 0.22, [226, 206, 150]);
        }
    }
    for _ in 0..(w * d / 70.0) as usize {
        let x = rng.range(-w / 2.0 + 2.0, w / 2.0 - 2.0);
        let y = rng.range(-d / 2.0 + 2.0, d / 2.0 - 2.0);
        if x.abs() < 3.0 || y.abs() < 3.0 {
            continue;
        }
        let (r, h) = (rng.range(1.8, 2.8), rng.range(5.0, 8.0));
        let col = rng.pick(&[TREE, TREE_D]);
        o.add_placed(&tree(r, h, col), x, y, 0.0);
    }
    let bench = rng.pick(&[[130u8, 90, 60], [60, 90, 70], [90, 94, 100]]);
    for x in [-5.0f32, 5.0] {
        o.bx(x - 0.9, 2.0, 0.15, x + 0.9, 2.5, 0.6, bench);
    }
    if rng.chance(0.5) {
        let c = rng.pick(&FLOWERS);
        o.bx_top(
            -w / 2.0 + 1.0,
            d / 2.0 - 2.4,
            0.15,
            -w / 2.0 + 3.6,
            d / 2.0 - 1.0,
            0.5,
            [112, 82, 58],
            c,
        );
    }
    o
}

fn region_centre(region: Region, rng: &mut Rng) -> u8 {
    match region {
        Region::Prairie => wpick(rng, &[(0u8, 3), (2, 4), (3, 3), (4, 2), (1, 1)]),
        Region::Sunbelt => wpick(rng, &[(1u8, 4), (0, 3), (4, 3), (3, 1), (2, 1)]),
        _ => rng.below(5) as u8,
    }
}

pub fn sports(region: Region, w: f32, d: f32, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let turf = rng.pick(&[[92u8, 160, 76], [100, 168, 84], [84, 150, 70]]);
    o.slab(-w / 2.0, -d / 2.0, 0.0, w / 2.0, d / 2.0, 0.15, turf);
    let inner = [-w / 2.0 + 2.0, -d / 2.0 + 2.0, w / 2.0 - 2.0, d / 2.0 - 2.0];
    let rect = |o: &mut Obj, a: [f32; 4], z: f32, c: Rgb| {
        o.poly(
            &[
                [a[0], a[1], z],
                [a[2], a[1], z],
                [a[2], a[3], z],
                [a[0], a[3], z],
            ],
            c,
            false,
            false,
            true,
        );
    };
    let kind = if region == Region::Sunbelt {
        rng.pick(&[1u8, 2, 3])
    } else {
        rng.below(4) as u8
    };
    match kind {
        0 => {
            rect(&mut o, inner, 0.2, [78, 150, 70]);
            rect(
                &mut o,
                [-0.15, inner[1], 0.15, inner[3]],
                0.24,
                [240, 240, 240],
            );
            for x in [-w / 2.0 + 2.5, w / 2.0 - 2.5] {
                o.bx(x - 0.1, -2.0, 0.15, x + 0.1, 2.0, 2.4, [240, 240, 240]);
            }
        }
        1 => {
            rect(
                &mut o,
                [-w / 2.0 + 3.0, -d / 2.0 + 2.5, w / 2.0 - 3.0, d / 2.0 - 2.5],
                0.2,
                [60, 120, 150],
            );
            rect(
                &mut o,
                [-0.1, -d / 2.0 + 2.5, 0.1, d / 2.0 - 2.5],
                0.24,
                [240, 240, 240],
            );
            o.bx(
                -0.05,
                -d / 2.0 + 2.5,
                0.2,
                0.05,
                d / 2.0 - 2.5,
                1.0,
                [236, 236, 236],
            );
            for y in [-d / 2.0 + 1.5, d / 2.0 - 1.5] {
                o.bx(
                    -w / 2.0 + 0.5,
                    y - 0.05,
                    0.15,
                    w / 2.0 - 0.5,
                    y + 0.05,
                    3.0,
                    [60, 66, 74],
                );
            }
        }
        2 => {
            rect(
                &mut o,
                [-w / 2.0 + 3.0, -d / 2.0 + 2.5, w / 2.0 - 3.0, d / 2.0 - 2.5],
                0.2,
                [196, 120, 70],
            );
            for x in [-w / 2.0 + 3.0, w / 2.0 - 3.0] {
                o.bx(x - 0.07, -0.07, 0.2, x + 0.07, 0.07, 3.0, [90, 94, 100]);
                o.bx(x - 0.5, -0.5, 3.0, x + 0.5, 0.5, 3.1, [236, 236, 236]);
            }
        }
        _ => {
            o.poly(
                &[
                    [0.0, -d / 2.0 + 1.5, 0.2],
                    [w / 2.0 - 2.0, 0.0, 0.2],
                    [0.0, d / 2.0 - 4.0, 0.2],
                    [-w / 2.0 + 2.0, 0.0, 0.2],
                ],
                [210, 170, 110],
                false,
                false,
                true,
            );
            rect(&mut o, [-1.4, -1.4, 1.4, 1.4], 0.24, [78, 150, 70]);
            o.bx(-0.3, -d / 2.0 + 1.0, 0.15, 0.3, -d / 2.0 + 1.5, 0.25, WHITE);
            o.bx(
                -w / 2.0 + 0.5,
                d / 2.0 - 2.5,
                0.15,
                w / 2.0 - 0.5,
                d / 2.0 - 2.2,
                4.0,
                [150, 154, 160],
            );
        }
    }
    if rng.chance(0.5) {
        let bc = rng.pick(&[[196u8, 58, 52], [40, 92, 160], [150, 154, 160]]);
        for k in 0..3 {
            let z = 0.15 + k as f32 * 0.45;
            o.bx(
                -w / 4.0,
                d / 2.0 - 1.4 - k as f32 * 0.5,
                z,
                w / 4.0,
                d / 2.0 - 0.9 - k as f32 * 0.5,
                z + 0.45,
                bc,
            );
        }
    }
    o
}

// ---- civic and religious ----

pub fn church(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (mat, wall) = match region {
        Region::Sunbelt => (Mat::Stucco, rng.pick(&STUCCOS)),
        _ => match rng.below(4) {
            0 => (
                Mat::Wood,
                rng.pick(&[WHITE, [236, 226, 196], [214, 206, 190]]),
            ),
            1 => (Mat::Brick, rng.pick(&BRICKS)),
            2 => (
                Mat::Concrete,
                rng.pick(&[[214u8, 206, 190], [196, 194, 188], [208, 200, 186]]),
            ),
            _ => (Mat::Wood, [214, 206, 190]),
        },
    };
    let _ = mat;
    let slate = if region == Region::Sunbelt {
        rng.pick(&TILES)
    } else {
        rng.pick(&[[86u8, 92, 104], [70, 74, 82], [120, 70, 60], [90, 100, 80]])
    };
    let w = rng.pick(&[9.0f32, 10.0, 10.0]);
    let len = rng.pick(&[14.0f32, 16.0]);
    let (hw, y1) = (w / 2.0, len - 4.0);
    let h = rng.pick(&[6.5f32, 7.0, 7.5]);
    o.bx(-hw, -4.0, 0.0, hw, y1, h, wall);
    o.gable(-hw, -4.0, hw, y1, h, 5.5, slate, wall, false, 0.3);
    let tw = rng.pick(&[4.6f32, 5.2]);
    let steeple = wpick(rng, &[(0u8, 4), (1, 3), (2, 2), (3, 2), (4, 1)]);
    let tower = |o: &mut Obj, cx: f32, th: f32| {
        o.bx(cx - tw / 2.0, -8.0, 0.0, cx + tw / 2.0, -2.8, th, wall);
        o.win_s(cx - 0.8, cx + 0.8, th - 4.5, th - 2.0, -8.0, GLASS);
    };
    match steeple {
        0 => {
            tower(&mut o, 0.0, 13.0);
            o.pyramid(0.0, -5.4, 13.0, tw * 0.72, 9.0, slate, 4, 45.0);
            o.win_s(-0.9, 0.9, 0.0, 3.0, -8.0, rng.pick(&DOORS));
        }
        1 => {
            tower(&mut o, 0.0, 11.0);
            o.prism(0.0, -5.4, 11.0, 14.0, tw * 0.42, shade(wall, 0.95), 8, None);
            o.pyramid(0.0, -5.4, 14.0, tw * 0.46, 6.0, slate, 8, 22.5);
            o.win_s(-0.9, 0.9, 0.0, 3.0, -8.0, rng.pick(&DOORS));
        }
        2 => {
            tower(&mut o, 0.0, 12.0);
            o.parapet(
                -tw / 2.0,
                -8.0,
                tw / 2.0,
                -2.8,
                12.0,
                0.9,
                wall,
                TRIM,
                shade(slate, 0.8),
            );
            o.win_s(-0.9, 0.9, 0.0, 3.0, -8.0, rng.pick(&DOORS));
        }
        3 => {
            let c = hw - tw / 2.0;
            for s in [-1.0f32, 1.0] {
                tower(&mut o, s * c, 10.5);
                o.pyramid(s * c, -5.4, 10.5, tw * 0.72, 5.5, slate, 4, 45.0);
            }
            o.win_s(-1.0, 1.0, 0.0, 3.0, -4.0, rng.pick(&DOORS));
        }
        _ => {
            // no tower: a bellcote on the front gable
            o.bx(-0.5, -4.0, h + 3.0, 0.5, -3.4, h + 5.0, wall);
            o.pyramid(0.0, -3.7, h + 5.0, 0.8, 1.2, slate, 4, 45.0);
            o.win_s(-0.9, 0.9, 0.0, 3.0, -4.0, rng.pick(&DOORS));
            o.slab(-2.0, -6.0, 0.0, 2.0, -4.0, 0.2, [214, 206, 190]);
        }
    }
    // a cross
    let top = match steeple {
        0 => 22.0,
        1 => 20.0,
        2 => 12.0,
        3 => 16.0,
        _ => h + 6.2,
    };
    if steeple != 2 && steeple != 3 {
        let ty = if steeple == 4 { -3.7 } else { -5.4 };
        o.bx(
            -0.06,
            ty - 0.06,
            top,
            0.06,
            ty + 0.06,
            top + 1.4,
            [236, 214, 120],
        );
        o.bx(
            -0.4,
            ty - 0.05,
            top + 0.8,
            0.4,
            ty + 0.05,
            top + 0.95,
            [236, 214, 120],
        );
    }
    let stained = rng.pick(&[
        [150u8, 120, 170],
        [90, 120, 190],
        [190, 90, 90],
        [210, 170, 70],
    ]);
    let n = 2 + rng.below(3);
    for i in 0..n {
        let cy = 0.0 + i as f32 * (y1 - 1.0) / n as f32;
        let sc = if (i + steeple as usize).is_multiple_of(2) {
            stained
        } else {
            shade(stained, 0.8)
        };
        o.win_w(cy - 0.6, cy + 0.6, 2.0, 5.6, -hw, sc);
        o.win_e(cy - 0.6, cy + 0.6, 2.0, 5.6, hw, sc);
    }
    if rng.chance(0.4) {
        o.bx_top(
            hw,
            2.0,
            0.0,
            hw + 3.2,
            y1 - 1.0,
            3.2,
            wall,
            shade(slate, 0.9),
        );
        o.win_s(hw + 0.6, hw + 2.6, 0.0, 2.0, 2.0, GLASS);
    }
    if rng.chance(0.3) {
        for (x, y) in [(-hw - 2.5, 4.0f32), (-hw - 3.5, 6.0), (-hw - 2.0, 8.0)] {
            o.bx(
                x - 0.3,
                y - 0.1,
                0.0,
                x + 0.3,
                y + 0.1,
                0.9,
                [170, 170, 166],
            );
        }
    }
    o
}

pub fn school(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = match region {
        Region::Sunbelt => rng.pick(&STUCCOS),
        _ => rng.pick(&[
            [222u8, 160, 110],
            [196, 120, 90],
            [214, 196, 150],
            [176, 100, 80],
            [206, 190, 170],
        ]),
    };
    let roof = rng.pick(&[[124u8, 120, 116], [100, 104, 110], [150, 140, 130]]);
    let pat = wpick(
        rng,
        &[
            (Pattern::Grid, 4),
            (Pattern::Pairs, 2),
            (Pattern::Banded, 2),
            (Pattern::Ribbon, 1),
        ],
    );
    let layout = rng.below(3);
    let fl = rng.pick(&[1usize, 2, 2]);
    let h = fl as f32 * 3.8;
    let wing = |o: &mut Obj, r: (f32, f32, f32, f32), faces: u8| {
        o.bx_top(r.0, r.1, 0.0, r.2, r.3, h, wall, roof);
        box_windows(o, r, 0.0, fl, 3.8, pat, GLASS, GLASS_D, faces);
    };
    match layout {
        0 => {
            wing(&mut o, (-14.0, -6.0, 6.0, 2.0), 15);
            wing(&mut o, (6.0, -6.0, 14.0, 10.0), 15);
        }
        1 => {
            wing(&mut o, (-15.0, -4.0, 15.0, 3.0), 15);
            wing(&mut o, (-15.0, 3.0, -8.0, 11.0), 15);
        }
        _ => {
            wing(&mut o, (-12.0, -6.0, 4.0, 2.0), 15);
            wing(&mut o, (4.0, -6.0, 12.0, 2.0), 15);
            wing(&mut o, (-12.0, 2.0, -4.0, 10.0), 15);
        }
    }
    let cc = rng.pick(&[WHITE, TRIM, [214, 196, 160]]);
    let door_y = if layout == 1 { -4.0 } else { -6.0 };
    o.bx(-3.0, door_y - 1.5, 0.0, 1.0, door_y, 3.2, cc);
    o.win_s(-2.0, 0.0, 0.0, 2.6, door_y - 1.5, rng.pick(&DOORS));
    if rng.chance(0.4) {
        o.bx(-2.6, door_y - 1.0, h, 0.6, door_y + 1.0, h + 3.6, cc);
        o.pyramid(-1.0, door_y, h + 3.6, 2.4, 2.2, shade(roof, 0.8), 4, 45.0);
        o.win_s(-1.4, -0.6, h + 0.8, h + 2.8, door_y - 1.0, WHITE);
    }
    let fy = -11.5;
    o.bx(-10.0, fy - 0.5, 0.0, -9.8, fy - 0.3, 10.0, [210, 210, 210]);
    let flag = rng.pick(&[[40u8, 90, 170], [196, 58, 52], [40, 130, 90]]);
    o.poly(
        &[
            [-9.8, fy - 0.4, 8.2],
            [-7.0, fy - 0.4, 8.2],
            [-7.0, fy - 0.4, 9.8],
            [-9.8, fy - 0.4, 9.8],
        ],
        flag,
        false,
        true,
        true,
    );
    o.slab(
        -13.0,
        4.0,
        0.0,
        -4.0,
        12.0,
        0.08,
        rng.pick(&[[196u8, 120, 90], [120, 126, 136], [90, 130, 100]]),
    );
    if layout != 1 {
        for (x, y) in [(-11.0f32, 6.0f32), (-8.0, 9.0)] {
            if rng.chance(0.7) {
                prop(
                    &mut o,
                    rng.pick(&[Prop::Swing, Prop::Trampoline, Prop::Table]),
                    x,
                    y,
                    rng,
                );
            }
        }
    }
    o
}

pub fn fire_station(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = if region == Region::Sunbelt {
        rng.pick(&STUCCOS)
    } else {
        rng.pick(&[
            [170u8, 70, 56],
            [186, 112, 78],
            [206, 190, 160],
            [150, 72, 58],
            [226, 220, 206],
        ])
    };
    let roof = rng.pick(&[[110u8, 104, 100], [90, 94, 100], [140, 90, 70]]);
    let bays = rng.pick(&[2usize, 3, 3]);
    let w = bays as f32 * 6.0 + 2.0;
    let door = rng.pick(&[[200u8, 44, 40], WHITE, [226, 190, 60], [60, 90, 150]]);
    o.bx_top(-w / 2.0, -6.0, 0.0, w / 2.0, 8.0, 6.4, wall, roof);
    for i in 0..bays {
        let cx = -w / 2.0 + 1.0 + 3.0 + i as f32 * 6.0;
        o.win_s(cx - 2.2, cx + 2.2, 0.0, 4.4, -6.0, door);
        let sh = if rng.chance(0.5) { 0.0 } else { 0.1 };
        o.win_s(
            cx - 2.2,
            cx + 2.2,
            3.2 + sh,
            3.3 + sh,
            -6.0,
            shade(door, 0.8),
        );
    }
    o.win_s(
        -w / 2.0 + 0.6,
        w / 2.0 - 0.6,
        4.9,
        5.8,
        -6.0,
        rng.pick(&[WHITE, [226, 190, 60], [240, 230, 200]]),
    );
    let side = if rng.chance(0.5) { 1.0f32 } else { -1.0 };
    let (ta, tb) = if side > 0.0 {
        (w / 2.0 - 3.0, w / 2.0)
    } else {
        (-w / 2.0, -w / 2.0 + 3.0)
    };
    match rng.below(3) {
        0 => {
            o.bx_top(ta, 3.0, 6.4, tb, 8.0, 13.0, wall, roof);
            o.win_w(4.6, 6.4, 9.0, 11.5, if side > 0.0 { ta } else { tb }, GLASS);
        }
        1 => {
            o.bx(ta, 3.0, 6.4, tb, 8.0, 12.0, wall);
            o.hip(ta, 3.0, tb, 8.0, 12.0, 2.2, roof, 0.3);
        }
        _ => {
            o.gable(
                -w / 2.0,
                -6.0,
                w / 2.0,
                8.0,
                6.4,
                2.0,
                roof,
                wall,
                true,
                0.3,
            );
        }
    }
    o.slab(
        -w / 2.0 - 1.0,
        -12.0,
        0.0,
        w / 2.0 + 1.0,
        -6.0,
        0.08,
        [150, 150, 146],
    );
    if rng.chance(0.4) {
        o.bx(-0.05, -9.0, 0.0, 0.05, -8.9, 7.0, [200, 200, 204]);
        o.bx(-0.7, -9.3, 7.0, 0.7, -8.6, 7.5, [200, 44, 40]);
    }
    o
}

pub fn gas_station(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let grey = rng.pick(&[[150u8, 150, 146], [130, 132, 138], [170, 168, 160]]);
    let brand = rng.pick(&[
        [30u8, 120, 190],
        [196, 58, 52],
        [40, 130, 90],
        [230, 170, 40],
        [226, 120, 40],
        [120, 70, 150],
    ]);
    let pole_h = rng.pick(&[6.5f32, 8.0, 9.0]);
    o.slab(-11.0, -9.0, 0.0, 11.0, 9.0, 0.12, grey);
    let sw = rng.pick(&[8.0f32, 8.0, 6.0]);
    let wallc = if region == Region::Sunbelt {
        rng.pick(&STUCCOS)
    } else {
        rng.pick(&[WHITE, [226, 220, 206], [196, 190, 178]])
    };
    o.bx_top(2.0, 2.0, 0.12, 2.0 + sw, 8.0, 3.6, wallc, [130, 130, 130]);
    o.win_s(3.0, 1.0 + sw, 0.3, 2.7, 2.0, GLASS_D);
    o.bx(1.8, 1.8, 3.6, 2.2 + sw, 8.2, 4.2, brand);
    for x in [-8.5f32, -1.5] {
        for y in [-6.5f32, -1.5] {
            o.bx(
                x - 0.2,
                y - 0.2,
                0.12,
                x + 0.2,
                y + 0.2,
                4.6,
                [200, 200, 200],
            );
        }
    }
    let pumps = rng.pick(&[2usize, 2, 4]);
    for i in 0..pumps {
        let x = [-8.5f32, -1.5][i % 2];
        let y = -4.6 + (i / 2) as f32 * 0.0;
        let _ = y;
        o.bx_top(
            x - 0.4,
            -4.6,
            0.12,
            x + 0.4,
            -3.4,
            1.5,
            [200, 60, 50],
            [60, 60, 60],
        );
        if pumps == 4 {
            o.bx_top(
                x - 0.4,
                -4.6 + 2.0,
                0.12,
                x + 0.4,
                -3.4 + 2.0,
                1.5,
                [200, 60, 50],
                [60, 60, 60],
            );
        }
    }
    o.bx_top(-10.0, -8.0, 4.6, 0.0, 0.0, 5.3, brand, WHITE);
    if rng.chance(0.3) {
        o.bx_top(-10.0, -8.0, 4.6, 0.0, -7.6, 5.3, WHITE, WHITE);
    }
    o.bx(9.0, -8.5, 0.12, 9.4, -8.1, pole_h, [90, 90, 96]);
    if rng.chance(0.5) {
        o.bx(8.2, -8.6, pole_h, 10.2, -8.0, pole_h + 2.1, brand);
    } else {
        o.prism(9.2, -8.3, pole_h, pole_h + 1.8, 1.1, brand, 8, None);
    }
    if rng.chance(0.25) {
        o.bx_top(
            -10.5,
            3.0,
            0.12,
            -4.5,
            8.5,
            3.4,
            [200, 204, 210],
            [150, 154, 160],
        );
        o.win_s(-9.5, -5.5, 0.1, 2.8, 3.0, GLASS_D);
    }
    o
}

// ---- industry ----

pub fn factory(_region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = rng.pick(&[
        [178u8, 168, 150],
        [150, 156, 164],
        [186, 116, 90],
        [196, 190, 176],
        [120, 126, 136],
        [176, 150, 110],
    ]);
    o.bx(-14.0, -8.0, 0.0, 14.0, 8.0, 6.0, wall);
    match rng.below(3) {
        0 => {
            let teeth = rng.pick(&[4usize, 5, 7]);
            let tw = 28.0 / teeth as f32;
            let glass = if rng.chance(0.3) { GLASS_D } else { GLASS };
            for i in 0..teeth {
                let x = -14.0 + i as f32 * tw;
                o.poly(
                    &[[x, -8.0, 6.0], [x + tw, -8.0, 6.0], [x, -8.0, 9.0]],
                    wall,
                    true,
                    true,
                    false,
                );
                o.poly(
                    &[[x + tw, 8.0, 6.0], [x, 8.0, 6.0], [x, 8.0, 9.0]],
                    wall,
                    true,
                    true,
                    false,
                );
                o.poly(
                    &[
                        [x + tw, -8.0, 6.0],
                        [x + tw, 8.0, 6.0],
                        [x, 8.0, 9.0],
                        [x, -8.0, 9.0],
                    ],
                    [110, 116, 126],
                    true,
                    true,
                    false,
                );
                o.poly(
                    &[[x, 8.0, 6.0], [x, -8.0, 6.0], [x, -8.0, 9.0], [x, 8.0, 9.0]],
                    glass,
                    true,
                    true,
                    false,
                );
            }
        }
        1 => {
            o.gable(
                -14.0,
                -8.0,
                14.0,
                8.0,
                6.0,
                3.0,
                rng.pick(&METALS),
                wall,
                true,
                0.4,
            );
        }
        _ => {
            o.parapet(
                -14.0,
                -8.0,
                14.0,
                8.0,
                6.0,
                0.8,
                wall,
                shade(wall, 1.1),
                [100, 98, 96],
            );
            roof_gear(&mut o, (-12.0, -6.0, 8.0, 6.0), 6.0, 3, rng);
        }
    }
    o.prism(
        11.0,
        4.0,
        0.0,
        20.0,
        1.3,
        rng.pick(&[[160u8, 90, 70], [150, 150, 150], [190, 160, 130]]),
        8,
        None,
    );
    let dc = rng.pick(&[[120u8, 126, 136], [90, 100, 110], [160, 110, 80]]);
    for cx in [-9.0f32, 0.0] {
        o.win_s(cx - 3.0, cx + 3.0, 0.0, 4.5, -8.0, dc);
    }
    o.slab(-14.0, -16.0, 0.0, -2.0, -9.0, 0.1, [150, 150, 146]);
    if rng.chance(0.5) {
        o.bx_top(
            -14.0,
            -12.0 + 3.0,
            0.0,
            -9.0,
            -8.0,
            3.6,
            shade(wall, 1.05),
            [120, 120, 124],
        );
        o.win_s(-13.0, -10.0, 1.0, 2.6, -8.0, GLASS);
    }
    if rng.chance(0.4) {
        o.prism(
            -12.0,
            11.0,
            0.0,
            7.0,
            2.2,
            rng.pick(&[[200u8, 204, 210], [150, 160, 170]]),
            10,
            None,
        );
    }
    o
}

pub fn warehouse(_region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&[
        [200u8, 196, 186],
        [176, 184, 194],
        [214, 200, 176],
        [150, 156, 160],
        [190, 150, 120],
        [120, 140, 150],
    ]);
    o.bx_top(-26.0, -16.0, 0.0, 26.0, 16.0, 10.0, col, [140, 140, 144]);
    let band = rng.pick(&[
        [196u8, 58, 52],
        [40, 92, 160],
        [226, 190, 60],
        shade(col, 0.7),
    ]);
    o.win_s(-26.0, 26.0, 6.8, 7.8, -16.0, band);
    let n = rng.pick(&[3usize, 4, 6]);
    let dc = rng.pick(&[[120u8, 126, 136], [90, 94, 100], [160, 100, 70]]);
    for i in 0..n {
        let x = -24.0 + (i as f32 + 0.5) * 48.0 / n as f32;
        o.win_s(x - 2.0, x + 2.0, 0.0, 4.4, -16.0, dc);
    }
    o.slab(-26.0, -26.0, 0.0, 26.0, -16.0, 0.1, [150, 150, 146]);
    for i in 0..n {
        if rng.chance(0.55) {
            let x = -24.0 + (i as f32 + 0.5) * 48.0 / n as f32;
            let tc = rng.pick(&[
                [236u8, 236, 236],
                [196, 58, 52],
                [40, 92, 160],
                [226, 226, 214],
            ]);
            o.bx(x - 1.3, -24.0, 0.1, x + 1.3, -18.0, 3.6, tc);
        }
    }
    if rng.chance(0.5) {
        o.bx_top(
            -26.0,
            -16.0,
            0.0,
            -18.0,
            -10.0,
            4.4,
            shade(col, 1.05),
            [130, 130, 134],
        );
        o.win_s(-24.0, -20.0, 1.0, 3.0, -16.0, GLASS);
    }
    if rng.chance(0.6) {
        roof_gear(&mut o, (-22.0, -12.0, 22.0, 12.0), 10.0, 4, rng);
    }
    o
}

pub fn water_tower(_region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let tank = rng.pick(&[
        [210u8, 214, 220],
        [226, 226, 214],
        [180, 200, 220],
        [236, 200, 90],
        [190, 196, 204],
    ]);
    let legs = rng.pick(&[4usize, 4, 6]);
    let style = rng.below(3);
    let band = rng.pick(&[[40u8, 90, 170], [196, 58, 52], [40, 130, 90], [60, 60, 66]]);
    match style {
        0 | 1 => {
            let r = if style == 0 { 2.5 } else { 3.0 };
            for i in 0..legs {
                let a = i as f32 * std::f32::consts::TAU / legs as f32 + 0.785;
                let (x, y) = (r * 1.2 * libm::cosf(a), r * 1.2 * libm::sinf(a));
                o.bx(
                    x - 0.25,
                    y - 0.25,
                    0.0,
                    x + 0.25,
                    y + 0.25,
                    14.0,
                    [120, 124, 130],
                );
            }
            let tr = if style == 0 { 4.2 } else { 3.6 };
            o.prism(0.0, 0.0, 14.0, 19.0, tr, tank, 10, None);
            o.pyramid(0.0, 0.0, 19.0, tr + 0.1, 2.2, shade(tank, 0.75), 10, 18.0);
            o.poly(
                &[
                    [-2.5, -tr, 15.5],
                    [2.5, -tr, 15.5],
                    [2.5, -tr, 17.4],
                    [-2.5, -tr, 17.4],
                ],
                band,
                false,
                true,
                true,
            );
        }
        _ => {
            o.prism(0.0, 0.0, 0.0, 14.0, 1.4, shade(tank, 0.9), 10, None);
            o.prism(0.0, 0.0, 14.0, 17.0, 4.4, tank, 12, None);
            o.pyramid(0.0, 0.0, 17.0, 4.5, 2.6, shade(tank, 0.8), 12, 15.0);
            o.poly(
                &[
                    [-2.5, -4.4, 14.8],
                    [2.5, -4.4, 14.8],
                    [2.5, -4.4, 16.2],
                    [-2.5, -4.4, 16.2],
                ],
                band,
                false,
                true,
                true,
            );
            o.prism(0.0, 0.0, 16.9, 17.1, 5.0, [150, 154, 160], 12, None);
        }
    }
    o
}

// ---- commercial boxes ----

pub fn strip_mall(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let units = rng.pick(&[4usize, 5, 5, 6]);
    let (mat, _) = (material(region, rng), 0);
    let (wall, trim) = if rng.chance(0.4) {
        wall_colours(mat, rng)
    } else {
        ([226, 214, 190], [196, 190, 178])
    };
    o.bx_top(-20.0, -6.0, 0.0, 20.0, 8.0, 5.2, wall, [130, 128, 124]);
    let uw = 38.0 / units as f32;
    for i in 0..units {
        let x0 = -19.0 + i as f32 * uw;
        let sign = rng.pick(&SIGNS);
        o.win_s(
            x0 + 0.6,
            x0 + uw - 0.8,
            0.0,
            3.0,
            -6.0,
            if rng.chance(0.3) { GLASS } else { GLASS_D },
        );
        o.win_s(x0 + 0.6, x0 + uw - 0.8, 3.5, 4.6, -6.0, sign);
        if rng.chance(0.3) {
            let (c1, c2) = rng.pick(&AWNINGS);
            awning(
                &mut o,
                x0 + 0.6,
                x0 + uw - 0.8,
                -6.0,
                2.4,
                3.2,
                1.4,
                c1,
                c2,
                6,
            );
        }
    }
    match rng.below(3) {
        0 => {
            o.bx(-20.2, -8.5, 3.2, 20.2, -6.0, 3.5, trim);
        }
        1 => {
            o.shed(
                -20.2,
                -8.2,
                20.2,
                -6.0,
                3.0,
                0.9,
                shade(trim, 0.8),
                wall,
                0,
                0.0,
            );
        }
        _ => {
            o.bx(-20.2, -7.0, 5.2, 20.2, -6.0, 6.4, trim);
        }
    }
    if rng.chance(0.4) {
        let px = rng.pick(&[-16.0f32, 16.0]);
        o.bx(px - 0.15, -9.0, 0.0, px + 0.15, -8.7, 7.5, [90, 90, 96]);
        o.bx(px - 1.4, -9.2, 7.5, px + 1.4, -8.6, 10.0, rng.pick(&SIGNS));
    }
    if rng.chance(0.5) {
        roof_gear(&mut o, (-18.0, -3.0, 18.0, 6.0), 5.2, 3, rng);
    }
    let p = parking(40.0, 14.0, rng);
    o.add_placed(&p, 0.0, -17.0, 0.0);
    o
}

pub fn big_box(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&[
        [210u8, 206, 196],
        [190, 200, 210],
        [226, 220, 200],
        [170, 176, 184],
        [214, 190, 160],
    ]);
    let col = if region == Region::Sunbelt {
        mix(col, [236, 214, 170], 0.4)
    } else {
        col
    };
    o.bx_top(-32.0, 2.0, 0.0, 32.0, 42.0, 9.0, col, [150, 150, 150]);
    let door_x = rng.pick(&[0.0f32, -14.0, 14.0]);
    o.win_s(door_x - 8.0, door_x + 8.0, 0.0, 4.0, 2.0, GLASS_D);
    let sign = rng.pick(&SIGNS);
    o.win_s(-26.0, 26.0, 6.6, 8.4, 2.0, sign);
    if rng.chance(0.5) {
        o.bx(
            door_x - 9.0,
            -1.0,
            0.0,
            door_x + 9.0,
            2.0,
            7.0,
            shade(col, 0.92),
        );
        o.win_s(door_x - 6.0, door_x + 6.0, 0.0, 3.6, -1.0, GLASS_D);
        o.win_s(door_x - 6.0, door_x + 6.0, 4.4, 6.2, -1.0, sign);
    }
    for x in [-20.0f32, -10.0, 10.0, 20.0] {
        o.bx(x - 0.4, 34.0, 9.0, x + 0.4, 38.0, 11.0, [170, 170, 170]);
    }
    if rng.chance(0.5) {
        o.bx_top(
            -32.0,
            2.0 - 4.0,
            0.0,
            -20.0,
            2.0,
            5.0,
            shade(col, 0.9),
            [150, 150, 150],
        );
        o.slab(-32.0, -10.0, 0.0, -20.0, -4.0, 0.1, [170, 168, 160]);
    }
    if rng.chance(0.5) {
        let pxx = rng.pick(&[-30.0f32, 30.0]);
        o.bx(pxx - 0.3, -2.0, 0.0, pxx + 0.3, -1.4, 12.0, [90, 90, 96]);
        o.bx(pxx - 2.5, -2.4, 12.0, pxx + 2.5, -1.0, 15.0, sign);
    }
    roof_gear(&mut o, (-28.0, 8.0, 28.0, 38.0), 9.0, 2 + rng.below(4), rng);
    let pk = parking(64.0, 36.0, rng);
    o.add_placed(&pk, 0.0, -18.0, 0.0);
    o
}

pub fn fast_food(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&[
        [220u8, 60, 50],
        [240, 200, 60],
        [60, 120, 190],
        [236, 226, 206],
        [90, 130, 90],
        [226, 130, 60],
    ]);
    let top = rng.pick(&[[120u8, 116, 112], [80, 84, 90]]);
    match wpick(rng, &[(0u8, 3), (1, 3), (2, 2)]) {
        0 => {
            o.bx_top(-6.0, -4.0, 0.0, 6.0, 5.0, 4.2, col, top);
        }
        1 => {
            o.bx(-6.0, -4.0, 0.0, 6.0, 5.0, 4.2, col);
            o.hip(
                -6.0,
                -4.0,
                6.0,
                5.0,
                4.2,
                2.0,
                rng.pick(&[[110u8, 70, 60], [90, 94, 100], [196, 70, 52]]),
                0.5,
            );
        }
        _ => {
            o.bx(-6.0, -4.0, 0.0, 6.0, 5.0, 4.2, col);
            o.mansard(
                -6.0,
                -4.0,
                6.0,
                5.0,
                4.2,
                1.5,
                1.1,
                shade(col, 0.7),
                shade(col, 0.6),
            );
        }
    }
    o.win_s(-5.0, 5.0, 0.3, 3.0, -4.0, GLASS_D);
    let sc = rng.pick(&[[240u8, 200, 60], [196, 58, 52], [40, 92, 160], WHITE]);
    o.bx(7.0, -6.0, 0.0, 7.4, -5.6, 7.0, [90, 90, 96]);
    if rng.chance(0.5) {
        o.bx(6.0, -6.4, 7.0, 9.4, -5.2, 9.4, sc);
    } else {
        o.prism(7.2, -5.8, 7.0, 9.0, 1.6, sc, 10, None);
    }
    if rng.chance(0.5) {
        // drive-through lane and a menu board
        o.slab(6.0, -3.0, 0.0, 9.0, 12.0, 0.1, [84, 86, 92]);
        o.bx(8.0, 4.0, 0.0, 8.4, 4.3, 2.0, [90, 90, 96]);
        o.bx(7.8, 3.9, 1.6, 8.6, 4.4, 2.4, [236, 200, 60]);
    }
    if rng.chance(0.4) {
        let pc = rng.pick(&[[196u8, 58, 52], [230, 170, 40], [40, 92, 160]]);
        prop(&mut o, Prop::Table, -8.0, 0.0, rng);
        let _ = pc;
    }
    let pk = parking(14.0, 12.0, rng);
    o.add_placed(&pk, -3.0, 12.0, 0.0);
    if region == Region::Sunbelt && rng.chance(0.5) {
        o.bx_top(-6.5, -7.0, 0.0, 6.5, -4.0, 0.3, [190, 170, 120], GREEN);
    }
    o
}

pub fn farm(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let red = rng.pick(&[
        [176u8, 50, 44],
        [176, 50, 44],
        [236, 232, 222],
        [130, 110, 90],
        [60, 100, 70],
    ]);
    let roof = rng.pick(&[[90u8, 92, 100], [120, 124, 130], [150, 90, 70]]);
    o.bx(-5.0, -6.0, 0.0, 5.0, 6.0, 5.0, red);
    if rng.chance(0.4) {
        o.hip(-5.0, -6.0, 5.0, 6.0, 5.0, 3.0, roof, 0.4);
    } else {
        o.gable(-5.0, -6.0, 5.0, 6.0, 5.0, 4.0, roof, red, false, 0.4);
    }
    o.win_s(-1.6, 1.6, 0.0, 3.6, -6.0, shade(red, 0.7));
    let silos = rng.pick(&[1usize, 2, 2, 3]);
    for i in 0..silos {
        let (x, y) = (9.0 + i as f32 * 3.8, 2.0 - i as f32 * 1.0);
        let sc = rng.pick(&[[210u8, 210, 214], [190, 196, 204], [196, 150, 100]]);
        o.prism(x, y, 0.0, 13.0 - i as f32 * 1.5, 2.6, sc, 10, None);
        o.pyramid(
            x,
            y,
            13.0 - i as f32 * 1.5,
            2.7,
            2.5,
            [150, 156, 166],
            10,
            18.0,
        );
    }
    if rng.chance(0.4) {
        o.bx(-11.0, 4.0, 0.0, -8.0, 7.0, 1.2, [200, 180, 100]);
        o.bx(-11.0, 7.2, 0.0, -8.0, 10.2, 1.2, [200, 180, 100]);
    }
    if rng.chance(0.3) {
        o.bx(
            -12.0,
            -4.0,
            0.0,
            -10.0,
            -1.0,
            1.4,
            rng.pick(&[[196u8, 58, 52], [60, 130, 96], [226, 190, 60]]),
        );
        o.bx(-11.8, -4.4, 0.0, -10.2, -3.8, 0.9, [40, 40, 46]);
    }
    let h = crate::kit::house(region, rng);
    o.add_placed(&h, -12.0, -10.0, 0.0);
    o
}
