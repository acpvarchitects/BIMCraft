//! FE-16b: Cut Geometry (GU), Uncut Geometry, Join Geometry (JO), Unjoin Geometry and Switch Join Order in a family's views.
//! Two picks each: the form to cut or join, then the form that cuts it (a void, which no view draws, by its row in the forms
//! list). GU and JO keep their project meaning on a project's tab; the other three have no key. Each pair is one step of the
//! history; Esc lets go; a refusal of the core is said in the status bar.
use super::family_drag_move::{family, status};
use super::family_modify_3d::{click3, in_3d};
use super::family_views::{click_family, editor, open, view};
use super::*;
use bimcraft_core::family::views::{Side, Which};
use bimcraft_core::ids::Guid;

const A: &str = "fe16b000-0000-4000-8000-00000000000a";
const B: &str = "fe16b000-0000-4000-8000-00000000000b";
const V: &str = "fe16b000-0000-4000-8000-0000000000c0";

/// Solid A (east, 100 to 500 square, 300 high), solid B (west of it, -1000 to -600), and a void V over A, listed after both,
/// which cuts them by the list's order.
pub(super) fn cutters() -> String {
    let sq = |x0: i32, x1: i32| format!("[[{x0}, 100], [{x1}, 100], [{x1}, 500], [{x0}, 500]]");
    format!(
        r#"family {{"name": "Movers", "category": "generic", "types": {{"T": {{}}}}, "forms": [{{"guid": "{A}", "profile": {}, "from": 0, "to": 300}}, {{"guid": "{B}", "profile": {}, "from": 0, "to": 300}}, {{"guid": "{V}", "void": true, "profile": {}, "from": 0, "to": 200}}]}}"#,
        sq(100, 500),
        sq(-1000, -600),
        sq(200, 400),
    )
}

fn plan() -> Harness<'static, BimApp> {
    let (mut h, _) = open(&cutters(), "Movers");
    view(&mut h, Some(Which::Plan));
    h
}

const ON_A: [f64; 3] = [300.0, 300.0, 0.0];
const ON_B: [f64; 3] = [-800.0, 300.0, 0.0];
const EMPTY: [f64; 3] = [1500.0, 300.0, 0.0];

fn guid(s: &str) -> Guid {
    Guid::parse(s).unwrap()
}

/// The row of the forms list for form `n` (from 1), clicked.
fn row(h: &mut Harness<BimApp>, n: usize, what: &str) {
    let label = format!("{n}. {what}");
    h.get_by_label(&label).scroll_to_me();
    h.run();
    press(h, &label);
}

fn steps(h: &Harness<BimApp>) -> usize {
    editor(h).steps()
}

/// A command with no key, as the ribbon's dropdown runs it.
fn run(h: &mut Harness<BimApp>, action: &str) {
    assert!(h.state_mut().family_action(action), "{action}");
    h.run();
}

fn prompt(h: &Harness<BimApp>) -> String {
    h.state().prompt().to_string()
}

fn mode(h: &Harness<BimApp>) -> Option<crate::editor::cutting::CutTool> {
    editor(h).views.cutting.map(|c| c.mode)
}

#[test]
fn cu_cuts_a_solid_by_a_solid_and_a_void_listed_after_it_still_cuts_it_one_step() {
    let mut h = plan();
    assert_eq!(family(&h).cutters(0), vec![2], "the void listed after A cuts it by the order");
    shortcut(&mut h, "GU");
    assert_eq!(mode(&h), Some(crate::editor::cutting::CutTool::Cut));
    assert!(prompt(&h).contains("form to cut"), "{}", prompt(&h));
    click_family(&mut h, ON_A);
    assert!(prompt(&h).contains("that cuts it"), "{}", prompt(&h));
    assert_eq!(editor(&h).selection.items().len(), 1, "the first form shows as picked");
    click_family(&mut h, ON_B);
    let f = family(&h);
    assert_eq!(f.forms[0].cut_by, vec![guid(B)], "{}", status(&h));
    assert_eq!(f.cutters(0), vec![2, 1], "the void and B");
    assert_eq!(steps(&h), 1);
    assert!(status(&h).contains("Form 2 now cuts form 1"), "{}", status(&h));
    // The tool stays for the next pair; nothing is picked, selected or left over.
    assert_eq!(editor(&h).views.cutting.and_then(|c| c.first), None);
    assert!(editor(&h).selection.is_empty());
    // One undo takes it back.
    h.state_mut().family_undo();
    h.run();
    assert!(family(&h).forms[0].cut_by.is_empty());
}

