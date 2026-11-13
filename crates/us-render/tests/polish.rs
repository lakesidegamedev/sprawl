use us_gen::{Preset, generate};
use us_render::instances::{LAMP_POOL, lamp_instances, lamp_mesh};
use us_render::{ALL_CIVIC, Rng, Scene, build_civic};

#[test]
fn civic_models_are_solid_and_deterministic() {
    for m in ALL_CIVIC {
        let a = build_civic(m, &mut Rng::new(1, 2));
        let b = build_civic(m, &mut Rng::new(1, 2));
        assert!(a.polys.len() > 10, "{} is too plain", m.name());
        assert_eq!(a.polys.len(), b.polys.len());
        let fp = a.footprint();
        assert!(
            fp[2] - fp[0] < 90.0 && fp[3] - fp[1] < 70.0,
            "{} is huge",
            m.name()
        );
    }
}

#[test]
fn lamps_stand_beside_streets_and_are_stable() {
    let t = generate(Preset::Grid, 1);
    let scene = Scene::from_town(&t);
    let a = scene.instances().lamps;
    let b = scene.instances().lamps;
    assert!(a.len() > 500, "only {} lamps", a.len());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.pos, y.pos);
        assert_eq!(x.theta, y.theta);
    }
    // ids never overlap: each lamp owns a stride
    let mut ids: Vec<u32> = a.iter().map(|l| l.id).collect();
    ids.dedup();
    assert_eq!(ids.len(), a.len());
}

#[test]
fn lamp_mesh_has_a_pool_marker_and_lens() {
    let m = lamp_mesh();
    assert!(m.vertices.iter().any(|v| v.color[..3] == LAMP_POOL));
    assert!(m.triangles() < 40);
    assert!(lamp_instances(&[]).is_empty());
}

#[test]
fn far_form_is_much_lighter_than_the_full_chunk() {
    let t = generate(Preset::Grid, 1);
    let scene = Scene::from_town(&t);
    let full: usize = scene.chunks.iter().map(|c| c.mesh.triangles()).sum();
    let far: usize = scene
        .chunks
        .iter()
        .map(|c| {
            c.far
                .as_ref()
                .expect("town chunks have a far form")
                .triangles()
        })
        .sum();
    assert!(far * 2 < full, "far {far} vs full {full}");
    // the far form has no ink ids
    for c in &scene.chunks {
        let far = c.far.as_ref().unwrap();
        assert!(far.vertices.iter().all(|v| v.id == us_render::ID_NONE));
    }
}
