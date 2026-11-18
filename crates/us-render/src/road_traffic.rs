//! Directional traffic drawn from the solver's approach counts. A finite road queue grows
//! when arrivals beat discharge capacity, then drains as demand falls through the day.

use crate::instances::{self, CAR_ID_BASE, ID_STRIDE, Instance, MAX_CARS, Path, tint};
use us_gen::RoadClass;

const SLOTS: usize = 96;
const BUMPER_SPACING: f32 = 6.8;
const MIN_LANE: f32 = 12.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Approach {
    /// Vehicles arriving per hour at the peak of the daily demand curve.
    pub rate: f32,
    /// Vehicles this direction of the approach can serve per hour.
    pub capacity: f32,
    /// Downstream stop position along the road, from 0 to 1.
    pub stop: f32,
}

#[derive(Default)]
pub(crate) struct Traffic {
    lanes: Vec<Lane>,
}

struct Lane {
    path: usize,
    reverse: bool,
    start: f32,
    extent: f32,
    side: f32,
    rate: [f32; 2],
    capacity: f32,
    speed: f32,
    // Calculated only for oversaturated approaches.
    queue: Option<Box<[f32; SLOTS + 1]>>,
}

fn daily_demand(hour: f32) -> [f32; 2] {
    fn peak(hour: f32, center: f32, width: f32) -> f32 {
        let x = (1.0 - (hour - center).abs() / width).max(0.0);
        x * x * (3.0 - 2.0 * x)
    }
    let day = peak(hour, 13.0, 8.0);
    let morning = peak(hour, 8.0, 2.75);
    let evening = peak(hour, 17.0, 2.9);
    let base = (0.12 + 0.32 * day) * (1.0 - morning.max(evening)) / 2.0;
    [base + morning, base + evening]
}

fn queue_profile(rate: [f32; 2], capacity: f32, storage: f32) -> Option<Box<[f32; SLOTS + 1]>> {
    if rate[0].max(rate[1]) <= capacity {
        return None;
    }
    let mut queue = Box::new([0.0; SLOTS + 1]);
    let step = 24.0 / SLOTS as f32;
    let mut waiting = 0.0;
    // Warm up one day so overloaded approaches can still have vehicles at midnight.
    for _ in 0..2 {
        queue[0] = waiting;
        for i in 0..SLOTS {
            let hour = (i as f32 + 0.5) * step;
            let [am, pm] = daily_demand(hour);
            let arrivals = rate[0] * am + rate[1] * pm;
            waiting = (waiting + (arrivals - capacity) * step).clamp(0.0, storage);
            queue[i + 1] = waiting;
        }
    }
    Some(queue)
}

impl Traffic {
    pub(crate) fn new(paths: &[Path], approaches: &[[Approach; 2]]) -> Self {
        let mut lanes = Vec::new();
        for (path, (p, demand)) in paths.iter().zip(approaches).enumerate() {
            if matches!(p.class, RoadClass::Rail | RoadClass::Alley) {
                continue;
            }
            let length = p.length();
            let speed = instances::cruise(p.class);
            let lanes_per_direction = if matches!(p.class, RoadClass::Arterial | RoadClass::Highway)
            {
                2
            } else {
                1
            };
            let width = p.class.half_width() as f32;
            for (direction, a) in demand.iter().enumerate() {
                let opposite = demand[1 - direction].rate;
                if a.rate.max(opposite) <= 0.0 || a.capacity <= 0.0 {
                    continue;
                }
                let reverse = direction == 1;
                let stop = if reverse { 1.0 - a.stop } else { a.stop };
                let start = 3.0;
                let extent = stop * length - width.min(13.0) - start;
                if extent < MIN_LANE {
                    continue;
                }
                let split = lanes_per_direction as f32;
                let rate = [a.rate / split, opposite / split];
                let capacity = a.capacity / split;
                for lane in 0..lanes_per_direction {
                    let side = width * (lane as f32 + 0.5) / split;
                    lanes.push(Lane {
                        path,
                        reverse,
                        start,
                        extent,
                        side: if reverse { side } else { -side },
                        rate,
                        capacity,
                        speed,
                        queue: queue_profile(rate, capacity, (extent / BUMPER_SPACING).floor()),
                    });
                    // Free-flow lanes continue on the far side of an interior junction.
                    let after = stop * length + width.min(13.0);
                    let remainder = length - after - start;
                    if remainder >= MIN_LANE {
                        lanes.push(Lane {
                            path,
                            reverse,
                            start: after,
                            extent: remainder,
                            side: if reverse { side } else { -side },
                            rate: rate.map(|r| r.min(capacity)),
                            capacity,
                            speed,
                            queue: None,
                        });
                    }
                }
            }
        }
        Self { lanes }
    }