#[test]
fn uncut_takes_a_solid_and_a_void_back_and_cut_gives_the_void_its_cut_again() {
    let mut h = plan();
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    run(&mut h, "Uncut Geometry");
    assert!(prompt(&h).starts_with("Uncut Geometry"), "{}", prompt(&h));
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    assert!(family(&h).forms[0].cut_by.is_empty(), "{}", status(&h));
    assert_eq!(family(&h).cutters(0), vec![2]);
    assert_eq!(steps(&h), 2);
    // The void: the views never draw it, its row picks it.
    click_family(&mut h, ON_A);
    row(&mut h, 3, "Extrusion (void)");
    assert_eq!(family(&h).forms[0].uncut_by, vec![guid(V)], "{}", status(&h));
    assert!(family(&h).cutters(0).is_empty());
    assert_eq!(steps(&h), 3);
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_A);
    row(&mut h, 3, "Extrusion (void)");
    assert!(family(&h).forms[0].uncut_by.is_empty(), "{}", status(&h));
    assert_eq!(family(&h).cutters(0), vec![2]);
    assert_eq!(steps(&h), 4);
}

#[test]
fn jo_joins_unjoin_takes_it_back_and_switch_join_order_turns_it_round() {
    let mut h = plan();
    shortcut(&mut h, "JO");
    assert_eq!(mode(&h), Some(crate::editor::cutting::CutTool::Join));
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    assert_eq!(family(&h).forms[0].joined_by, vec![guid(B)], "{}", status(&h));
    assert_eq!(steps(&h), 1);
    run(&mut h, "Switch Join Order");
    assert!(prompt(&h).starts_with("Switch Join Order"), "{}", prompt(&h));
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    let f = family(&h);
    assert!(f.forms[0].joined_by.is_empty() && f.forms[1].joined_by == vec![guid(A)], "{}", status(&h));
    assert_eq!(steps(&h), 2);
    // Either order of the pair unjoins.
    run(&mut h, "Unjoin Geometry");
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    let f = family(&h);
    assert!(f.forms[0].joined_by.is_empty() && f.forms[1].joined_by.is_empty(), "{}", status(&h));
    assert_eq!(steps(&h), 3);
}

#[test]
fn esc_lets_the_first_form_go_then_puts_the_tool_down_and_changes_nothing() {
    let mut h = plan();
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_A);
    assert!(editor(&h).views.cutting.and_then(|c| c.first).is_some());
    key(&mut h, Key::Escape);
    assert_eq!(editor(&h).views.cutting.and_then(|c| c.first), None);
    assert!(mode(&h).is_some(), "the tool stays");
    assert!(editor(&h).selection.is_empty());
    key(&mut h, Key::Escape);
    assert_eq!(mode(&h), None, "the second Esc puts it down");
    assert_eq!(steps(&h), 0);
    assert!(family(&h).forms.iter().all(|f| f.cut_by.is_empty() && f.joined_by.is_empty()));
}

#[test]
fn a_refusal_of_the_core_is_said_the_family_left_as_it_was_and_the_pair_begins_again() {
    let mut h = plan();
    shortcut(&mut h, "JO");
    // A void can't be joined: the core's reason, in the status bar.
    click_family(&mut h, ON_A);
    row(&mut h, 3, "Extrusion (void)");
    assert!(h.state().status.error && status(&h).contains("is a void"), "{}", status(&h));
    assert_eq!(steps(&h), 0);
    assert!(family(&h).forms[0].joined_by.is_empty());
    assert_eq!(editor(&h).views.cutting.and_then(|c| c.first), None);
    // The same form twice, and a click on nothing, are said too, the first form kept.
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_A);
    assert!(h.state().status.error && status(&h).contains("first form"), "{}", status(&h));
    assert!(editor(&h).views.cutting.and_then(|c| c.first).is_some());
    click_family(&mut h, EMPTY);
    assert!(h.state().status.error && status(&h).contains("Click a form"), "{}", status(&h));
    // A pair that was never joined.
    run(&mut h, "Unjoin Geometry");
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    assert!(h.state().status.error && status(&h).contains("aren't joined"), "{}", status(&h));
    assert_eq!(steps(&h), 0);
}

