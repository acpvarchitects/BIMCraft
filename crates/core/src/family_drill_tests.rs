//! GEO-2e: a void drilling straight through an extrusion is a hole of its profile, swept with it, not a cut of triangles:
//! the same solid, closed, with fewer triangles; and where it doesn't drill straight through, or meets another void, it is
//! cut as before.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::Session;
use crate::family::Piece;
use crate::view3d::P3;

/// A 1000 x 1000 x 100 slab (form 1) and `voids` after it; its pieces for its one type.
fn slab_less(voids: &[Value]) -> Vec<Piece> {
    let mut forms = vec![json!({"profile": [[0, 0], [1000, 0], [1000, 1000], [0, 1000]], "from": 0, "to": 100})];
    forms.extend(voids.iter().cloned());
    let mut s = Session::new();
    s.execute("family", &json!({"name": "S", "category": "generic", "types": {"A": {}}, "forms": forms})).unwrap_or_else(|e| panic!("{e}"));
    let f = &s.model.families["S"];
    f.pieces(&f.values("A", &BTreeMap::new()).unwrap()).unwrap()
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64, from: f64, to: f64) -> Value {
    json!({"profile": [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], "from": from, "to": to, "void": true})
}

fn volume(tris: &[[P3; 3]]) -> f64 {
    crate::ifc_geometry_tests::volume(tris)
}

/// Every edge shared by exactly two triangles, once each way.
fn closed(tris: &[[P3; 3]]) -> bool {
    let key = |p: &P3| p.map(|x| (x * 1e3).round() as i64);
    let mut edges: std::collections::HashMap<([i64; 3], [i64; 3]), i32> = std::collections::HashMap::new();
    for t in tris {
        for k in 0..3 {
            let (a, b) = (key(&t[k]), key(&t[(k + 1) % 3]));
            *edges.entry((a, b)).or_default() += 1;
            *edges.entry((b, a)).or_default() -= 1;
        }
    }
    edges.values().all(|n| *n == 0)
}

#[test]
fn a_void_through_a_slab_is_a_hole_of_its_profile() {
    let p = slab_less(&[square(400.0, 400.0, 600.0, 600.0, -10.0, 110.0)]);
    let tris = &p[0].tris;
    assert!((volume(tris) - (1e8 - 200.0 * 200.0 * 100.0)).abs() < 1e-3, "{}", volume(tris));
    assert!(closed(tris));
    // A square with a square hole: its two caps of 8 triangles each, its 8 sides of 2.
    assert_eq!(tris.len(), 32, "swept with its hole, not cut");
    // Two holes apart, one reaching just to the slab's faces.
    let p = slab_less(&[square(100.0, 100.0, 300.0, 300.0, -10.0, 110.0), square(700.0, 700.0, 900.0, 900.0, 0.0, 100.0)]);
    assert!((volume(&p[0].tris) - (1e8 - 2.0 * 200.0 * 200.0 * 100.0)).abs() < 1e-3);
    assert!(closed(&p[0].tris));
}

#[test]
fn a_void_that_doesnt_drill_straight_through_is_cut_as_before() {
    // Half way down: a pocket, cut.
    let pocket = slab_less(&[square(400.0, 400.0, 600.0, 600.0, 50.0, 110.0)]);
    assert!((volume(&pocket[0].tris) - (1e8 - 200.0 * 200.0 * 50.0)).abs() < 1e-3);
    // Over the slab's edge: a notch, cut.
    let notch = slab_less(&[square(900.0, 400.0, 1100.0, 600.0, -10.0, 110.0)]);
    assert!((volume(&notch[0].tris) - (1e8 - 100.0 * 200.0 * 100.0)).abs() < 1e-3);
    // Two voids overlapping each other: both cut, as before.
    let both = slab_less(&[square(400.0, 400.0, 600.0, 600.0, -10.0, 110.0), square(500.0, 400.0, 700.0, 600.0, -10.0, 110.0)]);
    assert!((volume(&both[0].tris) - (1e8 - 300.0 * 200.0 * 100.0)).abs() < 1e-3, "{}", volume(&both[0].tris));
    // Cut, its triangles meet at corners a cap's edges run through: more of them than the drilled slab's 32.
    assert!(both[0].tris.len() > 32);
}
