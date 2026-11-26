//! GEO-2c-2b: the 3D view's components chorded as finely as the camera shows them. A scene is made as ever, at the default;
//! [`Scene::refined`] then builds each component on the screen at about half a pixel where it is seen (the same fineness for
//! every component in a parallel view, its nearest point's in perspective), changes a component's band only half a step past
//! its boundary, spends a budget of triangles nearest first, and keeps copies in one band one shape.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::ids::Id;

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn id(v: &Value) -> Id {
    v["id"].as_u64().unwrap_or_else(|| panic!("no id: {v}"))
}

/// The family's Center (Front/Back) plane: a sketch on it is in x and z, and goes from its `from` to its `to` down y.
const FRONT: &str = "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2";
/// The cylinders' radius: 32 chords a turn stray 3.85 mm from it.
const R: f64 = 800.0;

/// Generic family `name` of `forms`, one type `T` and an instance parameter `K` that changes nothing but its key.
fn family(s: &mut Session, name: &str, forms: Vec<Value>) {
    run(
        s,
        "family",
        json!({"name": name, "category": "generic", "parameters": [{"name": "K", "value": 0, "instance": true}],
               "types": {"T": {}}, "forms": forms}),
    );
}

/// A component of family `name` at `at`, its `K` `k`.
fn place(s: &mut Session, name: &str, at: [f64; 2], k: f64) -> Id {
    id(&run(s, "component", json!({"family": name, "type": "T", "at": at, "params": {"K": k}})))
}

/// A cylinder of radius [`R`] lying along y, its axis at x = 0 and z = 1000: its round ends face south and north.
fn lying() -> Value {
    json!({"sketch": [{"centre": [0, 1000], "radius": R}], "host": {"plane": FRONT}, "from": -2000, "to": 0})
}

/// A cylinder of radius [`R`] standing from 0 to 2000 round x = 0, y = 1000.
fn standing() -> Value {
    json!({"sketch": [{"centre": [0, 1000], "radius": R}], "from": 0, "to": 2000})
}

/// A box 1000 by 600, 900 high: no curve.
fn boxy() -> Value {
    json!({"profile": [[0, 0], [1000, 0], [1000, 600], [0, 600]], "to": 900})
}

/// FNV-1a of `bytes`.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, x| (h ^ u64::from(*x)).wrapping_mul(0x0100_0000_01b3))
}

/// FNV-1a of a .glb, each node's GUID (a new one each time a model is made) left out: its JSON chunk without them, then its
/// binary chunk.
fn glb(bytes: &[u8]) -> u64 {
    let len = bytes.get(12..16).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut json: Value = serde_json::from_slice(bytes.get(20..20 + len).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    for node in json["nodes"].as_array_mut().into_iter().flatten() {
        if let Some(extras) = node["extras"].as_object_mut() {
            extras.remove("guid");
        }
    }
    let mut all = serde_json::to_vec(&json).unwrap_or_else(|e| panic!("{e}"));
    all.extend_from_slice(bytes.get(20 + len..).unwrap_or_default());
    fnv(&all)
}

/// A model of curved components and their copies, in two bands of a perspective view and off it, with a box, a curtain wall
/// and round pipes: what the guard holds the scene of.
fn curved_model() -> Session {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    family(&mut s, "Standing", vec![standing()]);
    family(&mut s, "Box", vec![boxy()]);
    for (k, x) in [0.0, 3000.0, 6000.0, 30_000.0].into_iter().enumerate() {
        place(&mut s, "Lying", [x, 0.0], (k % 2) as f64);
        place(&mut s, "Standing", [x, 6000.0], 0.0);
    }
    place(&mut s, "Box", [-3000.0, 0.0], 0.0);
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Facade", "curtain": {"columns": {"segments": [1500]}, "rows": {"segments": [1000]}}}),
    );
    run(&mut s, "wall", json!({"points": [[-4000, 9000], [5000, 9000]], "type": "Facade"}));
    run(&mut s, "pipe", json!({"points": [[-4000, -3000], [6000, -3000]]}));
    run(&mut s, "pipe", json!({"points": [[-4000, -4000], [6000, -4000]]}));
    s
}

