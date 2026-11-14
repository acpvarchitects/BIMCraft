//! FE-15 follow-up: a component's model lines show in the project's sections, elevations and 3D views as they do in its
//! plans: where they stand with it, at the detail levels and the ways of looking their Visibility Settings say (never in
//! projection when only shown when cut), hidden with their component or its category, inside a 3D view's section box only,
//! in the selection's colour while it is selected, and picking it.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::CENTER_FRONT_BACK;
use crate::graphics::Look;
use crate::model::Id;
use crate::render::Rgb;
use crate::view3d::{Camera, Image, Scene};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn id_of(v: Value) -> Id {
    v["id"].as_u64().unwrap()
}

/// Where the model line runs in the family, on its Center (Front/Back) plane, well away from its box: from (-900, 0, 200) to
/// (-300, 0, 800).
const FROM: [f64; 3] = [-900.0, 0.0, 200.0];
const TO: [f64; 3] = [-300.0, 0.0, 800.0];
const MIDDLE: [f64; 3] = [-600.0, 0.0, 500.0];

/// A generic family `P`, a small box at its origin and one model line with Visibility Settings `visibility` (none for
/// null), placed at the origin: the component's id.
fn placed(s: &mut Session, visibility: Value) -> Id {
    let mut line = json!({"start": FROM, "end": TO, "host": {"plane": CENTER_FRONT_BACK.to_string()}});
    if !visibility.is_null() {
        line["visibility"] = visibility;
    }
    run(
        s,
        "family",
        json!({"name": "P", "category": "generic", "types": {"A": {}},
               "forms": [{"profile": [[0, 0], [100, 0], [100, 50], [0, 50]], "to": 10}],
               "model_lines": [line]}),
    );
    id_of(run(s, "component", json!({"family": "P", "at": [0, 0]})))
}

/// Whether a pixel within 2 of `(x, y)` in `img` picks `id`, and one of those is coloured `color` if it says.
fn drawn_near(img: &Image, (x, y, _): (f64, f64, f64), id: Id, color: Option<Rgb>) -> bool {
    let around = (-2..=2).flat_map(|dx| (-2..=2).map(move |dy| (x + f64::from(dx), y + f64::from(dy))));
    let hits: Vec<(f64, f64)> = around.filter(|(x, y)| img.pick(*x, *y) == Some(id)).collect();
    !hits.is_empty() && color.is_none_or(|c| hits.iter().any(|(x, y)| img.color(*x, *y) == c))
}

/// A 3D view from the south-west, a little above, round (-300, -300, 500), which the line turned a quarter about the origin
/// is as near: 0.5 px a mm.
fn camera() -> Camera {
    Camera { azimuth: -120.0, elevation: 20.0, scale: 0.5, target: [-300.0, -300.0, 500.0], width: 800, height: 800, lens: None }
}

fn in_3d(s: &Session, tints: &[(Id, Rgb)]) -> Image {
    Scene::new(&s.model).render_in(&camera(), tints, &Look::of_3d(&s.model))
}

#[test]
fn a_components_model_line_shows_in_3d_where_it_stands_and_picks_it() {
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    let img = in_3d(&s, &[]);
    assert!(drawn_near(&img, camera().project(MIDDLE), id, Some(crate::view3d::EDGE)), "drawn at its middle, in the edges' colour");
    assert!(drawn_near(&img, camera().project([-750.0, 0.0, 350.0]), id, None), "and along it");
    assert!(!drawn_near(&img, camera().project([-600.0, 0.0, 900.0]), id, None), "not off it");
    // Selected, in the selection's colour.
    let red = Rgb(255, 0, 0);
    assert!(drawn_near(&in_3d(&s, &[(id, red)]), camera().project(MIDDLE), id, Some(red)), "selected, tinted");
}

#[test]
fn a_model_line_turns_with_its_component_in_3d() {
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    run(&mut s, "rotate", json!({"ids": [id], "about": [0, 0], "angle": 90}));
    // Turned a quarter about the origin: (-600, 0) goes to (0, -600).
    let img = in_3d(&s, &[]);
    assert!(drawn_near(&img, camera().project([0.0, -600.0, 500.0]), id, None), "where it stands now");
    assert!(!drawn_near(&img, camera().project(MIDDLE), id, None), "not where it stood");
}

#[test]
fn a_model_line_only_shown_when_cut_or_left_out_at_the_views_detail_isnt_drawn_in_3d() {
    let mut s = Session::new();
    let id = placed(&mut s, json!({"only_when_cut": true}));
    assert!(!drawn_near(&in_3d(&s, &[]), camera().project(MIDDLE), id, None), "never in projection");
    let mut s = Session::new();
    let id = placed(&mut s, json!({"fine": false}));
    run(&mut s, "detail", json!({"view": "{3D}", "detail": "fine"}));
    assert!(!drawn_near(&in_3d(&s, &[]), camera().project(MIDDLE), id, None), "left out at Fine");
    run(&mut s, "detail", json!({"view": "{3D}", "detail": "medium"}));
    assert!(drawn_near(&in_3d(&s, &[]), camera().project(MIDDLE), id, None), "drawn at Medium");
}

