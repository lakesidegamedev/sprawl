//! More building looks for the kit: shops and services, townhouses, mid-rise and glass towers,
//! factories, yards and a few civic look-alikes. `variant` is the single entry point: given a lot
//! kind, level, density and the seeded roll it either returns one of these models or `None`, in
//! which case the base kit builds the lot. Every model is in local metres, front door facing
//! south (-y), centred on the origin, and drawn only from the seeded `Rng`.

use crate::kit::{BRICK, CAR_COLS, DOOR, TREE, car, parking};
use crate::obj::{Obj, Rgb};
use crate::parts::*;
use crate::rng::Rng;
use us_gen::LotKind;

/// Evergreen leaf colour: not recoloured by the seasons.
pub const PINE: Rgb = [46, 104, 72];
const DARK: Rgb = [58, 62, 72];
const STEEL: Rgb = [150, 156, 164];
const RED: Rgb = [196, 58, 52];

fn sign_band(o: &mut Obj, x0: f32, x1: f32, y: f32, z0: f32, z1: f32, col: Rgb) {
    o.win_s(x0, x1, z0, z1, y, col);
    let n = (((x1 - x0) / 1.6) as usize).max(2);
    for i in 0..n {
        let a = x0 + 0.4 + (x1 - x0 - 0.8) * i as f32 / n as f32;
        o.win_s(a, a + 0.5, z0 + 0.25, z1 - 0.25, y - 0.02, WHITE);
    }
}

fn wall_pick(region: Region, rng: &mut Rng) -> (Rgb, Rgb) {
    let m = material(region, rng);
    wall_colours(m, rng)
}

fn flat_roof(o: &mut Obj, b: (f32, f32, f32, f32), z: f32, wall: Rgb) {
    o.parapet(b.0, b.1, b.2, b.3, z, 0.6, wall, TRIM, deck_of(wall));
}

fn rooftop_gear(o: &mut Obj, b: (f32, f32, f32, f32), z: f32, n: usize, rng: &mut Rng) {
    for _ in 0..n {
        let cx = rng.range(b.0 + 2.0, b.2 - 2.0);
        let cy = rng.range(b.1 + 2.0, b.3 - 2.0);
        let s = rng.range(0.7, 1.4);
        o.bx_top(
            cx - s,
            cy - s,
            z,
            cx + s,
            cy + s,
            z + 1.3,
            STEEL,
            [118, 116, 112],
        );
    }
}

// ---- commercial ----

pub fn supermarket(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    let wall = mix(wall, WHITE, 0.4);
    let (w, d, h) = (rng.range(8.5, 10.0), rng.range(5.5, 6.5), 5.0);
    o.bx(-w, -d, 0.0, w, d, h, wall);
    flat_roof(&mut o, (-w, -d, w, d), h, wall);
    o.win_s(-w + 1.0, -1.8, 0.4, 3.4, -d, GLASS);
    o.win_s(1.8, w - 1.0, 0.4, 3.4, -d, GLASS);
    o.win_s(-1.5, 1.5, 0.2, 3.0, -d, DOOR);
    o.bx(
        -2.6,
        -d - 1.8,
        3.2,
        2.6,
        -d,
        3.5,
        rng.pick(&[RED, [40, 110, 170], [60, 130, 80]]),
    );
    sign_band(&mut o, -w + 1.0, w - 1.0, -d, 3.8, 4.8, rng.pick(&SIGNS));
    rooftop_gear(&mut o, (-w, -d, w, d), h + 0.6, 3, rng);
    // trolley bay
    o.slab(
        w - 3.5,
        -d - 2.0,
        0.0,
        w - 1.0,
        -d - 0.5,
        0.9,
        [130, 134, 142],
    );
    let pk = parking(2.0 * w, 8.0, rng);
    o.add_placed(&pk, 0.0, -d - 6.0, 0.0);
    o
}

pub fn bank(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let stone = rng.pick(&[STONE, [226, 220, 206], [196, 188, 176]]);
    let _ = region;
    o.bx(-6.0, -5.0, 0.0, 6.0, 5.0, 6.5, stone);
    o.bx(-6.4, -5.4, 6.5, 6.4, 5.4, 6.9, TRIM);
    o.slab(-6.0, -6.5, 0.0, 6.0, -5.0, 0.5, CONC);
    for i in 0..4 {
        let x = -4.5 + i as f32 * 3.0;
        o.prism(x, -5.8, 0.5, 6.4, 0.5, WHITE, 8, Some(WHITE));
    }
    // pediment
    o.gable(
        -5.2,
        -6.6,
        5.2,
        -4.8,
        6.9,
        1.8,
        mix(stone, WHITE, 0.4),
        stone,
        true,
        0.2,
    );
    o.win_s(-0.9, 0.9, 0.5, 3.6, -5.0, DOOR);
    for x in [-4.2f32, 4.2] {
        o.win_s(x - 0.7, x + 0.7, 1.6, 4.6, -5.0, GLASS);
    }
    o.win_e(-3.0, 3.0, 1.6, 4.6, 6.0, GLASS);
    o.win_w(-3.0, 3.0, 1.6, 4.6, -6.0, GLASS);
    let flag = rng.pick(&[RED, [40, 92, 160]]);
    o.bx(5.5, -6.0, 0.0, 5.65, -5.85, 6.0, [90, 94, 100]);
    o.bx(5.65, -6.0, 4.4, 7.0, -5.9, 5.9, flag);
    o
}

fn hotel_body(
    o: &mut Obj,
    region: Region,
    rng: &mut Rng,
    (hw, hd): (f32, f32),
    floors: usize,
    fh: f32,
) {
    let (wall, _) = wall_pick(region, rng);
    let wall = mix(wall, WHITE, 0.3);
    let lobby = 4.2;
    let h = lobby + floors as f32 * fh;
    o.bx(-hw, -hd, 0.0, hw, hd, lobby, mix(wall, DARK, 0.4));
    o.bx(-hw, -hd, lobby, hw, hd, h, wall);
    o.win_s(-hw + 0.8, hw - 0.8, 0.4, 3.4, -hd, GLASS_D);
    o.win_s(-1.4, 1.4, 0.3, 3.0, -hd, GLASS);
    o.bx(
        -3.0,
        -hd - 3.0,
        3.6,
        3.0,
        -hd,
        3.9,
        rng.pick(&[RED, [150, 110, 50], [40, 92, 160]]),
    );
    box_windows(
        o,
        (-hw, -hd, hw, hd),
        lobby,
        floors,
        fh,
        Pattern::Grid,
        GLASS,
        GLASS_D,
        15,
    );
    flat_roof(o, (-hw, -hd, hw, hd), h, wall);
    rooftop_gear(o, (-hw, -hd, hw, hd), h + 0.6, 2, rng);
    // roof sign
    o.bx(
        -3.0,
        hd - 1.5,
        h + 0.6,
        3.0,
        hd - 1.2,
        h + 2.6,
        rng.pick(&SIGNS),
    );
}

