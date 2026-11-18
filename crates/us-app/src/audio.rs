//! Offline music and camera-relative town sound. The output device is optional;
//! a missing or interrupted device must never prevent the town from running.

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::Duration;

const DAY: &[u8] = include_bytes!("../assets/music/morning-neighborhoods.ogg");
const DUSK: &[u8] = include_bytes!("../assets/music/last-tram-home.ogg");
const RATE: u32 = 24_000;

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn urban_sprawl_ios_prepare_audio();
    fn urban_sprawl_ios_pause_audio();
    fn urban_sprawl_ios_audio_output_changed() -> bool;
}

#[cfg(target_os = "ios")]
pub(crate) fn pause_platform_audio() {
    // Called after releasing the Rust output stream on the application thread.
    unsafe { urban_sprawl_ios_pause_audio() }
}

#[cfg(target_os = "ios")]
pub(crate) fn platform_output_changed() -> bool {
    // The platform increments this flag from notification callbacks.
    unsafe { urban_sprawl_ios_audio_output_changed() }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Preferences {
    pub music: f32,
    pub ambience: f32,
    pub effects: f32,
    pub muted: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            music: 0.56,
            ambience: 0.65,
            effects: 0.65,
            muted: false,
        }
    }
}

impl Preferences {
    pub fn load() -> Self {
        let file = crate::saves::save_dir().join("audio-prefs.txt");
        std::fs::read_to_string(file).map_or_else(|_| Self::default(), |s| Self::parse(&s))
    }

    fn parse(text: &str) -> Self {
        let mut result = Self::default();
        for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
            if key == "muted" {
                result.muted = value == "true";
                continue;
            }
            let Ok(value) = value.parse::<f32>() else {
                continue;
            };
            if !value.is_finite() {
                continue;
            }
            let v = value.clamp(0., 1.);
            match key {
                "music" => result.music = v,
                "ambience" => result.ambience = v,
                "effects" => result.effects = v,
                _ => {}
            }
        }
        result
    }

    pub fn save(self) -> std::io::Result<()> {
        let dir = crate::saves::save_dir();
        std::fs::create_dir_all(&dir)?;
        let staging = dir.join("audio-prefs.partial");
        std::fs::write(
            &staging,
            format!(
                "music={}\nambience={}\neffects={}\nmuted={}\n",
                self.music, self.ambience, self.effects, self.muted
            ),
        )?;
        std::fs::rename(staging, dir.join("audio-prefs.txt"))
    }

    pub fn audible(self) -> bool {
        !self.muted && self.music.max(self.ambience).max(self.effects) > 0.0
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SoundScene {
    pub night: f32,
    pub wind: f32,
    pub rain: f32,
    pub traffic: f32,
    pub traffic_pan: f32,
    pub built: Option<usize>,
}

#[derive(Default)]
struct Levels {
    wind: AtomicU32,
    rain: AtomicU32,
    traffic: AtomicU32,
    pan: AtomicU32,
}

pub(crate) struct Audio {
    _stream: OutputStream,
    music: [Sink; 2],
    air: Sink,
    effects: Sink,
    levels: Arc<Levels>,
    fade: [f32; 2],
    previous: Option<usize>,
    last_effect: f32,
    scene: SoundScene,
    next_scene: f32,
}

impl Audio {
    pub fn open() -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "ios")]
        // Opening the audio unit belongs on the foreground application thread.
        unsafe {
            urban_sprawl_ios_prepare_audio();
        }
        let mut stream = OutputStreamBuilder::open_default_stream()?;
        stream.log_on_drop(false);
        let mix = stream.mixer();
        let music = [Sink::connect_new(mix), Sink::connect_new(mix)];
        let air = Sink::connect_new(mix);
        let effects = Sink::connect_new(mix);
        for (player, track) in music.iter().zip([DAY, DUSK]) {
            player.set_volume(0.0);
            player.append(Decoder::new_looped(Cursor::new(track))?);
        }
        air.set_volume(0.0);
        let levels = Arc::new(Levels::default());
        air.append(TownSound::new(levels.clone()));
        Ok(Self {
            _stream: stream,
            music,
            air,
            effects,
            levels,
            fade: [0.0; 2],
            previous: None,
            last_effect: 0.,
            scene: SoundScene::default(),
            next_scene: 0.,
        })
    }

    pub fn tick(&mut self, dt: f32, prefs: Preferences, read_scene: impl FnOnce() -> SoundScene) {
        self.next_scene -= dt;
        if self.next_scene <= 0.0 {
            self.scene = if prefs.audible() {
                read_scene()
            } else {
                SoundScene::default()
            };
            self.next_scene = 0.22;
        }
        let scene = self.scene;
        let dusk = (scene.night * 1.1).clamp(0.0, 1.0);
        let enabled = f32::from(!prefs.muted);
        let ease = 1.0 - (-dt * 0.85).exp();
        for ((volume, sink), weight) in self
            .fade
            .iter_mut()
            .zip(&self.music)
            .zip([1.0 - dusk, dusk])
        {
            if prefs.muted {
                *volume = 0.0;
            } else {
                *volume += (weight.sqrt() * prefs.music - *volume) * ease;
            }
            sink.set_volume(*volume);
        }
        self.air.set_volume(prefs.ambience * enabled);
        self.effects.set_volume(prefs.effects * enabled);
        self.levels.wind.store(scene.wind.to_bits(), Relaxed);
        self.levels.rain.store(scene.rain.to_bits(), Relaxed);
        self.levels.traffic.store(scene.traffic.to_bits(), Relaxed);
        self.levels.pan.store(scene.traffic_pan.to_bits(), Relaxed);
        self.last_effect -= dt;
        if let (Some(old), Some(new)) = (self.previous, scene.built)
            && new > old
            && self.last_effect <= 0.0
            && prefs.effects * enabled > 0.0
        {
            self.effects.append(placed_sound());
            self.last_effect = 0.85;
        }
        self.previous = scene.built;
    }
}

