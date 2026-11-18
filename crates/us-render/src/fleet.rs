//! Street vehicles beyond the plain cars (city bus, delivery truck, garbage truck, pickup, van)
//! and street furniture (traffic lights, bus shelters, billboards, benches, bins). Vehicles are
//! instanced like cars and run on the roads at a count set by the town's population; props are
//! fixed instances placed from the road layout. Everything here is worked out from the roads,
//! lots and population alone, so the same town always gets the same fleet.

use crate::instances::{ID_STRIDE, Instance, MARKER, Path, tint};
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::overlay::LotInfo;
use crate::rng::Rng;
use us_gen::{LotKind, RoadClass};

pub const FLEET_ID_BASE: u32 = 13_000_000;
pub const PROP_ID_BASE: u32 = 14_000_000;

const WIN: Rgb = [70, 96, 120];
const STEEL: Rgb = [74, 78, 88];
const HEAD: Rgb = crate::instances::HEADLIGHT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vehicle {
    Bus,
    Delivery,
    Garbage,
    Pickup,
    Van,
}

impl Vehicle {
    pub const ALL: [Vehicle; 5] = [
        Vehicle::Bus,
        Vehicle::Delivery,
        Vehicle::Garbage,
        Vehicle::Pickup,
        Vehicle::Van,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Vehicle::Bus => "bus",
            Vehicle::Delivery => "delivery truck",
            Vehicle::Garbage => "garbage truck",
            Vehicle::Pickup => "pickup",
            Vehicle::Van => "van",
        }
    }

    /// The vehicle model, long axis along +x, body in the marker colour.
    pub fn obj(self) -> Obj {
        let mut o = Obj::new();
        match self {
            Vehicle::Bus => {
                o.bx(-5.6, -1.25, 0.45, 5.6, 1.25, 3.2, MARKER);
                o.bx_top(
                    -5.6,
                    -1.25,
                    3.2,
                    5.6,
                    1.25,
                    3.4,
                    [226, 226, 222],
                    [200, 200, 196],
                );
                // window band and a destination board
                for side in [-1.0f32, 1.0] {
                    for i in 0..5 {
                        let x = -4.6 + i as f32 * 2.0;
                        let y = side * 1.26;
                        o.poly(
                            &[
                                [x, y, 1.5],
                                [x + 1.5, y, 1.5],
                                [x + 1.5, y, 2.7],
                                [x, y, 2.7],
                            ],
                            WIN,
                            false,
                            false,
                            true,
                        );
                    }
                }
                o.poly(
                    &[
                        [5.62, -1.0, 1.5],
                        [5.62, 1.0, 1.5],
                        [5.62, 1.0, 2.8],
                        [5.62, -1.0, 2.8],
                    ],
                    WIN,
                    false,
                    false,
                    true,
                );
                lights(&mut o, 5.62, 0.9, 0.9);
                wheels(&mut o, &[-3.6, 3.6], 1.25);
            }
            Vehicle::Delivery => {
                // cab
                o.bx(2.0, -1.0, 0.5, 3.6, 1.0, 2.3, MARKER);
                o.bx(2.9, -1.0, 1.4, 3.62, 1.0, 2.1, WIN);
                // box body, white
                o.bx_top(
                    -3.6,
                    -1.15,
                    0.7,
                    1.9,
                    1.15,
                    3.3,
                    [232, 232, 228],
                    [210, 210, 206],
                );
                o.bx(-3.6, -1.17, 1.4, 1.9, -1.15, 2.2, MARKER);
                lights(&mut o, 3.62, 0.7, 0.9);
                wheels(&mut o, &[-2.4, 2.6], 1.0);
            }
            Vehicle::Garbage => {
                o.bx(2.4, -1.05, 0.5, 4.0, 1.05, 2.5, [226, 226, 222]);
                o.bx(3.3, -1.05, 1.5, 4.02, 1.05, 2.3, WIN);
                // green body with the hopper at the rear
                o.bx(-4.0, -1.2, 0.8, 2.2, 1.2, 3.1, MARKER);
                o.bx(-4.5, -1.0, 0.6, -4.0, 1.0, 2.0, [90, 94, 100]);
                o.bx(-4.3, -0.9, 2.0, -3.6, 0.9, 2.6, [60, 64, 70]);
                o.bx(-3.4, -1.22, 0.8, 2.0, -1.2, 1.0, [232, 190, 60]);
                lights(&mut o, 4.02, 0.7, 0.8);
                wheels(&mut o, &[-2.6, 3.0], 1.0);
            }
            Vehicle::Pickup => {
                o.bx(0.2, -0.95, 0.4, 2.5, 0.95, 1.5, MARKER);
                o.bx_top(0.4, -0.9, 1.5, 1.9, 0.9, 2.0, WIN, MARKER);
                // open bed
                o.bx(-2.4, -0.95, 0.4, 0.2, 0.95, 1.0, MARKER);
                o.bx(-2.4, -0.95, 1.0, 0.2, -0.8, 1.35, MARKER);
                o.bx(-2.4, 0.8, 1.0, 0.2, 0.95, 1.35, MARKER);
                o.bx(-2.45, -0.95, 1.0, -2.3, 0.95, 1.35, MARKER);
                lights(&mut o, 2.52, 0.6, 0.7);
                wheels(&mut o, &[-1.5, 1.6], 0.95);
            }
            Vehicle::Van => {
                o.bx(-2.4, -0.95, 0.4, 2.2, 0.95, 2.1, MARKER);
                o.bx(1.6, -0.97, 1.2, 2.45, 0.97, 1.9, MARKER);
                o.poly(
                    &[
                        [2.46, -0.8, 1.2],
                        [2.46, 0.8, 1.2],
                        [2.46, 0.8, 1.9],
                        [2.46, -0.8, 1.9],
                    ],
                    WIN,
                    false,
                    false,
                    true,
                );
                for side in [-1.0f32, 1.0] {
                    o.poly(
                        &[
                            [0.2, side * 0.96, 1.2],
                            [1.5, side * 0.96, 1.2],
                            [1.5, side * 0.96, 1.9],
                            [0.2, side * 0.96, 1.9],
                        ],
                        WIN,
                        false,
                        false,
                        true,
                    );
                }
                lights(&mut o, 2.46, 0.6, 0.6);
                wheels(&mut o, &[-1.4, 1.5], 0.95);
            }
        }
        o
    }

    pub fn mesh(self) -> Mesh {
        emit(&self.obj())
    }
}

