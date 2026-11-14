//! Touch input: one finger pans, two fingers pinch, twist and tilt, a tap selects or places the
//! hall, a long press explains what is under the finger.

use super::{Screen, State};
use std::time::{Duration, Instant};

/// A finger that moves less than this (in pixels) is still a tap or a long press.
const SLOP: f32 = 12.0;
/// How long a finger must rest to count as a long press.
const LONG_PRESS: Duration = Duration::from_millis(450);

#[derive(Clone, Copy, PartialEq)]
pub enum Phase {
    Down,
    Move,
    Up,
    Cancel,
}

struct Finger {
    id: u64,
    pos: (f32, f32),
    began: Instant,
    travel: f32,
    /// The finger went down on a button or panel: the interface owns it, the camera never sees it.
    on_ui: bool,
    /// A long press already fired for this finger.
    held: bool,
    /// The finger took part in a two-finger gesture, so lifting it is never a tap.
    gestured: bool,
}

#[derive(Default)]
pub struct Touches {
    fingers: Vec<Finger>,
}

impl Touches {
    pub fn clear(&mut self) {
        self.fingers.clear();
    }

    /// The first two fingers that belong to the town view.
    fn view_pair(&self) -> Option<(usize, usize)> {
        let mut it = self
            .fingers
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.on_ui)
            .map(|(i, _)| i);
        Some((it.next()?, it.next()?))
    }
}

fn dist(p: (f32, f32), q: (f32, f32)) -> f32 {
    ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
}

fn angle_deg(a: (f32, f32), b: (f32, f32)) -> f32 {
    (b.1 - a.1).atan2(b.0 - a.0).to_degrees()
}

fn wrap_deg(mut d: f32) -> f32 {
    while d > 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}

impl State {
    /// One touch event in window pixels. `on_ui` says the finger landed on the interface (only
    /// meaningful for `Phase::Down`).
    pub fn touch(&mut self, id: u64, phase: Phase, x: f32, y: f32, on_ui: bool, height_px: u32) {
        let in_game = self.screen == Screen::Game && self.game.is_some();
        match phase {
            Phase::Down => {
                self.touches.fingers.retain(|f| f.id != id);
                self.touches.fingers.push(Finger {
                    id,
                    pos: (x, y),
                    began: Instant::now(),
                    travel: 0.0,
                    on_ui,
                    held: false,
                    gestured: false,
                });
                if self.touches.view_pair().is_some() {
                    for f in self.touches.fingers.iter_mut().filter(|f| !f.on_ui) {
                        f.gestured = true;
                    }
                }
            }
            Phase::Move => {
                let Some(i) = self.touches.fingers.iter().position(|f| f.id == id) else {
                    return;
                };
                let old = self.touches.fingers[i].pos;
                let (dx, dy) = (x - old.0, y - old.1);
                self.touches.fingers[i].pos = (x, y);
                self.touches.fingers[i].travel += (dx * dx + dy * dy).sqrt();
                if !in_game || self.touches.fingers[i].on_ui {
                    return;
                }
                if let Some((a, b)) = self.touches.view_pair() {
                    if i != a && i != b {
                        return;
                    }
                    let (pa, pb) = (self.touches.fingers[a].pos, self.touches.fingers[b].pos);
                    // where the pair was before this finger moved
                    let (qa, qb) = if i == a { (old, pb) } else { (pa, old) };
                    let (d0, d1) = (dist(qa, qb), dist(pa, pb));
                    let mid_dy = ((pa.1 + pb.1) - (qa.1 + qb.1)) * 0.5;
                    let twist = wrap_deg(angle_deg(pa, pb) - angle_deg(qa, qb));
                    if let Some(g) = self.game.as_mut()
                        && d0 > 20.0
                        && d1 > 20.0
                    {
                        g.cam.zoom_by(d1 / d0);
                        g.cam.twist(twist);
                        // a pinch or a twist should not also tilt the view
                        if (d1 / d0 - 1.0).abs() < 0.02 && twist.abs() < 1.5 {
                            g.cam.tilt_by(mid_dy * 0.25);
                        }
                    }
                } else if self.touches.fingers[i].travel > SLOP
                    && let Some(g) = self.game.as_mut()
                {
                    g.cam.drag_pan(dx, dy, height_px as f32);
                }
            }
            Phase::Up | Phase::Cancel => {
                let Some(i) = self.touches.fingers.iter().position(|f| f.id == id) else {
                    return;
                };
                let f = self.touches.fingers.remove(i);
                let tap = phase == Phase::Up
                    && in_game
                    && !f.on_ui
                    && !f.held
                    && !f.gestured
                    && f.travel <= SLOP
                    && f.began.elapsed() < LONG_PRESS;
                if tap {
                    self.click = Some(f.pos);
                }
            }
        }
    }

    /// Once a frame: a finger resting on the town for long enough explains what is under it.
    pub fn touch_tick(&mut self) {
        if self.screen != Screen::Game || self.game.is_none() {
            return;
        }
        let mut hit = None;
        for f in self.touches.fingers.iter_mut() {
            if !f.on_ui
                && !f.held
                && !f.gestured
                && f.travel <= SLOP
                && f.began.elapsed() >= LONG_PRESS
            {
                f.held = true;
                hit = Some(f.pos);
            }
        }
        // Nothing to explain until the town hall is down; placing it is a tap.
        if let Some(p) = hit
            && self.game.as_ref().is_some_and(|g| g.placed)
        {
            self.click = Some(p);
        }
    }
}
