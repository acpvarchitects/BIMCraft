//! Divide Parts (`divide_parts`): a part cut into pieces that are parts of their own, its division kept in its host's own
//! coordinates so the pieces follow the host as the part does.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::model::{Element, Id, Model};
use crate::parts::BuildingPart;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn fails(s: &mut Session, id: &str, p: Value) -> String {
    let before = s.model.clone();
    let e = s.execute(id, &p).expect_err(&format!("{id} {p} should fail"));
    assert_eq!(*s.model, *before, "a refused {id} changes nothing");
    e
}

/// A historic wall along x from the origin, 4 m long and 3 m tall, its location line down its middle: 20 mm of lime
/// plaster on its exterior side (north, y from 230 to 250), 460 of rubble masonry, 20 of plaster again; taken apart.
/// The session, the wall and its three parts, exterior plaster first.
fn historic() -> (Session, Id, [Id; 3]) {
    let mut s = Session::new();
    for (name, color) in [("Lime Plaster", "#e8e0d0"), ("Rubble Masonry", "#9a8f80")] {
        run(&mut s, "material", json!({"name": name, "color": color}));
    }
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 500mm", "layers": [
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 460},
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
        ]}),
    );
    let wall =
        run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "type": "Historic - 500mm", "height": 3000}))["ids"][0].as_u64().unwrap();
    let ids =
        run(&mut s, "create_parts", json!({"id": wall}))["ids"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect::<Vec<_>>();
    (s, wall, [ids[0], ids[1], ids[2]])
}

fn part(m: &Model, id: Id) -> &BuildingPart {
    match m.elements.get(id) {
        Some(Element::Part(p)) => p,
        other => panic!("{id} is no part: {other:?}"),
    }
}

/// Part `id` as `parts` lists it.
fn listed(s: &mut Session, id: Id) -> Value {
    run(s, "parts", json!({"ids": [id]}))["parts"][0].clone()
}

/// A plane square to the wall at `x`, the pieces east of it its front.
fn across(x: f64) -> Value {
    json!({"cut": {"plane": {"point": [x, 0, 0], "normal": [1, 0, 0]}}})
}

/// The pieces a reply names: each key and its id.
fn pieces(r: &Value) -> Vec<(String, Id)> {
    r["pieces"].as_array().unwrap().iter().map(|p| (p["key"].as_str().unwrap().to_string(), p["id"].as_u64().unwrap())).collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.abs().max(1.0)
}

#[test]
fn a_plane_divides_a_part_into_pieces_that_are_parts_and_whose_sum_it_is() {
    let (mut s, wall, [plaster, ..]) = historic();
    run(&mut s, "part", json!({"id": plaster, "Decay_Pathology": "crust", "SU_Code": "USR 101"}));
    let whole = s.model.part_state(plaster).unwrap();
    let r = run(&mut s, "divide_parts", json!({"id": plaster, "cuts": [across(1500.0)]}));
    let made = pieces(&r);
    assert_eq!(made.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["0.front.0", "0.back.0"], "{r}");
    let (east, west) = (made[0].1, made[1].1);
    // Each a part of the wall's plaster, divided from the plaster, with what was said of it.
    for id in [east, west] {
        let p = part(&s.model, id);
        assert_eq!((p.host, p.parent, p.source), (s.model.guid(wall), Some(s.model.guid(plaster)), part(&s.model, plaster).source));
        assert_eq!(p.heritage, part(&s.model, plaster).heritage);
    }
    // 2.5 m and 1.5 m of a face 3 m tall, 20 mm thick: what the plaster was, between them.
    let (e, w) = (s.model.part_state(east).unwrap(), s.model.part_state(west).unwrap());
    assert!(near(e.net_area.unwrap(), 2500.0 * 3000.0) && near(w.net_area.unwrap(), 1500.0 * 3000.0), "{e:?} {w:?}");
    assert!(near(e.net_volume + w.net_volume, whole.net_volume), "{} + {} against {}", e.net_volume, w.net_volume, whole.net_volume);
    // The plaster is their sum: listed with its divisions, measuring nothing itself.
    let shown = listed(&mut s, plaster);
    assert_eq!(shown["divisions"], json!([east, west]), "{shown}");
    assert!(shown.get("volume_m3").is_none());
    assert_eq!(listed(&mut s, east)["piece"], "0.front.0");
    // The other parts are as they were.
    assert!(part(&s.model, made[0].1).division.is_none());
}

