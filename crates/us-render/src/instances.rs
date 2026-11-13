//! Things that move or repeat cheaply: cars on the roads, smoke puffs over chimneys and a few
//! people on downtown pavements. Each kind is one small model drawn many times (instanced); the
//! per-frame positions are worked out here from a time value.

use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use bytemuck::{Pod, Zeroable};
use us_gen::{Pt, RoadClass};

/// Colour a model uses where the instance's own colour should show.
pub const MARKER: Rgb = [255, 0, 255];
/// Headlight colour: lit at night.
pub const HEADLIGHT: Rgb = [255, 236, 150];

pub const CAR_ID_BASE: u32 = 8_000_000;
pub const PUFF_ID_BASE: u32 = 9_500_000;
pub const PERSON_ID_BASE: u32 = 10_200_000;
pub const LAMP_ID_BASE: u32 = 10_600_000;
/// Outline ids reserved per instance.
pub const ID_STRIDE: u32 = 16;

/// Most cars drawn at once, however high the volumes are.
pub const MAX_CARS: usize = 8000;
pub const MAX_PUFFS: usize = 600;
pub const MAX_PEOPLE: usize = 160;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Instance {
    pub pos: [f32; 3],
    pub theta: f32,
    pub scale: f32,
    /// 0 for the model's own colours, else `0x01000000 | rgb` for the marker colour.
    pub tint: u32,
    /// Outline id of the instance's first face.
    pub id: u32,
}

pub const INSTANCE_BYTES: usize = std::mem::size_of::<Instance>();

pub fn tint(c: Rgb) -> u32 {
    0x0100_0000 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
}

fn emit(o: &Obj) -> Mesh {
    let mut m = Mesh::default();
    let mut id = 0u32;
    o.emit(&mut m, &mut id, 0.0, 0.0, 0.0, 1.0);
    m
}

/// The car model: 4.2 m long along +x, body in the marker colour.
pub fn car_mesh() -> Mesh {
    let mut o = Obj::new();
    o.bx(-2.1, -0.9, 0.3, 2.1, 0.9, 1.0, MARKER);
    o.bx_top(-1.1, -0.8, 1.0, 1.0, 0.8, 1.55, [70, 96, 120], MARKER);
    for y in [-0.65f32, 0.65] {
        o.poly(
            &[
                [2.12, y - 0.25, 0.5],
                [2.12, y + 0.25, 0.5],
                [2.12, y + 0.25, 0.8],
                [2.12, y - 0.25, 0.8],
            ],
            HEADLIGHT,
            false,
            true,
            true,
        );
    }
    emit(&o)
}

/// A smoke puff: a squat faceted lump in the marker colour.
pub fn puff_mesh() -> Mesh {
    let mut o = Obj::new();
    o.prism(0.0, 0.0, -0.5, 0.5, 0.8, MARKER, 6, None);
    o.pyramid(0.0, 0.0, 0.5, 0.8, 0.5, MARKER, 6, 0.0);
    emit(&o)
}

/// A person seen from above: a body block and a head.
pub fn person_mesh() -> Mesh {
    let mut o = Obj::new();
    o.bx(-0.22, -0.16, 0.0, 0.22, 0.16, 1.15, MARKER);
    o.bx(-0.14, -0.14, 1.15, 0.14, 0.14, 1.5, [226, 186, 150]);
    emit(&o)
}

/// Colour of a street lamp's lit quad (it glows at dusk, night and dawn).
pub const LAMP_LENS: Rgb = HEADLIGHT;
/// Marker colour of the pool of light a lamp throws on the street. The shader draws it only at
/// night, as a hard edged flat patch, and discards it by day.
pub const LAMP_POOL: Rgb = [250, 232, 150];

/// A street lamp: a pole, a short arm toward +x and a lit quad on the arm, with a flat octagon of
/// light on the ground under it. The arm points at the road.
pub fn lamp_mesh() -> Mesh {
    let mut o = Obj::new();
    let steel = [74, 78, 88];
    o.bx(-0.09, -0.09, 0.0, 0.09, 0.09, 5.6, steel);
    o.bx(-0.09, -0.12, 5.6, 1.7, 0.12, 5.8, steel);
    o.poly(
        &[
            [0.9, -0.3, 5.83],
            [1.9, -0.3, 5.83],
            [1.9, 0.3, 5.83],
            [0.9, 0.3, 5.83],
        ],
        LAMP_LENS,
        false,
        false,
        true,
    );
    let pool: Vec<[f32; 3]> = (0..8)
        .map(|i| {
            let a = (i as f32 + 0.5) * std::f32::consts::TAU / 8.0;
            [1.2 + 3.8 * libm::cosf(a), 3.8 * libm::sinf(a), 0.4]
        })
        .collect();
    o.poly(&pool, LAMP_POOL, false, false, true);
    emit(&o)
}

