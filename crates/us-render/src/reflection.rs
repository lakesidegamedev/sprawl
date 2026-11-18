//! Low-frequency radiance probes for surfaces that reflect outside the camera's view.
use crate::scene::Scene;
use glam::Vec3;

pub const GRID: u32 = 16;
pub const DIRECTIONS: u32 = 6;
const COVER: usize = 96;

#[derive(Clone, Copy, Default)]
struct Cell {
    height: f32,
    colour: [u8; 3],
}

pub fn bake(scene: &Scene) -> Vec<u8> {
    let size = scene.size.max(1.0);
    let cell_size = size / COVER as f32;
    let find = |x: f32, y: f32| {
        let i = (x / cell_size).clamp(0.0, COVER as f32 - 1.0) as usize;
        let j = (y / cell_size).clamp(0.0, COVER as f32 - 1.0) as usize;
        j * COVER + i
    };
    let mut cells = vec![Cell::default(); COVER * COVER];
    for chunk in &scene.chunks {
        let mesh = chunk.far.as_ref().unwrap_or(&chunk.mesh);
        for v in &mesh.vertices {
            let [x, y, z] = v.pos;
            let cell = &mut cells[find(x, y)];
            if z > cell.height && z > scene.land.height(x, y) + 3.5 {
                cell.height = z;
                cell.colour.copy_from_slice(&v.color[..3]);
            }
        }
    }
    let normals = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
    let mut data = Vec::with_capacity((GRID * GRID * DIRECTIONS * 4) as usize);
    let sample_step = cell_size.clamp(4.0, 20.0);
    for j in 0..GRID {
        for i in 0..GRID {
            let x = (i as f32 + 0.5) * size / GRID as f32;
            let y = (j as f32 + 0.5) * size / GRID as f32;
            let origin = Vec3::new(x, y, scene.land.height(x, y) + 15.0);
            for normal in normals {
                let right = if normal.z.abs() > 0.5 {
                    Vec3::X
                } else {
                    Vec3::Z.cross(normal)
                };
                let up = normal.cross(right);
                let mut hits = 0;
                let mut sum = [0u32; 3];
                for u in [-0.58, 0.0, 0.58] {
                    for v in [-0.58, 0.0, 0.58] {
                        let ray = (normal + right * u + up * v).normalize();
                        for step in 1..=18 {
                            let p = origin + ray * sample_step * step as f32;
                            if p.x < 0.0 || p.y < 0.0 || p.x >= size || p.y >= size {
                                break;
                            }
                            let cell = cells[find(p.x, p.y)];
                            let ground = scene.land.height(p.x, p.y);
                            let colour = if p.z < ground {
                                scene.base
                            } else if p.z < cell.height && cell.height > ground + 3.0 {
                                cell.colour
                            } else {
                                continue;
                            };
                            for (acc, component) in sum.iter_mut().zip(colour) {
                                *acc += u32::from(component);
                            }
                            hits += 1;
                            break;
                        }
                    }
                }
                data.extend(sum.map(|component| (component / hits.max(1)) as u8));
                data.push((hits * 255 / 9) as u8);
            }
        }
    }
    data
}