#[test]
fn a_form_with_no_guid_is_given_one_only_when_the_pair_works_so_a_pair_is_one_step_and_a_refusal_none() {
    let mut h = plan();
    let ed = h.state_mut().active_family_mut().unwrap();
    ed.forget_guid(0);
    h.run();
    let before = steps(&h);
    // A refusal (the void can't be joined) leaves it without one, and no step.
    shortcut(&mut h, "JO");
    click_family(&mut h, ON_A);
    assert_eq!(steps(&h), before, "the first pick is no step");
    row(&mut h, 3, "Extrusion (void)");
    assert!(h.state().status.error, "{}", status(&h));
    assert_eq!(steps(&h), before);
    assert!(editor(&h).draft_family().unwrap().forms[0].guid.is_nil(), "left as it was");
    // A pair that works: the GUID and the cut, one step.
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_A);
    assert_eq!(steps(&h), before);
    click_family(&mut h, ON_B);
    let f = family(&h);
    assert!(!f.forms[0].guid.is_nil(), "given a GUID");
    assert_eq!(f.forms[0].cut_by, vec![guid(B)], "{}", status(&h));
    assert_eq!(steps(&h), before + 1);
}

#[test]
fn a_pair_undone_and_redone_comes_and_goes_and_an_undo_with_the_first_form_held_goes_on() {
    let mut h = plan();
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    h.state_mut().family_undo();
    h.run();
    assert!(family(&h).forms[0].cut_by.is_empty());
    h.state_mut().family_redo();
    h.run();
    assert_eq!(family(&h).forms[0].cut_by, vec![guid(B)]);
    h.state_mut().family_undo();
    h.run();
    // The first form held through an undo: the form is still there, the pair goes on.
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_A);
    h.state_mut().family_undo();
    h.run();
    assert!(editor(&h).views.cutting.is_some());
    click_family(&mut h, ON_B);
    assert!(family(&h).forms.iter().all(|f| f.cut_by.len() <= 1));
}

#[test]
fn the_first_form_gone_before_the_second_pick_is_said_and_nothing_is_cut() {
    let mut h = plan();
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_A);
    h.state_mut().active_family_mut().unwrap().remove_form(0);
    h.run();
    let before = steps(&h);
    row(&mut h, 1, "Extrusion");
    assert!(h.state().status.error && status(&h).contains("gone"), "{}", status(&h));
    assert_eq!(steps(&h), before);
    assert_eq!(editor(&h).views.cutting.and_then(|c| c.first), None);
}

#[test]
fn a_solid_listed_before_its_target_cuts_it_through_cut_by_beside_the_void_that_cuts_it() {
    // A (listed first) cuts B (listed second): the cutter before the target, and the void after both cuts B too.
    let mut h = plan();
    shortcut(&mut h, "GU");
    click_family(&mut h, ON_B);
    click_family(&mut h, ON_A);
    let f = family(&h);
    assert_eq!(f.forms[1].cut_by, vec![guid(A)], "{}", status(&h));
    assert_eq!(f.cutters(1), vec![2, 0], "the void and A");
    // Uncut by the void (its row), then by the solid: the lists come clean.
    run(&mut h, "Uncut Geometry");
    click_family(&mut h, ON_B);
    row(&mut h, 3, "Extrusion (void)");
    assert_eq!(family(&h).forms[1].uncut_by, vec![guid(V)]);
    assert_eq!(family(&h).cutters(1), vec![0]);
    click_family(&mut h, ON_B);
    click_family(&mut h, ON_A);
    let f = family(&h);
    assert!(f.forms[1].cut_by.is_empty(), "{}", status(&h));
    assert!(f.cutters(1).is_empty());
}

#[test]
fn the_tools_work_in_an_elevation_and_in_3d() {
    let mut h = plan();
    view(&mut h, Some(Which::Elevation(Side::Front)));
    shortcut(&mut h, "GU");
    // Through the hole V cuts in A (up to 200), which the elevation draws clear: nothing there to take, the tool still in hand
    // (GEO-2c-2e). Above the hole, A.
    click_family(&mut h, [300.0, 300.0, 150.0]);
    assert!(mode(&h).is_some() && editor(&h).views.cutting.and_then(|c| c.first).is_none(), "{}", status(&h));
    click_family(&mut h, [300.0, 300.0, 250.0]);
    click_family(&mut h, [-800.0, 300.0, 150.0]);
    assert_eq!(family(&h).forms[0].cut_by, vec![guid(B)], "{}", status(&h));
    // {3D}: the tool stays in hand across the view change.
    view(&mut h, None);
    for _ in 0..4 {
        h.run();
    }
    assert!(mode(&h).is_some());
    run(&mut h, "Uncut Geometry");
    click3(&mut h, [300.0, 300.0, 300.0]);
    click3(&mut h, [-800.0, 300.0, 300.0]);
    assert!(family(&h).forms[0].cut_by.is_empty(), "{}", status(&h));
    let mut h = in_3d();
    shortcut(&mut h, "JO");
    click3(&mut h, [300.0, 300.0, 300.0]);
    click3(&mut h, [-800.0, 300.0, 300.0]);
    assert_eq!(family(&h).forms[0].joined_by.len(), 1, "{}", status(&h));
}

