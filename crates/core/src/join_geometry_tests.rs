//! Join Geometry in the project (T23): two of the walls, floors, roofs and ceilings that meet are joined by `join_geometry`,
//! and the first cuts the second, which loses what the first's own solids take of it (in section and in 3D); the IFC keeps
//! both bodies whole and says they're joined (`IfcRelConnectsElements`, which clips nothing). `switch_join_order` flips which
//! cuts which and `unjoin_geometry` takes the join off. The cut element names its cutter in `joined_by`: saved, undone and
//! redone, followed by copies and by Insert from File where both come along, gone with either element. Two that only
//! touch join (a shared face); two that don't meet are refused, and two an edit parts stay joined, cutting nothing, with a
//! warning. A join or a switch that would close a loop of joins is refused.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::face_void_tests::{filled, id_of, ifc, plumb, run};
use crate::model::{FORMAT, Id};

/// A wall along x at y 2000, from 0 to 6000, 3000 high, of the starter type (200 thick: y 1900 to 2100).
fn wall(s: &mut Session) -> Id {
    run(s, "wall", json!({"points": [[0, 2000], [6000, 2000]], "height": 3000}))["ids"][0].as_u64().unwrap()
}

/// A floor 6000 by 4000 of the starter 200 mm type with its top `offset` over the level: at 300 it runs from 100 to 300 up,
/// through the wall's foot; at 0, from -200 to 0, under it, touching it.
fn floor(s: &mut Session, offset: f64) -> Id {
    id_of(&run(s, "floor", json!({"boundary": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]], "offset": offset})))
}

fn join(s: &mut Session, first: Id, second: Id) -> Result<Value, String> {
    s.execute("join_geometry", &json!({"first": first, "second": second}))
}

/// The elements `id` is cut by, as its `joined_by` names them.
fn joined(s: &Session, id: Id) -> Vec<Id> {
    let m = &s.model;
    m.elements.get(id).map(|e| e.joined_by().iter().filter_map(|g| m.elements.id(*g)).collect()).unwrap_or_default()
}

/// A wall and a floor through its foot: (wall, floor).
fn crossing() -> (Session, Id, Id) {
    let mut s = Session::new();
    let w = wall(&mut s);
    let f = floor(&mut s, 300.0);
    (s, w, f)
}

/// Whether `owner` is filled at (x, z) in the section along y `y` (+10, off the centre line), looking north.
fn solid(s: &mut Session, owner: Id, y: f64, x: f64, z: f64) -> bool {
    filled(s, owner, [-1000.0, y + 10.0], [8000.0, y + 10.0])(x + 1000.0, z)
}

#[test]
fn the_first_cuts_the_second_in_section_and_in_3d_and_switch_and_unjoin_take_it_back() {
    let (mut s, w, f) = crossing();
    // Before: the floor runs through the wall's foot, both filled there.
    assert!(solid(&mut s, f, 2000.0, 3000.0, 200.0));
    assert!(solid(&mut s, w, 2000.0, 3000.0, 200.0));
    assert_eq!(plumb(&s, f, 3000.0, 2000.0).len(), 2, "the floor's top and underside, under the wall");
    let r = join(&mut s, w, f).unwrap();
    assert_eq!(r["first"], json!(w));
    assert_eq!(r["second"], json!(f));
    assert_eq!(joined(&s, f), vec![w]);
    assert!(joined(&s, w).is_empty());
    // The wall cuts the floor: under it, only the wall; away from it, the floor as it was.
    assert!(!solid(&mut s, f, 2000.0, 3000.0, 200.0), "the floor is cut where the wall stands");
    assert!(solid(&mut s, w, 2000.0, 3000.0, 200.0));
    assert!(solid(&mut s, f, 1000.0, 3000.0, 200.0), "and whole away from it");
    assert!(plumb(&s, f, 3000.0, 2000.0).is_empty(), "no floor under the wall in 3D");
    assert_eq!(plumb(&s, f, 3000.0, 1000.0).len(), 2);
    // Switched, the floor cuts the wall: the wall's foot is gone through the floor's thickness, its body above kept.
    run(&mut s, "switch_join_order", json!({"first": w, "second": f}));
    assert_eq!(joined(&s, w), vec![f]);
    assert!(joined(&s, f).is_empty());
    assert!(!solid(&mut s, w, 2000.0, 3000.0, 200.0), "the wall is cut through the floor");
    assert!(solid(&mut s, w, 2000.0, 3000.0, 1200.0));
    assert!(solid(&mut s, f, 2000.0, 3000.0, 200.0));
    assert_eq!(plumb(&s, f, 3000.0, 2000.0).len(), 2);
    // Unjoined (either order names the pair): both whole again.
    run(&mut s, "unjoin_geometry", json!({"first": f, "second": w}));
    assert!(joined(&s, w).is_empty() && joined(&s, f).is_empty());
    assert!(solid(&mut s, w, 2000.0, 3000.0, 200.0) && solid(&mut s, f, 2000.0, 3000.0, 200.0));
}

