//! FE-16c-1: a family form's faces painted, each by its GEO-2b key (`Form::paint`): what a build gives each triangle
//! (`Piece::paint`), how edits keep or drop it, what the file holds, the `family` command's `paint`, and what the 3D view and
//! glTF show while plans and sections stay as they were.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::{Family, FormFace, Piece};
use crate::ids::Guid;
use crate::render::Rgb;

fn gstr(n: u32) -> String {
    format!("9e01c000-0000-4000-8000-{n:012}")
}

fn guid(n: u32) -> Guid {
    Guid::parse(&gstr(n)).unwrap()
}

fn side(n: u32) -> FormFace {
    FormFace::side(guid(n))
}

fn line(n: u32, a: [f64; 2], b: [f64; 2]) -> Value {
    json!({"guid": gstr(n), "start": a, "end": b})
}

/// A rectangle's four lines, GUIDs `n` to `n + 3`: bottom, right, top, left.
fn rect(n: u32, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Value> {
    vec![line(n, [x0, y0], [x1, y0]), line(n + 1, [x1, y0], [x1, y1]), line(n + 2, [x1, y1], [x0, y1]), line(n + 3, [x0, y1], [x0, y0])]
}

const BOX: u32 = 100;
const VOID: u32 = 200;

/// A box 1000 x 500 x 300 (its lines 1 to 4), with `paint`.
fn boxed(paint: Value) -> Value {
    let sketch = json!([{"guid": gstr(1), "start": [0, 0], "end": ["W", 0]}, {"guid": gstr(2), "start": ["W", 0], "end": ["W", 500]},
                        {"guid": gstr(3), "start": ["W", 500], "end": [0, 500]}, line(4, [0.0, 500.0], [0.0, 0.0])]);
    json!({"guid": gstr(BOX), "sketch": sketch, "from": 0, "to": 300, "material": "Concrete", "paint": paint})
}

fn family(forms: Value) -> Family {
    serde_json::from_value(json!({
        "category": "generic",
        "parameters": [{"name": "W", "value": 1000}, {"name": "Finish", "kind": "material", "value": "Steel"}],
        "types": {"A": {}, "B": {"W": 1600, "Finish": "Glass"}},
        "forms": forms
    }))
    .unwrap()
}

fn pieces_of(f: &Family, t: &str) -> Vec<Piece> {
    let values = f.values(t, &BTreeMap::new()).unwrap();
    f.pieces(&values).unwrap()
}

/// What each triangle of `p` is painted, with its key: an empty name, unpainted.
fn painted(p: &Piece) -> Vec<(FormFace, String)> {
    p.faces
        .iter()
        .map(|k| {
            let key = p.keys[*k as usize].clone();
            let m = p.paint.get(*k as usize).cloned().unwrap_or_default();
            (key, m)
        })
        .collect()
}

/// The material each key's triangles are painted, every triangle of a key the same.
fn by_key(p: &Piece) -> BTreeMap<FormFace, String> {
    assert!(p.paint.is_empty() || p.paint.len() == p.keys.len(), "a name for each key, or none: {:?}", p.paint);
    let mut out: BTreeMap<FormFace, String> = BTreeMap::new();
    for (k, m) in painted(p) {
        let was = out.insert(k.clone(), m.clone());
        assert!(was.is_none_or(|w| w == m), "{k:?}: every triangle of a face is painted alike");
    }
    out
}

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

/// A project with family `Box` (one box and a void through it) and a component of it at the origin.
fn project(paint: Value) -> (Session, u64) {
    let mut s = Session::new();
    run(&mut s, "material", json!({"name": "Red", "color": [220, 30, 30]}));
    run(&mut s, "material", json!({"name": "Blue", "color": [30, 40, 220]}));
    run(
        &mut s,
        "family",
        json!({"name": "Box", "category": "generic", "parameters": [{"name": "W", "value": 1000}], "types": {"A": {}},
               "forms": [boxed(paint),
                         {"guid": gstr(VOID), "sketch": rect(21, 400.0, 200.0, 600.0, 300.0), "from": -10, "to": 310, "void": true}]}),
    );
    let id = run(&mut s, "component", json!({"family": "Box", "type": "A", "at": [0, 0]}))["id"].as_u64().unwrap();
    (s, id)
}

fn form(s: &Session) -> &crate::family::Form {
    &s.model.families["Box"].forms[0]
}

#[test]
fn a_painted_side_and_a_cap_give_their_triangles_the_material() {
    let f = family(json!([boxed(json!([{"face": {"side": gstr(2)}, "material": "Timber"}, {"face": "end", "material": "Finish"}]))]));
    f.check(&|_| true).unwrap();
    let p = &pieces_of(&f, "A")[0];
    let got = by_key(p);
    assert_eq!(got[&side(2)], "Timber");
    assert_eq!(got[&FormFace::End], "Steel", "a material parameter's, as the form's own material reads it");
    for k in [FormFace::Start, side(1), side(3), side(4)] {
        assert_eq!(got[&k], "", "{k:?} unpainted");
    }
    assert_eq!(p.material, "Concrete", "the form keeps its own material");
    assert_eq!(by_key(&pieces_of(&f, "B")[0])[&FormFace::End], "Glass", "type B's finish");
    // Nothing painted, nothing named.
    let bare = family(json!([boxed(json!([]))]));
    assert!(pieces_of(&bare, "A")[0].paint.is_empty());
}

#[test]
fn a_flexed_or_moved_line_keeps_its_paint() {
    let f = family(json!([boxed(json!([{"face": {"side": gstr(2)}, "material": "Timber"}]))]));
    // Type B is 1600 wide: the right side moved with it, painted still.
    let p = &pieces_of(&f, "B")[0];
    let right: Vec<_> = p.tris.iter().zip(painted(p)).filter(|(_, (k, _))| *k == side(2)).collect();
    assert!(!right.is_empty() && right.iter().all(|(t, (_, m))| m == "Timber" && t.iter().all(|q| (q[0] - 1600.0).abs() < 1e-9)));
    // A line moved by the command: its GUID stays, its paint with it.
    let (mut s, _) = project(json!([{"face": {"side": gstr(2)}, "material": "Red"}]));
    let mut sketch = rect(1, 0.0, 0.0, 1400.0, 500.0);
    sketch[1] = line(2, [1400.0, 0.0], [1400.0, 700.0]);
    sketch[2] = line(3, [1400.0, 700.0], [0.0, 500.0]);
    run(&mut s, "family", json!({"name": "Box", "form": {"guid": gstr(BOX), "sketch": sketch}}));
    assert_eq!(form(&s).paint.len(), 1, "kept");
}

#[test]
fn deleting_a_line_or_a_void_drops_the_paint_naming_it_and_undo_brings_both_back() {
    let cut = json!({"cut": gstr(VOID), "face": {"side": gstr(21)}});
    let (mut s, _) = project(json!([
        {"face": {"side": gstr(4)}, "material": "Red"},
        {"face": {"side": gstr(2)}, "material": "Blue"},
        {"face": cut, "material": "Blue"}
    ]));
    assert_eq!(form(&s).paint.len(), 3);
    // Line 4 gone: the profile a triangle of lines 1, 2 and a new one.
    let triangle =
        json!([line(1, [0.0, 0.0], [1000.0, 0.0]), line(2, [1000.0, 0.0], [1000.0, 500.0]), line(9, [1000.0, 500.0], [0.0, 0.0])]);
    let before = form(&s).clone();
    run(&mut s, "family", json!({"name": "Box", "form": {"guid": gstr(BOX), "sketch": triangle}}));
    let faces: Vec<FormFace> = form(&s).paint.iter().map(|p| p.face.clone()).collect();
    assert!(!faces.contains(&side(4)), "line 4's paint went with it: {faces:?}");
    assert!(faces.contains(&side(2)), "line 2's stays");
    run(&mut s, "undo", json!({}));
    assert_eq!(*form(&s), before, "one undo puts back the line and its paint");
    // The void deleted: the faces it cut go, and their paint.
    run(&mut s, "family", json!({"name": "Box", "delete_form": [gstr(VOID)]}));
    assert_eq!(form(&s).paint.len(), 2, "{:?}", form(&s).paint);
    assert!(form(&s).paint.iter().all(|p| !matches!(p.face, FormFace::Cut { .. })));
    run(&mut s, "undo", json!({}));
    assert_eq!(form(&s).paint.len(), 3);
}

#[test]
fn a_face_a_void_cuts_is_painted_drilled_or_cut() {
    let paint = json!([{"face": {"cut": gstr(VOID), "face": {"side": gstr(21)}}, "material": "Timber"}]);
    let f = family(json!([boxed(paint),
        {"guid": gstr(VOID), "sketch": rect(21, 400.0, 200.0, 600.0, 300.0), "from": -10, "to": 310, "void": true}]));
    for p in [pieces_of(&f, "A").remove(0), crate::family::without_drilling(|| pieces_of(&f, "A").remove(0))] {
        let hole: Vec<_> = p.tris.iter().zip(painted(&p)).filter(|(_, (_, m))| m == "Timber").collect();
        assert!(!hole.is_empty(), "the hole's side is painted");
        // Line 21 is the hole's bottom, at y = 200.
        assert!(hole.iter().all(|(t, _)| t.iter().all(|q| (q[1] - 200.0).abs() < 1e-9)), "only that side");
        assert_eq!(by_key(&p).values().filter(|m| !m.is_empty()).count(), 1);
    }
}

#[test]
fn a_nested_instances_piece_keeps_its_own_familys_paint() {
    let leaf = json!({"category": "generic", "types": {"B": {}},
        "forms": [{"guid": gstr(500), "sketch": rect(41, 0.0, 0.0, 100.0, 100.0), "from": 0, "to": 50,
                   "paint": [{"face": "end", "material": "Steel"}]}]});
    let f: Family = serde_json::from_value(json!({
        "category": "generic", "types": {"A": {}},
        "forms": [{"guid": gstr(BOX), "sketch": rect(1, 0.0, 0.0, 1000.0, 1000.0), "from": 0, "to": 100,
                   "paint": [{"face": "end", "material": "Timber"}]}],
        "nested": [{"name": "Leaf", "family": leaf}],
        "nested_instances": [{"family": "Leaf", "type": "B", "at": [2000, 0]}]
    }))
    .unwrap();
    let pieces = pieces_of(&f, "A");
    assert_eq!(by_key(&pieces[0])[&FormFace::End], "Timber");
    assert_eq!(by_key(&pieces[1])[&FormFace::End], "Steel", "the nested family's own");
    assert!(f.materials().contains(&"Steel") && f.materials().contains(&"Timber"));
}

#[test]
fn bad_paint_is_refused_saying_what_to_fix() {
    let lib = |m: &str| ["Timber", "Steel", "Glass", "Concrete"].contains(&m);
    for (paint, says) in [
        (json!([{"face": "top", "material": "Timber"}]), "a face is"),
        (json!([{"face": {"side": "x"}, "material": "Timber"}]), "a face is"),
        (json!([{"face": "end", "material": "Timber", "colour": 1}]), "unknown field"),
        (json!([{"face": "end"}]), "missing field"),
    ] {
        let e = serde_json::from_value::<Family>(json!({"types": {"A": {}}, "forms": [boxed(paint.clone())]}))
            .err()
            .unwrap_or_else(|| panic!("{paint} is read"));
        assert!(e.to_string().contains(says), "{paint}: {e}");
    }
    for (paint, says) in [
        (json!([{"face": "end", "material": "Gold"}]), "material `Gold` isn't in the library"),
        (json!([{"face": "end", "material": "Timber"}, {"face": "end", "material": "Steel"}]), "painted twice"),
        (json!([{"face": "end", "material": ""}]), "needs a material"),
    ] {
        let f = family(json!([boxed(paint.clone())]));
        let e = f.check(&lib).unwrap_err();
        assert!(e.contains(says), "{paint}: {e}");
    }
    let many: Vec<Value> = (0..=crate::family::MAX_PAINT as u32).map(|i| json!({"face": {"edge": i}, "material": "Timber"})).collect();
    let e = family(json!([boxed(json!(many))])).check(&lib).unwrap_err();
    assert!(e.contains("more than"), "{e}");
}

#[test]
fn the_family_commands_paint_lays_takes_off_and_clears() {
    let (mut s, _) = project(json!([]));
    let steps = s.history().len();
    let r = run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": {"side": gstr(2)}, "material": "Red"}}));
    assert!(r.get("warnings").is_some(), "{r}");
    assert_eq!(s.history().len(), steps + 1, "an undo step");
    let id = form(&s).id;
    // By id too; painted again, the face takes the new material.
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": id, "face": {"side": gstr(2)}, "material": "Blue"}}));
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": id, "face": "end", "material": "Red"}}));
    let got: Vec<(FormFace, String)> = form(&s).paint.iter().map(|p| (p.face.clone(), p.material.clone())).collect();
    assert_eq!(got, [(side(2), "Blue".to_string()), (FormFace::End, "Red".to_string())]);
    // One taken off, then all.
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": id, "face": {"side": gstr(2)}, "material": null}}));
    assert_eq!(form(&s).paint.len(), 1);
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": id, "face": null, "material": null}}));
    assert!(form(&s).paint.is_empty());
    // Refused, and nothing changed: a face the form hasn't, a face missing, a material the library hasn't, a void, no form.
    let before = s.model.clone();
    for (p, says) in [
        (json!({"form": id, "face": {"side": gstr(77)}, "material": "Red"}), "has no face"),
        (json!({"form": id, "material": "Red"}), "`face` is required"),
        (json!({"form": id, "face": null, "material": "Red"}), "`face` is required"),
        (json!({"form": id, "face": "end", "material": "Gold"}), "isn't in the library"),
        (json!({"form": id, "face": "end"}), "`material`"),
        (json!({"form": gstr(VOID), "face": "end", "material": "Red"}), "a void"),
        (json!({"face": "end", "material": "Red"}), "`form`"),
        (json!({"form": gstr(999), "face": "end", "material": "Red"}), "is not of family"),
        (json!({"form": id, "face": "top", "material": "Red"}), "a face is"),
        (json!("end"), "`paint` is"),
    ] {
        let e = s.execute("family", &json!({"name": "Box", "paint": p})).unwrap_err();
        assert!(e.contains(says), "{p}: {e}");
    }
    assert_eq!(*s.model, *before);
}

#[test]
fn paint_goes_through_the_file_by_the_materials_guid_and_follows_a_rename() {
    let (mut s, _) = project(json!([{"face": "end", "material": "Red"}]));
    let text = s.model.to_json().unwrap();
    let red = s.model.library.guid(&crate::library::Kind::Material, "Red").unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    let held = &file["families"]["Box"]["forms"][0]["paint"][0]["material"];
    assert_eq!(held, &json!(red), "a file holds the material's GUID");
    let (back, left_out) = crate::model::Model::read(&text).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(back.families["Box"].forms[0].paint, form(&s).paint);
    run(&mut s, "material", json!({"name": "Red", "rename": "Crimson"}));
    assert_eq!(form(&s).paint[0].material, "Crimson");
    // A material in use by paint is in use.
    let e = s.execute("material", &json!({"name": "Crimson", "delete": true})).unwrap_err();
    assert!(e.contains("Box"), "{e}");
}

#[test]
fn a_file_whose_paint_names_a_face_the_form_hasnt_leaves_it_out() {
    let (s, _) = project(json!([{"face": "end", "material": "Red"}]));
    let mut file: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    let paint = &mut file["families"]["Box"]["forms"][0]["paint"];
    let red = paint[0]["material"].clone();
    paint.as_array_mut().unwrap().push(json!({"face": {"side": gstr(77)}, "material": red}));
    let (back, left_out) = crate::model::Model::read(&file.to_string()).unwrap();
    assert_eq!(left_out, ["/families/Box/forms/0/paint/1"]);
    assert_eq!(back.families["Box"].forms[0].paint.len(), 1);
    back.check().unwrap();
}

#[test]
fn a_family_loaded_from_favorites_brings_its_paints_materials() {
    let file = std::env::temp_dir().join(format!("bimcraft-fe16c1-favorites-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let (mut a, _) = project(json!([{"face": "end", "material": "Red"}]));
    run(&mut a, "favorite", json!({"action": "save", "kind": "family", "name": "Box", "path": file}));
    let mut b = Session::new();
    assert!(!b.model.materials.contains_key("Red"));
    run(&mut b, "favorite", json!({"action": "load", "kind": "family", "name": "Box", "path": file}));
    assert_eq!(b.model.materials["Red"].color, [220, 30, 30]);
    let _ = std::fs::remove_file(&file);
}

/// The plan and the section of the project, as SVG: paint changes neither. The section is made first, so both plans have it.
fn drawings(s: &mut Session) -> (String, String) {
    use crate::render::{self, View};
    let section = match s.model.elements.iter().find(|(_, e)| e.category() == "section") {
        Some((id, _)) => *id,
        None => run(s, "section", json!({"start": [500, -1000], "end": [500, 1500]}))["id"].as_u64().unwrap(),
    };
    let plan = render::svg(&render::draw(&s.model, 100000, View { angle: 0.0, scale: 0.2, cx: 500.0, cy: 250.0, width: 800, height: 600 }));
    let d = render::section(&s.model, section, View { angle: 0.0, scale: 0.2, cx: 0.0, cy: 0.0, width: 800, height: 600 });
    (plan, render::svg(&d))
}

#[test]
fn paint_changes_no_plan_or_section() {
    let (mut s, _) = project(json!([]));
    let before = drawings(&mut s);
    let all = json!([{"face": "start", "material": "Red"}, {"face": "end", "material": "Red"}, {"face": {"side": gstr(1)}, "material": "Blue"},
                     {"face": {"side": gstr(2)}, "material": "Blue"}, {"face": {"side": gstr(3)}, "material": "Blue"},
                     {"face": {"side": gstr(4)}, "material": "Blue"}]);
    run(&mut s, "family", json!({"name": "Box", "form": {"guid": gstr(BOX), "paint": all}}));
    assert_eq!(form(&s).paint.len(), 6);
    let after = drawings(&mut s);
    assert!(before.0 == after.0, "the plan is as it was");
    assert!(before.1 == after.1, "the section is as it was");
}

/// The colour of the 3D view's picture of `s` at model point `p`, and what it picks there.
fn seen(s: &Session, p: [f64; 3]) -> (Rgb, Option<u64>) {
    use crate::view3d::{Camera, Scene};
    let scene = Scene::new(&s.model);
    let fitted = scene.fit(30.0, 30.0, 800, 600);
    let cam = Camera { target: p, scale: 0.5, ..fitted };
    let img = scene.render(&cam, &[]);
    let (x, y, _) = cam.project(p);
    (img.color(x, y), img.pick(x, y))
}

#[test]
fn the_3d_view_shows_a_painted_face_in_its_material() {
    let (mut s, id) = project(json!([]));
    // The box's top, beside the void.
    let top = [200.0, 250.0, 300.0];
    let (bare, picked) = seen(&s, top);
    assert_eq!(picked, Some(id));
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": "end", "material": "Blue"}}));
    let (blue, _) = seen(&s, top);
    assert!(blue.2 > blue.0 + 60 && blue.2 > blue.1 + 60, "blue where it was {bare:?}: {blue:?}");
    // Its triangles: the top's blue, the rest as the form's material.
    let scene = crate::view3d::Scene::new(&s.model);
    let look = crate::graphics::Look::of_3d(&s.model);
    let concrete = s.model.materials["Concrete"].color;
    let own: Vec<_> = scene.facets(&look).filter(|t| t.owner == id).collect();
    let up = |t: &&crate::view3d::Facet| t.corners.iter().all(|q| q[2] == 300.0);
    assert!(own.iter().filter(up).all(|t| t.color == Rgb(30, 40, 220)));
    assert!(own.iter().filter(|t| !up(t)).all(|t| t.color == Rgb(concrete[0], concrete[1], concrete[2])));
    // Project paint over the whole component lies over the family's.
    run(&mut s, "paint", json!({"id": id, "face": "all", "material": "Red"}));
    let (red, _) = seen(&s, top);
    assert!(red.0 > red.2 + 60, "{red:?}");
}

#[test]
fn gltf_writes_a_painted_triangle_in_its_material() {
    let (mut s, _) = project(json!([]));
    let colours = |s: &Session| -> Vec<Value> {
        let out = crate::gltf::export(&s.model).unwrap();
        let len = u32::from_le_bytes(out.bytes[12..16].try_into().unwrap()) as usize;
        let json: Value = serde_json::from_slice(&out.bytes[20..20 + len]).unwrap();
        json["materials"].as_array().unwrap().iter().map(|m| m["pbrMetallicRoughness"]["baseColorFactor"].clone()).collect()
    };
    let before = colours(&s).len();
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": "end", "material": "Blue"}}));
    let after = colours(&s);
    assert_eq!(after.len(), before + 1, "a material of the paint's: {after:?}");
}

#[test]
fn a_point_on_a_components_surface_names_its_face() {
    let (mut s, id) = project(json!([]));
    run(&mut s, "component", json!({"family": "Box", "type": "A", "at": [5000, 0], "angle": 90}));
    let turned = s.model.elements.iter().filter(|(_, e)| e.category() == "component").map(|(i, _)| *i).max().unwrap();
    assert_eq!(s.model.component_face(id, [200.0, 250.0, 300.0], [0.0, 0.0, -1.0]).map(|f| f.face), Some(FormFace::End));
    assert_eq!(s.model.component_face(id, [200.0, 250.0, 0.0], [0.0, 0.0, 1.0]).map(|f| f.face), Some(FormFace::Start));
    assert_eq!(s.model.component_face(id, [1000.0, 250.0, 150.0], [1.0, 0.0, 0.0]).map(|f| f.face), Some(side(2)));
    // The void's side, line 21, at y = 200 inside the box.
    let cut = FormFace::Cut { by: guid(VOID), face: Box::new(side(21)) };
    assert_eq!(s.model.component_face(id, [500.0, 200.0, 150.0], [0.0, 1.0, 0.0]).map(|f| f.face), Some(cut));
    // Turned a quarter: its right side (x = 1000 in its axes) faces +y.
    assert_eq!(s.model.component_face(turned, [4750.0, 1000.0, 150.0], [0.0, -1.0, 0.0]).map(|f| f.face), Some(side(2)));
    assert_eq!(s.model.component_face(id, [200.0, 250.0, 900.0], [0.0, 0.0, 1.0]).map(|f| f.face), None, "off it");
    assert_eq!(s.model.component_face(id, [f64::NAN, 0.0, 0.0], [0.0, 0.0, 1.0]).map(|f| f.face), None);
}

/// cp's review of #89: whether a face is still there is read from the family, never from a type's build. A void that reaches
/// the box only on an instance whose `Bottom` is low enough makes no face under the type's values, yet the instance shows it:
/// it is painted from a click on that instance, and an unrelated edit keeps its paint.
#[test]
fn a_face_only_some_instances_have_is_painted_from_one_and_kept_by_unrelated_edits() {
    let mut s = Session::new();
    run(&mut s, "material", json!({"name": "Red", "color": [220, 30, 30]}));
    run(
        &mut s,
        "family",
        json!({"name": "Box", "category": "generic",
               "parameters": [{"name": "W", "value": 1000}, {"name": "Bottom", "value": 350, "instance": true}],
               "types": {"A": {}},
               "forms": [boxed(json!([])),
                         {"guid": gstr(VOID), "sketch": rect(21, 400.0, 200.0, 600.0, 300.0), "from": "Bottom", "to": 400, "void": true}]}),
    );
    let id =
        run(&mut s, "component", json!({"family": "Box", "type": "A", "at": [0, 0], "params": {"Bottom": 100}}))["id"].as_u64().unwrap();
    let cut = FormFace::Cut { by: guid(VOID), face: Box::new(side(21)) };
    // Under the type's values the void stays above the box: no build of the type has the pocket's side.
    let f = &s.model.families["Box"];
    assert!(pieces_of(f, "A").iter().all(|p| !p.keys.contains(&cut)), "the type's build has no pocket");
    // The instance has it: a click on the pocket's side (line 21, y = 200) names it, and the command paints it.
    let at = s.model.component_face(id, [500.0, 200.0, 200.0], [0.0, 1.0, 0.0]).map(|f| f.face);
    assert_eq!(at, Some(cut.clone()));
    let face = serde_json::to_value(&cut).unwrap();
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": face, "material": "Red"}}));
    let reds = |s: &Session| {
        let Some(crate::model::Element::Component(c)) = s.model.elements.get(id) else { panic!("the component") };
        s.model.component_pieces(c).iter().map(|p| painted(p).into_iter().filter(|(k, m)| *k == cut && m == "Red").count()).sum::<usize>()
    };
    assert!(reds(&s) > 0, "the instance's pocket side is red");
    // An unrelated edit, which the engine follows by dropping dead paint: the pocket's paint stays.
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": "end", "material": "Red"}}));
    run(&mut s, "family", json!({"name": "Box", "form": {"guid": gstr(BOX), "material": "Concrete", "to": 320}}));
    assert_eq!(form(&s).paint.len(), 2, "{:?}", form(&s).paint);
    assert!(reds(&s) > 0);
    // And a file keeps it.
    let (back, left_out) = crate::model::Model::read(&s.model.to_json().unwrap()).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(back.families["Box"].forms[0].paint.len(), 2);
}

/// ec's review of #89: a file's paint this version can't read, or naming a material the file doesn't hold, is left out and
/// listed, and the project opens; a command's paint is still read strictly, and the check names the paint.
#[test]
fn paint_a_file_cant_be_read_by_or_whose_material_is_missing_is_left_out_and_the_project_opens() {
    let (s, _) = project(json!([{"face": "end", "material": "Red"}]));
    let mut file: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    let paint = &mut file["families"]["Box"]["forms"][0]["paint"];
    let red = paint[0]["material"].clone();
    for p in [
        json!({"face": {"zzz": 1}, "material": red}),
        json!({"face": "start", "material": red, "gloss": 3}),
        json!({"face": {"side": gstr(2)}, "material": "6c1e0000-0000-4000-8000-000000000000"}),
        json!({"face": {"side": gstr(3)}, "material": "Gold"}),
    ] {
        paint.as_array_mut().unwrap().push(p);
    }
    let (back, left_out) = crate::model::Model::read(&file.to_string()).unwrap();
    assert_eq!(
        left_out,
        [
            "/families/Box/forms/0/paint/1",
            "/families/Box/forms/0/paint/2",
            "/families/Box/forms/0/paint/3",
            "/families/Box/forms/0/paint/4"
        ],
    );
    assert_eq!(back.families["Box"].forms[0].paint, form(&s).paint);
    back.check().unwrap();
    // A command is read strictly, and the check says it is the paint whose material is missing.
    let mut s = s;
    let e = s
        .execute("family", &json!({"name": "Box", "form": {"guid": gstr(BOX), "paint": [{"face": "end", "material": "Red", "gloss": 3}]}}))
        .unwrap_err();
    assert!(e.contains("gloss"), "{e}");
    let e = s
        .execute("family", &json!({"name": "Box", "form": {"guid": gstr(BOX), "paint": [{"face": "end", "material": "Gold"}]}}))
        .unwrap_err();
    assert!(e.contains("paint on \"end\"") && e.contains("Gold"), "{e}");
}
