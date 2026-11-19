//! Winter road dressing: ploughed snowbanks along the kerbs, and the lane colours the carriageway
//! is drawn in so the shader can wear lanes into it.
//!
//! The strips are plain ground faces in marker colours. The shader draws banks in winter only
//! (they are discarded the rest of the year) and reads the blue channel as a position across the
//! strip: a bank runs from the road side (0) to the outer side (1), a carriageway from its right
//! edge (0) to its left edge (1). Nothing here is read by the simulation.

use crate::ground::Ground;
use crate::live::RoadDraw;
use crate::obj::Rgb;
use us_gen::RoadClass;

/// Layer of a bank: just under the carriageway, so it shows from the road edge outward and a
/// side street cuts through it.
const Z_BANK: f32 = 0.28;

/// Marker of a snowbank strip. Red 247 and green 248.
pub fn bank_colour(t: f32) -> Rgb {
    [247, 248, (140.0 + 113.0 * t.clamp(0.0, 1.0)).round() as u8]
}

/// Marker of a lane strip over a carriageway of half width `hw`. Red carries the half width
/// (200 plus four per metre), green is 246.
pub fn lane_colour(hw: f32, u: f32) -> Rgb {
    [
        (200.0 + hw * 4.0).round().clamp(200.0, 252.0) as u8,
        246,
        (20.0 + 230.0 * u.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// Banks along both kerbs. `hw_of` is the half
/// width each road is drawn with (slip lanes are narrower than their class).
pub fn paint(g: &mut Ground, roads: &[RoadDraw], hw_of: &dyn Fn(usize) -> f32, ramp: &[bool]) {
    for (i, r) in roads.iter().enumerate() {
        if matches!(r.class, RoadClass::Rail | RoadClass::Alley)
            || r.works.is_some()
            || r.points.len() < 2
        {
            continue;
        }
        let hw = hw_of(i);
        let ring = crate::roadpaint::ring_arc(r);
        if r.class == RoadClass::Highway || ramp.get(i).copied().unwrap_or(false) {
            continue;
        }
        if g.on_bridge(r.points) {
            continue;
        }
        for side in [-1.0f32, 1.0] {
            // a ring has its verge on the outside only
            if let Some((_, ccw)) = ring
                && (side > 0.0) == ccw
            {
                continue;
            }
            // t = 0 on the carriageway side
            let (right, left) = if side > 0.0 {
                (bank_colour(0.0), bank_colour(1.0))
            } else {
                (bank_colour(1.0), bank_colour(0.0))
            };
            g.ribbon_across(r.points, 0.8, side * (hw + 0.35), Z_BANK, right, left);
        }
    }
}