/// The guard: a scene made as ever, with nothing refined, is what it was before GEO-2c-2b to the bit (its triangles, planes,
/// normals, faces, shapes and copies), of the sample house and of a model of curves and copies; and so is a glTF of each.
#[test]
fn a_scene_made_as_ever_and_a_gltf_are_what_they_were_before_refining() {
    let mut house = Session::new();
    house.script(crate::sample::SAMPLE).unwrap_or_else(|e| panic!("{e}"));
    let curved = curved_model();
    let found: Vec<(u64, u64)> = [&house, &curved]
        .iter()
        .map(|s| {
            let scene = crate::view3d::Scene::new(&s.model);
            let file = crate::gltf::export(&s.model).unwrap_or_else(|e| panic!("{e}"));
            (scene.digest(), glb(&file.bytes))
        })
        .collect();
    // Made by the code before GEO-2c-2b (bc8b24b4), the glTF's GUIDs left out.
    assert_eq!(found, [(0x460e_0bdb_34da_d0e2, 0x1902_dbaf_6c59_2570), (0x04c8_7ff0_25fb_9466, 0x8514_310c_a381_3456)], "{found:#x?}");
}

use crate::component_geometry::{builds, fine_builds};
use crate::family::Fineness;
use crate::graphics::Look;
use crate::view3d::{Camera, Scene};

/// A parallel camera looking north, level, `scale` px a mm, at `target`, `w` by `h` px: right is east and up is up.
fn north(scale: f64, target: [f64; 3], (w, h): (u32, u32)) -> Camera {
    Camera { azimuth: -90.0, elevation: 0.0, scale, target, width: w, height: h, lens: None }
}

/// The focal length in px of a 35 mm lens on a picture `w` px wide.
fn focal(w: u32) -> f64 {
    f64::from(w) * 35.0 / crate::view3d::FILM
}

/// A camera through a 35 mm lens, level, looking from `azimuth` at `target` from `d` mm off, 800 by 600 px.
fn eye(azimuth: f64, target: [f64; 3], d: f64) -> Camera {
    Camera { azimuth, elevation: 0.0, scale: focal(800) / d, target, width: 800, height: 600, lens: Some(35.0) }
}

/// The box round component `id`'s triangles in `scene`.
fn box_of(scene: &Scene, id: Id) -> ([f64; 3], [f64; 3]) {
    scene.boxes(&[id].into_iter().collect()).remove(&id).unwrap_or_else(|| panic!("{id} drawn"))
}

/// In a picture through `cam` (looking north), how far the lying cylinder `c`'s round end, its centre at x = 0 and z = 1000,
/// falls short of the circle on its right, at most over the rows across it: px. Asserts that no pixel's middle lies past it.
fn shortfall(scene: &Scene, cam: &Camera, c: Id) -> f64 {
    let img = scene.render(cam, &[]);
    let (w, h) = (cam.width, cam.height);
    let column = |x: f64| f64::from(w) / 2.0 + (x - cam.target[0]) * cam.scale;
    let (mut rows, mut worst) = (0, 0f64);
    for y in 0..h {
        let dz = cam.target[2] - (f64::from(y) + 0.5 - f64::from(h) / 2.0) / cam.scale - 1000.0;
        let edge = column((R * R - dz * dz).max(0.0).sqrt());
        if dz.abs() >= R - 1.0 || !(2.0..f64::from(w) - 2.0).contains(&edge) {
            continue;
        }
        let Some(last) = (0..w).rev().find(|x| img.pick(f64::from(*x) + 0.5, f64::from(y) + 0.5) == Some(c)) else {
            panic!("row {y} shows nothing of it")
        };
        assert!(f64::from(last) + 0.5 <= edge + 0.01, "row {y}: a pixel past the circle");
        worst = worst.max(edge - (f64::from(last) + 0.5));
        rows += 1;
    }
    assert!(rows > 100, "{rows} rows across its edge");
    worst
}

