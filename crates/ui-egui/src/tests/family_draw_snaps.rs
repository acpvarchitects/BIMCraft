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

/// A model line of the family's own from `a` to `b` on Ref. Level, put straight into the editor; its GUID.
fn model_line(h: &mut Harness<BimApp>, a: [f64; 2], b: [f64; 2]) -> bimcraft_core::ids::Guid {
    let guid = bimcraft_core::ids::Guid::new();
    ed(h).model_lines.push(bimcraft_core::family::ModelLine {
        guid,
        start: [a[0], a[1], 0.0],
        end: [b[0], b[1], 0.0],
        ..Default::default()
    });
    h.run();
    guid
}

#[test]
fn reference_plane_points_snap_onto_a_model_line() {
    let mut h = plan();
    model_line(&mut h, [1700.0, -1000.0], [1700.0, 1000.0]);
    shortcut(&mut h, "RP");
    click(&mut h, 1720.0, 300.0);
    click(&mut h, 2300.0, 900.0);
    let p = editor(&h).planes.last().unwrap().clone();
    assert!(near(p.start, [1700.0, 300.0]), "onto the model line: {:?}", p.start);
}

#[test]
fn what_a_view_hides_is_no_snap_target() {
    use super::family_drag_move::{EDGE, g, pick};
    use crate::editor::select_box::Picked;
    // Edge hidden for now (HH): the point stays as clicked, 20 mm east of where Edge runs.
    let mut h = plan();
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    shortcut(&mut h, "HH");
    shortcut(&mut h, "RP");
    click(&mut h, 1020.0, 400.0);
    click(&mut h, 1620.0, 700.0);
    let p = editor(&h).planes.last().unwrap().clone();
    assert!(near(p.start, [1020.0, 400.0]), "Edge hidden, nothing to snap to: {:?}", p.start);
    // A model line the view leaves out for now likewise: box A isolated (HI), which the view draws alone.
    let mut h = plan();
    model_line(&mut h, [1700.0, -1000.0], [1700.0, 1000.0]);
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "HI");
    shortcut(&mut h, "RP");
    click(&mut h, 1720.0, 300.0);
    click(&mut h, 2300.0, 900.0);
    let p = editor(&h).planes.iter().rev().find(|p| near(p.end, [2300.0, 900.0])).cloned().expect("a plane drawn");
    assert!(near(p.start, [1720.0, 300.0]), "the hidden model line pulls nothing: {:?}", p.start);
}

#[test]
fn measure_in_an_elevation_snaps_to_a_plane_seen_edge_on() {
    use super::family_views::view;
    use bimcraft_core::family::views::{Side, Which};
    // Front sees Edge (x 1000) edge on: 20 mm off it, onto it; the second point, free, as clicked.
    let mut h = plan();
    view(&mut h, Some(Which::Elevation(Side::Front)));
    shortcut(&mut h, "ME");
    click_family(&mut h, [1020.0, 0.0, 500.0]);
    h.run();
    click_family(&mut h, [1300.0, 0.0, 1500.0]);
    h.run();
    let r = editor(&h).measured().expect("a ruler laid");
    assert!((r.dx.abs() - 300.0).abs() < 1e-2 && (r.dy.abs() - 1000.0).abs() < 1e-2, "from Edge: {r:?}");
}

#[test]
fn with_shift_a_point_is_squared_then_snapped_along_its_square_line() {
    // An oblique model line through (1850, 300). From (2500, 300), Shift near it: along y 300 onto it, at (1850, 300), not
    // onto its nearest point and then squared off it.
    let mut h = plan();
    model_line(&mut h, [1200.0, -1000.0], [2200.0, 1000.0]);
    shortcut(&mut h, "RP");
    click(&mut h, 2500.0, 300.0);
    let pos = editor(&h).views.screen_of([1860.0, 310.0, 0.0]).unwrap();
    h.event(egui::Event::ModifiersChanged(Modifiers::SHIFT));
    h.hover_at(pos);
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::SHIFT });
    }
    h.run();
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    let p = editor(&h).planes.last().unwrap().clone();
    assert!(near(p.end, [1850.0, 300.0]), "on the line and on the square: {:?}", p.end);
}

#[test]
fn two_clicks_that_snap_to_one_point_keep_the_first_and_say_so() {
    let mut h = plan();
    shortcut(&mut h, "RP");
    let before = editor(&h).planes.len();
    // Both near where Edge crosses Center (Front/Back): both onto (1000, 0).
    click(&mut h, 1015.0, 15.0);
    click(&mut h, 1010.0, -10.0);
    assert_eq!(editor(&h).planes.len(), before, "no plane of no length");
    assert!(matches!(editor(&h).views.drawing, Some(Some(q)) if (q.0 - 1000.0).abs() < 1e-6 && q.1.abs() < 1e-6), "the first kept");
    let said = format!("{:?} {}", editor(&h).views.status, h.state().status.text);
    assert!(said.contains("same"), "{said}");
}