fn emit(o: &Obj) -> Mesh {
    let mut m = Mesh::default();
    let mut id = 0u32;
    o.emit(&mut m, &mut id, 0.0, 0.0, 0.0, 1.0);
    m
}

fn lights(o: &mut Obj, x: f32, y: f32, z: f32) {
    for s in [-1.0f32, 1.0] {
        o.poly(
            &[
                [x + 0.01, s * y - 0.2, z - 0.15],
                [x + 0.01, s * y + 0.2, z - 0.15],
                [x + 0.01, s * y + 0.2, z + 0.15],
                [x + 0.01, s * y - 0.2, z + 0.15],
            ],
            HEAD,
            false,
            true,
            true,
        );
    }
}

fn wheels(o: &mut Obj, xs: &[f32], half_track: f32) {
    for &x in xs {
        for s in [-1.0f32, 1.0] {
            let y = s * half_track;
            o.bx(x - 0.5, y - 0.12, 0.0, x + 0.5, y + 0.12, 0.9, [30, 30, 34]);
        }
    }
}

// ---- props ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    TrafficLight,
    BusShelter,
    Billboard,
    Bench,
    Bin,
}

impl Prop {
    pub const ALL: [Prop; 5] = [
        Prop::TrafficLight,
        Prop::BusShelter,
        Prop::Billboard,
        Prop::Bench,
        Prop::Bin,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Prop::TrafficLight => "traffic light",
            Prop::BusShelter => "bus shelter",
            Prop::Billboard => "billboard",
            Prop::Bench => "bench",
            Prop::Bin => "bin",
        }
    }

    /// The prop's model. Its back is toward -y and its front (or the arm over the road) toward
    /// +y: the placement turns it so +y points at the road.
    pub fn obj(self) -> Obj {
        let mut o = Obj::new();
        match self {
            Prop::TrafficLight => {
                o.bx(-0.1, -0.1, 0.0, 0.1, 0.1, 5.6, STEEL);
                o.bx(-0.1, -0.1, 5.0, 0.1, 5.6, 5.3, STEEL);
                for (i, c) in [[214, 60, 52], [236, 190, 60], [70, 190, 100]]
                    .iter()
                    .enumerate()
                {
                    let y = 1.2 + i as f32 * 1.5;
                    o.bx(-0.28, y - 0.28, 4.3, 0.28, y + 0.28, 5.3, [40, 42, 48]);
                    for x in [-0.29f32, 0.29] {
                        o.poly(
                            &[
                                [x, y - 0.2, 4.45],
                                [x, y + 0.2, 4.45],
                                [x, y + 0.2, 4.85],
                                [x, y - 0.2, 4.85],
                            ],
                            *c,
                            false,
                            false,
                            true,
                        );
                    }
                }
                // signal head facing the road traffic and a pedestrian box on the pole
                o.bx(-0.25, -0.3, 2.2, 0.25, 0.2, 3.0, [40, 42, 48]);
            }
            Prop::BusShelter => {
                let roof = [60, 150, 190];
                o.bx(-2.0, -1.0, 2.5, 2.0, 1.0, 2.7, roof);
                for x in [-1.9f32, 1.8] {
                    o.bx(x, -0.9, 0.0, x + 0.12, -0.78, 2.5, STEEL);
                    o.bx(x, 0.8, 0.0, x + 0.12, 0.92, 2.5, STEEL);
                }
                // glass back and a bench
                o.poly(
                    &[
                        [-1.9, -0.95, 0.5],
                        [1.9, -0.95, 0.5],
                        [1.9, -0.95, 2.4],
                        [-1.9, -0.95, 2.4],
                    ],
                    [150, 196, 220],
                    false,
                    true,
                    true,
                );
                o.bx(-1.5, -0.7, 0.6, 1.5, -0.2, 0.75, [150, 112, 82]);
                // route sign
                o.bx(2.2, 0.6, 0.0, 2.3, 0.7, 3.0, STEEL);
                o.bx(1.9, 0.55, 3.0, 2.6, 0.75, 3.5, [230, 190, 60]);
            }
            Prop::Billboard => {
                for x in [-3.0f32, 3.0] {
                    o.bx(x - 0.15, -0.15, 0.0, x + 0.15, 0.15, 6.0, [120, 124, 132]);
                }
                o.bx(-4.5, -0.3, 5.5, 4.5, 0.3, 9.0, [90, 94, 100]);
                // the face, on the road side
                o.poly(
                    &[
                        [-4.3, 0.32, 5.7],
                        [4.3, 0.32, 5.7],
                        [4.3, 0.32, 8.8],
                        [-4.3, 0.32, 8.8],
                    ],
                    MARKER,
                    false,
                    true,
                    true,
                );
                o.poly(
                    &[
                        [-2.0, 0.34, 6.4],
                        [2.0, 0.34, 6.4],
                        [2.0, 0.34, 8.1],
                        [-2.0, 0.34, 8.1],
                    ],
                    [240, 236, 224],
                    false,
                    true,
                    true,
                );
            }
            Prop::Bench => {
                o.bx(-0.9, -0.25, 0.4, 0.9, 0.25, 0.55, [150, 112, 82]);
                o.bx(-0.9, -0.3, 0.55, 0.9, -0.2, 1.0, [150, 112, 82]);
                for x in [-0.8f32, 0.7] {
                    o.bx(x, -0.25, 0.0, x + 0.1, 0.25, 0.4, STEEL);
                }
            }
            Prop::Bin => {
                o.prism(
                    0.0,
                    0.0,
                    0.0,
                    0.9,
                    0.3,
                    [70, 120, 90],
                    8,
                    Some([50, 90, 70]),
                );
                o.bx(-0.32, -0.04, 0.9, 0.32, 0.04, 0.96, STEEL);
            }
        }
        o
    }

    pub fn mesh(self) -> Mesh {
        emit(&self.obj())
    }
}

