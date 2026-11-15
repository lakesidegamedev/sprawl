//! The weather as the rest of the simulation sees it. Until the weather system fills these in
//! the sky is calm: no wind, average dryness, no rain, snow or storm. Fires, traffic, power and
//! the renderer read the weather only through these calls, so they work with or without it.

use crate::Sim;
use us_gen::{Pt, pt};

impl Sim {
    /// Wind as a vector in metres per second (east is +x, north is +y).
    pub fn wind(&self) -> Pt {
        pt(0.0, 0.0)
    }

    /// How dry the town is, 0 (soaked) to 1 (drought). Fires catch and spread more when dry.
    pub fn dryness(&self) -> f64 {
        0.5
    }

    /// Rain falling now, 0 to 1.
    pub fn rain(&self) -> f32 {
        0.0
    }

    /// Snow falling now, 0 to 1.
    pub fn snowfall(&self) -> f32 {
        0.0
    }

    /// Snow lying on the ground, 0 to 1.
    pub fn snow_cover(&self) -> f32 {
        0.0
    }

    /// A thunderstorm is over the town now.
    pub fn storm(&self) -> bool {
        false
    }

    /// Lots that are under flood water now (lot indices). Empty when there is no flood.
    pub fn flooded(&self) -> &[usize] {
        &[]
    }
}