/// Where the street lamps stand: both sides in turn along every street, a lamp each `spacing`
/// metres, set just outside the road edge with the arm over the road.
pub fn lamp_instances(paths: &[Path]) -> Vec<Instance> {
    let mut out = Vec::new();
    for p in paths {
        let spacing = match p.class {
            RoadClass::Local => 44.0,
            RoadClass::Collector => 40.0,
            RoadClass::Arterial => 36.0,
            _ => continue,
        };
        let l = p.length();
        let off = p.class.half_width() as f32 + 1.3;
        let mut s = 14.0;
        let mut k = 0usize;
        while s < l - 6.0 {
            let (pt, d) = p.at(s);
            let side = if k.is_multiple_of(2) { 1.0 } else { -1.0 };
            let (nx, ny) = (-d[1] * side, d[0] * side);
            out.push(Instance {
                pos: [pt[0] + nx * off, pt[1] + ny * off, 0.0],
                theta: libm::atan2f(-ny, -nx),
                scale: 1.0,
                tint: 0,
                id: LAMP_ID_BASE + out.len() as u32 * ID_STRIDE,
            });
            s += spacing;
            k += 1;
        }
    }
    out
}

/// A road as the traffic needs it: its line with running lengths.
#[derive(Clone, Debug)]
pub struct Path {
    pub class: RoadClass,
    pub pts: Vec<[f32; 2]>,
    /// Distance along the road at each point.
    pub cum: Vec<f32>,
}

impl Path {
    pub fn new(class: RoadClass, line: &[Pt]) -> Path {
        let pts: Vec<[f32; 2]> = line.iter().map(|p| [p.x as f32, p.y as f32]).collect();
        let mut cum = Vec::with_capacity(pts.len());
        let mut s = 0.0;
        for i in 0..pts.len() {
            if i > 0 {
                s += (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]);
            }
            cum.push(s);
        }
        Path { class, pts, cum }
    }

    pub fn length(&self) -> f32 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    /// Point and direction of travel (unit) at distance `s`.
    pub fn at(&self, s: f32) -> ([f32; 2], [f32; 2]) {
        let n = self.pts.len();
        if n < 2 {
            return ([0.0; 2], [1.0, 0.0]);
        }
        let i = self.cum.partition_point(|&c| c <= s).clamp(1, n - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let seg = (self.cum[i] - self.cum[i - 1]).max(1e-4);
        let t = ((s - self.cum[i - 1]) / seg).clamp(0.0, 1.0);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy).max(1e-6);
        ([a[0] + dx * t, a[1] + dy * t], [dx / l, dy / l])
    }
}

/// One car: where on which road it started, which way it goes and how fast.
#[derive(Clone, Copy, Debug)]
pub struct Car {
    pub road: u32,
    pub phase: f32,
    pub dir: f32,
    pub speed: f32,
    pub colour: Rgb,
}

fn cruise(c: RoadClass) -> f32 {
    match c {
        RoadClass::Alley => 5.0,
        RoadClass::Local => 8.0,
        RoadClass::Collector => 11.0,
        RoadClass::Arterial => 14.0,
        RoadClass::Highway => 24.0,
        RoadClass::Rail => 0.0,
    }
}

/// Cars wanted on a road of this length at this volume (0 to 1): a car every 70 m on a quiet
/// road down to one every 9 m when jammed, counting both directions.
pub fn cars_wanted(length: f32, volume: f32) -> usize {
    let v = volume.clamp(0.0, 1.0);
    if v <= 0.0 || length < 6.0 {
        return 0;
    }
    let spacing = 70.0 + (9.0 - 70.0) * v;
    ((length / spacing).ceil() as usize).max(1)
}

/// The cars for a set of roads and their volumes, capped at [`MAX_CARS`] in total.
pub fn make_cars(paths: &[Path], volumes: &[f32]) -> Vec<Car> {
    let wanted: Vec<usize> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if p.class == RoadClass::Rail {
                0
            } else {
                cars_wanted(p.length(), volumes.get(i).copied().unwrap_or(0.0))
            }
        })
        .collect();
    let total: usize = wanted.iter().sum();
    let k = if total > MAX_CARS {
        MAX_CARS as f32 / total as f32
    } else {
        1.0
    };
    let mut cars = Vec::with_capacity(total.min(MAX_CARS));
    for (i, p) in paths.iter().enumerate() {
        let n = if k < 1.0 {
            (wanted[i] as f32 * k) as usize
        } else {
            wanted[i]
        };
        let v = volumes.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let mut rng = Rng::new(0xca75, i as u64);
        for j in 0..n {
            cars.push(Car {
                road: i as u32,
                phase: (j as f32 + rng.range(0.1, 0.9)) / n as f32,
                dir: if j % 2 == 0 { 1.0 } else { -1.0 },
                speed: cruise(p.class) * (1.0 - 0.55 * v) * rng.range(0.9, 1.1),
                colour: rng.pick(&crate::kit::CAR_COLS),
            });
        }
    }
    cars
}