const AD_COLOURS: [Rgb; 6] = [
    [196, 58, 52],
    [48, 92, 150],
    [226, 170, 50],
    [70, 130, 96],
    [150, 70, 160],
    [236, 120, 60],
];

type Grid = std::collections::BTreeMap<(i32, i32), Vec<(usize, [f32; 2])>>;

/// All fixed props for a set of roads and lots, one list per prop kind (indexed by
/// `Prop::index`). `centre` is the middle of downtown, where benches and bins stand.
pub fn make_props(paths: &[Path], lots: &[LotInfo], centre: [f32; 2]) -> [Vec<Instance>; 5] {
    let mut out: [Vec<Instance>; 5] = Default::default();
    let mut rng = Rng::new(0x9709, 7);
    let near_lot = |x: f32, y: f32, r: f32| -> bool {
        lots.iter()
            .any(|l| (l.x - x).hypot(l.y - y) < r + lot_reach(l.kind))
    };
    let put = |out: &mut [Vec<Instance>; 5], prop: Prop, pos: [f32; 3], theta: f32, c: u32| {
        let n = out[prop.index()].len() as u32;
        out[prop.index()].push(Instance {
            pos,
            theta,
            scale: 1.0,
            tint: c,
            id: PROP_ID_BASE + (prop.index() as u32 * 200_000 + n) * ID_STRIDE / 4,
        });
    };
    // junctions: end points of arterial and collector roads that meet another road
    let mut grid: Grid = Default::default();
    for (i, p) in paths.iter().enumerate() {
        for v in &p.pts {
            grid.entry(((v[0] / 6.0).floor() as i32, (v[1] / 6.0).floor() as i32))
                .or_default()
                .push((i, *v));
        }
    }
    let mut lit: Vec<[f32; 2]> = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        if !matches!(p.class, RoadClass::Arterial) {
            continue;
        }
        for end in [p.pts.first(), p.pts.last()].into_iter().flatten() {
            let (gx, gy) = ((end[0] / 6.0).floor() as i32, (end[1] / 6.0).floor() as i32);
            let mut meets = false;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(v) = grid.get(&(gx + dx, gy + dy)) {
                        meets |= v.iter().any(|(j, q)| {
                            *j != i
                                && paths[*j].class != RoadClass::Rail
                                && (q[0] - end[0]).hypot(q[1] - end[1]) < 5.0
                        });
                    }
                }
            }
            if !meets
                || lit
                    .iter()
                    .any(|q| (q[0] - end[0]).hypot(q[1] - end[1]) < 14.0)
            {
                continue;
            }
            lit.push(*end);
        }
    }
    for q in &lit {
        // the nearest arterial gives the way the corner faces
        let Some((p, s)) = nearest_on(paths, *q, |c| c == RoadClass::Arterial) else {
            continue;
        };
        let (pt, d) = paths[p].at(s);
        let hw = paths[p].class.half_width() as f32;
        let nrm = [-d[1], d[0]];
        // a pole on the right hand corner, its arm over the road
        let pos = [pt[0] + nrm[0] * (hw + 1.0), pt[1] + nrm[1] * (hw + 1.0)];
        let theta = libm::atan2f(-nrm[1], -nrm[0]) - std::f32::consts::FRAC_PI_2;
        put(
            &mut out,
            Prop::TrafficLight,
            [pos[0], pos[1], paths[p].z(s)],
            theta,
            0,
        );
    }
    // bus shelters and billboards along the big roads
    for (i, p) in paths.iter().enumerate() {
        let l = p.length();
        let (spacing, shelter) = match p.class {
            RoadClass::Arterial => (170.0, true),
            RoadClass::Collector => (240.0, true),
            RoadClass::Highway => (260.0, false),
            _ => continue,
        };
        let hw = p.class.half_width() as f32;
        let mut s = 40.0 + (i % 7) as f32 * 9.0;
        let mut k = 0usize;
        while s < l - 30.0 {
            let (pt, d) = p.at(s);
            let side = if k.is_multiple_of(2) { 1.0f32 } else { -1.0 };
            let nrm = [-d[1] * side, d[0] * side];
            if shelter {
                let pos = [pt[0] + nrm[0] * (hw + 2.4), pt[1] + nrm[1] * (hw + 2.4)];
                if !near_lot(pos[0], pos[1], 3.0) {
                    let theta = libm::atan2f(-nrm[1], -nrm[0]) - std::f32::consts::FRAC_PI_2;
                    put(
                        &mut out,
                        Prop::BusShelter,
                        [pos[0], pos[1], p.z(s)],
                        theta,
                        0,
                    );
                }
            } else {
                let pos = [pt[0] + nrm[0] * (hw + 12.0), pt[1] + nrm[1] * (hw + 12.0)];
                if !near_lot(pos[0], pos[1], 6.0) {
                    let theta = libm::atan2f(-nrm[1], -nrm[0]) - std::f32::consts::FRAC_PI_2;
                    let c = tint(rng.pick(&AD_COLOURS));
                    put(
                        &mut out,
                        Prop::Billboard,
                        [pos[0], pos[1], p.z(s)],
                        theta,
                        c,
                    );
                }
            }
            s += spacing;
            k += 1;
        }
        // arterial frontage billboards, spaced wider than shelters
        if p.class == RoadClass::Arterial {
            let mut s = 120.0 + (i % 5) as f32 * 11.0;
            while s < l - 40.0 {
                let (pt, d) = p.at(s);
                let side = if (s as i32 / 100) % 2 == 0 {
                    1.0f32
                } else {
                    -1.0
                };
                let nrm = [-d[1] * side, d[0] * side];
                let pos = [pt[0] + nrm[0] * (hw + 11.0), pt[1] + nrm[1] * (hw + 11.0)];
                if !near_lot(pos[0], pos[1], 6.0) && rng.chance(0.5) {
                    let theta = libm::atan2f(-nrm[1], -nrm[0]) - std::f32::consts::FRAC_PI_2;
                    let c = tint(rng.pick(&AD_COLOURS));
                    put(
                        &mut out,
                        Prop::Billboard,
                        [pos[0], pos[1], p.z(s)],
                        theta,
                        c,
                    );
                }
                s += 330.0;
            }
        }
    }
    // benches and bins downtown, by the pavement of the streets nearest the middle
    for p in paths {
        if !matches!(
            p.class,
            RoadClass::Local | RoadClass::Collector | RoadClass::Arterial
        ) {
            continue;
        }
        let hw = p.class.half_width() as f32;
        let l = p.length();
        let mut s = 8.0;
        let mut k = 0usize;
        while s < l - 4.0 {
            let (pt, d) = p.at(s);
            if (pt[0] - centre[0]).hypot(pt[1] - centre[1]) < 170.0 {
                let side = if k.is_multiple_of(2) { 1.0f32 } else { -1.0 };
                let nrm = [-d[1] * side, d[0] * side];
                let pos = [pt[0] + nrm[0] * (hw + 0.8), pt[1] + nrm[1] * (hw + 0.8)];
                let theta = libm::atan2f(-nrm[1], -nrm[0]) - std::f32::consts::FRAC_PI_2;
                let prop = if k % 3 == 2 { Prop::Bin } else { Prop::Bench };
                put(&mut out, prop, [pos[0], pos[1], p.z(s)], theta, 0);
            }
            s += 34.0;
            k += 1;
        }
    }
    out
}

