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
            let glb = crate::gltf::export(&s.model).unwrap_or_else(|e| panic!("{e}"));
            (scene.digest(), fnv(&glb.bytes))
        })
        .collect();
    // Made by the code before GEO-2c-2b (bc8b24b4).
    assert_eq!(found, [(0x460e_0bdb_34da_d0e2, 0x2078_f2ae_ad11_68e7), (0x04c8_7ff0_25fb_9466, 0x004f_4913_cab4_f53b)], "{found:#x?}");
}