pub fn hotel_mid(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let f = 4 + rng.below(3);
    hotel_body(&mut o, region, rng, (8.0, 5.0), f, 3.1);
    if rng.chance(0.5) {
        let pk = parking(14.0, 6.0, rng);
        o.add_placed(&pk, 0.0, -9.0, 0.0);
    }
    o
}

pub fn hotel_tall(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let f = 11 + rng.below(6);
    hotel_body(&mut o, region, rng, (6.5, 5.0), f, 3.1);
    o
}

pub fn cinema(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    let wall = mix(wall, DARK, 0.45);
    o.bx(-8.0, -2.0, 0.0, 8.0, 7.0, 10.0, wall);
    o.bx(-8.0, -6.0, 0.0, 8.0, -2.0, 4.6, mix(wall, WHITE, 0.2));
    o.win_s(-6.5, 6.5, 0.3, 3.2, -6.0, GLASS_D);
    o.win_s(-1.2, 1.2, 0.2, 2.8, -6.0, DOOR);
    // marquee and poster board
    o.bx(-5.0, -8.0, 3.8, 5.0, -6.0, 4.2, [236, 200, 60]);
    o.bx(-5.0, -8.0, 4.2, 5.0, -7.8, 5.4, rng.pick(&SIGNS));
    for i in 0..4 {
        let x = -4.2 + i as f32 * 2.7;
        o.win_s(
            x,
            x + 1.8,
            1.0,
            3.0,
            -6.02,
            rng.pick(&[RED, [40, 92, 160], [236, 200, 60]]),
        );
    }
    o.parapet(-8.0, -6.0, 8.0, 7.0, 10.0, 0.5, wall, TRIM, deck_of(wall));
    o.bx(5.5, -6.6, 0.0, 5.7, -6.4, 9.0, [90, 94, 100]);
    o
}

pub fn office_park(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    let n = 2 + rng.below(2);
    for i in 0..n {
        let x = -10.0 + i as f32 * 10.0;
        let floors = 2 + rng.below(2);
        let h = floors as f32 * 3.4;
        o.bx(x - 4.0, -3.0, 0.0, x + 4.0, 3.0, h, mix(wall, WHITE, 0.3));
        box_windows(
            &mut o,
            (x - 4.0, -3.0, x + 4.0, 3.0),
            0.0,
            floors,
            3.4,
            Pattern::Ribbon,
            GLASS,
            GLASS_D,
            15,
        );
        flat_roof(&mut o, (x - 4.0, -3.0, x + 4.0, 3.0), h, wall);
    }
    o.slab(
        -14.0,
        -3.0,
        0.0,
        -10.0 + 10.0 * n as f32 - 6.0,
        3.0,
        0.0,
        TRIM,
    );
    let pk = parking(24.0, 8.0, rng);
    o.add_placed(&pk, 0.0, -9.0, 0.0);
    for i in 0..4 {
        let x = -8.0 + i as f32 * 5.5;
        o.prism(x, -14.5, 0.0, 2.8, 1.2, TREE, 6, None);
    }
    o
}

pub fn car_dealership(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    o.bx(-9.0, 1.0, 0.0, 9.0, 7.0, 4.2, wall);
    o.win_s(-8.0, 8.0, 0.3, 3.4, 1.0, GLASS);
    flat_roof(&mut o, (-9.0, 1.0, 9.0, 7.0), 4.2, wall);
    sign_band(&mut o, -6.0, 6.0, 1.0, 3.5, 4.1, rng.pick(&SIGNS));
    o.slab(-11.0, -9.0, 0.0, 11.0, 1.0, 0.1, [92, 94, 102]);
    for i in 0..5 {
        let x = -8.0 + i as f32 * 4.2;
        let mut c = car(rng);
        let col = rng.pick(&CAR_COLS);
        for p in &mut c.polys {
            if p.color == CAR_COLS[0] || p.color == col {
                p.color = col;
            }
        }
        o.add_placed(&c, x, -3.0, std::f32::consts::FRAC_PI_2);
    }
    // flag line
    for i in 0..6 {
        let x = -10.0 + i as f32 * 4.0;
        o.bx(x, -8.8, 0.0, x + 0.12, -8.68, 4.0, [90, 94, 100]);
        o.bx(
            x + 0.12,
            -8.8,
            3.0,
            x + 1.2,
            -8.7,
            3.9,
            rng.pick(&[RED, [236, 200, 60], WHITE]),
        );
    }
    o
}

pub fn diner(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let trim = rng.pick(&[RED, [40, 110, 170], [60, 130, 110]]);
    o.bx(-8.0, -3.0, 0.0, 8.0, 3.0, 3.4, [214, 218, 224]);
    o.bx(-8.0, -3.0, 1.0, 8.0, -2.9, 1.5, trim);
    o.win_s(-7.0, 7.0, 1.6, 3.0, -3.0, GLASS);
    o.win_s(-1.0, 1.0, 0.2, 2.6, -3.0, DOOR);
    // barrel roof as three stepped boxes
    o.bx(-8.2, -3.2, 3.4, 8.2, 3.2, 3.8, trim);
    o.bx(-7.8, -2.4, 3.8, 7.8, 2.4, 4.1, [226, 228, 232]);
    o.bx(-7.4, -1.4, 4.1, 7.4, 1.4, 4.3, [236, 238, 240]);
    o.bx(-0.2, -0.2, 4.3, 0.2, 0.2, 7.0, [90, 90, 96]);
    o.bx(-2.5, -0.3, 6.2, 2.5, 0.3, 7.4, [236, 200, 60]);
    for x in [-6.0f32, -2.0, 2.0, 6.0] {
        o.bx(x - 0.4, -4.4, 0.0, x + 0.4, -3.6, 0.8, trim);
    }
    o
}

