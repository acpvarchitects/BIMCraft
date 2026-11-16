//! Shaped walls, option A (batter): a face leaning or curving up a wall's height, through the commands every front end drives,
//! measured on the shaped solid against what a hand works out.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::Session;
use crate::geom::P2;
use crate::model::{Element, Wall};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn fails(s: &mut Session, id: &str, p: Value) -> String {
    let before = s.model.clone();
    let e = s.execute(id, &p).expect_err(&format!("{id} {p} should fail"));
    assert_eq!(*s.model, *before, "a refused {id} changes nothing");
    e
}

fn wall_of(s: &Session, id: u64) -> &Wall {
    let Some(Element::Wall(w)) = s.model.elements.get(id) else { panic!("no wall {id}") };
    w
}

/// A session with a 300 mm wall type of no layers, `Thick`.
fn session() -> Session {
    let mut s = Session::new();
    run(&mut s, "type", json!({"category": "wall", "name": "Thick", "thickness": 300}));
    s
}

/// A straight wall of `Thick`, 3000 mm tall, its location line its centre, from `a` to `b`.
fn wall(s: &mut Session, a: [f64; 2], b: [f64; 2]) -> u64 {
    run(s, "wall", json!({"points": [a, b], "type": "Thick", "height": 3000, "location": "centre"}))["ids"][0].as_u64().unwrap()
}

fn details(s: &mut Session, id: u64) -> Value {
    let all = run(s, "elements", json!({"category": "walls"}));
    all["elements"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap().clone()
}

fn volume(s: &Session, id: u64, openings: bool) -> f64 {
    crate::walls::volume(&s.model, id, openings)
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Every edge of `tris` is used once each way: the solid is closed, with no gap.
fn closed(tris: &[[[f64; 3]; 3]]) -> bool {
    let key = |p: [f64; 3]| p.map(|x| (x * 1e3).round() as i64);
    let mut edges: BTreeMap<([i64; 3], [i64; 3]), i64> = BTreeMap::new();
    for t in tris {
        for i in 0..3 {
            let (a, b) = (key(t[i]), key(t[(i + 1) % 3]));
            if a == b {
                continue;
            }
            *edges.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
        }
    }
    edges.values().all(|n| *n == 0)
}

/// Wall `id`'s shaped solid, its pieces' triangles.
fn meshes(s: &Session, id: u64, openings: bool) -> Vec<Vec<[[f64; 3]; 3]>> {
    let prisms = crate::walls::prisms(&s.model, id, openings);
    crate::walls::shaped_meshes(&s.model, id, &prisms, None).expect("shaped")
}

#[test]
fn a_battered_wall_measures_its_volume_and_thickness_as_by_hand() {
    let mut s = session();
    let id = wall(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    assert!(near(volume(&s, id, true), 4000.0 * 300.0 * 3000.0, 1.0));
    // Its exterior face 100 mm in at its top: a wedge of 4000 × 100 × 3000 / 2 comes off.
    let reply = run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -100}));
    assert_eq!([&reply["thickness_min"], &reply["thickness_max"], &reply["thickness_avg"]], [200.0, 300.0, 250.0]);
    assert!(near(volume(&s, id, true), 3.0e9, 1e3), "{}", volume(&s, id, true));
    let d = details(&mut s, id);
    assert_eq!((d["volume_m3"].as_f64(), d["thickness_min"].as_f64(), d["thickness_avg"].as_f64()), (Some(3.0), Some(200.0), Some(250.0)));
    assert!(meshes(&s, id, true).iter().all(|m| closed(m)));
    // By an angle: the same face.
    let angle = (100.0f64 / 3000.0).atan().to_degrees();
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "angle": -angle}));
    assert!(near(volume(&s, id, true), 3.0e9, 1e3));
    // A scarp: 100 mm in over the first metre, plumb above: 4000 × (1000 × 250 + 2000 × 200).
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "profile": [[1000, -100]]}));
    assert!(near(volume(&s, id, true), 2.6e9, 1e3), "{}", volume(&s, id, true));
    let d = details(&mut s, id);
    assert_eq!((d["thickness_min"].as_f64(), d["thickness_max"].as_f64()), (Some(200.0), Some(300.0)));
    // Both faces: a buttress tapering 50 mm a side, and a wall leaning out of plumb, its faces parallel.
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -50}));
    run(&mut s, "batter", json!({"id": id, "face": "interior", "top": -50}));
    assert!(near(volume(&s, id, true), 4000.0 * 250.0 * 3000.0, 1e3));
    run(&mut s, "batter", json!({"id": id, "face": "interior", "top": 50}));
    let d = details(&mut s, id);
    assert_eq!((d["thickness_min"].as_f64(), d["thickness_max"].as_f64(), d["volume_m3"].as_f64()), (Some(300.0), Some(300.0), Some(3.6)));
    // Taken off, it is as it was; undone, shaped again.
    run(&mut s, "reset_wall_shape", json!({"id": id}));
    assert!(wall_of(&s, id).shape.is_empty());
    assert!(near(volume(&s, id, true), 3.6e9, 1.0));
    run(&mut s, "undo", json!({}));
    assert!(wall_of(&s, id).shape.exterior.is_some());
}

