//! Smooth orbit camera: right drag orbits, left drag or WASD pans, wheel zooms, Q and E rotate.

use glam::{Vec2, Vec3};
use us_render::{Camera, Land};

pub struct CamCtl {
    target: Vec3,
    dist: f32,
    yaw: f32,
    tilt: f32,
    g_target: Vec2,
    g_dist: f32,
    g_yaw: f32,
    g_tilt: f32,
    map: f32,
    /// Seconds left of a fly-to: the camera glides slowly instead of snapping to the goal.
    flying: f32,
    /// Extra tilt that keeps the view clear of terrain.
    lift: f32,
}

const FOV: f32 = 24.0;

impl CamCtl {
    pub fn overview(map: f32, aspect: f32) -> CamCtl {
        let c = Camera::framing(
            Vec3::new(map * 0.5, map * 0.42, 0.0),
            map * 1.05,
            aspect,
            0.0,
            52.0,
        );
        CamCtl::at(c.target, c.distance, 0.0, 52.0, map)
    }

    fn at(target: Vec3, dist: f32, yaw: f32, tilt: f32, map: f32) -> CamCtl {
        CamCtl {
            target,
            dist,
            yaw,
            tilt,
            g_target: target.truncate(),
            g_dist: dist,
            g_yaw: yaw,
            g_tilt: tilt,
            map,
            flying: 0.0,
            lift: 0.0,
        }
    }

    /// Fly to look at a point from a given distance.
    pub fn look_at(&mut self, p: Vec2, dist: f32) {
        self.g_target = p;
        self.g_dist = dist;
        self.g_tilt = 48.0;
        self.flying = 1.2;
    }

    /// Turn the camera goal to `yaw` degrees and tilt it to `tilt` (screenshots).
    pub fn set_angle(&mut self, yaw: f32, tilt: f32) {
        self.g_yaw = yaw;
        self.g_tilt = tilt;
    }

    fn forward(&self) -> Vec2 {
        let y = self.g_yaw.to_radians();
        Vec2::new(y.sin(), y.cos())
    }

    fn right(&self) -> Vec2 {
        let y = self.g_yaw.to_radians();
        Vec2::new(y.cos(), -y.sin())
    }

    fn metres_per_pixel(&self, height_px: f32) -> f32 {
        2.0 * self.g_dist * (FOV.to_radians() / 2.0).tan() / height_px.max(1.0)
    }

    pub fn drag_pan(&mut self, dx: f32, dy: f32, height_px: f32) {
        self.flying = 0.0;
        let mpp = self.metres_per_pixel(height_px);
        let sin_t = self.camera().tilt_deg.to_radians().sin().max(0.25);
        self.g_target += -self.right() * dx * mpp + self.forward() * dy * mpp / sin_t;
        self.clamp();
    }

    pub fn drag_orbit(&mut self, dx: f32, dy: f32) {
        self.g_yaw += dx * 0.25;
        self.g_tilt = (self.g_tilt + dy * 0.2).clamp(18.0, 82.0);
    }

    pub fn zoom(&mut self, lines: f32) {
        self.g_dist = (self.g_dist * 0.88f32.powf(lines)).clamp(70.0, 7000.0);
    }