#[test]
fn the_geometry_panels_buttons_drop_down_the_tools() {
    let mut h = plan();
    h.set_size(egui::vec2(3200.0, 1000.0));
    h.run();
    topmost(&mut h, "Modify");
    press(&mut h, "Cut");
    press(&mut h, "Uncut Geometry");
    assert_eq!(mode(&h), Some(crate::editor::cutting::CutTool::Uncut));
    press(&mut h, "Join Geometry");
    press(&mut h, "Unjoin Geometry");
    assert_eq!(mode(&h), Some(crate::editor::cutting::CutTool::Unjoin));
    press(&mut h, "Join Geometry");
    press(&mut h, "Switch Join Order");
    assert_eq!(mode(&h), Some(crate::editor::cutting::CutTool::Switch));
}

#[test]
fn keys_gu_and_jo_are_the_familys_in_the_family_editor_and_the_projects_in_the_project() {
    use crate::editor::keys::{FAMILY_KEYLESS, FAMILY_SHORTCUTS, in_family};
    assert!(FAMILY_SHORTCUTS.contains(&("GU", "Cut Geometry")) && FAMILY_SHORTCUTS.contains(&("JO", "Join Geometry")));
    for action in ["Uncut Geometry", "Unjoin Geometry", "Switch Join Order"] {
        assert!(FAMILY_KEYLESS.contains(&action) && in_family(action), "{action}");
        assert!(FAMILY_SHORTCUTS.iter().all(|(_, a)| *a != action), "{action} has no key");
    }
    // The project's tab: the project's tools.
    let mut h = app("");
    shortcut(&mut h, "GU");
    assert!(matches!(h.state().tool, Tool::CutGeometry { uncut: false, .. }), "{:?}", h.state().tool);
    shortcut(&mut h, "JO");
    assert!(matches!(h.state().tool, Tool::JoinGeometry { .. }), "{:?}", h.state().tool);
    // A family's tab: the family's, and the project's tool untouched.
    let mut h = plan();
    let before = h.state().tool.clone();
    shortcut(&mut h, "GU");
    assert_eq!(mode(&h), Some(crate::editor::cutting::CutTool::Cut));
    shortcut(&mut h, "JO");
    assert_eq!(mode(&h), Some(crate::editor::cutting::CutTool::Join));
    assert_eq!(h.state().tool, before);
}

#[test]
fn an_adaptive_family_and_a_sketch_refuse_the_tools_saying_why() {
    let mut h = plan();
    editor_sketch(&mut h);
    shortcut(&mut h, "GU");
    assert!(mode(&h).is_none() && status(&h).contains("sketch"), "{}", status(&h));
}

fn editor_sketch(h: &mut Harness<BimApp>) {
    h.state_mut().active_family_mut().unwrap().edit_form_sketch(0).unwrap();
    h.run();
}

/// A (100 to 500 square, 300 high) and B (300 to 700 square, 450 high) overlapping it by 200 in both ways, no void; or, with
/// `apart`, B west of A and clear of it.
fn two_solids(apart: bool) -> String {
    let (x0, y0) = if apart { (-1000, 100) } else { (300, 300) };
    format!(
        r#"family {{"name": "Movers", "category": "generic", "types": {{"T": {{}}}}, "forms": [{{"guid": "{A}", "profile": [[100, 100], [500, 100], [500, 500], [100, 500]], "from": 0, "to": 300}}, {{"guid": "{B}", "profile": [[{x0}, {y0}], [{}, {y0}], [{}, {}], [{x0}, {}]], "from": 0, "to": 450}}]}}"#,
        x0 + 400,
        x0 + 400,
        y0 + 400,
        y0 + 400
    )
}