/// A parallel view zoomed in on a cylinder's round end at 4 px a mm: refined, the picture shows it out to the circle, on every
/// row the middle of the last pixel it covers within a pixel and a half of it (half a pixel of chord, a pixel of raster), as
/// GEO-2c-2a's elevation does; the scene made as ever leaves some 15 px.
#[test]
fn zoomed_in_a_refined_parallel_view_draws_a_round_face_within_half_a_pixel() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    let c = place(&mut s, "Lying", [0.0, 0.0], 0.0);
    let scene = Scene::new(&s.model);
    let cam = north(4.0, [R - 30.0, -5000.0, 1000.0], (400, 1600));
    let before = shortfall(&scene, &cam, c);
    assert!(before > 5.0, "the default's chords {before:.1} px in");
    let refined = scene.refined(&s.model, &cam, &Look::of_3d(&s.model)).unwrap_or_else(|| panic!("refined"));
    assert_eq!(refined.fineness_of(c), Fineness::for_view(4.0));
    let after = shortfall(&refined, &cam, c);
    assert!(after <= 1.5 + 1e-6, "its edge {after:.2} px from the circle");
}

/// A curved component on the screen, a box beside it and a curved one far off it: a zoom within a band refines nothing and
/// builds nothing; one across a band builds the curved one on the screen again and nothing else; a zoom back builds nothing,
/// its builds kept; and each refined scene is the whole scene made at its finenesses.
#[test]
fn a_zoom_within_a_band_refines_nothing_and_one_across_rebuilds_only_the_curves_on_screen() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    family(&mut s, "Box", vec![boxy()]);
    let near = place(&mut s, "Lying", [0.0, 0.0], 0.0);
    let boxed = place(&mut s, "Box", [900.0, -3000.0], 0.0);
    let far = place(&mut s, "Lying", [200_000.0, 0.0], 1.0);
    let (m, look) = (std::sync::Arc::clone(&s.model), Look::of_3d(&s.model));
    let at = |scale: f64| north(scale, [800.0, -5000.0, 1000.0], (1200, 800));
    let refine = |scene: &Scene, scale: f64| {
        let (all, fine) = (builds(), fine_builds());
        let out = scene.refined(&m, &at(scale), &look);
        let built = (builds() - all, fine_builds() - fine);
        if let Some(r) = &out {
            r.same_as(&Scene::whole_at(&m, r)).unwrap_or_else(|e| panic!("at {scale} px a mm: {e}"));
        }
        (out, built.0, built.1)
    };
    let scene = Scene::new(&m);
    // 0.3 px a mm asks for chords within a millimetre: the cylinder's take more than the default's.
    let (first, ..) = refine(&scene, 0.3);
    let first = first.unwrap_or_else(|| panic!("refined at 0.3 px a mm"));
    assert_eq!([near, boxed, far].map(|id| first.fineness_of(id)), [Fineness::for_view(0.3), Fineness::DEFAULT, Fineness::DEFAULT]);
    // Within the band: nothing.
    let (within, all, _) = refine(&first, 0.36);
    assert!(within.is_none(), "a zoom within the band changes nothing");
    assert_eq!(all, 0, "and builds nothing");
    // Across one band, past its half step: the cylinder on the screen alone, built once.
    let (across, all, fine) = refine(&first, 0.8);
    let across = across.unwrap_or_else(|| panic!("refined at 0.8 px a mm"));
    assert_eq!([near, boxed, far].map(|id| across.fineness_of(id)), [Fineness::for_view(0.8), Fineness::DEFAULT, Fineness::DEFAULT]);
    assert_eq!((all, fine), (1, 1), "only the curved component on the screen built again");
    // Back: the builds kept.
    let (back, all, _) = refine(&across, 0.3);
    assert_eq!(back.map(|b| b.fineness_of(near)), Some(Fineness::for_view(0.3)));
    assert_eq!(all, 0, "the band's build kept");
    let (out, all, _) = refine(&first, 0.02);
    let out = out.unwrap_or_else(|| panic!("back to the default zoomed out"));
    assert_eq!(out.fineness_of(near), Fineness::DEFAULT);
    assert_eq!(all, 0, "the default build kept");
    assert_eq!(out.tris_of(near), scene.tris_of(near), "the default's triangles again");
}

