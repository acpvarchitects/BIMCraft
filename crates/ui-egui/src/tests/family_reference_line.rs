//! FE-11: Create > Datum > Reference Line in a family of any category that isn't adaptive (an adaptive family draws its own in
//! {3D}): two clicks in a plan or an elevation, a line on the view's work plane, selected once drawn; refused, saying why,
//! where the view sees the work plane edge on. Create Similar on a reference line turns the tool on. No key yet (#403).
use super::family_drag_move::status;
use super::family_views::{click_family, editor, open, view};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::views::{Side, Which};
use bimcraft_core::family::{CENTER_FRONT_BACK, PlaneRef, REF_LEVEL};

const BOX: &str = r#"family {"name": "Box", "category": "generic", "types": {"T": {}}, "forms": [{"profile": [[500, 300], [1000, 300], [1000, 600], [500, 600]], "to": 2000}]}"#;

const WINDOW: &str = r#"family {"name": "Pane", "category": "window", "parameters": [{"name": "Width", "value": 900}, {"name": "Height", "value": 1200}], "types": {"T": {}}, "forms": [{"plane": "xz", "profile": [[-450, 0], [450, 0], [450, 1200], [-450, 1200]], "from": -40, "to": 40}]}"#;

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 20.0)
}

#[test]
fn a_reference_line_drawn_in_plan_lies_on_ref_level_and_loads_with_the_family() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Plan));
    press(&mut h, "Reference Line");
    click_family(&mut h, [-1000.0, -800.0, 0.0]);
    click_family(&mut h, [1000.0, -800.0, 0.0]);
    let lines = &editor(&h).lines;
    assert_eq!(lines.len(), 1, "one line drawn");
    let l = &lines[0];
    assert!(near(l.start, [-1000.0, -800.0, 0.0]) && near(l.end, [1000.0, -800.0, 0.0]), "{l:?}");
    assert_eq!(l.host, PlaneRef::Plane(REF_LEVEL), "on the plan's work plane");
    assert_eq!(editor(&h).only_selected(), Some(Picked::Line(l.guid)), "selected once drawn");
    // The tool stays on for the next one, as Reference Plane's does.
    click_family(&mut h, [-1000.0, 800.0, 0.0]);
    click_family(&mut h, [-1000.0, 1600.0, 0.0]);
    assert_eq!(editor(&h).lines.len(), 2, "a second line");
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    press(&mut h, "Load into Project");
    if h.query_by_label("Overwrite the existing version").is_some() {
        press(&mut h, "Overwrite the existing version");
    }
    assert_eq!(h.state().session.model.families["Box"].lines.len(), 2, "the lines in the family loaded");
}

#[test]
fn a_reference_line_drawn_in_a_windows_front_elevation_lies_on_center_front_back() {
    let (mut h, _) = open(WINDOW, "Pane");
    view(&mut h, Some(Which::Elevation(Side::Front)));
    press(&mut h, "Reference Line");
    click_family(&mut h, [-300.0, 0.0, 200.0]);
    click_family(&mut h, [300.0, 0.0, 900.0]);
    let lines = &editor(&h).lines;
    assert_eq!(lines.len(), 1, "one line drawn: {}", status(&h));
    let l = &lines[0];
    assert!(near(l.start, [-300.0, 0.0, 200.0]) && near(l.end, [300.0, 0.0, 900.0]), "{l:?}");
    assert_eq!(l.host, PlaneRef::Plane(CENTER_FRONT_BACK), "on the elevation's work plane");
}

#[test]
fn a_work_plane_the_view_sees_edge_on_takes_no_reference_line_and_says_why() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Plan));
    h.state_mut().active_family_mut().unwrap().views.set_work_plane(PlaneRef::Plane(CENTER_FRONT_BACK));
    press(&mut h, "Reference Line");
    click_family(&mut h, [-1000.0, -800.0, 0.0]);
    click_family(&mut h, [1000.0, -800.0, 0.0]);
    assert!(editor(&h).lines.is_empty(), "no line");
    assert!(status(&h).contains("edge on"), "{}", status(&h));
}

#[test]
fn create_similar_on_a_reference_line_turns_the_tool_on() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Plan));
    press(&mut h, "Reference Line");
    click_family(&mut h, [-1000.0, -800.0, 0.0]);
    click_family(&mut h, [1000.0, -800.0, 0.0]);
    key(&mut h, Key::Escape);
    let g = editor(&h).lines[0].guid;
    let ed = h.state_mut().active_family_mut().unwrap();
    ed.select_only(Picked::Line(g));
    ed.views.drawing = None;
    assert_eq!(ed.create_similar(), Ok(()));
    assert!(ed.views.drawing.is_some() && ed.views.drawing_line, "the Reference Line tool on");
}

#[test]
fn an_adaptive_family_has_no_reference_line_tool_in_its_plans() {
    let (mut h, ..) = super::family_temporary_hide::adaptive();
    view(&mut h, Some(Which::Plan));
    press(&mut h, "Reference Line");
    assert!(editor(&h).views.drawing.is_none(), "no Reference Line in an adaptive family's views");
}

#[test]
fn reference_line_is_a_family_command_with_no_key_yet() {
    assert!(crate::editor::keys::FAMILY_KEYLESS.contains(&"Reference Line"));
}
