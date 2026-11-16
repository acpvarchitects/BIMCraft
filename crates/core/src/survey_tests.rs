//! Survey areas and drawn items (`survey.rs`, `commands/survey.rs`): an outline on a face in its own coordinates, which
//! follows its host moved, lengthened, thickened and retyped; measured on the true surface, less the openings that cut
//! it; kept in conflict off its face; surveys as library entries; undo; files.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::model::{Element, Id, Model};
use crate::survey::SurveyArea;
use crate::view3d::P3;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn err(s: &mut Session, id: &str, p: Value) -> String {
    match s.execute(id, &p) {
        Ok(r) => panic!("{id} {p} should have been refused, and did {r}"),
        Err(e) => e,
    }
}

fn first(v: &Value) -> Id {
    v["ids"].get(0).or(Some(&v["id"])).and_then(Value::as_u64).unwrap()
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

fn near3(a: P3, b: P3) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

fn area_of(s: &Session, id: Id) -> &SurveyArea {
    match s.model.elements.get(id) {
        Some(Element::SurveyArea(a)) => a,
        other => panic!("{id} is no survey area: {other:?}"),
    }
}

/// A metre square of black crust on the exterior of a wall 6 m long and 3 m tall, from 1 m along and 0.5 m up.
fn crusted() -> (Session, Id, Id) {
    let mut s = Session::new();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [6000, 0]], "height": 3000})));
    let a = first(&run(
        &mut s,
        "survey_area",
        json!({"host": w, "face": "exterior", "outline": [[1000, 500], [2000, 500], [2000, 1500], [1000, 1500]],
               "condition": "crust", "severity": 3, "description": "Black crust under the sill", "depth": 5}),
    ));
    (s, w, a)
}

#[test]
fn an_area_drawn_on_a_wall_measures_its_outline_and_holds_no_shape_of_its_own() {
    let (mut s, w, a) = crusted();
    let v = s.execute("survey_area", &json!({"id": a})).unwrap();
    assert!(near(v["area"].as_f64().unwrap(), 1e6, 1e-6), "{v}");
    assert_eq!(v["net_area"], v["area"]);
    assert_eq!(v["cut"], json!(0.0));
    assert!(near(v["volume"].as_f64().unwrap(), 5e6, 1e-3), "{v}");
    assert_eq!(v["conflict"], Value::Null);
    let area = area_of(&s, a);
    assert_eq!(area.host, s.model.guid(w));
    assert_eq!(area.condition, Some(crate::decay::Decay::Crust));
    // Listed with the elements, with its quantities.
    let listed = crate::commands::element_json(&s.model, a);
    assert_eq!(listed["category"], "surveyarea");
    assert!(near(listed["area"].as_f64().unwrap(), 1e6, 1e-6), "{listed}");
    // Nothing of the wall changed: its faces measure as before.
    let ext = s.model.face_chart(s.model.guid(w), crate::face::FaceId::Exterior).unwrap();
    assert!(near(ext.area(), 18e6, 1e-3));
}