    /// Pinch: a factor above 1 brings the camera closer.
    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.g_dist = (self.g_dist / factor).clamp(70.0, 7000.0);
        }
    }

    /// Two-finger twist: turn the town with the fingers (clockwise degrees on screen).
    pub fn twist(&mut self, clockwise_deg: f32) {
        self.g_yaw -= clockwise_deg;
    }

    /// Two-finger vertical drag: raise or lower the viewing angle.
    pub fn tilt_by(&mut self, deg: f32) {
        self.g_tilt = (self.g_tilt + deg).clamp(18.0, 82.0);
    }

    pub fn keys(&mut self, dt: f32, pan: (f32, f32), rot: f32, fast: bool) {
        let speed = self.g_dist * 0.9 * if fast { 2.5 } else { 1.0 };
        if pan != (0.0, 0.0) {
            self.flying = 0.0;
        }
        self.g_target += (self.forward() * pan.1 + self.right() * pan.0) * speed * dt;
        self.g_yaw += rot * 70.0 * dt;
        self.clamp();
    }

    fn clamp(&mut self) {
        self.g_target.x = self.g_target.x.clamp(0.0, self.map);
        self.g_target.y = self.g_target.y.clamp(0.0, self.map);
    }

    /// Ease the shown camera toward the goal, then fit that frame to the land it crosses.
    pub fn update(&mut self, dt: f32, land: &Land) {
        let rate = if self.flying > 0.0 { 4.5 } else { 14.0 };
        self.flying = (self.flying - dt).max(0.0);
        let reduced = crate::theme::reduce_motion();
        let k = if reduced {
            1.0
        } else {
            1.0 - (-dt * rate).exp()
        };
        let xy = self.target.truncate().lerp(self.g_target, k);
        self.target = xy.extend(land.height(xy.x, xy.y));
        self.dist += (self.g_dist - self.dist) * k;
        self.yaw += (self.g_yaw - self.yaw) * k;
        self.tilt += (self.g_tilt - self.tilt) * k;

        // Ease back down when possible, then check the angle the player will actually see.
        let settling = if reduced {
            0.0
        } else {
            self.lift * (-dt * 7.0).exp()
        };
        let candidate = (self.tilt + settling).min(90.0);
        self.lift = self.clear_tilt(land, candidate) - self.tilt;
    }

    fn clear_tilt(&self, land: &Land, candidate: f32) -> f32 {
        if land.is_flat() {
            return candidate;
        }

        let (sin_yaw, cos_yaw) = self.yaw.to_radians().sin_cos();
        let back = -Vec2::new(sin_yaw, cos_yaw);
        let clears = |tilt: f32| {
            let (run, rise) = (90.0 - tilt).to_radians().sin_cos();
            // Start at the ground under the focus and build up to 8 m of clearance at the eye.
            // A fixed margin near the focus would force even an open plateau overhead.
            let offset = (back * (self.dist * run)).extend(self.dist * rise - 8.0);
            land.segment_above_ground(self.target, self.target + offset)
        };

        if clears(candidate) {
            return candidate;
        }

        let (mut blocked, mut clear) = (candidate, 90.0);
        // Start with the unobstructed vertical view and keep the upper angle clear.
        for _ in 0..9 {
            let middle = (blocked + clear) * 0.5;
            if clears(middle) {
                clear = middle;
            } else {
                blocked = middle;
            }
        }
        clear
    }

    pub fn camera(&self) -> Camera {
        Camera {
            target: self.target,
            distance: self.dist,
            yaw_deg: self.yaw,
            tilt_deg: self.tilt + self.lift,
            fov_deg: FOV,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};
    use us_render::RockLook;

    fn frame(cam: &mut CamCtl, land: &Land) -> Camera {
        cam.update(1.0 / 60.0, land);
        cam.camera()
    }

    #[test]
    fn an_open_plateau_keeps_the_requested_angle_at_every_zoom() {
        let land =
            Land::from_grid(3, 200.0, vec![120.0; 9], vec![]).with_rock(RockLook::of(Preset::Mesa));
        for distance in [70.0, 700.0, 7000.0] {
            let mut cam = CamCtl::at(Vec3::new(100.0, 100.0, 120.0), distance, 0.0, 30.0, 200.0);
            let view = frame(&mut cam, &land);
            assert_eq!(view.tilt_deg, 30.0, "distance {distance}");
        }
    }

    #[test]
    fn flying_to_the_valley_keeps_the_intermediate_ridge_above_ground() {
        let land = Land::from_grid(
            3,
            200.0,
            vec![0.0, 210.0, 0.0, 0.0, 210.0, 0.0, 0.0, 210.0, 0.0],
            vec![],
        );
        let mut cam = CamCtl::at(Vec3::new(0.0, 100.0, 0.0), 70.0, 0.0, 40.0, 200.0);
        cam.look_at(Vec2::new(200.0, 100.0), 70.0);
        for _ in 0..120 {
            let camera = frame(&mut cam, &land);
            assert!(camera.target.z >= land.height(camera.target.x, camera.target.y));
        }
    }

    #[test]
    fn camera_stays_clear_while_traversing_steep_presets() {
        for preset in [Preset::Valley, Preset::Gorge, Preset::Coast, Preset::Mesa] {
            let town = generate(preset, 4);
            let land = Land::from_town(&town);
            let size = land.size();
            let mut cam = CamCtl::overview(size, 1.7);
            for fraction in [0.25, 0.75, 0.5] {
                cam.look_at(Vec2::new(fraction * size, 0.5 * size), 450.0);
                cam.set_angle(fraction * 720.0, 18.0);
                for step in 0..100 {
                    if step == 70 {
                        cam.zoom_by(20.0);
                    }
                    assert_clear(frame(&mut cam, &land), &land);
                }
            }
        }
    }

    fn assert_clear(camera: Camera, land: &Land) {
        let eye = camera.view(1.7).eye;
        for step in 1..=400 {
            let p = camera.target.lerp(eye, step as f32 / 400.0);
            assert!(
                p.z > land.height(p.x, p.y),
                "view blocked at {p:?}: {camera:?}"
            );
        }
    }

    #[test]
    fn orbit_lifts_over_a_ridge_and_settles_back_after_turning() {
        // A ridge with a clear eye and a clear target can still block the view between them.
        let land = Land::from_grid(
            3,
            200.0,
            vec![0.0, 0.0, 0.0, 120.0, 120.0, 120.0, 0.0, 0.0, 0.0],
            vec![],
        );
        let mut cam = CamCtl::at(Vec3::new(100.0, 200.0, 0.0), 220.0, 0.0, 30.0, 200.0);
        let camera = frame(&mut cam, &land);
        assert!(camera.tilt_deg > 45.0);
        assert_clear(camera, &land);

        cam.set_angle(180.0, 30.0);
        for _ in 0..120 {
            assert_clear(frame(&mut cam, &land), &land);
        }
        assert!((cam.camera().tilt_deg - 30.0).abs() < 0.01);
    }
}