fn lot_reach(k: LotKind) -> f32 {
    k.nominal_radius() as f32
}

fn nearest_on(paths: &[Path], q: [f32; 2], ok: impl Fn(RoadClass) -> bool) -> Option<(usize, f32)> {
    let mut best: Option<(f32, usize, f32)> = None;
    for (i, p) in paths.iter().enumerate() {
        if !ok(p.class) {
            continue;
        }
        for (j, v) in p.pts.iter().enumerate() {
            let d = (v[0] - q[0]).hypot(v[1] - q[1]);
            if d < 6.0 && best.is_none_or(|b| d < b.0) {
                best = Some((d, i, p.cum[j]));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

// ---- moving vehicles ----

/// One street vehicle on its road.
#[derive(Clone, Copy, Debug)]
pub struct Veh {
    pub kind: Vehicle,
    pub road: u32,
    pub phase: f32,
    pub dir: f32,
    pub speed: f32,
    pub colour: Rgb,
}

/// How many of each vehicle a town of this population runs: (buses, delivery trucks, garbage
/// trucks, pickups, vans). Buses come with the town: one at 600 people, then one more for every
/// 1,400.
pub fn fleet_size(population: u32) -> [usize; 5] {
    let p = population as usize;
    let bus = if p < 600 {
        0
    } else {
        (1 + (p - 600) / 1400).min(40)
    };
    [
        bus,
        (p / 700).clamp(0, 60),
        (p / 2500).clamp(0, 12),
        (p / 400).clamp(0, 90),
        (p / 500).clamp(0, 70),
    ]
}

/// Crude sum of shops and industry beside a road, used to send delivery trucks where the
/// goods go.
fn goods_score(p: &Path, lots: &[LotInfo]) -> f32 {
    let mut s = 0.0;
    let mut n = 0.0;
    let l = p.length();
    let mut at = 0.0;
    while at <= l {
        let (pt, _) = p.at(at);
        for lot in lots {
            let d = (lot.x - pt[0]).hypot(lot.y - pt[1]);
            if d < 30.0 {
                s += match lot.kind {
                    LotKind::Shop | LotKind::Strip | LotKind::BigBox | LotKind::FastFood => 1.0,
                    LotKind::Workshop | LotKind::Warehouse | LotKind::Gas => 1.2,
                    LotKind::Office => 0.5,
                    _ => 0.0,
                };
            }
        }
        n += 1.0;
        at += 30.0;
    }
    if n == 0.0 { 0.0 } else { s / n }
}

pub fn make_fleet(paths: &[Path], lots: &[LotInfo], population: u32) -> Vec<Veh> {
    let want = fleet_size(population);
    let mut out = Vec::new();
    let mut rng = Rng::new(0xf1ee7, 3);
    let bus_roads: Vec<usize> = {
        let mut v: Vec<(usize, f32)> = paths
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                matches!(p.class, RoadClass::Arterial | RoadClass::Collector) && p.length() > 80.0
            })
            .map(|(i, p)| {
                (
                    i,
                    p.length()
                        * if p.class == RoadClass::Arterial {
                            1.5
                        } else {
                            1.0
                        },
                )
            })
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        v.into_iter().map(|a| a.0).collect()
    };
    for k in 0..want[0] {
        if bus_roads.is_empty() {
            break;
        }
        let road = bus_roads[k % bus_roads.len()];
        out.push(Veh {
            kind: Vehicle::Bus,
            road: road as u32,
            phase: rng.f32(),
            dir: if (k / bus_roads.len()).is_multiple_of(2) {
                1.0
            } else {
                -1.0
            },
            speed: 8.5 * rng.range(0.9, 1.1),
            colour: [48, 110, 170],
        });
    }
    // delivery trucks: the roads with the most shops and industry beside them
    let mut goods: Vec<(usize, f32)> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            matches!(
                p.class,
                RoadClass::Arterial | RoadClass::Collector | RoadClass::Local
            ) && p.length() > 40.0
        })
        .map(|(i, p)| (i, goods_score(p, lots)))
        .filter(|(_, s)| *s > 0.0)
        .collect();
    goods.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    goods.truncate(goods.len().min(want[1].max(1) * 2));
    for k in 0..want[1] {
        if goods.is_empty() {
            break;
        }
        let road = goods[k % goods.len()].0;
        out.push(Veh {
            kind: Vehicle::Delivery,
            road: road as u32,
            phase: rng.f32(),
            dir: if rng.chance(0.5) { 1.0 } else { -1.0 },
            speed: 7.0 * rng.range(0.9, 1.1),
            colour: rng.pick(&[
                [196u8, 58, 52],
                [48, 92, 150],
                [60, 130, 96],
                [226, 170, 50],
            ]),
        });
    }
    let homes: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            matches!(p.class, RoadClass::Local | RoadClass::Collector) && p.length() > 30.0
        })
        .map(|(i, _)| i)
        .collect();
    let any: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            matches!(
                p.class,
                RoadClass::Local | RoadClass::Collector | RoadClass::Arterial
            ) && p.length() > 30.0
        })
        .map(|(i, _)| i)
        .collect();
    let mut add =
        |kind: Vehicle, n: usize, pool: &[usize], speed: f32, cols: &[Rgb], rng: &mut Rng| {
            if pool.is_empty() {
                return;
            }
            for _ in 0..n {
                out.push(Veh {
                    kind,
                    road: pool[rng.below(pool.len())] as u32,
                    phase: rng.f32(),
                    dir: if rng.chance(0.5) { 1.0 } else { -1.0 },
                    speed: speed * rng.range(0.85, 1.15),
                    colour: rng.pick(cols),
                });
            }
        };
    add(
        Vehicle::Garbage,
        want[2],
        &homes,
        4.0,
        &[[60, 130, 96], [70, 110, 150]],
        &mut rng,
    );
    add(
        Vehicle::Pickup,
        want[3],
        &any,
        9.0,
        &[
            [196, 58, 52],
            [48, 84, 150],
            [226, 226, 222],
            [60, 62, 70],
            [150, 112, 82],
        ],
        &mut rng,
    );
    add(
        Vehicle::Van,
        want[4],
        &any,
        9.0,
        &[
            [226, 226, 222],
            [48, 84, 150],
            [60, 130, 96],
            [230, 190, 60],
        ],
        &mut rng,
    );
    out
}

