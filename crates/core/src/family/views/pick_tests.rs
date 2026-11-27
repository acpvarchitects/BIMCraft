//! GEO-2c-2e: a family's plan and elevations pick what they drew. Where a view draws a curve finer than its default chords
//! (GEO-2c), a click on it, on its edge and a box take the form or the nested instance drawn there, not the default build's;
//! where the view keeps the default build (too fine to cut), they take that; through a hole, the form seen below; in an
//! elevation, the one nearest the eye; and what the plan leaves out (a window's hood above the cut) is no one's.

use super::*;
use crate::engine::Session;
use crate::ids::Id;
use serde_json::json;

/// `f` placed and staged as the Family Editor stages it, of type `ty`: the staged family, its values and the component's id.
fn staged(f: &Family, ty: &str) -> (Staged, BTreeMap<String, Val>, Id) {
    let mut s = Session::new();
    s.model_mut().families.insert("Specimen".into(), f.clone());
    let placed = s.execute("component", &json!({"family": "Specimen", "type": ty, "at": [0, 0]})).unwrap();
    let id = placed["id"].as_u64().unwrap();
    let values = f.values(ty, &BTreeMap::new()).unwrap();
    (stage(std::sync::Arc::unwrap_or_clone(s.model.clone()), f, &values, &|_| true), values, id)
}

/// View `which` of `st` drawn `scale` px a mm round `p`, as the editor draws it once zoomed there.
fn zoom_at(st: &mut Staged, which: Which, p: P2, scale: f64, (width, height): (u32, u32)) -> Lines {
    let v = render::View { cx: p.0, cy: p.1, scale, width, height, angle: 0.0 };
    st.draw_lines_at(which, v, 1.0, P2(0.0, 0.0)).unwrap()
}

/// The closed rings drawn in the cut pen, in mm.
fn cut_rings(drawn: &Lines) -> Vec<Vec<P2>> {
    drawn
        .drawing
        .prims
        .iter()
        .zip(&drawn.drawing.parts)
        .filter_map(|(p, part)| {
            let render::Prim::Line { pts, closed: true, .. } = p else { return None };
            (*part == render::Part::Cut).then(|| pts.iter().map(|q| drawn.view.mm(*q)).collect())
        })
        .collect()
}

/// A circle 1 m across, through the plan's cut: far off, its cut is the default's 32 chords; close in, a finer ring, which
/// takes in a point between a default chord and the circle. A click there and on the circle's edge take the form (or the
/// nested instance it is the form of) just where the plan draws it, at each zoom and back.
#[test]
fn a_plan_picks_the_drawn_curve_of_a_form_or_a_nested_instance_at_each_zoom() {
    let circle = json!({"category": "generic", "types": {"T": {}}, "forms": [
        {"sketch": [{"centre": [0, 0], "radius": 1000}], "from": 0, "to": 2000}
    ]});
    // A default chord's middle is 4.8 mm inside the circle: 999 mm out is outside the chords and inside the circle.
    let (sin, cos) = (std::f64::consts::PI / 32.0).sin_cos();
    let (q, edge) = (P2(cos * 999.0, sin * 999.0), P2(cos * 1000.0, sin * 1000.0));
    for nested in [false, true] {
        let f: Family = serde_json::from_value(if nested {
            json!({"category": "generic", "types": {"T": {}}, "nested": [{"name": "Round", "family": circle}],
                "nested_instances": [{"family": "Round", "type": "T", "at": [0, 0]}]})
        } else {
            circle.clone()
        })
        .unwrap();
        let (mut st, values, _) = staged(&f, "T");
        for (scale, inside) in [(0.01, false), (4.0, true), (0.01, false)] {
            let drawn = zoom_at(&mut st, Which::Plan, q, scale, (400, 320));
            assert_eq!(cut_rings(&drawn).iter().any(|r| geom::contains(r, q)), inside, "the cut drawn at {scale} px a mm");
            let hit = if nested { nested_at(&f, &values, &st, Which::Plan, q) } else { form_at(&f, &values, &st, Which::Plan, q) };
            assert_eq!(hit, inside.then_some(0), "nested {nested}: a click inside what is drawn at {scale}");
            let hit = if nested {
                nested_edge_at(&f, &values, &st, Which::Plan, edge, 0.5)
            } else {
                form_edge_at(&f, &values, &st, Which::Plan, edge, 0.5)
            };
            assert_eq!(hit, inside.then_some(0), "nested {nested}: a click on the edge drawn at {scale}");
        }
    }
}

