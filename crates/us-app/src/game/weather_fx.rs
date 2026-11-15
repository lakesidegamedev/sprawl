//! The weather on screen: what the renderer is told each frame (snow, wet, cloud, wind, flood
//! water, lightning) and the rain and snow that fall in front of the map.

use super::Game;
use egui::{Color32, Context, Pos2, Stroke, vec2};
use us_render::WeatherLook;
use us_sim::Sky;

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

    pub(super) fn weather_look(&self) -> WeatherLook {
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
        WeatherLook {
            active: true,
            snow: sim.snow_cover(),
            wet: sim.ground_wet(),
            overcast: sim.overcast(),
            flash: self.lightning_flash(),
            wind: [w.x as f32, w.y as f32],
            flood,
        }
    }

    /// Rain streaks and snowflakes over the map, thicker as the weather gets harder.
    pub(super) fn paint_weather(&self, ctx: &Context, map: egui::Rect) {
        let rain = self.sim.rain();
        let snow = self.sim.snowfall();
        if rain <= 0.0 && snow <= 0.0 {
            return;
        }
        // only over the map: the panels round it stay dry
        let rect = map.intersect(ctx.content_rect());
        let (w, h) = (rect.width(), rect.height());
        if w < 10.0 || h < 10.0 {
            return;
        }
        let painter = ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Background,
                egui::Id::new("weather-fx"),
            ))
            .with_clip_rect(rect);
        let t = self.live.scene.time;
        let wind = self.sim.wind();
        // wind leans the fall sideways on screen
        let lean = (wind.x as f32 * 0.06).clamp(-0.8, 0.8);
        if rain > 0.0 {
            let n = (160.0 + 900.0 * rain) as u32;
            let col = Color32::from_rgba_unmultiplied(188, 206, 236, 150);
            let stroke = Stroke::new(1.2, col);
            let storm = self.sim.sky_now() == Sky::Storm;
            let speed = if storm { 1.9 } else { 1.4 };
            for i in 0..n {
                let x0 = unit(i, 1) * (w + 200.0) - 100.0;
                let y =
                    (unit(i, 2) + t * speed * (0.8 + 0.4 * unit(i, 3))).fract() * (h + 40.0) - 20.0;
                let x = x0 + lean * y;
                let len = 10.0 + 12.0 * unit(i, 4);
                let a = Pos2::new(rect.left() + x, rect.top() + y);
                painter.line_segment([a, a + vec2(lean * len, len)], stroke);
            }
        }
        if snow > 0.0 {
            let n = (120.0 + 700.0 * snow) as u32;
            let col = Color32::from_rgba_unmultiplied(250, 252, 255, 220);
            for i in 0..n {
                let fall = 0.16 + 0.12 * unit(i, 3);
                let y = (unit(i, 2) + t * fall).fract() * (h + 20.0) - 10.0;
                let sway =
                    (t * (0.8 + unit(i, 5)) + unit(i, 6) * std::f32::consts::TAU).sin() * 14.0;
                let x = unit(i, 1) * (w + 200.0) - 100.0 + lean * y * 0.6 + sway;
                let r = 1.0 + 1.6 * unit(i, 4);
                painter.circle_filled(Pos2::new(rect.left() + x, rect.top() + y), r, col);
            }
        }
    }
}
