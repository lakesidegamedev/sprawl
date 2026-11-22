//! Vertex and mesh types shared by the CPU builders and the GPU renderer.

use crate::obj::{Finish, Rgb, V3, newell};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Outline id of ground: nothing draws an ink line between ground and ground.
pub const ID_GROUND: u32 = 0;
/// Outline id of a face that takes part in no outline (parking lines, paths).
pub const ID_NONE: u32 = 0x00FF_FFFF;
/// Base heightfield material, distinguished from roads, rivers and zoned paving.
pub(crate) const TERRAIN_SURFACE: u8 = 253;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// RGB plus a finish code. Ordinary cel and flat faces use 255 and 0.
    pub color: [u8; 4],
    pub id: u32,
}

pub const VERTEX_BYTES: usize = std::mem::size_of::<Vertex>();

/// Triangles in two lists sharing one vertex list: `cast` throws shadows, `plain` does not.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub cast: Vec<u32>,
    pub plain: Vec<u32>,
}

impl Mesh {
    pub fn triangles(&self) -> usize {
        (self.cast.len() + self.plain.len()) / 3
    }

    /// Exact world-space bounds, including any tower, antenna or crane.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let first = Vec3::from(self.vertices.first()?.pos);
        Some(
            self.vertices
                .iter()
                .skip(1)
                .fold((first, first), |(lo, hi), v| {
                    let p = Vec3::from(v.pos);
                    (lo.min(p), hi.max(p))
                }),
        )
    }

    /// Round the foliage added since vertex `from`: leaf-coloured vertices that share a place
    /// take the average of their faces' normals, so a crown built from flat faces is lit as one
    /// rounded mass instead of facet by facet.
    pub fn round_foliage(&mut self, from: usize) {
        fn leafy(c: [u8; 4]) -> bool {
            let near = |r: u8, g: u8, b: u8| c[0] == r && c[1] == g && c[2] >= b && c[2] <= b + 3;
            c[3] == 255
                && (near(88, 150, 76)
                    || near(70, 128, 66)
                    || near(100, 168, 58)
                    || near(46, 104, 72))
        }
        let mut sums: std::collections::BTreeMap<[i32; 3], Vec3> =
            std::collections::BTreeMap::new();
        let key = |p: [f32; 3]| {
            [
                (p[0] * 64.0).round() as i32,
                (p[1] * 64.0).round() as i32,
                (p[2] * 64.0).round() as i32,
            ]
        };
        for v in &self.vertices[from.min(self.vertices.len())..] {
            if leafy(v.color) {
                *sums.entry(key(v.pos)).or_default() += Vec3::from(v.normal);
            }
        }
        if sums.is_empty() {
            return;
        }
        for v in &mut self.vertices[from..] {
            if leafy(v.color)
                && let Some(n) = sums.get(&key(v.pos))
            {
                let n = n.normalize_or_zero();
                if n != Vec3::ZERO {
                    v.normal = n.to_array();
                }
            }
        }
    }

    /// Add a convex polygon (fan triangulated). Degenerate polygons are dropped.
    pub fn push_poly(&mut self, pts: &[V3], color: Rgb, flat: bool, cast: bool, id: u32) {
        self.push_finished_poly(pts, color, flat, cast, id, Finish::Plain);
    }

    pub(crate) fn push_finished_poly(
        &mut self,
        pts: &[V3],
        color: Rgb,
        flat: bool,
        cast: bool,
        id: u32,
        finish: Finish,
    ) {
        if pts.len() < 3 {
            return;
        }
        let n = newell(pts);
        if n == [0.0; 3] {
            return;
        }
        let base = self.vertices.len() as u32;
        let alpha = match (flat, finish) {
            (true, _) => 0,
            (false, Finish::Plain) => 255,
            (false, finish) => finish as u8,
        };
        for p in pts {
            self.vertices.push(Vertex {
                pos: *p,
                normal: n,
                color: [color[0], color[1], color[2], alpha],
                id,
            });
        }
        let list = if cast {
            &mut self.cast
        } else {
            &mut self.plain
        };
        for i in 1..pts.len() as u32 - 1 {
            list.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }

    /// FNV-1a over every vertex byte and index: a stable fingerprint of the geometry.
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for b in bytes {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        eat(bytemuck::cast_slice(&self.vertices));
        eat(bytemuck::cast_slice(&self.cast));
        eat(bytemuck::cast_slice(&self.plain));
        h
    }
}