#[test]
fn an_outline_drawn_on_a_face_to_a_depth_takes_a_patch_of_plaster() {
    let (mut s, _, [plaster, ..]) = historic();
    // On the exterior face, looking out north: its x along the wall, its y down (the normal across x). A metre square,
    // from 1 m to 2 m along and up, 10 mm deep.
    let face = json!({"origin": [0, 250, 0], "x": [1, 0, 0], "normal": [0, 1, 0]});
    let square = json!([[1000, -1000], [2000, -1000], [2000, -2000], [1000, -2000]]);
    let r = run(
        &mut s,
        "divide_parts",
        json!({"id": plaster, "cuts": [{"cut": {"sketch": {"frame": face, "points": square, "closed": true, "depth": 10}}}]}),
    );
    let made = pieces(&r);
    assert_eq!(made.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["0.inside.0", "0.outside.0"], "{r}");
    let patch = s.model.part_state(made[0].1).unwrap();
    assert!(near(patch.net_volume, 1000.0 * 1000.0 * 10.0), "{patch:?}");
    assert!(near(patch.net_area.unwrap(), 1000.0 * 1000.0), "{patch:?}");
    // The rest keeps the whole face, less the patch, and what lies behind it.
    let rest = s.model.part_state(made[1].1).unwrap();
    assert!(near(rest.net_volume, 4000.0 * 3000.0 * 20.0 - 1e7), "{rest:?}");
}

#[test]
fn the_pieces_follow_their_wall_as_it_moves_turns_and_stretches() {
    let (mut s, wall, [plaster, ..]) = historic();
    let made = pieces(&run(&mut s, "divide_parts", json!({"id": plaster, "cuts": [across(1500.0)]})));
    let (east, west) = (made[0].1, made[1].1);
    let volume = |s: &Session, id: Id| s.model.part_state(id).unwrap().net_volume;
    let (e0, w0) = (volume(&s, east), volume(&s, west));
    run(&mut s, "move", json!({"ids": [wall], "by": [1000, 500]}));
    run(&mut s, "rotate", json!({"ids": [wall], "about": [0, 0], "angle": 90}));
    assert!(near(volume(&s, east), e0) && near(volume(&s, west), w0));
    // Turned a quarter, the wall runs north from (-500, 1000): the cut 1.5 m up it, the west piece below.
    let tris = s.model.part_solid(part(&s.model, west)).unwrap();
    let top = tris.iter().flatten().map(|q| q[1]).fold(f64::MIN, f64::max);
    assert!(near(top, 2500.0), "{top}");
    // Stretched at its end, the cut stays 1.5 m from its start: the east piece grows.
    run(&mut s, "move", json!({"ids": [wall], "by": [0, 1000], "end": "end"}));
    assert!(near(volume(&s, west), w0) && volume(&s, east) > e0 * 1.3);
}

#[test]
fn a_division_given_again_keeps_the_pieces_it_still_makes_and_none_takes_them_all_back() {
    let (mut s, _, [plaster, ..]) = historic();
    let first = pieces(&run(&mut s, "divide_parts", json!({"id": plaster, "cuts": [across(1500.0)]})));
    run(&mut s, "part", json!({"id": first[1].1, "Intervention_Type": "consolidation"}));
    // A second cut: the west piece cut again, the east one kept with its id; nothing named twice.
    let r = run(&mut s, "divide_parts", json!({"id": plaster, "cuts": [across(1500.0), across(3000.0)]}));
    let again = pieces(&r);
    let keys: Vec<&str> = again.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["0.front.0/1.front.0", "0.front.0/1.back.0", "0.back.0"], "{r}");
    assert_eq!(again[2], first[1].clone());
    assert_eq!(part(&s.model, first[1].1).heritage.intervention_type, "consolidation");
    assert_eq!(r["deleted"], json!([first[0].1]), "{r}");
    // No cuts at all: whole again.
    let r = run(&mut s, "divide_parts", json!({"id": plaster, "cuts": []}));
    assert!(s.model.part_divisions(plaster).is_empty() && part(&s.model, plaster).division.is_none(), "{r}");
    assert!(listed(&mut s, plaster).get("volume_m3").is_some());
    // And undone, divided again as it was.
    run(&mut s, "undo", json!({}));
    assert_eq!(s.model.part_divisions(plaster).len(), 3);
}