pub fn petrol_a(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let brand = rng.pick(&[RED, [40, 110, 170], [60, 130, 80], [230, 170, 40]]);
    o.slab(-9.0, -9.0, 0.0, 9.0, 7.0, 0.1, [96, 98, 106]);
    o.bx(-5.0, 2.5, 0.0, 5.0, 6.5, 3.4, [236, 234, 226]);
    o.win_s(-4.0, 4.0, 0.5, 2.6, 2.5, GLASS_D);
    o.parapet(
        -5.0,
        2.5,
        5.0,
        6.5,
        3.4,
        0.4,
        [236, 234, 226],
        brand,
        deck_of(brand),
    );
    // canopy on four posts
    o.bx(-7.0, -7.0, 4.6, 7.0, -1.0, 5.2, brand);
    o.bx(-7.0, -7.0, 5.2, 7.0, -1.0, 5.4, WHITE);
    for (x, y) in [(-6.4f32, -6.4f32), (6.0, -6.4), (-6.4, -1.6), (6.0, -1.6)] {
        o.bx(x, y, 0.0, x + 0.4, y + 0.4, 4.6, [196, 196, 200]);
    }
    for x in [-3.0f32, 3.0] {
        o.bx(x - 0.4, -4.4, 0.0, x + 0.4, -3.6, 1.5, brand);
        o.bx(x - 0.5, -4.5, 1.5, x + 0.5, -3.5, 1.7, DARK);
    }
    o.bx(8.0, -8.0, 0.0, 8.3, -7.7, 7.5, [90, 90, 96]);
    o.bx(7.2, -8.2, 6.0, 9.1, -7.5, 8.2, brand);
    o
}

pub fn petrol_b(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let brand = rng.pick(&[[236, 200, 60], RED, [40, 92, 160]]);
    o.slab(-11.0, -10.0, 0.0, 11.0, 8.0, 0.1, [84, 86, 94]);
    // convenience store with a hipped roof and a long canopy in front
    o.bx(-8.0, 3.0, 0.0, 2.0, 7.5, 3.6, rng.pick(&BRICK));
    o.hip(-8.0, 3.0, 2.0, 7.5, 3.6, 1.8, shade(brand, 0.7), 0.5);
    o.win_s(-7.0, 1.0, 0.5, 2.8, 3.0, GLASS);
    o.bx(-9.0, -6.0, 5.0, 9.0, 0.0, 5.5, brand);
    for x in [-8.0f32, 0.0, 8.0] {
        o.bx(x - 0.3, -3.3, 0.0, x + 0.3, -2.7, 5.0, [196, 196, 200]);
    }
    for x in [-5.0f32, 4.0] {
        for y in [-4.5f32, -1.5] {
            o.bx(x - 0.4, y - 0.3, 0.0, x + 0.4, y + 0.3, 1.5, brand);
        }
    }
    // air and water bay and a truck lane
    o.slab(4.0, 2.0, 0.0, 10.0, 8.0, 0.15, [70, 72, 78]);
    o.bx(9.0, 2.0, 0.0, 9.8, 2.8, 1.4, [196, 196, 200]);
    o.bx(5.0, 7.4, 0.0, 9.0, 7.8, 3.0, DARK);
    o
}

pub fn corner_store(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    let brick = rng.pick(&BRICK);
    let wall = if rng.chance(0.5) { brick } else { wall };
    o.bx(-5.0, -5.0, 0.0, 5.0, 5.0, 5.4, wall);
    o.win_s(-4.0, 4.0, 0.5, 2.9, -5.0, GLASS);
    o.win_e(-3.5, 3.5, 0.5, 2.9, 5.0, GLASS);
    o.win_s(-0.8, 0.8, 0.2, 2.5, -5.0, DOOR);
    let (a, b) = rng.pick(&AWNINGS);
    awning(&mut o, -4.4, 4.4, -5.0, 3.2, 3.8, 1.5, a, b, 8);
    for x in [-3.0f32, 0.0, 3.0] {
        o.win_s(x - 0.6, x + 0.6, 3.9, 5.0, -5.0, GLASS_D);
    }
    flat_roof(&mut o, (-5.0, -5.0, 5.0, 5.0), 5.4, wall);
    o.bx(3.0, -5.8, 0.0, 3.6, -5.2, 1.0, rng.pick(&SIGNS));
    o.bx(-4.8, -5.0, 3.9, -2.6, -4.9, 4.9, rng.pick(&SIGNS));
    o
}

pub fn pharmacy(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    let wall = mix(wall, WHITE, 0.5);
    o.bx(-6.0, -5.0, 0.0, 6.0, 5.0, 4.6, wall);
    o.win_s(-5.0, -1.2, 0.5, 3.2, -5.0, GLASS);
    o.win_s(1.2, 5.0, 0.5, 3.2, -5.0, GLASS);
    o.win_s(-0.9, 0.9, 0.2, 2.8, -5.0, DOOR);
    flat_roof(&mut o, (-6.0, -5.0, 6.0, 5.0), 4.6, wall);
    let g = [60, 150, 96];
    o.bx(-5.0, -5.0, 3.4, 5.0, -4.9, 4.4, g);
    // green cross on a pole
    o.bx(5.0, -5.6, 0.0, 5.2, -5.4, 4.2, [90, 94, 100]);
    o.bx(4.2, -5.7, 4.2, 6.0, -5.3, 4.6, g);
    o.bx(4.8, -5.7, 3.7, 5.4, -5.3, 5.1, g);
    o.slab(-6.0, 5.0, 0.0, 6.0, 12.0, 0.1, [92, 94, 102]);
    o
}

// ---- residential ----

