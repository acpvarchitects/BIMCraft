//! FE-16c-2: a placed component's single faces painted in the project (`Component::face_paint`), each by its form's GUID, the
//! nested instances down to it and its GEO-2b key: the `paint` command, what the file holds and what a read leaves out, what
//! copies, mirrors, groups, type changes and reloads do to it, which paint wins in the 3D view and glTF, and a shared nested
//! instance's faces.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::FormFace;
use crate::ids::{Guid, Id};
use crate::model::{Element, Model};
use crate::render::Rgb;

pub(crate) fn gstr(n: u32) -> String {
    format!("9e01c000-0000-4000-8000-{n:012}")
}

pub(crate) fn guid(n: u32) -> Guid {
    Guid::parse(&gstr(n)).unwrap()
}

pub(crate) fn line(n: u32, a: [f64; 2], b: [f64; 2]) -> Value {
    json!({"guid": gstr(n), "start": a, "end": b})
}

/// A rectangle's four lines, GUIDs `n` to `n + 3`: bottom, right, top, left.
pub(crate) fn rect(n: u32, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
    vec![line(n, [x0, y0], [x1, y0]), line(n + 1, [x1, y0], [x1, y1]), line(n + 2, [x1, y1], [x0, y1]), line(n + 3, [x0, y1], [x0, y0])]
}

pub(crate) const BOX: u32 = 100;
pub(crate) const VOID: u32 = 200;
pub(crate) const OTHER: u32 = 300;

/// The box form: 1000 x 500 x 300, lines 1 to 4.
pub(crate) fn boxed() -> Value {
    json!({"guid": gstr(BOX), "sketch": rect(1, 0.0, 0.0, 1000.0, 500.0), "from": 0, "to": 300, "material": "Concrete"})
}

pub(crate) fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

/// A project with materials Red, Blue and Green, family `Box` (types A and B, one box) and family `Other` (one box of its own
/// form), and a component of `Box` at the origin.
pub(crate) fn project() -> (Session, Id) {
    let mut s = Session::new();
    run(&mut s, "material", json!({"name": "Red", "color": [220, 30, 30]}));
    run(&mut s, "material", json!({"name": "Blue", "color": [30, 40, 220]}));
    run(&mut s, "material", json!({"name": "Green", "color": [30, 200, 40]}));
    run(
        &mut s,
        "family",
        json!({"name": "Box", "category": "generic", "parameters": [{"name": "W", "value": 1000}], "types": {"A": {}, "B": {"W": 1200}},
               "forms": [boxed()]}),
    );
    run(
        &mut s,
        "family",
        json!({"name": "Other", "category": "generic", "types": {"X": {}},
               "forms": [{"guid": gstr(OTHER), "sketch": rect(31, 0.0, 0.0, 800.0, 800.0), "from": 0, "to": 200}]}),
    );
    let id = run(&mut s, "component", json!({"family": "Box", "type": "A", "at": [0, 0]}))["id"].as_u64().unwrap();
    (s, id)
}

pub(crate) fn component(m: &Model, id: Id) -> &crate::model::Component {
    match m.elements.get(id) {
        Some(Element::Component(c)) => c,
        e => panic!("{id} is no component: {e:?}"),
    }
}

/// What component `id`'s faces are painted: (form, nested, face, material).
pub(crate) fn faces(m: &Model, id: Id) -> Vec<(Guid, Vec<Guid>, FormFace, String)> {
    component(m, id).face_paint.iter().map(|p| (p.form, p.nested.clone(), p.face.clone(), p.material.clone())).collect()
}

pub(crate) fn side(n: u32) -> FormFace {
    FormFace::side(guid(n))
}

pub(crate) fn paint(s: &mut Session, id: Id, face: Value, material: Value) -> Value {
    run(s, "paint", json!({"id": id, "form": gstr(BOX), "face": face, "material": material}))
}