/// A block drilled through by a void, and a small one on the floor under the hole: the plan sees the small one through the
/// hole. A click there, an edge click on the hole's rim and a crossing box agree on what is seen; the void keeps the corners
/// it was drawn with, to edit.
#[test]
fn a_plan_picks_the_form_seen_through_a_hole_by_a_click_an_edge_and_a_box() {
    let f: Family = serde_json::from_value(json!({"category": "generic", "types": {"T": {}}, "forms": [
        {"profile": [[0, 0], [1000, 0], [1000, 600], [0, 600]], "from": 800, "to": 1600},
        {"profile": [[200, 100], [800, 100], [800, 500], [200, 500]], "from": 700, "to": 1700, "void": true},
        {"profile": [[400, 200], [600, 200], [600, 400], [400, 400]], "from": 0, "to": 100}
    ]}))
    .unwrap();
    let (mut st, values, _) = staged(&f, "T");
    zoom_at(&mut st, Which::Plan, P2(500.0, 300.0), 0.5, (800, 500));
    for (p, part) in [(P2(500.0, 300.0), Some(2)), (P2(300.0, 300.0), None), (P2(100.0, 300.0), Some(0))] {
        assert_eq!(form_at(&f, &values, &st, Which::Plan, p), part, "a click at {p:?}");
    }
    assert_eq!(form_edge_at(&f, &values, &st, Which::Plan, P2(200.0, 300.0), 1.0), Some(0), "the hole's rim is the block's");
    let centre = geom::SelectBox::dragged(P2(515.0, 315.0), P2(485.0, 285.0));
    assert!(!face_in_box(&f, &values, &st, Which::Plan, Part::Form(0), &centre));
    assert!(face_in_box(&f, &values, &st, Which::Plan, Part::Form(2), &centre));
    assert!(!face_in_box(&f, &values, &st, Which::Plan, Part::Form(1), &centre));
    assert!(!form_corners(&f, &values, 1).is_empty(), "the void keeps the corners it was drawn with");
}

/// A disc 40 m across with two voids is too fine to cut at 8 px a mm (GEO-2c): the plan draws its default build there, and
/// a click takes the form just inside its default chords, never between a chord and the circle.
#[test]
fn a_plan_too_fine_to_cut_picks_the_default_chords_it_draws() {
    let mut forms = vec![json!({"sketch": [{"centre": [0, 0], "radius": 20000}], "from": 0, "to": 3000})];
    for x in [-10000, 10000] {
        forms.push(json!({"sketch": [{"centre": [x, 0], "radius": 6000}], "from": 1000, "to": 4000, "void": true}));
    }
    let f: Family = serde_json::from_value(json!({"category": "generic", "types": {"T": {}}, "forms": forms})).unwrap();
    let (mut st, values, _) = staged(&f, "T");
    assert!(crate::family::Fineness::for_view(8.0).scoped(|| f.pieces(&values)).is_err(), "too fine to cut at 8 px a mm");
    // A default chord's middle, 96 mm inside the circle.
    let (sin, cos) = (std::f64::consts::PI / 32.0).sin_cos();
    let chord = 20000.0 * cos;
    let at = |r: f64| P2(r * cos, r * sin);
    let drawn = zoom_at(&mut st, Which::Plan, at(chord), 8.0, (320, 200));
    let rings = cut_rings(&drawn);
    assert!(rings.iter().any(|r| geom::contains(r, at(chord - 2.0))) && !rings.iter().any(|r| geom::contains(r, at(chord + 2.0))));
    assert_eq!(form_at(&f, &values, &st, Which::Plan, at(chord - 2.0)), Some(0));
    assert_eq!(form_at(&f, &values, &st, Which::Plan, at(chord + 2.0)), None);
    assert_eq!(form_edge_at(&f, &values, &st, Which::Plan, at(chord), 0.5), Some(0));
    assert_eq!(form_edge_at(&f, &values, &st, Which::Plan, at(19999.0), 0.5), None);
}

