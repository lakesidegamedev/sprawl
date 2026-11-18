//! The weather on screen: what the renderer is told each frame (snow, wet, cloud, wind, flood
//! water, lightning and ground fog).

use super::Game;
use us_render::WeatherLook;

/// Most flood water tiles sent to the renderer.
const MAX_FLOOD_TILES: usize = 900;

/// A cheap hash to 0..1 for particle positions.
fn unit(i: u32, salt: u32) -> f32 {
    let mut h = i
        .wrapping_mul(747_796_405)
        .wrapping_add(salt.wrapping_mul(2_891_336_453));
    h = ((h >> ((h >> 28) + 4)) ^ h).wrapping_mul(277_803_737);
    h = (h >> 22) ^ h;
    (h % 10_000) as f32 / 10_000.0
}

impl Game {
    /// Lightning: now and then a double flash during a storm. Zero otherwise.
    pub(super) fn lightning_flash(&self) -> f32 {
        if !self.sim.storm() {
            return 0.0;
        }
        let t = self.live.scene.time * 0.5;
        let slot = t.floor() as u32;
        if unit(slot, 91) > 0.4 {
            return 0.0;
        }
        let f = t - t.floor();
        let a = (1.0 - f * 9.0).max(0.0);
        let b = (1.0 - (f - 0.2).abs() * 14.0).max(0.0);
        a.max(b) * 0.8
    }

    pub(super) fn weather_look(&self, visual_hour: f32, dt: f32) -> WeatherLook {
        let sim = &self.sim;
        let w = sim.wind();
        let mut flood: Vec<[f32; 3]> = Vec::new();
        for &i in sim.flooded() {
            let l = &sim.lots()[i];
            flood.push([l.pos.x as f32, l.pos.y as f32, (l.radius * 2.3) as f32]);
        }
        let runs = sim.road_runs();
        for &r in sim.flooded_roads() {
            let Some(run) = runs.get(r) else { continue };
            let size = (run.class.half_width() * 2.0 + 5.0) as f32;
            let step = (size * 0.9) as f64;
            for seg in run.points.windows(2) {
                let len = seg[0].dist(seg[1]);
                let n = (len / step).ceil().max(1.0) as usize;
                for k in 0..n {
                    let p = seg[0].lerp(seg[1], (k as f64 + 0.5) / n as f64);
                    flood.push([p.x as f32, p.y as f32, size]);
                }
            }
            if flood.len() >= MAX_FLOOD_TILES {
                break;
            }
        }
        flood.truncate(MAX_FLOOD_TILES);
        let previous = &self.live.scene.weather;
        let blend = |from: f32, target: f32, seconds: f32| {
            if previous.active {
                target + (from - target) * (-dt.max(0.0) / seconds).exp()
            } else {
                target
            }
        };
        WeatherLook {
            active: true,
            snow: blend(previous.snow, sim.snow_cover(), 4.0),
            wet: blend(previous.wet, sim.ground_wet(), 2.4),
            rain: blend(previous.rain, sim.rain(), 1.9),
            snowfall: blend(previous.snowfall, sim.snowfall(), 1.9),
            overcast: blend(previous.overcast, sim.overcast(), 3.5),
            ground_fog: blend(previous.ground_fog, self.ground_fog(visual_hour), 3.0),
            flash: self.lightning_flash(),
            wind: [
                blend(previous.wind[0], w.x as f32, 3.0),
                blend(previous.wind[1], w.y as f32, 3.0),
            ],
            flood,
        }
    }

    /// Damp ground feeds gentle dawn banks. As the ground warms, or strong wind mixes
    /// the air, they dissipate. Persistent wet weather can sustain lighter mist by day.
    pub(super) fn ground_fog(&self, visual_hour: f32) -> f32 {
        let sim = &self.sim;
        let dawn = 1.0 - ((visual_hour - 5.8).abs() / 3.4).clamp(0.0, 1.0);
        let damp = (1.0 - sim.dryness() as f32).clamp(0.0, 1.0);
        let calm = 1.0 - (sim.weather().wind_speed() / 8.0).clamp(0.0, 1.0);
        let thawed = ((sim.temperature_c() + 4.0) / 9.0).clamp(0.0, 1.0);
        let dawn_banks = dawn * dawn * damp.sqrt() * calm * thawed * 1.22;
        let drizzle = sim.rain() * sim.overcast() * calm * 0.42;
        (dawn_banks + drizzle).clamp(0.0, 1.0)
    }
}