#[test]
fn only_the_core_takes_up_the_batter_its_finishes_keeping_their_thickness() {
    let mut s = Session::new();
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Rendered", "layers": [
            {"function": "finish", "thickness": 20}, {"function": "structure", "thickness": 260}, {"function": "finish", "thickness": 20}]}),
    );
    let id = run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "type": "Rendered", "height": 3000, "location": "centre"}))["ids"]
        [0]
    .as_u64()
    .unwrap();
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -100}));
    let w = wall_of(&s, id).clone();
    let shaper = crate::walls::shape::Shaper::of(&s.model, &w).unwrap();
    // At its top: the render 20 mm, the core 160 mm, the plaster 20 mm.
    let b = crate::walls::boundaries(&s.model, &w);
    let top: Vec<f64> = b.iter().map(|off| shaper.offset(*off, 3000.0)).collect();
    assert_eq!(top, vec![50.0, 30.0, -130.0, -150.0]);
    // A core thinner than nothing is refused, saying where.
    let e = fails(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -300}));
    assert!(e.contains("layer 2 (260 mm)") && e.contains("-40.0 mm thick 3000 mm over its base"), "{e}");
}

#[test]
fn a_plan_cuts_a_battered_wall_where_its_cut_plane_is() {
    let mut s = session();
    let id = wall(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    // Its exterior (north) face 300 mm in at its top: at 1000 mm up 100 mm in, at 2000 mm up 200 mm.
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -300}));
    let view = crate::render::View { angle: 0.0, scale: 0.2, cx: 2000.0, cy: 0.0, width: 1200, height: 400 };
    let inside = |s: &Session, y: f64| crate::render::draw(&s.model, 100000, view).pick(view.px(P2(2000.0, y)), 0.0) == Some(id);
    run(&mut s, "modify", json!({"id": 100000, "view_range": {"cut": 1000}}));
    assert!(inside(&s, 30.0) && !inside(&s, 70.0) && inside(&s, -140.0), "cut at 1000: 250 thick, its north face at y 50");
    run(&mut s, "modify", json!({"id": 100000, "view_range": {"cut": 2000}}));
    let at = [30.0, -30.0, -70.0, -140.0].map(|y| inside(&s, y));
    assert_eq!(at, [false, false, true, true], "cut at 2000: 150 thick, its north face at y -50");
    // The outline at those heights, as the plan fills it.
    let outline = |z: f64| {
        let ring = crate::walls::outline(&s.model, id);
        crate::walls::at_height(&s.model, id, ring, z, None)
    };
    let north = |ring: Vec<P2>| ring.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    assert!(near(north(outline(1000.0)), 50.0, 1e-6) && near(north(outline(2000.0)), -50.0, 1e-6));
}

#[test]
fn an_opening_cuts_through_a_battered_wall_at_its_faces() {
    let mut s = session();
    let id = wall(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    let win = run(&mut s, "window", json!({"wall": id, "offset": 2000, "sill": 900}))["id"].as_u64().unwrap();
    let all = run(&mut s, "elements", json!({"category": "windows"}));
    let wd = all["elements"].as_array().unwrap().iter().find(|e| e["id"] == win).unwrap().clone();
    let (w, h) = (wd["width"].as_f64().unwrap(), wd["height"].as_f64().unwrap());
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -150}));
    // The hole takes the thickness there is between its sill and its head: 300 − 0.05 h mm at h up.
    let (z0, z1) = (900.0, 900.0 + h);
    let hole = w * (300.0 * (z1 - z0) - 0.025 * (z1 * z1 - z0 * z0));
    let gross = 4000.0 * (300.0 * 3000.0 - 0.025 * 3000.0 * 3000.0);
    assert!(near(volume(&s, id, false), gross, 1e3));
    assert!(near(volume(&s, id, true), gross - hole, 1e3), "{} vs {}", volume(&s, id, true), gross - hole);
    assert!(meshes(&s, id, true).iter().all(|m| closed(m)));
    // In 3D it is meshes, closed, that a section cuts; a plan through the window clears it.
    let solids = crate::view3d::solids(&s.model);
    assert!(solids.iter().filter(|x| x.owner() == id).all(|x| matches!(x, crate::view3d::Solid::Mesh { cut: true, .. })));
}

