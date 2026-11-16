//! Face coordinates (`face.rs`): `(u, v)` to a point and back on every kind of face, arc length along a curved wall, the
//! outline and openings of a face in its own coordinates, and an outline measured on the true surface.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::face::{Chart, FaceId};
use crate::geom::P2;
use crate::model::{Guid, Id};
use crate::view3d::P3;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn first(v: &Value) -> Id {
    v["ids"].get(0).or(Some(&v["id"])).and_then(Value::as_u64).unwrap()
}

fn chart(s: &Session, id: Id, face: FaceId) -> Chart {
    let g: Guid = s.model.guid(id);
    s.model.face_chart(g, face).unwrap_or_else(|| panic!("{id} has no {face:?}"))
}

fn near3(a: P3, b: P3, tol: f64) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

/// `(u, v)` to a point and back is where it started, and the point lies on the face's normal side of the element.
fn round_trips(c: &Chart, uvs: &[P2]) {
    for uv in uvs {
        let p = c.point(*uv);
        let back = c.uv(p);
        assert!(near(back.0, uv.0, 1e-6) && near(back.1, uv.1, 1e-6), "{:?}: {uv:?} to {p:?} and back to {back:?}", c.face);
        let n = c.normal(*uv);
        assert!(near((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt(), 1.0, 1e-9), "{n:?}");
    }
}

fn square(u0: f64, v0: f64, side: f64) -> Vec<P2> {
    vec![P2(u0, v0), P2(u0 + side, v0), P2(u0 + side, v0 + side), P2(u0, v0 + side)]
}

#[test]
fn a_straight_walls_faces_map_both_ways() {
    let mut s = Session::new();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [6000, 0]], "height": 3000})));
    let ext = chart(&s, w, FaceId::Exterior);
    round_trips(&ext, &[P2(0.0, 0.0), P2(2500.0, 1200.0), P2(6000.0, 3000.0)]);
    // The exterior is to the left of start to end, 100 out of a wall 200 thick; `u` along the wall, `v` up it.
    assert!(near3(ext.point(P2(2000.0, 1200.0)), [2000.0, 100.0, 1200.0], 1e-9), "{:?}", ext.point(P2(2000.0, 1200.0)));
    assert!(near3(ext.normal(P2(0.0, 0.0)), [0.0, 1.0, 0.0], 1e-12));
    let int = chart(&s, w, FaceId::Interior);
    assert!(near3(int.point(P2(2000.0, 1200.0)), [2000.0, -100.0, 1200.0], 1e-9));
    assert!(near3(int.normal(P2(0.0, 0.0)), [0.0, -1.0, 0.0], 1e-12));
    // The side is 6 m by 3 m.
    assert!(near(ext.area(), 18e6, 1e-3), "{}", ext.area());
    // The ends: across the wall from its exterior, up from its base.
    let start = chart(&s, w, FaceId::Start);
    round_trips(&start, &[P2(50.0, 10.0), P2(200.0, 3000.0)]);
    assert!(near3(start.point(P2(0.0, 0.0)), [0.0, 100.0, 0.0], 1e-9) && near3(start.point(P2(200.0, 0.0)), [0.0, -100.0, 0.0], 1e-9));
    assert!(near3(start.normal(P2(0.0, 0.0)), [-1.0, 0.0, 0.0], 1e-12));
    assert!(near(chart(&s, w, FaceId::End).area(), 200.0 * 3000.0, 1e-3));
    let top = chart(&s, w, FaceId::Top);
    round_trips(&top, &[P2(100.0, 50.0)]);
    assert!(near3(top.point(P2(100.0, 50.0)), [100.0, 50.0, 3000.0], 1e-9));
    assert!(near(top.area(), 6000.0 * 200.0, 1e-3));
    // A wall has no numbered sides; a floor has no exterior.
    assert!(s.model.face_chart(s.model.guid(w), FaceId::Side(0)).is_none());
    assert_eq!(s.model.faces_of(s.model.guid(w)).len(), 6);
}

