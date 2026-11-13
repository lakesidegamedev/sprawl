//! Orbit camera in metres: it looks at a target point from a distance, a heading and a tilt.

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
