//! Modify > Geometry > Cut Geometry and Uncut Geometry in a plan (T23): pick the wall, then the component whose voids cut it.

use super::*;

/// A wall along x, and a cutter at (3000, 0): a void 400 by 800 from 900 to 1500 up, which a plan doesn't draw, and a marker
/// 100 square at (3000, -600), which it does. The tool picks the cutter by the void's outline (its east side at x 3200) or its
/// place, not by the marker.
const CUTTER: &str = r#"family {"name": "Cutter", "category": "generic", "types": {"A": {}}, "options": {"cut_with_voids_when_loaded": true}, "forms": [{"void": true, "profile": [[-200, -400], [200, -400], [200, 400], [-200, 400]], "from": 900, "to": 1500}, {"profile": [[-50, -650], [50, -650], [50, -550], [-50, -550]], "from": 0, "to": 50}]}
wall {"points": [[0, 0], [6000, 0]], "height": 3000}
component {"family": "Cutter", "at": [3000, 0]}"#;

/// On the void's east side, clear of the wall.
const EDGE: P2 = P2(3200.0, -300.0);

fn cuts(h: &Harness<BimApp>) -> usize {
    let m = &h.state().session.model;
    ids(h, "wall").iter().map(|w| m.wall(*w).map_or(0, |w| w.void_cuts.len())).sum()
}

#[test]
fn cut_geometry_takes_the_wall_then_the_cutter_and_uncut_takes_it_back() {
    let mut h = app(CUTTER);
    topmost(&mut h, "Modify");
    press(&mut h, "Cut");
    press(&mut h, "Cut Geometry");
    assert!(h.state().prompt().contains("wall to cut"), "{}", h.state().prompt());
    // A click on the cutter first picks nothing: the wall comes first.
    click(&mut h, P2(3000.0, -600.0));
    assert!(matches!(h.state().tool, Tool::CutGeometry { wall: None, .. }));
    click(&mut h, P2(1000.0, 0.0));
    assert!(h.state().prompt().contains("component"), "{}", h.state().prompt());
    // The marker the plan draws is not what the second pick takes; the void's edge is.
    click(&mut h, P2(3000.0, -600.0));
    assert_eq!(cuts(&h), 0);
    click(&mut h, EDGE);
    assert_eq!(cuts(&h), 1, "{:?}", h.state().status);
    // The tool goes on, for the next wall.
    assert!(matches!(h.state().tool, Tool::CutGeometry { uncut: false, wall: None }));
    press(&mut h, "Cut");
    press(&mut h, "Uncut Geometry");
    click(&mut h, P2(1000.0, 0.0));
    click(&mut h, EDGE);
    assert_eq!(cuts(&h), 0, "{:?}", h.state().status);
}

#[test]
fn cu_starts_cut_geometry_and_esc_lets_go_of_the_wall_then_the_tool() {
    let mut h = app(CUTTER);
    let before = h.state().session.model.elements.clone();
    shortcut(&mut h, "CU");
    assert!(matches!(h.state().tool, Tool::CutGeometry { uncut: false, wall: None }));
    click(&mut h, P2(1000.0, 0.0));
    assert!(matches!(h.state().tool, Tool::CutGeometry { wall: Some(_), .. }));
    key(&mut h, Key::Escape);
    assert!(matches!(h.state().tool, Tool::CutGeometry { wall: None, .. }));
    key(&mut h, Key::Escape);
    assert_eq!(h.state().tool, Tool::Modify);
    assert_eq!(h.state().session.model.elements, before, "Esc twice edits nothing");
}

#[test]
fn a_cut_the_core_refuses_says_why_and_leaves_the_wall() {
    // The family's flag off: Cut is refused with the core's reason.
    let script = CUTTER.replace("\"cut_with_voids_when_loaded\": true", "\"cut_with_voids_when_loaded\": false");
    let mut h = app(&script);
    shortcut(&mut h, "CU");
    click(&mut h, P2(1000.0, 0.0));
    click(&mut h, EDGE);
    assert_eq!(cuts(&h), 0);
    let status = &h.state().status;
    assert!(status.error && status.text.contains("Cut with Voids When Loaded"), "{status:?}");
}

#[test]
fn a_void_only_family_is_picked_by_its_outline_and_uncut_finds_a_sleeping_tie() {
    // Only the void: the plan draws nothing of the component.
    let script = CUTTER.replace(r#", {"profile": [[-50, -650], [50, -650], [50, -550], [-50, -550]], "from": 0, "to": 50}"#, "");
    assert!(!script.contains("-650"));
    let mut h = app(&script);
    shortcut(&mut h, "CU");
    click(&mut h, P2(1000.0, 0.0));
    click(&mut h, EDGE);
    assert_eq!(cuts(&h), 1, "{:?}", h.state().status);
    // The family's flag off, the tie sleeps and cuts nothing; Uncut still finds it by the void's outline.
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    h.state_mut().session.execute("family", &json!({"name": "Cutter", "options": {"cut_with_voids_when_loaded": false}})).unwrap();
    h.run();
    topmost(&mut h, "Modify");
    press(&mut h, "Cut");
    press(&mut h, "Uncut Geometry");
    click(&mut h, P2(1000.0, 0.0));
    click(&mut h, EDGE);
    assert_eq!(cuts(&h), 0, "{:?}", h.state().status);
}