/// Three continuous bed sounds, with independently smoothed environmental targets.
/// The callback allocates nothing and never locks the simulation.
struct TownSound {
    levels: Arc<Levels>,
    level: [f32; 4],
    seed: u32,
    filters: [f32; 3],
    sample: u32,
    right: Option<f32>,
}

impl TownSound {
    fn new(levels: Arc<Levels>) -> Self {
        Self {
            levels,
            level: [0.0; 4],
            seed: 21876,
            filters: [0.0; 3],
            sample: 0,
            right: None,
        }
    }

    fn noise(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 * (2.0 / u32::MAX as f32) - 1.0
    }
}

impl Iterator for TownSound {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let targets = [
            &self.levels.wind,
            &self.levels.rain,
            &self.levels.traffic,
            &self.levels.pan,
        ];
        for (x, target) in self.level.iter_mut().zip(targets) {
            *x += (f32::from_bits(target.load(Relaxed)) - *x) * 0.00045;
        }
        let white = self.noise();
        for (state, a) in self.filters.iter_mut().zip([0.003, 0.052, 0.28]) {
            *state += a * (white - *state);
        }
        let [wind, rain, traffic, pan] = self.level;
        // Broad air, fine droplets and a low wash of engines. No individual car
        // consumes an audio voice, even on a busy road.
        let air = self.filters[1] * wind * 0.18 + (white - self.filters[2]) * rain * 0.10;
        let t = self.sample as f32 / RATE as f32;
        self.sample = (self.sample + 1) % (RATE * 120);
        let ripple = (std::f32::consts::TAU * t * 0.4).sin();
        let road =
            (self.filters[0] * 2.6 + self.filters[1] * 0.35) * traffic * (0.55 + ripple * 0.16);
        self.right = Some(air + road * (0.75 + 0.25 * pan));
        Some(air + road * (0.75 - 0.25 * pan))
    }
}

impl Source for TownSound {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

fn placed_sound() -> rodio::buffer::SamplesBuffer {
    let samples = (0..RATE / 4)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let fade_in = (t * 68.0).min(1.0);
            let phase = std::f32::consts::TAU * t;
            ((phase * 392.0).sin() * 0.07 * (-t * 15.0).exp()
                + (phase * 587.33).sin() * 0.035 * (-t * 21.0).exp())
                * fade_in
        })
        .collect::<Vec<_>>();
    rodio::buffer::SamplesBuffer::new(1, RATE, samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_is_stereo_and_loops_without_a_device() {
        for track in [DAY, DUSK] {
            let decoded = Decoder::new(Cursor::new(track)).unwrap();
            assert_eq!(decoded.channels(), 2);
            assert!(
                decoded
                    .take(RATE as usize * 3)
                    .any(|sample| sample.abs() > 0.05)
            );
            assert!(Decoder::new_looped(Cursor::new(track)).is_ok());
        }
    }

    #[test]
    fn preferences_clamp_old_or_invalid_values() {
        let prefs = Preferences::parse("music=1.4\nambience=-2\neffects=NaN\nmuted=true");
        assert_eq!(prefs.music, 1.0);
        assert_eq!(prefs.ambience, 0.0);
        assert_eq!(prefs.effects, Preferences::default().effects);
        assert!(!prefs.audible());
    }
}