/// Where each vehicle is at `time` seconds, by vehicle kind.
pub fn fleet_instances(paths: &[Path], fleet: &[Veh], time: f32) -> [Vec<Instance>; 5] {
    let mut out: [Vec<Instance>; 5] = Default::default();
    for (n, v) in fleet.iter().enumerate() {
        let p = &paths[v.road as usize];
        let l = p.length();
        if l < 1.0 {
            continue;
        }
        let dir = p
            .one_way
            .map_or(v.dir, |forward| if forward { 1.0 } else { -1.0 });
        let s = (v.phase * l + dir * v.speed * time).rem_euclid(l);
        let (pt, d) = p.at(s);
        let hw = p.class.half_width() as f32;
        let side = if p.one_way.is_some() {
            0.0
        } else {
            -dir * hw * 0.5
        };
        let (nx, ny) = (-d[1], d[0]);
        let heading = libm::atan2f(d[1] * dir, d[0] * dir);
        out[v.kind.index()].push(Instance {
            pos: [pt[0] + nx * side, pt[1] + ny * side, p.z(s)],
            theta: heading,
            scale: 1.0,
            tint: tint(v.colour),
            id: FLEET_ID_BASE + n as u32 * ID_STRIDE,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_grows_with_population() {
        assert_eq!(fleet_size(0), [0, 0, 0, 0, 0]);
        assert_eq!(fleet_size(600)[0], 1);
        assert!(fleet_size(8000)[0] > fleet_size(2000)[0]);
        assert!(fleet_size(8000)[1] > fleet_size(2000)[1]);
    }

    #[test]
    fn every_model_has_faces() {
        for v in Vehicle::ALL {
            assert!(v.mesh().triangles() > 20, "{}", v.name());
        }
        for p in Prop::ALL {
            assert!(p.mesh().triangles() > 8, "{}", p.name());
        }
    }
}
