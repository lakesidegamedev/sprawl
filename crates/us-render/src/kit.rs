//! The building kit: one builder per lot kind, ported from the approved mockup kit. Each model
//! is in local metres with its front door facing south (-y), centred on the origin.

use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use us_gen::LotKind;

pub const GLASS: Rgb = [98, 156, 204];
pub const GLASS_D: Rgb = [64, 88, 118];
pub const DOOR: Rgb = [96, 62, 44];
pub const WHITE: Rgb = [238, 234, 222];
pub const CONC: Rgb = [176, 172, 164];
pub const TRIM: Rgb = [226, 220, 206];
pub const WALLS: [Rgb; 6] = [
    [244, 216, 150],
    [160, 204, 228],
    [240, 180, 140],
    [186, 222, 160],
    [246, 236, 214],
    [218, 186, 226],
];
pub const ROOFS: [Rgb; 5] = [
    [196, 70, 52],
    [84, 96, 124],
    [150, 84, 52],
    [52, 120, 100],
    [176, 120, 60],
];
pub const BRICK: [Rgb; 4] = [[170, 86, 64], [150, 72, 58], [186, 112, 78], [128, 92, 80]];
pub const TREE: Rgb = [88, 150, 76];
pub const TREE_D: Rgb = [70, 128, 66];
pub const CAR_COLS: [Rgb; 6] = [
    [196, 58, 52],
    [48, 84, 150],
    [226, 226, 222],
    [60, 62, 70],
    [230, 190, 60],
    [60, 130, 96],
];

pub fn tree(r: f32, h: f32, col: Rgb) -> Obj {
    let mut o = Obj::new();
    o.bx(-0.25, -0.25, 0.0, 0.25, 0.25, h * 0.35, [110, 80, 56]);
    o.prism(0.0, 0.0, h * 0.3, h * 0.75, r, col, 6, None);
    o.pyramid(0.0, 0.0, h * 0.75, r, h * 0.3, col, 6, 0.0);
    // fallen leaves: a flat triangle on the ground that only shows in autumn (the shader reads
    // the blue channel as the tree's colour step)
    let step = col[2]
        .wrapping_sub(TREE[2])
        .min(col[2].wrapping_sub(TREE_D[2]))
        .min(3);
    let a = 0.9 * r + col[2] as f32 * 1.7;
    let pt = |k: f32| {
        let ang = a + k * 2.094;
        [r * 1.6 * libm::cosf(ang), r * 1.6 * libm::sinf(ang), 0.05]
    };
    o.poly(
        &[pt(0.0), pt(1.0), pt(2.0)],
        leaf_patch_colour(step),
        false,
        false,
        true,
    );
    o
}

/// The colour of a fallen leaf patch for tree colour step 0 to 3.
fn leaf_patch_colour(step: u8) -> Rgb {
    [250, 126, 16 + step]
}

pub fn house(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (w, d) = rng.pick(&[(8.0f32, 9.0f32), (9.0, 8.0), (10.0, 8.0)]);
    let (wall, roof) = (rng.pick(&WALLS), rng.pick(&ROOFS));
    let (x0, x1, y0, y1) = (-w / 2.0, w / 2.0, -d / 2.0, d / 2.0);
    o.bx(x0, y0, 0.0, x1, y1, 3.0, wall);
    let along_x = rng.chance(0.6);
    o.gable(x0, y0, x1, y1, 3.0, 2.6, roof, wall, along_x, 0.4);
    o.bx(x1 - 2.2, 0.5, 3.2, x1 - 1.4, 1.3, 6.0, [150, 90, 70]);
    o.win_s(-0.6, 0.6, 0.0, 2.1, y0, DOOR);
    for cx in [x0 + 1.8, x1 - 1.8] {
        o.win_s(cx - 0.7, cx + 0.7, 1.0, 2.2, y0, GLASS);
    }
    o.win_w(-1.5, 0.0, 1.0, 2.2, x0, GLASS);
    o.win_e(-1.5, 0.0, 1.0, 2.2, x1, GLASS);
    o.bx(-1.1, y0 - 1.2, 0.0, 1.1, y0, 0.25, CONC);
    if rng.chance(0.5) {
        o.bx_top(
            x1,
            y0 + 1.0,
            0.0,
            x1 + 3.6,
            y0 + 6.5,
            2.6,
            wall,
            [150, 150, 150],
        );
        o.win_s(x1 + 0.4, x1 + 3.2, 0.0, 2.1, y0 + 1.0, [200, 200, 196]);
    }
    o
}