/// A window's plan (F10, F10b): a click on a jamb the cut passes through takes the frame; on the sill board below the cut,
/// outside the wall, the board; and where the hood stands over the cut, which the plan doesn't draw, nothing.
#[test]
fn a_window_familys_plan_picks_its_cut_jamb_and_its_sill_and_not_its_hood_above_the_cut() {
    let f: Family = serde_json::from_value(json!({
        "category": "window",
        "parameters": [{"name": "Width", "value": 1200}, {"name": "Height", "value": 1500}],
        "types": {"T": {}},
        "forms": [
            {"host": {"plane": crate::family::CENTER_FRONT_BACK}, "from": -40, "to": 40, "sketch": [
                {"start": [-600, 0], "end": [600, 0]}, {"start": [600, 0], "end": [600, 1500]},
                {"start": [600, 1500], "end": [-600, 1500]}, {"start": [-600, 1500], "end": [-600, 0]},
                {"start": [-550, 50], "end": [-550, 1450]}, {"start": [-550, 1450], "end": [550, 1450]},
                {"start": [550, 1450], "end": [550, 50]}, {"start": [550, 50], "end": [-550, 50]}]},
            {"profile": [[-650, -150], [650, -150], [650, -40], [-650, -40]], "from": 0, "to": 30},
            {"profile": [[-650, 40], [650, 40], [650, 160], [-650, 160]], "from": 1400, "to": 1450}
        ],
    }))
    .unwrap();
    let mut s = Session::new();
    s.model_mut().families.insert("W".into(), f.clone());
    s.execute("wall", &json!({"points": [[-2500, 0], [2500, 0]]})).unwrap();
    s.execute("window", &json!({"at": [0, 0], "family": "W", "type": "T", "sill": 0})).unwrap();
    let values = f.values("T", &BTreeMap::new()).unwrap();
    let mut st = stage(std::sync::Arc::unwrap_or_clone(s.model.clone()), &f, &values, &|_| true);
    st.draw(Which::Plan, 600, 400, 1.0, P2(0.0, 0.0)).unwrap();
    assert_eq!(form_at(&f, &values, &st, Which::Plan, P2(575.0, 0.0)), Some(0), "the cut jamb");
    assert_eq!(form_at(&f, &values, &st, Which::Plan, P2(0.0, -130.0)), Some(1), "the sill board");
    assert_eq!(form_at(&f, &values, &st, Which::Plan, P2(0.0, 140.0)), None, "the hood over the cut isn't drawn");
}

/// A disc 40 m across standing up, as the family's own form and as a nested instance's 50 m off: at 1 px a mm an elevation
/// draws it finer than its default chords, out to a point the default build falls 96 mm short of. A click there, on its
/// edge, and past it take what is drawn, from the front and from the back.
#[test]
fn an_elevation_picks_the_drawn_chord_and_face_of_a_form_and_a_nested_instance() {
    let disc = |guid: &str| json!({"guid": guid, "sketch": [{"centre": [0, 0], "radius": 20000}], "host": {"plane": crate::family::CENTER_FRONT_BACK}, "from": -80, "to": 80});
    let f: Family = serde_json::from_value(json!({
        "category": "generic", "types": {"T": {}}, "forms": [disc("00000000-0000-4000-8000-000000000111")],
        "nested": [{"name": "Wheel", "family": {"category": "generic", "types": {"T": {}},
            "forms": [disc("00000000-0000-4000-8000-000000000222")]}}],
        "nested_instances": [{"guid": "00000000-0000-4000-8000-000000000333", "family": "Wheel", "type": "T", "at": [50000, 0]}]
    }))
    .unwrap();
    let (mut st, values, owner) = staged(&f, "T");
    let angle = std::f64::consts::PI / 32.0;
    for which in [Which::Elevation(Side::Front), Which::Elevation(Side::Back)] {
        for (nested, x) in [(false, 0.0), (true, 50000.0)] {
            let point = |r: f64| [x + r * angle.cos(), 0.0, r * angle.sin()];
            let (inside, edge, outside) =
                (st.to_view(which, point(19970.0)), st.to_view(which, point(20000.0)), st.to_view(which, point(20020.0)));
            let shown = zoom_at(&mut st, which, edge, 1.0, (220, 180));
            let picture = shown.picture.as_ref().unwrap();
            let pixel = |p: P2| {
                let (x, y) = shown.view.px(p);
                picture.pick(f64::from(x), f64::from(y))
            };
            assert_eq!(pixel(inside), Some(owner), "{which:?} nested {nested}: drawn there");
            assert_eq!(pixel(outside), None, "{which:?} nested {nested}: nothing drawn past it");
            if nested {
                assert_eq!(nested_at(&f, &values, &st, which, inside), Some(0));
                assert_eq!(nested_edge_at(&f, &values, &st, which, edge, 1.0), Some(0));
                assert_eq!(nested_at(&f, &values, &st, which, outside), None);
                assert_eq!(form_at(&f, &values, &st, which, inside), None);
            } else {
                assert_eq!(form_at(&f, &values, &st, which, inside), Some(0));
                assert_eq!(form_edge_at(&f, &values, &st, which, edge, 1.0), Some(0));
                assert_eq!(form_at(&f, &values, &st, which, outside), None);
                assert_eq!(nested_at(&f, &values, &st, which, inside), None);
            }
        }
    }
}

