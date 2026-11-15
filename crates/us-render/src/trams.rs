//! Tram lines: the tracks in the street with overhead wire poles, and the trams on them.
//!
//! The simulation owns the lines. The renderer gets a [`TramService`]: for every line its path,
//! where its stops are, how often trams run and whether it is open. Which trams are on a line at
//! a moment is a pure function of the clock, as for trains: tram `k` leaves at `k` times the
//! headway and follows a fixed profile (cruise, brake, dwell at every stop, go).

use crate::ground::{Ground, layer};
use crate::instances::MARKER;
use crate::instances::{HEADLIGHT, Instance, Path, tint};
use crate::land::Land;
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use crate::trains::{KIND_ID_SPAN, TRAIN_ID_BASE, TRAIN_ID_STRIDE, TrainInstances, Vehicle};
use us_gen::{Pt, RoadClass, pt};

/// Length of one tram section, metres.
pub const SECTION_LEN: f32 = 9.0;
const JOINT_LEN: f32 = 1.0;
/// Sideways distance of a track from the middle of the street, metres.
pub const TRACK_OFFSET: f32 = 1.9;
const RAIL_GAUGE_HALF: f32 = 0.72;
/// Height of the wire above the street, metres.
const WIRE_Z: f32 = 4.9;
const POLE_GAP: f32 = 30.0;
const SPEED: f32 = 11.0;
const DWELL_S: f32 = 6.0;
/// Game-hour rates become animation seconds between trams, per direction.
const HEADWAY_SCALE: f32 = 240.0;
/// Trams drawn at once, per model.
const MAX_PER_KIND: usize = 300;
const STEP: f32 = 0.5;

const STEEL: Rgb = [58, 62, 70];
const ROOF: Rgb = [200, 204, 210];
const WINDOW: Rgb = [70, 96, 120];
const BED: Rgb = [112, 106, 98];
const GRAVEL: Rgb = [150, 138, 118];
const RAIL: Rgb = [176, 180, 186];
const WIRE: Rgb = [40, 42, 48];
const COLOURS: [Rgb; 4] = [[48, 84, 150], [60, 130, 96], [196, 58, 52], [226, 176, 52]];

/// Total length of a tram of `sections` sections.
pub fn tram_len(sections: usize) -> f32 {
    sections as f32 * SECTION_LEN + (sections as f32 - 1.0) * JOINT_LEN
}

/// A tram of two or three sections, 2.5 m wide, nose towards +x, body in the marker colour.
pub fn tram(sections: usize) -> Obj {
    let mut o = Obj::new();
    let len = tram_len(sections);
    let h = len / 2.0;
    for k in 0..sections {
        let x0 = -h + k as f32 * (SECTION_LEN + JOINT_LEN);
        let x1 = x0 + SECTION_LEN;
        // skirt, body, roof
        o.bx(x0, -1.25, 0.45, x1, 1.25, 1.0, STEEL);
        o.bx_top(x0, -1.25, 1.0, x1, 1.25, 3.3, MARKER, ROOF);
        // window band and doors
        o.bx(x0 + 0.9, -1.29, 1.6, x1 - 0.9, -1.25, 2.8, WINDOW);
        o.bx(x0 + 0.9, 1.25, 1.6, x1 - 0.9, 1.29, 2.8, WINDOW);
        let mid = (x0 + x1) / 2.0;
        o.bx(mid - 0.6, -1.3, 1.1, mid + 0.6, -1.25, 3.0, [34, 38, 46]);
        o.bx(mid - 0.6, 1.25, 1.1, mid + 0.6, 1.3, 3.0, [34, 38, 46]);
        // bogies at the ends of the train and under every joint
        for x in [x0 + 1.6, x1 - 1.6] {
            o.bx(x - 1.0, -1.1, 0.12, x + 1.0, 1.1, 0.55, [36, 38, 44]);
        }
        if k + 1 < sections {
            // the bellows between sections
            o.bx(x1, -1.1, 0.7, x1 + JOINT_LEN, 1.1, 3.0, [44, 46, 52]);
        }
    }
    // windscreen and lamps on both ends (a tram runs both ways)
    for sx in [-1.0f32, 1.0] {
        let x = sx * (h + 0.02);
        o.poly(
            &[[x, -0.9, 1.5], [x, 0.9, 1.5], [x, 0.9, 2.8], [x, -0.9, 2.8]],
            WINDOW,
            false,
            true,
            true,
        );
        for y in [-0.95f32, 0.95] {
            o.poly(
                &[
                    [x, y - 0.18, 0.75],
                    [x, y + 0.18, 0.75],
                    [x, y + 0.18, 1.1],
                    [x, y - 0.18, 1.1],
                ],
                HEADLIGHT,
                false,
                true,
                true,
            );
        }
    }
    // pantograph on the first joint (or the middle of a short tram)
    let px = -h + SECTION_LEN * 0.5 + 0.5;
    o.bx(px - 0.5, -0.1, 3.3, px + 0.5, 0.1, 4.0, [90, 94, 100]);
    o.bx(px - 0.35, -0.9, 4.0, px + 0.35, 0.9, 4.15, [90, 94, 100]);
    o
}