/// A row of townhouses with its own facade: 0 bay fronts under gables, 1 flat fronts with
/// stoops, 2 painted narrow fronts with mansards, 3 brick fronts with balconies.
pub fn townhouse_row(_region: Region, facade: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let n = 3 + rng.below(3);
    let w = 3.4f32;
    let x0 = -(n as f32) * w / 2.0;
    let floors = 3;
    let h = floors as f32 * 3.0;
    let brick = rng.pick(&BRICKS);
    for i in 0..n {
        let a = x0 + i as f32 * w;
        let b = a + w;
        let c = match facade {
            1 | 3 => brick,
            _ => rng.pick(&PAINTS),
        };
        let c = if facade == 3 && i % 2 == 1 {
            shade(c, 0.92)
        } else {
            c
        };
        o.bx(a, -4.5, 0.0, b, 4.5, h, c);
        let cx = (a + b) / 2.0;
        match facade {
            0 => {
                bay(&mut o, cx, -4.5, c, TRIM, GLASS, 0.9, 2.8);
                o.win_s(cx - 0.5, cx + 0.5, 3.6, 5.6, -4.5, GLASS);
                o.win_s(cx - 0.5, cx + 0.5, 6.6, 8.4, -4.5, GLASS);
                o.gable(a, -4.5, b, 4.5, h, 1.8, rng.pick(&SHINGLES), c, false, 0.2);
            }
            1 => {
                o.slab(cx - 1.2, -5.8, 0.0, cx + 1.2, -4.5, 0.6, STONE);
                o.win_s(cx - 0.5, cx + 0.5, 0.7, 2.6, -4.5, rng.pick(&DOORS));
                for z in [3.4f32, 6.4] {
                    o.win_s(cx - 0.9, cx + 0.9, z, z + 1.8, -4.5, GLASS);
                }
                o.parapet(a, -4.5, b, 4.5, h, 0.5, c, TRIM, deck_of(c));
            }
            2 => {
                o.win_s(cx - 0.5, cx + 0.5, 0.2, 2.4, -4.5, rng.pick(&DOORS));
                o.win_s(cx - 0.8, cx + 0.8, 3.4, 5.4, -4.5, GLASS);
                o.mansard(
                    a,
                    -4.5,
                    b,
                    4.5,
                    h,
                    1.6,
                    0.9,
                    shade(rng.pick(&SHINGLES), 0.8),
                    DARK,
                );
                o.win_s(cx - 0.5, cx + 0.5, 6.4, 8.2, -4.5, GLASS);
            }
            _ => {
                o.win_s(cx - 0.5, cx + 0.5, 0.2, 2.4, -4.5, rng.pick(&DOORS));
                for z in [3.0f32, 6.0] {
                    o.win_s(cx - 0.9, cx + 0.9, z + 0.3, z + 2.0, -4.5, GLASS);
                    balcony(&mut o, cx, 1.2, -4.5, z, 0.9, TRIM, DARK);
                }
                o.parapet(a, -4.5, b, 4.5, h, 0.6, c, STONE, deck_of(c));
            }
        }
    }
    o
}

pub fn midrise_balconies(region: Region, floors: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, trim) = wall_pick(region, rng);
    let fh = 3.0;
    let h = floors as f32 * fh + 0.6;
    let (hw, hd) = (7.0, 5.0);
    o.bx(-hw, -hd, 0.0, hw, hd, h, wall);
    o.win_s(-1.2, 1.2, 0.2, 2.6, -hd, DOOR);
    let rail = rng.pick(&[DARK, WHITE, [90, 130, 150]]);
    for f in 1..floors {
        let z = 0.6 + f as f32 * fh - 0.3;
        for i in 0..3 {
            let cx = -4.6 + i as f32 * 4.6;
            o.win_s(cx - 0.9, cx + 0.9, z + 0.5, z + 2.1, -hd, GLASS);
            balcony(&mut o, cx, 1.5, -hd, z, 1.3, trim, rail);
        }
    }
    for i in 0..3 {
        let cx = -4.6 + i as f32 * 4.6;
        o.win_s(cx - 0.9, cx + 0.9, 0.6, 2.1, -hd, GLASS);
    }
    box_windows(
        &mut o,
        (-hw, -hd, hw, hd),
        0.0,
        floors,
        fh,
        Pattern::Grid,
        GLASS,
        GLASS_D,
        14,
    );
    flat_roof(&mut o, (-hw, -hd, hw, hd), h, wall);
    o.bx(-2.0, -1.5, h + 0.6, 2.0, 1.5, h + 2.4, shade(wall, 0.85));
    o
}

/// Glass tower: a plinth, a curtain wall shaft and a crown. `style` 0 slab, 1 stepped, 2 twin.
pub fn glass_tower(region: Region, floors: usize, style: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let (glass, glass2) = rng.pick(&CURTAINS);
    let fh = 3.4;
    let plinth = 5.0;
    o.bx(-7.5, -6.5, 0.0, 7.5, 6.5, plinth, mix(glass2, WHITE, 0.3));
    o.win_s(-6.5, 6.5, 0.4, 3.8, -6.5, GLASS_D);
    o.win_s(-1.2, 1.2, 0.2, 3.2, -6.5, DOOR);
    let top = |o: &mut Obj, b: (f32, f32, f32, f32), z0: f32, fl: usize| -> f32 {
        let h = z0 + fl as f32 * fh;
        o.bx_top(b.0, b.1, z0, b.2, b.3, h, glass2, mix(glass2, WHITE, 0.3));
        box_windows(o, b, z0, fl, fh, Pattern::Curtain, glass, glass2, 15);
        h
    };
    let h = match style {
        0 => {
            let h = top(&mut o, (-5.0, -4.5, 5.0, 4.5), plinth, floors);
            o.bx(-1.8, -1.8, h, 1.8, 1.8, h + 2.0, STEEL);
            h + 2.0
        }
        1 => {
            let a = floors * 5 / 10;
            let b = floors * 8 / 10;
            let h1 = top(&mut o, (-5.2, -4.8, 5.2, 4.8), plinth, a.max(2));
            let h2 = top(&mut o, (-4.0, -3.8, 4.0, 3.8), h1, (b - a).max(2));
            let h3 = top(&mut o, (-2.8, -2.8, 2.8, 2.8), h2, (floors - b).max(2));
            o.bx(-0.15, -0.15, h3, 0.15, 0.15, h3 + 6.0, STEEL);
            h3 + 6.0
        }
        _ => {
            let h1 = top(&mut o, (-6.2, -4.0, -0.6, 4.0), plinth, floors);
            let h2 = top(&mut o, (0.6, -4.0, 6.2, 4.0), plinth, floors * 8 / 10);
            o.bx(
                -0.6,
                -1.0,
                plinth + 3.0 * fh,
                0.6,
                1.0,
                plinth + 3.0 * fh + 2.0,
                glass,
            );
            h1.max(h2)
        }
    };
    let _ = h;
    o
}