pub fn row_houses(n: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (uw, d) = (5.5f32, 10.0f32);
    let mut x = -(n as f32) * uw / 2.0;
    let mut all: Vec<Rgb> = BRICK.to_vec();
    all.extend_from_slice(&WALLS[..2]);
    for _ in 0..n {
        let wall = rng.pick(&all);
        let h = rng.pick(&[6.0f32, 6.0, 6.6]);
        o.bx_top(x, -d / 2.0, 0.0, x + uw, d / 2.0, h, wall, [110, 104, 100]);
        o.bx(
            x - 0.05,
            -d / 2.0 - 0.3,
            h - 0.5,
            x + uw + 0.05,
            -d / 2.0,
            h + 0.15,
            TRIM,
        );
        o.win_s(x + 0.7, x + 1.8, 0.0, 2.2, -d / 2.0, DOOR);
        o.win_s(x + 2.6, x + 4.6, 0.9, 2.3, -d / 2.0, GLASS);
        for cx in [x + 1.6, x + 3.9] {
            o.win_s(cx - 0.6, cx + 0.6, 3.6, 5.2, -d / 2.0, GLASS);
        }
        o.bx(x + 0.5, -d / 2.0 - 1.4, 0.0, x + 2.0, -d / 2.0, 0.5, CONC);
        x += uw;
    }
    o.win_w(-2.5, 1.0, 3.6, 5.2, -(n as f32) * uw / 2.0, GLASS);
    o
}

pub fn shop(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (w, d, h) = (12.0f32, 10.0f32, 7.2f32);
    let wall = rng.pick(&BRICK);
    let (x0, x1, y0, y1) = (-w / 2.0, w / 2.0, -d / 2.0, d / 2.0);
    o.bx_top(x0, y0, 0.0, x1, y1, h, wall, [120, 116, 112]);
    o.bx_top(
        x0 - 0.1,
        y0 - 0.25,
        h - 0.2,
        x1 + 0.1,
        y1 + 0.1,
        h + 0.6,
        TRIM,
        TRIM,
    );
    o.win_s(x0 + 0.6, x1 - 0.6, 0.0, 3.2, y0, GLASS_D);
    o.win_s(-0.8, 0.8, 0.0, 2.6, y0, DOOR);
    let sign = rng.pick(&[[40u8, 92, 70], [36, 60, 110], [130, 40, 40]]);
    o.win_s(x0 + 0.6, x1 - 0.6, 3.5, 4.3, y0, sign);
    for cx in [-3.5f32, 0.0, 3.5] {
        o.win_s(cx - 0.8, cx + 0.8, 4.9, 6.4, y0, GLASS);
    }
    for cy in [-2.0f32, 2.0] {
        o.win_w(cy - 0.8, cy + 0.8, 4.9, 6.4, x0, GLASS);
    }
    o.win_w(-4.0, 3.0, 0.4, 3.0, x0, GLASS_D);
    let stripes = 8;
    for i in 0..stripes {
        let a = x0 + 0.4 + i as f32 * (w - 0.8) / stripes as f32;
        let b = a + (w - 0.8) / stripes as f32;
        let col = if i % 2 == 1 { WHITE } else { [196, 58, 52] };
        o.poly(
            &[
                [a, y0 - 2.0, 2.7],
                [b, y0 - 2.0, 2.7],
                [b, y0, 3.5],
                [a, y0, 3.5],
            ],
            col,
            true,
            true,
            false,
        );
    }
    o
}