#[test]
fn a_cut_that_no_longer_meets_its_part_leaves_its_pieces_in_conflict_and_says_so() {
    let (mut s, wall, [plaster, ..]) = historic();
    let made = pieces(&run(&mut s, "divide_parts", json!({"id": plaster, "cuts": [across(3000.0)]})));
    // Shortened to 2 m, the wall ends before the cut: the plaster stays whole, in one piece no cut made.
    run(&mut s, "move", json!({"ids": [wall], "by": [-2000, 0], "end": "end"}));
    let shown = listed(&mut s, plaster);
    assert!(shown["division_conflicts"][0].as_str().unwrap().contains("no longer meets the part"), "{shown}");
    for (_, id) in &made {
        assert!(s.model.part_state(*id).unwrap().conflict.is_some(), "{}", listed(&mut s, *id));
    }
    // Put right by dividing it again where the wall is: the same pieces, the same parts, out of conflict.
    let r = run(&mut s, "divide_parts", json!({"id": plaster, "cuts": [across(1000.0)]}));
    assert_eq!(pieces(&r), made, "{r}");
    assert!(made.iter().all(|(_, id)| s.model.part_state(*id).unwrap().conflict.is_none()));
    assert!(listed(&mut s, plaster).get("division_conflicts").is_none());
}

#[test]
fn divide_parts_refuses_what_it_cant_divide_and_changes_nothing() {
    let (mut s, wall, [plaster, ..]) = historic();
    for (p, says) in [
        (json!({"id": wall, "cuts": [across(1500.0)]}), "isn't a part"),
        (json!({"id": plaster}), "`cuts`"),
        (json!({"id": plaster, "cuts": [across(9000.0)]}), "no longer meets the part"),
        (json!({"id": plaster, "cuts": [{"cut": {"plane": {"point": [1500, 0, 0]}}}]}), "normal"),
        (json!({"id": plaster, "cuts": [{"cut": {"plane": {"point": [1500, 0, 0], "normal": [0, 0, 0]}}}]}), "normal"),
        (json!({"id": plaster, "cuts": [{"cut": {"plane": {"point": [1e300, 0, 0], "normal": [1, 0, 0]}}}]}), "finite"),
        (json!({"id": plaster, "cuts": [across(1500.0)], "colour": "red"}), "colour"),
        (json!({"id": plaster, "cuts": vec![across(1500.0); 100]}), "cuts"),
    ] {
        let e = fails(&mut s, "divide_parts", p.clone());
        assert!(e.contains(says), "{p}: {e}");
    }
}

#[test]
fn a_file_keeps_a_division_and_its_pieces() {
    let (mut s, _, [plaster, ..]) = historic();
    let made = pieces(&run(&mut s, "divide_parts", json!({"id": plaster, "cuts": [across(1500.0)]})));
    let back = Model::from_json(&s.model.to_json().unwrap()).unwrap();
    assert_eq!(part(&back, plaster).division, part(&s.model, plaster).division);
    for (_, id) in &made {
        assert_eq!(part(&back, *id), part(&s.model, *id));
        assert!(near(back.part_state(*id).unwrap().net_volume, s.model.part_state(*id).unwrap().net_volume));
    }
}

#[test]
fn a_gap_takes_a_joint_out_and_an_offset_moves_a_pieces_face_in() {
    let (mut s, _, [plaster, ..]) = historic();
    let whole = s.model.part_state(plaster).unwrap().net_volume;
    // A 10 mm joint at 1.5 m; the west piece's face on it moved 5 mm further in, named as the reply names it.
    let r = run(
        &mut s,
        "divide_parts",
        json!({"id": plaster, "cuts": [{"cut": {"plane": {"point": [1500, 0, 0], "normal": [1, 0, 0]}}, "gap": 10}],
               "offsets": [{"piece": "0.back.0", "face": {"cut": 0}, "distance": 5}]}),
    );
    let made = pieces(&r);
    let (e, w) = (s.model.part_state(made[0].1).unwrap().net_volume, s.model.part_state(made[1].1).unwrap().net_volume);
    let slice = 3000.0 * 20.0;
    assert!(near(e, 2495.0 * slice) && near(w, 1490.0 * slice), "{e} {w}");
    assert!(near(whole - e - w, 15.0 * slice));
    // A piece that isn't there is refused.
    let e = fails(
        &mut s,
        "divide_parts",
        json!({"id": plaster, "cuts": [across(1500.0)], "offsets": [{"piece": "0.left.0", "face": {"cut": 0}, "distance": 5}]}),
    );
    assert!(e.contains("piece"), "{e}");
    let e = fails(
        &mut s,
        "divide_parts",
        json!({"id": plaster, "cuts": [across(1500.0)], "offsets": [{"piece": "zero", "face": {"cut": 0}, "distance": 5}]}),
    );
    assert!(e.contains("zero"), "{e}");
}