#[test]
fn a_join_is_saved_undone_and_redone_and_the_file_is_of_format_21() {
    let (mut s, w, f) = crossing();
    join(&mut s, w, f).unwrap();
    let path = std::env::temp_dir().join(format!("bimcraft-join-{}.bimc", std::process::id()));
    run(&mut s, "save", json!({"path": path}));
    let text = std::fs::read_to_string(&path).unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    // FORMAT 21: a reader of 20 refuses the file (it would open the floor whole, the join left out), as it refuses any newer.
    assert_eq!(FORMAT, 21);
    assert_eq!(file["format"], json!(21));
    let mut older = file.clone();
    older["format"] = json!(20);
    assert!(crate::model::Model::read(&older.to_string()).is_ok(), "a file of 20 needs no step");
    assert!(text.contains("joined_by"));
    let mut t = Session::new();
    run(&mut t, "open", json!({"path": path}));
    let _ = std::fs::remove_file(&path);
    assert_eq!(joined(&t, f), vec![w]);
    assert!(!solid(&mut t, f, 2000.0, 3000.0, 200.0));
    run(&mut s, "undo", json!({}));
    assert!(joined(&s, f).is_empty());
    run(&mut s, "redo", json!({}));
    assert_eq!(joined(&s, f), vec![w]);
}

#[test]
fn two_that_touch_join_and_two_that_dont_meet_are_refused() {
    // A floor under the wall's foot, sharing its face: a join, which cuts nothing.
    let mut s = Session::new();
    let w = wall(&mut s);
    let f = floor(&mut s, 0.0);
    join(&mut s, f, w).unwrap();
    assert_eq!(joined(&s, w), vec![f]);
    assert!(solid(&mut s, w, 2000.0, 3000.0, 100.0));
    // A floor across the site from it: refused, nothing joined.
    let far = id_of(&run(&mut s, "floor", json!({"boundary": [[20000, 0], [26000, 0], [26000, 4000], [20000, 4000]], "offset": 300})));
    let e = join(&mut s, w, far).unwrap_err();
    assert!(e.contains("don't meet"), "{e}");
    assert!(joined(&s, far).is_empty());
}