pub fn gas_station(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let grey = [150, 150, 146];
    let blue = [30, 120, 190];
    o.slab(-11.0, -9.0, 0.0, 11.0, 9.0, 0.12, grey);
    o.bx_top(2.0, 2.0, 0.12, 10.0, 8.0, 3.6, WHITE, [130, 130, 130]);
    o.win_s(3.0, 8.0, 0.3, 2.7, 2.0, GLASS_D);
    o.bx(1.8, 1.8, 3.6, 10.2, 8.2, 4.2, blue);
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
    for x in [-8.5f32, -1.5] {
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
    }
    o.bx_top(-10.0, -8.0, 4.6, 0.0, 0.0, 5.3, blue, WHITE);
    o.bx(9.0, -8.5, 0.12, 9.4, -8.1, 6.5, [90, 90, 96]);
    o.bx(8.2, -8.6, 6.5, 10.2, -8.0, 8.6, blue);
    o
}

pub fn church(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = [214, 206, 190];
    let slate = [86, 92, 104];
    o.bx(-5.0, -4.0, 0.0, 5.0, 12.0, 7.0, wall);
    o.gable(-5.0, -4.0, 5.0, 12.0, 7.0, 5.5, slate, wall, false, 0.3);
    o.bx(-2.6, -8.0, 0.0, 2.6, -2.8, 13.0, wall);
    o.pyramid(0.0, -5.4, 13.0, 3.7, 9.0, slate, 4, 45.0);
    o.win_s(-0.9, 0.9, 0.0, 3.0, -8.0, DOOR);
    o.win_s(-0.8, 0.8, 8.5, 11.0, -8.0, GLASS);
    for cy in [0.0f32, 4.0, 8.0] {
        o.win_w(cy - 0.6, cy + 0.6, 2.0, 5.6, -5.0, [150, 120, 170]);
    }
    o
}

pub fn school(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = [222, 160, 110];
    let roof = [124, 120, 116];
    o.bx_top(-14.0, -6.0, 0.0, 6.0, 2.0, 7.6, wall, roof);
    o.bx_top(6.0, -6.0, 0.0, 14.0, 10.0, 7.6, wall, roof);
    o.windows_box(-14.0, -6.0, 6.0, 2.0, 2, 3.8, GLASS, 2.0, 3.4);
    o.windows_box(6.0, -6.0, 14.0, 10.0, 2, 3.8, GLASS, 2.0, 3.4);
    o.bx(-3.0, -7.5, 0.0, 1.0, -6.0, 3.2, WHITE);
    o.win_s(-2.0, 0.0, 0.0, 2.6, -7.5, DOOR);
    o.bx(-10.0, -12.0, 0.0, -9.8, -11.8, 10.0, [210, 210, 210]);
    o.poly(
        &[
            [-9.8, -11.9, 8.2],
            [-7.0, -11.9, 8.2],
            [-7.0, -11.9, 9.8],
            [-9.8, -11.9, 9.8],
        ],
        [40, 90, 170],
        false,
        true,
        true,
    );
    o.slab(-13.0, 4.0, 0.0, -4.0, 12.0, 0.08, [196, 120, 90]);
    o
}

pub fn apartments(floors: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = rng.pick(&[[206u8, 196, 180], [180, 196, 204], [214, 186, 160]]);
    let (w, d, fh) = (18.0f32, 12.0f32, 3.1f32);
    let h = floors as f32 * fh;
    o.bx_top(
        -w / 2.0,
        -d / 2.0,
        0.0,
        w / 2.0,
        d / 2.0,
        h,
        wall,
        [118, 114, 110],
    );
    o.windows_box(
        -w / 2.0,
        -d / 2.0,
        w / 2.0,
        d / 2.0,
        floors,
        fh,
        GLASS,
        1.5,
        3.0,
    );
    for f in 1..floors {
        let z = f as f32 * fh;
        for cx in [-5.5f32, 0.0, 5.5] {
            o.bx(
                cx - 1.6,
                -d / 2.0 - 1.3,
                z,
                cx + 1.6,
                -d / 2.0,
                z + 0.25,
                [226, 222, 214],
            );
            o.bx(
                cx - 1.6,
                -d / 2.0 - 1.3,
                z + 0.25,
                cx + 1.6,
                -d / 2.0 - 1.15,
                z + 1.1,
                [90, 96, 104],
            );
        }
    }
    o.bx(-3.0, -2.0, h, 3.0, 2.0, h + 2.4, [150, 146, 140]);
    o.win_s(-1.2, 1.2, 0.0, 2.6, -d / 2.0, DOOR);
    o
}