pub fn duplex(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, trim) = wall_pick(region, rng);
    let w2 = shade(wall, 0.93);
    let roof = roof_colour(region, rng);
    o.bx(-7.0, -4.5, 0.0, 0.0, 4.5, 3.4, wall);
    o.bx(0.0, -4.5, 0.0, 7.0, 4.5, 3.4, w2);
    for (cx, c) in [(-3.5f32, wall), (3.5, w2)] {
        o.win_s(cx - 0.5, cx + 0.5, 0.2, 2.4, -4.5, rng.pick(&DOORS));
        o.win_s(cx - 2.5, cx - 1.4, 1.0, 2.4, -4.5, GLASS);
        o.win_s(cx + 1.4, cx + 2.5, 1.0, 2.4, -4.5, GLASS);
        o.bx(cx - 1.0, -5.6, 3.0, cx + 1.0, -4.5, 3.2, trim);
        let _ = c;
    }
    o.gable(-7.0, -4.5, 7.0, 4.5, 3.4, 2.6, roof, wall, true, 0.5);
    chimney(&mut o, -3.5, 1.0, 5.0, 7.0, shade(wall, 0.7));
    o.slab(-7.0, -4.5, 0.0, 7.0, -4.2, 0.2, TRIM);
    o
}

pub fn mobile_homes(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    o.slab(-9.0, -8.0, 0.0, 9.0, 8.0, 0.1, [150, 142, 120]);
    let n = 2 + rng.below(2);
    for i in 0..n {
        let y = -5.2 + i as f32 * 5.6;
        let c = rng.pick(&[
            [226u8, 222, 208],
            [200, 214, 226],
            [236, 210, 160],
            [190, 214, 190],
        ]);
        let x = rng.range(-1.0, 1.0);
        o.bx(x - 5.0, y - 1.6, 0.4, x + 5.0, y + 1.6, 3.0, c);
        o.bx_top(
            x - 5.1,
            y - 1.7,
            3.0,
            x + 5.1,
            y + 1.7,
            3.3,
            shade(c, 0.8),
            [176, 178, 184],
        );
        o.win_s(x - 4.0, x - 2.4, 1.2, 2.4, y - 1.6, GLASS);
        o.win_s(x - 0.8, x + 0.8, 0.5, 2.5, y - 1.6, DOOR);
        o.win_s(x + 2.4, x + 4.0, 1.2, 2.4, y - 1.6, GLASS);
        o.slab(
            x - 1.6,
            y - 3.0,
            0.0,
            x + 1.6,
            y - 1.6,
            0.4,
            [176, 160, 130],
        );
        o.bx(x - 5.0, y - 1.6, 0.0, x + 5.0, y + 1.6, 0.4, [90, 92, 100]);
        if rng.chance(0.4) {
            o.bx(
                x + 5.2,
                y - 0.8,
                0.0,
                x + 6.8,
                y + 0.8,
                1.4,
                [116, 120, 128],
            );
        }
    }
    for x in [-8.0f32, 8.0] {
        o.prism(x, rng.range(-6.0, 6.0), 0.0, 3.4, 1.3, TREE, 6, None);
    }
    o
}

// ---- industrial ----

pub fn factory_sawtooth(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let b = rng.pick(&BRICK);
    let wall = rng.pick(&[b, [186, 180, 168], [160, 168, 176]]);
    let (hw, hd, h) = (10.0, 7.0, 5.5);
    o.bx(-hw, -hd, 0.0, hw, hd, h, wall);
    let bays = 4;
    let bw = 2.0 * hw / bays as f32;
    for i in 0..bays {
        let a = -hw + i as f32 * bw;
        // each sawtooth: a vertical glazed face to the north and a sloping roof back
        o.poly(
            &[
                [a, -hd, h],
                [a + bw, -hd, h],
                [a + bw, -hd, h + 1.8],
                [a, -hd, h + 1.0],
            ],
            shade(wall, 0.9),
            true,
            true,
            false,
        );
        o.poly(
            &[
                [a, -hd, h + 1.0],
                [a + bw, -hd, h + 1.8],
                [a + bw, hd, h + 1.8],
                [a, hd, h + 1.0],
            ],
            [140, 144, 152],
            true,
            true,
            false,
        );
        o.poly(
            &[
                [a + bw, -hd, h + 1.8],
                [a + bw, hd, h + 1.8],
                [a + bw, hd, h],
                [a + bw, -hd, h],
            ],
            GLASS,
            false,
            true,
            false,
        );
    }
    o.win_s(-4.0, 4.0, 0.0, 3.6, -hd, [70, 72, 80]);
    for x in [-7.0f32, 7.0] {
        o.win_s(x - 1.0, x + 1.0, 2.0, 3.6, -hd, GLASS_D);
    }
    o.prism(hw - 2.0, hd - 2.0, h, h + 6.0, 0.9, [150, 90, 70], 8, None);
    o
}

pub fn factory_stacks(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let b = rng.pick(&BRICK);
    let wall = rng.pick(&[[170u8, 172, 178], b, [190, 184, 170]]);
    o.bx(-9.0, -6.0, 0.0, 3.0, 6.0, 7.0, wall);
    o.bx(3.0, -4.0, 0.0, 9.0, 6.0, 4.5, shade(wall, 0.9));
    o.shed(
        -9.0,
        -6.0,
        3.0,
        6.0,
        7.0,
        1.6,
        [110, 114, 122],
        wall,
        0,
        0.3,
    );
    o.win_s(-7.0, 1.0, 0.0, 4.0, -6.0, [70, 72, 80]);
    o.win_s(-8.5, -7.5, 4.6, 6.0, -6.0, GLASS_D);
    let n = 2 + rng.below(2);
    for i in 0..n {
        let x = 4.0 + i as f32 * 2.4;
        let h = 14.0 + rng.range(0.0, 8.0);
        o.prism(
            x,
            2.0,
            4.5,
            h,
            0.9,
            rng.pick(&[[160u8, 90, 70], [176, 172, 166]]),
            8,
            None,
        );
        o.bx(x - 1.0, 1.0, h - 0.8, x + 1.0, 3.0, h - 0.4, RED);
    }
    o.bx(-8.0, 6.0, 0.0, -4.0, 8.0, 3.0, [120, 126, 134]);
    o
}

