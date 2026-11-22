//! Graphics settings: a quality preset over individual controls, saved between launches.

use us_render::Style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Quality {
    Low,
    Medium,
    High,
    Ultra,
    Custom,
}

impl Quality {
    pub const PICKABLE: [Quality; 4] =
        [Quality::Low, Quality::Medium, Quality::High, Quality::Ultra];

    pub fn name(self) -> &'static str {
        match self {
            Quality::Low => "Low",
            Quality::Medium => "Medium",
            Quality::High => "High",
            Quality::Ultra => "Ultra",
            Quality::Custom => "Custom",
        }
    }

    /// The default for this device: High on desktop, Medium on phones and tablets.
    pub fn platform_default() -> Quality {
        if cfg!(any(target_os = "ios", target_os = "android")) {
            Quality::Medium
        } else {
            Quality::High
        }
    }
}

/// How large the city is drawn before it is scaled to the window. Menus and text stay sharp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// Starts at 80 percent and adapts to slow frames.
    Auto,
    P50,
    P67,
    P80,
    P100,
}

impl Resolution {
    pub const ALL: [Resolution; 5] = [
        Resolution::Auto,
        Resolution::P50,
        Resolution::P67,
        Resolution::P80,
        Resolution::P100,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Resolution::Auto => "Auto",
            Resolution::P50 => "50%",
            Resolution::P67 => "67%",
            Resolution::P80 => "80%",
            Resolution::P100 => "100%",
        }
    }

    /// The fixed render scale, or `None` when it adapts.
    pub fn fixed_scale(self) -> Option<f32> {
        match self {
            Resolution::Auto => None,
            Resolution::P50 => Some(0.5),
            Resolution::P67 => Some(0.67),
            Resolution::P80 => Some(0.8),
            Resolution::P100 => Some(1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShadowQuality {
    Low,
    Medium,
    High,
}

impl ShadowQuality {
    pub const ALL: [ShadowQuality; 3] = [
        ShadowQuality::Low,
        ShadowQuality::Medium,
        ShadowQuality::High,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ShadowQuality::Low => "Low",
            ShadowQuality::Medium => "Medium",
            ShadowQuality::High => "High",
        }
    }

    pub fn map_size(self) -> u32 {
        match self {
            ShadowQuality::Low => 1024,
            ShadowQuality::Medium => 2048,
            ShadowQuality::High => 4096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Graphics {
    pub quality: Quality,
    pub resolution: Resolution,
    pub shadows: ShadowQuality,
    /// Darkening in creases and at the foot of walls.
    pub ambient_shading: bool,
    /// Finer atmosphere, smoother edges and detailed surface lighting.
    pub detailed_surfaces: bool,
    /// Draw far buildings in a simpler form (true) or at full detail at any distance (false).
    pub reduced_far_detail: bool,
    /// Thin (1) or bold (2) outlines.
    pub bold_outlines: bool,
    pub tilt_shift: bool,
    pub vsync: bool,
}

impl Default for Graphics {
    fn default() -> Self {
        Self::preset(Quality::platform_default())
    }
}

impl Graphics {
    /// Every control as the given preset sets it. `Custom` returns the High settings.
    pub fn preset(q: Quality) -> Graphics {
        let base = Graphics {
            quality: q,
            resolution: Resolution::Auto,
            shadows: ShadowQuality::High,
            ambient_shading: true,
            detailed_surfaces: false,
            reduced_far_detail: true,
            bold_outlines: true,
            tilt_shift: false,
            vsync: true,
        };
        match q {
            Quality::Low => Graphics {
                shadows: ShadowQuality::Low,
                ambient_shading: false,
                bold_outlines: false,
                ..base
            },
            Quality::Medium => Graphics {
                shadows: ShadowQuality::Medium,
                ambient_shading: false,
                ..base
            },
            Quality::High | Quality::Custom => base,
            Quality::Ultra => Graphics {
                resolution: Resolution::P100,
                detailed_surfaces: true,
                reduced_far_detail: false,
                ..base
            },
        }
    }

    pub fn apply_preset(&mut self, q: Quality) {
        let tilt = self.tilt_shift;
        let vsync = self.vsync;
        *self = Graphics::preset(q);
        // tilt shift and VSync are taste and display choices, not quality levels
        self.tilt_shift = tilt;
        self.vsync = vsync;
    }

    /// The preset whose controls all match, or `Custom`.
    pub fn detect(&self) -> Quality {
        Quality::PICKABLE
            .into_iter()
            .find(|&q| {
                let p = Graphics::preset(q);
                Graphics {
                    quality: self.quality,
                    tilt_shift: self.tilt_shift,
                    vsync: self.vsync,
                    ..p
                } == *self
            })
            .unwrap_or(Quality::Custom)
    }

    /// Call after any control changes so the preset label follows.
    pub fn touch(&mut self) {
        self.quality = self.detect();
    }

    /// Writes the settings into the renderer style. `scale` is the adaptive world scale
    /// and `under_pressure` is true while sustained slowdown has cut quality (Auto only).
    pub fn apply_to(&self, style: &mut Style, under_pressure: bool) {
        let pressure = under_pressure && self.resolution == Resolution::Auto;
        style.shadow_size = if pressure {
            self.shadows.map_size().min(2048)
        } else {
            self.shadows.map_size()
        };
        style.high_detail = self.detailed_surfaces;
        style.contact_shade = (self.ambient_shading && !pressure) || self.detailed_surfaces;
        style.lod = self.reduced_far_detail;
        style.ink_width = if self.bold_outlines { 2 } else { 1 };
        style.tilt_shift = self.tilt_shift;
        style.vsync = self.vsync;
    }

    fn key_values(&self) -> [(&'static str, String); 9] {
        [
            ("quality", self.quality.name().to_lowercase()),
            ("resolution", self.resolution.name().to_string()),
            ("shadows", self.shadows.name().to_lowercase()),
            ("ambient_shading", self.ambient_shading.to_string()),
            ("detailed_surfaces", self.detailed_surfaces.to_string()),
            ("reduced_far_detail", self.reduced_far_detail.to_string()),
            ("bold_outlines", self.bold_outlines.to_string()),
            ("tilt_shift", self.tilt_shift.to_string()),
            ("vsync", self.vsync.to_string()),
        ]
    }

    fn set(&mut self, key: &str, value: &str) {
        let flag = || match value {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        match key {
            "resolution" => {
                if let Some(r) = Resolution::ALL.into_iter().find(|r| r.name() == value) {
                    self.resolution = r;
                }
            }
            "shadows" => {
                if let Some(s) = ShadowQuality::ALL
                    .into_iter()
                    .find(|s| s.name().eq_ignore_ascii_case(value))
                {
                    self.shadows = s;
                }
            }
            "ambient_shading" => self.ambient_shading = flag().unwrap_or(self.ambient_shading),
            "detailed_surfaces" => {
                self.detailed_surfaces = flag().unwrap_or(self.detailed_surfaces)
            }
            "reduced_far_detail" => {
                self.reduced_far_detail = flag().unwrap_or(self.reduced_far_detail);
            }
            "bold_outlines" => self.bold_outlines = flag().unwrap_or(self.bold_outlines),
            "tilt_shift" => self.tilt_shift = flag().unwrap_or(self.tilt_shift),
            "vsync" => self.vsync = flag().unwrap_or(self.vsync),
            _ => {}
        }
    }
}

/// Everything the Settings screen keeps between launches besides audio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Display {
    pub fullscreen: bool,
    pub ui_scale: f32,
    pub graphics: Graphics,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            fullscreen: false,
            ui_scale: 1.0,
            graphics: Graphics::default(),
        }
    }
}

impl Display {
    pub fn load() -> Self {
        let file = crate::saves::save_dir().join("graphics-prefs.txt");
        std::fs::read_to_string(file).map_or_else(|_| Self::default(), |s| Self::parse(&s))
    }

    pub fn parse(text: &str) -> Self {
        let mut d = Self::default();
        for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
            let value = value.trim();
            match key.trim() {
                "fullscreen" => d.fullscreen = value == "true",
                "ui_scale" => {
                    if let Ok(v) = value.parse::<f32>()
                        && v.is_finite()
                    {
                        d.ui_scale = v.clamp(0.8, 1.8);
                    }
                }
                k => d.graphics.set(k, value),
            }
        }
        // the preset label is derived from the controls, so a hand-edited file stays honest
        d.graphics.touch();
        d
    }

    pub fn to_text(self) -> String {
        let mut out = format!(
            "fullscreen={}\nui_scale={}\n",
            self.fullscreen, self.ui_scale
        );
        for (k, v) in self.graphics.key_values() {
            out.push_str(&format!("{k}={v}\n"));
        }
        out
    }

    pub fn save(self) -> std::io::Result<()> {
        let dir = crate::saves::save_dir();
        std::fs::create_dir_all(&dir)?;
        let staging = dir.join("graphics-prefs.partial");
        std::fs::write(&staging, self.to_text())?;
        std::fs::rename(staging, dir.join("graphics-prefs.txt"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_set_every_control() {
        let low = Graphics::preset(Quality::Low);
        assert_eq!(low.shadows.map_size(), 1024);
        assert!(!low.ambient_shading && !low.bold_outlines);
        let ultra = Graphics::preset(Quality::Ultra);
        assert_eq!(ultra.resolution.fixed_scale(), Some(1.0));
        assert_eq!(ultra.shadows.map_size(), 4096);
        assert!(ultra.detailed_surfaces && !ultra.reduced_far_detail);
        let high = Graphics::preset(Quality::High);
        assert_eq!(high.resolution, Resolution::Auto);
        assert!(high.reduced_far_detail && high.ambient_shading && !high.detailed_surfaces);
    }

    #[test]
    fn changing_a_control_makes_the_preset_custom() {
        let mut g = Graphics::preset(Quality::High);
        assert_eq!(g.detect(), Quality::High);
        g.shadows = ShadowQuality::Low;
        g.touch();
        assert_eq!(g.quality, Quality::Custom);
        g.shadows = ShadowQuality::High;
        g.touch();
        assert_eq!(g.quality, Quality::High);
    }

    #[test]
    fn tilt_shift_and_vsync_do_not_change_the_preset() {
        let mut g = Graphics::preset(Quality::Medium);
        g.tilt_shift = true;
        g.vsync = false;
        g.touch();
        assert_eq!(g.quality, Quality::Medium);
        g.apply_preset(Quality::Ultra);
        assert!(g.tilt_shift && !g.vsync);
    }

    #[test]
    fn style_follows_settings_and_pressure() {
        let mut style = Style::default();
        let g = Graphics::preset(Quality::High);
        g.apply_to(&mut style, false);
        assert_eq!(style.shadow_size, 4096);
        assert!(style.contact_shade && style.lod);
        g.apply_to(&mut style, true);
        assert_eq!(style.shadow_size, 2048);
        assert!(!style.contact_shade);
        // a fixed resolution never drops shadows behind the player's back
        let fixed = Graphics {
            resolution: Resolution::P80,
            ..g
        };
        fixed.apply_to(&mut style, true);
        assert_eq!(style.shadow_size, 4096);
    }

    #[test]
    fn round_trip() {
        let mut d = Display {
            fullscreen: true,
            ui_scale: 1.3,
            ..Display::default()
        };
        d.graphics.apply_preset(Quality::Ultra);
        d.graphics.resolution = Resolution::P67;
        d.graphics.vsync = false;
        d.graphics.touch();
        assert_eq!(d.graphics.quality, Quality::Custom);
        assert_eq!(Display::parse(&d.to_text()), d);
    }

    #[test]
    fn corrupt_or_missing_file_means_defaults() {
        assert_eq!(Display::parse(""), Display::default());
        assert_eq!(
            Display::parse("\u{0}\u{1}garbage\n===\n"),
            Display::default()
        );
        let d = Display::parse("shadows=huge\nui_scale=NaN\nvsync=maybe\nresolution=7%\n");
        assert_eq!(d, Display::default());
        assert_eq!(Display::parse("ui_scale=9\n").ui_scale, 1.8);
    }
}
