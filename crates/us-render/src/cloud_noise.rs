//! Tileable density and erosion fields for the cloud ray marcher.

use glam::{IVec3, Vec3};

pub(crate) const SIZE: u32 = 48;

fn hash(cell: IVec3, salt: u32) -> f32 {
    let mut h = (cell.x as u32).wrapping_mul(73_856_093)
        ^ (cell.y as u32).wrapping_mul(19_349_663)
        ^ (cell.z as u32).wrapping_mul(83_492_791)
        ^ salt;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    ((h ^ (h >> 16)) & 65535) as f32 / 65535.0
}

pub(crate) fn bake() -> Vec<u8> {
    const CELLS: i32 = 5;
    let cell_index = |p: IVec3| {
        let p = p.rem_euclid(IVec3::splat(CELLS));
        (p.x + CELLS * (p.y + CELLS * p.z)) as usize
    };
    let mut offsets = [Vec3::ZERO; (CELLS * CELLS * CELLS) as usize];
    for z in 0..CELLS {
        for y in 0..CELLS {
            for x in 0..CELLS {
                let p = IVec3::new(x, y, z);
                offsets[cell_index(p)] =
                    Vec3::new(hash(p, 7), hash(p, 71), hash(p, 713)) * 0.73 + 0.135;
            }
        }
    }
    let mut bytes = Vec::with_capacity((2 * SIZE.pow(3)) as usize);
    for z in 0..SIZE {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let p =
                    (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * (CELLS as f32 / SIZE as f32);
                let cell = p.floor().as_ivec3();
                let mut closest = f32::INFINITY;
                let mut next = f32::INFINITY;
                for dz in -1..=1 {
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let c = cell + IVec3::new(dx, dy, dz);
                            let distance = p.distance_squared(c.as_vec3() + offsets[cell_index(c)]);
                            next = next.min(closest.max(distance));
                            closest = closest.min(distance);
                        }
                    }
                }
                let billow = 1.0 - (closest.sqrt() * 0.94).clamp(0.0, 1.0);
                let folds = ((next.sqrt() - closest.sqrt()) * 1.7).clamp(0.0, 1.0);
                bytes.extend([
                    (billow * 255.0).round() as u8,
                    (folds * 255.0).round() as u8,
                ]);
            }
        }
    }
    bytes
}
