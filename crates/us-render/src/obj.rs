//! Model builder: a list of flat polygons in local metres (x east, y north, z up), front door
//! facing south (-y). Port of the mockup kit's `Obj`.

use crate::mesh::{ID_NONE, Mesh};

pub type V3 = [f32; 3];
pub type Rgb = [u8; 3];

#[derive(Clone, Debug)]
pub struct Poly {
    pub pts: Vec<V3>,
    pub color: Rgb,
    /// Casts a shadow.
    pub shadow: bool,
    /// Gets an ink outline.
    pub outline: bool,
    /// Flat colour: no cel tone (windows, doors, signs).
    pub flat: bool,
    /// Index into `Obj::solids` of the solid this face belongs to, if any.
    pub solid: Option<u32>,
    /// A sloping roof face.
    pub roof: bool,
}

/// A convex solid built from several polygons. `centre` lies inside it.
#[derive(Clone, Debug)]
pub struct Solid {
    pub centre: V3,
}

#[derive(Clone, Debug, Default)]
pub struct Obj {
    pub polys: Vec<Poly>,
    pub solids: Vec<Solid>,
}

fn cosd(deg: f32) -> f32 {
    libm::cosf(deg.to_radians())
}
fn sind(deg: f32) -> f32 {
    libm::sinf(deg.to_radians())
}

impl Obj {
    pub fn new() -> Obj {
        Obj::default()
    }

    pub fn poly(
        &mut self,
        pts: &[V3],
        color: Rgb,
        shadow: bool,
        outline: bool,
        flat: bool,
    ) -> &mut Self {
        self.polys.push(Poly {
            pts: pts.to_vec(),
            color,
            shadow,
            outline,
            flat,
            solid: None,
            roof: false,
        });
        self
    }

    fn solid_poly(&mut self, s: u32, pts: &[V3], color: Rgb, shadow: bool, roof: bool) {
        self.polys.push(Poly {
            pts: pts.to_vec(),
            color,
            shadow,
            outline: true,
            flat: false,
            solid: Some(s),
            roof,
        });
    }

    fn new_solid(&mut self, centre: V3) -> u32 {
        self.solids.push(Solid { centre });
        (self.solids.len() - 1) as u32
    }

    #[allow(clippy::too_many_arguments)]
    pub fn box_(
        &mut self,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        color: Rgb,
        top: Option<Rgb>,
        shadow: bool,
    ) -> &mut Self {
        let top = top.unwrap_or(color);
        let s = self.new_solid([(x0 + x1) / 2.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0]);
        let p = |x, y, z| [x, y, z];
        self.solid_poly(
            s,
            &[p(x0, y0, z1), p(x1, y0, z1), p(x1, y1, z1), p(x0, y1, z1)],
            top,
            shadow,
            false,
        );
        // south, north, west, east, each wound counter-clockwise seen from outside
        self.solid_poly(
            s,
            &[p(x0, y0, z0), p(x1, y0, z0), p(x1, y0, z1), p(x0, y0, z1)],
            color,
            shadow,
            false,
        );
        self.solid_poly(
            s,
            &[p(x1, y1, z0), p(x0, y1, z0), p(x0, y1, z1), p(x1, y1, z1)],
            color,
            shadow,
            false,
        );
        self.solid_poly(
            s,
            &[p(x0, y1, z0), p(x0, y0, z0), p(x0, y0, z1), p(x0, y1, z1)],
            color,
            shadow,
            false,
        );
        self.solid_poly(
            s,
            &[p(x1, y0, z0), p(x1, y1, z0), p(x1, y1, z1), p(x1, y0, z1)],
            color,
            shadow,
            false,
        );
        self
    }

    /// A plain box that casts a shadow.
    #[allow(clippy::too_many_arguments)]
    pub fn bx(
        &mut self,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        color: Rgb,
    ) -> &mut Self {
        self.box_(x0, y0, z0, x1, y1, z1, color, None, true)
    }

    /// A box with its own top colour.
    #[allow(clippy::too_many_arguments)]
    pub fn bx_top(
        &mut self,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        color: Rgb,
        top: Rgb,
    ) -> &mut Self {
        self.box_(x0, y0, z0, x1, y1, z1, color, Some(top), true)
    }

    /// A slab on the ground that casts no shadow (aprons, courts, lawns).
    #[allow(clippy::too_many_arguments)]
    pub fn slab(
        &mut self,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        color: Rgb,
    ) -> &mut Self {
        self.box_(x0, y0, z0, x1, y1, z1, color, Some(color), false)
    }

