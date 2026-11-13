use std::collections::BTreeSet;
use us_gen::{Preset, RoadClass, generate};
use us_render::kit::ALL_KINDS;
use us_render::overlay::{OVERLAY_ID_BASE, build_mesh};
use us_render::{
    Calendar, DayPhase, LotInfo, LotState, MAX_LEVEL, Overlay, OverlayKind, Rng, Scene, Season,
    build_lot_level, use_category,
};

fn town() -> us_gen::Town {
    generate(Preset::Grid, 1)
}

#[test]
fn overlay_has_one_block_per_lot() {
    let t = town();
    let lots: Vec<LotInfo> = t
        .lots
        .iter()
        .map(|l| LotInfo {
            x: l.centre.x as f32,
            y: l.centre.y as f32,
            theta: l.facing as f32,
            kind: l.kind,
            level: 2,
        })
        .collect();
    let ov = Overlay {
        kind: OverlayKind::Use,
        lots: t.lots.iter().map(|l| use_category(l.kind) as f32).collect(),
        roads: vec![],
    };
    let m = build_mesh(&lots, &[], None, &[], &ov);
    let blocks: BTreeSet<u32> = m
        .vertices
        .iter()
        .filter(|v| v.id >= OVERLAY_ID_BASE)
        .map(|v| (v.id - OVERLAY_ID_BASE) / 16)
        .collect();
    assert_eq!(blocks.len(), t.lots.len());
    // a shorter value list leaves the rest out
    let half = Overlay {
        lots: ov.lots[..10].to_vec(),
        ..ov.clone()
    };
    let m = build_mesh(&lots, &[], None, &[], &half);
    let blocks: BTreeSet<u32> = m
        .vertices
        .iter()
        .filter(|v| v.id >= OVERLAY_ID_BASE)
        .map(|v| (v.id - OVERLAY_ID_BASE) / 16)
        .collect();
    assert_eq!(blocks.len(), 10);
}

#[test]
fn overlay_colours_are_stepped_palette_colours() {
    for kind in [
        OverlayKind::LandValue,
        OverlayKind::BuildingLevel,
        OverlayKind::Use,
        OverlayKind::ServiceCoverage,
        OverlayKind::Demand,
        OverlayKind::Traffic,
    ] {
        let p = kind.palette();
        let mut seen = BTreeSet::new();
        for i in 0..=100 {
            let c = kind.colour(i as f32 / 100.0);
            assert!(p.contains(&c));
            seen.insert(c);
        }
        if kind != OverlayKind::Use {
            assert_eq!(seen.len(), p.len(), "{kind:?} uses every step");
        }
        // values are clamped
        assert_eq!(kind.colour(-3.0), p[0]);
    }
}

#[test]
fn every_kind_has_every_level() {
    for &k in &ALL_KINDS {
        let mut prints = Vec::new();
        for level in 1..=MAX_LEVEL {
            let o = build_lot_level(k, level, &mut Rng::new(5, 5));
            assert!(!o.polys.is_empty(), "{k:?} level {level}");
            let mut m = us_render::Mesh::default();
            let mut id = 1;
            o.emit(&mut m, &mut id, 0.0, 0.0, 0.0, 1.0);
            prints.push(m.fingerprint());
            // same inputs, same model
            let o2 = build_lot_level(k, level, &mut Rng::new(5, 5));
            let mut m2 = us_render::Mesh::default();
            let mut id2 = 1;
            o2.emit(&mut m2, &mut id2, 0.0, 0.0, 0.0, 1.0);
            assert_eq!(m.fingerprint(), m2.fingerprint());
        }
        let distinct: BTreeSet<_> = prints.iter().collect();
        assert_eq!(distinct.len(), 5, "{k:?}: five different models");
    }
}

#[test]
fn buildings_grow_with_level() {
    use us_gen::LotKind::{Apartments, Office};
    for k in [Apartments, Office] {
        let mut last = 0.0;
        for level in 1..=MAX_LEVEL {
            let o = build_lot_level(k, level, &mut Rng::new(3, 3));
            let top = us_render::levels::top_of(&o);
            assert!(top > last, "{k:?} level {level} is taller");
            last = top;
        }
    }
}

#[test]
fn level_change_rebuilds_only_its_chunk() {
    let t = town();
    let mut s = Scene::from_town(&t);
    let revs: Vec<u64> = s.chunks.iter().map(|c| c.rev).collect();
    let before = s
        .chunks
        .iter()
        .map(|c| c.mesh.fingerprint())
        .collect::<Vec<_>>();
    s.set_lot(7, Some(4), LotState::Normal);
    let changed: Vec<usize> = (0..s.chunks.len())
        .filter(|&i| s.chunks[i].rev != revs[i])
        .collect();
    assert_eq!(changed.len(), 1);
    assert_ne!(s.chunks[changed[0]].mesh.fingerprint(), before[changed[0]]);
    // back to the original look restores the exact mesh
    s.set_lot(7, None, LotState::Normal);
    assert_eq!(s.chunks[changed[0]].mesh.fingerprint(), before[changed[0]]);
    // a building site and a burned lot differ from the standing building
    s.set_lot(7, Some(3), LotState::Normal);
    let normal = s.chunks[changed[0]].mesh.fingerprint();
    s.set_lot(7, Some(3), LotState::Building(0.4));
    let site = s.chunks[changed[0]].mesh.fingerprint();
    s.set_lot(7, Some(3), LotState::Burned);
    let burned = s.chunks[changed[0]].mesh.fingerprint();
    assert!(normal != site && site != burned && normal != burned);
}

