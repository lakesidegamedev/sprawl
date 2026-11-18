//! Building sites that read as a job in progress: a cleared and fenced plot (or the old building
//! coming down), a foundation slab, a frame of columns and floor plates inside scaffolding, the
//! finished walls going up floor by floor, then a tidy-up. Tall jobs get a tower crane, small ones
//! a mobile crane on a truck.
//!
//! One function, [`site`], draws the whole thing for a progress from 0 to 1. The app asks for
//! [`STEPS`] distinct pictures per job and rebuilds a lot's mesh only when the step changes.

use crate::levels::massing;
use crate::obj::{Obj, Poly, Rgb, V3};

/// Distinct pictures of one job. Twelve is enough that the job reads as continuous over its
/// 45 to 90 game days while a lot is rebuilt only about once every four to seven days.
pub const STEPS: u8 = 12;

/// The progress (0 to 1) a step is drawn at: the middle of the step.
pub fn step_progress(step: u8) -> f32 {
    (step.min(STEPS - 1) as f32 + 0.5) / STEPS as f32
}

/// The step a progress falls in.
pub fn step_of(progress: f32) -> u8 {
    ((progress.clamp(0.0, 1.0) * STEPS as f32) as u8).min(STEPS - 1)
}

/// What the job is.
#[derive(Clone, Copy)]
pub struct Spec<'a> {
    /// The building being replaced, shown coming down at the start. `None` for a bare lot.
    pub old: Option<&'a Obj>,
    /// A tower crane instead of a mobile one: levels 4 and 5 and offices.
    pub tall: bool,
}

const DEMO_END: f32 = 0.17;
const CLEAR_END: f32 = 0.12;
const FOUND_END: f32 = 0.26;
const FRAME_END: f32 = 0.60;
const CLAD_END: f32 = 0.92;

const DIRT: Rgb = [148, 118, 82];
const CONCRETE: Rgb = [168, 168, 164];
const CONCRETE_DARK: Rgb = [132, 132, 130];
const STEEL: Rgb = [112, 118, 128];
const TIMBER: Rgb = [176, 144, 92];
const YELLOW: Rgb = [232, 190, 60];
const FENCE: Rgb = [232, 150, 52];

/// The model with everything above height `cut` removed. Walls that cross the cut are trimmed to
/// it; roofs, windows and signs that cross it are left off until the wall under them is whole.
pub fn clip_height(obj: &Obj, cut: f32) -> Obj {
    let mut o = Obj {
        polys: Vec::with_capacity(obj.polys.len()),
        solids: obj.solids.clone(),
    };
    for p in &obj.polys {
        let (lo, hi) = p
            .pts
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v[2]), b.max(v[2])));
        if hi <= cut + 1e-3 {
            o.polys.push(p.clone());
        } else if lo >= cut - 1e-3 || p.roof || p.flat {
            continue;
        } else if let Some(q) = trim(p, cut) {
            o.polys.push(q);
        }
    }
    o
}

/// Sutherland-Hodgman against the plane z = cut, keeping the part below.
fn trim(p: &Poly, cut: f32) -> Option<Poly> {
    let n = p.pts.len();
    let mut out: Vec<V3> = Vec::with_capacity(n + 2);
    for i in 0..n {
        let (a, b) = (p.pts[i], p.pts[(i + 1) % n]);
        let (ain, bin) = (a[2] <= cut, b[2] <= cut);
        if ain {
            out.push(a);
        }
        if ain != bin {
            let t = (cut - a[2]) / (b[2] - a[2]);
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, cut]);
        }
    }
    (out.len() >= 3).then(|| Poly {
        pts: out,
        ..p.clone()
    })
}

fn shade(c: Rgb, k: f32) -> Rgb {
    let m = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    [m(c[0]), m(c[1]), m(c[2])]
}