pub fn tank_farm(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    o.slab(-11.0, -9.0, 0.0, 11.0, 9.0, 0.2, [128, 126, 118]);
    let n = 3 + rng.below(2);
    for i in 0..n {
        let x = -7.5 + (i % 2) as f32 * 8.0 + if i == 3 { 1.0 } else { 0.0 };
        let y = -4.5 + (i / 2) as f32 * 8.0;
        let r = rng.range(2.8, 3.8);
        let h = rng.range(5.0, 8.0);
        let c = rng.pick(&[[226u8, 228, 230], [196, 200, 206], [214, 206, 190]]);
        o.prism(x, y, 0.2, h, r, c, 12, Some(shade(c, 0.88)));
        o.prism(x, y, h, h + 0.5, r + 0.1, shade(c, 0.8), 12, None);
        o.bx(x + r, y - 0.2, 0.2, x + r + 0.5, y + 0.2, h, [90, 94, 100]);
    }
    // pipe run and a pump house
    o.bx(-9.0, -0.4, 0.2, 9.0, 0.2, 0.8, [196, 150, 60]);
    o.bx(5.0, 5.0, 0.2, 9.0, 8.0, 3.0, [160, 164, 172]);
    o.bx(4.8, 4.8, 3.0, 9.2, 8.2, 3.3, DARK);
    o
}

pub fn freight_yard(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    o.slab(-11.0, -9.0, 0.0, 11.0, 9.0, 0.15, [120, 116, 108]);
    // containers in stacks
    let cols: [Rgb; 6] = [
        [196, 62, 52],
        [48, 92, 150],
        [226, 170, 50],
        [70, 130, 96],
        [220, 220, 214],
        [150, 90, 60],
    ];
    for i in 0..3 {
        for j in 0..3 {
            let (x, y) = (-9.5 + i as f32 * 3.5, -7.0 + j as f32 * 2.9);
            let levels = 1 + rng.below(3);
            for l in 0..levels {
                let z = 0.15 + l as f32 * 2.0;
                o.bx(x, y, z, x + 3.1, y + 2.5, z + 1.9, rng.pick(&cols));
            }
        }
    }
    // rails and a gantry crane
    for y in [-2.0f32, 4.0] {
        o.slab(0.0, y, 0.15, 11.0, y + 0.3, 0.3, [90, 80, 70]);
    }
    o.bx(3.0, -5.0, 0.15, 3.4, -4.6, 7.0, [232, 190, 60]);
    o.bx(9.0, -5.0, 0.15, 9.4, -4.6, 7.0, [232, 190, 60]);
    o.bx(3.0, -5.0, 7.0, 9.4, 7.0, 7.5, [232, 190, 60]);
    o.bx(3.0, 6.6, 0.15, 3.4, 7.0, 7.0, [232, 190, 60]);
    o.bx(9.0, 6.6, 0.15, 9.4, 7.0, 7.0, [232, 190, 60]);
    o
}

pub fn port_cranes(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    o.slab(-11.0, -9.0, 0.0, 11.0, 9.0, 0.2, [132, 130, 124]);
    let col = rng.pick(&[[196u8, 62, 52], [48, 92, 150], [232, 190, 60]]);
    for k in 0..2 {
        let x = -6.0 + k as f32 * 11.0;
        for (dx, dy) in [(0.0f32, 0.0f32), (3.0, 0.0), (0.0, 6.0), (3.0, 6.0)] {
            o.bx(x + dx, -7.0 + dy, 0.2, x + dx + 0.5, -6.5 + dy, 13.0, col);
        }
        o.bx(x - 0.2, -7.2, 13.0, x + 3.7, -0.5, 13.8, col);
        // boom reaching out over the water to the south
        o.bx(x + 1.0, -9.0, 13.0, x + 2.2, -0.5, 13.8, shade(col, 0.85));
        o.bx(x + 1.4, -9.0, 8.0, x + 1.8, -8.6, 13.0, [70, 74, 82]);
        o.bx(x + 0.8, -9.4, 7.0, x + 2.4, -8.2, 8.0, [200, 150, 60]);
    }
    for i in 0..3 {
        o.bx(
            -9.5 + i as f32 * 3.4,
            3.0,
            0.2,
            -6.4 + i as f32 * 3.4,
            5.5,
            2.1,
            rng.pick(&[RED, [48, 92, 150], [226, 170, 50]]),
        );
    }
    o.bx(6.0, 4.0, 0.2, 10.0, 8.0, 4.4, [190, 186, 176]);
    o
}

pub fn recycling_centre(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    o.slab(-10.0, -8.0, 0.0, 10.0, 8.0, 0.15, [110, 112, 116]);
    o.bx(-9.0, 2.0, 0.0, 3.0, 7.5, 6.0, [96, 130, 108]);
    o.shed(
        -9.0,
        2.0,
        3.0,
        7.5,
        6.0,
        1.2,
        [110, 114, 122],
        [96, 130, 108],
        0,
        0.3,
    );
    o.win_s(-7.0, -1.0, 0.0, 4.2, 2.0, [60, 64, 72]);
    // sorted heaps and bins
    let heaps: [Rgb; 4] = [
        [170, 170, 176],
        [196, 150, 100],
        [90, 130, 190],
        [120, 160, 96],
    ];
    for (i, hc) in heaps.iter().enumerate() {
        let x = -7.0 + i as f32 * 4.0;
        o.pyramid(x, -3.5, 0.15, 2.4, 2.6, *hc, 5, 0.0);
    }
    for i in 0..3 {
        let x = 5.0 + i as f32 * 1.7;
        o.bx(
            x,
            -5.0,
            0.15,
            x + 1.5,
            -3.2,
            2.0,
            rng.pick(&[[60, 130, 80], [48, 92, 150], [196, 62, 52]]),
        );
    }
    o.prism(8.0, 4.0, 0.15, 7.0, 0.5, [140, 140, 146], 6, None);
    o
}

