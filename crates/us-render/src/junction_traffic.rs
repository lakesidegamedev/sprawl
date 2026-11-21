//! Traffic at paved junctions. Each approach has a protected turn, followed by
//! clearance time. The signal and the car crossing both use that same phase.

use crate::fleet::Prop;
use crate::instances::{CAR_ID_BASE, ID_STRIDE, Instance, MAX_CARS, Path, tint};
use crate::road_traffic::Approach;
use crate::roadpaint::{TrafficArm, TrafficJunction};
use glam::Vec2;

const SLOT: f32 = us_gen::SIGNAL_APPROACH_SECONDS as f32;
const GREEN: f32 = us_gen::SIGNAL_GREEN_SECONDS as f32;
const AMBER: f32 = 1.0;
const ENTRY: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Aspect {
    Green,
    Amber,
    Red,
}

fn phase(age: f32) -> Aspect {
    if (0.0..GREEN).contains(&age) {
        Aspect::Green
    } else if (GREEN..GREEN + AMBER).contains(&age) {
        Aspect::Amber
    } else {
        Aspect::Red
    }
}

fn direction(arm: &TrafficArm, incoming: bool) -> usize {
    usize::from(arm.outward_forward == incoming)
}

fn distance(arm: &TrafficArm, offset: f32) -> f32 {
    arm.s + if arm.outward_forward { offset } else { -offset }
}

fn lane_pose(path: &Path, arm: &TrafficArm, offset: f32, incoming: bool) -> (Vec2, Vec2, f32) {
    let s = distance(arm, offset);
    let (p, forward) = path.at(s);
    let heading = Vec2::from(forward)
        * if direction(arm, incoming) == 0 {
            1.0
        } else {
            -1.0
        };
    let width = path.class.half_width() as f32;
    let lateral = if matches!(path.class, us_gen::RoadClass::Arterial) {
        width * 0.75
    } else {
        width * 0.5
    };
    (Vec2::from(p) - heading.perp() * lateral, heading, path.z(s))
}

pub(crate) fn limits(junctions: &[TrafficJunction], count: usize) -> Vec<Vec<(f32, f32)>> {
    let mut limits = vec![Vec::new(); count];
    for junction in junctions {
        for arm in &junction.arms {
            let edge = distance(arm, arm.stop + ENTRY);
            limits[arm.road].push((arm.s.min(edge), arm.s.max(edge)));
        }
    }
    for road in &mut limits {
        road.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for &(lo, hi) in road.iter() {
            if let Some(last) = merged.last_mut()
                && last.1 >= lo
            {
                last.1 = last.1.max(hi);
            } else {
                merged.push((lo, hi));
            }
        }
        *road = merged;
    }
    limits
}

fn draw_car(cars: &mut Vec<Instance>, p: Vec2, heading: Vec2, z: f32, colour: usize) {
    if cars.len() >= MAX_CARS {
        return;
    }
    cars.push(Instance {
        pos: [p.x, p.y, z],
        theta: libm::atan2f(heading.y, heading.x),
        scale: 1.0,
        tint: tint(crate::kit::CAR_COLS[colour % crate::kit::CAR_COLS.len()]),
        id: CAR_ID_BASE + cars.len() as u32 * ID_STRIDE,
    });
}

fn cross(
    entry: (Vec2, Vec2, f32),
    exit: (Vec2, Vec2, f32),
    reach: [f32; 2],
    t: f32,
) -> (Vec2, Vec2, f32) {
    let (a, incoming, z0) = entry;
    let (d, outgoing, z1) = exit;
    let b = a + incoming * reach[0];
    let c = d - outgoing * reach[1];
    let ab = a.lerp(b, t);
    let bc = b.lerp(c, t);
    let cd = c.lerp(d, t);
    let p = ab.lerp(bc, t);
    let q = bc.lerp(cd, t);
    (p.lerp(q, t), q - p, z0 + (z1 - z0) * t)
}

