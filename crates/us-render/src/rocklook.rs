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
    /// Metres of height one band covers.
    pub band_h: f32,
    /// Flat ground standing high above its surroundings (a mesa top).
    pub cap: Option<(Rgb, f32)>,
    /// Beach colour and the height below which it lies.
    pub beach: Option<(Rgb, f32)>,
}

const MESA: RockLook = RockLook {
    flat: [208, 160, 116],
    hollow: [230, 200, 150],
    talus: [184, 116, 80],
    bands: [
        [170, 78, 52],
        [214, 124, 62],
        [190, 96, 62],
        [228, 164, 100],
        [148, 62, 46],
    ],
    band_h: 42.0,
    cap: Some(([198, 118, 74], 90.0)),
    beach: None,
};

const COAST: RockLook = RockLook {
    flat: [112, 164, 96],
    hollow: [150, 176, 110],
    talus: [136, 130, 112],
    bands: [
        [120, 120, 126],
        [150, 142, 128],
        [98, 102, 112],
        [170, 152, 122],
        [132, 126, 120],
    ],
    band_h: 30.0,
    cap: None,
    beach: Some(([226, 212, 166], 4.2)),
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
    pub fn colour(&self, h: f32, slope: f32, hollow: f32) -> Rgb {
        let mut c = self.flat;
        if let Some((cap, from)) = self.cap {
            c = mix(c, cap, smooth(from * 0.6, from, h));
        }
        if let Some((sand, below)) = self.beach {
            c = mix(sand, c, smooth(below * 0.6, below + 1.2, h));
        }
        c = mix(c, self.hollow, smooth(0.8, 2.4, hollow));
        c = mix(c, self.talus, smooth(0.13, 0.3, slope));
        let w = smooth(0.34, 0.6, slope);
        if w > 0.0 {
            // bands step with height; the order is shuffled so neighbours differ
            let k = (h / self.band_h).floor().max(0.0) as usize;
            let band = self.bands[[0, 2, 1, 4, 3][k % 5]];
            c = mix(c, band, w);
        }
        c
    }
}

/// Ground colour at a vertex of the land grid.
pub fn vertex_colour(look: &RockLook, land: &Land, x: f32, y: f32) -> Rgb {
    let e = land.step().max(1.0);
    let h = land.height(x, y);
    let (hx0, hx1) = (land.height(x - e, y), land.height(x + e, y));
    let (hy0, hy1) = (land.height(x, y - e), land.height(x, y + e));
    let slope = (((hx1 - hx0) / (2.0 * e)).powi(2) + ((hy1 - hy0) / (2.0 * e)).powi(2)).sqrt();
    let far = 3.0 * e;
    let ring = (land.height(x - far, y)
        + land.height(x + far, y)
        + land.height(x, y - far)
        + land.height(x, y + far))
        * 0.25;
    look.colour(h, slope, ring - h)
}
