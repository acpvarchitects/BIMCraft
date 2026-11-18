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

/// Its points, each at most once, rounded to a thousandth of a millimetre.
fn corners(tris: &[[P3; 3]]) -> std::collections::BTreeSet<[i64; 3]> {
    tris.iter().flatten().map(|p| p.map(|x| (x * 1e3).round() as i64)).collect()
}

#[test]
fn a_drilled_hole_is_where_its_void_is() {
    // An L, off the slab's centre and like no turn or mirror of itself: a hole shifted, turned or mirrored has other points.
    let l = [[150.0, 620.0], [380.0, 620.0], [380.0, 700.0], [230.0, 700.0], [230.0, 860.0], [150.0, 860.0]];
    let void = json!({"profile": l, "from": -10, "to": 110, "void": true});
    let p = slab_less(&[void]);
    let tris = &p[0].tris;
    let mut want = std::collections::BTreeSet::new();
    for z in [0.0, 100.0] {
        for [x, y] in [[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0], [0.0, 1000.0]].into_iter().chain(l) {
            want.insert([x, y, z].map(|v: f64| (v * 1e3).round() as i64));
        }
    }
    assert_eq!(corners(tris), want, "the slab's corners and the L's, top and bottom");
    // Swept with its hole: two caps of ten triangles (ten corners, one hole) and ten sides of two, no cut's extra corners.
    assert_eq!(tris.len(), 2 * 10 + 10 * 2, "{}", tris.len());
    assert!(closed(tris));
    let area = 230.0 * 80.0 + 80.0 * 160.0;
    assert!((volume(tris) - (1e8 - area * 100.0)).abs() < 1e-3, "{}", volume(tris));
}

#[test]
fn a_void_that_misses_the_body_leaves_the_others_drilled() {
    // The second void is off the slab: it cuts nothing, so the first still drills it.
    let p = slab_less(&[square(400.0, 400.0, 600.0, 600.0, -10.0, 110.0), square(1200.0, 400.0, 1400.0, 600.0, -10.0, 110.0)]);
    assert_eq!(p[0].tris.len(), 32, "swept with its hole, not cut");
    assert!((volume(&p[0].tris) - (1e8 - 200.0 * 200.0 * 100.0)).abs() < 1e-3);
}

#[test]
fn checking_where_voids_drill_has_a_budget() {
    // Many holes in a grid, their edges in line: drilled, every one. The checks of each against each are counted, on a
    // budget of their own, and one too small for them sends it to the cut.
    let holes: Vec<Value> = (0..36)
        .map(|k| {
            let (x, y) = (100.0 + 140.0 * (k % 6) as f64, 100.0 + 140.0 * (k / 6) as f64);
            square(x, y, x + 60.0, y + 60.0, -10.0, 110.0)
        })
        .collect();
    let mut forms = vec![json!({"profile": [[0, 0], [1000, 0], [1000, 1000], [0, 1000]], "from": 0, "to": 100})];
    forms.extend(holes);
    let mut s = Session::new();
    s.execute("family", &json!({"name": "S", "category": "generic", "types": {"A": {}}, "forms": forms})).unwrap_or_else(|e| panic!("{e}"));
    let f = &s.model.families["S"];
    let values = f.values("A", &BTreeMap::new()).unwrap();
    let mut read = std::collections::HashMap::new();
    let body = f.solid(0, &values, &mut read).unwrap().unwrap().tris;
    let voids: std::collections::HashMap<usize, Vec<[P3; 3]>> =
        (1..f.forms.len()).map(|i| (i, f.solid(i, &values, &mut read).unwrap().unwrap().tris)).collect();
    let mut shapes = std::collections::HashMap::new();
    let mut rich = crate::csg::Meter::default();
    let d = f.drill(0, &body, &voids, &values, &mut read, &mut shapes, &mut rich).expect("drilled");
    assert_eq!(d.holes.len(), 36, "each void's rings");
    assert!(d.holes.iter().all(|(_, rings)| rings.len() == 1));
    assert_eq!(f.pieces(&values).unwrap()[0].tris, d.tris, "the build drills it too");
    let mut poor = crate::csg::Meter::poor(2000);
    assert!(f.drill(0, &body, &voids, &values, &mut read, &mut shapes, &mut poor).is_none(), "out of budget: cut instead");
}

