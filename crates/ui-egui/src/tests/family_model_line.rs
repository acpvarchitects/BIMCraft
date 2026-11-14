//! FE-15: Create > Model > Model Line in a family that isn't adaptive: two clicks in a plan or an elevation, a line of the
//! family's own on the view's work plane, which every view draws; picked by a click, taken away with Delete and back with
//! Undo, its subcategory set in Properties, carried with the plane it is drawn on. No key yet (#403).
use super::family_drag_move::{EDGE, drag, family_with, plan_of, status};
use super::family_views::{click_family, editor, open, view};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::views::{Side, Which};
use bimcraft_core::family::{CENTER_FRONT_BACK, PlaneRef, REF_LEVEL};

const BOX: &str = r#"family {"name": "Box", "category": "generic", "types": {"T": {}}, "forms": [{"profile": [[500, 300], [1000, 300], [1000, 600], [500, 600]], "to": 2000}]}"#;

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 20.0)
}

/// Model Line pressed: on the ribbon, or, in a window too narrow for it, from the Model panel's list.
fn model_line(h: &mut Harness<BimApp>) {
    if h.query_by_label("Model Line").is_none() {
        press(h, "Model ▾");
    }
    press(h, "Model Line");
}

/// A model line drawn in the view shown from `a` to `b`, the tool put down after.
fn draw(h: &mut Harness<BimApp>, a: [f64; 3], b: [f64; 3]) {
    model_line(h);
    click_family(h, a);
    click_family(h, b);
    key(h, Key::Escape);
}

#[test]
fn a_model_line_drawn_in_plan_lies_on_ref_level_is_selected_and_loads_with_the_family() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Plan));
    model_line(&mut h);
    click_family(&mut h, [-1000.0, -800.0, 0.0]);
    click_family(&mut h, [1000.0, -800.0, 0.0]);
    assert!(h.query_all_by_label("Modify | Place Lines").count() > 0, "the contextual tab names the tool");
    assert!(h.query_by_label_contains("a model line between them").is_some(), "the prompt says what is drawn");
    let ed = editor(&h);
    assert_eq!(ed.model_lines.len(), 1, "one model line: {}", status(&h));
    assert!(ed.lines.is_empty(), "no reference line");
    let l = &ed.model_lines[0];
    assert!(near(l.start, [-1000.0, -800.0, 0.0]) && near(l.end, [1000.0, -800.0, 0.0]), "{l:?}");
    assert_eq!(l.host, PlaneRef::Plane(REF_LEVEL));
    assert_eq!(ed.only_selected(), Some(Picked::ModelLine(l.guid)), "selected once drawn");
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    press(&mut h, "Load into Project");
    if h.query_by_label("Overwrite the existing version").is_some() {
        press(&mut h, "Overwrite the existing version");
    }
    assert_eq!(h.state().session.model.families["Box"].model_lines.len(), 1, "in the family loaded: {}", status(&h));
}

#[test]
fn a_model_line_drawn_in_the_front_elevation_lies_on_center_front_back() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Elevation(Side::Front)));
    draw(&mut h, [-300.0, 0.0, 200.0], [300.0, 0.0, 900.0]);
    let l = &editor(&h).model_lines[0];
    assert!(near(l.start, [-300.0, 0.0, 200.0]) && near(l.end, [300.0, 0.0, 900.0]), "{l:?}");
    assert_eq!(l.host, PlaneRef::Plane(CENTER_FRONT_BACK));
}

#[test]
fn a_click_on_a_model_line_selects_it_and_delete_takes_it_away_until_undo() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Plan));
    draw(&mut h, [-1000.0, -800.0, 0.0], [1000.0, -800.0, 0.0]);
    key(&mut h, Key::Escape);
    let g = editor(&h).model_lines[0].guid;
    assert_eq!(editor(&h).only_selected(), None, "let go");
    click_family(&mut h, [-500.0, -800.0, 0.0]);
    assert_eq!(editor(&h).only_selected(), Some(Picked::ModelLine(g)), "picked by a click on it");
    assert!(h.query_all_by_label("Modify | Lines").count() > 0, "its contextual tab");
    key(&mut h, Key::Delete);
    assert!(editor(&h).model_lines.is_empty(), "taken away: {}", status(&h));
    h.state_mut().family_undo();
    h.run();
    assert_eq!(editor(&h).model_lines.len(), 1, "back with Undo");
    h.state_mut().family_redo();
    h.run();
    assert!(editor(&h).model_lines.is_empty(), "gone again with Redo");
}

#[test]
fn a_model_lines_subcategory_is_set_in_properties() {
    let mut h = app("");
    h.set_size(egui::vec2(1600.0, 1200.0));
    h.state_mut().new_family(bimcraft_core::family::Category::Door);
    h.run();
    view(&mut h, Some(Which::Plan));
    draw(&mut h, [-1000.0, -800.0, 0.0], [1000.0, -800.0, 0.0]);
    let g = editor(&h).model_lines[0].guid;
    h.state_mut().active_family_mut().unwrap().select_only(Picked::ModelLine(g));
    h.run();
    assert_eq!(editor(&h).model_lines[0].subcategory, None, "the category's own to begin with");
    assert!(h.query_by_label("Subcategory").is_some(), "its subcategory");
    h.get_all_by_role(egui::accesskit::Role::ComboBox)
        .find(|n| n.value().is_some_and(|v| v.starts_with('<')))
        .expect("the subcategory list")
        .click_accesskit();
    h.run();
    press(&mut h, "Opening");
    assert_eq!(editor(&h).model_lines[0].subcategory.as_deref(), Some("Opening"), "a door's subcategory");
}

