//! Trains on the railways: passenger trains (a locomotive and three to five coaches) that stop at
//! an open depot, and freight trains (a locomotive with hopper or box cars) that slow by the grain
//! elevator. Each kind of vehicle is one small model drawn many times, like the cars.
//!
//! Which trains are on a line at a moment is a pure function of the clock: train `k` of a line
//! leaves at `k` times the headway and follows a fixed speed profile (cruise, slow, stop, go).

use crate::instances::{HEADLIGHT, ID_STRIDE, Instance, MARKER, Path, tint};
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use us_gen::RoadClass;

pub const TRAIN_ID_BASE: u32 = 12_000_000;
/// Outline ids reserved per vehicle: a vehicle has far more faces than a car.
pub const TRAIN_ID_STRIDE: u32 = ID_STRIDE * 8;
const KIND_ID_SPAN: u32 = 40_000;
/// Most vehicles of one kind drawn at once.
pub const MAX_PER_KIND: usize = 300;

pub const LOCO_LEN: f32 = 16.0;
pub const COACH_LEN: f32 = 20.0;
pub const HOPPER_LEN: f32 = 14.0;
pub const BOXCAR_LEN: f32 = 15.0;
const GAP: f32 = 1.0;
/// Height of the rail head above the road bed.
const RAIL_Z: f32 = 0.25;

/// Game-hour rates become animation seconds between trains: a game hour is a few seconds of
/// play, so the headway is scaled to leave a train in view now and then, not a constant stream.
const HEADWAY_SCALE: f32 = 600.0;
const PASSENGER_SPEED: f32 = 26.0;
const FREIGHT_SPEED: f32 = 18.0;
const DWELL_S: f32 = 14.0;
/// A depot or elevator this close to a rail line belongs to it.
const NEAR_LINE_M: f32 = 60.0;

/// What runs on the rails right now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrainService {
    pub passenger_per_hour: f32,
    pub freight_per_hour: f32,
    /// Centres of depots in service. Trains stop only at these.
    pub depots: Vec<[f32; 2]>,
    /// Centres of grain elevators, where freight slows.
    pub elevators: Vec<[f32; 2]>,
}

/// Which model of vehicle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vehicle {
    Loco,
    Coach,
    Hopper,
    Boxcar,
}

impl Vehicle {
    pub const ALL: [Vehicle; 4] = [
        Vehicle::Loco,
        Vehicle::Coach,
        Vehicle::Hopper,
        Vehicle::Boxcar,
    ];