#[test]
fn a_curved_walls_side_is_laid_out_by_its_own_arc_length() {
    let mut s = Session::new();
    // Half a circle of radius 2000 about (2000, 0), turning left: its exterior, to the left, is the inside of the curve.
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "arc": 180, "height": 3000})));
    let ext = chart(&s, w, FaceId::Exterior);
    let int = chart(&s, w, FaceId::Interior);
    let pi = std::f64::consts::PI;
    // Radii 1900 and 2100: each side's length is its own arc.
    let (re, ri) = (1900.0, 2100.0);
    let ext_len = ext.outline[0].iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
    let int_len = int.outline[0].iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
    assert!(near(ext_len, pi * re, 1e-6) || near(ext_len, pi * ri, 1e-6), "{ext_len}");
    assert!(near(ext_len + int_len, pi * (re + ri), 1e-6), "{ext_len} + {int_len}");
    // Developed: each side's area is its arc length by its height.
    assert!(near(ext.area(), ext_len * 3000.0, 1.0), "{} against {}", ext.area(), ext_len * 3000.0);
    for c in [&ext, &int] {
        let len = c.outline[0].iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
        round_trips(c, &[P2(0.0, 0.0), P2(len / 3.0, 700.0), P2(len, 3000.0)]);
        // Every point of the side is its radius from the centre, and faces along the radius.
        for u in [0.0, len / 4.0, len / 2.0, len] {
            let p = c.point(P2(u, 500.0));
            let r = (p[0] - 2000.0).hypot(p[1]);
            assert!(near(r, re, 1e-6) || near(r, ri, 1e-6), "{r}");
            let n = c.normal(P2(u, 500.0));
            let radial = [(p[0] - 2000.0) / r, p[1] / r];
            let along = n[0] * radial[0] + n[1] * radial[1];
            assert!(near(along.abs(), 1.0, 1e-9), "{n:?} at {p:?}");
            // The inside of the curve faces the centre, the outside away from it.
            assert_eq!(along > 0.0, near(r, ri, 1e-6), "{:?} at radius {r}", c.face);
        }
        // `u` starts square to the location line's start, and a square drawn on the side measures its own area.
        let p0 = c.point(P2(0.0, 0.0));
        assert!(near(p0[1], 0.0, 1e-6), "{p0:?}");
        let m = c.measure(&square(len / 2.0 - 500.0, 1000.0, 1000.0), &[]);
        assert!(near(m.area, 1e6, 1e-3) && m.outside == 0.0, "{m:?}");
    }
}

#[test]
fn a_floors_top_and_bottom_are_in_its_own_plan_and_measure_on_the_slope() {
    let mut s = Session::new();
    let f = first(&run(&mut s, "floor", json!({"boundary": [[1000, 1000], [7000, 1000], [7000, 5000], [1000, 5000]], "offset": 500})));
    let top = chart(&s, f, FaceId::Top);
    round_trips(&top, &[P2(0.0, 0.0), P2(1500.0, 2000.0)]);
    // From its first point, along its first edge.
    assert!(near3(top.point(P2(1500.0, 2000.0)), [2500.0, 3000.0, 500.0], 1e-9), "{:?}", top.point(P2(1500.0, 2000.0)));
    assert!(near3(top.normal(P2(0.0, 0.0)), [0.0, 0.0, 1.0], 1e-12));
    let bottom = chart(&s, f, FaceId::Bottom);
    assert!(near3(bottom.point(P2(1500.0, 2000.0)), [2500.0, 3000.0, 300.0], 1e-9));
    assert!(near3(bottom.normal(P2(0.0, 0.0)), [0.0, 0.0, -1.0], 1e-12));
    assert!(near(top.area(), 24e6, 1e-3));
    // Its sides: along each edge, up from its bottom, outward.
    let south = chart(&s, f, FaceId::Side(0));
    round_trips(&south, &[P2(100.0, 50.0)]);
    assert!(near3(south.normal(P2(0.0, 0.0)), [0.0, -1.0, 0.0], 1e-12), "{:?}", south.normal(P2(0.0, 0.0)));
    assert!(near(south.area(), 6000.0 * 200.0, 1e-3));
    assert!(s.model.face_chart(s.model.guid(f), FaceId::Side(4)).is_none());
    // Sloped 30°: the top's area is its plan's over the cosine.
    run(&mut s, "modify", json!({"id": f, "slope": 30, "arrow": [[1000, 3000], [7000, 3000]]}));
    let top = chart(&s, f, FaceId::Top);
    round_trips(&top, &[P2(0.0, 0.0), P2(1500.0, 2000.0), P2(6000.0, 4000.0)]);
    let cos = 30f64.to_radians().cos();
    assert!(near(top.area(), 24e6 / cos, 1.0), "{}", top.area());
    let m = top.measure(&square(1000.0, 1000.0, 1000.0), &[]);
    assert!(near(m.area, 1e6 / cos, 1e-3), "{m:?}");
    let n = top.normal(P2(1000.0, 1000.0));
    assert!(near(n[2], cos, 1e-9), "{n:?}");
}

#[test]
fn a_roofs_slopes_are_one_top_measured_slope_by_slope() {
    let mut s = Session::new();
    let r = first(&run(&mut s, "roof", json!({"boundary": [[0, 0], [8000, 0], [8000, 6000], [0, 6000]], "slope": 30, "overhang": 0})));
    let top = chart(&s, r, FaceId::Top);
    round_trips(&top, &[P2(1000.0, 1000.0), P2(4000.0, 3000.0), P2(7000.0, 5500.0)]);
    let cos = 30f64.to_radians().cos();
    assert!(near(top.area(), 48e6 / cos, 10.0), "{}", top.area());
    // Each point of it is on the roof's top, which a component on its top would stand on.
    let g = s.model.guid(r);
    for uv in [P2(1000.0, 1000.0), P2(4000.0, 3000.0)] {
        let p = top.point(uv);
        let at = s.model.face_at(g, crate::model::FaceRole::Top, [p[0], p[1]]).unwrap();
        assert!(near3(p, at.point, 1e-6), "{p:?} against {:?}", at.point);
    }
}