#[test]
fn an_area_follows_its_wall_moved_lengthened_thickened_and_retyped() {
    let (mut s, w, a) = crusted();
    let corner = |s: &Session| s.model.survey_resolved(area_of(s, a)).outline[0];
    assert!(near3(corner(&s), [1000.0, 100.0, 500.0]), "{:?}", corner(&s));
    run(&mut s, "move", json!({"ids": [w], "by": [0, 1000]}));
    assert!(near3(corner(&s), [1000.0, 1100.0, 500.0]), "moved: {:?}", corner(&s));
    run(&mut s, "modify", json!({"id": w, "end": [9000, 1000]}));
    assert!(near3(corner(&s), [1000.0, 1100.0, 500.0]), "lengthened: {:?}", corner(&s));
    run(&mut s, "type", json!({"category": "wall", "name": "Thick", "thickness": 600}));
    run(&mut s, "modify", json!({"id": w, "type": "Thick"}));
    assert!(near3(corner(&s), [1000.0, 1300.0, 500.0]), "retyped, thicker: {:?}", corner(&s));
    run(&mut s, "type", json!({"name": "Thick", "category": "wall", "thickness": 1000}));
    assert!(near3(corner(&s), [1000.0, 1500.0, 500.0]), "its type thickened: {:?}", corner(&s));
    let r = s.model.survey_resolved(area_of(&s, a));
    assert!(near(r.area, 1e6, 1e-6) && r.conflict.is_none(), "{r:?}");
    // Turned with it.
    run(&mut s, "rotate", json!({"ids": [w], "about": [0, 0], "angle": 90}));
    assert!(near3(corner(&s), [-1500.0, 1000.0, 500.0]), "turned: {:?}", corner(&s));
    // Moved alone, it says where to go instead.
    let e = err(&mut s, "move", json!({"ids": [a], "by": [100, 0]}));
    assert!(e.contains("outline"), "{e}");
    // Moved with its host, it goes along.
    run(&mut s, "move", json!({"ids": [w, a], "by": [100, 0]}));
    assert!(near3(corner(&s), [-1400.0, 1000.0, 500.0]), "{:?}", corner(&s));
}

#[test]
fn an_area_on_a_curved_wall_measures_its_developed_area() {
    let mut s = Session::new();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "arc": 180, "height": 3000})));
    // Along the whole of the interior side, a metre tall: its arc length by a metre.
    let faces = run(&mut s, "faces", json!({"id": w, "face": "interior"}));
    let len = faces["faces"][0]["outline"][0].as_array().unwrap().iter().map(|q| q[0].as_f64().unwrap()).fold(0.0, f64::max);
    let a = first(&run(
        &mut s,
        "survey_area",
        json!({"host": w, "face": "interior", "outline": [[0, 1000], [len, 1000], [len, 2000], [0, 2000]], "condition": "efflorescence"}),
    ));
    let r = s.model.survey_resolved(area_of(&s, a));
    // `u` is along the location line (radius 2000); the interior side, radius 2100, is that much longer.
    assert!(near(len, std::f64::consts::PI * 2000.0, 1e-6), "{len}");
    let developed = std::f64::consts::PI * 2100.0 * 1000.0;
    assert!(near(r.area, developed, 1e-3), "{} against {developed}", r.area);
    assert!(r.area > 4000.0 * 1000.0, "more than its chord: {}", r.area);
    assert!(r.conflict.is_none(), "{r:?}");
    // In space its outline follows the curve, every point on the side's radius.
    let radii: Vec<f64> = r.outline.iter().map(|p| (p[0] - 2000.0).hypot(p[1])).collect();
    assert!(r.outline.len() > 20, "{}", r.outline.len());
    assert!(radii.iter().all(|x| near(*x, radii[0], 1e-6)), "{radii:?}");
}

#[test]
fn an_area_over_a_window_in_a_curved_wall_stays_over_it_thickened_retyped_and_its_line_moved() {
    let mut s = Session::new();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "arc": 180, "height": 3000})));
    run(&mut s, "window", json!({"wall": w, "offset": 3000}));
    // An outline the window's own hole on the exterior: all of it is cut.
    let hole = s.model.face_chart(s.model.guid(w), crate::face::FaceId::Exterior).unwrap().openings[0].clone();
    let outline: Vec<[f64; 2]> = hole.iter().map(|q| [q.0, q.1]).collect();
    let a = first(&run(&mut s, "survey_area", json!({"host": w, "face": "exterior", "outline": outline, "condition": "crack"})));
    let all_cut = |s: &Session| {
        let r = s.model.survey_resolved(area_of(s, a));
        near(r.cut_share, 1.0, 1e-9) && r.conflict.is_none()
    };
    assert!(all_cut(&s));
    run(&mut s, "type", json!({"category": "wall", "name": "Thick", "thickness": 700}));
    run(&mut s, "modify", json!({"id": w, "type": "Thick"}));
    assert!(all_cut(&s), "retyped thicker: {:?}", s.model.survey_resolved(area_of(&s, a)));
    run(&mut s, "type", json!({"category": "wall", "name": "Thick", "thickness": 900}));
    assert!(all_cut(&s), "its type thickened");
    run(&mut s, "modify", json!({"id": w, "location": "exterior-finish"}));
    assert!(all_cut(&s), "its location line moved: {:?}", s.model.survey_resolved(area_of(&s, a)));
}

