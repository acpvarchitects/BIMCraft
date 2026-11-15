//! FE-15, part 2 (FORMAT 21): a family's model line as a dimension's reference, as a reference line is. `{"model_line": guid}`
//! is the line seen edge on, the plane through it square to its host; `{"model_line": guid, "point": "start" | "end"}` one of
//! its ends. A dimension labelled with a parameter, or locked, moves the line on its host as it moves a reference line, and
//! the project draws it where the type puts it; a pinned one refuses to be moved.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::{CENTER_FRONT_BACK, CENTER_LEFT_RIGHT, REF_LEVEL};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

const LINE: &str = "6a7b8c9d-0e1f-4a2b-9c3d-4e5f60718293";
const EDGE: &str = "7b8c9d0e-1f2a-4b3c-8d4e-5f6071829304";

/// A generic family `P` with one box, a parameter `D` (300; type B 500), a model line `LINE` on Ref. Level along x at y -300,
/// pinned or not, and `dimensions`.
fn family(pinned: bool, dimensions: Value) -> Value {
    json!({"name": "P", "category": "generic", "parameters": [{"name": "D", "value": 300}], "types": {"A": {}, "B": {"D": 500}},
           "forms": [{"profile": [[0, 0], [100, 0], [100, 50], [0, 50]], "to": 10}],
           "ref_planes": [{"guid": EDGE, "name": "Edge", "start": [-1000, -300, 0], "end": [1000, -300, 0], "up": [0, 0, 1]}],
           "model_lines": [{"guid": LINE, "start": [0, -300, 0], "end": [400, -300, 0], "pinned": pinned}],
           "dimensions": dimensions})
}

/// Model line `LINE`'s ends as type `t` puts them, and as the project draws a placed `P` of it.
fn ends(s: &Session, t: &str) -> ([[f64; 3]; 2], [[f64; 3]; 2]) {
    let f = &s.model.families["P"];
    let values = f.values(t, &Default::default()).unwrap();
    let seg = f.model_line_segments(&values, None, false);
    let parts = f.model_line_parts(&values);
    (seg[0].0, parts[0].0)
}

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

