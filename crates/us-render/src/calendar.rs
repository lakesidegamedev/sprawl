//! Time of day and season from the game date. The day is smooth: tint, light direction and the
//! share of lit windows move continuously with the hour. Seasons are one look each.

/// The four seasons of the northern year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// A name for the part of the day (labels only; the picture itself never steps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

/// The moment the world is drawn at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calendar {
    /// Day of the year, 0 (1 January) to 364.
    pub day_of_year: u16,
    /// Hour of the day, 0.0 to 24.0.
    pub hour: f32,
}

const MONTH_START: [u16; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

impl Calendar {
    /// From a calendar date (month 1 to 12, day 1 to 31) and an hour of the day.
    pub fn from_date(month: u32, day: u32, hour: f32) -> Calendar {
        let m = (month.clamp(1, 12) - 1) as usize;
        let d = (day.max(1) - 1) as u16;
        Calendar {
            day_of_year: (MONTH_START[m] + d).min(364),
            hour: hour.rem_euclid(24.0),
        }
    }

    pub fn season(&self) -> Season {
        match self.day_of_year {
            0..=58 | 334..=u16::MAX => Season::Winter,
            59..=150 => Season::Spring,
            151..=242 => Season::Summer,
            _ => Season::Autumn,
        }
    }

    pub fn phase(&self) -> DayPhase {
        let h = self.hour.rem_euclid(24.0);
        if (5.0..7.0).contains(&h) {
            DayPhase::Dawn
        } else if (7.0..17.5).contains(&h) {
            DayPhase::Day
        } else if (17.5..20.0).contains(&h) {
            DayPhase::Dusk
        } else {
            DayPhase::Night
        }
    }

    /// How dark it is, 0 (full day) to 1 (full night). Smooth: it ramps up through the evening
    /// and down through the morning, so tone, light and lit windows never step.
    pub fn darkness(&self) -> f32 {
        let h = self.hour.rem_euclid(24.0);
        let (rise0, rise1, fall0, fall1) = (5.0, 7.5, 17.5, 20.5);
        let n = if h < rise0 || h >= fall1 {
            1.0
        } else if h < rise1 {
            1.0 - smooth((h - rise0) / (rise1 - rise0))
        } else if h < fall0 {
            0.0
        } else {
            smooth((h - fall0) / (fall1 - fall0))
        };
        n.clamp(0.0, 1.0)
    }

    /// Flat colour multiplier for the whole picture: day, a warm or cool twilight tint, night,
    /// blended continuously by `darkness`.
    pub fn tone(&self) -> [f32; 3] {
        let day = [1.0, 1.0, 1.0];
        let night = [0.36, 0.42, 0.66];
        let twilight = if self.hour.rem_euclid(24.0) < 12.0 {
            [0.86, 0.80, 0.88]
        } else {
            [1.0, 0.80, 0.68]
        };
        let n = self.darkness();
        if n < 0.5 {
            lerp3(day, twilight, n * 2.0)
        } else {
            lerp3(twilight, night, n * 2.0 - 1.0)
        }
    }

    /// Share of windows that are lit, 0 to 1. Rises through the evening so windows come on a
    /// few at a time, and falls again through the morning.
    pub fn lit_windows(&self) -> f32 {
        0.75 * smooth(((self.darkness() - 0.05) / 0.8).clamp(0.0, 1.0)) * self.late_share()
    }

    /// Fewer windows stay lit in the small hours: full until 23:00, about half by 03:00, back to
    /// full as the morning comes on at 05:00.
    fn late_share(&self) -> f32 {
        let h = self.hour.rem_euclid(24.0);
        if h >= 23.0 {
            1.0 - 0.15 * (h - 23.0)
        } else if h < 3.0 {
            0.85 - 0.35 * (h / 3.0)
        } else if h < 5.0 {
            0.5 + 0.5 * ((h - 3.0) / 2.0)
        } else {
            1.0
        }
    }

    /// Share of vehicle lights and street lamps that are on, 0 to 1 (they come on a little
    /// after the first windows).
    pub fn lamps(&self) -> f32 {
        smooth(((self.darkness() - 0.25) / 0.5).clamp(0.0, 1.0))
    }

    /// Unit vector toward the light: the sun crossing the sky by day, blending into the moon as
    /// it gets dark. Moves smoothly with the hour.
    pub fn light(&self) -> [f32; 3] {
        let h = self.hour.rem_euclid(24.0);
        let a = std::f32::consts::PI * ((h.clamp(5.0, 19.0) - 6.0) / 12.0).clamp(-0.1, 1.1);
        let up = libm::sinf(a).abs();
        let sun = [
            libm::cosf(a),
            -0.55 * libm::sinf(a),
            (0.32 + 0.5 * up).min(0.85),
        ];
        let moon = [-0.45f32, 0.3, 0.84];
        let w = smooth(((self.darkness() - 0.1) / 0.9).clamp(0.0, 1.0));
        let v = lerp3(sun, moon, w);
        let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / m, v[1] / m, v[2] / m]
    }

    /// Brightness multiplier of a shadowed face (night shadows are barely darker).
    pub fn shadow_tone(&self) -> f32 {
        0.60 + 0.26 * self.darkness()
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Default for Calendar {
    fn default() -> Calendar {
        Calendar {
            day_of_year: 180,
            hour: 12.0,
        }
    }
}