#[test]
fn the_paint_command_paints_a_components_face_replaces_it_and_takes_it_off() {
    let (mut s, id) = project();
    let steps = s.history().len();
    let r = paint(&mut s, id, json!({"side": gstr(2)}), json!("Red"));
    assert_eq!(r["painted"], json!([id]), "{r}");
    assert_eq!(s.history().len(), steps + 1, "one undo step");
    paint(&mut s, id, json!("end"), json!("Blue"));
    // Again on the same face: replaced, never a second entry.
    paint(&mut s, id, json!({"side": gstr(2)}), json!("Green"));
    assert_eq!(
        faces(&s.model, id),
        [(guid(BOX), vec![], side(2), "Green".to_string()), (guid(BOX), vec![], FormFace::End, "Blue".to_string())]
    );
    // By the form's id as well as its GUID.
    let form_id = s.model.families["Box"].forms[0].id;
    run(&mut s, "paint", json!({"id": id, "form": form_id, "face": "end", "material": "Red"}));
    assert_eq!(faces(&s.model, id)[1].3, "Red");
    // `material: null` takes one face's paint off.
    paint(&mut s, id, json!({"side": gstr(2)}), Value::Null);
    assert_eq!(faces(&s.model, id).len(), 1);
    // The whole component's `all` still works, beside it.
    run(&mut s, "paint", json!({"id": id, "face": "all", "material": "Green"}));
    assert_eq!(component(&s.model, id).params["Paint"], json!({"all": "Green"}));
    assert_eq!(faces(&s.model, id).len(), 1, "`all` leaves the faces' paint alone");
    // Refused, saying what to fix, and nothing changed.
    let before = s.model.clone();
    for (p, says) in [
        (json!({"id": id, "form": gstr(BOX), "face": {"side": gstr(77)}, "material": "Red"}), "has no face"),
        (json!({"id": id, "form": gstr(999), "face": "end", "material": "Red"}), "no form"),
        (json!({"id": id, "form": gstr(BOX), "face": "end", "material": "Gold"}), "no `Gold`"),
        (json!({"id": id, "form": gstr(BOX), "face": "top", "material": "Red"}), "face"),
        (json!({"id": id, "form": gstr(BOX), "material": "Red"}), "`face`"),
        (json!({"id": id, "form": gstr(BOX), "face": "end", "material": "Red", "nested": [gstr(5)]}), "nested"),
        (json!({"id": id, "form": gstr(BOX), "face": "end", "material": "Red", "gloss": 1}), "gloss"),
        (json!({"id": id, "form": 12, "face": "end", "material": "Red"}), "form"),
    ] {
        let e = s.execute("paint", &p).unwrap_err();
        assert!(e.contains(says), "{p}: {e}");
    }
    // A wall has no forms.
    let wall = run(&mut s, "wall", json!({"points": [[0, 3000], [4000, 3000]]}))["ids"][0].as_u64().unwrap();
    let e = s.execute("paint", &json!({"id": wall, "form": gstr(BOX), "face": "end", "material": "Red"})).unwrap_err();
    assert!(e.contains("component"), "{e}");
    s.execute("undo", &json!({})).unwrap();
    assert_eq!(*s.model, *before);
}

#[test]
fn two_painted_faces_go_through_the_file_by_the_materials_guid() {
    let (mut s, id) = project();
    paint(&mut s, id, json!({"side": gstr(2)}), json!("Red"));
    paint(&mut s, id, json!("end"), json!("Blue"));
    let text = s.model.to_json().unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    let held = &file["elements"][id.to_string()]["face_paint"];
    let red = s.model.library.guid(&crate::library::Kind::Material, "Red").unwrap();
    let blue = s.model.library.guid(&crate::library::Kind::Material, "Blue").unwrap();
    assert_eq!(held[0]["material"], json!(red), "{held}");
    assert_eq!(held[1]["material"], json!(blue));
    assert_eq!(held[0]["form"], json!(gstr(BOX)));
    assert!(held[0].get("nested").is_none(), "an empty path isn't written: {held}");
    let (back, left_out) = Model::read(&text).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(faces(&back, id), faces(&s.model, id));
    back.check().unwrap();
    // Nothing painted, nothing written.
    let (bare, other) = project();
    let file: Value = serde_json::from_str(&bare.model.to_json().unwrap()).unwrap();
    assert!(file["elements"][other.to_string()].get("face_paint").is_none());
    // A rename follows, and a material painted on a face is in use.
    run(&mut s, "material", json!({"name": "Red", "rename": "Crimson"}));
    assert_eq!(faces(&s.model, id)[0].3, "Crimson");
    let e = s.execute("material", &json!({"name": "Blue", "delete": true})).unwrap_err();
    assert!(e.contains(&id.to_string()), "{e}");
}