#[test]
fn a_model_line_is_hidden_with_its_component_and_kept_to_a_section_box() {
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    run(&mut s, "graphics", json!({"view": "{3D}", "hide": ["generic"]}));
    assert!(!drawn_near(&in_3d(&s, &[]), camera().project(MIDDLE), id, None), "its category hidden");
    run(&mut s, "graphics", json!({"view": "{3D}", "show": ["generic"]}));
    assert!(drawn_near(&in_3d(&s, &[]), camera().project(MIDDLE), id, None), "shown again");
    // A section box round its lower half keeps that half only.
    run(&mut s, "section_box", json!({"view": "{3D}", "min": [-2000, -2000, -100], "max": [2000, 2000, 450]}));
    let img = in_3d(&s, &[]);
    assert!(drawn_near(&img, camera().project([-750.0, 0.0, 350.0]), id, None), "inside the box");
    assert!(!drawn_near(&img, camera().project([-400.0, 0.0, 700.0]), id, None), "not above it");
}

/// Section `sec`'s picture, fitted, and where model point `p` is in it.
fn section_at(s: &Session, sec: Id, p: [f64; 3]) -> (Image, (f64, f64, f64)) {
    let view = crate::render::section_fit(&s.model, sec, 1000, 800);
    let img = crate::render::section_picture(&s.model, sec, view, &[]).unwrap();
    let (cam, _) = crate::render::section_camera(&s.model, sec, view).unwrap();
    (img, cam.project(p))
}

#[test]
fn a_components_model_line_shows_in_a_section_past_its_line_not_before_it() {
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    // Along y = -1000, looking north at it.
    let sec = id_of(run(&mut s, "section", json!({"start": [-3000, -1000], "end": [3000, -1000]})));
    assert!(s.model.section(sec).unwrap().looking().1 > 0.5, "looking north");
    let (img, at) = section_at(&s, sec, MIDDLE);
    assert!(drawn_near(&img, at, id, None), "seen past the line");
    // From the north, looking south: the line lies past it too.
    let back = id_of(run(&mut s, "section", json!({"start": [3000, 1000], "end": [-3000, 1000]})));
    assert!(drawn_near(&section_at(&s, back, MIDDLE).0, section_at(&s, back, MIDDLE).1, id, None), "from the other side");
    // A section whose line lies past it (y = 1000, looking north) leaves it out.
    let past = id_of(run(&mut s, "section", json!({"start": [-3000, 1000], "end": [3000, 1000]})));
    let (img, at) = section_at(&s, past, MIDDLE);
    assert!(!drawn_near(&img, at, id, None), "before the line, cut away");
}

#[test]
fn a_model_line_left_out_front_and_back_isnt_in_a_section_looking_along_its_y_but_is_in_one_along_its_x() {
    let mut s = Session::new();
    let id = placed(&mut s, json!({"front_back": false}));
    let north = id_of(run(&mut s, "section", json!({"start": [-3000, -1000], "end": [3000, -1000]})));
    let (img, at) = section_at(&s, north, MIDDLE);
    assert!(!drawn_near(&img, at, id, None), "looking along the family's y: Front/Back");
    // Looking east, along its x, from x = -2000: Left/Right, which shows it. Seen edge on there, it is a line up.
    let east = id_of(run(&mut s, "section", json!({"start": [-2000, 3000], "end": [-2000, -3000]})));
    assert!(s.model.section(east).unwrap().looking().0 > 0.5, "looking east");
    let (img, at) = section_at(&s, east, MIDDLE);
    assert!(drawn_near(&img, at, id, None), "looking along its x: Left/Right");
}

#[test]
fn a_components_model_line_shows_in_an_elevation() {
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    run(&mut s, "wall", json!({"points": [[-3000, 2000], [3000, 2000]]}));
    let wall = s.model.elements.iter().find(|(_, e)| matches!(e, crate::model::Element::Wall(_))).map(|(id, _)| *id).unwrap();
    // Its mark at (0, -2000), looking north at the wall: the line stands between.
    let e = id_of(run(&mut s, "elevation", json!({"on": wall, "at": [0, -2000]})));
    assert!(s.model.section(e).unwrap().looking().1 > 0.5, "looking north");
    let (img, at) = section_at(&s, e, MIDDLE);
    assert!(drawn_near(&img, at, id, None), "seen in the elevation");
}

#[test]
fn a_model_line_shows_in_wireframe_and_hidden_line_too() {
    for style in ["wireframe", "hidden_line"] {
        let mut s = Session::new();
        let id = placed(&mut s, Value::Null);
        run(&mut s, "visual_style", json!({"style": style}));
        assert!(drawn_near(&in_3d(&s, &[]), camera().project(MIDDLE), id, None), "{style}");
    }
}

