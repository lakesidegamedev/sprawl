//! Railway buildings: the passenger depot with its platform and canopy, and the prairie grain
//! elevator. Same conventions as the kit: local metres, the front facing south (-y), centred on
//! the building. The track runs along x behind the building, `us_gen` places its centre line
//! `TRACK_Y` metres north of the origin.

use crate::obj::Obj;
use crate::parts::{BRICKS, CONC, DOORS, GLASS, SHINGLES, TRIM, WHITE, WOOD, shade};
use crate::rng::Rng;
use us_gen::{Station, StationKind};

/// Distance from the depot's centre to the middle of the track, metres.
pub const TRACK_Y: f32 = 18.0;
/// Distance from the elevator's centre to the middle of the track, metres.
pub const ELEVATOR_TRACK_Y: f32 = 15.0;

/// The model of a railway building, local to it, facing south.
pub fn station_model(s: &Station, seed: u64) -> Obj {
    let mut rng = Rng::new(seed ^ 0x5747_1011, (s.pos.x * 7.0 + s.pos.y * 13.0) as u64);
    match s.kind {
        StationKind::Depot => depot(s.length as f32, &mut rng),
        StationKind::Elevator => grain_elevator(&mut rng),
    }
}

/// A brick depot with deep eaves, a bay window toward the track, a paved forecourt, and a
/// platform with a canopy along the track.
pub fn depot(platform_len: f32, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let brick = rng.pick(&BRICKS);
    let roof = rng.pick(&SHINGLES);
    let door = rng.pick(&DOORS);
    let half = platform_len / 2.0;
    // forecourt toward the street
    o.slab(-12.0, -10.0, 0.0, 12.0, -4.5, 0.12, [196, 190, 178]);
    // platform between the building and the track
    let (py0, py1) = (TRACK_Y - 9.0, TRACK_Y - 3.4);
    o.slab(-12.0, 5.0, 0.0, 12.0, py0, 0.12, [196, 190, 178]);
    o.slab(-half, py0, 0.0, half, py1, 0.9, CONC);
    o.bx(-half, py1 - 0.3, 0.9, half, py1, 0.92, [236, 206, 70]);
    // the building: a long single-storey hall with a two-storey middle
    o.bx_top(-10.0, -4.5, 0.0, 10.0, 5.0, 5.0, brick, shade(brick, 0.8));
    o.bx(-10.2, -4.7, 4.6, 10.2, 5.2, 5.1, TRIM);
    o.hip(-10.0, -4.5, 10.0, 5.0, 5.1, 2.6, roof, 1.8);
    o.bx_top(-3.5, -4.0, 5.1, 3.5, 4.5, 8.4, brick, shade(brick, 0.8));
    o.gable(-3.5, -4.0, 3.5, 4.5, 8.4, 2.4, roof, brick, false, 0.6);
    // the agent's bay window looking up and down the line
    o.bx_top(-1.6, 5.0, 0.0, 1.6, 6.3, 3.6, brick, TRIM);
    o.win_n(-1.2, 1.2, 1.2, 3.0, 6.3, GLASS);
    // doors and windows
    o.win_s(-1.3, 1.3, 0.1, 3.2, -4.5, door);
    o.win_n(-6.0, -4.0, 0.1, 3.0, 5.0, door);
    o.win_n(4.0, 6.0, 0.1, 3.0, 5.0, door);
    for x in [-8.0f32, -5.5, 3.5, 6.0, 8.5] {
        o.win_s(x - 0.7, x + 0.7, 1.4, 3.6, -4.5, GLASS);
    }
    for x in [-2.0f32, 2.0] {
        o.win_s(x - 0.6, x + 0.6, 6.0, 7.6, -4.0, GLASS);
    }
    for y in [-1.5f32, 2.5] {
        o.win_w(y - 0.6, y + 0.6, 1.4, 3.6, -10.0, GLASS);
        o.win_e(y - 0.6, y + 0.6, 1.4, 3.6, 10.0, GLASS);
    }
    // name board over the street door
    o.poly(
        &[
            [-3.0, -4.56, 3.6],
            [3.0, -4.56, 3.6],
            [3.0, -4.56, 4.4],
            [-3.0, -4.56, 4.4],
        ],
        WHITE,
        false,
        true,
        true,
    );
    // platform canopy on posts, beyond the building's eaves
    let ch = 4.0;
    let cx = half - 3.0;
    for x in [-cx, -cx * 0.5, cx * 0.5, cx] {
        o.bx(
            x - 0.12,
            py1 - 1.6,
            0.9,
            x + 0.12,
            py1 - 1.36,
            ch,
            [70, 74, 80],
        );
    }
    o.bx_top(
        -cx - 1.0,
        py0 + 0.6,
        ch,
        cx + 1.0,
        py1 - 0.3,
        ch + 0.25,
        shade(roof, 0.9),
        shade(roof, 1.05),
    );
    // benches and a lamp or two on the platform
    for x in [-cx * 0.75, cx * 0.75] {
        o.bx(x - 0.9, py0 + 1.5, 0.9, x + 0.9, py0 + 2.0, 1.35, WOOD);
    }
    for x in [-half + 1.5, half - 1.5] {
        o.bx(
            x - 0.08,
            py1 - 1.0,
            0.9,
            x + 0.08,
            py1 - 0.84,
            4.2,
            [60, 62, 66],
        );
        o.bx(
            x - 0.3,
            py1 - 1.2,
            4.2,
            x + 0.3,
            py1 - 0.6,
            4.5,
            [250, 232, 170],
        );
    }
    o
}

/// A row of concrete silos with a headhouse on top and a loading shed by the track.
pub fn grain_elevator(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let conc = rng.pick(&[[214u8, 210, 200], [200, 196, 186], [226, 220, 206]]);
    let n = rng.pick(&[3usize, 4]);
    let h = rng.range(22.0, 28.0);
    let r = 2.6;
    let x0 = -(n as f32 - 1.0) * r;
    for i in 0..n {
        let x = x0 + i as f32 * 2.0 * r;
        o.prism(x, -1.0, 0.0, h, r, conc, 12, Some(shade(conc, 1.06)));
    }
    // headhouse along the top
    let w = n as f32 * r;
    o.bx_top(
        -1.8,
        -2.6,
        h,
        1.8,
        0.6,
        h + 7.0,
        shade(conc, 0.95),
        [150, 148, 144],
    );
    o.bx(-w, -1.6, h, w, -0.4, h + 1.1, shade(conc, 0.9));
    o.poly(
        &[
            [-1.6, -2.66, h + 4.0],
            [1.6, -2.66, h + 4.0],
            [1.6, -2.66, h + 5.6],
            [-1.6, -2.66, h + 5.6],
        ],
        [196, 58, 52],
        false,
        true,
        true,
    );
    // loading shed and spout toward the track
    o.bx_top(
        -4.0,
        2.0,
        0.0,
        4.0,
        ELEVATOR_TRACK_Y - 4.0,
        5.0,
        [170, 120, 80],
        [120, 84, 60],
    );
    o.bx(
        -0.2,
        2.0,
        5.0,
        0.2,
        ELEVATOR_TRACK_Y - 3.0,
        5.4,
        [110, 112, 118],
    );
    // office at the foot
    o.bx_top(-w - 4.0, -6.0, 0.0, -w, -1.0, 3.2, TRIM, [130, 126, 120]);
    o.win_s(-w - 2.8, -w - 1.6, 1.0, 2.4, -6.0, GLASS);
    o.slab(-w - 6.0, -9.0, 0.0, w + 4.0, -6.0, 0.1, [190, 182, 160]);
    o
}
