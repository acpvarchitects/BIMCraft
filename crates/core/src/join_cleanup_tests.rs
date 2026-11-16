//! Join cleanup in a section (T23): where two elements Join Geometry joins meet in the section's cut, the line between them
//! is drawn only where the layers touching there differ (another material, unless both cut alike: the same fill pattern and
//! cut pen). Where they share a material the two read as one region, as every cut does where it meets another, joined or
//! not: one poché, the patterns in one lattice, no line. Unjoined elements, and a model with no joins, draw as they did.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::face_void_tests::{id_of, run};
use crate::geom::P2;
use crate::model::Id;
use crate::render::{self, Part, Prim};

/// Wall and floor types of one layer each (Concrete or Brick), and compound ones: a wall of plaster, a concrete core and
/// plaster (15, 170, 15), and a floor of 50 tiles over 150 concrete (screed would cut as concrete does: the same pattern).
fn types(s: &mut Session) {
    for (category, name, layers) in [
        ("wall", "Concrete 200", json!([{"material": "Concrete", "thickness": 200}])),
        ("wall", "Brick 200", json!([{"material": "Brick", "thickness": 200}])),
        ("floor", "Concrete 200", json!([{"material": "Concrete", "thickness": 200}])),
        (
            "wall",
            "Plastered concrete",
            json!([
                {"material": "Plaster", "thickness": 15, "function": "finish"},
                {"material": "Concrete", "thickness": 170},
                {"material": "Plaster", "thickness": 15, "function": "finish"}
            ]),
        ),
        (
            "floor",
            "Tiles on concrete",
            json!([{"material": "Ceramic tile", "thickness": 50, "function": "finish"}, {"material": "Concrete", "thickness": 150}]),
        ),
    ] {
        run(s, "type", json!({"category": category, "name": name, "layers": layers}));
    }
}

/// A wall along x at y 2000 (y 1900 to 2100), 3000 high, of `wall_type`, through the foot of a floor of `floor_type`, 6000
/// by 4000, its top 300 over the level (z 100 to 300): (session, wall, floor).
fn crossing(wall_type: &str, floor_type: &str) -> (Session, Id, Id) {
    let mut s = Session::new();
    types(&mut s);
    let w =
        run(&mut s, "wall", json!({"points": [[0, 2000], [6000, 2000]], "height": 3000, "type": wall_type}))["ids"][0].as_u64().unwrap();
    let f =
        id_of(&run(&mut s, "floor", json!({"boundary": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]], "offset": 300, "type": floor_type})));
    (s, w, f)
}

fn join(s: &mut Session, first: Id, second: Id) -> Value {
    run(s, "join_geometry", json!({"first": first, "second": second}))
}

/// The section across the wall at x 3000, from y -1000 (along is y + 1000: the wall 2900 to 3100), at `detail`: the lines
/// `owners` draw in it, as segments in mm (along, z), and the whole display list as text.
fn across(s: &mut Session, owners: &[Id], detail: &str) -> (Vec<[P2; 2]>, String) {
    let sec = id_of(&run(s, "section", json!({"start": [3000, -1000], "end": [3000, 5000]})));
    run(s, "detail", json!({"view": sec, "detail": detail}));
    let view = render::section_fit(&s.model, sec, 1200, 800);
    let d = render::section(&s.model, sec, view);
    let mut segments = Vec::new();
    for ((p, o), part) in d.prims.iter().zip(&d.owners).zip(&d.parts) {
        if *part == Part::Text || !o.is_some_and(|o| owners.contains(&o)) {
            continue;
        }
        if let Prim::Line { pts, closed, .. } = p {
            let mut pts: Vec<P2> = pts.iter().map(|q| view.mm(*q)).collect();
            if *closed {
                pts.extend(pts.first().copied());
            }
            segments.extend(pts.windows(2).map(|w| [w[0], w[1]]));
        }
    }
    let list = format!("{}{:?}{:?}", render::svg(&d), d.owners, d.parts);
    run(s, "delete", json!({"ids": [sec]}));
    (segments, list)
}