#[test]
fn a_model_line_drawn_on_a_plane_goes_with_it_when_the_plane_is_dragged() {
    // Edge stands at x = 1000; a model line drawn on it, rising from Ref. Level.
    let script = family_with("").replace(
        r#""ref_lines": ["#,
        &format!(
            r#""model_lines": [{{"start": [1000, -500, 0], "end": [1000, -500, 600], "host": {{"plane": "{EDGE}"}}}}], "ref_lines": ["#
        ),
    );
    let mut h = plan_of(&script);
    assert_eq!(editor(&h).model_lines.len(), 1);
    drag(&mut h, [1000.0, 0.0, 0.0], [200.0, 0.0, 0.0], Modifiers::NONE);
    let l = &editor(&h).model_lines[0];
    assert!((l.start[0] - 1200.0).abs() < 1.0 && (l.end[0] - 1200.0).abs() < 1.0, "carried with Edge: {l:?} {}", status(&h));
}

#[test]
fn model_line_is_a_family_command_with_no_key_yet() {
    assert!(crate::editor::keys::FAMILY_KEYLESS.contains(&"Model Line"));
}

/// The Model panel, a model line just drawn in plan, selected, and one in the Front elevation, saved where
/// `BIMCRAFT_FE15_DUMP` says (only then).
#[test]
fn family_model_line_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE15_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe15-{name}.png"))).unwrap();
    };
    let mut h = app(BOX);
    h.set_size(egui::vec2(1920.0, 1000.0));
    h.state_mut().edit_family("Box");
    h.run();
    view(&mut h, Some(Which::Plan));
    save(&mut h, "ribbon");
    model_line(&mut h);
    click_family(&mut h, [-1000.0, -800.0, 0.0]);
    click_family(&mut h, [1000.0, -800.0, 0.0]);
    save(&mut h, "plan-drawn");
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    view(&mut h, Some(Which::Elevation(Side::Front)));
    draw(&mut h, [-300.0, 0.0, 200.0], [300.0, 0.0, 900.0]);
    key(&mut h, Key::Escape);
    save(&mut h, "front");
}

/// The grip shown nearest model point `p` in the view, within 3 px.
fn grip_at(h: &Harness<BimApp>, p: [f64; 3]) -> Option<egui::Pos2> {
    let want = editor(h).views.screen_of(p)?;
    editor(h).views.grips_shown.iter().map(|(_, at)| *at).find(|at| at.distance(want) < 3.0)
}

#[test]
fn a_selected_model_lines_end_grip_stretches_it_on_its_host_as_one_step() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Plan));
    draw(&mut h, [-1000.0, -800.0, 0.0], [1000.0, -800.0, 0.0]);
    let was = editor(&h).model_lines[0].clone();
    assert!(grip_at(&h, was.start).is_some() && grip_at(&h, was.end).is_some(), "a grip at each end: {:?}", editor(&h).views.grips_shown);
    let steps = editor(&h).steps();
    let grip = grip_at(&h, was.end).unwrap();
    drag_screen(&mut h, grip, grip + vec2(40.0, -30.0), PointerButton::Primary);
    let now = editor(&h).model_lines[0].clone();
    assert_eq!(now.start, was.start, "its start stays");
    assert!(now.end[0] > was.end[0] + 20.0 && now.end[1] > was.end[1] + 20.0, "its end follows the grip: {now:?}");
    assert!(now.end[2].abs() < 1e-6, "still on Ref. Level: {now:?}");
    assert_eq!(editor(&h).steps(), steps + 1, "one step");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(editor(&h).model_lines[0].end, was.end, "Undo puts it back");
}

#[test]
fn a_model_lines_end_grip_in_an_elevation_keeps_it_on_its_plane() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Elevation(Side::Front)));
    draw(&mut h, [-300.0, 0.0, 200.0], [300.0, 0.0, 900.0]);
    let was = editor(&h).model_lines[0].clone();
    let grip = grip_at(&h, was.start).expect("its start's grip");
    drag_screen(&mut h, grip, grip + vec2(-30.0, 30.0), PointerButton::Primary);
    let now = editor(&h).model_lines[0].clone();
    assert_eq!(now.end, was.end, "its end stays");
    assert!(now.start[0] < was.start[0] - 20.0 && now.start[2] < was.start[2] - 20.0, "{now:?}");
    assert!(now.start[1].abs() < 1e-6, "still on Center (Front/Back): {now:?}");
}

#[test]
fn a_pinned_model_line_shows_no_grips() {
    let (mut h, _) = open(BOX, "Box");
    view(&mut h, Some(Which::Plan));
    draw(&mut h, [-1000.0, -800.0, 0.0], [1000.0, -800.0, 0.0]);
    let l = editor(&h).model_lines[0].clone();
    press(&mut h, "Pin");
    assert!(editor(&h).model_lines[0].pinned, "pinned");
    assert!(grip_at(&h, l.start).is_none() && grip_at(&h, l.end).is_none(), "no grips: {:?}", editor(&h).views.grips_shown);
}