pub fn mesh(sections: usize) -> Mesh {
    let o = tram(sections);
    let mut m = Mesh::default();
    let mut id = 0u32;
    o.emit(&mut m, &mut id, 0.0, 0.0, 0.0, 1.0);
    m
}

/// A pole at the middle of the street with an arm over both tracks. Arm along local y.
fn pole() -> Obj {
    let mut o = Obj::new();
    o.bx(-0.13, -0.13, 0.0, 0.13, 0.13, 5.7, [76, 78, 86]);
    o.bx(-0.09, -2.5, 5.4, 0.09, 2.5, 5.6, [76, 78, 86]);
    for y in [-TRACK_OFFSET, TRACK_OFFSET] {
        o.bx(-0.05, y - 0.05, WIRE_Z, 0.05, y + 0.05, 5.4, [76, 78, 86]);
    }
    o
}

/// The first `frac` of a polyline by length.
fn prefix(points: &[Pt], frac: f64) -> Vec<Pt> {
    let total: f64 = points.windows(2).map(|w| w[0].dist(w[1])).sum();
    let want = total * frac.clamp(0.0, 1.0);
    let mut out = vec![points[0]];
    let mut acc = 0.0;
    for w in points.windows(2) {
        let l = w[0].dist(w[1]);
        if acc + l >= want {
            if l > 1e-9 {
                out.push(w[0].lerp(w[1], (want - acc) / l));
            }
            return out;
        }
        acc += l;
        out.push(w[1]);
    }
    out
}

/// How far a line is built, as the renderer draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildStage {
    /// Bed and rails, up to `progress`.
    Track,
    /// All track, wire poles up to `progress`.
    Wires,
    Open,
}

/// One line to draw: its path and how far it is built.
pub struct TrackDraw<'a> {
    pub points: &'a [Pt],
    pub stage: BuildStage,
    /// 0 to 1 within the stage.
    pub progress: f32,
}

/// The tracks and wire poles of every line, to be drawn as one chunk in the streets.
pub fn track_mesh(land: &Land, lines: &[TrackDraw]) -> Mesh {
    let mut g = Ground::on(land);
    let p = pole();
    for line in lines {
        if line.points.len() < 2 {
            continue;
        }
        let (track, wires) = match line.stage {
            BuildStage::Track => (line.progress as f64, 0.0),
            BuildStage::Wires => (1.0, line.progress as f64),
            BuildStage::Open => (1.0, 1.0),
        };
        let laid = prefix(line.points, track);
        if laid.len() >= 2 {
            let bed = if line.stage == BuildStage::Track {
                GRAVEL
            } else {
                BED
            };
            for off in [-TRACK_OFFSET, TRACK_OFFSET] {
                g.ribbon(&laid, 1.3, off, layer::RAIL + 0.02, bed);
                for r in [-RAIL_GAUGE_HALF, RAIL_GAUGE_HALF] {
                    g.ribbon(&laid, 0.07, off + r, layer::RAIL + 0.08, RAIL);
                }
            }
        }
        if wires > 0.0 {
            let strung = prefix(line.points, wires);
            if strung.len() >= 2 {
                for off in [-TRACK_OFFSET, TRACK_OFFSET] {
                    g.ribbon(&strung, 0.05, off, WIRE_Z, WIRE);
                }
                let total: f64 = strung.windows(2).map(|w| w[0].dist(w[1])).sum();
                let mut s = 6.0;
                while s < total {
                    if let Some((at, dir)) = along(&strung, s) {
                        let theta = libm::atan2(dir.1, dir.0) as f32;
                        g.place_prop(&p, at.x as f32, at.y as f32, theta);
                    }
                    s += POLE_GAP as f64;
                }
            }
        }
    }
    g.mesh
}

fn along(line: &[Pt], s: f64) -> Option<(Pt, (f64, f64))> {
    let mut acc = 0.0;
    for w in line.windows(2) {
        let l = w[0].dist(w[1]);
        if l > 1e-9 && acc + l >= s {
            let t = (s - acc) / l;
            return Some((
                w[0].lerp(w[1], t),
                ((w[1].x - w[0].x) / l, (w[1].y - w[0].y) / l),
            ));
        }
        acc += l;
    }
    None
}

/// One line as the simulation reports it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TramLineService {
    pub points: Vec<[f32; 2]>,
    /// Distance along `points` of each stop.
    pub stop_at: Vec<f32>,
    /// Trams per game hour in each direction.
    pub per_hour: f32,
    /// Only open lines have trams.
    pub open: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TramService {
    pub lines: Vec<TramLineService>,
}

/// Distance covered over time, with a halt at every stop.
struct Profile {
    s: Vec<f32>,
}

