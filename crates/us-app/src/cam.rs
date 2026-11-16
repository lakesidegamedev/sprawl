//! Smooth orbit camera: right drag orbits, left drag or WASD pans, wheel zooms, Q and E rotate.

use glam::Vec3;
use us_render::Camera;

pub struct CamCtl {
    target: Vec3,
    dist: f32,
    yaw: f32,
    tilt: f32,
    g_target: Vec3,
    g_dist: f32,
    g_yaw: f32,
    g_tilt: f32,
    map: f32,
    /// Seconds left of a fly-to: the camera glides slowly instead of snapping to the goal.
    flying: f32,
    /// Extra tilt that lifts the camera over a mountain in front of it (shown, goal).
    lift: f32,
    g_lift: f32,
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
            g_target: target,
            g_dist: dist,
            g_yaw: yaw,
            g_tilt: tilt,
            map,
            flying: 0.0,
            lift: 0.0,
            g_lift: 0.0,
        }
    }

    /// Fly to look at a point from a given distance.
    pub fn look_at(&mut self, p: Vec3, dist: f32) {
        self.g_target = p;
        self.g_dist = dist;
        self.g_tilt = 48.0;
        self.flying = 1.2;
    }

    /// Keep the camera's target on the ground under it: `z` is the ground height there.
    pub fn follow_ground(&mut self, z: f32) {
        self.g_target.z = z;
    }

    /// Where the camera goal is on the map.
    pub fn goal_xy(&self) -> (f32, f32) {
        (self.g_target.x, self.g_target.y)
    }

    fn forward(&self) -> Vec3 {
        let y = self.g_yaw.to_radians();
        Vec3::new(y.sin(), y.cos(), 0.0)
    }

    fn right(&self) -> Vec3 {
        let y = self.g_yaw.to_radians();
        Vec3::new(y.cos(), -y.sin(), 0.0)
    }

    fn metres_per_pixel(&self, height_px: f32) -> f32 {
        2.0 * self.g_dist * (FOV.to_radians() / 2.0).tan() / height_px.max(1.0)
    }

    pub fn drag_pan(&mut self, dx: f32, dy: f32, height_px: f32) {
        self.flying = 0.0;
        let mpp = self.metres_per_pixel(height_px);
        let sin_t = self.g_tilt.to_radians().sin().max(0.25);
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

    /// Ease the shown camera toward the goal.
    pub fn update(&mut self, dt: f32) {
        let rate = if self.flying > 0.0 { 4.5 } else { 14.0 };
        self.flying = (self.flying - dt).max(0.0);
        let k = if crate::theme::reduce_motion() {
            1.0
        } else {
            1.0 - (-dt * rate).exp()
        };
        self.target += (self.g_target - self.target) * k;
        self.dist += (self.g_dist - self.dist) * k;
        self.yaw += (self.g_yaw - self.yaw) * k;
        self.tilt += (self.g_tilt - self.tilt) * k;
        self.lift += (self.g_lift - self.lift) * k;
    }

    /// On land with mountains: raise the viewing angle just enough that the line from the
    /// target to the eye clears the ground, so the player sees the town and not a ridge in front
    /// of the camera. Level land and gentle hills are left alone.
    pub fn clear_of_terrain(&mut self, land: &us_render::Land) {
        if land.rock().is_none() {
            self.g_lift = 0.0;
            return;
        }
        let yaw = self.g_yaw.to_radians();
        let fwd = (yaw.sin(), yaw.cos());
        let base = self.g_tilt;
        let mut tilt = base;
        while tilt < 84.0 {
            let t = tilt.to_radians();
            let (back, up) = (self.g_dist * t.cos(), self.g_dist * t.sin());
            let clear = (1..=40).all(|k| {
                let f = k as f32 / 40.0;
                let x = self.g_target.x - fwd.0 * back * f;
                let y = self.g_target.y - fwd.1 * back * f;
                self.g_target.z + up * f > land.height(x, y) + 40.0
            });
            if clear {
                break;
            }
            tilt += 2.0;
        }
        self.g_lift = (tilt - base).max(0.0);
    }

    pub fn camera(&self) -> Camera {
        Camera {
            target: self.target,
            distance: self.dist,
            yaw_deg: self.yaw,
            tilt_deg: (self.tilt + self.lift).min(88.0),
            fov_deg: FOV,
        }
    }
}