#[test]
fn a_scene_brought_up_to_date_draws_a_moved_components_model_line_where_it_stands_now() {
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    let before = s.model.clone();
    let scene = Scene::new(&before);
    run(&mut s, "move", json!({"ids": [id], "by": [0, -300]}));
    let look = Look::of_3d(&s.model);
    let updated = scene.update(&before, &s.model).render_in(&camera(), &[], &look);
    updated.same_as(&Scene::new(&s.model).render_in(&camera(), &[], &look)).unwrap();
    assert!(drawn_near(&updated, camera().project([-600.0, -300.0, 500.0]), id, None), "where it stands now");
}

#[test]
fn a_sections_selection_repainted_paints_its_model_line_in_the_selections_colour() {
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    let sec = id_of(run(&mut s, "section", json!({"start": [-3000, -1000], "end": [3000, -1000]})));
    let (mut img, at) = section_at(&s, sec, MIDDLE);
    let red = Rgb(255, 0, 0);
    assert!(img.retint_section(&[(id, red)]));
    assert!(drawn_near(&img, at, id, Some(red)), "red where selected");
    assert!(img.retint_section(&[]));
    assert!(!drawn_near(&img, at, id, Some(red)) && drawn_near(&img, at, id, None), "back to its own colour");
}

#[test]
fn the_cards_mesh_draws_and_picks_a_model_line_as_the_processors_picture_does() {
    use crate::view3d::gpu::Mesh;
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    let (scene, look) = (Scene::new(&s.model), Look::of_3d(&s.model));
    let mesh = Mesh::new(&scene, &look).unwrap();
    assert_eq!(mesh.strokes_cut(&[], None), vec![([FROM, TO], crate::view3d::EDGE)], "its line, in the edges' colour");
    let red = Rgb(255, 0, 0);
    assert_eq!(mesh.strokes_cut(&[(id, red)], None)[0].1, red, "tinted when selected");
    // A section box round its lower half keeps that half.
    let half = mesh.strokes_cut(&[], Some(([-2000.0, -2000.0, -100.0], [2000.0, 2000.0, 450.0])));
    assert!(half.len() == 1 && (half[0].0[1][2] - 450.0).abs() < 1e-6, "{half:?}");
    // Picked where the processor's picture picks it.
    let (x, y, _) = camera().project(MIDDLE);
    let near = (-2..=2).flat_map(|dx| (-2..=2).map(move |dy| (x + f64::from(dx), y + f64::from(dy))));
    let picked: Vec<(f64, f64)> = near.filter(|(x, y)| mesh.pick(&camera(), &[], *x, *y).is_some_and(|h| h.id == id)).collect();
    assert!(!picked.is_empty(), "the card's mesh picks it");
    let img = in_3d(&s, &[]);
    assert!(picked.iter().all(|(x, y)| img.pick(*x, *y) == Some(id)), "where the processor's picture does");
    // Hidden with its category, the card draws none.
    run(&mut s, "graphics", json!({"view": "{3D}", "hide": ["generic"]}));
    let look = Look::of_3d(&s.model);
    assert!(Mesh::new(&Scene::new(&s.model), &look).unwrap().strokes_cut(&[], None).is_empty());
}

#[test]
fn in_wireframe_the_cards_mesh_picks_a_model_line_where_the_processors_picture_does() {
    use crate::view3d::gpu::Mesh;
    let mut s = Session::new();
    let id = placed(&mut s, Value::Null);
    run(&mut s, "visual_style", json!({"style": "wireframe"}));
    let (scene, look) = (Scene::new(&s.model), Look::of_3d(&s.model));
    let mesh = Mesh::new(&scene, &look).unwrap();
    let img = in_3d(&s, &[]);
    // At the line's middle and two pixels either side of it, as a wireframe's edges are picked.
    let (x, y, _) = camera().project(MIDDLE);
    for (dx, dy) in [(0.0, 0.0), (2.0, 0.0), (-2.0, 0.0), (0.0, 2.0), (0.0, -2.0)] {
        let (px, py) = (x.floor() + 0.5 + dx, y.floor() + 0.5 + dy);
        assert_eq!(img.pick(px, py), Some(id), "the processor's picture at ({dx}, {dy})");
        assert_eq!(mesh.pick(&camera(), &[], px, py).map(|h| h.id), Some(id), "the card's mesh at ({dx}, {dy})");
    }
    // Every pixel round it picks alike, selected or not.
    for tints in [vec![], vec![(id, Rgb(255, 0, 0))]] {
        let img = in_3d(&s, &tints);
        for py in (y as i64 - 8)..(y as i64 + 8) {
            for px in (x as i64 - 8)..(x as i64 + 8) {
                let (fx, fy) = (px as f64 + 0.5, py as f64 + 0.5);
                assert_eq!(mesh.pick(&camera(), &tints, fx, fy).map(|h| h.id), img.pick(fx, fy), "pixel ({px}, {py})");
            }
        }
    }
}
