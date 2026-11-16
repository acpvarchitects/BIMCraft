//! FE-24, part 2: in the family's {3D} a Modify tool's point snaps on the work plane to what the view shows crossing it (the
//! planes' traces, the reference and model lines and the forms' edges, seen square on to it), as a plan's does; Mirror - Pick
//! Axis, Offset and Align pick references in the picture.

use super::family_drag_move::{EDGE, g, pick, status};
use super::family_modify_3d::{in_3d, tool};
use super::*;
use crate::editor::modify::Tool as M;
use crate::editor::select_box::Picked;
use bimcraft_core::geom::P2;

/// The Movers family in its {3D}, zoomed out to show Edge (x 1000) and the reference line (y 1200).
fn zoomed() -> Harness<'static, BimApp> {
    let mut h = in_3d();
    if let Some(c) = h.state_mut().active_family_mut().and_then(|f| f.orbit.camera.as_mut()) {
        c.scale *= 0.5;
    }
    for _ in 0..4 {
        h.run();
    }
    h
}

/// A click in {3D} a few pixels off where family point `p` shows.
fn click_off(h: &mut Harness<BimApp>, p: [f64; 3]) {
    let at = h.state().family_screen3(p).expect("{3D} drawn") + egui::vec2(3.0, 2.0);
    click_at(h, at);
}

/// Where Move's first click landed, on the work plane (Ref. Level: the family's x and y).
fn from(h: &Harness<BimApp>) -> P2 {
    match tool(h) {
        Some(M::Move { from: Some(q), .. }) => q,
        t => panic!("no first point: {t:?}: {}", status(h)),
    }
}

fn near(a: P2, b: P2) -> bool {
    a.dist(b) < 1e-6
}

#[test]
fn a_modify_point_in_3d_snaps_to_a_planes_trace_and_where_two_cross() {
    // A few pixels off Edge (x 1000), 400 north of Center (Front/Back): onto Edge.
    let mut h = zoomed();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    click_off(&mut h, [1000.0, 400.0, 0.0]);
    let q = from(&h);
    assert!((q.0 - 1000.0).abs() < 1e-6 && (q.1 - 400.0).abs() > 1e-3, "onto Edge, along it as clicked: {q:?}");
    // Off where Edge crosses Center (Front/Back): onto the crossing.
    let mut h = zoomed();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    click_off(&mut h, [1000.0, 0.0, 0.0]);
    assert!(near(from(&h), P2(1000.0, 0.0)), "{:?}", from(&h));
}

#[test]
fn a_modify_point_in_3d_snaps_to_a_reference_line_and_a_forms_edge() {
    // The reference line along y 1200, from x -200 to 200.
    let mut h = zoomed();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    click_off(&mut h, [120.0, 1200.0, 0.0]);
    assert!((from(&h).1 - 1200.0).abs() < 1e-6, "{:?}", from(&h));
    // Box A's east edge, x 500, seen on Ref. Level from above.
    let mut h = zoomed();
    pick(&mut h, &[Picked::Form(1)]);
    shortcut(&mut h, "MV");
    click_off(&mut h, [500.0, 250.0, 0.0]);
    assert!((from(&h).0 - 500.0).abs() < 1e-6, "{:?}", from(&h));
}

#[test]
fn in_3d_what_the_view_hides_is_no_snap_target_and_so_takes_the_point_as_clicked() {
    use crate::editor::select_box::Picked;
    // Edge hidden for now (HH): the point stays off it.
    let mut h = zoomed();
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    shortcut(&mut h, "HH");
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    click_off(&mut h, [1000.0, 400.0, 0.0]);
    assert!((from(&h).0 - 1000.0).abs() > 1e-3, "Edge hidden: {:?}", from(&h));
    // SO: the next point as clicked.
    let mut h = zoomed();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    shortcut(&mut h, "SO");
    click_off(&mut h, [1000.0, 400.0, 0.0]);
    assert!((from(&h).0 - 1000.0).abs() > 1e-3, "SO: {:?}", from(&h));
}