#[test]
fn a_dimension_to_a_model_line_saves_as_21_and_reads_back() {
    let mut s = Session::new();
    let dims = json!([{"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, {"model_line": LINE}], "at": [600, -150, 0]},
                      {"view": "plan", "refs": [{"plane": CENTER_LEFT_RIGHT.to_string()}, {"model_line": LINE, "point": "end"}], "at": [200, -600, 0]}]);
    run(&mut s, "family", family(false, dims));
    let f = &s.model.families["P"];
    let v = serde_json::to_value(&f.dimensions).unwrap();
    assert_eq!(v[0]["refs"][1], json!({"model_line": LINE}), "{v}");
    assert_eq!(v[1]["refs"][1], json!({"model_line": LINE, "point": "end"}), "{v}");
    let text = s.model.to_json().unwrap();
    let saved: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(saved["format"], json!(21), "a model line as a reference is FORMAT 21's");
    let back = crate::model::Model::read(&text).unwrap();
    assert_eq!(back.0.families["P"].dimensions, f.dimensions);
    // What it measures: 300 from Center (Front/Back), 400 from Center (Left/Right) to the line's end.
    let values = f.values("A", &Default::default()).unwrap();
    let measured = f.measured(&values).unwrap();
    assert!((measured[0][0] - 300.0).abs() < 1e-6 && (measured[1][0] - 400.0).abs() < 1e-6, "{measured:?}");
}

#[test]
fn a_labelled_dimension_moves_a_model_line_on_its_host_in_the_family_and_the_project() {
    let mut s = Session::new();
    let dims = json!([{"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, {"model_line": LINE}], "at": [600, -150, 0], "label": "D"}]);
    run(&mut s, "family", family(false, dims));
    let (a, pa) = ends(&s, "A");
    assert!(close(a[0], [0.0, -300.0, 0.0]) && close(a[1], [400.0, -300.0, 0.0]), "{a:?}");
    assert_eq!(a, pa);
    let (b, pb) = ends(&s, "B");
    assert!(close(b[0], [0.0, -500.0, 0.0]) && close(b[1], [400.0, -500.0, 0.0]), "with D 500, 500 from the centre: {b:?}");
    assert_eq!(b, pb, "the project draws it where the type puts it");
}

#[test]
fn a_model_line_aligned_and_locked_to_a_plane_follows_it() {
    let mut s = Session::new();
    let dims = json!([
        {"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, {"plane": EDGE}], "at": [-600, -150, 0], "label": "D"},
        {"view": "plan", "refs": [{"plane": EDGE}, {"model_line": LINE}], "at": [200, -300, 0], "locked": [{"segment": 0, "length": 0}], "alignment": true}]);
    run(&mut s, "family", family(false, dims));
    let (b, pb) = ends(&s, "B");
    assert!(close(b[0], [0.0, -500.0, 0.0]) && close(b[1], [400.0, -500.0, 0.0]), "on Edge, 500 from the centre: {b:?}");
    assert_eq!(b, pb);
}

#[test]
fn a_labelled_dimension_to_a_model_lines_end_stretches_it_on_its_host() {
    let mut s = Session::new();
    let mut def = family(false, json!([]));
    def["parameters"] = json!([{"name": "W", "value": 400}]);
    def["types"] = json!({"A": {}, "B": {"W": 700}});
    def["dimensions"] = json!([{"view": "plan", "refs": [{"plane": CENTER_LEFT_RIGHT.to_string()}, {"model_line": LINE, "point": "end"}], "at": [200, -600, 0], "label": "W"}]);
    run(&mut s, "family", def);
    let (b, pb) = ends(&s, "B");
    assert!((b[1][0] - 700.0).abs() < 1e-6, "its end 700 from the centre: {b:?}");
    assert!(b.iter().all(|p| p[2].abs() < 1e-6), "still on Ref. Level: {b:?}");
    assert_eq!(b, pb);
}

#[test]
fn a_pinned_model_line_is_not_moved_by_a_dimension() {
    let mut s = Session::new();
    let dims = json!([{"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, {"model_line": LINE}], "at": [600, -150, 0], "label": "D"}]);
    let e = s.execute("family", &family(true, dims)).map(|_| ()).unwrap_err();
    assert!(e.contains("dimension 1") && e.contains("model line 1 is pinned"), "{e}");
    assert!(!s.model.families.contains_key("P"), "nothing kept");
    // A dimension that only measures it is fine.
    let dims = json!([{"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, {"model_line": LINE}], "at": [600, -150, 0]}]);
    run(&mut s, "family", family(true, dims));
}

#[test]
fn deleting_a_model_line_takes_it_from_its_dimensions() {
    let mut s = Session::new();
    let dims = json!([{"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, {"model_line": LINE}], "at": [600, -150, 0], "label": "D"},
                      {"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, {"plane": EDGE}, {"model_line": LINE}], "at": [-600, -150, 0]}]);
    run(&mut s, "family", family(false, dims));
    let out = run(&mut s, "family", json!({"name": "P", "delete_model_line": [LINE]}));
    let f = &s.model.families["P"];
    assert!(f.model_lines.is_empty());
    assert_eq!(f.dimensions.len(), 1, "the one with two references left keeps them: {out}");
    assert!(f.dimensions[0].refs.iter().all(|r| !r.names(serde_json::from_value(json!(LINE)).unwrap())));
    assert!(f.dangling_dimensions().is_empty());
}

#[test]
fn a_dimension_to_a_model_line_the_family_has_not_is_refused() {
    let mut s = Session::new();
    let dims = json!([{"view": "plan", "refs": [{"plane": REF_LEVEL.to_string()}, {"model_line": EDGE}], "at": [600, -150, 0]}]);
    let e = s.execute("family", &family(false, dims)).map(|_| ()).unwrap_err();
    assert!(e.contains("dimension 1") && e.contains("no longer has"), "{e}");
    let dims = json!([{"view": "plan", "refs": [{"plane": REF_LEVEL.to_string()}, {"model_line": LINE, "point": "centre"}], "at": [600, -150, 0]}]);
    let e = s.execute("family", &family(false, dims)).map(|_| ()).unwrap_err();
    assert!(e.contains("centre"), "{e}");
    let dims = json!([{"view": "plan", "refs": [{"model_line": LINE, "face": "vertical"}], "at": [600, -150, 0]}]);
    assert!(s.execute("family", &family(false, dims)).is_err());
}

/// For the hostile sweep (`tests::hostile_params_never_panic_or_break_the_model`): `family` payloads with model-line
/// references that name nothing or a point a line hasn't.
pub(crate) fn sweep_payloads() -> Vec<(&'static str, Value)> {
    let bad = |r: Value| {
        (
            "family",
            family(false, json!([{"view": "plan", "refs": [{"plane": CENTER_FRONT_BACK.to_string()}, r], "at": [0, 0, 0], "label": "D"}])),
        )
    };
    vec![
        bad(json!({"model_line": "00000000-0000-4000-8000-000000000001"})),
        bad(json!({"model_line": LINE, "point": "middle"})),
        bad(json!({"model_line": LINE, "point": "centre"})),
        bad(json!({"model_line": 5, "point": 7})),
        bad(json!({"model_line": LINE, "line": LINE})),
        bad(json!({"model_line": null})),
    ]
}
