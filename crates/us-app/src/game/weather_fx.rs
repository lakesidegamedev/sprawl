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
    pub(super) fn storm_cue(&self, storm: f32) -> Option<(u32, [f32; 2], f32)> {
        if storm < 0.28 {
            return None;
        }
        let time = self.live.scene.time / 7.0;
        let slot = time.floor() as u32;
        if unit(slot, 91) > 0.59 {
            return None;
        }
        let elapsed = (time.fract() - 0.14 - unit(slot, 35) * 0.65) * 7.0;
        if !(0.0..1.3).contains(&elapsed) {
            return None;
        }
        let hall = self.sim.town().hall;
        let [x, y] = self.sim.lightning_strikes().first().map_or_else(
            || {
                [
                    hall.x as f32 + (unit(slot, 11) - 0.5) * 850.0,
                    hall.y as f32 + (unit(slot, 67) - 0.5) * 850.0,
                ]
            },
            |&i| {
                let pos = self.sim.lots()[i].pos;
                [pos.x as f32, pos.y as f32]
            },
        );
        Some((slot, [x, y], elapsed))
    }

    pub(super) fn weather_look(
        &self,
        visual_hour: f32,
        dt: f32,
        reduce_motion: bool,
    ) -> WeatherLook {
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
        let storm = blend(previous.storm, f32::from(sim.storm()), 4.0);
        let wind = [
            blend(previous.wind[0], w.x as f32, 3.0),
            blend(previous.wind[1], w.y as f32, 3.0),
        ];
        let gust =
            (self.live.scene.time * 0.44).sin() * 0.21 + (self.live.scene.time * 1.23).sin() * 0.10;
        let gust_wind = wind.map(|speed| speed * (1.0 + storm * gust));
        let drift = std::array::from_fn(|i| previous.drift[i] + gust_wind[i] * dt * 0.24);
        let lead = if sim.weather_tomorrow().sky == us_sim::Sky::Storm {
            ((visual_hour - 14.0) / 10.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let lightning = if reduce_motion {
            [0.0; 3]
        } else {
            self.storm_cue(storm)
                .map_or([0.0; 3], |(_, [x, y], elapsed)| {
                    let flash = (-elapsed * 10.0).exp() * (elapsed / 0.045).min(1.0);
                    [x, y, flash * storm]
                })
        };
        WeatherLook {
            active: true,
            snow: blend(previous.snow, sim.snow_cover(), 4.0),
            wet: blend(
                previous.wet,
                sim.ground_wet(),
                if sim.rain() > 0.1 { 2.8 } else { 11.0 },
            ),
            rain: blend(previous.rain, sim.rain(), 2.3),
            snowfall: blend(previous.snowfall, sim.snowfall(), 2.5),
            overcast: blend(previous.overcast, sim.overcast().max(lead * 0.54), 8.0),
            ground_fog: blend(previous.ground_fog, self.ground_fog(visual_hour), 3.0),
            storm,
            lightning,
            drift,
            wind,
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