    pub(crate) fn instances(&self, paths: &[Path], time: f32, hour: f32) -> Vec<Instance> {
        let hour = hour.rem_euclid(24.0);
        let [am, pm] = daily_demand(hour);
        let sample = hour * SLOTS as f32 / 24.0;
        let i = (sample as usize).min(SLOTS - 1);
        let t = sample - i as f32;
        let mut out = Vec::new();
        // Draw stopped cars first so the shared budget keeps the important bottlenecks.
        for phase in [true, false] {
            for (n, lane) in self.lanes.iter().enumerate() {
                let waiting = lane
                    .queue
                    .as_ref()
                    .map_or(0.0, |q| q[i] + (q[i + 1] - q[i]) * t);
                let span = (lane.extent - waiting * BUMPER_SPACING).max(0.0);
                let path = &paths[lane.path];
                if phase {
                    let count = waiting.round() as usize;
                    let discharge = time * lane.capacity / 3600.0 + (n as f32 * 0.618_034).fract();
                    let tick = discharge as usize;
                    let t = discharge.fract();
                    let advance = t * t * (3.0 - 2.0 * t);
                    for car in 0..count {
                        let d = lane.extent - (car as f32 + 1.0 - advance) * BUMPER_SPACING;
                        if !place(&mut out, path, lane, d, n, car.wrapping_add(tick)) {
                            return out;
                        }
                    }
                } else if span >= MIN_LANE {
                    let speed = if waiting > 0.25 {
                        lane.speed * 0.63
                    } else {
                        lane.speed
                    };
                    let arrival_rate = lane.rate[0] * am + lane.rate[1] * pm;
                    if arrival_rate <= 0.0 {
                        continue;
                    }
                    let spacing = (3600.0 * speed / arrival_rate).max(BUMPER_SPACING);
                    let offset = ((n as f32 * 0.618_034).fract() * spacing + time * speed)
                        .rem_euclid(spacing);
                    let mut d = offset;
                    let mut car = 512;
                    while d + BUMPER_SPACING <= span {
                        if !place(&mut out, path, lane, d, n, car) {
                            return out;
                        }
                        d += spacing;
                        car += 1;
                    }
                }
            }
        }
        out
    }
}

fn place(out: &mut Vec<Instance>, path: &Path, lane: &Lane, d: f32, n: usize, car: usize) -> bool {
    if out.len() == MAX_CARS {
        return false;
    }
    let along = lane.start + d;
    let (s, sign) = if lane.reverse {
        (path.length() - along, -1.0)
    } else {
        (along, 1.0)
    };
    let (pt, dir) = path.at(s);
    let colours = &crate::kit::CAR_COLS;
    let index = (n.wrapping_mul(19) ^ car.wrapping_mul(37)) % colours.len();
    out.push(Instance {
        pos: [
            pt[0] - dir[1] * lane.side,
            pt[1] + dir[0] * lane.side,
            path.z(s),
        ],
        theta: libm::atan2f(dir[1] * sign, dir[0] * sign),
        scale: 1.0,
        tint: tint(colours[index]),
        id: CAR_ID_BASE + out.len() as u32 * ID_STRIDE,
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::pt;

    #[test]
    fn rush_queues_form_only_in_the_loaded_direction_and_clear_after_the_peak() {
        let paths = [Path::new(
            RoadClass::Collector,
            &[pt(0.0, 0.0), pt(300.0, 0.0)],
        )];
        let model = Traffic::new(
            &paths,
            &[[
                Approach {
                    rate: 1600.0,
                    capacity: 900.0,
                    stop: 1.0,
                },
                Approach {
                    rate: 300.0,
                    capacity: 900.0,
                    stop: 0.0,
                },
            ]],
        );
        let q = model.lanes[0].queue.as_ref().unwrap();
        assert!(q[8 * 4] > 15.0);
        assert_eq!(q[13 * 4], 0.0);
        let inbound = model.lanes[1].queue.as_ref().unwrap();
        assert_eq!(inbound[8 * 4], 0.0);
        assert!(inbound[17 * 4] > 15.0);
        assert_eq!(q[17 * 4], 0.0);
        let cars = model.instances(&paths, 0.0, 8.0);
        let queued: Vec<_> = cars.iter().filter(|c| c.pos[1] < 0.0).collect();
        assert!(queued.len() > 15);
        assert!(
            queued
                .windows(2)
                .all(|x| (x[0].pos[0] - x[1].pos[0]).abs() >= 6.0)
        );
        assert!(cars.iter().any(|c| c.pos[1] > 0.0));
    }

    #[test]
    fn queues_stay_upstream_of_the_solvers_stop_point() {
        let paths = [Path::new(
            RoadClass::Collector,
            &[pt(0.0, 0.0), pt(300.0, 0.0)],
        )];
        let model = Traffic::new(
            &paths,
            &[[
                Approach {
                    rate: 1600.0,
                    capacity: 900.0,
                    stop: 0.5,
                },
                Approach::default(),
            ]],
        );
        let cars = model.instances(&paths, 0.0, 8.0);
        assert!(cars.len() > 5);
        let queued = model.lanes[0].queue.as_ref().unwrap()[8 * 4].round() as usize;
        assert!(queued > 5);
        assert!(
            cars[..queued]
                .iter()
                .all(|c| c.pos[0] < 145.0 && c.pos[0] > 3.0)
        );
        assert!(cars[queued..].iter().any(|c| c.pos[0] > 155.0));
    }
}
