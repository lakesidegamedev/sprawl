//! The fixed stage: map frame, river, and the curve helpers the skeleton uses.

use crate::geom::{Pt, cos, pt, sin};
use crate::rng::Rng;
use crate::terrain::Terrain;

pub const S: f64 = 4096.0;
pub const HWY_Y: f64 = 70.0;
pub const HALL: Pt = pt(1900.0, 1450.0);
pub const TAU: f64 = std::f64::consts::TAU;

#[derive(Clone, Debug)]
pub struct River {
    pub base: f64,
    pub a1: f64,
    pub p1: f64,
    pub a2: f64,
    pub p2: f64,
}

impl River {
    pub fn new(rng: &mut Rng) -> River {
        River {
            base: 2950.0 + rng.uniform(-120.0, 160.0),
            a1: 170.0 * rng.uniform(0.8, 1.2),
            p1: rng.uniform(0.0, TAU),
            a2: 50.0 * rng.uniform(0.8, 1.2),
            p2: rng.uniform(0.0, TAU),
        }
    }

    pub fn y(&self, x: f64) -> f64 {
        self.base + self.a1 * sin(x / 700.0 + self.p1) + self.a2 * sin(x / 260.0 + self.p2)
    }

    /// Closed polygon of the strip between offsets `off0` and `off1` from the centre line,
    /// running 50 m past the map edge on both sides.
    pub fn band(&self, off0: f64, off1: f64) -> Vec<Pt> {
        let n = 200;
        let mut xs: Vec<f64> = vec![-50.0];
        xs.extend((0..=n).map(|i| S * i as f64 / n as f64));
        xs.push(S + 50.0);
        let mut ring: Vec<Pt> = xs
            .iter()
            .map(|&x| pt(x, self.y(x.clamp(0.0, S)) + off0))
            .collect();
        ring.extend(
            xs.iter()
                .rev()
                .map(|&x| pt(x, self.y(x.clamp(0.0, S)) + off1)),
        );
        ring
    }

    pub fn line(&self, off: f64) -> Vec<Pt> {
        (0..=200)
            .map(|i| {
                let x = S * i as f64 / 200.0;
                pt(x, self.y(x) + off)
            })
            .collect()
    }
}

pub fn map_ring() -> Vec<Pt> {
    vec![pt(0.0, HWY_Y), pt(S, HWY_Y), pt(S, S), pt(0.0, S)]
}

pub fn dist_hall(p: Pt) -> f64 {
    p.dist(HALL)
}

pub fn noise(x: f64, y: f64) -> f64 {
    (sin(x / 310.0 + 1.7) * cos(y / 270.0 - 0.4) + 0.6 * sin((x + y) / 190.0)) / 1.6
}

/// An old road that wanders from `start` along `heading` until it leaves the map.
pub fn wander_line(
    start: Pt,
    heading: f64,
    rng: &mut Rng,
    amp: f64,
    terrain: Option<&Terrain>,
) -> Vec<Pt> {
    let step = 20.0;
    let ph = [
        rng.uniform(0.0, 6.3),
        rng.uniform(0.0, 6.3),
        rng.uniform(0.0, 6.3),
    ];
    let mut pts = vec![start];
    let (mut x, mut y) = (start.x, start.y);
    let mut l = 0.0;
    while l < 6000.0 {
        let mut h = heading
            + amp
                * (0.55 * sin(l / 520.0 + ph[0])
                    + 0.3 * sin(l / 230.0 + ph[1])
                    + 0.15 * sin(l / 110.0 + ph[2]));
        if let Some(t) = terrain {
            h = follow_contour(t, pt(x, y), h);
        }
        x += step * cos(h);
        y += step * sin(h);
        pts.push(pt(x, y));
        l += step;
        if !(-40.0 < x && x < S + 40.0 && HWY_Y - 40.0 < y && y < S + 40.0) {
            break;
        }
    }
    pts
}

/// A wobbly ring road around the hall.
pub fn ring_road(r0: f64, rng: &mut Rng) -> Vec<Pt> {
    let (a, b) = (rng.uniform(0.0, 6.3), rng.uniform(0.0, 6.3));
    let n = 240;
    (0..=n)
        .map(|i| {
            let t = TAU * i as f64 / n as f64;
            let r = r0 * (1.0 + 0.13 * sin(3.0 * t + a) + 0.07 * sin(5.0 * t + b));
            pt(HALL.x + r * cos(t), HALL.y + r * sin(t))
        })
        .collect()
}

/// Long straight line through `through` at angle `a`.
pub fn ln_angle(a: f64, through: Pt) -> Vec<Pt> {
    let (dx, dy) = (cos(a), sin(a));
    let l = 7000.0;
    vec![
        pt(through.x - dx * l, through.y - dy * l),
        pt(through.x + dx * l, through.y + dy * l),
    ]
}

/// Turn a heading towards the nearest contour direction where the ground is steep, so old roads
/// bend round hillsides instead of climbing them.
pub fn follow_contour(t: &Terrain, at: Pt, h: f64) -> f64 {
    let g = t.gradient(at);
    let s = g.len();
    if s < 0.025 {
        return h;
    }
    let pi = std::f64::consts::PI;
    let along = libm::atan2(g.x, -g.y);
    // the contour direction closest to the heading (it runs both ways)
    let mut d = (along - h).rem_euclid(pi);
    if d > pi / 2.0 {
        d -= pi;
    }
    // no pull when the heading is square to the contour, or the road would flip back and forth
    let blend = ((s - 0.025) / 0.07).clamp(0.0, 0.75) * cos(d).powi(2);
    h + d * blend
}

/// A blobby lake with a shore that streets can follow.
#[derive(Clone, Debug)]
pub struct Lake {
    pub c: Pt,
    pub r: f64,
    pub a: f64,
    pub pa: f64,
    pub b: f64,
    pub pb: f64,
}

impl Lake {
    pub fn new(rng: &mut Rng) -> Lake {
        Lake {
            c: pt(
                2800.0 + rng.uniform(-80.0, 120.0),
                1450.0 + rng.uniform(-200.0, 250.0),
            ),
            r: 430.0 * rng.uniform(0.9, 1.1),
            a: rng.uniform(0.1, 0.17),
            pa: rng.uniform(0.0, TAU),
            b: rng.uniform(0.05, 0.1),
            pb: rng.uniform(0.0, TAU),
        }
    }

    fn radius(&self, t: f64) -> f64 {
        self.r * (1.0 + self.a * sin(2.0 * t + self.pa) + self.b * sin(3.0 * t + self.pb))
    }

    /// Closed ring `off` metres out from the shoreline (negative goes inland), first point repeated.
    pub fn ring(&self, off: f64) -> Vec<Pt> {
        let n = 180;
        (0..=n)
            .map(|i| {
                let t = TAU * (i % n) as f64 / n as f64;
                let r = self.radius(t) + off;
                pt(self.c.x + r * cos(t), self.c.y + r * sin(t))
            })
            .collect()
    }
}