#[test]
fn an_opening_cut_later_takes_its_share_out() {
    let (mut s, _, _) = crusted();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 5000], [6000, 5000]], "height": 3000})));
    let a = first(&run(
        &mut s,
        "survey_area",
        json!({"host": w, "face": "interior", "outline": [[500, 0], [4500, 0], [4500, 2800], [500, 2800]], "condition": "moist-area", "depth": 20}),
    ));
    let before = s.model.survey_resolved(area_of(&s, a));
    assert!(near(before.area, 4000.0 * 2800.0, 1e-3) && before.cut == 0.0);
    run(&mut s, "door", json!({"at": [2000, 5000]}));
    let after = s.model.survey_resolved(area_of(&s, a));
    let ext = s.model.face_chart(s.model.guid(w), crate::face::FaceId::Interior).unwrap();
    let door = &ext.openings[0];
    let width = door.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max) - door.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
    let height = door.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
    assert!(near(after.area, before.area, 1e-3), "{after:?}");
    assert!(near(after.cut, width * height, 1e-3), "{} against {}", after.cut, width * height);
    assert!(near(after.net, before.area - width * height, 1e-3));
    assert!(near(after.cut_share, width * height / before.area, 1e-9));
    assert!(near(after.volume.unwrap(), after.net * 20.0, 1e-3));
    let v = run(&mut s, "survey_area", json!({"id": a}));
    assert!(near(v["cut_share"].as_f64().unwrap(), after.cut_share, 1e-12), "{v}");
}

#[test]
fn an_outline_off_its_face_keeps_its_data_and_is_in_conflict() {
    let (mut s, w, a) = crusted();
    // Shortened to 1.5 m: half the square is past the wall's end.
    run(&mut s, "modify", json!({"id": w, "end": [1500, 0]}));
    let r = s.model.survey_resolved(area_of(&s, a));
    assert!(near(r.area, 500_000.0, 1e-3) && near(r.outside, 500_000.0, 1e-3), "{r:?}");
    assert!(r.conflict.as_deref().is_some_and(|c| c.contains("off")), "{r:?}");
    // Shorter still: all of it is off, and it is still there, with all it said.
    run(&mut s, "modify", json!({"id": w, "end": [800, 0]}));
    let r = s.model.survey_resolved(area_of(&s, a));
    assert_eq!(r.area, 0.0);
    assert!(r.conflict.is_some());
    assert_eq!(area_of(&s, a).description, "Black crust under the sill");
    let v = run(&mut s, "survey_area", json!({"id": a}));
    assert!(v["conflict"].is_string(), "{v}");
    // Back to its length: no conflict.
    run(&mut s, "modify", json!({"id": w, "end": [6000, 0]}));
    assert!(s.model.survey_resolved(area_of(&s, a)).conflict.is_none());
    // A face its host no longer has: a floor's side past its edges once its outline lost a corner.
    let f = first(&run(&mut s, "floor", json!({"boundary": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]]})));
    let edge = first(&run(
        &mut s,
        "survey_area",
        json!({"host": f, "face": {"side": 3}, "outline": [[100, 50], [900, 50], [900, 150], [100, 150]]}),
    ));
    run(&mut s, "modify", json!({"id": f, "boundary": [[0, 0], [6000, 0], [0, 4000]]}));
    let r = s.model.survey_resolved(area_of(&s, edge));
    assert!(r.conflict.as_deref().is_some_and(|c| c.contains("side 3")), "{r:?}");
}

