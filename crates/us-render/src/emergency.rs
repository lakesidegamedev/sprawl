//! Fire engines, police cars and ambulances with flashing lights, and flames and smoke on
//! burning buildings. The simulation says where each vehicle and fire is; this draws them.
//!
//! Each vehicle is one small model drawn many times (instanced). The light bar is in the marker
//! colour, so a vehicle's instance colour flashes it red and blue while it drives.

use crate::instances::{HEADLIGHT, ID_STRIDE, Instance, MARKER, PUFF_ID_BASE, emit, tint};
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};

pub const EMERGENCY_ID_BASE: u32 = 14_000_000;
pub const FLAME_ID_BASE: u32 = 14_600_000;
const MAX_PER_KIND: usize = 64;
const MAX_FLAMES: usize = 400;
const MAX_SMOKE: usize = 900;

/// Which vehicle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigKind {
    FireEngine,
    PoliceCar,
    Ambulance,
}

impl RigKind {
    pub const ALL: [RigKind; 3] = [RigKind::FireEngine, RigKind::PoliceCar, RigKind::Ambulance];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn mesh(self) -> Mesh {
        emit(&match self {
            RigKind::FireEngine => fire_engine(),
            RigKind::PoliceCar => police_car(),
            RigKind::Ambulance => ambulance(),
        })
    }
}

/// A vehicle to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmVehicle {
    pub kind: RigKind,
    pub pos: [f32; 2],
    pub heading: f32,
    /// Lights flash only while driving.
    pub moving: bool,
}

/// A fire to draw: stage 0 smoke, 1 flames, 2 fully involved. `top` is the roof height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmBlaze {
    pub pos: [f32; 2],
    pub stage: u8,
    pub top: f32,
}

fn lights(o: &mut Obj, x0: f32, x1: f32, z: f32, half_w: f32) {
    // a light bar across the roof in the marker colour
    o.bx(x0, -half_w, z, x1, half_w, z + 0.28, MARKER);
}

fn wheels(o: &mut Obj, xs: &[f32], half_w: f32, r: f32) {
    for &x in xs {
        for s in [-1.0f32, 1.0] {
            o.bx(
                x - r,
                s * half_w - 0.14,
                0.0,
                x + r,
                s * half_w + 0.14,
                r * 2.0,
                [28, 28, 32],
            );
        }
    }
}

fn headlamps(o: &mut Obj, x: f32, y: f32, z: f32) {
    for s in [-1.0f32, 1.0] {
        o.poly(
            &[
                [x, s * y - 0.25, z],
                [x, s * y + 0.25, z],
                [x, s * y + 0.25, z + 0.3],
                [x, s * y - 0.25, z + 0.3],
            ],
            HEADLIGHT,
            false,
            true,
            true,
        );
    }
}

/// A red engine 8 m long: cab, hose bed and a ladder along the top.
pub fn fire_engine() -> Obj {
    let red: Rgb = [196, 34, 34];
    let mut o = Obj::new();
    wheels(&mut o, &[-2.6, 2.4], 1.0, 0.5);
    o.bx(-4.0, -1.1, 0.5, 4.0, 1.1, 1.9, red);
    o.bx_top(1.6, -1.05, 1.9, 3.8, 1.05, 3.0, [70, 96, 120], red);
    o.bx(-3.8, -0.9, 1.9, 1.2, 0.9, 2.3, [210, 210, 214]);
    // ladder
    o.bx(-3.8, -0.25, 2.3, 1.2, -0.12, 2.45, [188, 188, 194]);
    o.bx(-3.8, 0.12, 2.3, 1.2, 0.25, 2.45, [188, 188, 194]);
    lights(&mut o, 2.2, 3.0, 3.0, 0.8);
    headlamps(&mut o, 4.02, 0.7, 0.9);
    o
}

/// A white and blue patrol car 4.6 m long.
pub fn police_car() -> Obj {
    let mut o = Obj::new();
    wheels(&mut o, &[-1.4, 1.4], 0.8, 0.32);
    o.bx(-2.3, -0.9, 0.3, 2.3, 0.9, 1.0, [238, 238, 242]);
    o.bx(-2.3, -0.92, 0.55, 2.3, 0.92, 0.8, [40, 70, 140]);
    o.bx_top(
        -1.2,
        -0.8,
        1.0,
        1.1,
        0.8,
        1.55,
        [70, 96, 120],
        [238, 238, 242],
    );
    lights(&mut o, -0.3, 0.3, 1.55, 0.6);
    headlamps(&mut o, 2.32, 0.65, 0.5);
    o
}

/// A white van with a red stripe and a light bar, 5.6 m long.
pub fn ambulance() -> Obj {
    let mut o = Obj::new();
    wheels(&mut o, &[-1.8, 1.8], 0.9, 0.38);
    o.bx(-2.8, -1.0, 0.4, 2.8, 1.0, 1.4, [240, 240, 242]);
    o.bx_top(
        -2.8,
        -1.0,
        1.4,
        1.2,
        1.0,
        2.5,
        [240, 240, 242],
        [240, 240, 242],
    );
    o.bx(1.2, -0.95, 1.4, 2.6, 0.95, 2.1, [70, 96, 120]);
    o.bx(-2.82, -1.02, 0.9, 2.2, 1.02, 1.15, [200, 40, 40]);
    lights(&mut o, 0.5, 1.1, 2.5, 0.7);
    headlamps(&mut o, 2.82, 0.7, 0.7);
    o
}