impl Profile {
    /// `stops`: where the front halts, ascending.
    fn build(total: f32, stops: &[f32]) -> Profile {
        let mut s = vec![0.0f32];
        let (mut pos, mut v) = (0.0f32, SPEED);
        let mut next = 0usize;
        let mut dwell = 0.0f32;
        let mut guard = 0;
        while pos < total && guard < 40_000 {
            guard += 1;
            if dwell > 0.0 {
                dwell -= STEP;
                s.push(pos);
                continue;
            }
            let mut target = SPEED;
            if let Some(&sp) = stops.get(next) {
                let d = sp - pos;
                if d <= 0.4 {
                    pos = pos.max(sp.min(pos + 0.4));
                    dwell = DWELL_S;
                    v = 0.0;
                    next += 1;
                    s.push(pos);
                    continue;
                }
                target = target.min(1.2 + libm::sqrtf(2.0 * 1.6 * d));
            }
            let dv = (target - v).clamp(-3.0 * STEP, 1.5 * STEP);
            v = (v + dv).max(1.0);
            pos += v * STEP;
            if let Some(&sp) = stops.get(next)
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
        self.s[i] + (self.s[i + 1] - self.s[i]) * (f - i as f32)
    }
}

struct Run {
    profile: Profile,
    headway: f32,
    dir: f32,
}

struct Planned {
    path: Path,
    runs: Vec<Run>,
    colour: Rgb,
    seed: u64,
}

/// Everything needed to place trams for a time value.
#[derive(Default)]
pub struct Trams {
    lines: Vec<Planned>,
    pub service: TramService,
}

impl Trams {
    pub fn plan(service: TramService, land: &Land) -> Trams {
        let mut lines = Vec::new();
        for (li, l) in service.lines.iter().enumerate() {
            if !l.open || l.per_hour <= 0.01 || l.points.len() < 2 {
                continue;
            }
            let pts: Vec<Pt> = l
                .points
                .iter()
                .map(|p| pt(p[0] as f64, p[1] as f64))
                .collect();
            let path = Path::new_on(RoadClass::Arterial, &pts, land);
            let total = path.length();
            if total < 20.0 {
                continue;
            }
            let long = tram_len(3);
            let mut runs = Vec::new();
            for dir in [1.0f32, -1.0] {
                // the front halts so the middle of the tram is at the stop
                let mut stops: Vec<f32> = l
                    .stop_at
                    .iter()
                    .map(|&s| if dir > 0.0 { s } else { total - s } + long * 0.5)
                    .collect();
                stops.sort_by(|a, b| a.total_cmp(b));
                runs.push(Run {
                    profile: Profile::build(total + long, &stops),
                    headway: HEADWAY_SCALE / l.per_hour,
                    dir,
                });
            }
            lines.push(Planned {
                path,
                runs,
                colour: COLOURS[li % COLOURS.len()],
                seed: li as u64,
            });
        }
        Trams { lines, service }
    }

    /// Add every tram at `time` seconds to the train instance sets.
    pub fn add_instances(&self, out: &mut TrainInstances, time: f32) {
        let mut counts = [0u32; 6];
        for line in &self.lines {
            let l = line.path.length();
            for (ri, run) in line.runs.iter().enumerate() {
                let dur = run.profile.duration();
                let first = ((time - dur) / run.headway).floor() as i64;
                let last = (time / run.headway).floor() as i64;
                for k in first..=last {
                    let dep = k as f32 * run.headway + line.seed as f32 * 23.0 + ri as f32 * 11.0;
                    let tau = time - dep;
                    if !(0.0..dur).contains(&tau) {
                        continue;
                    }
                    let mut rng = Rng::new(0x7a3d_0000 + ri as u64, (k as u64) ^ line.seed << 40);
                    let three = rng.f32() < 0.5;
                    let (veh, len) = if three {
                        (Vehicle::Tram3, tram_len(3))
                    } else {
                        (Vehicle::Tram2, tram_len(2))
                    };
                    let mid = run.profile.at(tau) - len / 2.0;
                    let n = &mut counts[veh as usize];
                    if mid < 0.0 || mid > l || *n as usize >= MAX_PER_KIND {
                        continue;
                    }
                    let s = if run.dir > 0.0 { mid } else { l - mid };
                    let (p, d) = line.path.at(s);
                    out.sets[veh as usize].push(Instance {
                        pos: [
                            p[0] + d[1] * run.dir * TRACK_OFFSET,
                            p[1] - d[0] * run.dir * TRACK_OFFSET,
                            line.path.z(s) + 0.5,
                        ],
                        theta: libm::atan2f(d[1] * run.dir, d[0] * run.dir),
                        scale: 1.0,
                        tint: tint(line.colour),
                        id: TRAIN_ID_BASE + veh as u32 * KIND_ID_SPAN + *n * TRAIN_ID_STRIDE,
                    });
                    *n += 1;
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}