// ---- civic look-alikes ----

pub fn school_b(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    let b = rng.pick(&BRICK);
    let wall = rng.pick(&[b, mix(wall, WHITE, 0.2)]);
    o.bx(-9.0, -3.0, 0.0, 9.0, 3.0, 7.0, wall);
    box_windows(
        &mut o,
        (-9.0, -3.0, 9.0, 3.0),
        0.0,
        2,
        3.4,
        Pattern::Pairs,
        GLASS,
        GLASS_D,
        3,
    );
    o.bx(-2.0, -4.5, 0.0, 2.0, -3.0, 8.5, shade(wall, 0.9));
    o.win_s(-1.0, 1.0, 0.2, 2.8, -4.5, DOOR);
    o.hip(-2.0, -4.5, 2.0, -3.0, 8.5, 1.4, [84, 96, 124], 0.3);
    o.parapet(-9.0, -3.0, 9.0, 3.0, 7.0, 0.5, wall, TRIM, deck_of(wall));
    o.bx(11.0, -6.0, 0.0, 11.15, -5.85, 9.0, [196, 196, 200]);
    o.bx(11.15, -6.0, 7.2, 13.0, -5.9, 8.8, RED);
    o.slab(-10.0, -8.0, 0.0, 10.0, -4.5, 0.1, [178, 172, 160]);
    o
}

pub fn church_b(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let b = rng.pick(&BRICK);
    let wall = rng.pick(&[[236u8, 232, 222], [190, 180, 168], b]);
    o.bx(-4.5, -7.0, 0.0, 4.5, 7.0, 6.0, wall);
    o.gable(
        -4.5,
        -7.0,
        4.5,
        7.0,
        6.0,
        4.5,
        rng.pick(&[[84u8, 96, 124], [90, 94, 100]]),
        wall,
        false,
        0.4,
    );
    // a slim tower with a copper spire beside the front
    o.bx(-6.6, -8.0, 0.0, -4.0, -5.4, 11.0, shade(wall, 0.95));
    o.pyramid(-5.3, -6.7, 11.0, 2.0, 7.0, [92, 150, 130], 4, 45.0);
    o.win_s(-6.0, -4.6, 6.0, 9.5, -8.0, GLASS_D);
    o.win_s(-1.0, 1.0, 0.2, 3.0, -7.0, DOOR);
    o.win_s(-1.8, 1.8, 3.8, 6.6, -7.0, [120, 90, 170]);
    for y in [-3.0f32, 1.0, 5.0] {
        o.win_e(y - 0.5, y + 0.5, 1.8, 4.8, 4.5, [120, 90, 170]);
        o.win_w(y - 0.5, y + 0.5, 1.8, 4.8, -4.5, [120, 90, 170]);
    }
    o
}

pub fn fire_b(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (wall, _) = wall_pick(region, rng);
    let b = rng.pick(&BRICK);
    let wall = rng.pick(&[b, mix(wall, WHITE, 0.3)]);
    o.bx(-8.0, -5.0, 0.0, 4.0, 5.0, 5.6, wall);
    o.bx(4.0, -4.0, 0.0, 7.0, 3.0, 10.5, shade(wall, 0.9));
    for x in [-6.4f32, -3.6, -0.8] {
        o.win_s(x - 1.0, x + 1.0, 0.0, 4.2, -5.0, [226, 226, 230]);
        o.bx(x - 1.0, -5.1, 4.2, x + 1.0, -5.0, 4.5, RED);
    }
    o.win_s(4.6, 6.4, 3.0, 5.0, -4.0, GLASS);
    o.win_s(4.6, 6.4, 7.0, 9.0, -4.0, GLASS);
    o.parapet(-8.0, -5.0, 4.0, 5.0, 5.6, 0.5, wall, TRIM, deck_of(wall));
    o.parapet(4.0, -4.0, 7.0, 3.0, 10.5, 0.5, wall, TRIM, deck_of(wall));
    o.slab(-8.0, -9.0, 0.0, 4.0, -5.0, 0.1, [84, 86, 94]);
    o
}

pub fn water_b(region: Region, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let _ = region;
    let c = rng.pick(&[[226u8, 228, 230], [150, 190, 220], [236, 210, 120]]);
    // a ground tank with a domed roof
    o.prism(0.0, 0.0, 0.0, 7.0, 5.0, c, 14, Some(shade(c, 0.9)));
    o.pyramid(0.0, 0.0, 7.0, 5.0, 2.0, shade(c, 0.8), 14, 0.0);
    o.bx(5.0, -0.3, 0.0, 5.4, 0.3, 7.0, [90, 94, 100]);
    o.bx(-9.0, -3.0, 0.0, -6.0, 0.0, 2.6, [176, 172, 164]);
    o
}

// ---- selection ----

/// A variant for this lot, or `None` to use the base kit. `density` is 0 to 1 from the town
/// generator (0.5 when unknown).
pub fn variant(
    region: Region,
    kind: LotKind,
    level: u8,
    density: f32,
    rng: &mut Rng,
) -> Option<Obj> {
    let o = variant_raw(region, kind, level, density, rng)?;
    Some(diversify(o, region, rng))
}

