//! The look of the ground of the rock styles (red rock desert, coastal cliffs): flat ground,
//! washes and beaches, scree at the foot of a slope, and sheer faces in stepped bands of rock.
//! Colours are chosen per ground vertex from the height and steepness of the land there.

use crate::land::Land;
use crate::obj::Rgb;
use us_gen::Preset;

/// Colours of one style's ground.
#[derive(Clone, Copy, Debug)]
pub struct RockLook {
    /// Open flat ground.
    pub flat: Rgb,
    /// Sandy floor of a hollow (a dry wash).
    pub hollow: Rgb,
    /// Scree and talus.
    pub talus: Rgb,
    /// The layers of a rock face, one colour per band.
    pub bands: [Rgb; 5],
    /// Flat ground standing high above its surroundings (a mesa top).
    pub cap: Option<(Rgb, f32)>,
    /// Beach colour and the height below which it lies.
    pub beach: Option<(Rgb, f32)>,
    /// Bare wet rock of the shelf at the foot of a cliff, and the height below which it lies.
    pub shelf: Option<(Rgb, f32)>,
}

const MESA: RockLook = RockLook {
    flat: [230, 178, 124],
    hollow: [242, 210, 160],
    talus: [184, 116, 80],
    bands: [
        [170, 78, 52],
        [214, 124, 62],
        [190, 96, 62],
        [228, 164, 100],
        [148, 62, 46],
    ],
    cap: Some(([198, 118, 74], 90.0)),
    beach: None,
    shelf: None,
};

const COAST: RockLook = RockLook {
    flat: [112, 164, 96],
    hollow: [150, 176, 110],
    talus: [150, 130, 104],
    bands: [
        [152, 124, 98],
        [180, 150, 112],
        [122, 102, 86],
        [194, 166, 126],
        [142, 116, 94],
    ],
    cap: None,
    beach: Some(([226, 212, 166], 4.2)),
    shelf: Some(([104, 98, 96], 2.2)),
};

/// The ground look of a style, if it has one (the five 1.0 styles keep their plain grass).
pub fn look_for(p: Preset) -> Option<&'static RockLook> {
    match p {
        Preset::Mesa => Some(&MESA),
        Preset::Coast => Some(&COAST),
        _ => None,
    }
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let f = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    [f(0), f(1), f(2)]
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl RockLook {
    /// Colour of the ground at height `h` metres where the land rises `slope` per metre and sits
    /// `hollow` metres below the average of its surroundings.
    pub fn colour(&self, h: f32, slope: f32, hollow: f32, beach: f32) -> Rgb {
        let mut c = self.flat;
        if let Some((cap, from)) = self.cap {
            c = mix(c, cap, smooth(from * 0.6, from, h));
        }
        if let Some((rock, below)) = self.shelf {
            c = mix(
                c,
                rock,
                (1.0 - smooth(below * 0.3, below, h)) * (1.0 - beach),
            );
        }
        if let Some((sand, below)) = self.beach {
            c = mix(c, sand, beach * (1.0 - smooth(below * 0.6, below + 1.2, h)));
        }
        c = mix(c, self.hollow, smooth(0.8, 2.4, hollow));
        c = mix(c, self.talus, smooth(0.13, 0.3, slope));
        // a sheer face is one mid tone of its rock: the layers are drawn per pixel by the shader,
        // since a colour blended between vertices would smear down a tall wall
        let w = smooth(0.34, 0.6, slope);
        if w > 0.0 {
            let mut sum = [0u32; 3];
            for b in &self.bands {
                for i in 0..3 {
                    sum[i] += b[i] as u32;
                }
            }
            let n = self.bands.len() as u32;
            c = mix(
                c,
                [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8],
                w,
            );
        }
        c
    }
}

/// Ground colour at a vertex of the land grid. `height` gives the ground height anywhere (past the
/// edge of the map it may follow the terrain rather than hold the edge value).
pub fn vertex_colour(
    look: &RockLook,
    land: &Land,
    height: &dyn Fn(f32, f32) -> f32,
    beach: &dyn Fn(f32, f32) -> f32,
    x: f32,
    y: f32,
) -> Rgb {
    let e = land.step().max(1.0);
    let h = height(x, y);
    let (hx0, hx1) = (height(x - e, y), height(x + e, y));
    let (hy0, hy1) = (height(x, y - e), height(x, y + e));
    let slope = (((hx1 - hx0) / (2.0 * e)).powi(2) + ((hy1 - hy0) / (2.0 * e)).powi(2)).sqrt();
    let far = 3.0 * e;
    // a hollow has higher ground on every side; the foot of a rock has lower ground on one
    let ring = height(x - far, y)
        .min(height(x + far, y))
        .min(height(x, y - far))
        .min(height(x, y + far));
    let c = look.colour(h, slope, ring - h, beach(x, y));
    let size = land.size();
    if x < 0.0 || y < 0.0 || x > size || y > size {
        return apron_tint(look, c, x, y);
    }
    c
}

fn hash(ix: i32, iy: i32) -> f32 {
    let mut h = (ix as u32).wrapping_mul(0x9E37_79B1) ^ (iy as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth value noise, one cell per `cell` metres.
fn noise(x: f32, y: f32, cell: f32) -> f32 {
    let (fx, fy) = (x / cell, y / cell);
    let (ix, iy) = (fx.floor(), fy.floor());
    let (u, v) = (smooth(0.0, 1.0, fx - ix), smooth(0.0, 1.0, fy - iy));
    let (ix, iy) = (ix as i32, iy as i32);
    let a = hash(ix, iy) + (hash(ix + 1, iy) - hash(ix, iy)) * u;
    let b = hash(ix, iy + 1) + (hash(ix + 1, iy + 1) - hash(ix, iy + 1)) * u;
    a + (b - a) * v
}

/// Ground past the map edge in a desert style: patches of dry scrub and pale sand, so it is not
/// a single flat plane.
fn apron_tint(look: &RockLook, c: Rgb, x: f32, y: f32) -> Rgb {
    if look.cap.is_none() {
        return c;
    }
    let n = noise(x, y, 260.0) * 0.6 + noise(x + 77.0, y - 31.0, 110.0) * 0.4;
    let scrub = [196, 146, 100];
    let sand = [242, 204, 154];
    let c = mix(c, scrub, smooth(0.5, 0.78, n) * 0.7);
    mix(c, sand, smooth(0.5, 0.25, n) * 0.55)
}
