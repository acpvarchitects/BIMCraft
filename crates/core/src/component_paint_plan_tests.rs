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

/// How dark section `id`'s picture is at model point `(x, z)` on the line it looks along from `start_x`: the darkest of the
/// pixels within two of it up and down, as the sum of its channels.
fn darkest(s: &Session, id: Id, x: f64, z: f64) -> u32 {
    let view = crate::render::View { angle: 0.0, scale: 0.5, cx: 1500.0, cy: 150.0, width: 800, height: 400 };
    let img = crate::render::section_picture(&s.model, id, view, &[]).unwrap();
    let (px, py) = (400.0 + (x + 1000.0 - 1500.0) * 0.5, 200.0 - (z - 150.0) * 0.5);
    (-2..=2)
        .map(|k| {
            let c = img.color(px, py + f64::from(k));
            u32::from(c.0) + u32::from(c.1) + u32::from(c.2)
        })
        .min()
        .unwrap_or(0)
}

#[test]
fn a_painted_front_face_shows_its_pattern_in_an_elevation_only_where_nothing_nearer_hides_it() {
    let (mut s, id) = tiled();
    // A box of `Other`'s, 200 high, standing between the elevation and the right part of the box's front (x 600 to 1400).
    run(&mut s, "component", json!({"family": "Other", "type": "X", "at": [600, -900]}));
    // Looking north from y = -1000, its line from x = -1000.
    let section = run(&mut s, "section", json!({"start": [-1000, -1000], "end": [2000, -1000]}))["id"].as_u64().unwrap();
    let paper = darkest(&s, section, 300.0, 100.0);
    assert!(paper > 700, "the front face is paper in hidden line: {paper}");
    // Its front, side 1 (y = 0), painted Tile: its courses, every 100 up, show where nothing is nearer.
    paint(&mut s, id, json!({"side": gstr(1)}), json!("Tile"));
    assert!(darkest(&s, section, 300.0, 100.0) < 600, "a course at 100 on the front's left part");
    assert!(darkest(&s, section, 300.0, 250.0) > 700, "between courses, paper");
    assert!(darkest(&s, section, 800.0, 100.0) > 700, "none on Other's nearer face, which hides the front there");
    // Painted on its back (side 3), which faces away, nothing shows.
    paint(&mut s, id, json!({"side": gstr(1)}), Value::Null);
    paint(&mut s, id, json!({"side": gstr(3)}), json!("Tile"));
    assert!(darkest(&s, section, 300.0, 100.0) > 700, "a face turned away shows no pattern");
}

/// ec's review of FE-16c-2: what a plan finds and keeps of a build for its painted tops (the faces that face up, a pattern's
/// lines over them) is charged to the store's budget, so a budget the builds alone fit in, but not with those, lets some
/// go; and the bytes the store counts are those its shapes sum to.
#[test]
fn what_a_plan_keeps_of_a_build_for_its_painted_tops_is_charged_to_the_stores_budget() {
    use crate::component_geometry::{kept_bytes, kept_shapes, with_budgets};
    let mut s = Session::new();
    run(&mut s, "fill_pattern", json!({"name": "Fine", "target": "model", "lines": [{"angle": 0, "spacing": 1}]}));
    run(&mut s, "material", json!({"name": "Fine", "color": [200, 200, 200], "surface": "Fine"}));
    let sketch = json!([{"start": [0, 0], "end": ["W", 0]}, {"start": ["W", 0], "end": ["W", 300]},
                        {"start": ["W", 300], "end": [0, 300]}, {"start": [0, 300], "end": [0, 0]}]);
    run(
        &mut s,
        "family",
        json!({"name": "Tile", "category": "generic", "parameters": [{"name": "W", "value": 300, "instance": true}],
               "types": {"T": {}}, "forms": [{"guid": gstr(BOX), "sketch": sketch, "from": 0, "to": 100}]}),
    );
    // Twenty, each a shape of its own.
    let mut ids = Vec::new();
    for i in 0..20u32 {
        let at = [f64::from(i % 5) * 400.0, f64::from(i / 5) * 400.0];
        let w = 300.0 + f64::from(i) * 0.5;
        ids.push(run(&mut s, "component", json!({"family": "Tile", "type": "T", "at": at, "params": {"W": w}}))["id"].as_u64().unwrap());
    }
    let view = crate::render::View { angle: 0.0, scale: 1.0, cx: 900.0, cy: 700.0, width: 2400, height: 2000 };
    let draw = |s: &Session| {
        crate::render::draw(&s.model, 100000, view);
    };
    // The builds alone, with room to spare: each found once, all kept.
    let roomy = with_budgets([1 << 30, 1 << 30], || {
        draw(&s);
        kept_bytes()
    });
    assert_eq!(roomy.0, roomy.1);
    let budget = roomy.0[0] * 3 / 2;
    // Painted, a line a millimetre apart over each top: what the plan keeps for them doesn't fit beside the builds.
    let form = s.model.families["Tile"].forms[0].guid;
    run(&mut s, "paint", json!({"ids": ids, "form": form, "face": "end", "material": "Fine"}));
    let (bare, painted) = with_budgets([budget, budget], || {
        draw(&s);
        let bare = kept_shapes();
        draw(&s);
        draw(&s);
        (bare, (kept_shapes(), kept_bytes()))
    });
    let (kept, (counted, summed)) = painted;
    assert_eq!(counted, summed, "the bytes counted are what the shapes sum to");
    assert!(counted[0] <= budget, "within the budget: {counted:?} of {budget}");
    assert!(kept < 20 && bare <= 20, "some builds went to make room for their tops' lines: {kept} kept");
}