#[test]
fn join_refuses_itself_what_isnt_a_wall_floor_roof_or_ceiling_and_the_same_pair_twice() {
    let (mut s, w, f) = crossing();
    let e = join(&mut s, w, w).unwrap_err();
    assert!(e.contains("itself"), "{e}");
    let door = id_of(&run(&mut s, "door", json!({"at": [1000, 2000]})));
    let e = join(&mut s, door, f).unwrap_err();
    assert!(e.contains("wall, floor, roof or ceiling"), "{e}");
    let e = join(&mut s, w, 100000).unwrap_err();
    assert!(e.contains("wall, floor, roof or ceiling"), "{e}");
    join(&mut s, w, f).unwrap();
    // Joined already, either way round: switch_join_order flips it.
    for (a, b) in [(w, f), (f, w)] {
        let e = join(&mut s, a, b).unwrap_err();
        assert!(e.contains("already joined") && e.contains("switch_join_order"), "{e}");
    }
    let lone = id_of(&run(&mut s, "floor", json!({"boundary": [[20000, 0], [26000, 0], [26000, 4000], [20000, 4000]]})));
    for c in ["unjoin_geometry", "switch_join_order"] {
        let e = s.execute(c, &json!({"first": w, "second": lone})).unwrap_err();
        assert!(e.contains("aren't joined"), "{c}: {e}");
    }
    let e = s.execute("join_geometry", &json!({"first": w})).unwrap_err();
    assert!(e.contains("`second`"), "{e}");
    let e = s.execute("join_geometry", &json!({"first": w, "second": f, "third": 1})).unwrap_err();
    assert!(e.contains("third"), "{e}");
}

#[test]
fn a_join_or_a_switch_that_would_close_a_loop_is_refused_and_what_three_share_stays() {
    // The wall cuts the floor, and the floor a wall across both. The one across cutting the first wall would close a loop:
    // each cut by the others' own solids, the 200 cube all three share at the crossing would be taken out of all three.
    let (mut s, w, f) = crossing();
    let across = run(&mut s, "wall", json!({"points": [[3000, 0], [3000, 4000]], "height": 3000}))["ids"][0].as_u64().unwrap();
    join(&mut s, w, f).unwrap();
    join(&mut s, f, across).unwrap();
    let e = join(&mut s, across, w).unwrap_err();
    assert!(e.contains("loop"), "{e}");
    let names = [w, f, across, w].map(|id| s.model.shown(id)).join(" cuts ");
    assert!(e.contains(&names), "{e}");
    assert!(joined(&s, w).is_empty());
    // The shared cube is still there, the first wall's: a plumb line through the crossing meets its foot and its top.
    assert_eq!(plumb(&s, w, 3000.0, 2000.0), vec![0.0, 3000.0]);
    // Joined the other way round, there is no loop: the first wall cuts the one across too.
    join(&mut s, w, across).unwrap();
    // Switched, the one across would cut the first wall, which cuts the floor, which cuts the one across: a loop again.
    let e = s.execute("switch_join_order", &json!({"first": w, "second": across})).unwrap_err();
    assert!(e.contains("loop"), "{e}");
    assert_eq!(joined(&s, across), vec![f, w]);
    // Switching the floor and the one across makes none: both walls cut the floor.
    run(&mut s, "switch_join_order", json!({"first": f, "second": across}));
    assert_eq!((joined(&s, f), joined(&s, across)), (vec![w, across], vec![w]));
}

#[test]
fn a_pair_an_edit_parts_stays_joined_cuts_nothing_and_is_warned_of() {
    let (mut s, w, f) = crossing();
    join(&mut s, w, f).unwrap();
    assert!(s.model.warnings().iter().all(|x| !x.contains("no longer meet")));
    // The wall moved far off: the model still checks, the join stays, and the floor is whole.
    run(&mut s, "move", json!({"ids": [w], "by": [0, 20000]}));
    s.model.check().unwrap();
    assert_eq!(joined(&s, f), vec![w]);
    assert_eq!(plumb(&s, f, 3000.0, 2000.0).len(), 2);
    let warned: Vec<String> = s.model.warnings().into_iter().filter(|x| x.contains("no longer meet")).collect();
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(warned[0].contains("unjoin_geometry"), "{}", warned[0]);
    // Back: it cuts again.
    run(&mut s, "move", json!({"ids": [w], "by": [0, -20000]}));
    assert!(plumb(&s, f, 3000.0, 2000.0).is_empty());
}