/// A building site at `progress`.
pub fn site(new: &Obj, spec: Spec, progress: f32) -> Obj {
    let p = progress.clamp(0.0, 1.0);
    let m = massing(new);
    let (bx0, by0, bx1, by1) = (m[0], m[1], m[2], m[3]);
    let h = m[4].max(3.0);
    let floors = ((h / 3.0).round() as usize).clamp(1, 24);
    let fh = h / floors as f32;
    let demo_end = if spec.old.is_some() { DEMO_END } else { 0.0 };
    let clear_end = if spec.old.is_some() {
        DEMO_END
    } else {
        CLEAR_END
    };
    let mut o = Obj::new();

    // the fenced plot
    let (x0, y0, x1, y1) = (bx0 - 1.5, by0 - 1.5, bx1 + 1.5, by1 + 1.5);
    o.slab(x0, y0, 0.0, x1, y1, 0.05, DIRT);
    let post = 0.15;
    o.slab(x0, y0, 0.0, x1, y0 + post, 1.1, FENCE);
    o.slab(x0, y1 - post, 0.0, x1, y1, 1.1, FENCE);
    o.slab(x0, y0, 0.0, x0 + post, y1, 1.1, FENCE);
    o.slab(x1 - post, y0, 0.0, x1, y1, 1.1, FENCE);

    // demolition: the old building loses height from the top, rubble piles up
    if let Some(old) = spec.old
        && p < demo_end
    {
        let om = massing(old);
        let k = 1.0 - p / demo_end;
        let cut = om[4] * k;
        let mut gone = clip_height(old, cut);
        for q in &mut gone.polys {
            q.color = shade(q.color, 0.8);
            q.finish = q.finish.dark();
        }
        let base = o.solids.len() as u32;
        o.solids.extend(gone.solids.iter().cloned());
        for mut q in gone.polys {
            q.solid = q.solid.map(|s| s + base);
            o.polys.push(q);
        }
        let rub = 0.2 + 0.9 * (1.0 - k);
        let (cx, cy) = ((bx0 + bx1) / 2.0, (by0 + by1) / 2.0);
        o.bx(
            cx - 3.2,
            cy - 2.8,
            0.0,
            cx - 0.6,
            cy - 1.0,
            rub,
            [128, 120, 110],
        );
        o.bx(
            cx + 0.8,
            cy + 0.6,
            0.0,
            cx + 3.4,
            cy + 2.6,
            rub * 0.8,
            [150, 138, 120],
        );
        excavator(&mut o, x1 - 3.0, y1 - 2.2, k);
    }

    site_props(&mut o, x0, y0, x1, y1, p);

    // foundation
    if p >= clear_end {
        let q = ((p - clear_end) / (FOUND_END - clear_end)).clamp(0.0, 1.0);
        let t = 0.12 + 0.38 * q;
        o.slab(bx0, by0, 0.05, bx1, by1, 0.05 + t, CONCRETE);
        if q > 0.55 && p < FRAME_END {
            // rebar stubs
            let (w, d) = (bx1 - bx0, by1 - by0);
            for (fx, fy) in [
                (0.12, 0.12),
                (0.88, 0.12),
                (0.12, 0.88),
                (0.88, 0.88),
                (0.5, 0.5),
            ] {
                let (x, y) = (bx0 + w * fx, by0 + d * fy);
                o.slab(
                    x - 0.07,
                    y - 0.07,
                    0.05 + t,
                    x + 0.07,
                    y + 0.07,
                    1.4,
                    [96, 70, 56],
                );
            }
        }
        if q < 0.9 {
            mixer(&mut o, x0 + 2.2, y1 - 2.0);
        }
    }

    // frame: columns and floor plates, one floor at a time
    let framed = if p < FOUND_END {
        0
    } else if p >= FRAME_END {
        floors
    } else {
        let q = (p - FOUND_END) / (FRAME_END - FOUND_END);
        ((q * floors as f32).ceil() as usize).clamp(1, floors)
    };
    let clad = if p < FRAME_END {
        0
    } else if p >= CLAD_END {
        floors
    } else {
        let q = (p - FRAME_END) / (CLAD_END - FRAME_END);
        ((q * floors as f32).ceil() as usize).clamp(1, floors)
    };
    let cut = clad as f32 * fh;
    let frame_top = framed as f32 * fh;

    if clad >= floors {
        // finished walls and roof
        let base = o.solids.len() as u32;
        o.solids.extend(new.solids.iter().cloned());
        for q in &new.polys {
            let mut q = q.clone();
            q.solid = q.solid.map(|s| s + base);
            o.polys.push(q);
        }
    } else {
        if clad > 0 {
            let part = clip_height(new, cut + 0.01);
            let base = o.solids.len() as u32;
            o.solids.extend(part.solids.iter().cloned());
            for mut q in part.polys {
                q.solid = q.solid.map(|s| s + base);
                o.polys.push(q);
            }
        }
        if framed > 0 {
            frame(&mut o, [bx0, by0, bx1, by1], fh, framed, cut);
            scaffold(&mut o, [bx0, by0, bx1, by1], fh, frame_top.max(cut));
        }
    }

    // cranes while the job runs
    if p < CLAD_END && p >= clear_end * 0.5 {
        let reach = (bx1 - bx0).max(by1 - by0);
        if spec.tall {
            tower_crane(
                &mut o,
                x1 - 0.8,
                y0 + 0.8,
                frame_top.max(8.0) + 7.0,
                reach * 1.1 + 5.0,
                p,
            );
        } else {
            mobile_crane(&mut o, x0 + 2.4, y0 + 2.2, frame_top.max(3.0) + 1.5);
        }
    }
    o
}