pub fn office(floors: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (w, d, fh) = (16.0f32, 16.0f32, 3.6f32);
    let h = floors as f32 * fh;
    let frame = rng.pick(&[[196u8, 202, 210], [210, 196, 170], [120, 132, 150]]);
    let roof = [110, 112, 118];
    o.bx_top(-w / 2.0, -d / 2.0, 0.0, w / 2.0, d / 2.0, h, frame, roof);
    for f in 0..floors {
        let (za, zb) = (f as f32 * fh + 0.9, f as f32 * fh + 3.1);
        o.win_s(-w / 2.0 + 0.6, w / 2.0 - 0.6, za, zb, -d / 2.0, GLASS);
        o.win_w(-d / 2.0 + 0.6, d / 2.0 - 0.6, za, zb, -w / 2.0, GLASS);
        o.win_n(-w / 2.0 + 0.6, w / 2.0 - 0.6, za, zb, d / 2.0, GLASS);
        o.win_e(-d / 2.0 + 0.6, d / 2.0 - 0.6, za, zb, w / 2.0, GLASS);
    }
    o.bx_top(-5.0, -5.0, h, 5.0, 5.0, h + 4.0, frame, roof);
    o.bx(-2.0, 1.0, h + 4.0, 2.0, 3.5, h + 6.0, [150, 150, 150]);
    o.bx(
        -w / 2.0 - 1.0,
        -d / 2.0 - 3.0,
        3.4,
        w / 2.0 + 1.0,
        -d / 2.0,
        3.8,
        [60, 64, 72],
    );
    o
}

pub fn town_hall(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let stone = [230, 222, 200];
    let teal = [90, 150, 140];
    o.bx_top(-15.0, -8.0, 0.0, 15.0, 8.0, 9.0, stone, [126, 124, 120]);
    o.bx(-15.3, -8.3, 8.6, 15.3, 8.3, 9.5, TRIM);
    o.windows_box(-15.0, -8.0, 15.0, 8.0, 2, 4.5, GLASS, 1.4, 3.0);
    o.bx(-6.0, -12.0, 0.0, 6.0, -8.0, 0.6, CONC);
    for x in [-5.0f32, -2.5, 0.0, 2.5, 5.0] {
        o.prism(x, -10.5, 0.6, 7.8, 0.45, WHITE, 6, None);
    }
    o.bx(-6.2, -12.0, 7.8, 6.2, -8.0, 8.6, stone);
    o.gable(-6.2, -12.0, 6.2, -8.0, 8.6, 2.6, stone, stone, true, 0.2);
    o.bx(-3.2, -3.2, 9.0, 3.2, 3.2, 17.0, stone);
    o.poly(
        &[
            [-1.4, -3.26, 13.0],
            [1.4, -3.26, 13.0],
            [1.4, -3.26, 15.6],
            [-1.4, -3.26, 15.6],
        ],
        WHITE,
        false,
        false,
        true,
    );
    o.bx(-3.5, -3.5, 17.0, 3.5, 3.5, 17.6, TRIM);
    o.prism(0.0, 0.0, 17.6, 20.0, 2.6, teal, 8, None);
    o.pyramid(0.0, 0.0, 20.0, 2.7, 3.2, teal, 8, 22.5);
    o.bx(-0.1, -0.1, 23.0, 0.1, 0.1, 26.5, [80, 80, 80]);
    o.poly(
        &[
            [0.1, 0.0, 25.0],
            [2.4, 0.0, 25.0],
            [2.4, 0.0, 26.4],
            [0.1, 0.0, 26.4],
        ],
        [40, 90, 170],
        false,
        true,
        true,
    );
    o.win_s(-1.2, 1.2, 0.6, 3.6, -8.0, DOOR);
    o
}