/// A perspective view orbiting a cylinder at a constant distance, its nearest point now one side of a band's edge, now the
/// other: once refined, each settle after refines nothing and builds nothing, the band changing only half a step past its edge.
#[test]
fn orbiting_at_a_constant_distance_round_a_bands_edge_refines_nothing_after_the_first() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    let c = place(&mut s, "Lying", [0.0, 0.0], 0.0);
    let (m, look) = (std::sync::Arc::clone(&s.model), Look::of_3d(&s.model));
    let scene = Scene::new(&m);
    let (lo, hi) = box_of(&scene, c);
    let target: [f64; 3] = std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0);
    let half: [f64; 3] = std::array::from_fn(|k| (hi[k] - lo[k]) / 2.0);
    // Looking north, its nearest point is half its length nearer than the target: 2^0.3 times the edge of the 1 mm band near.
    let edge = 4.0 * focal(800);
    let refined =
        scene.refined(&m, &eye(-90.0, target, edge * 2f64.powf(0.3 - 1.0) + half[1]), &look).unwrap_or_else(|| panic!("refined near"));
    assert_eq!(refined.fineness_of(c), Fineness::for_view(0.5));
    // Back to where, orbiting, its nearest point is now nearer, now farther than that edge.
    let d = edge + (half[0] + half[0].hypot(half[1])) / 2.0;
    let fine = fine_builds();
    let mut bands = std::collections::BTreeSet::new();
    for k in 0..13 {
        let a = -90.0 + 10.0 * f64::from(k);
        let (sin, cos) = a.to_radians().sin_cos();
        let nearest = d - half[0] * cos.abs() - half[1] * sin.abs();
        bands.insert(Fineness::for_view(focal(800) / nearest));
        assert!(refined.refined(&m, &eye(a, target, d), &look).is_none(), "at {a}°, {nearest:.0} mm off, the band kept");
    }
    assert!(bands.len() > 1, "the orbit crosses the band's edge: {bands:?}");
    assert_eq!(fine_builds() - fine, 0, "nothing built");
}

/// In perspective two copies near and two far of one family: each pair in its band, a shape a band, each copy placed from its
/// shape, as the card draws them.
#[test]
fn near_and_far_copies_of_a_family_are_two_shapes_each_copied() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    let copies: Vec<Id> =
        [[0.0, 0.0], [1800.0, 0.0], [0.0, 2100.0], [1800.0, 2100.0]].iter().map(|at| place(&mut s, "Lying", *at, 0.0)).collect();
    let (m, look) = (std::sync::Arc::clone(&s.model), Look::of_3d(&s.model));
    let scene = Scene::new(&m);
    let mesh = crate::view3d::gpu::Mesh::new(&scene, &look).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((mesh.shapes.len(), mesh.instances.len()), (1, 4), "made as ever, one shape copied four times");
    // Looking north 2 m off the near pair's nearest point: it 0.36 a power of two finer than the 2 mm band's edge, the far pair
    // 2.1 m farther.
    let (lo, _) = box_of(&scene, copies[0]);
    let cam = eye(-90.0, [900.0, lo[1] + 1000.0, 1000.0], 3000.0);
    let refined = scene.refined(&m, &cam, &look).unwrap_or_else(|| panic!("refined"));
    let at = copies.iter().map(|c| refined.fineness_of(*c)).collect::<Vec<_>>();
    assert_eq!(at, [Fineness::for_view(0.5), Fineness::for_view(0.5), Fineness::for_view(0.25), Fineness::for_view(0.25)]);
    let shape = |id: Id| refined.copies().into_iter().find(|(c, _)| *c == id).map(|(_, s)| s).unwrap_or_else(|| panic!("{id} copied"));
    let [a, b, c, d] = [0, 1, 2, 3].map(|k| shape(copies[k]));
    assert_eq!((a, c), (b, d), "a shape a band");
    assert_ne!(a, c, "two bands, two shapes");
    assert!(refined.shape_size(a) > refined.shape_size(c), "the near band's the finer");
    refined.same_as(&Scene::whole_at(&m, &refined)).unwrap_or_else(|e| panic!("{e}"));
    let mesh = crate::view3d::gpu::Mesh::new(&refined, &look).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((mesh.shapes.len(), mesh.instances.len()), (2, 4), "on the card: two shapes, each copied twice");
}