    pub fn length(self) -> f32 {
        match self {
            Vehicle::Loco => LOCO_LEN,
            Vehicle::Coach => COACH_LEN,
            Vehicle::Hopper => HOPPER_LEN,
            Vehicle::Boxcar => BOXCAR_LEN,
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    pub fn mesh(self) -> Mesh {
        let o = match self {
            Vehicle::Loco => loco(),
            Vehicle::Coach => coach(),
            Vehicle::Hopper => hopper(),
            Vehicle::Boxcar => boxcar(),
        };
        let mut m = Mesh::default();
        let mut id = 0u32;
        o.emit(&mut m, &mut id, 0.0, 0.0, 0.0, 1.0);
        m
    }
}

/// The instances of every vehicle kind, indexed like [`Vehicle::ALL`].
#[derive(Default, Clone)]
pub struct TrainInstances {
    pub sets: [Vec<Instance>; 4],
}

impl TrainInstances {
    pub fn total(&self) -> usize {
        self.sets.iter().map(Vec::len).sum()
    }
}

const STEEL: Rgb = [58, 62, 70];
const ROOF: Rgb = [196, 200, 206];
const WINDOW: Rgb = [70, 96, 120];

/// Bogies and a frame under a vehicle of length `len`, width 2.9 m.
fn underframe(o: &mut Obj, len: f32) {
    let h = len / 2.0;
    o.bx(-h + 0.3, -1.3, 0.25, h - 0.3, 1.3, 0.9, STEEL);
    for x in [-h * 0.62, h * 0.62] {
        o.bx(x - 1.4, -1.45, 0.0, x + 1.4, 1.45, 0.7, [36, 38, 44]);
    }
}

/// A diesel locomotive: 16 m, long hood and a cab, in the marker colour.
pub fn loco() -> Obj {
    let mut o = Obj::new();
    let h = LOCO_LEN / 2.0;
    underframe(&mut o, LOCO_LEN);
    o.bx(-h, -1.45, 0.9, h, 1.45, 2.9, MARKER);
    o.bx_top(-h + 0.6, -1.3, 2.9, 2.0, 1.3, 3.5, MARKER, [90, 94, 100]);
    o.bx_top(2.4, -1.45, 2.9, h, 1.45, 4.3, MARKER, ROOF);
    o.bx(2.5, -1.5, 3.0, h - 0.1, -1.46, 4.0, WINDOW);
    o.bx(2.5, 1.46, 3.0, h - 0.1, 1.5, 4.0, WINDOW);
    for y in [-0.9f32, 0.9] {
        o.poly(
            &[
                [h + 0.02, y - 0.3, 1.6],
                [h + 0.02, y + 0.3, 1.6],
                [h + 0.02, y + 0.3, 2.2],
                [h + 0.02, y - 0.3, 2.2],
            ],
            HEADLIGHT,
            false,
            true,
            true,
        );
    }
    o
}

/// A passenger coach: 20 m, a row of windows under a pale roof.
pub fn coach() -> Obj {
    let mut o = Obj::new();
    let h = COACH_LEN / 2.0;
    underframe(&mut o, COACH_LEN);
    o.bx_top(-h, -1.45, 0.9, h, 1.45, 3.9, MARKER, ROOF);
    let mut x = -h + 1.6;
    while x < h - 2.0 {
        o.bx(x, -1.47, 1.8, x + 1.4, -1.45, 3.1, WINDOW);
        o.bx(x, 1.45, 1.8, x + 1.4, 1.47, 3.1, WINDOW);
        x += 2.4;
    }
    o
}

/// A covered hopper: 14 m, a ribbed body with a roof hatch row.
pub fn hopper() -> Obj {
    let mut o = Obj::new();
    let h = HOPPER_LEN / 2.0;
    underframe(&mut o, HOPPER_LEN);
    o.bx_top(-h, -1.4, 0.9, h, 1.4, 3.5, MARKER, [150, 150, 154]);
    for x in [-h * 0.5, 0.0, h * 0.5] {
        o.bx(x - 0.4, -0.5, 3.5, x + 0.4, 0.5, 3.85, [110, 112, 118]);
    }
    o
}

/// A box car: 15 m, a plain body with a sliding door.
pub fn boxcar() -> Obj {
    let mut o = Obj::new();
    let h = BOXCAR_LEN / 2.0;
    underframe(&mut o, BOXCAR_LEN);
    o.bx_top(-h, -1.45, 0.9, h, 1.45, 4.1, MARKER, [128, 112, 100]);
    o.bx(-1.4, -1.5, 1.0, 1.4, -1.46, 3.7, [70, 54, 44]);
    o.bx(-1.4, 1.46, 1.0, 1.4, 1.5, 3.7, [70, 54, 44]);
    o
}

const PASSENGER_COLS: [Rgb; 4] = [[196, 58, 52], [48, 84, 150], [60, 130, 96], [226, 226, 218]];
const FREIGHT_COLS: [Rgb; 5] = [
    [150, 70, 52],
    [96, 92, 88],
    [214, 170, 60],
    [70, 106, 130],
    [120, 54, 48],
];
const LOCO_COLS: [Rgb; 3] = [[196, 58, 52], [236, 188, 56], [54, 70, 110]];

/// Distance covered by a train after `i * STEP` seconds, with its stop and slow zones.
struct Profile {
    /// Front position along the line at each step.
    s: Vec<f32>,
}

const STEP: f32 = 0.5;

impl Profile {
    /// `stop`: where the front halts (and dwells). `slow`: the middle of a zone to crawl through.
    fn build(total: f32, vmax: f32, stop: Option<f32>, slow: Option<f32>) -> Profile {
        let mut s = vec![0.0f32];
        let (mut pos, mut v) = (0.0f32, vmax);
        let mut dwell = 0.0f32;
        let mut dwelled = stop.is_none();
        let mut guard = 0;
        while pos < total && guard < 20_000 {
            guard += 1;
            if dwell > 0.0 {
                dwell -= STEP;
                s.push(pos);
                continue;
            }
            let mut target = vmax;
            if let Some(sp) = stop {
                let d = sp - pos;
                if !dwelled && d <= 0.6 {
                    dwell = DWELL_S;
                    dwelled = true;
                    v = 0.0;
                    s.push(pos);
                    continue;
                }
                if !dwelled {
                    target = target.min(1.5 + d * 0.16);
                } else {
                    target = target.min(1.5 + (pos - sp) * 0.12);
                }
            }
            if let Some(c) = slow {
                let k = ((pos - c).abs() / 160.0).clamp(0.0, 1.0);
                target = target.min(vmax * (0.4 + 0.6 * k));
            }
            // limited acceleration and braking
            let dv = (target - v).clamp(-3.5 * STEP, 1.8 * STEP);
            v = (v + dv).max(1.0);
            pos += v * STEP;
            if let Some(sp) = stop
                && !dwelled
                && pos > sp
            {
                pos = sp;
            }
            s.push(pos);
        }
        Profile { s }
    }

    fn duration(&self) -> f32 {
        (self.s.len() - 1) as f32 * STEP
    }

    fn at(&self, tau: f32) -> f32 {
        if tau <= 0.0 {
            return self.s[0];
        }
        let f = tau / STEP;
        let i = f as usize;
        if i + 1 >= self.s.len() {
            return *self.s.last().unwrap();
        }
        let t = f - i as f32;
        self.s[i] + (self.s[i + 1] - self.s[i]) * t
    }
}

/// One direction of travel on a line, one kind of train.
struct Run {
    profile: Profile,
    headway: f32,
    freight: bool,
}

/// A rail line with its train runs, ready to turn into instances.
struct Line {
    path: usize,
    runs: Vec<Run>,
}

/// Everything needed to place trains for a time value.
#[derive(Default)]
pub struct Trains {
    lines: Vec<Line>,
    pub service: TrainService,
}

fn nearest_s(p: &Path, q: [f32; 2]) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for i in 1..p.pts.len() {
        let (a, b) = (p.pts[i - 1], p.pts[i]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = (dx * dx + dy * dy).max(1e-6);
        let t = (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
        let (cx, cy) = (a[0] + dx * t, a[1] + dy * t);
        let d = (q[0] - cx).hypot(q[1] - cy);
        if best.is_none_or(|b| d < b.1) {
            best = Some((p.cum[i - 1] + (p.cum[i] - p.cum[i - 1]) * t, d));
        }
    }
    best
}

impl Trains {
    /// Plan the runs for the rail paths among `paths`. Passenger trains stop at the first open
    /// depot on their line, and pass through where there is none.
    pub fn plan(paths: &[Path], service: TrainService) -> Trains {
        let mut lines = Vec::new();
        for (pi, p) in paths.iter().enumerate() {
            if p.class != RoadClass::Rail || p.length() < 150.0 {
                continue;
            }
            let l = p.length();
            let on_line = |pts: &[[f32; 2]]| -> Option<f32> {
                pts.iter()
                    .filter_map(|&q| nearest_s(p, q))
                    .filter(|b| b.1 < NEAR_LINE_M)
                    .map(|b| b.0)
                    .next()
            };
            let depot = on_line(&service.depots);
            let elevator = on_line(&service.elevators);
            let mut runs = Vec::new();
            // each kind runs both ways: forward trains and reverse trains share the table, the
            // reverse ones mirror it
            for dir in [1.0f32, -1.0] {
                for freight in [false, true] {
                    let per_hour = if freight {
                        service.freight_per_hour
                    } else {
                        service.passenger_per_hour
                    };
                    if per_hour <= 0.01 {
                        continue;
                    }
                    let len = if freight { 150.0 } else { 100.0 };
                    let at = |s: Option<f32>| s.map(|s| if dir > 0.0 { s } else { l - s });
                    let stop = if freight {
                        None
                    } else {
                        depot.map(|s| at(Some(s)).unwrap() + len * 0.5)
                    };
                    let slow = if freight {
                        elevator.map(|s| at(Some(s)).unwrap() + len * 0.5)
                    } else {
                        None
                    };
                    let vmax = if freight {
                        FREIGHT_SPEED
                    } else {
                        PASSENGER_SPEED
                    };
                    runs.push(Run {
                        profile: Profile::build(l + len, vmax, stop, slow),
                        // both directions share the line's headway between them
                        headway: HEADWAY_SCALE / per_hour * 2.0,
                        freight,
                    });
                }
            }
            lines.push(Line { path: pi, runs });
        }
        Trains { lines, service }
    }

    /// Where every vehicle is at `time` seconds.
    pub fn instances(&self, paths: &[Path], time: f32) -> TrainInstances {
        let mut out = TrainInstances::default();
        let mut counts = [0u32; 4];
        for line in &self.lines {
            let p = &paths[line.path];
            let l = p.length();
            for (ri, run) in line.runs.iter().enumerate() {
                let dur = run.profile.duration();
                let dir = if ri < line.runs.len() / 2 {
                    1.0f32
                } else {
                    -1.0
                };
                let first = ((time - dur) / run.headway).floor() as i64;
                let last = (time / run.headway).floor() as i64;
                for k in first..=last {
                    let dep = k as f32 * run.headway + line.path as f32 * 37.0;
                    let tau = time - dep;
                    if !(0.0..dur).contains(&tau) {
                        continue;
                    }
                    let front = run.profile.at(tau);
                    let mut rng = Rng::new(
                        0x7a1d_0000 + ri as u64,
                        (k as u64) ^ (line.path as u64) << 40,
                    );
                    let consist = consist(run.freight, &mut rng);
                    // the front vehicle first, then the rest behind it
                    let mut s_front = front;
                    for (veh, colour) in consist {
                        let mid = s_front - veh.length() / 2.0;
                        s_front -= veh.length() + GAP;
                        let n = &mut counts[veh.index()];
                        if mid < 0.0 || mid > l || *n as usize >= MAX_PER_KIND {
                            continue;
                        }
                        let s = if dir > 0.0 { mid } else { l - mid };
                        let (pt, d) = p.at(s);
                        let heading = libm::atan2f(d[1] * dir, d[0] * dir);
                        out.sets[veh.index()].push(Instance {
                            pos: [pt[0], pt[1], p.z(s) + RAIL_Z],
                            theta: heading,
                            scale: 1.0,
                            tint: tint(colour),
                            id: TRAIN_ID_BASE
                                + veh.index() as u32 * KIND_ID_SPAN
                                + *n * TRAIN_ID_STRIDE,
                        });
                        *n += 1;
                    }
                }
            }
        }
        out
    }

    /// Whether any train runs at all.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// The vehicles of one train, front first, each with its colour.
fn consist(freight: bool, rng: &mut Rng) -> Vec<(Vehicle, Rgb)> {
    let mut v = Vec::new();
    if freight {
        v.push((Vehicle::Loco, rng.pick(&LOCO_COLS)));
        let n = 6 + rng.below(7);
        let hoppers = rng.chance(0.5);
        for _ in 0..n {
            let veh = if hoppers || rng.chance(0.25) {
                Vehicle::Hopper
            } else {
                Vehicle::Boxcar
            };
            v.push((veh, rng.pick(&FREIGHT_COLS)));
        }
    } else {
        let body = rng.pick(&PASSENGER_COLS);
        v.push((Vehicle::Loco, rng.pick(&LOCO_COLS)));
        for _ in 0..3 + rng.below(3) {
            v.push((Vehicle::Coach, body));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::pt;

    fn straight(len: f32) -> Path {
        Path::new(RoadClass::Rail, &[pt(0.0, 100.0), pt(len as f64, 100.0)])
    }

    fn service(depot: bool) -> TrainService {
        TrainService {
            passenger_per_hour: 4.0,
            freight_per_hour: 2.0,
            depots: if depot { vec![[1000.0, 118.0]] } else { vec![] },
            elevators: vec![],
        }
    }

    #[test]
    fn trains_follow_the_line() {
        let paths = vec![straight(2000.0)];
        let t = Trains::plan(&paths, service(true));
        assert!(!t.is_empty());
        let mut seen = 0;
        for k in 0..400 {
            let inst = t.instances(&paths, k as f32 * 3.0);
            for set in &inst.sets {
                for i in set {
                    assert!((i.pos[1] - 100.0).abs() < 0.01, "off the rail: {:?}", i.pos);
                    assert!((-0.1..=2000.1).contains(&i.pos[0]));
                    seen += 1;
                }
            }
        }
        assert!(seen > 0, "some train runs");
    }

    #[test]
    fn same_clock_same_trains() {
        let paths = vec![straight(2000.0)];
        let a = Trains::plan(&paths, service(true));
        let b = Trains::plan(&paths, service(true));
        for time in [0.0, 17.3, 400.0, 1234.5] {
            let (x, y) = (a.instances(&paths, time), b.instances(&paths, time));
            for k in 0..4 {
                assert_eq!(x.sets[k].len(), y.sets[k].len());
                for (p, q) in x.sets[k].iter().zip(&y.sets[k]) {
                    assert_eq!(p.pos, q.pos);
                }
            }
        }
    }

    #[test]
    fn a_passenger_train_stops_at_an_open_depot_and_not_without_one() {
        let paths = vec![straight(2000.0)];
        let stopped = |svc: TrainService| -> bool {
            let t = Trains::plan(&paths, svc);
            // look for a locomotive near the depot that has not moved over 8 s
            let mut prev: Option<f32> = None;
            for k in 0..4000 {
                let time = k as f32 * 0.5;
                let inst = t.instances(&paths, time);
                // forward passenger locos are the ones in set 0 on the first run: track the
                // one nearest the depot
                let near = inst.sets[0]
                    .iter()
                    .map(|i| i.pos[0])
                    .filter(|x| (*x - 1050.0).abs() < 60.0)
                    .fold(None, |a: Option<f32>, x| Some(a.map_or(x, |a| a.max(x))));
                if let (Some(p), Some(n)) = (prev, near)
                    && (n - p).abs() < 0.01
                {
                    return true;
                }
                prev = near;
            }
            false
        };
        assert!(stopped(service(true)), "stops at the depot");
        assert!(!stopped(service(false)), "passes through without one");
    }

    #[test]
    fn frequency_grows_with_service() {
        let paths = vec![straight(3000.0)];
        let count = |ph: f32| {
            let t = Trains::plan(
                &paths,
                TrainService {
                    passenger_per_hour: ph,
                    ..Default::default()
                },
            );
            (0..600)
                .map(|k| t.instances(&paths, k as f32 * 2.0).total())
                .sum::<usize>()
        };
        assert!(count(8.0) > count(1.0));
    }

    #[test]
    fn models_fit_their_stride() {
        for (v, o) in [
            (Vehicle::Loco, loco()),
            (Vehicle::Coach, coach()),
            (Vehicle::Hopper, hopper()),
            (Vehicle::Boxcar, boxcar()),
        ] {
            assert!(v.mesh().triangles() > 0);
            let ids = o.polys.iter().filter(|p| p.outline).count() as u32;
            assert!(ids <= TRAIN_ID_STRIDE, "{v:?} uses {ids} outline ids");
        }
    }
}
