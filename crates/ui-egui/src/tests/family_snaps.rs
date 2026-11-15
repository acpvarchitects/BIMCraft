//! FE-21: the snap overrides in the family document, as the project's. SE, SM, SI, SC, SP, SQ, ST and SN keep the next point
//! picked to that snap alone, SO picks it with no snap, and SS ends the override; the point it was for ends it too, as do Escape,
//! another tool and another view. A form's sketch and the Modify and Measure tools' points follow it, and the status bar says
//! it is waiting. SX and SW are no keys of the family: it has no points and no work plane grid to snap to.
use super::family_drag_move::{family_with, plan_of};
use super::family_views::{click_family, editor, view};
use super::*;
use crate::Once;
use bimcraft_core::family::views::{Side, Which};
use bimcraft_core::snap::Kind;
use egui::accesskit::Role;

fn plan() -> Harness<'static, BimApp> {
    plan_of(&family_with(""))
}

fn once(h: &Harness<BimApp>) -> Option<Once> {
    h.state().snap_once
}

fn button(h: &mut Harness<BimApp>, label: &str) {
    h.get_by_role_and_label(Role::Button, label).click();
    h.run();
}

/// The points the sketch's shape has so far.
fn points(h: &Harness<BimApp>) -> Vec<P2> {
    editor(h).sketch.as_ref().expect("sketching").points.clone()
}

/// Extrusion's sketch begun in the plan, Line in hand.
fn sketching(h: &mut Harness<BimApp>) {
    button(h, "Extrusion");
    button(h, "Line");
}

/// A click at `q` of the plan's work plane (Ref. Level), then the frame after it.
fn click_plan(h: &mut Harness<BimApp>, q: P2) {
    click_family(h, [q.0, q.1, 0.0]);
    h.run();
}

#[test]
fn each_snap_key_overrides_the_next_point_in_a_family_and_ss_ends_it() {
    let mut h = plan();
    for (letters, o) in [
        ("SE", Once::Only(Kind::Endpoint)),
        ("SM", Once::Only(Kind::Midpoint)),
        ("SI", Once::Only(Kind::Intersection)),
        ("SC", Once::Only(Kind::Centre)),
        ("SP", Once::Only(Kind::Perpendicular)),
        ("SQ", Once::Only(Kind::Quadrant)),
        ("ST", Once::Only(Kind::Tangent)),
        ("SN", Once::Only(Kind::Nearest)),
        ("SO", Once::Off),
    ] {
        shortcut(&mut h, letters);
        assert_eq!(once(&h), Some(o), "{letters}");
        assert!(h.state().active_family().is_some() && h.state().tool == Tool::Modify, "{letters}: nothing of the project's");
    }
    shortcut(&mut h, "SS");
    assert_eq!(once(&h), None, "SS ends it");
    // No points to snap to, and no work plane grid: no keys of the family's.
    for letters in ["SX", "SW"] {
        shortcut(&mut h, letters);
        assert_eq!(once(&h), None, "{letters}");
    }
}

#[test]
fn with_shortcuts_need_ctrl_the_snap_keys_are_typed_with_ctrl_held() {
    let mut h = plan();
    h.state_mut().shortcuts_need_ctrl = true;
    shortcut(&mut h, "SE");
    assert_eq!(once(&h), Some(Once::Only(Kind::Endpoint)));
    shortcut(&mut h, "SO");
    assert_eq!(once(&h), Some(Once::Off));
    shortcut(&mut h, "SS");
    assert_eq!(once(&h), None);
    assert!(!editor(&h).unsaved(), "no Ctrl+S on the way");
}

#[test]
fn a_sketch_point_takes_the_override_and_the_next_one_snaps_as_ever() {
    let mut h = plan();
    sketching(&mut h);
    // Box A's corner at (500, 500): a click 5 and 7 off it lands on it.
    let off = P2(505.0, 493.0);
    click_plan(&mut h, off);
    assert_eq!(points(&h), [P2(500.0, 500.0)], "the corner, as ever");
    key(&mut h, Key::Escape);
    // Nearest alone: onto the east edge.
    shortcut(&mut h, "SN");
    click_plan(&mut h, off);
    assert_eq!(points(&h), [P2(500.0, 493.0)], "the nearest point of an edge");
    assert_eq!(once(&h), None, "the point it was for ends it");
    key(&mut h, Key::Escape);
    // Snaps off: the point as picked, to the nearest 10 mm.
    shortcut(&mut h, "SO");
    let off = P2(507.0, 486.0);
    let mark = editor(&h).views.screen_of([off.0, off.1, 0.0]).unwrap();
    h.hover_at(mark);
    h.run();
    assert!(editor(&h).sketch.as_ref().unwrap().snapped().is_none(), "no marker with snaps off");
    click_plan(&mut h, off);
    assert_eq!(points(&h), [P2(510.0, 490.0)], "where it was clicked, to the nearest 10 mm");
    assert_eq!(once(&h), None);
    key(&mut h, Key::Escape);
    click_plan(&mut h, off);
    assert_eq!(points(&h), [P2(500.0, 500.0)], "then the corner again");
}