/// Where each car is at `time` seconds.
pub fn car_instances(paths: &[Path], cars: &[Car], time: f32) -> Vec<Instance> {
    let mut out = Vec::with_capacity(cars.len());
    for (n, c) in cars.iter().enumerate() {
        let p = &paths[c.road as usize];
        let l = p.length();
        if l < 1.0 {
            continue;
        }
        let s = (c.phase * l + c.dir * c.speed * time).rem_euclid(l);
        let (pt, d) = p.at(s);
        let hw = p.class.half_width() as f32;
        // keep to the right of the direction of travel
        let side = -c.dir * hw * 0.5;
        let (nx, ny) = (-d[1], d[0]);
        let heading = libm::atan2f(d[1] * c.dir, d[0] * c.dir);
        out.push(Instance {
            pos: [pt[0] + nx * side, pt[1] + ny * side, 0.0],
            theta: heading,
            scale: 1.0,
            tint: tint(c.colour),
            id: CAR_ID_BASE + n as u32 * ID_STRIDE,
        });
    }
    out
}

/// Smoke puffs rising from chimney tops (world x, y, top z), drifting east and a little north.
pub fn puff_instances(chimneys: &[[f32; 3]], time: f32) -> Vec<Instance> {
    const PER: usize = 5;
    const PERIOD: f32 = 5.0;
    let greys: [Rgb; 3] = [[236, 236, 240], [206, 206, 214], [176, 176, 188]];
    let mut out = Vec::new();
    for (ci, c) in chimneys.iter().enumerate() {
        for k in 0..PER {
            if out.len() >= MAX_PUFFS {
                return out;
            }
            let ph = (time / PERIOD + k as f32 / PER as f32 + ci as f32 * 0.37).rem_euclid(1.0);
            let grey = greys[((ph * 3.0) as usize).min(2)];
            out.push(Instance {
                pos: [c[0] + ph * 9.0, c[1] + ph * 2.5, c[2] + 0.6 + ph * 13.0],
                theta: ph * 2.0 + ci as f32,
                scale: 0.7 + ph * 1.9,
                tint: tint(grey),
                id: PUFF_ID_BASE + out.len() as u32 * ID_STRIDE,
            });
        }
    }
    out
}

/// A walker: paces back and forth over a stretch of pavement near the centre of town.
#[derive(Clone, Copy, Debug)]
pub struct Walker {
    pub road: u32,
    /// Middle of the stretch, metres along the road.
    pub s0: f32,
    /// Half length of the stretch.
    pub span: f32,
    pub off: f32,
    pub dir: f32,
    pub side: f32,
    pub colour: Rgb,
}

/// People on the pavements of the roads nearest the centre of town.
pub fn make_walkers(paths: &[Path], centre: [f32; 2], radius: f32) -> Vec<Walker> {
    let shirts: [Rgb; 6] = [
        [196, 58, 52],
        [48, 84, 150],
        [230, 190, 60],
        [60, 130, 96],
        [236, 236, 230],
        [150, 70, 160],
    ];
    // for each street the point nearest the centre, if it is within the radius
    let mut near: Vec<(usize, f32)> = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        if !matches!(
            p.class,
            RoadClass::Local | RoadClass::Collector | RoadClass::Arterial
        ) {
            continue;
        }
        let l = p.length();
        let mut best: Option<(f32, f32)> = None;
        let mut s = 0.0;
        while s <= l {
            let (pt, _) = p.at(s);
            let d = (pt[0] - centre[0]).hypot(pt[1] - centre[1]);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, s));
            }
            s += 6.0;
        }
        if let Some((d, s)) = best
            && d < radius
        {
            near.push((i, s));
        }
    }
    let mut out = Vec::new();
    if near.is_empty() {
        return out;
    }
    let mut rng = Rng::new(0x9e0b, 1);
    while out.len() < MAX_PEOPLE {
        let (r, s0) = near[rng.below(near.len())];
        out.push(Walker {
            road: r as u32,
            s0,
            span: rng.range(25.0, 80.0),
            off: rng.range(0.0, 400.0),
            dir: if rng.chance(0.5) { 1.0 } else { -1.0 },
            side: if rng.chance(0.5) { 1.0 } else { -1.0 },
            colour: rng.pick(&shirts),
        });
    }
    out
}

pub fn walker_instances(paths: &[Path], walkers: &[Walker], time: f32) -> Vec<Instance> {
    let mut out = Vec::new();
    for (n, w) in walkers.iter().enumerate() {
        let p = &paths[w.road as usize];
        let l = p.length();
        if l < 1.0 {
            continue;
        }
        let u = (w.off + w.dir * 1.3 * time) / (2.0 * w.span);
        let f = u - u.floor();
        let (tri, fwd) = if f < 0.5 {
            (f * 2.0, 1.0)
        } else {
            (2.0 - 2.0 * f, -1.0)
        };
        let s = (w.s0 - w.span + 2.0 * w.span * tri).clamp(0.0, l);
        let (pt, d) = p.at(s);
        let off = p.class.half_width() as f32 + 1.4;
        let (nx, ny) = (-d[1], d[0]);
        let dir = fwd * w.dir.signum() * if w.dir > 0.0 { 1.0 } else { -1.0 };
        out.push(Instance {
            pos: [pt[0] + nx * off * w.side, pt[1] + ny * off * w.side, 0.0],
            theta: libm::atan2f(d[1] * dir, d[0] * dir),
            scale: 1.0,
            tint: tint(w.colour),
            id: PERSON_ID_BASE + n as u32 * ID_STRIDE,
        });
    }
    out
}