/// A flame: a faceted tongue in the marker colour.
pub fn flame_mesh() -> Mesh {
    let mut o = Obj::new();
    o.prism(0.0, 0.0, 0.0, 0.7, 1.1, MARKER, 6, None);
    o.pyramid(0.0, 0.0, 1.1, 0.7, 1.6, MARKER, 6, 0.0);
    emit(&o)
}

fn flash(time: f32, k: usize) -> Rgb {
    if ((time * 4.0) as usize + k).is_multiple_of(2) {
        [236, 40, 40]
    } else {
        [60, 100, 255]
    }
}

/// Vehicles at this frame's clock.
pub fn vehicle_instances(
    v: &[EmVehicle],
    z: &dyn Fn(f32, f32) -> f32,
    time: f32,
) -> [Vec<Instance>; 3] {
    let mut out: [Vec<Instance>; 3] = Default::default();
    for (n, e) in v.iter().enumerate() {
        let set = &mut out[e.kind.index()];
        if set.len() >= MAX_PER_KIND {
            continue;
        }
        let col = if e.moving {
            flash(time, n)
        } else {
            [120, 120, 126]
        };
        set.push(Instance {
            pos: [e.pos[0], e.pos[1], z(e.pos[0], e.pos[1])],
            theta: e.heading,
            scale: 1.0,
            tint: tint(col),
            id: EMERGENCY_ID_BASE + (e.kind.index() as u32 * 1000 + set.len() as u32) * ID_STRIDE,
        });
    }
    out
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

/// Flames on the roof and smoke columns drifting up and east.
pub fn blaze_instances(
    b: &[EmBlaze],
    z: &dyn Fn(f32, f32) -> f32,
    time: f32,
) -> (Vec<Instance>, Vec<Instance>) {
    let mut flames = Vec::new();
    let mut smoke = Vec::new();
    let greys: [Rgb; 3] = [[96, 94, 98], [128, 126, 130], [170, 168, 174]];
    for (bi, f) in b.iter().enumerate() {
        let ground = z(f.pos[0], f.pos[1]);
        let seed =
            bi as u32 * 31 + (f.pos[0] as i32 as u32).wrapping_mul(7) + (f.pos[1] as i32 as u32);
        let n_flames = match f.stage {
            0 => 0,
            1 => 3,
            _ => 6,
        };
        for k in 0..n_flames {
            if flames.len() >= MAX_FLAMES {
                break;
            }
            let h = hash(seed + k);
            let flick = 0.75 + 0.35 * libm::sinf(time * (5.0 + 3.0 * h) + k as f32 * 2.0);
            let a = h * std::f32::consts::TAU;
            let r = 1.0 + 2.2 * hash(seed + k + 99);
            let size = if f.stage == 1 { 1.6 } else { 2.4 };
            flames.push(Instance {
                pos: [
                    f.pos[0] + libm::cosf(a) * r,
                    f.pos[1] + libm::sinf(a) * r,
                    ground + f.top * (0.6 + 0.4 * h),
                ],
                theta: h * 6.0,
                scale: size * flick,
                tint: tint(if k % 2 == 0 {
                    [255, 140, 30]
                } else {
                    [255, 214, 70]
                }),
                id: FLAME_ID_BASE + flames.len() as u32 * ID_STRIDE,
            });
        }
        // a column: small dark puffs leave the roof, rise, spread a little, lean downwind and
        // lighten as they climb; many small puffs read as one column, not as loose blocks
        let n_puffs = 8 + f.stage as usize * 5;
        let period = 9.0;
        for k in 0..n_puffs {
            if smoke.len() >= MAX_SMOKE {
                break;
            }
            let ph = (time / period + k as f32 / n_puffs as f32 + hash(seed) * 0.5).rem_euclid(1.0);
            let grey = greys[((ph * 3.0) as usize).min(2)];
            let rise = 18.0 + 22.0 * f.stage as f32;
            let lean = ph * ph * (6.0 + 4.0 * f.stage as f32);
            let wobble = libm::sinf(ph * 9.0 + seed as f32 + k as f32) * (0.4 + ph);
            smoke.push(Instance {
                pos: [
                    f.pos[0] + lean + wobble * 0.5,
                    f.pos[1] + lean * 0.3 + wobble,
                    ground + f.top.max(3.0) + 0.5 + ph * rise,
                ],
                theta: ph * 2.0 + bi as f32,
                scale: 0.45 + ph * (0.9 + 0.35 * f.stage as f32),
                tint: tint(grey),
                id: PUFF_ID_BASE + 400_000 + smoke.len() as u32 * ID_STRIDE,
            });
        }
    }
    (flames, smoke)
}