/// A build of 21 from before FE-16c-2 doesn't know `face_paint`: it leaves the field out, lists it, and opens the component
/// unpainted. Simulated with a field this build doesn't know in its place, which the lenient read treats the same way.
#[test]
fn a_build_that_doesnt_know_the_field_leaves_it_out_and_lists_it() {
    let (mut s, id) = project();
    paint(&mut s, id, json!("end"), json!("Red"));
    let mut file: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    let e = file["elements"][id.to_string()].as_object_mut().unwrap();
    let held = e.remove("face_paint").unwrap();
    e.insert("face_paint_of_a_newer_build".into(), held);
    let (back, left_out) = Model::read(&file.to_string()).unwrap();
    assert_eq!(left_out, [format!("/elements/{id}/face_paint_of_a_newer_build")]);
    assert!(component(&back, id).face_paint.is_empty());
    back.check().unwrap();
}

#[test]
fn entries_a_file_cant_stand_by_are_left_out_by_their_place_in_the_file() {
    let (mut s, id) = project();
    paint(&mut s, id, json!("end"), json!("Red"));
    let mut file: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    let list = file["elements"][id.to_string()]["face_paint"].as_array_mut().unwrap();
    let red = list[0]["material"].clone();
    for p in [
        // A face key of a newer build's.
        json!({"form": gstr(BOX), "face": {"zzz": 1}, "material": red}),
        // A field of a newer build's.
        json!({"form": gstr(BOX), "face": "start", "material": red, "gloss": 3}),
        // A material the file doesn't hold.
        json!({"form": gstr(BOX), "face": {"side": gstr(1)}, "material": "6c1e0000-0000-4000-8000-000000000000"}),
        // A form the family hasn't.
        json!({"form": gstr(999), "face": "end", "material": red}),
        // A nested instance the family hasn't.
        json!({"form": gstr(BOX), "nested": [gstr(998)], "face": "end", "material": red}),
        // A face the form hasn't.
        json!({"form": gstr(BOX), "face": {"side": gstr(77)}, "material": red}),
        // Kept: another face, after all those.
        json!({"form": gstr(BOX), "face": {"side": gstr(3)}, "material": red}),
    ] {
        list.push(p);
    }
    let (back, left_out) = Model::read(&file.to_string()).unwrap();
    let want: Vec<String> = (1..=6).map(|i| format!("/elements/{id}/face_paint/{i}")).collect();
    assert_eq!(left_out, want);
    assert_eq!(faces(&back, id), [(guid(BOX), vec![], FormFace::End, "Red".to_string()), (guid(BOX), vec![], side(3), "Red".to_string())]);
    back.check().unwrap();
}