#[test]
fn copies_keep_a_join_when_both_are_copied_and_deleting_either_takes_it_off() {
    let (mut s, w, f) = crossing();
    join(&mut s, w, f).unwrap();
    // Both copied: the copy of the floor is cut by the copy of the wall.
    let made: Vec<Id> = run(&mut s, "copy", json!({"ids": [w, f], "by": [0, 10000]}))["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    let of = |s: &Session, cat: &str| *made.iter().find(|id| s.model.elements.get(**id).unwrap().category() == cat).unwrap();
    let (w2, f2) = (of(&s, "wall"), of(&s, "floor"));
    assert_eq!(joined(&s, f2), vec![w2]);
    assert!(plumb(&s, f2, 3000.0, 12000.0).is_empty());
    // The floor alone: its copy is joined to nothing.
    let lone = run(&mut s, "copy", json!({"ids": [f], "by": [10000, 0]}))["ids"][0].as_u64().unwrap();
    assert!(joined(&s, lone).is_empty());
    // Deleting the cutter takes the join off the floor; deleting a cut floor leaves its cutter as it was.
    run(&mut s, "delete", json!({"ids": [w]}));
    assert!(joined(&s, f).is_empty());
    s.model.check().unwrap();
    run(&mut s, "delete", json!({"ids": [f2]}));
    assert!(s.model.elements.get(w2).is_some_and(|e| e.joined_by().is_empty()));
    s.model.check().unwrap();
}

#[test]
fn insert_from_file_keeps_a_join_when_both_come_in_and_drops_it_when_one_does() {
    let (mut s, w, f) = crossing();
    join(&mut s, w, f).unwrap();
    let path = std::env::temp_dir().join(format!("bimcraft-join-insert-{}.bimc", std::process::id()));
    run(&mut s, "save", json!({"path": path}));
    // The floor alone: it comes in joined to nothing, and the wall doesn't come in with it.
    let mut t = Session::new();
    let r = run(&mut t, "insert", json!({"path": path, "ids": [f]}));
    let here = r["ids"][f.to_string()].as_u64().unwrap();
    assert!(joined(&t, here).is_empty());
    assert!(!t.model.elements.values().any(|e| e.category() == "wall"));
    // Both: joined here as there.
    let mut t = Session::new();
    let r = run(&mut t, "insert", json!({"path": path, "ids": [w, f]}));
    let (w2, f2) = (r["ids"][w.to_string()].as_u64().unwrap(), r["ids"][f.to_string()].as_u64().unwrap());
    assert_eq!(joined(&t, f2), vec![w2]);
    // The file into itself: copies, the copy of the floor joined to the copy of the wall, not to the wall.
    let r = run(&mut s, "insert", json!({"path": path}));
    let (w3, f3) = (r["ids"][w.to_string()].as_u64().unwrap(), r["ids"][f.to_string()].as_u64().unwrap());
    assert_ne!(w3, w);
    assert_eq!(joined(&s, f3), vec![w3]);
    assert_eq!(joined(&s, f), vec![w]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_ifc_keeps_both_bodies_and_says_they_are_joined() {
    let (mut s, w, f) = crossing();
    let before = ifc(&mut s);
    assert!(!before.contains("IFCRELCONNECTSELEMENTS("));
    join(&mut s, w, f).unwrap();
    let text = ifc(&mut s);
    let rels: Vec<&str> = text.lines().filter(|l| l.contains("=IFCRELCONNECTSELEMENTS(")).collect();
    assert_eq!(rels.len(), 1, "{rels:?}");
    // It relates the wall (relating, the one that cuts) and the slab (related), and clips neither: no opening voids the slab.
    let entity = |class: &str| text.lines().find(|l| l.contains(&format!("={class}("))).unwrap().split('=').next().unwrap().to_string();
    let (wall_ref, slab_ref) = (entity("IFCWALL"), entity("IFCSLAB"));
    let args: Vec<&str> = rels[0].trim_end_matches(");").split(',').map(str::trim).collect();
    assert_eq!(&args[args.len() - 2..], &[wall_ref.as_str(), slab_ref.as_str()], "{}", rels[0]);
    assert!(!text.contains("IFCOPENINGELEMENT("));
}

#[test]
fn a_file_naming_too_many_joins_is_refused() {
    let (mut s, w, f) = crossing();
    join(&mut s, w, f).unwrap();
    let path = std::env::temp_dir().join(format!("bimcraft-join-many-{}.bimc", std::process::id()));
    run(&mut s, "save", json!({"path": path}));
    let mut file: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // The floor's `joined_by`, wherever the file holds it, made one longer than may be.
    fn lengthen(v: &mut Value) -> bool {
        match v {
            Value::Object(o) => match o.get_mut("joined_by").and_then(Value::as_array_mut) {
                Some(list) => {
                    let one = list[0].clone();
                    *list = vec![one; crate::model::MAX_JOINS + 1];
                    true
                }
                None => o.values_mut().any(lengthen),
            },
            Value::Array(a) => a.iter_mut().any(lengthen),
            _ => false,
        }
    }
    assert!(lengthen(&mut file), "the join was found in the file");
    std::fs::write(&path, file.to_string()).unwrap();
    let mut t = Session::new();
    let e = t.execute("open", &json!({"path": path})).unwrap_err();
    let _ = std::fs::remove_file(&path);
    assert!(e.contains("joined_by") && e.contains(&crate::model::MAX_JOINS.to_string()), "{e}");
}

#[test]
fn join_and_switch_refuse_an_element_cut_by_max_joins_already() {
    let (mut s, w, f) = crossing();
    // Floors far off, each cutting the wall as the file would say: as many as one may be cut by.
    let others: Vec<Id> = (0..crate::model::MAX_JOINS)
        .map(|i| {
            let x = 20000.0 + i as f64 * 2000.0;
            id_of(&run(&mut s, "floor", json!({"boundary": [[x, 0], [x + 1000.0, 0], [x + 1000.0, 1000.0], [x, 1000.0]]})))
        })
        .collect();
    let guids: Vec<_> = others.iter().map(|id| s.model.guid(*id)).collect();
    if let Some(list) = s.model_mut().elements.get_mut(w).and_then(crate::model::Element::joined_by_mut) {
        *list = guids;
    }
    s.model.check().unwrap();
    // The floor cuts the wall: refused, the wall is full.
    let e = join(&mut s, f, w).unwrap_err();
    assert!(e.contains("cut by 100 joined elements already"), "{e}");
    // The wall cuts the floor; switched, the wall would be cut by one more: refused before anything changes.
    join(&mut s, w, f).unwrap();
    let e = s.execute("switch_join_order", &json!({"first": w, "second": f})).unwrap_err();
    assert!(e.contains("cut by 100 joined elements already"), "{e}");
    assert_eq!(joined(&s, f), vec![w]);
    assert_eq!(joined(&s, w).len(), crate::model::MAX_JOINS);
}

#[test]
fn a_long_list_on_a_later_element_is_refused_by_its_own_cap_not_read_through() {
    // The wall (the lower id) names the floor; the floor names the wall more times than may be. The wall's check reads the
    // floor's list only within the cap, so the floor's own check refuses it, for its length.
    let (mut s, w, f) = crossing();
    let (gw, gf) = (s.model.guid(w), s.model.guid(f));
    assert!(w < f);
    let m = s.model_mut();
    if let Some(list) = m.elements.get_mut(w).and_then(crate::model::Element::joined_by_mut) {
        *list = vec![gf];
    }
    if let Some(list) = m.elements.get_mut(f).and_then(crate::model::Element::joined_by_mut) {
        *list = vec![gw; 50 * crate::model::MAX_JOINS];
    }
    let e = s.model.check().unwrap_err();
    assert!(e.contains(&format!("floor {f}")) && e.contains("no more than 100"), "{e}");
}

/// A curtain wall of type `CW` (one 2000 column, one 1000 row) where `wall` stands: along x at y 2000, 0 to 6000, 3000 high.
fn curtain_wall(s: &mut Session) -> Id {
    run(s, "type", json!({"category": "wall", "name": "CW", "curtain": {"columns": {"segments": [2000]}, "rows": {"segments": [1000]}}}));
    run(s, "wall", json!({"points": [[0, 2000], [6000, 2000]], "type": "CW", "height": 3000}))["ids"][0].as_u64().unwrap()
}

#[test]
fn a_curtain_wall_and_a_wall_in_its_panel_are_refused_and_unjoin_still_takes_one_off() {
    let mut s = Session::new();
    let cw = curtain_wall(&mut s);
    let f = floor(&mut s, 300.0);
    // Its own solid is never drawn, only its panels and mullions: what it would cut out of the floor is a block no one sees.
    for (a, b) in [(cw, f), (f, cw)] {
        let e = join(&mut s, a, b).unwrap_err();
        assert!(e.contains("is a curtain wall"), "{e}");
    }
    assert!(!s.model.joins_geometry(cw) && s.model.joins_geometry(f));
    // A wall standing in a panel's place is its curtain wall's.
    let panel = run(&mut s, "curtain_panel_wall", json!({"wall": cw, "column": 0, "row": 0}))["id"].as_u64().unwrap();
    let e = join(&mut s, panel, f).unwrap_err();
    assert!(e.contains("curtain wall's panel"), "{e}");
    // A plain wall joined first, its type then made a curtain one: the join cuts nothing, `warnings` says so, Switch is
    // refused and Unjoin takes it off.
    let w = wall(&mut s);
    join(&mut s, w, f).unwrap();
    assert!(!solid(&mut s, f, 2000.0, 1000.0, 200.0), "the plain wall cuts the floor");
    run(&mut s, "modify", json!({"id": w, "type": "CW", "location": "exterior-finish"}));
    assert!(solid(&mut s, f, 2000.0, 1000.0, 200.0), "a curtain wall cuts nothing");
    assert!(s.model.warnings().iter().any(|w| w.contains("is a curtain wall")), "{:?}", s.model.warnings());
    assert!(s.execute("switch_join_order", &json!({"first": w, "second": f})).unwrap_err().contains("is a curtain wall"));
    run(&mut s, "unjoin_geometry", json!({"first": w, "second": f}));
    assert!(joined(&s, f).is_empty());
}

#[test]
fn a_file_with_a_join_of_a_curtain_wall_opens_without_it_and_says_so() {
    let mut s = Session::new();
    let cw = curtain_wall(&mut s);
    let f = floor(&mut s, 300.0);
    // As a build before they were refused could save it: the floor cut by the curtain wall.
    let g = s.model.guid(cw);
    let mut file: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    file["elements"][f.to_string()]["joined_by"] = json!([g]);
    let (m, left_out) = crate::model::Model::read(&file.to_string()).unwrap();
    assert!(m.elements.get(f).unwrap().joined_by().is_empty(), "the join is left out");
    assert_eq!(left_out, vec![format!("/elements/{f}/joined_by/0")]);
}

#[test]
fn the_ifc_leaves_out_a_join_kept_after_its_wall_became_a_curtain_one() {
    // Either way round: the wall cutting the floor, and the floor cutting the wall.
    for wall_cuts in [true, false] {
        let (mut s, w, f) = crossing();
        run(
            &mut s,
            "type",
            json!({"category": "wall", "name": "CW", "curtain": {"columns": {"segments": [2000]}, "rows": {"segments": [1000]}}}),
        );
        let (a, b) = if wall_cuts { (w, f) } else { (f, w) };
        join(&mut s, a, b).unwrap();
        let rels = |s: &mut Session| ifc(s).lines().filter(|l| l.contains("=IFCRELCONNECTSELEMENTS(")).count();
        assert_eq!(rels(&mut s), 1);
        // The wall's type made a curtain one: the join is kept, cutting nothing in 3D or a section, and goes out of the IFC.
        run(&mut s, "modify", json!({"id": w, "type": "CW", "location": "exterior-finish"}));
        assert_eq!(joined(&s, b), vec![a]);
        assert_eq!(rels(&mut s), 0, "wall cuts: {wall_cuts}");
    }
}