#[test]
fn an_area_is_refused_what_doesnt_make_sense_and_goes_with_its_host() {
    let (mut s, w, a) = crusted();
    let square = json!([[0, 0], [100, 0], [100, 100], [0, 100]]);
    let cases = [
        (json!({"host": w, "face": "top", "outline": [[0, 0], [100, 0]]}), "3 to"),
        (json!({"host": w, "face": "exterior", "outline": [[0, 0], [100, 0], [200, 0]]}), "no area"),
        (json!({"host": w, "face": "exterior", "outline": [[0, 0], [100, 100], [100, 0], [0, 100]]}), "crosses itself"),
        (json!({"host": w, "face": {"side": 0}, "outline": square}), "no side 0"),
        (json!({"host": w, "face": "exterior", "outline": square, "severity": 6}), "severity"),
        (json!({"host": w, "face": "exterior", "outline": square, "condition": "rust"}), "condition"),
        (json!({"host": w, "face": "exterior", "outline": square, "survey": "Spring"}), "no survey"),
        (json!({"host": w, "face": "exterior", "outline": square, "photos": ["../secret.jpg"]}), "photo"),
        (json!({"host": w, "face": "exterior", "outline": square, "depth": -1}), "depth"),
        (json!({"host": w, "face": "exterior", "outline": square, "holes": [[[500, 500], [600, 500], [600, 600]]]}), "hole 0"),
        (json!({"host": a, "face": "exterior", "outline": square}), "no faces"),
        (json!({"face": "exterior", "outline": square}), "host"),
    ];
    for (p, says) in cases {
        let e = err(&mut s, "survey_area", p.clone());
        assert!(e.contains(says), "{p}: {e}");
    }
    // A custom condition, an item, photos.
    let item = first(&run(
        &mut s,
        "survey_area",
        json!({"host": w, "face": "exterior", "outline": [[3000, 0], [3300, 0], [3300, 200], [3000, 200]], "item": "stone",
               "condition": {"custom": "Salt fringe"}, "photos": ["photos/north-01.jpg"], "intervention": "Desalination"}),
    ));
    assert_eq!(area_of(&s, item).kind, crate::survey::SurveyKind::Item("stone".into()));
    assert_eq!(area_of(&s, item).condition, Some(crate::decay::Decay::Custom("Salt fringe".into())));
    // Deleted with its host.
    run(&mut s, "delete", json!({"ids": [w]}));
    assert!(s.model.elements.get(a).is_none() && s.model.elements.get(item).is_none());
}

#[test]
fn surveys_are_named_campaigns_areas_follow_renamed_and_hold_by_guid_in_a_file() {
    let (mut s, _, a) = crusted();
    run(&mut s, "survey", json!({"name": "Spring 2026", "date": "2026-04-12", "method": "thermography"}));
    assert!(err(&mut s, "survey", json!({"name": "Bad", "date": "2026-02-30"})).contains("YYYY-MM-DD"));
    assert!(err(&mut s, "survey", json!({"name": "Bad", "method": "dowsing"})).contains("method"));
    run(&mut s, "survey_area", json!({"id": a, "survey": "Spring 2026"}));
    let entry = s.model.library.id(&crate::library::Kind::Survey, "Spring 2026");
    assert!(entry.is_some());
    // In a file, the area holds the survey's GUID; read back, its name.
    let text = s.model.to_json().unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    let held = file["elements"].as_object().unwrap().values().find(|e| e["category"] == "surveyarea").unwrap()["survey"].clone();
    assert_ne!(held, json!("Spring 2026"), "{held}");
    let (back, left_out) = Model::read(&text).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(back.elements.get(a), s.model.elements.get(a));
    assert_eq!(back.surveys, s.model.surveys);
    // Renamed: the area follows, the entry keeps its id. One in use can't be deleted.
    run(&mut s, "survey", json!({"name": "Spring 2026", "rename": "April survey"}));
    assert_eq!(area_of(&s, a).survey.as_deref(), Some("April survey"));
    assert_eq!(s.model.library.id(&crate::library::Kind::Survey, "April survey"), entry);
    assert!(err(&mut s, "survey", json!({"name": "April survey", "delete": true})).contains(&a.to_string()));
    run(&mut s, "survey_area", json!({"id": a, "survey": null}));
    run(&mut s, "survey", json!({"name": "April survey", "delete": true}));
    assert!(s.model.surveys.is_empty());
    let listed = run(&mut s, "survey", json!({}));
    assert_eq!(listed["surveys"], json!({}));
}