#[test]
fn a_battered_wall_joins_a_straight_one_with_no_gap_or_overlap() {
    let mut s = session();
    // An L, both faces out to the north-west: the west wall battered on its exterior, 150 mm in at its top.
    let a = wall(&mut s, [0.0, 0.0], [0.0, 4000.0]);
    let b = wall(&mut s, [0.0, 4000.0], [4000.0, 4000.0]);
    let (va, vb) = (volume(&s, a, true), volume(&s, b, true));
    assert!(near(va, 1.2e6 * 3000.0, 1.0) && near(vb, 1.2e6 * 3000.0, 1.0), "mitred, each 300 × 4000 in plan");
    run(&mut s, "batter", json!({"id": a, "face": "exterior", "top": -150}));
    // The strip off its exterior face runs up to the mitre: ∫ (4150 d − d²/2) dh with d = h / 20, over 3000 mm.
    let off = 4150.0 * 0.05 * 3000.0f64.powi(2) / 2.0 - 0.00125 * 3000.0f64.powi(3) / 3.0;
    assert!(near(volume(&s, a, true), 1.2e6 * 3000.0 - off, 1e3), "{}", volume(&s, a, true));
    assert!(near(volume(&s, b, true), vb, 1e-3), "the straight wall is as it was");
    assert!(meshes(&s, a, true).iter().all(|m| closed(m)));
    // Its corner slides along the mitre: at its top, on the line x + y = 4000 it shares with the other.
    let top = crate::walls::at_height(&s.model, a, crate::walls::outline(&s.model, a), 3000.0, None);
    assert!(top.iter().any(|p| near(p.0, 0.0, 1e-6) && near(p.1, 4000.0, 1e-6)) && top.iter().all(|p| p.0 >= -1e-6), "{top:?}");
}

#[test]
fn a_room_bounded_by_faces_ends_at_the_battered_face() {
    let mut s = session();
    // A box 6 × 4 m drawn clockwise, its exteriors out, its west wall's interior battered 200 mm out at its top.
    let ids = run(
        &mut s,
        "wall",
        json!({"points": [[0, 0], [0, 4000], [6000, 4000], [6000, 0], [0, 0]], "type": "Thick", "height": 3000, "location": "centre"}),
    )["ids"]
        .clone();
    let west = ids[0].as_u64().unwrap();
    run(&mut s, "room_area", json!({"at": "finish"}));
    let room = run(&mut s, "room", json!({"point": [3000, 2000]}))["id"].as_u64().unwrap();
    let area = |s: &mut Session| {
        let all = run(s, "elements", json!({"category": "rooms"}));
        all["elements"].as_array().unwrap().iter().find(|e| e["id"] == room).unwrap()["area_m2"].as_f64().unwrap()
    };
    let before = area(&mut s);
    assert!(near(before, 5.7 * 3.7, 0.006), "{before}");
    run(&mut s, "batter", json!({"id": west, "face": "interior", "top": 200}));
    // At the level's computation height (1200 mm), the face stands 80 mm into the room; at each corner it ends on the
    // mitre run on into the room, the room keeping the triangle between it and the other wall's face.
    let z = s.model.levels()[0].1.computation_height;
    let after = area(&mut s);
    // Measured at the wall's centre line, it keeps the line.
    run(&mut s, "room_area", json!({"at": "centre"}));
    assert!(near(area(&mut s), 24.0, 0.006));
    let d = 0.2 * z / 3000.0;
    assert!(near(after, (5.7 - d) * 3.7 + d * d, 0.006), "{after}");
}