pub(crate) fn instances(
    junctions: &[TrafficJunction],
    paths: &[Path],
    demand: &[[Approach; 2]],
    time: f32,
    daily: [f32; 2],
) -> (Vec<Instance>, Vec<Instance>) {
    let mut cars = Vec::new();
    let mut signals = Vec::new();
    for (node, junction) in junctions.iter().enumerate() {
        let cycle = junction.arms.len() as f32 * SLOT;
        let now = time + (node as f32 * 2.618).rem_euclid(cycle);
        for (index, arm) in junction.arms.iter().enumerate() {
            let path = &paths[arm.road];
            let start = index as f32 * SLOT;
            let age = (now - start).rem_euclid(cycle);
            let aspect = phase(age);
            let entry = lane_pose(path, arm, arm.stop, true);
            if junction.signalized {
                let outward = -entry.1;
                let s = distance(arm, arm.stop);
                let (p, _) = path.at(s);
                let pos = Vec2::from(p) + outward.perp() * (path.class.half_width() as f32 + 1.3);
                let colour = match aspect {
                    Aspect::Red => [250, 55, 48],
                    Aspect::Amber => [255, 186, 46],
                    Aspect::Green => [45, 234, 120],
                };
                signals.push(Instance {
                    pos: [pos.x, pos.y, path.z(s)],
                    theta: libm::atan2f(outward.y, outward.x) + std::f32::consts::PI,
                    scale: 1.0,
                    tint: tint(colour),
                    id: crate::fleet::PROP_ID_BASE
                        + Prop::TrafficLight.index() as u32 * 200_000
                        + signals.len() as u32 * ID_STRIDE,
                });
            }
            let flows = demand.get(arm.road).copied().unwrap_or_default();
            let dir = direction(arm, true);
            let rate = flows[dir].rate * daily[0] + flows[1 - dir].rate * daily[1];
            if rate <= 0.0 || flows[dir].capacity <= 0.0 {
                continue;
            }
            let leaving = age < GREEN;
            let lap = ((now - start) / cycle).floor() as i32;
            let batch = if leaving { lap } else { lap + 1 };
            let hash = (batch as u32).wrapping_mul(0x9e3779b9)
                ^ (node as u32).wrapping_mul(0x85ebca6b)
                ^ (index as u32).wrapping_mul(0xc2b2ae35);
            let chance = (rate * cycle / 3600.0).min(1.0);
            if ((hash >> 8) as f32 / 16_777_216.0) >= chance {
                continue;
            }
            let Some((exit, exit_stop)) = junction
                .arms
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .filter_map(|(i, arm)| {
                    let flows = demand.get(arm.road)?;
                    let out = flows[direction(arm, false)];
                    if out.capacity <= 0.0 {
                        return None;
                    }
                    let pose = lane_pose(&paths[arm.road], arm, arm.stop + ENTRY, false);
                    let variation = ((hash.rotate_left((i * 7 % 32) as u32) & 255) as f32) / 255.0;
                    let score = (1.0 + out.rate) * (1.7 + entry.1.dot(pose.1)) * (0.4 + variation);
                    Some((score, (pose, arm.stop + ENTRY)))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, pose)| pose)
            else {
                continue;
            };
            let colour = hash as usize;
            if leaving {
                // The last vehicle clears the junction before the next approach gets green.
                let (p, heading, z) =
                    cross(entry, exit, [arm.stop * 0.9, exit_stop * 0.9], age / GREEN);
                draw_car(&mut cars, p, heading, z, colour);
            } else if age >= GREEN + AMBER {
                let arrive = ((age - (GREEN + AMBER)) / 2.0).clamp(0.0, 1.0);
                let offset = arm.stop + ENTRY * (1.0 - arrive).powi(2);
                let (p, heading, z) = lane_pose(path, arm, offset, true);
                draw_car(&mut cars, p, heading, z, colour);
            }
        }
    }
    (cars, signals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Land;
    use crate::live::RoadDraw;
    use us_gen::{RoadClass, pt};

    #[test]
    fn a_car_waits_for_green_and_turns_into_the_connected_street() {
        let centre = pt(150.0, 150.0);
        let roads = [
            RoadDraw {
                class: RoadClass::Arterial,
                points: &[pt(50.0, 150.0), centre],
                works: None,
            },
            RoadDraw {
                class: RoadClass::Arterial,
                points: &[centre, pt(250.0, 150.0)],
                works: None,
            },
            RoadDraw {
                class: RoadClass::Local,
                points: &[centre, pt(190.0, 250.0)],
                works: None,
            },
        ];
        let paths: Vec<_> = roads
            .iter()
            .map(|road| Path::new(road.class, road.points))
            .collect();
        let junctions = crate::roadpaint::traffic_junctions(&roads, &paths, &Land::flat(300.0));
        assert_eq!(junctions.len(), 1);
        let junction = &junctions[0];
        assert!(junction.signalized);
        let entry = junction.arms.iter().position(|arm| arm.road == 0).unwrap();
        let exit = junction.arms.iter().find(|arm| arm.road == 2).unwrap();
        let flows = [
            [
                Approach {
                    rate: 1500.0,
                    capacity: 430.0,
                    stop: 1.0,
                },
                Approach::default(),
            ],
            [Approach::default(); 2],
            [
                Approach {
                    rate: 1500.0,
                    capacity: 900.0,
                    stop: 1.0,
                },
                Approach::default(),
            ],
        ];
        let t = entry as f32 * SLOT;
        let sample = |time| instances(&junctions, &paths, &flows, time, [1.0, 0.0]);
        let (red, lamps) = sample(t - 1.0);
        assert_eq!(lamps[entry].tint, tint([250, 55, 48]));
        let wait = lane_pose(
            &paths[0],
            &junction.arms[entry],
            junction.arms[entry].stop,
            true,
        )
        .0;
        let near =
            |car: &Instance, p: Vec2| Vec2::from_array([car.pos[0], car.pos[1]]).distance(p) < 1.0;
        assert!(red.iter().any(|car| near(car, wait)));
        let (green, lamps) = sample(t + 0.01);
        assert_eq!(lamps[entry].tint, tint([45, 234, 120]));
        let first = green.iter().find(|car| near(car, wait)).unwrap();
        let (turned, _) = sample(t + GREEN - 0.01);
        let destination = lane_pose(&paths[2], exit, exit.stop + ENTRY, false).0;
        assert!(
            turned
                .iter()
                .any(|car| car.tint == first.tint && near(car, destination))
        );
        let (_, lamps) = sample(t + GREEN + 0.1);
        assert_eq!(lamps[entry].tint, tint([255, 186, 46]));
        assert!(lamps.iter().all(|lamp| lamp.tint != tint([45, 234, 120])));
    }
}