/// Copies in a row going away from a parallel view, every one asking the same band: a budget for two and a half of them refines
/// the two nearest and no other; none, nothing.
#[test]
fn a_small_budget_refines_the_nearest_and_never_one_farther() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    let row: Vec<Id> = (0..6).map(|k| place(&mut s, "Lying", [0.0, 2500.0 * f64::from(k)], 0.0)).collect();
    let (m, look) = (std::sync::Arc::clone(&s.model), Look::of_3d(&s.model));
    let scene = Scene::new(&m);
    let cam = north(0.3, [0.0, -5000.0, 1000.0], (1200, 800));
    let all = scene.refined(&m, &cam, &look).unwrap_or_else(|| panic!("refined"));
    assert!(row.iter().all(|c| all.fineness_of(*c) == Fineness::for_view(0.3)), "every one, with room for all");
    let each = all.tris_of(row[0]);
    let some = crate::view3d::with_finer_budget(each * 5 / 2, || scene.refined(&m, &cam, &look)).unwrap_or_else(|| panic!("refined"));
    let at: Vec<bool> = row.iter().map(|c| some.fineness_of(*c) != Fineness::DEFAULT).collect();
    assert_eq!(at, [true, true, false, false, false, false], "the two nearest");
    assert!(crate::view3d::with_finer_budget(0, || scene.refined(&m, &cam, &look)).is_none(), "no budget, nothing refined");
}

/// An edit after a refinement: the scene brought up to date keeps each component's fineness, the edited one's too, and is the
/// whole scene made at them; an edit of the library, which makes the whole scene again, keeps them too.
#[test]
fn an_edit_after_refining_keeps_every_components_fineness() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    family(&mut s, "Box", vec![boxy()]);
    let a = place(&mut s, "Lying", [0.0, 0.0], 0.0);
    let b = place(&mut s, "Lying", [-2000.0, 0.0], 1.0);
    let boxed = place(&mut s, "Box", [900.0, -3000.0], 0.0);
    let look = Look::of_3d(&s.model);
    let cam = north(0.8, [0.0, -5000.0, 1000.0], (2400, 800));
    let mut scene = Scene::new(&s.model).refined(&s.model, &cam, &look).unwrap_or_else(|| panic!("refined"));
    let fine = Fineness::for_view(0.8);
    for (cmd, p) in [
        ("move", json!({"ids": [boxed], "by": [100, 0]})),
        ("move", json!({"ids": [a], "by": [0, 300]})),
        ("modify", json!({"id": b, "params": {"K": 2}})),
        ("material", json!({"name": "Timber", "color": [200, 40, 40]})),
    ] {
        let before = std::sync::Arc::clone(&s.model);
        run(&mut s, cmd, p.clone());
        scene = scene.update(&before, &s.model);
        assert_eq!([a, b, boxed].map(|id| scene.fineness_of(id)), [fine, fine, Fineness::DEFAULT], "after {cmd} {p}");
        scene.same_as(&Scene::whole_at(&s.model, &scene)).unwrap_or_else(|e| panic!("after {cmd} {p}: {e}"));
    }
}