/// Form `i`'s sketch's extent: least and most x, least and most y.
fn extent(h: &Harness<BimApp>, i: usize) -> [f64; 4] {
    let pts = super::family_drag_move::points(h, i);
    let f = |k: usize, most: bool| {
        pts.iter().map(|p| p[k]).fold(if most { f64::NEG_INFINITY } else { f64::INFINITY }, if most { f64::max } else { f64::min })
    };
    [f(0, false), f(0, true), f(1, false), f(1, true)]
}

fn about(a: [f64; 4], b: [f64; 4]) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-3)
}

/// Mirror - Pick Axis in hand in {3D} on box A, not copying.
fn mirroring() -> Harness<'static, BimApp> {
    let mut h = zoomed();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MM");
    super::family_drag_move::ed(&mut h).views.modify_options.mirror_copy = false;
    assert_eq!(tool(&h), Some(M::Mirror { draw: false, start: None }), "{}", status(&h));
    h
}

#[test]
fn mirror_pick_axis_in_3d_takes_a_plane_a_reference_line_or_a_forms_edge() {
    // Edge (x 1000), by the line it was drawn as: box A from x 100 to 500 goes to 1500 to 1900.
    let mut h = mirroring();
    click_off(&mut h, [1000.0, 600.0, 0.0]);
    assert!(about(extent(&h, 0), [1500.0, 1900.0, 100.0, 500.0]), "{:?}: {}", extent(&h, 0), status(&h));
    // The reference line along y 1200: from y 100 to 500 to 1900 to 2300.
    let mut h = mirroring();
    click_off(&mut h, [0.0, 1200.0, 0.0]);
    assert!(about(extent(&h, 0), [100.0, 500.0, 1900.0, 2300.0]), "{:?}: {}", extent(&h, 0), status(&h));
    // Box B's east edge on its top (x -600), seen square on to Ref. Level: from x 100 to 500 to -1700 to -1300.
    let mut h = mirroring();
    click_off(&mut h, [-600.0, 300.0, 300.0]);
    assert!(about(extent(&h, 0), [-1700.0, -1300.0, 100.0, 500.0]), "{:?}: {}", extent(&h, 0), status(&h));
}

#[test]
fn mirror_pick_axis_in_3d_refuses_a_line_along_the_planes_normal_and_a_plane_parallel_to_it() {
    use super::family_drag_move::ed;
    // An upright model line: square to Ref. Level, it is a point on it.
    let mut h = mirroring();
    ed(&mut h).model_lines.push(bimcraft_core::family::ModelLine {
        guid: bimcraft_core::ids::Guid::new(),
        start: [1400.0, -600.0, 0.0],
        end: [1400.0, -600.0, 900.0],
        ..Default::default()
    });
    h.run();
    let before = extent(&h, 0);
    click_off(&mut h, [1400.0, -600.0, 450.0]);
    assert!(about(extent(&h, 0), before), "nothing mirrored");
    assert!(status(&h).contains("square to the work plane"), "{}", status(&h));
    // A plane level with Ref. Level, 600 over it: it crosses the plane nowhere.
    let mut h = mirroring();
    let shelf: bimcraft_core::family::RefPlane = serde_json::from_value(serde_json::json!({
        "guid": bimcraft_core::ids::Guid::new(), "name": "Shelf", "start": [-500, 800, 600], "end": [500, 800, 600], "up": [0, 1, 0]
    }))
    .unwrap();
    ed(&mut h).planes.push(shelf);
    h.run();
    click_off(&mut h, [0.0, 800.0, 600.0]);
    assert!(about(extent(&h, 0), before), "nothing mirrored");
    assert!(status(&h).contains("parallel to the work plane"), "{}", status(&h));
    // A line lying on the plane is an axis: the reference line, as above.
    click_off(&mut h, [0.0, 1200.0, 0.0]);
    assert!(about(extent(&h, 0), [100.0, 500.0, 1900.0, 2300.0]), "{:?}: {}", extent(&h, 0), status(&h));
}
