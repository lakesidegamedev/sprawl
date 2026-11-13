//! Vertex and mesh types shared by the CPU builders and the GPU renderer.

use crate::obj::{Rgb, V3, newell};
use bytemuck::{Pod, Zeroable};

/// Outline id of ground: nothing draws an ink line between ground and ground.
pub const ID_GROUND: u32 = 0;
/// Outline id of a face that takes part in no outline (parking lines, paths).
pub const ID_NONE: u32 = 0x00FF_FFFF;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// rgb colour; alpha is 255 for a cel-toned face and 0 for a flat one.
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

    /// Add a convex polygon (fan triangulated). Degenerate polygons are dropped.
    pub fn push_poly(&mut self, pts: &[V3], color: Rgb, flat: bool, cast: bool, id: u32) {
        if pts.len() < 3 {
            return;
        }
        let n = newell(pts);
        if n == [0.0; 3] {
            return;
        }
        let base = self.vertices.len() as u32;
        let alpha = if flat { 0 } else { 255 };
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