#[test]
fn a_shape_that_crosses_the_faces_or_is_out_of_range_is_refused() {
    let mut s = session();
    let id = wall(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    let e = fails(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -200, "angle": 3}));
    assert!(e.contains("one of `angle`, `top` or `profile`"), "{e}");
    fails(&mut s, "batter", json!({"id": id, "face": "exterior"}));
    fails(&mut s, "batter", json!({"id": id, "face": "sideways", "top": 10}));
    fails(&mut s, "batter", json!({"id": id, "top": 10}));
    fails(&mut s, "batter", json!({"id": id, "face": "exterior", "angle": 85}));
    fails(&mut s, "batter", json!({"id": id, "face": "exterior", "angle": f64::NAN}));
    fails(&mut s, "batter", json!({"id": id, "face": "exterior", "profile": [[1000, 10], [500, 20]]}));
    fails(&mut s, "batter", json!({"id": id, "face": "exterior", "profile": [[0, 10]]}));
    fails(&mut s, "batter", json!({"id": id, "face": "exterior", "top": 10, "bulge": 2}));
    // The faces would cross: 150 in at the top on both sides of a 300 wall is a knife edge, more is past it.
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -150}));
    run(&mut s, "batter", json!({"id": id, "face": "interior", "top": -150}));
    let e = fails(&mut s, "batter", json!({"id": id, "face": "interior", "top": -151}));
    assert!(e.contains("lessen the batter or thicken the wall"), "{e}");
    // A profile crossing them half way up, though plumb at the top, is refused too.
    let e = fails(&mut s, "batter", json!({"id": id, "face": "interior", "profile": [[1500, -400], [2500, 0]]}));
    assert!(e.contains("1500 mm over its base"), "{e}");
    // Raising the wall carries an angle's batter higher: past the knife edge, refused.
    run(&mut s, "batter", json!({"id": id, "face": "interior", "angle": -(150.0f64 / 3000.0).atan().to_degrees()}));
    fails(&mut s, "modify", json!({"id": id, "height": 3100}));
    // Not a wall.
    let level = 100000;
    fails(&mut s, "batter", json!({"id": level, "face": "exterior", "top": 10}));
}

#[test]
fn a_shaped_wall_round_trips_through_its_file_and_an_unshaped_one_is_written_as_before() {
    let mut s = Session::new();
    s.script(crate::sample::SAMPLE).unwrap();
    let before = s.model.to_json().unwrap();
    let ifc_before: String = crate::ifc::export(&s.model);
    assert!(!before.contains("\"batter\""));
    let id = s.model.elements.iter().find(|(_, e)| matches!(e, Element::Wall(_))).map(|(id, _)| *id).unwrap();
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "profile": [[600, -40], [1200, -60]]}));
    let text = s.model.to_json().unwrap();
    assert!(text.contains("\"shape\":{\"exterior\":{\"batter\":{\"profile\":[[600.0,-40.0],[1200.0,-60.0]]}}}"), "the shape as saved");
    let (back, left_out) = crate::model::Model::read(&text).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(back.elements.get(id), s.model.elements.get(id));
    assert_eq!(back.to_json().unwrap(), text);
    // Taken off again, the file and its IFC are byte for byte what they were.
    run(&mut s, "reset_wall_shape", json!({"id": id, "face": "exterior"}));
    assert_eq!(s.model.to_json().unwrap(), before);
    // (but for the time it was written, in its header)
    let timeless = |f: String| f.lines().filter(|l| !l.starts_with("FILE_NAME(")).collect::<Vec<_>>().join("\n");
    assert_eq!(timeless(crate::ifc::export(&s.model)), timeless(ifc_before));
    // Unshaped, no wall's solid is bent.
    let walls: Vec<u64> = s.model.elements.iter().filter(|(_, e)| matches!(e, Element::Wall(_))).map(|(id, _)| *id).collect();
    assert!(walls.iter().all(|w| crate::walls::shaped_meshes(&s.model, *w, &[], None).is_none()));
}

