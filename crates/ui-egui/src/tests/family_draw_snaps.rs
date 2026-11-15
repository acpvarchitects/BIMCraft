//! FE-21b: Measure and the Reference Plane, Reference Line and Model Line tools' points in a family's views snap to the planes'
//! and lines' traces, as a Modify tool's do (FE-8): where two cross near the click, else the nearest point of the nearest; a
//! snap override (FE-21) governs the next point, and SO picks it as clicked.
use super::family_drag_move::{ed, plan};
use super::family_views::{click_family, editor};
use super::*;

/// A click at (x, y) of the plan's work plane (Ref. Level), then the frame after it.
fn click(h: &mut Harness<BimApp>, x: f64, y: f64) {
    click_family(h, [x, y, 0.0]);
    h.run();
}

/// Within 0.01 mm: a point snapped lands exactly, one as clicked where the pixel under it is.
fn near(a: [f64; 3], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-2 && (a[1] - b[1]).abs() < 1e-2
}

#[test]
fn measure_in_a_family_plan_snaps_to_a_plane_and_to_where_two_cross() {
    // 20 mm east of Edge (x 1000), then 15 mm off where Edge crosses Center (Front/Back): on Edge, then on the crossing.
    let mut h = plan();
    shortcut(&mut h, "ME");
    click(&mut h, 1020.0, 400.0);
    click(&mut h, 1015.0, 15.0);
    let r = editor(&h).measured().expect("a ruler laid");
    assert!(r.dx.abs() < 1e-6 && (r.dy + 400.0).abs() < 1e-6 && (r.length - 400.0).abs() < 1e-6, "{r:?}");
    // SO: the first point as clicked.
    let mut h = plan();
    shortcut(&mut h, "ME");
    shortcut(&mut h, "SO");
    click(&mut h, 1020.0, 400.0);
    click(&mut h, 1015.0, 15.0);
    let r = editor(&h).measured().expect("a ruler laid");
    assert!((r.dx + 20.0).abs() < 1e-2 && (r.dy + 400.0).abs() < 1e-2, "SO keeps the first as clicked: {r:?}");
}

#[test]
fn reference_plane_points_snap_to_a_plane_and_to_where_two_cross() {
    let mut h = plan();
    let before = editor(&h).planes.len();
    shortcut(&mut h, "RP");
    click(&mut h, 1015.0, 15.0);
    click(&mut h, 1620.0, 700.0);
    assert_eq!(editor(&h).planes.len(), before + 1, "{}", h.state().status.text);
    let p = editor(&h).planes.last().unwrap().clone();
    assert!(near(p.start, [1000.0, 0.0]), "from where Edge crosses Center (Front/Back): {:?}", p.start);
    assert!(near(p.end, [1620.0, 700.0]), "to where it was clicked, nothing near: {:?}", p.end);
    // The second point snaps too: onto Edge.
    let mut h = plan();
    shortcut(&mut h, "RP");
    click(&mut h, 1620.0, 700.0);
    click(&mut h, 1020.0, 400.0);
    let p = editor(&h).planes.last().unwrap().clone();
    assert!(near(p.end, [1000.0, 400.0]), "onto Edge: {:?}", p.end);
}

#[test]
fn reference_line_points_snap_and_a_snap_override_governs_the_next() {
    let mut h = plan();
    ed(&mut h).reference_line_tool();
    h.run();
    let before = editor(&h).lines.len();
    click(&mut h, 1015.0, 15.0);
    click(&mut h, 1620.0, 700.0);
    assert_eq!(editor(&h).lines.len(), before + 1, "{}", h.state().status.text);
    let l = editor(&h).lines.last().unwrap().clone();
    assert!(near(l.start, [1000.0, 0.0]) && near(l.end, [1620.0, 700.0]), "{:?} {:?}", l.start, l.end);
    // SO for the first point: as clicked; it takes the override, so the second snaps as ever.
    let mut h = plan();
    ed(&mut h).reference_line_tool();
    h.run();
    shortcut(&mut h, "SO");
    click(&mut h, 1015.0, 15.0);
    assert_eq!(h.state().snap_once, None, "the first point took it");
    click(&mut h, 1620.0, 420.0);
    click(&mut h, 1620.0, 900.0);
    click(&mut h, 1020.0, 420.0);
    let lines = &editor(&h).lines;
    let first = &lines[lines.len() - 2];
    assert!(near(first.start, [1015.0, 15.0]), "SO: as clicked: {:?}", first.start);
    let second = lines.last().unwrap();
    assert!(near(second.end, [1000.0, 420.0]), "the next line snaps again: {:?}", second.end);
}

#[test]
fn model_line_points_snap_to_a_reference_line() {
    // The reference line along y 1200 from x -200 to 200: 20 mm north of it, onto it.
    let mut h = plan();
    ed(&mut h).model_line_tool();
    h.run();
    click(&mut h, 0.0, 1220.0);
    click(&mut h, -400.0, 1800.0);
    let l = editor(&h).model_lines.last().cloned().expect("a model line");
    assert!(near(l.start, [0.0, 1200.0]), "on the reference line, where it crosses Center (Left/Right): {:?}", l.start);
    assert!(near(l.end, [-400.0, 1800.0]), "{:?}", l.end);
}
