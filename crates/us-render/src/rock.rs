//! How steep ground looks: rock faces in stepped bands of strata, loose talus at the foot, bare
//! ground above the tree line and snow above the snow line.
//!
//! Each town style that has real mountains or cliffs gets a [`RockLook`]. The ground shader reads
//! it from the frame's globals and paints natural ground by slope and height, so the bands stay
//! crisp at any zoom. Styles without a look keep plain grass.

use crate::land::Land;
use crate::obj::Rgb;
use us_gen::Preset;

/// The colour of the terrain mesh where the shader may swap in rock. It is grass with a blue
/// of 95, which nothing else uses. Level maps and styles without a look draw plain grass.
pub const TERRAIN_MARK: Rgb = [116, 172, 95];

/// Palette and heights of the rock of one town style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RockLook {
    /// Strata colours. Each band of `band_height` metres takes one of the four.
    pub bands: [Rgb; 4],
    /// Loose stone at the foot of a face and on steep slopes that are not quite cliffs.
    pub talus: Rgb,
    /// Snow colour above the snow line.
    pub snow: Rgb,
    /// Height of one stratum, metres.
    pub band_height: f32,
    /// Height above which snow lies all year, metres (a very large number for none).
    pub snow_line: f32,
    /// How far the snow line drops at full winter cover, metres.
    pub winter_drop: f32,
    /// Height above which gentle slopes are bare rather than grass, metres.
    pub tree_line: f32,
    /// Granite is drawn as irregular blocks and cracks rather than level strata.
    pub fractured: bool,
    /// Steepest slope (rise over run) that still carries forest below the tree line; 0 for no
    /// forest.
    pub forest_slope: f32,
    /// Thin dark ledge line at the top of each stratum, 0 for none (per pixel in the shader).
    pub ledge: f32,
    /// Ground colours chosen per vertex on the CPU (desert flats, washes, beaches, wet shelf).
    /// Looks without it paint all their ground in the shader, by slope and height.
    pub ground: Option<GroundTint>,
}

/// Colours of the open ground of a rock style that the shader leaves alone: flat ground, washes
/// and beaches. Only steep ground is repainted by the shader (rock and scree).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundTint {
    /// Open flat ground.
    pub flat: Rgb,
    /// Sandy floor of a hollow (a dry wash).
    pub hollow: Rgb,
    /// Flat ground standing high above its surroundings (a mesa top) and the height it starts at.
    pub cap: Option<(Rgb, f32)>,
    /// Beach colour and the height below which it lies.
    pub beach: Option<(Rgb, f32)>,
    /// Bare wet rock of the shelf at the foot of a cliff, and the height below which it lies.
    pub shelf: Option<(Rgb, f32)>,
    /// Ground past the map edge is patched with scrub and pale sand.
    pub apron: bool,
}

const NONE: f32 = 1.0e6;

impl RockLook {
    /// Grey granite and sandstone cliffs, no snow: the river gorge.
    pub const GORGE: RockLook = RockLook {
        bands: [
            [150, 142, 130],
            [176, 158, 126],
            [136, 130, 122],
            [190, 170, 134],
        ],
        talus: [150, 140, 124],
        snow: [236, 241, 247],
        band_height: 11.0,
        snow_line: NONE,
        winter_drop: 0.0,
        tree_line: NONE,
        fractured: false,
        forest_slope: 0.0,
        ledge: 0.0,
        ground: None,
    };

    /// Cold grey granite with a snow line: the mountain valley.
    pub const VALLEY: RockLook = RockLook {
        bands: [
            [132, 136, 144],
            [156, 156, 158],
            [126, 130, 140],
            [170, 168, 164],
        ],
        talus: [146, 146, 148],
        snow: [238, 243, 249],
        band_height: 16.0,
        snow_line: 520.0,
        winter_drop: 220.0,
        tree_line: 400.0,
        fractured: true,
        forest_slope: 1.3,
        ledge: 0.0,
        ground: None,
    };