#[test]
fn a_shaped_wall_goes_to_ifc_as_a_wall_with_a_triangulated_body() {
    let mut s = session();
    let id = wall(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    run(&mut s, "window", json!({"wall": id, "offset": 2000, "sill": 900}));
    let plain = crate::ifc::export(&s.model);
    let n = |f: &str, e: &str| f.matches(&format!("={e}(")).count();
    assert_eq!((n(&plain, "IFCWALL"), n(&plain, "IFCTRIANGULATEDFACESET")), (1, 0));
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -100}));
    let f = crate::ifc::export(&s.model);
    assert_eq!((n(&f, "IFCWALL"), n(&f, "IFCTRIANGULATEDFACESET"), n(&f, "IFCOPENINGELEMENT")), (1, 1, 1));
    assert!(f.contains("'Body','Tessellation'"));
    // Its quantities are the shaped solid's: 3.0 m³ gross.
    assert!(
        f.contains("IFCQUANTITYVOLUME('GrossVolume',$,$,3.,$)"),
        "{}",
        f.lines().filter(|l| l.contains("Volume")).collect::<Vec<_>>().join("\n")
    );
    // IFC2x3 has no face sets: a faceted B-rep.
    let old = crate::ifc::export_as(&s.model, crate::ifc::Origin::Internal, crate::ifc::Schema::Ifc2x3);
    assert_eq!((n(&old, "IFCWALL"), n(&old, "IFCTRIANGULATEDFACESET")), (1, 0));
    assert!(n(&old, "IFCFACETEDBREP") >= 1);
}

#[test]
fn a_section_across_a_battered_wall_shows_its_true_profile() {
    let mut s = session();
    let id = wall(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "top": -100}));
    let sec = run(&mut s, "section", json!({"start": [2000, -1000], "end": [2000, 1000]}))["id"].as_u64().unwrap();
    let view = crate::render::section_fit(&s.model, sec, 1000, 800);
    let d = crate::render::section(&s.model, sec, view);
    // The cut's corners, along the line and up: 300 wide at its base, 200 at its top, its exterior side leaning.
    let pts: Vec<P2> = d
        .prims
        .iter()
        .zip(&d.owners)
        .filter_map(|(p, o)| match p {
            crate::render::Prim::Fill { rings, .. } if *o == Some(id) => {
                Some(rings.iter().flatten().map(|q| view.mm(*q)).collect::<Vec<_>>())
            }
            _ => None,
        })
        .flatten()
        .collect();
    let width = |z: f64| {
        let at: Vec<f64> = pts.iter().filter(|p| (p.1 - z).abs() < 5.0).map(|p| p.0).collect();
        at.iter().copied().fold(f64::NEG_INFINITY, f64::max) - at.iter().copied().fold(f64::INFINITY, f64::min)
    };
    assert!(near(width(0.0), 300.0, 5.0) && near(width(3000.0), 200.0, 5.0), "{} {} {pts:?}", width(0.0), width(3000.0));
}

#[test]
fn a_battered_wall_exported_to_ifc_and_imported_comes_back_battered() {
    let mut s = session();
    let id = wall(&mut s, [0.0, 0.0], [4000.0, 0.0]);
    run(&mut s, "batter", json!({"id": id, "face": "exterior", "profile": [[1000, -60], [2000, -80]]}));
    run(&mut s, "batter", json!({"id": id, "face": "interior", "top": 30}));
    let file = crate::ifc::export(&s.model);
    let i = crate::import::ifc(file.as_bytes(), "x").unwrap();
    assert_eq!(i.skipped, Vec::<String>::new());
    let (back, w) = i
        .model
        .elements
        .iter()
        .find_map(|(id, e)| match e {
            Element::Wall(w) => Some((*id, w.clone())),
            _ => None,
        })
        .expect("a wall");
    assert_eq!(i.model.thickness(&w), 300.0, "as thick as at its base");
    let profile = |f: &Option<crate::walls::shape::FaceShape>| match f {
        Some(crate::walls::shape::FaceShape::Batter(b)) => b.profile.clone(),
        None => Vec::new(),
    };
    assert_eq!(profile(&w.shape.exterior), vec![[1000.0, -60.0], [2000.0, -80.0]]);
    assert!(matches!(&w.shape.interior, Some(crate::walls::shape::FaceShape::Batter(b)) if b.top == Some(30.0)));
    assert!(near(crate::walls::volume(&i.model, back, false), volume(&s, id, false), 1e3));
    // A plumb wall comes back plumb.
    let mut plain = session();
    wall(&mut plain, [0.0, 0.0], [4000.0, 0.0]);
    let i = crate::import::ifc(crate::ifc::export(&plain.model).as_bytes(), "x").unwrap();
    assert!(i.model.elements.values().all(|e| !matches!(e, Element::Wall(w) if !w.shape.is_empty())));
}
