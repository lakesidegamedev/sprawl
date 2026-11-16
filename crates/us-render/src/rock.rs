//! How steep ground looks: rock faces in stepped bands of strata, loose talus at the foot, bare
//! ground above the tree line and snow above the snow line.
//!
//! Each town style that has real mountains or cliffs gets a [`RockLook`]. The ground shader reads
//! it from the frame's globals and paints natural ground by slope and height, so the bands stay
//! crisp at any zoom. Styles without a look keep plain grass.

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
    };

    /// The look of a town style, if it has one. Add a line here for a style with its own rock.
    pub fn of(p: Preset) -> Option<RockLook> {
        match p {
            Preset::Gorge => Some(RockLook::GORGE),
            Preset::Valley => Some(RockLook::VALLEY),
            _ => None,
        }
    }

    /// The eight rows the shader reads: four bands, talus, then (snow line, tree line, band
    /// height, 1), then (snow rgb, winter drop). Row 7 is spare.
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
            [0.0; 4],
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
}

/// Steepness (rise over run) from which ground is a rock face.
pub const ROCK_SLOPE: f32 = 0.7;
/// Steepness from which ground is loose stone.
pub const TALUS_SLOPE: f32 = 0.45;

impl RockLook {
    /// Classify ground at `height` metres with `slope` (rise over run) in summer.
    pub fn classify(&self, height: f32, slope: f32) -> Surface {
        if height > self.snow_line && slope < ROCK_SLOPE + 0.35 {
            Surface::Snow
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steep_ground_is_rock_and_high_ground_has_snow() {
        let v = RockLook::VALLEY;
        assert_eq!(v.classify(50.0, 0.05), Surface::Grass);
        assert_eq!(v.classify(50.0, 1.5), Surface::Rock);
        assert_eq!(v.classify(50.0, 0.5), Surface::Talus);
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