/// Whether a line runs along `a` to `b` through the point a `t`th of the way: a segment on that line (within half a
/// millimetre) that passes the point.
fn drawn(segments: &[[P2; 2]], a: P2, b: P2, t: f64) -> bool {
    let at = a + (b - a) * t;
    let dir = (b - a).unit();
    segments.iter().any(|[p, q]| {
        let off = |x: P2| (x - a).cross(dir).abs();
        let (sp, sq) = ((*p - a).dot(dir), (*q - a).dot(dir));
        let s = (at - a).dot(dir);
        off(*p) < 0.5 && off(*q) < 0.5 && sp.min(sq) - 0.5 <= s && s <= sp.max(sq) + 0.5 && (*q - *p).length() > 1e-6
    })
}

/// Whether a line runs along the wall's two faces through the floor (along 2900 and 3100, z 100 to 300), through their
/// middle at height `t` of the way up the floor.
fn up_the_faces(segments: &[[P2; 2]], t: f64) -> [bool; 2] {
    [2900.0, 3100.0].map(|x| drawn(segments, P2(x, 100.0), P2(x, 300.0), t))
}

/// Whether a line runs along the floor's underside and top across the wall (z 100 and 300, along 2900 to 3100), `t` of
/// the way across.
fn across_the_wall(segments: &[[P2; 2]], z: f64, t: f64) -> bool {
    drawn(segments, P2(2900.0, z), P2(3100.0, z), t)
}

#[test]
fn a_wall_and_a_floor_of_one_material_joined_read_as_one_region_either_way_round() {
    let (mut s, w, f) = crossing("Concrete 200", "Concrete 200");
    // Unjoined, every cut that meets another is one outline: the floor runs through the wall's foot, neither drawn there.
    let (before, _) = across(&mut s, &[w, f], "medium");
    assert_eq!(up_the_faces(&before, 0.5), [false, false]);
    join(&mut s, w, f);
    let (joined, _) = across(&mut s, &[w, f], "medium");
    assert_eq!(up_the_faces(&joined, 0.5), [false, false], "no line where the floor meets the wall: one concrete");
    assert!(drawn(&joined, P2(1000.0, 300.0), P2(2900.0, 300.0), 0.5), "the floor's top is still drawn away from the wall");
    run(&mut s, "switch_join_order", json!({"first": w, "second": f}));
    let (switched, _) = across(&mut s, &[w, f], "medium");
    assert!(!across_the_wall(&switched, 100.0, 0.5) && !across_the_wall(&switched, 300.0, 0.5), "nor where the wall meets the floor");
}

#[test]
fn a_wall_and_a_floor_of_different_materials_joined_keep_the_line_between_them_until_unjoined() {
    let (mut s, w, f) = crossing("Brick 200", "Concrete 200");
    let (before, list) = across(&mut s, &[w, f], "medium");
    assert_eq!(up_the_faces(&before, 0.5), [false, false], "unjoined, as it always was: one outline");
    // The wall cuts the floor: a line up each of its faces, as high as the floor is thick.
    join(&mut s, w, f);
    let (joined, _) = across(&mut s, &[w, f], "medium");
    for t in [0.1, 0.5, 0.9] {
        assert_eq!(up_the_faces(&joined, t), [true, true], "brick against concrete, {t} of the way up");
    }
    // Switched, the floor cuts the wall: the line along the floor's underside and its top, across the wall.
    run(&mut s, "switch_join_order", json!({"first": w, "second": f}));
    let (switched, _) = across(&mut s, &[w, f], "medium");
    assert!(across_the_wall(&switched, 100.0, 0.5) && across_the_wall(&switched, 300.0, 0.5));
    assert_eq!(up_the_faces(&switched, 0.5), [false, false]);
    // Unjoined: the section is drawn as it was before the join, to the last primitive.
    run(&mut s, "unjoin_geometry", json!({"first": w, "second": f}));
    let (after, unjoined) = across(&mut s, &[w, f], "medium");
    assert_eq!(up_the_faces(&after, 0.5), [false, false]);
    assert_eq!(unjoined, list);
}