/// Columns on a grid and a plate at every framed floor. Plates and columns below the finished
/// walls are inside them and left out.
fn frame(o: &mut Obj, b: [f32; 4], fh: f32, framed: usize, cut: f32) {
    let (x0, y0, x1, y1) = (b[0] + 0.3, b[1] + 0.3, b[2] - 0.3, b[3] - 0.3);
    let (w, d) = (x1 - x0, y1 - y0);
    let nx = ((w / 5.0).ceil() as usize + 1).clamp(2, 5);
    let ny = ((d / 5.0).ceil() as usize + 1).clamp(2, 5);
    let top = framed as f32 * fh;
    let from = cut.max(0.5);
    if top > from {
        for i in 0..nx {
            for j in 0..ny {
                let x = x0 + w * i as f32 / (nx - 1) as f32;
                let y = y0 + d * j as f32 / (ny - 1) as f32;
                o.bx(x - 0.2, y - 0.2, from, x + 0.2, y + 0.2, top, CONCRETE_DARK);
            }
        }
    }
    for k in 1..=framed {
        let z = k as f32 * fh;
        if z > cut + 0.01 {
            o.bx(x0 - 0.1, y0 - 0.1, z - 0.3, x1 + 0.1, y1 + 0.1, z, CONCRETE);
        }
    }
}

/// Scaffolding round the job: poles and a timber platform every two floors.
fn scaffold(o: &mut Obj, b: [f32; 4], fh: f32, top: f32) {
    let off = 0.9;
    let (x0, y0, x1, y1) = (b[0] - off, b[1] - off, b[2] + off, b[3] + off);
    let hs = top + 1.2;
    let pole = 0.07;
    let n = |len: f32| ((len / 3.0).ceil() as usize).max(1);
    for i in 0..=n(x1 - x0) {
        let x = x0 + (x1 - x0) * i as f32 / n(x1 - x0) as f32;
        for y in [y0, y1] {
            o.slab(x - pole, y - pole, 0.0, x + pole, y + pole, hs, STEEL);
        }
    }
    for j in 1..n(y1 - y0) {
        let y = y0 + (y1 - y0) * j as f32 / n(y1 - y0) as f32;
        for x in [x0, x1] {
            o.slab(x - pole, y - pole, 0.0, x + pole, y + pole, hs, STEEL);
        }
    }
    let lift = 2.0 * fh;
    let mut z = lift;
    while z <= hs {
        let t = 0.08;
        o.slab(x0, y0, z - t, x1, y0 + 0.7, z, TIMBER);
        o.slab(x0, y1 - 0.7, z - t, x1, y1, z, TIMBER);
        o.slab(x0, y0, z - t, x0 + 0.7, y1, z, TIMBER);
        o.slab(x1 - 0.7, y0, z - t, x1, y1, z, TIMBER);
        z += lift;
    }
}

/// Things that stand on a working site: a site cabin, a skip, stacks of material.
fn site_props(o: &mut Obj, x0: f32, y0: f32, x1: f32, y1: f32, p: f32) {
    if p < CLAD_END {
        o.bx(
            x0 + 0.5,
            y1 - 2.3,
            0.0,
            x0 + 3.3,
            y1 - 0.5,
            1.9,
            [214, 214, 204],
        );
        o.bx(
            x0 + 0.4,
            y1 - 2.4,
            1.9,
            x0 + 3.4,
            y1 - 0.4,
            2.1,
            [70, 110, 160],
        );
        o.bx(
            x1 - 2.6,
            y1 - 2.0,
            0.0,
            x1 - 0.6,
            y1 - 0.6,
            1.0,
            [60, 120, 70],
        );
    }
    if p < FRAME_END {
        o.bx(
            x0 + 0.6,
            y0 + 0.5,
            0.0,
            x0 + 2.8,
            y0 + 1.5,
            0.7,
            [190, 150, 100],
        );
        o.bx(
            x0 + 0.7,
            y0 + 1.6,
            0.0,
            x0 + 2.6,
            y0 + 2.4,
            0.5,
            [150, 150, 154],
        );
    } else if p < CLAD_END {
        o.bx(
            x0 + 0.6,
            y0 + 0.5,
            0.0,
            x0 + 3.0,
            y0 + 1.7,
            1.0,
            [196, 120, 96],
        );
    }
}

fn mixer(o: &mut Obj, x: f32, y: f32) {
    o.bx(x - 2.2, y - 0.9, 0.5, x + 0.4, y + 0.9, 1.0, [90, 94, 100]);
    o.bx(
        x + 0.4,
        y - 0.9,
        0.5,
        x + 1.5,
        y + 0.9,
        2.0,
        [220, 220, 224],
    );
    o.bx(x - 1.9, y - 0.8, 1.0, x + 0.2, y + 0.8, 2.3, [236, 130, 50]);
}