pub fn fire_station(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let brick = [170, 70, 56];
    let roof = [110, 104, 100];
    o.bx_top(-10.0, -6.0, 0.0, 10.0, 8.0, 6.4, brick, roof);
    for cx in [-6.0f32, 0.0, 6.0] {
        o.win_s(cx - 2.2, cx + 2.2, 0.0, 4.4, -6.0, [200, 44, 40]);
    }
    o.win_s(-9.4, 9.4, 4.9, 5.8, -6.0, WHITE);
    o.bx_top(7.0, 3.0, 6.4, 10.0, 8.0, 13.0, brick, roof);
    o.win_w(4.6, 6.4, 9.0, 11.5, 7.0, GLASS);
    o
}

pub fn factory(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = [178, 168, 150];
    o.bx(-14.0, -8.0, 0.0, 14.0, 8.0, 6.0, wall);
    for i in 0..5 {
        // sawtooth roof, glazed faces toward the west
        let x = -14.0 + i as f32 * 5.6;
        let s = o.solids.len() as u32;
        let _ = s;
        o.poly(
            &[[x, -8.0, 6.0], [x + 5.6, -8.0, 6.0], [x, -8.0, 9.0]],
            wall,
            true,
            true,
            false,
        );
        o.poly(
            &[[x + 5.6, 8.0, 6.0], [x, 8.0, 6.0], [x, 8.0, 9.0]],
            wall,
            true,
            true,
            false,
        );
        o.poly(
            &[
                [x + 5.6, -8.0, 6.0],
                [x + 5.6, 8.0, 6.0],
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
            GLASS,
            true,
            true,
            false,
        );
    }
    o.prism(11.0, 4.0, 0.0, 20.0, 1.3, [160, 90, 70], 8, None);
    o.win_s(-12.0, -6.0, 0.0, 4.5, -8.0, [120, 126, 136]);
    o.win_s(-3.0, 3.0, 0.0, 4.5, -8.0, [120, 126, 136]);
    o.slab(-14.0, -16.0, 0.0, -2.0, -9.0, 0.1, [150, 150, 146]);
    o
}

pub fn water_tower(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    for (x, y) in [(-2.5f32, -2.5f32), (2.5, -2.5), (2.5, 2.5), (-2.5, 2.5)] {
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
    o.prism(0.0, 0.0, 14.0, 19.0, 4.2, [210, 214, 220], 10, None);
    o.pyramid(0.0, 0.0, 19.0, 4.3, 2.2, [150, 160, 172], 10, 18.0);
    o.poly(
        &[
            [-2.5, -4.0, 15.5],
            [2.5, -4.0, 15.5],
            [2.5, -4.0, 17.4],
            [-2.5, -4.0, 17.4],
        ],
        [40, 90, 170],
        false,
        true,
        true,
    );
    o
}

pub fn car(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&CAR_COLS);
    o.bx(-2.1, -0.9, 0.3, 2.1, 0.9, 1.0, col);
    o.bx_top(-1.1, -0.8, 1.0, 1.0, 0.8, 1.55, [70, 96, 120], col);
    o
}

pub fn parking(w: f32, d: f32, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let asphalt = [92, 94, 102];
    o.slab(-w / 2.0, -d / 2.0, 0.0, w / 2.0, d / 2.0, 0.12, asphalt);
    let n = (w / 3.0) as usize;
    let line = [230, 230, 226];
    for i in 0..=n {
        let x = -w / 2.0 + 1.0 + i as f32 * (w - 2.0) / n as f32;
        for (y0, y1) in [
            (-d / 2.0 + 0.6, -d / 2.0 + 5.4),
            (d / 2.0 - 5.4, d / 2.0 - 0.6),
        ] {
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
    for i in 0..n {
        let x = -w / 2.0 + 1.0 + (i as f32 + 0.5) * (w - 2.0) / n as f32;
        for y in [-d / 2.0 + 3.0, d / 2.0 - 3.0] {
            if rng.chance(0.6) {
                let c = car(rng);
                o.add_placed(&c, x, y, std::f32::consts::FRAC_PI_2);
            }
        }
    }
    o
}

pub fn park(w: f32, d: f32, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let green = [104, 168, 84];
    o.slab(-w / 2.0, -d / 2.0, 0.0, w / 2.0, d / 2.0, 0.15, green);
    let path = [222, 210, 170];
    let mut strip = |a: [f32; 4]| {
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
    strip([-w / 2.0, -1.2, w / 2.0, 1.2]);
    strip([-1.2, -d / 2.0, 1.2, d / 2.0]);
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
    for x in [-5.0f32, 5.0] {
        o.bx(x - 0.9, 2.0, 0.15, x + 0.9, 2.5, 0.6, [130, 90, 60]);
    }
    o
}

pub fn sports(w: f32, d: f32, _rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    o.slab(
        -w / 2.0,
        -d / 2.0,
        0.0,
        w / 2.0,
        d / 2.0,
        0.15,
        [92, 160, 76],
    );
    o.poly(
        &[
            [-w / 2.0 + 2.0, -d / 2.0 + 2.0, 0.2],
            [w / 2.0 - 2.0, -d / 2.0 + 2.0, 0.2],
            [w / 2.0 - 2.0, d / 2.0 - 2.0, 0.2],
            [-w / 2.0 + 2.0, d / 2.0 - 2.0, 0.2],
        ],
        [78, 150, 70],
        false,
        false,
        true,
    );
    o.poly(
        &[
            [-0.15, -d / 2.0 + 2.0, 0.24],
            [0.15, -d / 2.0 + 2.0, 0.24],
            [0.15, d / 2.0 - 2.0, 0.24],
            [-0.15, d / 2.0 - 2.0, 0.24],
        ],
        [240, 240, 240],
        false,
        false,
        true,
    );
    for x in [-w / 2.0 + 2.5, w / 2.0 - 2.5] {
        o.bx(x - 0.1, -2.0, 0.15, x + 0.1, 2.0, 2.4, [240, 240, 240]);
    }
    o
}

pub fn strip_mall(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    o.bx_top(
        -20.0,
        -6.0,
        0.0,
        20.0,
        8.0,
        5.2,
        [226, 214, 190],
        [130, 128, 124],
    );
    for i in 0..5 {
        let x0 = -19.0 + i as f32 * 7.8;
        o.win_s(x0 + 0.6, x0 + 7.0, 0.0, 3.0, -6.0, GLASS_D);
        let sign = rng.pick(&[
            [196u8, 58, 52],
            [40, 92, 160],
            [40, 130, 90],
            [230, 170, 40],
        ]);
        o.win_s(x0 + 0.6, x0 + 7.0, 3.5, 4.6, -6.0, sign);
    }
    o.bx(-20.2, -8.5, 3.2, 20.2, -6.0, 3.5, [196, 190, 178]);
    let p = parking(40.0, 14.0, rng);
    o.add_placed(&p, 0.0, -17.0, 0.0);
    o
}

pub fn big_box(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&[[210u8, 206, 196], [190, 200, 210]]);
    o.bx_top(-32.0, 2.0, 0.0, 32.0, 42.0, 9.0, col, [150, 150, 150]);
    o.win_s(-8.0, 8.0, 0.0, 4.0, 2.0, GLASS_D);
    let sign = rng.pick(&[[40u8, 92, 160], [196, 58, 52], [230, 170, 40]]);
    o.win_s(-26.0, 26.0, 6.6, 8.4, 2.0, sign);
    for x in [-20.0f32, -10.0, 10.0, 20.0] {
        o.bx(x - 0.4, 34.0, 9.0, x + 0.4, 38.0, 11.0, [170, 170, 170]);
    }
    let pk = parking(64.0, 36.0, rng);
    o.add_placed(&pk, 0.0, -18.0, 0.0);
    o
}

pub fn warehouse(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&[[200u8, 196, 186], [176, 184, 194], [214, 200, 176]]);
    o.bx_top(-26.0, -16.0, 0.0, 26.0, 16.0, 10.0, col, [140, 140, 144]);
    let mut x = -22.0f32;
    while x <= 22.0 {
        o.win_s(x - 2.0, x + 2.0, 0.0, 4.4, -16.0, [120, 126, 136]);
        x += 7.0;
    }
    o.slab(-26.0, -26.0, 0.0, 26.0, -16.0, 0.1, [150, 150, 146]);
    for x in [-15.0f32, 0.0, 15.0] {
        o.bx(x - 1.3, -24.0, 0.1, x + 1.3, -18.0, 3.6, [236, 236, 236]);
    }
    o
}

pub fn fast_food(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&[[220u8, 60, 50], [240, 200, 60], [60, 120, 190]]);
    o.bx_top(-6.0, -4.0, 0.0, 6.0, 5.0, 4.2, col, [120, 116, 112]);
    o.win_s(-5.0, 5.0, 0.3, 3.0, -4.0, GLASS_D);
    o.bx(7.0, -6.0, 0.0, 7.4, -5.6, 7.0, [90, 90, 96]);
    o.bx(6.0, -6.4, 7.0, 9.4, -5.2, 9.4, [240, 200, 60]);
    let pk = parking(14.0, 12.0, rng);
    o.add_placed(&pk, -3.0, 12.0, 0.0);
    o
}

pub fn farm(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let red = [176, 50, 44];
    o.bx(-5.0, -6.0, 0.0, 5.0, 6.0, 5.0, red);
    o.gable(
        -5.0,
        -6.0,
        5.0,
        6.0,
        5.0,
        4.0,
        [90, 92, 100],
        red,
        false,
        0.4,
    );
    o.prism(9.0, 2.0, 0.0, 13.0, 2.6, [210, 210, 214], 10, None);
    o.pyramid(9.0, 2.0, 13.0, 2.7, 2.5, [150, 156, 166], 10, 18.0);
    let h = house(rng);
    o.add_placed(&h, -12.0, -10.0, 0.0);
    o
}

/// The model for one lot, from its kind and a seeded RNG.
pub fn build_lot(kind: LotKind, rng: &mut Rng) -> Obj {
    match kind {
        LotKind::House => house(rng),
        LotKind::Row => {
            let n = 2 + rng.below(3);
            row_houses(n, rng)
        }
        LotKind::Shop => shop(rng),
        LotKind::Apartments => {
            let f = 4 + rng.below(3);
            apartments(f, rng)
        }
        LotKind::Office => {
            let f = rng.pick(&[7usize, 9, 12]);
            office(f, rng)
        }
        LotKind::Gas => gas_station(rng),
        LotKind::Workshop => factory(rng),
        LotKind::Church => church(rng),
        LotKind::School => school(rng),
        LotKind::Fire => fire_station(rng),
        LotKind::Water => water_tower(rng),
        LotKind::Parking => parking(20.0, 16.0, rng),
        LotKind::Park => park(26.0, 24.0, rng),
        LotKind::Sports => sports(30.0, 20.0, rng),
        LotKind::Strip => strip_mall(rng),
        LotKind::BigBox => big_box(rng),
        LotKind::Warehouse => warehouse(rng),
        LotKind::FastFood => fast_food(rng),
        LotKind::Farm => farm(rng),
    }
}

pub const ALL_KINDS: [LotKind; 19] = [
    LotKind::House,
    LotKind::Row,
    LotKind::Shop,
    LotKind::Apartments,
    LotKind::Office,
    LotKind::Gas,
    LotKind::Workshop,
    LotKind::Church,
    LotKind::School,
    LotKind::Fire,
    LotKind::Water,
    LotKind::Parking,
    LotKind::Park,
    LotKind::Sports,
    LotKind::Strip,
    LotKind::BigBox,
    LotKind::Warehouse,
    LotKind::FastFood,
    LotKind::Farm,
];

/// A tree colour with one of four tiny blue-channel steps picked from where the tree stands, so
/// autumn can give each tree its own leaf colour. Differences are invisible in summer.
pub fn tree_variant(col: Rgb, x: f32, y: f32) -> Rgb {
    let h = ((x * 7.0) as i32).wrapping_mul(31) ^ ((y * 13.0) as i32).wrapping_mul(17);
    [col[0], col[1], col[2] + (h & 3) as u8]
}