#[test]
fn copy_mirror_array_and_groups_keep_it() {
    let (mut s, id) = project();
    paint(&mut s, id, json!({"side": gstr(2)}), json!("Red"));
    let want = faces(&s.model, id);
    let copied = run(&mut s, "copy", json!({"ids": [id], "by": [3000, 0]}));
    let copy = copied["ids"][0].as_u64().unwrap_or_else(|| panic!("{copied}"));
    assert_eq!(faces(&s.model, copy), want, "a copy");
    // An array: copies one after another, as the window's Array makes them.
    for k in 1..=3 {
        let made = run(&mut s, "copy", json!({"ids": [id], "by": [0, 2000 * k]}));
        assert_eq!(faces(&s.model, made["ids"][0].as_u64().unwrap()), want, "array copy {k}");
    }
    // Mirrored in place and as a copy: the key is the line's GUID, which a mirror keeps.
    run(&mut s, "mirror", json!({"ids": [id], "axis": [[-500, 0], [-500, 1000]]}));
    assert_eq!(faces(&s.model, id), want, "mirrored");
    let mirrored = run(&mut s, "mirror", json!({"ids": [copy], "axis": [[0, -2000], [1000, -2000]], "copy": true}));
    let mcopy = mirrored["ids"][0].as_u64().unwrap_or_else(|| panic!("{mirrored}"));
    assert_eq!(faces(&s.model, mcopy), want, "a mirrored copy");
    // The mirrored one's side 2 is still red in the 3D view.
    assert!(reds(&s.model, id) > 0);
    // A model group and a copy of it placed.
    let g = run(&mut s, "create_group", json!({"ids": [copy], "name": "G"}));
    let placed = run(&mut s, "model_group", json!({"type": "G", "at": [20000, 0]}));
    let members: Vec<Id> = s
        .model
        .elements
        .iter()
        .filter(|(i, e)| e.category() == "component" && ![id, copy, mcopy].contains(i) && component(&s.model, **i).at.0 > 15000.0)
        .map(|(i, _)| *i)
        .collect();
    assert!(!members.is_empty(), "the placed group's component: {g} {placed}");
    for m in members {
        assert_eq!(faces(&s.model, m), want, "the group's copy");
    }
}

/// How many triangles of component `id` the 3D scene shows red.
fn reds(m: &Model, id: Id) -> usize {
    let scene = crate::view3d::Scene::new(m);
    let look = crate::graphics::Look::of_3d(m);
    scene.facets(&look).filter(|t| t.owner == id && t.color == Rgb(220, 30, 30)).count()
}

#[test]
fn a_type_change_in_the_family_keeps_it_and_one_to_another_family_drops_and_lists_it() {
    let (mut s, id) = project();
    paint(&mut s, id, json!({"side": gstr(2)}), json!("Red"));
    let r = run(&mut s, "modify", json!({"id": id, "type": "B"}));
    assert_eq!(faces(&s.model, id).len(), 1, "type B of the same family: kept");
    assert!(r.get("paint_dropped").is_none(), "{r}");
    assert!(reds(&s.model, id) > 0, "and shown on type B's wider box");
    let before = faces(&s.model, id);
    let r = run(&mut s, "modify", json!({"id": id, "family": "Other", "type": "X"}));
    assert!(faces(&s.model, id).is_empty(), "another family's: dropped");
    assert_eq!(r["paint_dropped"], json!([{"id": id, "form": gstr(BOX), "face": {"side": gstr(2)}}]), "{r}");
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&s.model, id), before, "one undo puts back the family and its paint");
}

#[test]
fn a_reload_that_deletes_the_painted_line_drops_it_in_the_same_step_and_undo_restores_both() {
    let (mut s, id) = project();
    paint(&mut s, id, json!({"side": gstr(4)}), json!("Red"));
    paint(&mut s, id, json!({"side": gstr(2)}), json!("Blue"));
    let steps = s.history().len();
    let before = (*s.model).clone();
    // The family loaded again with line 4 gone: a triangle of lines 1, 2 and a new one.
    let triangle =
        json!([line(1, [0.0, 0.0], [1000.0, 0.0]), line(2, [1000.0, 0.0], [1000.0, 500.0]), line(9, [1000.0, 500.0], [0.0, 0.0])]);
    let definition = json!({"category": "generic", "parameters": [{"name": "W", "value": 1000}], "types": {"A": {}, "B": {"W": 1200}},
                            "forms": [{"guid": gstr(BOX), "sketch": triangle, "from": 0, "to": 300, "material": "Concrete"}]});
    let r = run(&mut s, "family", json!({"name": "Box", "definition": definition, "overwrite": "version"}));
    assert_eq!(s.history().len(), steps + 1, "one step");
    assert_eq!(faces(&s.model, id), [(guid(BOX), vec![], side(2), "Blue".to_string())], "line 4's paint went with it");
    assert_eq!(r["paint_dropped"], json!([{"id": id, "form": gstr(BOX), "face": {"side": gstr(4)}}]), "{r}");
    run(&mut s, "undo", json!({}));
    assert_eq!(s.model.families["Box"], before.families["Box"], "the family back");
    assert_eq!(faces(&s.model, id), faces(&before, id), "and its paint");
}