#[test]
fn undo_takes_an_area_and_its_edits_back() {
    let (mut s, _, a) = crusted();
    run(&mut s, "survey_area", json!({"id": a, "severity": 5, "outline": [[0, 0], [500, 0], [500, 500], [0, 500]]}));
    assert_eq!(area_of(&s, a).severity, Some(5));
    run(&mut s, "undo", json!({}));
    assert_eq!(area_of(&s, a).severity, Some(3));
    assert_eq!(area_of(&s, a).outline.len(), 4);
    run(&mut s, "undo", json!({}));
    assert!(s.model.elements.get(a).is_none());
    run(&mut s, "redo", json!({}));
    assert!(s.model.elements.get(a).is_some());
}

#[test]
fn an_area_reads_back_from_json_as_it_was() {
    let (mut s, _, a) = crusted();
    run(&mut s, "survey_area", json!({"id": a, "holes": [[[1200, 700], [1400, 700], [1400, 900], [1200, 900]]], "item": "brick"}));
    let area = area_of(&s, a).clone();
    let v = serde_json::to_value(Element::SurveyArea(area.clone())).unwrap();
    assert_eq!(v["category"], "surveyarea");
    assert_eq!(v["face"], "exterior");
    assert_eq!(v["kind"], json!({"item": "brick"}));
    assert_eq!(v["condition"], "crust");
    let back: Element = serde_json::from_value(v).unwrap();
    assert_eq!(back, Element::SurveyArea(area));
    // A side reads as {"side": i}.
    let side: crate::face::FaceId = serde_json::from_value(json!({"side": 2})).unwrap();
    assert_eq!(side, crate::face::FaceId::Side(2));
    // The whole project, through a file and back.
    let text = s.model.to_json().unwrap();
    let (back, _) = Model::read(&text).unwrap();
    assert_eq!(back.to_json().unwrap(), text);
}

#[test]
fn a_project_without_surveys_saves_as_it_always_did() {
    let mut s = Session::new();
    s.script(crate::sample::SAMPLE).unwrap();
    let text = s.model.to_json().unwrap();
    assert!(!text.contains("\"surveys\"") && !text.contains("surveyarea"), "an unused feature leaves no trace in the file");
    let (back, left_out) = Model::read(&text).unwrap();
    assert!(left_out.is_empty());
    assert_eq!(back.to_json().unwrap(), text);
}

#[test]
fn a_schedule_of_survey_areas_lists_their_condition_and_net_quantities() {
    let (mut s, w, _) = crusted();
    run(&mut s, "door", json!({"at": [1500, 0]}));
    let r = run(
        &mut s,
        "schedule",
        json!({"name": "Condition survey", "category": "surveyarea",
        "fields": ["Kind", "Condition", "Severity", "Area", "Openings Area", "Volume", "Face", {"name": "Count", "total": true}]}),
    );
    let rows = r["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{r}");
    let row = rows[0].as_array().unwrap();
    assert_eq!(row[0], "Survey Area");
    assert_eq!(row[1], "Crust");
    assert_eq!(row[2], json!(3.0));
    let (net, cut) = (row[3].as_f64().unwrap(), row[4].as_f64().unwrap());
    assert!(cut > 0.0 && near(net + cut, 1.0, 1e-9), "{row:?}");
    assert!(near(row[5].as_f64().unwrap(), net * 0.005, 1e-9), "{row:?}");
    assert_eq!(row[6], "exterior");
    let _ = w;
}