    /// Red banded sandstone of the desert towns: flat sand, pale washes, darker mesa tops.
    pub const MESA: RockLook = RockLook {
        bands: [
            [170, 78, 52],
            [214, 124, 62],
            [190, 96, 62],
            [228, 164, 100],
        ],
        talus: [184, 116, 80],
        snow: [236, 241, 247],
        band_height: 22.0,
        snow_line: NONE,
        winter_drop: 0.0,
        tree_line: NONE,
        fractured: false,
        forest_slope: 0.0,
        ledge: 0.5,
        ground: Some(GroundTint {
            flat: [230, 178, 124],
            hollow: [242, 210, 160],
            cap: Some(([198, 118, 74], 90.0)),
            beach: None,
            shelf: None,
            apron: true,
        }),
    };

    /// Banded sea cliffs over a wet rock shelf, with a sandy beach inside the cove.
    pub const COAST: RockLook = RockLook {
        bands: [
            [146, 143, 130],
            [177, 167, 146],
            [126, 125, 117],
            [191, 184, 164],
        ],
        talus: [147, 139, 122],
        snow: [236, 241, 247],
        band_height: 22.0,
        snow_line: NONE,
        winter_drop: 0.0,
        tree_line: NONE,
        fractured: false,
        forest_slope: 0.0,
        ledge: 0.16,
        ground: Some(GroundTint {
            flat: [105, 134, 97],
            hollow: [130, 148, 104],
            cap: None,
            beach: Some(([226, 212, 166], 4.2)),
            shelf: Some(([104, 98, 96], 2.2)),
            apron: false,
        }),
    };
    /// The look of a town style, if it has one. Add a line here for a style with its own rock.
    pub fn of(p: Preset) -> Option<RockLook> {
        match p {
            Preset::Gorge => Some(RockLook::GORGE),
            Preset::Valley => Some(RockLook::VALLEY),
            Preset::Mesa => Some(RockLook::MESA),
            Preset::Coast => Some(RockLook::COAST),
            _ => None,
        }
    }

    /// The eight rows the shader reads: four bands, talus, then (snow line, tree line, band
    /// height, 1), then (snow rgb, winter drop). Row 7 is (fractured, forest slope, vertex ground, ledge).
    pub fn rows(&self) -> [[f32; 4]; 8] {
        let c = |c: Rgb| {
            [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                0.0,
            ]
        };
        [
            c(self.bands[0]),
            c(self.bands[1]),
            c(self.bands[2]),
            c(self.bands[3]),
            c(self.talus),
            [self.snow_line, self.tree_line, self.band_height, 1.0],
            [
                self.snow[0] as f32 / 255.0,
                self.snow[1] as f32 / 255.0,
                self.snow[2] as f32 / 255.0,
                self.winter_drop,
            ],
            [
                self.fractured as u8 as f32,
                self.forest_slope,
                self.ground.is_some() as u8 as f32,
                self.ledge,
            ],
        ]
    }

    /// Rows for a frame with no rock look.
    pub fn no_rows() -> [[f32; 4]; 8] {
        [[0.0; 4]; 8]
    }
}

/// What a point of ground looks like, for tests and the CPU side (the shader does the same).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Grass,
    Bare,
    Talus,
    Rock,
    Snow,
    Forest,
}

/// Steepness (rise over run) from which ground is a rock face.
pub const ROCK_SLOPE: f32 = 0.7;
/// Steepness from which ground is loose stone.
pub const TALUS_SLOPE: f32 = 0.45;

impl RockLook {
    /// Classify ground at `height` metres with `slope` (rise over run) in summer.
    pub fn classify(&self, height: f32, slope: f32) -> Surface {
        if height > self.snow_line && slope < ROCK_SLOPE + 0.4 {
            Surface::Snow
        } else if self.forest_slope > 0.0
            && height < self.tree_line
            && slope > 0.14
            && slope < self.forest_slope
        {
            Surface::Forest
        } else if slope > ROCK_SLOPE {
            Surface::Rock
        } else if slope > TALUS_SLOPE {
            Surface::Talus
        } else if height > self.tree_line {
            Surface::Bare
        } else {
            Surface::Grass
        }
    }
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let f = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    [f(0), f(1), f(2)]
}