/// A plate standing up with a square void through it: its front elevation draws the hole clear. A click in the hole takes
/// nothing, on the plate the plate; an edge click on the hole's rim the plate, and in the middle of the hole nothing.
#[test]
fn an_elevation_picks_the_rim_of_a_hole_and_leaves_the_hole_clear() {
    let square = |h: f64| json!([[-h, -h], [h, -h], [h, h], [-h, h]]);
    let f: Family = serde_json::from_value(json!({"category": "generic", "types": {"T": {}}, "forms": [
        {"profile": square(500.0), "host": {"plane": crate::family::CENTER_FRONT_BACK}, "from": -100, "to": 100},
        {"profile": square(120.0), "host": {"plane": crate::family::CENTER_FRONT_BACK}, "from": -200, "to": 200, "void": true}
    ]}))
    .unwrap();
    let (mut st, values, owner) = staged(&f, "T");
    let which = Which::Elevation(Side::Front);
    let (centre, plate, rim) =
        (st.to_view(which, [0.0, 0.0, 0.0]), st.to_view(which, [260.0, 0.0, 0.0]), st.to_view(which, [120.0, 0.0, 0.0]));
    let shown = zoom_at(&mut st, which, centre, 0.3, (220, 180));
    let picture = shown.picture.as_ref().unwrap();
    for (at, drawn, hit) in [(centre, None, None), (plate, Some(owner), Some(0))] {
        let (x, y) = shown.view.px(at);
        assert_eq!(picture.pick(f64::from(x), f64::from(y)), drawn, "drawn at {at:?}");
        assert_eq!(form_at(&f, &values, &st, which, at), hit, "a click at {at:?}");
    }
    assert_eq!(form_edge_at(&f, &values, &st, which, rim, 1.0), Some(0), "the hole's rim");
    assert_eq!(form_edge_at(&f, &values, &st, which, centre, 1.0), None, "the middle of the hole");
}