/// Make neighbours of the same model differ: a slightly different size and a body colour
/// nudged toward one of the region's paints, all from the seeded roll.
fn diversify(o: Obj, region: Region, rng: &mut Rng) -> Obj {
    let sx = rng.range(0.86, 1.08);
    let sy = rng.range(0.86, 1.08);
    let sz = rng.range(0.92, 1.12);
    let mut o = crate::levels::scaled(&o, sx, sy, sz);
    let mut counts: Vec<(Rgb, usize)> = Vec::new();
    for p in &o.polys {
        if p.color == GLASS
            || p.color == GLASS_D
            || p.color == MARKER_SKIP
            || p.color == TREE
            || p.color == crate::kit::TREE_D
        {
            continue;
        }
        match counts.iter_mut().find(|c| c.0 == p.color) {
            Some(c) => c.1 += 1,
            None => counts.push((p.color, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if let Some(&(body, _)) = counts.first() {
        let paint = match material(region, rng) {
            Mat::Brick => rng.pick(&BRICKS),
            Mat::Concrete => rng.pick(&CONCS),
            _ => rng.pick(&PAINTS),
        };
        let k = rng.range(0.15, 0.45);
        let j = |rng: &mut Rng| rng.range(-9.0, 9.0);
        let mut c = mix(body, paint, k);
        for v in &mut c {
            *v = (*v as f32 + j(rng)).clamp(0.0, 255.0) as u8;
        }
        for p in &mut o.polys {
            if p.color == body {
                p.color = c;
            }
        }
    }
    o
}

const MARKER_SKIP: Rgb = [255, 0, 255];

fn variant_raw(
    region: Region,
    kind: LotKind,
    level: u8,
    density: f32,
    rng: &mut Rng,
) -> Option<Obj> {
    let r = rng.below(100);
    let tall = (density - 0.75).max(0.0) / 0.25;
    let l = level;
    match kind {
        LotKind::House => match l {
            1 if r < 30 => Some(mobile_homes(region, rng)),
            2 | 3 if r < 25 => Some(duplex(region, rng)),
            _ => None,
        },
        LotKind::Row => match l {
            1..=3 if r < 75 => Some(townhouse_row(region, r % 4, rng)),
            4 | 5 if r < 45 => Some(midrise_balconies(region, 4 + l as usize, rng)),
            _ => None,
        },
        LotKind::Shop => match l {
            1 if r < 25 => Some(corner_store(region, rng)),
            1 if r < 45 => Some(pharmacy(region, rng)),
            2 if r < 20 => Some(supermarket(region, rng)),
            2 if r < 35 => Some(pharmacy(region, rng)),
            2 if r < 50 => Some(bank(region, rng)),
            3 if r < 20 => Some(bank(region, rng)),
            3 if r < 40 => Some(cinema(region, rng)),
            3 if r < 55 => Some(hotel_mid(region, rng)),
            4 if r < 30 => Some(hotel_mid(region, rng)),
            5 if r < 25 => Some(hotel_tall(region, rng)),
            _ => None,
        },
        LotKind::Apartments => match l {
            2..=4 if r < 40 && tall == 0.0 => Some(midrise_balconies(region, 3 + l as usize, rng)),
            4 | 5 if tall > 0.0 && r < 70 => {
                Some(glass_tower(region, 14 + (tall * 8.0) as usize, r % 3, rng))
            }
            5 if r < 40 => Some(glass_tower(region, 12 + r % 6, r % 3, rng)),
            _ => None,
        },
        LotKind::Office => match l {
            1 | 2 if r < 50 => Some(office_park(region, rng)),
            3 if r < 15 => Some(hotel_mid(region, rng)),
            4 | 5 if tall > 0.0 && r < 60 => {
                Some(glass_tower(region, 16 + (tall * 10.0) as usize, r % 3, rng))
            }
            4 | 5 if r < 30 => Some(glass_tower(region, 10 + r % 6, r % 3, rng)),
            4 | 5 if r < 45 => Some(hotel_tall(region, rng)),
            _ => None,
        },
        LotKind::Gas if r < 35 => Some(petrol_a(region, rng)),
        LotKind::Gas if r < 70 => Some(petrol_b(region, rng)),
        LotKind::Workshop => match r {
            0..=29 => Some(factory_sawtooth(region, rng)),
            30..=54 => Some(factory_stacks(region, rng)),
            55..=69 => Some(recycling_centre(region, rng)),
            70..=84 => Some(tank_farm(region, rng)),
            _ => None,
        },
        LotKind::Warehouse => match r {
            0..=19 => Some(tank_farm(region, rng)),
            20..=39 => Some(freight_yard(region, rng)),
            40..=79 if region == Region::Lake => Some(port_cranes(region, rng)),
            80..=89 => Some(recycling_centre(region, rng)),
            _ => None,
        },
        LotKind::BigBox if r < 35 => Some(supermarket(region, rng)),
        LotKind::Strip if r < 25 => Some(car_dealership(region, rng)),
        LotKind::FastFood if r < 35 => Some(diner(region, rng)),
        LotKind::School if r < 40 => Some(school_b(region, rng)),
        LotKind::Church if r < 40 => Some(church_b(region, rng)),
        LotKind::Fire if r < 40 => Some(fire_b(region, rng)),
        LotKind::Water if r < 40 => Some(water_b(region, rng)),
        _ => None,
    }
}

// ---- street trees ----

/// A tree of a species picked from `roll` (0 to 1). Deciduous species use the leaf colours the
/// seasons recolour; pines stay green all year.
pub fn tree_species(roll: f32, r: f32, h: f32, col: Rgb) -> Obj {
    if roll < 0.5 {
        return crate::kit::tree(r, h, col);
    }
    let mut o = Obj::new();
    if roll < 0.68 {
        // poplar: tall and narrow
        o.bx(-0.2, -0.2, 0.0, 0.2, 0.2, h * 0.3, [110, 80, 56]);
        o.prism(0.0, 0.0, h * 0.25, h * 1.0, r * 0.6, col, 6, None);
        o.pyramid(0.0, 0.0, h * 1.0, r * 0.6, h * 0.35, col, 6, 0.0);
    } else if roll < 0.84 {
        // oak: short trunk, wide round crown in two tiers
        o.bx(-0.3, -0.3, 0.0, 0.3, 0.3, h * 0.35, [104, 76, 54]);
        o.prism(0.0, 0.0, h * 0.3, h * 0.6, r * 1.25, col, 7, None);
        o.prism(0.0, 0.0, h * 0.6, h * 0.8, r * 0.8, col, 7, None);
        o.pyramid(0.0, 0.0, h * 0.8, r * 0.8, h * 0.2, col, 7, 0.0);
    } else {
        // pine: dark cone tiers that never turn
        o.bx(-0.2, -0.2, 0.0, 0.2, 0.2, h * 0.25, [96, 70, 50]);
        for k in 0..3 {
            let z = h * (0.2 + 0.24 * k as f32);
            o.pyramid(
                0.0,
                0.0,
                z,
                r * (1.1 - 0.25 * k as f32),
                h * 0.4,
                PINE,
                7,
                0.0,
            );
        }
    }
    o
}
