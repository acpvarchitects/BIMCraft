//! FE-15: a family's model lines, 3D lines drawn on one of its planes (or a reference line's face) that show in every view,
//! their look their subcategory's. Kept by the `family` command (`model_lines`, `model_line`, `delete_model_line`), checked as
//! a reference line is, given an id and a GUID as parts are; they host nothing.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::{CENTER_FRONT_BACK, PlaneRef, REF_LEVEL};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn err(s: &mut Session, id: &str, p: Value) -> String {
    match s.execute(id, &p) {
        Ok(r) => panic!("{id} {p} should have been refused, and did {r}"),
        Err(e) => e,
    }
}

const SHELF: &str = "1b2c3d4e-5f60-4a71-8b92-a3b4c5d6e7f8";
const EDGE: &str = "3d4e5f60-7182-4c93-8db4-c5d6e7f8091a";

/// A generic family `P` with one box, a plane of its own, Shelf, 500 above Ref. Level, and the model lines `lines`.
fn family(s: &mut Session, lines: Value) {
    run(
        s,
        "family",
        json!({"name": "P", "category": "generic", "types": {"A": {}},
               "forms": [{"profile": [[0, 0], [100, 0], [100, 50], [0, 50]], "to": 10}],
               "ref_planes": [{"guid": SHELF, "name": "Shelf", "start": [-1000, 0, 500], "end": [1000, 0, 500], "up": [0, 1, 0]}],
               "model_lines": lines}),
    );
}

#[test]
fn model_lines_are_kept_on_their_host_with_an_id_and_a_guid() {
    let mut s = Session::new();
    family(
        &mut s,
        json!([{"start": [0, -300, 0], "end": [400, -300, 0]},
               {"guid": EDGE, "start": [-200, 0, 100], "end": [200, 0, 300], "host": {"plane": CENTER_FRONT_BACK.to_string()}}]),
    );
    let f = &s.model.families["P"];
    assert_eq!(f.model_lines.len(), 2);
    assert_eq!(f.model_lines[0].host, PlaneRef::Plane(REF_LEVEL), "Ref. Level when none is given");
    assert_eq!(f.model_lines[1].guid.to_string(), EDGE, "a GUID given is kept");
    assert!(f.model_lines.iter().all(|l| l.id != 0 && !l.guid.is_nil()), "each has an id and a GUID");
    assert!(f.lines.is_empty(), "not reference lines");
    // Saved and read back as they were.
    let text = s.model.to_json().unwrap();
    let back = crate::model::Model::read(&text).unwrap();
    assert_eq!(back.0.families["P"].model_lines, f.model_lines);
}

#[test]
fn a_family_without_model_lines_writes_none() {
    let mut s = Session::new();
    family(&mut s, json!([]));
    let v = serde_json::to_value(&s.model.families["P"]).unwrap();
    assert!(v.get("model_lines").is_none(), "{v}");
}

#[test]
fn a_model_line_off_its_host_or_of_one_point_or_a_foreign_subcategory_is_refused() {
    let mut s = Session::new();
    family(&mut s, json!([]));
    let e = err(&mut s, "family", json!({"name": "P", "model_lines": [{"start": [0, 0, 10], "end": [100, 0, 10]}]}));
    assert!(e.contains("model line 1") && e.contains("doesn't lie on"), "{e}");
    let e = err(&mut s, "family", json!({"name": "P", "model_lines": [{"start": [5, 5, 0], "end": [5, 5, 0]}]}));
    assert!(e.contains("model line 1") && e.contains("one point"), "{e}");
    let e = err(&mut s, "family", json!({"name": "P", "model_lines": [{"start": [0, 0, 0], "end": [9e12, 0, 0]}]}));
    assert!(e.contains("model line 1"), "{e}");
    let e =
        err(&mut s, "family", json!({"name": "P", "model_lines": [{"start": [0, 0, 0], "end": [100, 0, 0], "subcategory": "Nonsense"}]}));
    assert!(e.contains("model line 1") && e.contains("subcategory"), "{e}");
    assert!(s.model.families["P"].model_lines.is_empty(), "nothing kept");
}

#[test]
fn a_model_line_on_shelf_keeps_shelf_and_patched_and_deleted_by_id() {
    let mut s = Session::new();
    family(&mut s, json!([{"start": [0, 0, 500], "end": [300, 0, 500], "host": {"plane": SHELF}}]));
    // Shelf holds a line: it can't go.
    let e = err(&mut s, "family", json!({"name": "P", "delete_ref_plane": [SHELF]}));
    assert!(e.contains("model line 1"), "{e}");
    let id = s.model.families["P"].model_lines[0].id;
    run(&mut s, "family", json!({"name": "P", "model_line": {"id": id, "end": [600, 0, 500]}}));
    assert_eq!(s.model.families["P"].model_lines[0].end, [600.0, 0.0, 500.0]);
    run(&mut s, "family", json!({"name": "P", "delete_model_line": [id]}));
    assert!(s.model.families["P"].model_lines.is_empty());
    run(&mut s, "family", json!({"name": "P", "delete_ref_plane": [SHELF]}));
}

#[test]
fn a_model_line_hosts_nothing() {
    let mut s = Session::new();
    family(&mut s, json!([{"guid": EDGE, "start": [0, -300, 0], "end": [400, -300, 0]}]));
    let e = err(
        &mut s,
        "family",
        json!({"name": "P", "model_lines": [{"guid": EDGE, "start": [0, -300, 0], "end": [400, -300, 0]},
                                            {"start": [0, -300, 0], "end": [0, -300, 400], "host": {"line": EDGE, "face": "vertical"}}]}),
    );
    assert!(e.contains("model line 2"), "{e}");
}
