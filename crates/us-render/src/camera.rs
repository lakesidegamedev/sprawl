//! Orbit camera in metres: it looks at a target point from a distance, a heading and a tilt.

use crate::land::Land;
use glam::{Mat4, Vec3, Vec4};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Point on the ground the camera looks at, metres.
    pub target: Vec3,
    /// Eye to target distance, metres.
    pub distance: f32,
    /// Compass heading of the view: 0 looks north (+y), 90 looks east (+x).
    pub yaw_deg: f32,
    /// Degrees above the horizon the camera sits, 30 to 60 for the town views.
    pub tilt_deg: f32,
    /// Vertical field of view in degrees.
    pub fov_deg: f32,
}

pub struct View {
    pub eye: Vec3,
    pub view_proj: Mat4,
    pub inv_view_proj: Mat4,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    /// A camera that sees `width_m` metres of ground across the middle of a frame of this aspect.
    pub fn framing(target: Vec3, width_m: f32, aspect: f32, yaw_deg: f32, tilt_deg: f32) -> Camera {
        let fov_deg: f32 = 24.0;
        let half_h = (fov_deg.to_radians() / 2.0).tan() * aspect;
        Camera {
            target,
            distance: (width_m / 2.0) / half_h,
            yaw_deg,
            tilt_deg,
            fov_deg,
        }
    }

    pub fn view(&self, aspect: f32) -> View {
        let tilt = self.tilt_deg.clamp(5.0, 89.0).to_radians();
        let yaw = self.yaw_deg.to_radians();
        let fwd = Vec3::new(yaw.sin(), yaw.cos(), 0.0);
        let eye = self.target - fwd * (self.distance * tilt.cos())
            + Vec3::Z * (self.distance * tilt.sin());
        let (near, far) = (self.distance * 0.1, self.distance * 8.0);
        let view = Mat4::look_at_rh(eye, self.target, Vec3::Z);
        let proj = Mat4::perspective_rh(self.fov_deg.to_radians(), aspect, near, far);
        let view_proj = proj * view;
        View {
            eye,
            view_proj,
            inv_view_proj: view_proj.inverse(),
            near,
            far,
        }
    }
}

/// Where the screen point `ndc` (x right, y up, both -1 to 1) meets the ground plane z = 0.
pub fn pick_ground(view: &View, ndc: (f32, f32)) -> Option<Vec3> {
    let near = view.inv_view_proj * Vec4::new(ndc.0, ndc.1, 0.0, 1.0);
    let far = view.inv_view_proj * Vec4::new(ndc.0, ndc.1, 1.0, 1.0);
    let (a, b) = (near.truncate() / near.w, far.truncate() / far.w);
    let d = b - a;
    if d.z.abs() < 1e-6 {
        return None;
    }
    let t = -a.z / d.z;
    (t >= 0.0).then(|| a + d * t)
}

/// Screen position (x right, y up, both -1 to 1) of a world point, or None behind the camera.
pub fn project(view: &View, p: Vec3) -> Option<(f32, f32)> {
    let c = view.view_proj * p.extend(1.0);
    (c.w > 1e-6).then(|| (c.x / c.w, c.y / c.w))
}

/// Distance along the ray `a + t * dir` (dir unit length) to where it first meets the ground, up
/// to `t_max`. Level land is the plane z = 0; on hills the ray is walked in short steps against
/// the height grid and the hit refined by halving.
pub fn ray_ground(a: Vec3, dir: Vec3, t_max: f32, land: &Land) -> Option<f32> {
    if land.is_flat() {
        if dir.z.abs() < 1e-6 {
            return None;
        }
        let t = -a.z / dir.z;
        return (t >= 0.0 && t <= t_max).then_some(t);
    }
    let above = |t: f32| {
        let p = a + dir * t;
        p.z - land.height(p.x, p.y)
    };
    let step = 6.0f32;
    let mut t0 = 0.0f32;
    let mut h0 = above(t0);
    if h0 <= 0.0 {
        return Some(0.0);
    }
    let mut t1 = step;
    while t0 < t_max {
        let t = t1.min(t_max);
        let h1 = above(t);
        if h1 <= 0.0 {
            let (mut lo, mut hi) = (t0, t);
            for _ in 0..18 {
                let mid = 0.5 * (lo + hi);
                if above(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(0.5 * (lo + hi));
        }
        t0 = t;
        h0 = h1;
        t1 = t + step.max(h0.abs() * 0.25);
        if t >= t_max {
            break;
        }
    }
    None
}

/// Where the screen point `ndc` meets the land: the ground plane on level land, the hills
/// otherwise.
pub fn pick_terrain(view: &View, ndc: (f32, f32), land: &Land) -> Option<Vec3> {
    if land.is_flat() {
        return pick_ground(view, ndc);
    }
    let near = view.inv_view_proj * Vec4::new(ndc.0, ndc.1, 0.0, 1.0);
    let far = view.inv_view_proj * Vec4::new(ndc.0, ndc.1, 1.0, 1.0);
    let (a, b) = (near.truncate() / near.w, far.truncate() / far.w);
    let d = b - a;
    let len = d.length();
    if len < 1e-6 {
        return None;
    }
    let dir = d / len;
    ray_ground(a, dir, len, land).map(|t| a + dir * t)
}