pub(crate) fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl GroundTint {
    /// Colour of the ground at height `h` metres where the land rises `slope` per metre and sits
    /// `hollow` metres below the average of its surroundings.
    pub fn colour(
        &self,
        look: &RockLook,
        h: f32,
        slope: f32,
        hollow: f32,
        (cove, beach): (f32, f32),
    ) -> Rgb {
        let mut c = self.flat;
        if let Some((cap, from)) = self.cap {
            c = mix(c, cap, smooth(from * 0.6, from, h));
        }
        if let Some((rock, below)) = self.shelf {
            c = mix(
                c,
                rock,
                (1.0 - smooth(below * 0.3, below, h)) * (1.0 - cove),
            );
        }
        if let Some((sand, below)) = self.beach {
            c = mix(c, sand, beach * (1.0 - smooth(below * 0.6, below + 1.2, h)));
        }
        c = mix(c, self.hollow, smooth(0.8, 2.4, hollow));
        c = mix(c, look.talus, smooth(0.13, 0.3, slope));
        // a sheer face is one mid tone of its rock: the layers are drawn per pixel by the shader,
        // since a colour blended between vertices would smear down a tall wall
        let w = smooth(0.34, 0.6, slope);
        if w > 0.0 {
            let mut sum = [0u32; 3];
            for b in &look.bands {
                for i in 0..3 {
                    sum[i] += b[i] as u32;
                }
            }
            let n = look.bands.len() as u32;
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
    beach: &dyn Fn(f32, f32) -> (f32, f32),
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
    let Some(tint) = &look.ground else {
        return TERRAIN_MARK;
    };
    let c = tint.colour(look, h, slope, ring - h, beach(x, y));
    let size = land.size();
    if x < 0.0 || y < 0.0 || x > size || y > size {
        return apron_tint(tint, c, x, y);
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

/// Slow noise in 0 to 1 that wanders along a coast: wide features a few hundred metres across with
/// finer ones on top.
pub(crate) fn wander(x: f32, y: f32) -> f32 {
    let n = noise(x + 31.0, y - 57.0, 190.0) * 0.7 + noise(x - 12.0, y + 90.0, 60.0) * 0.3;
    smooth(0.25, 0.75, n)
}

/// Ground past the map edge in a desert style: patches of dry scrub and pale sand, so it is not
/// a single flat plane.
fn apron_tint(tint: &GroundTint, c: Rgb, x: f32, y: f32) -> Rgb {
    if !tint.apron {
        return c;
    }
    let n = noise(x, y, 260.0) * 0.6 + noise(x + 77.0, y - 31.0, 110.0) * 0.4;
    let scrub = [196, 146, 100];
    let sand = [242, 204, 154];
    let c = mix(c, scrub, smooth(0.5, 0.78, n) * 0.7);
    mix(c, sand, smooth(0.5, 0.25, n) * 0.55)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steep_ground_is_rock_and_high_ground_has_snow() {
        let v = RockLook::VALLEY;
        assert_eq!(v.classify(50.0, 0.05), Surface::Grass);
        assert_eq!(v.classify(50.0, 1.5), Surface::Rock);
        assert_eq!(v.classify(50.0, 0.5), Surface::Forest);
        assert_eq!(v.classify(450.0, 0.5), Surface::Talus);
        assert_eq!(RockLook::GORGE.classify(50.0, 0.5), Surface::Talus);
        assert_eq!(v.classify(450.0, 0.1), Surface::Bare);
        assert_eq!(v.classify(700.0, 0.3), Surface::Snow);
        assert_eq!(v.classify(700.0, 2.0), Surface::Rock);
        assert_eq!(RockLook::GORGE.classify(900.0, 0.1), Surface::Grass);
    }

    #[test]
    fn only_mountain_styles_have_a_look() {
        assert!(RockLook::of(Preset::Gorge).is_some());
        assert!(RockLook::of(Preset::Valley).is_some());
        assert!(RockLook::of(Preset::Grid).is_none());
        assert_eq!(RockLook::GORGE.rows()[5][3], 1.0);
        assert_eq!(RockLook::no_rows()[5][3], 0.0);
    }
}