#[test]
fn cars_follow_volumes() {
    let t = town();
    let mut s = Scene::from_town(&t);
    assert_eq!(s.car_count(), 0);
    assert!(s.instances().cars.is_empty());
    let n = t.roads.len();
    s.set_road_volumes(&vec![0.2; n]);
    let low = s.car_count();
    assert!(low > 0);
    s.set_road_volumes(&vec![0.6; n]);
    let mid = s.car_count();
    s.set_road_volumes(&vec![1.0; n]);
    let high = s.car_count();
    assert!(low < mid && mid <= high, "{low} {mid} {high}");
    assert!(high <= us_render::instances::MAX_CARS);
    assert_eq!(s.instances().cars.len(), high);
    // one empty road, all others full: that road has no cars
    let mut v = vec![0.5; n];
    let empty = t
        .roads
        .iter()
        .position(|r| r.class == RoadClass::Local)
        .unwrap();
    v[empty] = 0.0;
    s.set_road_volumes(&v);
    let mut only = vec![0.0; n];
    only[empty] = 0.5;
    s.set_road_volumes(&only);
    let with = s.car_count();
    s.set_road_volumes(&vec![0.0; n]);
    assert_eq!(s.car_count(), 0);
    assert!(with > 0);
}

#[test]
fn cars_move_and_stay_deterministic() {
    let t = town();
    let mut a = Scene::from_town(&t);
    let mut b = Scene::from_town(&t);
    let v = vec![0.5; t.roads.len()];
    a.set_road_volumes(&v);
    b.set_road_volumes(&v);
    a.set_time(3.0);
    b.set_time(3.0);
    let (ia, ib) = (a.instances().cars, b.instances().cars);
    assert_eq!(ia.len(), ib.len());
    for (x, y) in ia.iter().zip(&ib) {
        assert_eq!(bytemuck::bytes_of(x), bytemuck::bytes_of(y));
    }
    a.set_time(9.0);
    let moved = a.instances().cars;
    assert!(ia.iter().zip(&moved).any(|(x, y)| x.pos != y.pos));
}

#[test]
fn scene_meshes_are_deterministic() {
    let t = town();
    let a = Scene::from_town(&t);
    let b = Scene::from_town(&t);
    assert_eq!(a.ground.fingerprint(), b.ground.fingerprint());
    assert_eq!(a.chunks.len(), b.chunks.len());
    for (x, y) in a.chunks.iter().zip(&b.chunks) {
        assert_eq!(x.mesh.fingerprint(), y.mesh.fingerprint());
    }
}

#[test]
fn road_class_change_remeshes_ground() {
    let t = town();
    let mut s = Scene::from_town(&t);
    let i = t
        .roads
        .iter()
        .position(|r| r.class == RoadClass::Collector)
        .unwrap();
    let (rev, fp) = (s.ground_rev, s.ground.fingerprint());
    s.set_roads(&[(i, RoadClass::Collector, Some(0.4))]);
    assert_eq!(s.ground_rev, rev + 1);
    let works = s.ground.fingerprint();
    assert_ne!(works, fp);
    s.set_roads(&[(i, RoadClass::Arterial, None)]);
    assert_eq!(s.road_class(i), Some(RoadClass::Arterial));
    assert_ne!(s.ground.fingerprint(), works);
    // same state again does nothing
    let rev = s.ground_rev;
    s.set_roads(&[(i, RoadClass::Arterial, None)]);
    assert_eq!(s.ground_rev, rev);
    // back to the original road restores the ground
    s.set_roads(&[(i, RoadClass::Collector, None)]);
    assert_eq!(s.ground.fingerprint(), fp);
}

#[test]
fn calendar_steps() {
    let c = Calendar::from_date(1, 15, 12.0);
    assert_eq!(c.season(), Season::Winter);
    assert_eq!(Calendar::from_date(10, 20, 12.0).season(), Season::Autumn);
    assert_eq!(Calendar::from_date(4, 20, 12.0).season(), Season::Spring);
    assert_eq!(Calendar::from_date(7, 4, 12.0).season(), Season::Summer);
    assert_eq!(Calendar::from_date(12, 31, 12.0).season(), Season::Winter);
    let at = |h| Calendar {
        day_of_year: 100,
        hour: h,
    };
    assert_eq!(at(12.0).phase(), DayPhase::Day);
    assert_eq!(at(22.0).phase(), DayPhase::Night);
    assert_eq!(at(3.0).phase(), DayPhase::Night);
    assert_eq!(at(18.0).phase(), DayPhase::Dusk);
    assert_eq!(at(6.0).phase(), DayPhase::Dawn);
    assert_eq!(at(12.0).lit_windows(), 0.0);
    assert!(at(22.0).lit_windows() > at(18.0).lit_windows());
    // stepped: the same hour block gives the same light
    assert_eq!(at(12.1).light(), at(12.9).light());
    let l = at(22.0).light();
    assert!(l[2] > 0.5);
}

#[test]
fn smoke_comes_from_factories() {
    let t = town();
    let mut s = Scene::from_town(&t);
    let w = t
        .lots
        .iter()
        .position(|l| l.kind == us_gen::LotKind::Workshop);
    if let Some(w) = w {
        s.set_lot(w, Some(4), LotState::Normal);
        assert!(!s.instances().puffs.is_empty());
        s.set_lot(w, Some(4), LotState::Burned);
        let before = s.instances().puffs.len();
        s.set_lot(w, Some(4), LotState::Normal);
        assert!(s.instances().puffs.len() > before);
    }
}
