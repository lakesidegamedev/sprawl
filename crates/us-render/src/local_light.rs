//! A bounded, camera-local grid of lights. Binning on the CPU makes light lookup
//! independent of total town size, and leaves the daytime shader path unchanged.

use crate::instances::Instance;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};

const SIDE: usize = 32;
const SLOTS: usize = 6;
const LAMP_SLOTS: usize = 3;
const TILE: f32 = 24.0;
const LAMP_RADIUS: f32 = 19.0;
const HEADLIGHT_RADIUS: f32 = 28.0;

pub(crate) const WIDTH: u32 = (SIDE * SLOTS * 2) as u32;
pub(crate) const HEIGHT: u32 = SIDE as u32;

#[derive(Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
struct Light {
    position_power: [f32; 4],
    direction_radius: [f32; 4],
}

pub(crate) struct LightGrid {
    origin: Vec2,
    lights: Vec<Light>,
    priorities: Vec<f32>,
}

impl Default for LightGrid {
    fn default() -> Self {
        Self {
            origin: Vec2::ZERO,
            lights: vec![Light::default(); SIDE * SIDE * SLOTS],
            priorities: vec![f32::INFINITY; SIDE * SIDE * SLOTS],
        }
    }
}

impl LightGrid {
    pub fn build(&mut self, target: Vec3, lamps: &[Instance], vehicles: &[Instance]) {
        self.origin = (target.truncate() / TILE).floor() - (SIDE / 2) as f32;
        self.lights.fill(Light::default());
        self.priorities.fill(f32::INFINITY);
        for lamp in lamps {
            let arm = Vec3::new(lamp.theta.cos() * 1.4, lamp.theta.sin() * 1.4, 5.75) * lamp.scale;
            self.insert(
                Vec3::from_array(lamp.pos) + arm,
                Vec2::ZERO,
                45.0,
                LAMP_RADIUS,
            );
        }
        for car in vehicles {
            let forward = Vec2::new(car.theta.cos(), car.theta.sin());
            let front = (forward * 2.12).extend(0.7) * car.scale;
            self.insert(
                Vec3::from_array(car.pos) + front,
                forward,
                95.0,
                HEADLIGHT_RADIUS,
            );
        }
    }

    fn insert(&mut self, p: Vec3, heading: Vec2, power: f32, radius: f32) {
        let cell = p.truncate() / TILE - self.origin;
        let reach = radius / TILE;
        let lo = ((cell - reach).floor().as_ivec2()).max(glam::IVec2::ZERO);
        let hi = ((cell + reach).floor().as_ivec2()).min(glam::IVec2::splat(SIDE as i32 - 1));
        let light = Light {
            position_power: p.extend(power).to_array(),
            direction_radius: [heading.x, heading.y, radius, 0.0],
        };
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let base = (y as usize * SIDE + x as usize) * SLOTS;
                let (i, end) = if heading == Vec2::ZERO {
                    (base, base + LAMP_SLOTS)
                } else {
                    (base + LAMP_SLOTS, base + SLOTS)
                };
                let offset = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - cell;
                // Prefer lights near this cell and headlight beams that face it.
                let behind = (offset.dot(heading) < -0.5) as u8 as f32;
                let priority = offset.length_squared() / (reach * reach) + behind * 2.0;
                let lane = self.priorities[i..end].partition_point(|&k| k <= priority);
                if i + lane < end {
                    let slot = i + lane;
                    self.priorities.copy_within(slot..end - 1, slot + 1);
                    self.lights.copy_within(slot..end - 1, slot + 1);
                    self.priorities[slot] = priority;
                    self.lights[slot] = light;
                }
            }
        }
    }

    pub fn origin(&self) -> [f32; 2] {
        self.origin.to_array()
    }

    pub fn bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.lights)
    }
}