#[test]
fn drawing_an_area_changes_nothing_of_its_host_in_ifc() {
    let mut s = Session::new();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [6000, 0]], "height": 3000})));
    let entities = |m: &Model| {
        let text = crate::ifc::export(m);
        let mut kinds: Vec<String> = text
            .lines()
            .filter_map(|l| l.split_once('=').map(|(_, r)| r.split('(').next().unwrap_or_default().trim().to_string()))
            .collect();
        kinds.sort();
        kinds
    };
    let before = entities(&s.model);
    run(&mut s, "survey", json!({"name": "First", "method": "visual"}));
    run(
        &mut s,
        "survey_area",
        json!({"host": w, "face": "exterior", "outline": [[0, 0], [500, 0], [500, 500], [0, 500]], "condition": "graffiti", "survey": "First"}),
    );
    assert_eq!(entities(&s.model), before);
}

/// A file saved by the commit this work starts from (format 20, before survey areas): opened and saved again, the same
/// bytes.
#[test]
fn a_file_from_before_survey_areas_opens_and_saves_byte_identical() {
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/format20-before-surveys.bimcraft");
    let before = std::fs::read(&from).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let to = dir.path().join("again.bimcraft");
    let mut s = Session::new();
    let opened = run(&mut s, "open", json!({"path": from.to_string_lossy()}));
    assert!(opened["left_out"].as_array().is_none_or(|l| l.is_empty()), "{opened}");
    run(&mut s, "save", json!({"path": to.to_string_lossy()}));
    let after = std::fs::read(&to).unwrap();
    assert!(after == before, "{} bytes against {}", after.len(), before.len());
    // Its walls, floor, roof and column have faces to survey.
    let curved = s.model.elements.iter().find(|(_, e)| matches!(e, Element::Wall(w) if w.arc != 0.0)).map(|(id, _)| *id).unwrap();
    assert_eq!(s.model.faces_of(s.model.guid(curved)).len(), 6);
}

/// The area's outline in space, its points rounded to a micron and sorted, to compare as a set.
fn points_of(s: &Session, a: Id) -> Vec<[i64; 3]> {
    let r = s.model.survey_resolved(area_of(s, a));
    let mut out: Vec<[i64; 3]> = r.outline.iter().map(|p| p.map(|c| (c * 1000.0).round() as i64)).collect();
    out.sort_unstable();
    out
}

fn mirrored_x(points: &[[i64; 3]], x: i64) -> Vec<[i64; 3]> {
    let mut out: Vec<[i64; 3]> = points.iter().map(|p| [2 * x * 1000 - p[0], p[1], p[2]]).collect();
    out.sort_unstable();
    out
}

#[test]
fn a_flipped_wall_keeps_its_areas_where_they_are_on_its_other_side() {
    let (mut s, w, a) = crusted();
    let before = points_of(&s, a);
    run(&mut s, "flip", json!({"ids": [w]}));
    assert_eq!(area_of(&s, a).face, crate::face::FaceId::Interior);
    assert_eq!(points_of(&s, a), before);
    let r = s.model.survey_resolved(area_of(&s, a));
    assert!(r.conflict.is_none() && near(r.area, 1e6, 1e-6), "{r:?}");
    // An area on its start goes to its end, as far across from its exterior as it was from its interior.
    let end =
        first(&run(&mut s, "survey_area", json!({"host": w, "face": "start", "outline": [[20, 100], [60, 100], [60, 300], [20, 300]]})));
    let before = points_of(&s, end);
    run(&mut s, "flip", json!({"ids": [w]}));
    assert_eq!(area_of(&s, end).face, crate::face::FaceId::End);
    assert_eq!(points_of(&s, end), before);
    assert_eq!(area_of(&s, a).face, crate::face::FaceId::Exterior);
}