#[test]
fn a_void_that_splits_the_box_keeps_the_side_painted_on_both_parts() {
    let (mut s, id) = project();
    paint(&mut s, id, json!({"side": gstr(1)}), json!("Red"));
    // A void across the box's middle, from y below to above it: two parts, each with a piece of side 1.
    run(
        &mut s,
        "family",
        json!({"name": "Box", "forms": [boxed(), {"guid": gstr(VOID), "sketch": rect(21, 450.0, -100.0, 550.0, 600.0), "from": -10, "to": 310, "void": true}]}),
    );
    assert_eq!(faces(&s.model, id).len(), 1, "kept");
    let scene = crate::view3d::Scene::new(&s.model);
    let look = crate::graphics::Look::of_3d(&s.model);
    let red: Vec<_> = scene.facets(&look).filter(|t| t.owner == id && t.color == Rgb(220, 30, 30)).collect();
    let left = red.iter().any(|t| t.corners.iter().all(|q| q[0] <= 450.0 + 1e-6));
    let right = red.iter().any(|t| t.corners.iter().all(|q| q[0] >= 550.0 - 1e-6));
    assert!(left && right, "both parts' side 1 red");
    assert!(red.iter().all(|t| t.corners.iter().all(|q| q[1].abs() < 1e-6)), "only side 1, at y = 0");
}

#[test]
fn the_check_refuses_a_face_twice_an_unknown_material_a_void_and_too_many() {
    let (mut s, id) = project();
    run(
        &mut s,
        "family",
        json!({"name": "Box", "forms": [boxed(), {"guid": gstr(VOID), "sketch": rect(21, 100.0, 100.0, 200.0, 200.0), "from": 100, "to": 400, "void": true}]}),
    );
    let entry = |form: u32, face: Value, m: &str| json!({"form": gstr(form), "face": face, "material": m});
    for (list, says) in [
        (json!([entry(BOX, json!("end"), "Red"), entry(BOX, json!("end"), "Blue")]), "twice"),
        (json!([entry(BOX, json!("end"), "Gold")]), "Gold"),
        (json!([entry(BOX, json!("end"), " ")]), "material"),
        (json!([entry(VOID, json!("end"), "Red")]), "void"),
    ] {
        let mut m = (*s.model).clone();
        let Some(Element::Component(c)) = m.elements.get_mut(id) else { panic!() };
        c.face_paint = serde_json::from_value(list.clone()).unwrap();
        let e = m.check().unwrap_err();
        assert!(e.contains(says), "{list}: {e}");
    }
    let many: Vec<Value> = (0..=crate::family::MAX_PAINT as u32).map(|i| entry(BOX, json!({"edge": i}), "Red")).collect();
    let mut m = (*s.model).clone();
    let Some(Element::Component(c)) = m.elements.get_mut(id) else { panic!() };
    c.face_paint = serde_json::from_value(json!(many)).unwrap();
    assert!(m.check().unwrap_err().contains("more than"));
    // The command refuses a void too, saying so.
    let e = s.execute("paint", &json!({"id": id, "form": gstr(VOID), "face": "end", "material": "Red"})).unwrap_err();
    assert!(e.contains("void"), "{e}");
}