/// The tool in use, saved where `BIMCRAFT_FE16B_DUMP` says (only then): the panel, picks and a refusal in plan on the void
/// family; its void's notch in {3D} cut and uncut; overlapping solids joined in plan and {3D}; and solids that miss each other
/// joined, which leaves both whole.
#[test]
fn family_cut_join_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE16B_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let opened = |script: &str| {
        let mut h = app(script);
        h.set_size(egui::vec2(1800.0, 1000.0));
        h.state_mut().edit_family("Movers");
        h.run();
        view(&mut h, Some(Which::Plan));
        topmost(&mut h, "Modify");
        h
    };
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe16b-{name}.png"))).unwrap();
    };
    let to_3d = |h: &mut Harness<BimApp>| {
        view(h, None);
        for _ in 0..4 {
            h.run();
        }
    };
    // The void family: the panel, a refusal, and the void's notch cut and uncut.
    let mut h = opened(&cutters());
    press(&mut h, "Cut");
    save(&mut h, "geometry-panel");
    press(&mut h, "Cut Geometry");
    click_family(&mut h, ON_A);
    row(&mut h, 3, "Extrusion (void)");
    to_3d(&mut h);
    save(&mut h, "3d-void-cut");
    run(&mut h, "Uncut Geometry");
    click3(&mut h, [300.0, 300.0, 300.0]);
    row(&mut h, 3, "Extrusion (void)");
    save(&mut h, "3d-void-uncut");
    view(&mut h, Some(Which::Plan));
    shortcut(&mut h, "JO");
    click_family(&mut h, ON_A);
    row(&mut h, 3, "Extrusion (void)");
    save(&mut h, "plan-refusal");
    // Overlapping solids, joined.
    let mut h = opened(&two_solids(false));
    shortcut(&mut h, "JO");
    click_family(&mut h, [200.0, 200.0, 0.0]);
    save(&mut h, "plan-first-pick");
    click_family(&mut h, [650.0, 650.0, 0.0]);
    save(&mut h, "plan-joined-overlap");
    to_3d(&mut h);
    save(&mut h, "3d-joined-overlap");
    // Solids that miss each other, joined: both whole.
    let mut h = opened(&two_solids(true));
    shortcut(&mut h, "JO");
    click_family(&mut h, ON_A);
    click_family(&mut h, ON_B);
    to_3d(&mut h);
    save(&mut h, "3d-joined-apart");
}

/// A slab with a lock holding its end to a plane, a form named by GUID in a dimension, as FE-25's handles test has it.
const LOCKED: &str = r#"family {"name": "Slab", "category": "generic", "types": {"T": {}}, "ref_planes": [{"guid": "fe16b111-0000-4000-8000-000000000001", "name": "Top", "start": [-2000, 0, 300], "end": [2000, 0, 300], "up": [0, 1, 0]}], "forms": [{"guid": "fe16b111-0000-4000-8000-00000000000a", "profile": [[0, 0], [400, 0], [400, 400], [0, 400]], "from": 0, "to": 300}, {"guid": "fe16b111-0000-4000-8000-00000000000b", "profile": [[-1000, 0], [-600, 0], [-600, 400], [-1000, 400]], "from": 0, "to": 300}], "dimensions": [{"refs": [{"plane": "fe16b111-0000-4000-8000-000000000001"}, {"form": "fe16b111-0000-4000-8000-00000000000a", "end": "end"}], "view": "front", "at": [600, 0, 300], "locked": [{"segment": 0, "length": 0}], "alignment": true}]}"#;

#[test]
fn a_family_with_a_dimension_naming_a_form_stages_in_3d_with_or_without_a_cut_or_join() {
    let (mut h, _) = open(LOCKED, "Slab");
    view(&mut h, None);
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(editor(&h).problem, None);
    assert!(editor(&h).scene3d.is_some(), "{{3D}} is not blank");
    // With a cut and then a join applied.
    run(&mut h, "Cut Geometry");
    click3(&mut h, [200.0, 200.0, 300.0]);
    click3(&mut h, [-800.0, 200.0, 300.0]);
    assert_eq!(family(&h).forms[0].cut_by.len(), 1, "{}", status(&h));
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(editor(&h).problem, None);
    assert!(editor(&h).scene3d.is_some());
    run(&mut h, "Uncut Geometry");
    click3(&mut h, [200.0, 200.0, 300.0]);
    click3(&mut h, [-800.0, 200.0, 300.0]);
    run(&mut h, "Join Geometry");
    click3(&mut h, [200.0, 200.0, 300.0]);
    click3(&mut h, [-800.0, 200.0, 300.0]);
    assert_eq!(family(&h).forms[0].joined_by.len(), 1, "{}", status(&h));
    for _ in 0..4 {
        h.run();
    }
    assert_eq!(editor(&h).problem, None);
    assert!(editor(&h).scene3d.is_some());
}