#[test]
fn escape_another_tool_and_another_view_end_a_waiting_override() {
    let mut h = plan();
    sketching(&mut h);
    click_plan(&mut h, P2(-2000.0, -2000.0));
    shortcut(&mut h, "SE");
    key(&mut h, Key::Escape);
    assert_eq!(once(&h), None, "Escape ends the override first");
    assert_eq!(points(&h).len(), 1, "and only that");
    shortcut(&mut h, "SE");
    button(&mut h, "Rectangle");
    assert_eq!(once(&h), None, "another shape");
    shortcut(&mut h, "SE");
    shortcut(&mut h, "MD");
    assert_eq!(once(&h), None, "Modify");
    shortcut(&mut h, "SE");
    view(&mut h, Some(Which::Elevation(Side::Front)));
    assert_eq!(once(&h), None, "another view");
}

#[test]
fn the_status_bar_says_an_override_is_waiting_in_a_family() {
    let mut h = plan();
    assert!(h.query_by_label("Snap: Endpoint (next point)").is_none());
    shortcut(&mut h, "SE");
    assert!(h.query_by_label("Snap: Endpoint (next point)").is_some());
    shortcut(&mut h, "SO");
    assert!(h.query_by_label("Snaps off (next point)").is_some());
    shortcut(&mut h, "SS");
    assert!(h.query_by_label("Snaps off (next point)").is_none());
}

#[test]
fn modify_and_measure_points_follow_the_override() {
    use super::family_drag_move::{pick, points};
    use crate::editor::select_box::Picked;
    // Move's first point 20 mm east of Center (Left/Right): onto it as ever, so box A goes 600 east; with snaps off, from where
    // it was clicked, so 580.
    for (letters, by) in [(None, 600.0), (Some("SO"), 580.0), (Some("SI"), 580.0), (Some("SN"), 600.0)] {
        let mut h = plan();
        pick(&mut h, &[Picked::Form(0)]);
        let before = points(&h, 0);
        shortcut(&mut h, "MV");
        if let Some(letters) = letters {
            shortcut(&mut h, letters);
        }
        click_plan(&mut h, P2(20.0, 200.0));
        assert_eq!(once(&h), None, "{letters:?}: Move's point took it");
        click_plan(&mut h, P2(600.0, 200.0));
        let after = points(&h, 0);
        for (a, b) in before.iter().zip(&after) {
            assert!((b[0] - a[0] - by).abs() < 1.0 && (b[1] - a[1]).abs() < 1.0, "{letters:?}: {a:?} → {b:?}");
        }
    }
    // Measure's first point takes it too.
    let mut h = plan();
    shortcut(&mut h, "ME");
    shortcut(&mut h, "SO");
    click_plan(&mut h, P2(20.0, 200.0));
    assert_eq!(once(&h), None, "Measure's point took it");
}

#[test]
fn snap_to_quadrants_and_tangents_are_actions_a_key_can_be_given() {
    for (action, kind) in [("Snap to Quadrants", Kind::Quadrant), ("Snap to Tangents", Kind::Tangent)] {
        assert!(crate::shortcuts::ACTIONS.contains(&action), "{action}");
        // On the project's tab.
        let mut app = BimApp::new(Session::new());
        app.custom_shortcuts.insert("QQ".to_string(), action.to_string());
        app.typed = "QQ".to_string();
        assert!(app.shortcut());
        assert_eq!(app.snap_once, Some(Once::Only(kind)), "{action}");
        // On a family's.
        let mut h = plan();
        h.state_mut().custom_shortcuts.insert("QQ".to_string(), action.to_string());
        shortcut(&mut h, "QQ");
        assert_eq!(once(&h), Some(Once::Only(kind)), "{action} in a family");
    }
}

/// The plan with a snap override waiting, Endpoint and then Midpoint, its marker under the pointer and the status bar saying it,
/// in the three looks, when `BIMCRAFT_FE21_DUMP` names a folder.
#[test]
fn family_snap_overrides_in_three_looks() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE21_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    for (name, dark, light_canvas) in [("light", false, false), ("dark", true, false), ("dark-ui-light-canvas", true, true)] {
        let mut h = app(&family_with(""));
        h.set_size(egui::vec2(1600.0, 1000.0));
        h.state_mut().edit_family("Movers");
        h.run();
        view(&mut h, Some(Which::Plan));
        h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        h.state_mut().light_canvas = light_canvas;
        h.run();
        sketching(&mut h);
        click_plan(&mut h, P2(-200.0, 1000.0));
        let save = |h: &mut Harness<BimApp>, what: &str, at: P2| {
            let pos = editor(h).views.screen_of([at.0, at.1, 0.0]).unwrap();
            h.hover_at(pos);
            for _ in 0..4 {
                h.run();
            }
            h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe21-{what}-{name}.png"))).unwrap();
        };
        shortcut(&mut h, "SE");
        save(&mut h, "endpoint", P2(470.0, 520.0));
        shortcut(&mut h, "SM");
        save(&mut h, "midpoint", P2(330.0, 515.0));
    }
}