/// How dark the picture of section `id` (its line from x = -1000) is at model point `(x, z)`, drawn at `scale` px a mm centred
/// there: the darkest of the pixels within two of it up and down, as the sum of its channels.
fn darkest_at(s: &Session, id: Id, scale: f64, x: f64, z: f64) -> u32 {
    let view = crate::render::View { angle: 0.0, scale, cx: x + 1000.0, cy: z, width: 400, height: 400 };
    let img = crate::render::section_picture(&s.model, id, view, &[]).unwrap();
    (-2..=2)
        .map(|k| {
            let c = img.color(200.0, 200.0 + f64::from(k));
            u32::from(c.0) + u32::from(c.1) + u32::from(c.2)
        })
        .min()
        .unwrap_or(0)
}

/// A project with the `Tiles` pattern's material, family `name` of `forms`, a component of it at the origin and a section
/// looking north at it from y = -1000.
fn faced(name: &str, forms: Value) -> (Session, Id, Id) {
    let mut s = Session::new();
    run(&mut s, "fill_pattern", json!({"name": "Tiles", "target": "model", "lines": [{"angle": 0, "spacing": 100}]}));
    run(&mut s, "material", json!({"name": "Tile", "color": [200, 200, 200], "surface": "Tiles"}));
    run(&mut s, "family", json!({"name": name, "category": "generic", "types": {"T": {}}, "forms": forms}));
    let id = run(&mut s, "component", json!({"family": name, "type": "T", "at": [0, 0]}))["id"].as_u64().unwrap();
    let section = run(&mut s, "section", json!({"start": [-1000, -1000], "end": [3000, -1000]}))["id"].as_u64().unwrap();
    (s, id, section)
}

/// cp's review of #91: the face a picture shows is the triangle's own, not one found again by where its middle is. A part
/// revolve's cap, a disc bounded by a circle, painted: its courses show in an elevation at a zoom finer than the default.
#[test]
fn a_painted_cap_of_a_part_revolve_shows_its_pattern_in_an_elevation_close_up() {
    let ring = json!([{"centre": [400, 300], "radius": 100}, {"start": [0, 0], "end": [0, 100], "axis": true}]);
    let (mut s, id, section) = faced(
        "Ring",
        json!([{"guid": gstr(800), "sketch": ring, "host": {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2"}, "from": 0, "to": 90}]),
    );
    // The cap the elevation sees, in the family's xz plane at y = 0.
    let face = s.model.component_face(id, [400.0, 0.0, 330.0], [0.0, -1.0, 0.0]).expect("the cap at y = 0").face;
    assert!(darkest_at(&s, section, 4.0, 400.0, 300.0) > 700, "paper before");
    run(&mut s, "paint", json!({"id": id, "form": gstr(800), "face": face, "material": "Tile"}));
    assert!(darkest_at(&s, section, 4.0, 400.0, 300.0) < 600, "its course at 300, close up");
    assert!(darkest_at(&s, section, 4.0, 400.0, 250.0) > 700, "between courses");
}

/// A painted side with a round void through it: the side's triangles round the hole show its pattern, close up.
#[test]
fn a_painted_side_round_a_round_void_shows_its_pattern_in_an_elevation_close_up() {
    let void = json!({"guid": gstr(820), "sketch": [{"centre": [500, 150], "radius": 60}], "host": {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2"},
                      "from": -100, "to": 600, "void": true});
    let (mut s, id, section) = faced("Holed", json!([boxed(), void]));
    paint(&mut s, id, json!({"side": gstr(1)}), json!("Tile"));
    // Beside the hole, on the course at 200, and just over the hole's top on the course at 300 (the box is 300 high: under it).
    assert!(darkest_at(&s, section, 4.0, 580.0, 200.0) < 600, "a course beside the hole");
    assert!(darkest_at(&s, section, 4.0, 500.0, 230.0) > 700, "between courses over the hole");
    assert!(darkest_at(&s, section, 4.0, 200.0, 100.0) < 600, "a course away from it");
}

/// Two pieces whose fronts lie on each other, painted apart: the first's a material of a pattern, the second's one of none.
/// The picture's frontmost triangle (the first piece's, drawn first) says which face it is, and so which paint; found again
/// by where its middle is, it could be either.
#[test]
fn of_two_pieces_whose_fronts_coincide_the_frontmost_ones_paint_shows() {
    let a = json!({"guid": gstr(840), "sketch": rect(841, 0.0, 0.0, 1000.0, 500.0), "from": 0, "to": 300});
    let b = json!({"guid": gstr(850), "sketch": rect(851, 0.0, 0.0, 1000.0, 200.0), "from": 0, "to": 300});
    let (mut s, id, section) = faced("Pair", json!([a, b]));
    run(&mut s, "material", json!({"name": "Plain", "color": [90, 90, 200]}));
    run(&mut s, "paint", json!({"id": id, "form": gstr(840), "face": {"side": gstr(841)}, "material": "Tile"}));
    run(&mut s, "paint", json!({"id": id, "form": gstr(850), "face": {"side": gstr(851)}, "material": "Plain"}));
    assert!(darkest_at(&s, section, 0.5, 300.0, 100.0) < 600, "the first piece's front, painted Tile, shows its courses");
}