/// A camera of no numbers, or of none that make a picture, refines nothing and doesn't panic.
#[test]
fn a_hostile_camera_refines_nothing() {
    let mut s = Session::new();
    family(&mut s, "Lying", vec![lying()]);
    place(&mut s, "Lying", [0.0, 0.0], 0.0);
    let (m, look) = (std::sync::Arc::clone(&s.model), Look::of_3d(&s.model));
    let scene = Scene::new(&m);
    let good = north(0.8, [0.0, -5000.0, 1000.0], (1200, 800));
    assert!(scene.refined(&m, &good, &look).is_some(), "a good camera refines it");
    let (nan, inf) = (f64::NAN, f64::INFINITY);
    let hostile = [
        Camera { scale: 0.0, ..good },
        Camera { scale: -1.0, ..good },
        Camera { scale: nan, ..good },
        Camera { scale: inf, ..good },
        Camera { lens: Some(0.0), ..good },
        Camera { lens: Some(nan), ..good },
        Camera { lens: Some(-35.0), ..good },
        Camera { lens: Some(inf), ..good },
        Camera { width: 0, ..good },
        Camera { height: 0, ..good },
        Camera { target: [nan, 0.0, 0.0], ..good },
        Camera { target: [inf, 0.0, 0.0], ..good },
        Camera { azimuth: nan, ..good },
        Camera { elevation: inf, ..good },
        Camera { scale: 1e-300, lens: Some(35.0), ..good },
    ];
    for cam in hostile {
        assert!(scene.refined(&m, &cam, &look).is_none(), "{cam:?}");
    }
}

/// The radius of [`ring`]'s outer arc: 30° through 1000 mm.
fn ring_radius() -> f64 {
    500.0 / 15f64.to_radians().sin()
}

/// How far east [`ring`] reaches, at 600 up: its outer arc at its widest, turned to the east.
fn apex() -> f64 {
    2800.0 + ring_radius() * (1.0 - 15f64.to_radians().cos())
}

/// A ring curved two ways, as GEO-2c-2a's review has it: a profile 200 mm wide from 100 to 1100 up, its outer side an arc of
/// 30° bulging east to [`apex`], turned about the family's z from -16.875° to 16.875°. Its default build stops 21 mm short of
/// its apex.
fn ring() -> Value {
    json!({"sketch": [
        {"start": [2800, 100], "end": [2800, 1100], "turn": 30},
        {"start": [2800, 1100], "end": [2600, 1100]},
        {"start": [2600, 1100], "end": [2600, 100]},
        {"start": [2600, 100], "end": [2800, 100]},
        {"start": [0, 0], "end": [0, 100], "axis": true}],
        "host": {"plane": FRONT}, "from": -16.875, "to": 16.875})
}

/// GEO-2c-2a's review, in the 3D view: a component is asked whether it is on the picture by its default build's box widened by
/// as far as a finer build may reach past it (`fineness::finer_reach`). [`ring`]'s default box stops short of a picture
/// zoomed in to 2 px a mm whose left edge its curve crosses: refined, its curve shows there out to its apex, within half a pixel
/// of chord and one of raster.
#[test]
fn a_finer_curve_reaching_into_the_picture_past_its_default_box_is_refined_and_shows() {
    let mut s = Session::new();
    family(&mut s, "Ring", vec![ring()]);
    let c = place(&mut s, "Ring", [0.0, 0.0], 0.0);
    let (m, look) = (std::sync::Arc::clone(&s.model), Look::of_3d(&s.model));
    let scene = Scene::new(&m);
    let (_, hi) = box_of(&scene, c);
    // Looking north, 2 px a mm, the picture's left edge at 2850 east: past its default box, short of its apex.
    let (scale, w, h) = (2.0, 400u32, 400u32);
    let left = 2850.0;
    assert!(hi[0] < left && apex() > left + 10.0, "its default box to {:.1}, its apex at {:.1}", hi[0], apex());
    let cam = north(scale, [left + f64::from(w) / 2.0 / scale, -5000.0, 600.0], (w, h));
    let shows = |scene: &Scene| {
        let img = scene.render(&cam, &[]);
        (0..w).rev().find(|x| img.pick(f64::from(*x) + 0.5, f64::from(h / 2) + 0.5) == Some(c)).map(|x| left + (f64::from(x) + 0.5) / scale)
    };
    assert_eq!(shows(&scene), None, "made as ever, nothing of it on the picture");
    let refined = scene.refined(&m, &cam, &look).unwrap_or_else(|| panic!("refined: its finer curve reaches into the picture"));
    assert_eq!(refined.fineness_of(c), Fineness::for_view(scale));
    let last = shows(&refined).unwrap_or_else(|| panic!("its curve on the picture"));
    assert!(last <= apex() + 0.01 && apex() - last <= 2.0 / scale, "its last pixel at {last:.2}, its apex at {:.2}", apex());
}