/// The colour the 3D scene gives component `id`'s triangles whose corners all pass `on`.
fn colour(m: &Model, id: Id, on: impl Fn(&[f64; 3]) -> bool) -> Vec<Rgb> {
    let scene = crate::view3d::Scene::new(m);
    let look = crate::graphics::Look::of_3d(m);
    let mut out: Vec<Rgb> = scene.facets(&look).filter(|t| t.owner == id && t.corners.iter().all(&on)).map(|t| t.color).collect();
    out.dedup();
    out
}

#[test]
fn a_face_painted_in_the_project_wins_over_all_over_the_familys_over_the_material() {
    let (mut s, id) = project();
    let top = |q: &[f64; 3]| (q[2] - 300.0).abs() < 1e-6;
    let bottom = |q: &[f64; 3]| q[2].abs() < 1e-6;
    let concrete = s.model.materials["Concrete"].color;
    let concrete = Rgb(concrete[0], concrete[1], concrete[2]);
    let (red, blue, green) = (Rgb(220, 30, 30), Rgb(30, 40, 220), Rgb(30, 200, 40));
    assert_eq!(colour(&s.model, id, top), [concrete], "the form's material");
    // The family paints the top and the bottom green.
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": "end", "material": "Green"}}));
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": "start", "material": "Green"}}));
    assert_eq!(colour(&s.model, id, top), [green], "the family's paint over the material");
    // The project's `all` over the family's.
    run(&mut s, "paint", json!({"id": id, "face": "all", "material": "Blue"}));
    assert_eq!(colour(&s.model, id, top), [blue]);
    assert_eq!(colour(&s.model, id, bottom), [blue]);
    // The project's face paint over `all`: the top red, the bottom still `all`'s.
    paint(&mut s, id, json!("end"), json!("Red"));
    assert_eq!(colour(&s.model, id, top), [red]);
    assert_eq!(colour(&s.model, id, bottom), [blue]);
    // Without `all`, the bottom is the family's again, the top still the project's.
    run(&mut s, "paint", json!({"id": id, "face": "all", "material": null}));
    assert_eq!(colour(&s.model, id, top), [red]);
    assert_eq!(colour(&s.model, id, bottom), [green]);
    // glTF writes what the scene shows: a material of red's.
    let out = crate::gltf::export(&s.model).unwrap();
    let len = u32::from_le_bytes(out.bytes[12..16].try_into().unwrap()) as usize;
    let file: Value = serde_json::from_slice(&out.bytes[20..20 + len]).unwrap();
    let colours: Vec<Value> =
        file["materials"].as_array().unwrap().iter().map(|m| m["pbrMetallicRoughness"]["baseColorFactor"].clone()).collect();
    let near = |c: &Value, rgb: [f64; 3]| (0..3).all(|k| c[k].as_f64().is_some_and(|v| (v - rgb[k]).abs() < 0.02));
    let srgb = |v: f64| {
        let v = v / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let red_lin = [srgb(220.0), srgb(30.0), srgb(30.0)];
    let red_raw = [220.0 / 255.0, 30.0 / 255.0, 30.0 / 255.0];
    assert!(colours.iter().any(|c| near(c, red_lin) || near(c, red_raw)), "a red material in the glTF: {colours:?}");
}

#[test]
fn a_click_names_the_form_the_nested_path_and_the_face() {
    let (s, id) = project();
    let at = s.model.component_face(id, [1000.0, 250.0, 150.0], [1.0, 0.0, 0.0]).unwrap();
    assert_eq!((at.form, at.nested.clone(), at.face.clone()), (guid(BOX), vec![], side(2)));
    assert!(s.model.component_face(id, [5000.0, 250.0, 150.0], [1.0, 0.0, 0.0]).is_none());
}

/// A cabinet (one box 600 x 400 x 800) placing a handle (one box, its form `HANDLE`), shared or not, at (100, 50) on its top,
/// and a component of the cabinet at the origin.
const HANDLE: u32 = 600;
fn cabinet(shared: bool) -> (Session, Id) {
    let mut s = Session::new();
    run(&mut s, "material", json!({"name": "Red", "color": [220, 30, 30]}));
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "category": "furniture", "types": {"Small": {}},
               "forms": [{"guid": gstr(BOX), "sketch": rect(1, 0.0, 0.0, 600.0, 400.0), "from": 0, "to": 800}]}),
    );
    let handle = json!({"category": "generic", "shared": shared, "types": {"Lever": {}},
                        "forms": [{"guid": gstr(HANDLE), "sketch": rect(61, 0.0, 0.0, 40.0, 20.0), "from": 0, "to": 30}]});
    run(&mut s, "family", json!({"name": "Cabinet", "nest": {"definition": handle, "name": "Handle"}}));
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nested_instance": {"family": "Handle", "type": "Lever", "at": [100, 50], "offset": 800}}),
    );
    let id = run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [0, 0]}))["id"].as_u64().unwrap();
    (s, id)
}