#[test]
fn compound_layers_clean_up_only_where_they_share_a_material() {
    let (mut s, w, f) = crossing("Plastered concrete", "Tiles on concrete");
    // The floor cuts the wall, which stops under it and starts again over it. Under the floor (z 100) the wall's concrete
    // core meets the floor's concrete: one region; its plaster meets concrete: a line. Over it (z 300) all of the wall
    // meets tiles: a line right across.
    join(&mut s, f, w);
    let (cut, _) = across(&mut s, &[w, f], "medium");
    let under = |x0: f64, x1: f64| drawn(&cut, P2(x0, 100.0), P2(x1, 100.0), 0.5);
    assert!(under(2900.0, 2915.0) && under(3085.0, 3100.0), "plaster against concrete");
    assert!(!under(2915.0, 3085.0), "concrete against concrete: no line");
    for t in [0.05, 0.5, 0.95] {
        assert!(across_the_wall(&cut, 300.0, t), "the wall against tiles, {t} of the way across");
    }
    // The other way round, the wall cuts the floor: plaster faces both the tiles and the concrete, a line all the way up.
    run(&mut s, "switch_join_order", json!({"first": f, "second": w}));
    let (switched, _) = across(&mut s, &[w, f], "medium");
    for t in [0.1, 0.5, 0.9] {
        assert_eq!(up_the_faces(&switched, t), [true, true], "{t} of the way up");
    }
    // Coarse shows no layers, and no line between materials either: one poché.
    let (coarse, _) = across(&mut s, &[w, f], "coarse");
    assert_eq!(up_the_faces(&coarse, 0.5), [false, false]);
}

#[test]
fn materials_named_apart_that_cut_alike_clean_up() {
    // A copy of Concrete under another name: the same pattern, and walls and floors are cut in the same pen.
    let (mut s, w, f) = crossing("Concrete 200", "Concrete 200");
    let concrete = s.model.materials.get("Concrete").cloned().unwrap();
    let as_json = serde_json::to_value(&concrete).unwrap();
    run(&mut s, "material", json!({"name": "Site concrete", "color": as_json["color"], "cut": as_json["cut"]}));
    run(&mut s, "type", json!({"category": "floor", "name": "Site", "layers": [{"material": "Site concrete", "thickness": 200}]}));
    run(&mut s, "modify", json!({"id": f, "type": "Site"}));
    join(&mut s, w, f);
    let (joined, _) = across(&mut s, &[w, f], "medium");
    assert_eq!(up_the_faces(&joined, 0.5), [false, false]);
    // The floors' cut pen made another: the two no longer cut alike, and the line is drawn.
    run(&mut s, "object_styles", json!({"categories": {"floor": {"cut": 5}}}));
    let (pens, _) = across(&mut s, &[w, f], "medium");
    assert_eq!(up_the_faces(&pens, 0.5), [true, true]);
}

#[test]
fn a_join_the_section_does_not_cut_changes_nothing_in_it_and_a_model_with_no_joins_does_no_join_work() {
    let (mut s, w, f) = crossing("Brick 200", "Concrete 200");
    // A second pair far along the wall's line, which the section doesn't cut.
    let w2 = run(&mut s, "wall", json!({"points": [[20000, 2000], [26000, 2000]], "height": 3000, "type": "Brick 200"}))["ids"][0]
        .as_u64()
        .unwrap();
    let f2 = id_of(&run(
        &mut s,
        "floor",
        json!({"boundary": [[20000, 0], [26000, 0], [26000, 4000], [20000, 4000]], "offset": 300, "type": "Concrete 200"}),
    ));
    render::joint_work_reset();
    let (_, before) = across(&mut s, &[w, f], "fine");
    assert_eq!(render::joint_work(), 0, "no join, no join work");
    join(&mut s, w2, f2);
    let (_, after) = across(&mut s, &[w, f], "fine");
    assert_eq!(after, before);
    assert_eq!(render::joint_work(), 0, "a joined pair the section doesn't cut costs it nothing");
    join(&mut s, w, f);
    across(&mut s, &[w, f], "fine");
    assert!(render::joint_work() > 0);
    // The sample house, which has no joins, draws its sections without any.
    let mut sample = Session::new();
    sample.script(crate::sample::SAMPLE).unwrap();
    let sections: Vec<Id> = sample.model.elements.keys().filter(|id| sample.model.section(**id).is_some()).copied().collect();
    assert!(!sections.is_empty());
    render::joint_work_reset();
    for sec in sections {
        let view = render::section_fit(&sample.model, sec, 800, 600);
        render::section(&sample.model, sec, view);
    }
    assert_eq!(render::joint_work(), 0);
}