#[test]
fn a_mirrored_wall_takes_its_areas_with_it_from_its_other_end() {
    let (mut s, w, a) = crusted();
    let before = points_of(&s, a);
    run(&mut s, "mirror", json!({"ids": [w], "axis": [[8000, 0], [8000, 1000]]}));
    assert_eq!(points_of(&s, a), mirrored_x(&before, 8000));
    assert!(s.model.survey_resolved(area_of(&s, a)).conflict.is_none());
    // Selected with it, the same.
    let before = points_of(&s, a);
    run(&mut s, "mirror", json!({"ids": [w, a], "axis": [[0, 0], [0, 1000]]}));
    assert_eq!(points_of(&s, a), mirrored_x(&before, 0));
}

#[test]
fn an_area_on_a_top_goes_with_its_host_moved_turned_and_mirrored() {
    let mut s = Session::new();
    let f = first(&run(&mut s, "floor", json!({"boundary": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]]})));
    let a = first(&run(
        &mut s,
        "survey_area",
        json!({"host": f, "face": "top", "outline": [[1000, 1000], [2000, 1000], [2000, 1500], [1000, 1500]], "condition": "deposit"}),
    ));
    let before = points_of(&s, a);
    run(&mut s, "move", json!({"ids": [f], "by": [500, 0]}));
    let moved: Vec<[i64; 3]> = before.iter().map(|p| [p[0] + 500_000, p[1], p[2]]).collect();
    assert_eq!(points_of(&s, a), moved);
    run(&mut s, "mirror", json!({"ids": [f], "axis": [[10000, 0], [10000, 1000]]}));
    assert_eq!(points_of(&s, a), mirrored_x(&moved, 10000));
    run(&mut s, "rotate", json!({"ids": [f], "about": [0, 0], "angle": 90}));
    let r = s.model.survey_resolved(area_of(&s, a));
    assert!(r.conflict.is_none() && near(r.area, 500_000.0, 1e-3), "{r:?}");
    // A column's top too.
    run(&mut s, "type", json!({"category": "column", "name": "Pier 600", "width": 600, "depth": 600}));
    let c = first(&run(&mut s, "column", json!({"at": [20000, 0], "type": "Pier 600", "height": 3000})));
    let on = first(&run(
        &mut s,
        "survey_area",
        json!({"host": c, "face": "top", "outline": [[19800, -200], [20000, -200], [20000, 0], [19800, 0]]}),
    ));
    run(&mut s, "move", json!({"ids": [c], "by": [0, 3000]}));
    let r = s.model.survey_resolved(area_of(&s, on));
    assert!(r.conflict.is_none() && near(r.outline[0][1], 2800.0, 1e-6), "{r:?}");
}

#[test]
fn an_area_copied_with_its_host_is_on_the_copy() {
    let (mut s, w, a) = crusted();
    let made = run(&mut s, "copy", json!({"ids": [w, a], "by": [0, 5000]}));
    let copies: Vec<Id> = made["ids"].as_array().unwrap().iter().filter_map(Value::as_u64).collect();
    let copy = copies.iter().copied().find(|id| matches!(s.model.elements.get(*id), Some(Element::SurveyArea(_)))).unwrap();
    let wall = copies.iter().copied().find(|id| s.model.wall(*id).is_some()).unwrap();
    assert_eq!(area_of(&s, copy).host, s.model.guid(wall));
    let moved: Vec<[i64; 3]> = points_of(&s, a).iter().map(|p| [p[0], p[1] + 5_000_000, p[2]]).collect();
    assert_eq!(points_of(&s, copy), moved);
}