/// The family `forms` make, its values for its one type, and each form's solid.
fn solids(forms: Vec<Value>) -> (crate::family::Family, BTreeMap<String, crate::family::Val>, Vec<Vec<[P3; 3]>>) {
    let mut s = Session::new();
    s.execute("family", &json!({"name": "S", "category": "generic", "types": {"A": {}}, "forms": forms})).unwrap_or_else(|e| panic!("{e}"));
    let f = s.model.families["S"].clone();
    let values = f.values("A", &BTreeMap::new()).unwrap();
    let mut read = std::collections::HashMap::new();
    let tris = (0..f.forms.len()).map(|i| f.solid(i, &values, &mut read).unwrap().unwrap().tris).collect();
    (f, values, tris)
}

#[test]
fn a_drill_that_falls_back_leaves_the_cut_its_whole_budget() {
    // A round hole through the slab, then a pocket that doesn't go through: neither drills, and checking whether they do
    // mustn't take from what cutting them may spend. A family that cut with just enough before still builds.
    let slab = json!({"profile": [[0, 0], [1000, 0], [1000, 1000], [0, 1000]], "from": 0, "to": 100});
    let round = json!({"sketch": [{"centre": [300, 300], "radius": 100}], "from": -10, "to": 110, "void": true});
    let (f, values, tris) = solids(vec![slab, round, square(600.0, 600.0, 800.0, 800.0, 50.0, 110.0)]);
    let mut csg = crate::csg::Meter::default();
    let once = crate::csg::subtract(&tris[0], &tris[1], &mut csg).unwrap();
    let cut = crate::csg::subtract(&once, &tris[2], &mut csg).unwrap();
    let work = crate::csg::Meter::default().left() - csg.left();
    let built = f.build(&values, &mut std::collections::HashMap::new(), &mut crate::csg::Meter::poor(work)).expect("as much as before");
    assert_eq!(built[0].tris, cut, "cut as before");
    assert!(f.build(&values, &mut std::collections::HashMap::new(), &mut crate::csg::Meter::poor(work - 1)).is_err(), "no more");
}

#[test]
fn edges_in_line_but_for_rounding_still_meet() {
    // A slab turned on its plane, and a void inside it, one side along the slab's own: computed apart, the two lie on one
    // line only to within rounding. The void's side is the slab's, so it leaves a wall of no width: it isn't drilled.
    for degrees in [17.0_f64, 30.0, 41.0, 63.0] {
        let (s, c) = degrees.to_radians().sin_cos();
        let at = |x: f64, y: f64| json!([x * c - y * s, x * s + y * c]);
        let slab = json!({"profile": [at(0.0, 0.0), at(1000.0, 0.0), at(1000.0, 1000.0), at(0.0, 1000.0)], "from": 0, "to": 100});
        let flush = json!({"profile": [at(1000.0 / 3.0, 0.0), at(2000.0 / 3.0, 0.0), at(2000.0 / 3.0, 200.0), at(1000.0 / 3.0, 200.0)],
                           "from": -10, "to": 110, "void": true});
        let (f, values, tris) = solids(vec![slab, flush]);
        let voids = std::collections::HashMap::from([(1, tris[1].clone())]);
        let (mut read, mut shapes) = (std::collections::HashMap::new(), std::collections::HashMap::new());
        let d = f.drill(0, &tris[0], &voids, &values, &mut read, &mut shapes, &mut crate::csg::Meter::default());
        assert!(d.is_none(), "turned {degrees}°: a hole flush with the outline is cut, not drilled");
    }
}