#[test]
fn a_columns_faces_square_and_round() {
    let mut s = Session::new();
    run(&mut s, "type", json!({"category": "column", "name": "Pier 400", "width": 400, "depth": 600}));
    let c = first(&run(&mut s, "column", json!({"at": [1000, 1000], "type": "Pier 400", "height": 3000})));
    let side = chart(&s, c, FaceId::Side(0));
    round_trips(&side, &[P2(100.0, 100.0)]);
    assert!(near(side.area(), 400.0 * 3000.0, 1e-3), "{}", side.area());
    assert!(near(chart(&s, c, FaceId::Side(1)).area(), 600.0 * 3000.0, 1e-3));
    assert!(near(chart(&s, c, FaceId::Top).area(), 400.0 * 600.0, 1e-3));
    let round = first(&run(&mut s, "column", json!({"at": [5000, 3000], "type": "Round 300", "height": 2500})));
    let side = chart(&s, round, FaceId::Side(0));
    let tau = std::f64::consts::TAU;
    round_trips(&side, &[P2(0.0, 0.0), P2(400.0, 1000.0), P2(tau * 150.0 - 1.0, 2500.0)]);
    assert!(near(side.area(), tau * 150.0 * 2500.0, 1e-3), "{}", side.area());
    assert!(s.model.face_chart(s.model.guid(round), FaceId::Side(1)).is_none());
}

#[test]
fn a_faces_openings_are_holes_in_its_own_coordinates_and_an_outline_reads_what_they_take() {
    let mut s = Session::new();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [6000, 0]], "height": 3000})));
    run(&mut s, "door", json!({"at": [2000, 0]}));
    let ext = chart(&s, w, FaceId::Exterior);
    assert_eq!(ext.openings.len(), 1);
    let door = &ext.openings[0];
    let (u0, u1) = door.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| (a.min(q.0), b.max(q.0)));
    let (v0, v1) = door.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| (a.min(q.1), b.max(q.1)));
    assert!(u0 < 2000.0 && u1 > 2000.0 && near(v0, 0.0, 1e-9) && v1 > 1500.0, "{door:?}");
    // An outline over all of the door and a metre beyond either jamb: the door takes its own area out.
    let outline = vec![P2(u0 - 1000.0, 0.0), P2(u1 + 1000.0, 0.0), P2(u1 + 1000.0, 2500.0), P2(u0 - 1000.0, 2500.0)];
    let m = ext.measure(&outline, &[]);
    let door_area = (u1 - u0) * (v1 - v0);
    assert!(near(m.area, (u1 - u0 + 2000.0) * 2500.0, 1e-3), "{m:?}");
    assert!(near(m.cut, door_area, 1e-3), "{m:?} against {door_area}");
    assert!(near(m.net(), m.area - door_area, 1e-3));
    // A hole in the outline is left out of both.
    let hole = square(u1 + 100.0, 100.0, 500.0);
    let m2 = ext.measure(&outline, std::slice::from_ref(&hole));
    assert!(near(m2.area, m.area - 250_000.0, 1e-3) && near(m2.cut, m.cut, 1e-3), "{m2:?}");
    // Past the wall's end and over its top: what lies off the face is said, in (u, v).
    let off = ext.measure(&square(5500.0, 2500.0, 1000.0), &[]);
    assert!(near(off.area, 250_000.0, 1e-3) && near(off.outside, 750_000.0, 1e-3), "{off:?}");
}

#[test]
fn a_walls_face_follows_it_moved_lengthened_thickened_and_retyped() {
    let mut s = Session::new();
    let w = first(&run(&mut s, "wall", json!({"points": [[0, 0], [6000, 0]], "height": 3000})));
    let uv = P2(2000.0, 1200.0);
    run(&mut s, "move", json!({"ids": [w], "by": [0, 1000]}));
    assert!(near3(chart(&s, w, FaceId::Exterior).point(uv), [2000.0, 1100.0, 1200.0], 1e-9));
    run(&mut s, "modify", json!({"id": w, "end": [9000, 1000]}));
    let ext = chart(&s, w, FaceId::Exterior);
    assert!(near3(ext.point(uv), [2000.0, 1100.0, 1200.0], 1e-9));
    assert!(near(ext.area(), 9000.0 * 3000.0, 1e-3));
    run(&mut s, "type", json!({"category": "wall", "name": "Thick", "thickness": 600}));
    run(&mut s, "modify", json!({"id": w, "type": "Thick"}));
    assert!(near3(chart(&s, w, FaceId::Exterior).point(uv), [2000.0, 1300.0, 1200.0], 1e-9));
    assert!(near3(chart(&s, w, FaceId::Interior).point(uv), [2000.0, 700.0, 1200.0], 1e-9));
}