    /// Pitched roof. Ridge along x (or y), eaves overhang `over` metres.
    #[allow(clippy::too_many_arguments)]
    pub fn gable(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        z: f32,
        h: f32,
        roof: Rgb,
        wall: Rgb,
        along_x: bool,
        over: f32,
    ) -> &mut Self {
        let s = self.new_solid([(x0 + x1) / 2.0, (y0 + y1) / 2.0, z + h * 0.3]);
        let (a, b) = (x0 - over, x1 + over);
        let (c, d) = (y0 - over, y1 + over);
        if along_x {
            let ym = (y0 + y1) / 2.0;
            self.solid_poly(
                s,
                &[[a, c, z], [b, c, z], [b, ym, z + h], [a, ym, z + h]],
                roof,
                true,
                true,
            );
            self.solid_poly(
                s,
                &[[b, d, z], [a, d, z], [a, ym, z + h], [b, ym, z + h]],
                roof,
                true,
                true,
            );
            self.solid_poly(
                s,
                &[[x0, y1, z], [x0, y0, z], [x0, ym, z + h]],
                wall,
                true,
                false,
            );
            self.solid_poly(
                s,
                &[[x1, y0, z], [x1, y1, z], [x1, ym, z + h]],
                wall,
                true,
                false,
            );
        } else {
            let xm = (x0 + x1) / 2.0;
            self.solid_poly(
                s,
                &[[a, d, z], [a, c, z], [xm, c, z + h], [xm, d, z + h]],
                roof,
                true,
                true,
            );
            self.solid_poly(
                s,
                &[[b, c, z], [b, d, z], [xm, d, z + h], [xm, c, z + h]],
                roof,
                true,
                true,
            );
            self.solid_poly(
                s,
                &[[x0, y0, z], [x1, y0, z], [xm, y0, z + h]],
                wall,
                true,
                false,
            );
            self.solid_poly(
                s,
                &[[x1, y1, z], [x0, y1, z], [xm, y1, z + h]],
                wall,
                true,
                false,
            );
        }
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pyramid(
        &mut self,
        cx: f32,
        cy: f32,
        z: f32,
        r: f32,
        h: f32,
        color: Rgb,
        sides: usize,
        rot: f32,
    ) -> &mut Self {
        let s = self.new_solid([cx, cy, z + h * 0.25]);
        let pts: Vec<V3> = (0..sides)
            .map(|i| {
                let a = rot + i as f32 * 360.0 / sides as f32;
                [cx + r * cosd(a), cy + r * sind(a), z]
            })
            .collect();
        let apex = [cx, cy, z + h];
        for i in 0..sides {
            self.solid_poly(s, &[pts[i], pts[(i + 1) % sides], apex], color, true, true);
        }
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prism(
        &mut self,
        cx: f32,
        cy: f32,
        z0: f32,
        z1: f32,
        r: f32,
        color: Rgb,
        sides: usize,
        top: Option<Rgb>,
    ) -> &mut Self {
        let s = self.new_solid([cx, cy, (z0 + z1) / 2.0]);
        let pts: Vec<[f32; 2]> = (0..sides)
            .map(|i| {
                let a = 360.0 * i as f32 / sides as f32;
                [cx + r * cosd(a), cy + r * sind(a)]
            })
            .collect();
        for i in 0..sides {
            let (a, b) = (pts[i], pts[(i + 1) % sides]);
            self.solid_poly(
                s,
                &[
                    [a[0], a[1], z0],
                    [b[0], b[1], z0],
                    [b[0], b[1], z1],
                    [a[0], a[1], z1],
                ],
                color,
                true,
                false,
            );
        }
        let ring: Vec<V3> = pts.iter().map(|p| [p[0], p[1], z1]).collect();
        self.solid_poly(s, &ring, top.unwrap_or(color), true, false);
        self
    }

    /// Rectangle on a south facing wall at y, drawn just outside it.
    pub fn win_s(&mut self, x0: f32, x1: f32, z0: f32, z1: f32, y: f32, color: Rgb) -> &mut Self {
        let y = y - 0.06;
        self.poly(
            &[[x0, y, z0], [x1, y, z0], [x1, y, z1], [x0, y, z1]],
            color,
            false,
            true,
            true,
        )
    }

    pub fn win_n(&mut self, x0: f32, x1: f32, z0: f32, z1: f32, y: f32, color: Rgb) -> &mut Self {
        let y = y + 0.06;
        self.poly(
            &[[x1, y, z0], [x0, y, z0], [x0, y, z1], [x1, y, z1]],
            color,
            false,
            true,
            true,
        )
    }

    pub fn win_w(&mut self, y0: f32, y1: f32, z0: f32, z1: f32, x: f32, color: Rgb) -> &mut Self {
        let x = x - 0.06;
        self.poly(
            &[[x, y1, z0], [x, y0, z0], [x, y0, z1], [x, y1, z1]],
            color,
            false,
            true,
            true,
        )
    }

    pub fn win_e(&mut self, y0: f32, y1: f32, z0: f32, z1: f32, x: f32, color: Rgb) -> &mut Self {
        let x = x + 0.06;
        self.poly(
            &[[x, y0, z0], [x, y1, z0], [x, y1, z1], [x, y0, z1]],
            color,
            false,
            true,
            true,
        )
    }

    /// Rows of windows on all four walls of a box.
    #[allow(clippy::too_many_arguments)]
    pub fn windows_box(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        floors: usize,
        fh: f32,
        color: Rgb,
        w: f32,
        gap: f32,
    ) -> &mut Self {
        for f in 0..floors {
            let (za, zb) = (f as f32 * fh + fh * 0.32, f as f32 * fh + fh * 0.78);
            let n = (((x1 - x0) / gap) as usize).max(1);
            for i in 0..n {
                let cx = x0 + (i as f32 + 0.5) * (x1 - x0) / n as f32;
                self.win_s(cx - w / 2.0, cx + w / 2.0, za, zb, y0, color);
                self.win_n(cx - w / 2.0, cx + w / 2.0, za, zb, y1, color);
            }
            let n = (((y1 - y0) / gap) as usize).max(1);
            for i in 0..n {
                let cy = y0 + (i as f32 + 0.5) * (y1 - y0) / n as f32;
                self.win_w(cy - w / 2.0, cy + w / 2.0, za, zb, x0, color);
                self.win_e(cy - w / 2.0, cy + w / 2.0, za, zb, x1, color);
            }
        }
        self
    }

    /// Append `other` moved to (x, y) and turned by `theta` radians.
    pub fn add_placed(&mut self, other: &Obj, x: f32, y: f32, theta: f32) {
        let (s, c) = (libm::sinf(theta), libm::cosf(theta));
        let base = self.solids.len() as u32;
        for sol in &other.solids {
            let [px, py, pz] = sol.centre;
            self.solids.push(Solid {
                centre: [x + c * px - s * py, y + s * px + c * py, pz],
            });
        }
        for p in &other.polys {
            let mut q = p.clone();
            for v in &mut q.pts {
                *v = [x + c * v[0] - s * v[1], y + s * v[0] + c * v[1], v[2]];
            }
            q.solid = p.solid.map(|i| i + base);
            self.polys.push(q);
        }
    }

    /// Bounding box in x and y: (min x, min y, max x, max y).
    pub fn footprint(&self) -> [f32; 4] {
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for p in &self.polys {
            for v in &p.pts {
                b[0] = b[0].min(v[0]);
                b[1] = b[1].min(v[1]);
                b[2] = b[2].max(v[0]);
                b[3] = b[3].max(v[1]);
            }
        }
        b
    }

    /// Emit into a mesh, moved to (x, y), turned by `theta` and scaled. `next_id` hands out the
    /// outline ids: one per outlined polygon.
    pub fn emit(&self, mesh: &mut Mesh, next_id: &mut u32, x: f32, y: f32, theta: f32, scale: f32) {
        let (s, c) = (libm::sinf(theta), libm::cosf(theta));
        let mut pts: Vec<V3> = Vec::with_capacity(8);
        for p in &self.polys {
            pts.clear();
            pts.extend(p.pts.iter().map(|v| {
                [
                    x + scale * (c * v[0] - s * v[1]),
                    y + scale * (s * v[0] + c * v[1]),
                    scale * v[2],
                ]
            }));
            let id = if p.outline {
                let id = *next_id;
                *next_id += 1;
                id
            } else {
                ID_NONE
            };
            mesh.push_poly(&pts, p.color, p.flat, p.shadow, id);
        }
    }
}

/// Newell normal of a planar polygon, unit length (zero for a degenerate polygon).
pub fn newell(pts: &[V3]) -> V3 {
    let (mut nx, mut ny, mut nz) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        nx += (a[1] - b[1]) * (a[2] + b[2]);
        ny += (a[2] - b[2]) * (a[0] + b[0]);
        nz += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let m = (nx * nx + ny * ny + nz * nz).sqrt();
    if m < 1e-12 {
        [0.0; 3]
    } else {
        [nx / m, ny / m, nz / m]
    }
}
