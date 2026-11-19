//! FE-16c-2: a painted face's surface pattern in plan: a top painted in the project, as a whole component or by its family,
//! shows its material's surface pattern at Fine detail, as a floor's top does, hidden where a higher piece stands over it;
//! a side shows none, and a section's cut stays the form's material's.

use serde_json::{Value, json};

use crate::component_paint_tests::{BOX, OTHER, boxed, gstr, paint, project, rect, run};
use crate::engine::Session;
use crate::ids::Id;

/// A project as [`project`]'s, its family `Box` also with a post 200 square from 0 to 600 at (400..600, 150..350), standing
/// over the box's top; and a material `Tile` whose surface is a model pattern of lines running east, 100 apart.
fn tiled() -> (Session, Id) {
    let (mut s, id) = project();
    run(&mut s, "fill_pattern", json!({"name": "Tiles", "target": "model", "lines": [{"angle": 0, "spacing": 100}]}));
    run(&mut s, "material", json!({"name": "Tile", "color": [200, 200, 200], "surface": "Tiles"}));
    let post = json!({"guid": gstr(OTHER + 1), "sketch": rect(41, 400.0, 150.0, 600.0, 350.0), "from": 0, "to": 600});
    run(&mut s, "family", json!({"name": "Box", "forms": [boxed(), post]}));
    (s, id)
}

/// The pattern lines component `id` draws in the plan of level 100000 at `detail`, as model points.
fn pattern_lines(s: &mut Session, id: Id, detail: &str) -> Vec<[crate::geom::P2; 2]> {
    run(s, "detail", json!({"view": 100000, "detail": detail}));
    let view = crate::render::View { angle: 0.0, scale: 0.4, cx: 500.0, cy: 250.0, width: 800, height: 600 };
    let d = crate::render::draw(&s.model, 100000, view);
    d.prims
        .iter()
        .zip(&d.parts)
        .zip(&d.owners)
        .filter(|((_, part), owner)| **part == crate::render::Part::Pattern && **owner == Some(id))
        .filter_map(|((p, _), _)| match p {
            crate::render::Prim::Line { pts, .. } if pts.len() == 2 => Some([view.mm(pts[0]), view.mm(pts[1])]),
            _ => None,
        })
        .map(|[a, b]| [crate::geom::P2(a.0, a.1), crate::geom::P2(b.0, b.1)])
        .collect()
}

#[test]
fn a_painted_top_shows_its_materials_surface_pattern_in_plan_at_fine_hidden_under_a_higher_piece() {
    let (mut s, id) = tiled();
    assert!(pattern_lines(&mut s, id, "fine").is_empty(), "unpainted, no pattern, as before");
    paint(&mut s, id, json!("end"), json!("Tile"));
    let lines = pattern_lines(&mut s, id, "fine");
    assert!(lines.len() >= 4, "the top's tiles: {lines:?}");
    for [a, b] in &lines {
        // Running east, on the box's top, on the lattice from the model's origin.
        assert!((a.1 - b.1).abs() < 0.5 && a.1 > -1.0 && a.1 < 501.0, "{a:?} {b:?}");
        assert!((a.1 / 100.0 - (a.1 / 100.0).round()).abs() < 0.02, "on the lattice: {a:?}");
        assert!(a.0.min(b.0) > -1.0 && a.0.max(b.0) < 1001.0, "within the box: {a:?} {b:?}");
        // Not over the post, which stands higher: a line at y 200 or 300 stops at x 400 and starts again at 600.
        let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
        if a.1 > 151.0 && a.1 < 349.0 {
            assert!(hi < 401.0 || lo > 599.0, "under the post: {a:?} {b:?}");
        }
    }
    assert!(lines.iter().any(|[a, b]| (a.1 - 200.0).abs() < 0.5 && a.0.max(b.0) > 650.0), "past the post");
    // Coarse and medium draw none, as a floor's.
    assert!(pattern_lines(&mut s, id, "medium").is_empty());
    assert!(pattern_lines(&mut s, id, "coarse").is_empty());
    // The whole component's paint and the family's show it too.
    paint(&mut s, id, json!("end"), Value::Null);
    assert!(pattern_lines(&mut s, id, "fine").is_empty());
    run(&mut s, "paint", json!({"id": id, "face": "all", "material": "Tile"}));
    assert!(!pattern_lines(&mut s, id, "fine").is_empty(), "`all`");
    run(&mut s, "paint", json!({"id": id, "face": "all", "material": null}));
    run(&mut s, "family", json!({"name": "Box", "paint": {"form": gstr(BOX), "face": "end", "material": "Tile"}}));
    assert!(!pattern_lines(&mut s, id, "fine").is_empty(), "the family's paint");
    // The project's paint of the face over the family's: Red has no surface pattern, so none.
    paint(&mut s, id, json!("end"), json!("Red"));
    assert!(pattern_lines(&mut s, id, "fine").is_empty(), "the project's face paint wins");
}

#[test]
fn a_painted_side_shows_no_pattern_in_plan_and_cut_patterns_stay_the_forms() {
    let (mut s, id) = tiled();
    let section = run(&mut s, "section", json!({"start": [700, -1000], "end": [700, 1500]}))["id"].as_u64().unwrap();
    let cut = |s: &Session| {
        let v = crate::render::View { angle: 0.0, scale: 0.2, cx: 0.0, cy: 0.0, width: 800, height: 600 };
        crate::render::svg(&crate::render::section(&s.model, section, v))
    };
    let before = cut(&s);
    // A side faces no one looking down: no pattern in plan; the section's cut stays the form's material's.
    paint(&mut s, id, json!({"side": gstr(1)}), json!("Tile"));
    paint(&mut s, id, json!({"side": gstr(3)}), json!("Red"));
    assert!(pattern_lines(&mut s, id, "fine").is_empty());
    assert!(cut(&s) == before, "the section's cut is as it was");
}