/// A disc of the family's and a smaller nested one behind it (from the front; before it from the back), overlapping: each
/// elevation's click, and an edge click on the small one's rim, take the one its picture shows nearest the eye there.
#[test]
fn front_and_back_pick_the_nearer_of_a_form_and_a_nested_disc_overlapping() {
    let disc = |guid: &str, radius: f64| json!({"guid": guid, "sketch": [{"centre": [0, 0], "radius": radius}], "host": {"plane": crate::family::CENTER_FRONT_BACK}, "from": -50, "to": 50});
    let f: Family = serde_json::from_value(json!({
        "category": "generic", "types": {"T": {}}, "forms": [disc("00000000-0000-4000-8000-000000000111", 900.0)],
        "nested": [{"name": "Wheel", "family": {"category": "generic", "types": {"T": {}},
            "forms": [disc("00000000-0000-4000-8000-000000000222", 360.0)]}}],
        "nested_instances": [{"guid": "00000000-0000-4000-8000-000000000333", "family": "Wheel", "type": "T", "at": [0, 500]}]
    }))
    .unwrap();
    let (mut st, values, owner) = staged(&f, "T");
    for (side, nested) in [(Side::Front, false), (Side::Back, true)] {
        let which = Which::Elevation(side);
        let q = st.to_view(which, [0.0, 0.0, 80.0]);
        let shown = zoom_at(&mut st, which, q, 0.25, (220, 180));
        let (x, y) = shown.view.px(q);
        let picture = shown.picture.as_ref().unwrap();
        assert_eq!(picture.pick(f64::from(x), f64::from(y)), Some(owner));
        let (camera, _) = render::section_camera(&st.model, st.elevation(side).unwrap(), shown.view).unwrap();
        let surface = picture.point(&camera, f64::from(x), f64::from(y)).unwrap();
        assert_eq!(surface[1] > 250.0, nested, "the disc nearest the eye from the {side:?}");
        assert_eq!(form_at(&f, &values, &st, which, q), (!nested).then_some(0));
        assert_eq!(nested_at(&f, &values, &st, which, q), nested.then_some(0));
        let rim = st.to_view(which, [360.0, 500.0, 0.0]);
        let (x, y) = shown.view.px(st.to_view(which, [350.0, 500.0, 0.0]));
        let inside = picture.point(&camera, f64::from(x), f64::from(y)).unwrap();
        assert_eq!(inside[1] > 250.0, nested, "the disc drawn just inside the small one's rim from the {side:?}");
        assert_eq!(nested_edge_at(&f, &values, &st, which, rim, 1.0), nested.then_some(0));
    }
}

/// A nested instance of two panels, a small one 400 mm behind a wide one (from the front): an elevation selects the instance
/// whole, so its wide panel hides the small one's rim from the front, as the picture does, and from the back the rim shows.
/// An edge click on that rim and a crossing box over it take the instance only where the rim is seen (ec's follow-up to the
/// port, e23c403b0).
#[test]
fn a_nested_instances_elevation_leaves_its_hidden_rear_rim_out_of_a_click_and_a_crossing_box() {
    let f: Family = serde_json::from_value(json!({
        "category": "generic", "types": {"T": {}}, "forms": [],
        "nested": [{"name": "Panels", "family": {"category": "generic", "types": {"T": {}}, "forms": [
            {"guid": "00000000-0000-4000-8000-000000000111", "profile": [[-500, -100], [500, -100], [500, 0], [-500, 0]], "from": -500, "to": 500},
            {"guid": "00000000-0000-4000-8000-000000000222", "profile": [[-150, 400], [150, 400], [150, 500], [-150, 500]], "from": -150, "to": 150}
        ]}}],
        "nested_instances": [{"guid": "00000000-0000-4000-8000-000000000333", "family": "Panels", "type": "T", "at": [0, 0]}]
    }))
    .unwrap();
    let (mut st, values, owner) = staged(&f, "T");
    for (side, visible) in [(Side::Front, false), (Side::Back, true)] {
        let which = Which::Elevation(side);
        let (rim, inside) = (st.to_view(which, [150.0, 500.0, 25.0]), st.to_view(which, [145.0, 500.0, 25.0]));
        let shown = zoom_at(&mut st, which, rim, 1.0, (220, 180));
        let picture = shown.picture.as_ref().unwrap();
        let (x, y) = shown.view.px(inside);
        assert_eq!(picture.pick(f64::from(x), f64::from(y)), Some(owner));
        let (camera, _) = render::section_camera(&st.model, st.elevation(side).unwrap(), shown.view).unwrap();
        assert_eq!(
            picture.point(&camera, f64::from(x), f64::from(y)).unwrap()[1] > 250.0,
            visible,
            "the small panel seen from the {side:?}"
        );
        assert_eq!(nested_edge_at(&f, &values, &st, which, rim, 1.0), visible.then_some(0), "an edge click on its rim from the {side:?}");
        let crossing = geom::SelectBox::dragged(rim + P2(2.0, 2.0), rim - P2(2.0, 2.0));
        assert_eq!(
            crossing.takes(&nested_outline_edges(&f, &values, &st, which, 0)),
            visible,
            "a crossing box over its rim from the {side:?}"
        );
    }
}
