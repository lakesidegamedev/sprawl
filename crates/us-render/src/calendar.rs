//! Time of day and season from the game date: stepped, never smooth. Each phase of the day has
//! one flat tint, one sun direction and one share of lit windows; each season has one look.

/// The four seasons of the northern year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// One of the four stepped parts of the day.
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

    /// Flat colour multiplier for the whole picture in this phase.
    pub fn tone(&self) -> [f32; 3] {
        match self.phase() {
            DayPhase::Day => [1.0, 1.0, 1.0],
            DayPhase::Dawn => [0.86, 0.80, 0.88],
            DayPhase::Dusk => [1.0, 0.80, 0.68],
            DayPhase::Night => [0.36, 0.42, 0.66],
        }
    }

    /// Share of windows that are lit in this phase, 0 to 1.
    pub fn lit_windows(&self) -> f32 {
        match self.phase() {
            DayPhase::Day => 0.0,
            DayPhase::Dawn => 0.25,
            DayPhase::Dusk => 0.45,
            DayPhase::Night => 0.75,
        }
    }

    /// Unit vector toward the light (sun by day, moon by night), stepped by the hour.
    pub fn light(&self) -> [f32; 3] {
        let h = self.hour.rem_euclid(24.0);
        let (x, y, z) = if self.phase() == DayPhase::Night {
            (-0.45f32, 0.3f32, 0.84f32)
        } else {
            let hs = libm::floorf(h.clamp(5.0, 19.0));
            let a = std::f32::consts::PI * ((hs + 0.5 - 6.0) / 12.0).clamp(-0.1, 1.1);
            let up = libm::sinf(a).abs().max(0.0);
            (
                libm::cosf(a),
                -0.55 * libm::sinf(a),
                (0.32 + 0.5 * up).min(0.85),
            )
        };
        let m = (x * x + y * y + z * z).sqrt();
        [x / m, y / m, z / m]
    }

    /// Brightness multiplier of a shadowed face (night shadows are barely darker).
    pub fn shadow_tone(&self) -> f32 {
        if self.phase() == DayPhase::Night {
            0.86
        } else {
            0.72
        }
    }
}

impl Default for Calendar {
    fn default() -> Calendar {
        Calendar {
            day_of_year: 180,
            hour: 12.0,
        }
    }
}