fn excavator(o: &mut Obj, x: f32, y: f32, k: f32) {
    o.bx(x - 1.3, y - 1.0, 0.0, x + 1.3, y + 1.0, 0.6, [60, 60, 64]);
    o.bx(x - 1.0, y - 0.9, 0.6, x + 0.6, y + 0.9, 1.9, YELLOW);
    // arm: steps up and out, higher while the wall is still tall
    let reach = 1.5 + 2.5 * k;
    let mut z = 1.6;
    let mut xx = x - 1.0;
    for _ in 0..4 {
        o.bx(xx - 0.9, y - 0.2, z, xx, y + 0.2, z + 0.5 + k, YELLOW);
        xx -= 0.9;
        z += 0.5 + k * 0.7;
    }
    let _ = reach;
    o.bx(
        xx - 0.8,
        y - 0.6,
        z - 0.3,
        xx + 0.1,
        y + 0.6,
        z + 0.2,
        STEEL,
    );
}

fn mobile_crane(o: &mut Obj, x: f32, y: f32, boom_top: f32) {
    o.bx(x - 2.6, y - 1.0, 0.0, x + 2.6, y + 1.0, 0.9, [60, 62, 66]);
    o.bx(
        x - 2.6,
        y - 1.0,
        0.9,
        x - 1.2,
        y + 1.0,
        2.3,
        [236, 236, 236],
    );
    o.bx(x - 1.2, y - 1.0, 0.9, x + 2.6, y + 1.0, 1.6, YELLOW);
    // boom in steps leaning toward the plot
    let steps = 5;
    let rise = (boom_top - 1.6).max(2.0) / steps as f32;
    for i in 0..steps {
        let xx = x + 2.0 + 0.9 * i as f32;
        let z = 1.6 + rise * i as f32;
        o.bx(xx, y - 0.3, z, xx + 1.0, y + 0.3, z + rise + 0.2, YELLOW);
    }
    // hook cable
    let hx = x + 2.0 + 0.9 * steps as f32;
    let top = 1.6 + rise * steps as f32;
    o.slab(
        hx - 0.04,
        y - 0.04,
        top - 2.4,
        hx + 0.04,
        y + 0.04,
        top,
        STEEL,
    );
    o.bx(
        hx - 0.2,
        y - 0.2,
        top - 2.9,
        hx + 0.2,
        y + 0.2,
        top - 2.4,
        [90, 94, 100],
    );
}

fn tower_crane(o: &mut Obj, x: f32, y: f32, mast: f32, jib: f32, p: f32) {
    let w = 0.6;
    o.bx(
        x - w / 2.0 - 0.3,
        y - w / 2.0 - 0.3,
        0.0,
        x + w / 2.0 + 0.3,
        y + w / 2.0 + 0.3,
        0.6,
        [90, 94, 100],
    );
    o.bx(
        x - w / 2.0,
        y - w / 2.0,
        0.0,
        x + w / 2.0,
        y + w / 2.0,
        mast,
        YELLOW,
    );
    let mut z = 1.5;
    while z < mast {
        o.slab(
            x - w / 2.0 - 0.05,
            y - w / 2.0 - 0.05,
            z,
            x + w / 2.0 + 0.05,
            y + w / 2.0 + 0.05,
            z + 0.25,
            [240, 240, 236],
        );
        z += 2.0;
    }
    // jib toward the plot (negative x), counter jib behind
    o.bx(
        x - jib,
        y - 0.25,
        mast,
        x + 2.4,
        y + 0.25,
        mast + 0.5,
        YELLOW,
    );
    o.bx(
        x - 0.2,
        y - 0.25,
        mast + 0.5,
        x + 0.2,
        y + 0.25,
        mast + 2.2,
        YELLOW,
    );
    o.bx(
        x + 1.2,
        y - 0.5,
        mast - 1.2,
        x + 2.6,
        y + 0.5,
        mast,
        [90, 94, 100],
    );
    o.bx(
        x - 0.9,
        y - 0.5,
        mast - 1.4,
        x + 0.4,
        y + 0.5,
        mast,
        [236, 236, 236],
    );
    // trolley, cable and a load that goes up and down with the job
    let tx = x - jib * (0.45 + 0.2 * libm::sinf(p * 9.0));
    let drop = 1.5 + (mast - 3.0) * (0.35 + 0.3 * libm::cosf(p * 7.0)).clamp(0.1, 0.9);
    o.bx(
        tx - 0.3,
        y - 0.3,
        mast - 0.3,
        tx + 0.3,
        y + 0.3,
        mast,
        [90, 94, 100],
    );
    o.slab(
        tx - 0.04,
        y - 0.04,
        mast - drop,
        tx + 0.04,
        y + 0.04,
        mast - 0.3,
        STEEL,
    );
    o.bx(
        tx - 0.6,
        y - 0.4,
        mast - drop - 0.7,
        tx + 0.6,
        y + 0.4,
        mast - drop,
        [196, 120, 96],
    );
}