fn instance(s: &Session) -> Guid {
    s.model.families["Cabinet"].nested_instances[0].guid
}

#[test]
fn a_nested_instances_face_is_painted_by_its_path_and_a_shared_ones_on_its_own_element() {
    // Not shared: the handle is part of the cabinet, its faces painted through the cabinet with the path to it.
    let (mut s, id) = cabinet(false);
    let path = vec![instance(&s)];
    let top = [120.0, 60.0, 830.0];
    let at = s.model.component_face(id, top, [0.0, 0.0, 1.0]).unwrap();
    assert_eq!((at.form, at.nested.clone(), at.face.clone()), (guid(HANDLE), path.clone(), FormFace::End));
    run(&mut s, "paint", json!({"id": id, "form": gstr(HANDLE), "nested": [path[0]], "face": "end", "material": "Red"}));
    assert_eq!(faces(&s.model, id), [(guid(HANDLE), path.clone(), FormFace::End, "Red".to_string())]);
    assert!(colour(&s.model, id, |q| (q[2] - 830.0).abs() < 1e-6) == [Rgb(220, 30, 30)], "the handle's top red");
    let (back, left_out) = Model::read(&s.model.to_json().unwrap()).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(faces(&back, id), faces(&s.model, id));

    // Shared: the handle is an element of its own, and its faces are painted on it, held by the cabinet under the same path.
    let (mut s, id) = cabinet(true);
    let path = vec![instance(&s)];
    let handle = s.model.elements.iter().find(|(_, e)| matches!(e, Element::Nested(_))).map(|(i, _)| *i).unwrap();
    // Through the host's path it is refused, naming the element to paint.
    let e =
        s.execute("paint", &json!({"id": id, "form": gstr(HANDLE), "nested": [path[0]], "face": "end", "material": "Red"})).unwrap_err();
    assert!(e.contains("shared") && e.contains(&handle.to_string()), "{e}");
    // On the nested element itself.
    run(&mut s, "paint", json!({"id": handle, "form": gstr(HANDLE), "face": "end", "material": "Red"}));
    assert_eq!(faces(&s.model, id), [(guid(HANDLE), path.clone(), FormFace::End, "Red".to_string())], "held by its host");
    assert!(colour(&s.model, id, |q| (q[2] - 830.0).abs() < 1e-6) == [Rgb(220, 30, 30)], "the handle's top red");
    // What a click on it names leads to that element.
    let at = s.model.component_face(id, top, [0.0, 0.0, 1.0]).unwrap();
    assert_eq!(s.model.face_owner(id, &at.nested), Some((handle, vec![])));
    // Taken off on the element too.
    run(&mut s, "paint", json!({"id": handle, "form": gstr(HANDLE), "face": "end", "material": null}));
    assert!(faces(&s.model, id).is_empty());
}

#[test]
fn the_paint_reply_on_a_command_that_drops_nothing_says_nothing() {
    let (mut s, id) = project();
    paint(&mut s, id, json!("end"), json!("Red"));
    let r = run(&mut s, "move", json!({"ids": [id], "by": [100, 0]}));
    assert!(r.get("paint_dropped").is_none(), "{r}");
    assert_eq!(faces(&s.model, id).len(), 1);
}
