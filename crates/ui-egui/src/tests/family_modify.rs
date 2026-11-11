//! FE-8: the Modify panel's tools in a family that isn't adaptive. Move, Copy, Rotate, Mirror (Pick and Draw Axis), Scale and
//! Offset on what is selected in a plan, by clicks and the Options Bar, each one step of the history; what can't be done says
//! why (a form for Scale, a sketch's tool outside a sketch, a constraint asked about as "Constraints are not satisfied"). In
//! sketch mode the same on the lines picked, with Trim/Extend Single Element and Split Element.
use super::family_drag_move::{A, A_LINE, CENTER_LR, EDGE, ed, family, family_with, g, near, pick, plan, plan_of, points, status};
use super::family_views::editor;
use super::*;
use crate::editor::modify::Tool as M;
use crate::editor::select_box::Picked;
use bimcraft_core::family::views::Which;

/// A click at family point `p` (on Ref. Level) in the view shown.
fn click(h: &mut Harness<BimApp>, p: [f64; 2]) {
    let at = editor(h).views.screen_of([p[0], p[1], 0.0]).expect("a view shown");
    click_at(h, at);
}

fn tool(h: &Harness<BimApp>) -> Option<M> {
    editor(h).modify_in_hand()
}

fn plane_x(h: &Harness<BimApp>, guid: &str) -> f64 {
    family(h).planes.iter().find(|p| p.guid == g(guid)).map(|p| p.start[0]).expect("the plane")
}

fn same(a: [f64; 2], b: [f64; 2]) -> bool {
    near(a[0], b[0]) && near(a[1], b[1])
}

#[test]
fn move_by_two_clicks_moves_the_selection_as_one_step_and_puts_the_tool_down() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    let before = points(&h, 0);
    shortcut(&mut h, "MV");
    assert_eq!(tool(&h), Some(M::Move { copy: false, from: None }), "{}", status(&h));
    assert!(h.state().prompt().contains("point to move from"), "{}", h.state().prompt());
    click(&mut h, [300.0, 300.0]);
    assert!(h.state().prompt().contains("point to move to"), "{}", h.state().prompt());
    click(&mut h, [600.0, 500.0]);
    let after = points(&h, 0);
    for (a, b) in before.iter().zip(&after) {
        assert!(same(*b, [a[0] + 300.0, a[1] + 200.0]), "{a:?} → {b:?} {}", status(&h));
    }
    assert_eq!(editor(&h).steps(), 1);
    assert_eq!(tool(&h), None);
    assert_eq!(editor(&h).selection.items(), &[Picked::Form(0)]);
    h.state_mut().family_undo();
    h.run();
    assert_eq!(points(&h, 0), before);
}

#[test]
fn what_temporary_hide_hides_isnt_moved_or_rotated() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(1)]);
    shortcut(&mut h, "HH");
    pick(&mut h, &[Picked::Form(0), Picked::Form(1)]);
    let (a, b) = (points(&h, 0), points(&h, 1));
    shortcut(&mut h, "MV");
    click(&mut h, [300.0, 300.0]);
    click(&mut h, [300.0, 800.0]);
    assert!(same(points(&h, 0)[0], [a[0][0], a[0][1] + 500.0]), "{:?} {}", points(&h, 0), status(&h));
    assert_eq!(points(&h, 1), b, "the hidden box stays");
    shortcut(&mut h, "RO");
    click(&mut h, [700.0, 800.0]);
    click(&mut h, [300.0, 1200.0]);
    assert_eq!(points(&h, 1), b, "the hidden box stays");
    assert_eq!(editor(&h).steps(), 2, "{}", status(&h));
}

#[test]
fn copy_with_multiple_makes_a_copy_at_each_click_and_escape_steps_back() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    let n = family(&h).forms.len();
    shortcut(&mut h, "CO");
    ed(&mut h).views.modify_options.multiple = true;
    click(&mut h, [300.0, 300.0]);
    click(&mut h, [300.0, 1300.0]);
    click(&mut h, [300.0, 2300.0]);
    let f = family(&h);
    assert_eq!(f.forms.len(), n + 2, "{}", status(&h));
    // Each copy a step further, the original where it was.
    assert_eq!(points(&h, 0)[0], [100.0, 100.0]);
    assert!(same(points(&h, 1)[0], [100.0, 1100.0]), "{:?}", points(&h, 1));
    assert!(same(points(&h, 2)[0], [100.0, 2100.0]), "{:?}", points(&h, 2));
    assert_eq!(editor(&h).steps(), 2);
    // Still copying; Escape lets the start go, then the tool.
    assert!(matches!(tool(&h), Some(M::Move { copy: true, from: Some(_) })));
    key(&mut h, Key::Escape);
    assert_eq!(tool(&h), Some(M::Move { copy: true, from: None }));
    key(&mut h, Key::Escape);
    assert_eq!(tool(&h), None);
}

#[test]
fn rotate_turns_the_selection_about_its_centre_by_two_clicks_or_a_typed_angle() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    let before = points(&h, 0);
    shortcut(&mut h, "RO");
    // Its centre found as the view is drawn: the middle of the box.
    match tool(&h) {
        Some(M::Rotate { centre: Some(c), .. }) => assert!(near(c.0, 300.0) && near(c.1, 300.0), "{c:?}"),
        t => panic!("{t:?}"),
    }
    click(&mut h, [700.0, 300.0]);
    click(&mut h, [300.0, 700.0]);
    // A quarter turn counter-clockwise: (x, y) to (600 − y, x).
    for (a, b) in before.iter().zip(points(&h, 0)) {
        assert!(same(b, [600.0 - a[1], a[0]]), "{a:?} → {b:?} {}", status(&h));
    }
    assert_eq!(editor(&h).steps(), 1);
    assert_eq!(tool(&h), None);
    // Typed: 90 more on the Options Bar, Enter.
    shortcut(&mut h, "RO");
    type_into(&mut h, "Angle:", "90");
    key(&mut h, Key::Enter);
    h.run();
    for (a, b) in before.iter().zip(points(&h, 0)) {
        assert!(same(b, [600.0 - a[0], 600.0 - a[1]]), "{a:?} → {b:?} {}", status(&h));
    }
    assert_eq!(editor(&h).steps(), 2);
}

#[test]
fn mirror_pick_axis_copies_across_a_plane_and_draw_axis_mirrors_in_place() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    let before = points(&h, 0);
    let n = family(&h).forms.len();
    shortcut(&mut h, "MM");
    assert!(h.state().prompt().contains("Pick Axis"), "{}", h.state().prompt());
    // Edge, at x 1000: a mirrored copy (Copy is on, as Revit's), selected.
    click(&mut h, [1000.0, 0.0]);
    assert_eq!(family(&h).forms.len(), n + 1, "{}", status(&h));
    for (a, b) in before.iter().zip(points(&h, 1)) {
        assert!(same(b, [2000.0 - a[0], a[1]]), "{a:?} → {b:?}");
    }
    assert_eq!(points(&h, 0), before);
    assert_eq!(editor(&h).selection.items(), &[Picked::Form(1)]);
    // Draw Axis with Copy off: the copy mirrored back across x 1000 in place.
    shortcut(&mut h, "DM");
    ed(&mut h).views.modify_options.mirror_copy = false;
    click(&mut h, [1000.0, -100.0]);
    click(&mut h, [1000.0, 100.0]);
    assert_eq!(family(&h).forms.len(), n + 1);
    for (a, b) in before.iter().zip(points(&h, 1)) {
        assert!(same(b, [a[0], a[1]]) || before.iter().any(|q| same(b, *q)), "{a:?} → {b:?} {}", status(&h));
    }
    assert_eq!(editor(&h).steps(), 2);
}

#[test]
fn scale_is_greyed_for_a_form_saying_why_and_scales_a_plane_by_a_factor() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "RE");
    assert_eq!(tool(&h), None);
    assert!(status(&h).contains("a form is sized by its dimensions"), "{}", status(&h));
    assert!(h.state().status.error);
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    shortcut(&mut h, "RE");
    assert!(matches!(tool(&h), Some(M::Scale { .. })), "{}", status(&h));
    {
        let o = &mut ed(&mut h).views.modify_options;
        o.numerical = true;
        o.factor = "1.5".into();
    }
    click(&mut h, [0.0, 0.0]);
    assert!(near(plane_x(&h, EDGE), 1500.0), "{} {}", plane_x(&h, EDGE), status(&h));
    assert_eq!(editor(&h).steps(), 1);
}

#[test]
fn offset_copies_a_plane_toward_the_side_clicked_and_refuses_a_form() {
    let mut h = plan();
    shortcut(&mut h, "OF");
    assert_eq!(tool(&h), Some(M::Offset));
    ed(&mut h).views.modify_options.offset = "250".into();
    // Just west of Edge's trace: a copy 250 west of it.
    let (a, b) = (editor(&h).views.screen_of([1000.0, 0.0, 0.0]).unwrap(), editor(&h).views.screen_of([1100.0, 0.0, 0.0]).unwrap());
    // Clear of Center (Front/Back), whose trace runs along y 0.
    let up = editor(&h).views.screen_of([1000.0, 600.0, 0.0]).unwrap() - a;
    let west = a + up - (b - a).normalized() * 2.0;
    click_at(&mut h, west);
    let xs: Vec<f64> = family(&h).planes.iter().map(|p| p.start[0]).collect();
    assert!(xs.iter().any(|x| near(*x, 1000.0)) && xs.iter().any(|x| near(*x, 750.0)), "{xs:?} {}", status(&h));
    assert_eq!(editor(&h).steps(), 1);
    // A form says what Offset takes.
    click(&mut h, [300.0, 300.0]);
    assert!(status(&h).contains("Offset takes reference planes and reference lines"), "{}", status(&h));
}

#[test]
fn trim_split_and_the_selection_tools_with_nothing_selected_say_why_outside_a_sketch() {
    let mut h = plan();
    for (keys, said) in [
        ("SL", "works on a sketch's lines"),
        ("TR", "works on a sketch's lines"),
        ("MV", "Select what Move takes first"),
        ("RO", "Select what Rotate takes first"),
    ] {
        h.state_mut().status = crate::Status::default();
        shortcut(&mut h, keys);
        assert!(status(&h).contains(said), "{keys}: {}", status(&h));
        assert!(h.state().status.error);
        assert_eq!(tool(&h), None);
    }
    // In {3D}: a plan or an elevation.
    super::family_views::view(&mut h, None);
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "MV");
    assert!(status(&h).contains("plan or an elevation"), "{}", status(&h));
}

#[test]
fn rotating_a_form_a_label_ties_to_a_datum_asks_and_remove_constraints_does_it() {
    // Len (100) from Center (Left/Right) to the box's west side.
    let dim = format!(
        r#", {{"refs": [{{"plane": "{CENTER_LR}"}}, {{"form": "{A}", "line": "{A_LINE}3"}}], "view": "plan", "at": [50, 0, 0], "label": "Len"}}"#
    );
    let mut h = plan_of(&family_with(&dim));
    pick(&mut h, &[Picked::Form(0)]);
    let before = points(&h, 0);
    shortcut(&mut h, "RO");
    click(&mut h, [700.0, 300.0]);
    click(&mut h, [300.0, 700.0]);
    // Nothing turned yet: asked.
    assert_eq!(points(&h, 0), before);
    let asked = editor(&h).views.unsatisfied.clone().expect("Constraints are not satisfied");
    assert!(asked.message.contains("ties what is rotated to what isn't"), "{}", asked.message);
    press(&mut h, "Remove Constraints");
    h.run();
    assert!(editor(&h).views.unsatisfied.is_none());
    for (a, b) in before.iter().zip(points(&h, 0)) {
        assert!(same(b, [600.0 - a[1], a[0]]), "{a:?} → {b:?} {}", status(&h));
    }
    // The label taken off; its dimension, which turned to face across Center (Left/Right), deleted.
    assert!(family(&h).dimensions.iter().all(|d| d.label.is_none()), "the label taken off");
    assert_eq!(family(&h).dimensions.len(), 1);
}

#[test]
fn a_formula_the_rotation_would_change_refuses_it_leaving_everything_as_it_was() {
    // The box's first corner's x is Len (100): a turn would change it.
    let script = family_with("").replacen(r#""start": [100, 100]"#, r#""start": ["Len", 100]"#, 1);
    let mut h = plan_of(&script);
    pick(&mut h, &[Picked::Form(0)]);
    let before = family(&h).forms[0].clone();
    shortcut(&mut h, "RO");
    click(&mut h, [700.0, 300.0]);
    click(&mut h, [300.0, 700.0]);
    assert!(status(&h).contains("worked out by a formula"), "{}", status(&h));
    assert!(h.state().status.error);
    assert_eq!(family(&h).forms[0], before);
    assert_eq!(editor(&h).steps(), 0);
}

#[test]
fn in_a_sketch_the_tools_act_on_the_lines_picked_and_split_cuts_the_line_clicked() {
    let mut h = plan();
    ed(&mut h).edit_form_sketch(0).unwrap();
    h.run();
    // Nothing picked: Move says to pick lines first.
    shortcut(&mut h, "MV");
    assert!(status(&h).contains("Pick lines of the sketch first"), "{}", status(&h));
    {
        let s = ed(&mut h).sketch.as_mut().unwrap();
        s.sketch.picked = (0..4).collect();
    }
    h.run();
    shortcut(&mut h, "MV");
    assert!(matches!(tool(&h), Some(M::Move { .. })), "{}", status(&h));
    click(&mut h, [100.0, 100.0]);
    click(&mut h, [300.0, 100.0]);
    let starts: Vec<_> = ed(&mut h).sketch.as_ref().unwrap().sketch.lines.iter().map(|l| l.curve.start()).collect();
    assert!(starts.iter().any(|p| near(p.0, 300.0) && near(p.1, 100.0)), "{starts:?} {}", status(&h));
    // Split the south line (now 300 to 700 along y 100) at x 500.
    shortcut(&mut h, "SL");
    assert_eq!(tool(&h), Some(M::Split));
    click(&mut h, [500.0, 100.0]);
    assert_eq!(ed(&mut h).sketch.as_ref().unwrap().sketch.lines.len(), 5, "{}", status(&h));
    // Mirror - Draw Axis, Copy off, across x 0: the lines picked go west.
    {
        let e = ed(&mut h);
        e.views.modify_options.mirror_copy = false;
        let s = e.sketch.as_mut().unwrap();
        s.sketch.picked = (0..5).collect();
    }
    shortcut(&mut h, "DM");
    click(&mut h, [0.0, -100.0]);
    click(&mut h, [0.0, 600.0]);
    let xs: Vec<f64> = ed(&mut h).sketch.as_ref().unwrap().sketch.lines.iter().map(|l| l.curve.start().0).collect();
    assert!(xs.iter().all(|x| *x < 0.0), "{xs:?} {}", status(&h));
    // Finished, the form is where the sketch put it.
    shortcut(&mut h, "FG");
    assert!(ed(&mut h).sketch.is_none(), "{}", status(&h));
    assert!(points(&h, 0).iter().all(|p| p[0] <= 0.0 && p[0] >= -700.0 - 1e-6), "{:?}", points(&h, 0));
}

#[test]
fn in_a_sketch_offset_copies_a_line_and_trim_extend_single_brings_one_to_a_boundary() {
    let mut h = plan();
    ed(&mut h).edit_form_sketch(0).unwrap();
    h.run();
    shortcut(&mut h, "OF");
    ed(&mut h).views.modify_options.offset = "50".into();
    // The north line (y 500), clicked just inside: a copy at y 450.
    click(&mut h, [300.0, 495.0]);
    let lines = ed(&mut h).sketch.as_ref().unwrap().sketch.lines.clone();
    assert_eq!(lines.len(), 5, "{}", status(&h));
    let new = lines.last().unwrap();
    assert!(near(new.curve.start().1, 450.0) && near(new.curve.end().1, 450.0), "{:?}", new.curve);
    assert_ne!(new.guid, lines[2].guid);
    // Trim/Extend Single Element: the copy cut back to the east side (x 500)... it runs 500 to 100: extend its start past
    // nothing, so trim its west end to the west side kept east of it.
    {
        let e = ed(&mut h);
        e.modify_tool_on(M::TrimTo { boundary: None, many: false }).unwrap();
    }
    h.run();
    click(&mut h, [100.0, 300.0]);
    assert!(matches!(tool(&h), Some(M::TrimTo { boundary: Some(3), .. })), "{:?}", tool(&h));
    click(&mut h, [300.0, 450.0]);
    let l = ed(&mut h).sketch.as_ref().unwrap().sketch.lines[4].clone();
    let xs = [l.curve.start().0, l.curve.end().0];
    assert!(xs.iter().any(|x| near(*x, 100.0)) && xs.iter().any(|x| near(*x, 500.0)), "{xs:?} {}", status(&h));
}

#[test]
fn the_ribbon_greys_a_tool_that_cant_be_used_and_its_button_picks_it() {
    let mut h = plan();
    // Nothing selected: Move greyed; Offset enabled.
    assert!(editor(&h).modify_blocked(&M::Move { copy: false, from: None }).is_some());
    assert!(editor(&h).modify_blocked(&M::Offset).is_none());
    pick(&mut h, &[Picked::Form(0)]);
    press(&mut h, "Rotate");
    assert!(matches!(tool(&h), Some(M::Rotate { .. })), "{}", status(&h));
    assert!(editor(&h).modify_blocked(&M::Split).is_some());
    let _ = Which::Plan;
}

/// Each tool at work in plan, saved where `BIMCRAFT_FE8_DUMP` says (only then): its marks, the preview of where the click would
/// put the selection, its Options Bar and its prompt.
#[test]
fn family_modify_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE8_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let start = || {
        let dim = format!(
            r#", {{"refs": [{{"plane": "{CENTER_LR}"}}, {{"form": "{A}", "line": "{A_LINE}3"}}], "view": "plan", "at": [50, 0, 0], "label": "Len"}}"#
        );
        let mut h = app(&family_with(&dim));
        h.set_size(vec2(1600.0, 1000.0));
        h.state_mut().edit_family("Movers");
        h.run();
        super::family_views::view(&mut h, Some(bimcraft_core::family::views::Which::Plan));
        h
    };
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe8-{name}.png"))).unwrap();
    };
    let hover = |h: &mut Harness<BimApp>, p: [f64; 2]| {
        let at = editor(h).views.screen_of([p[0], p[1], 0.0]).unwrap();
        h.hover_at(at);
        h.run();
    };
    // Move: from the box's corner, the pointer north-east.
    let mut h = start();
    pick(&mut h, &[Picked::Form(1)]);
    shortcut(&mut h, "MV");
    click(&mut h, [-1000.0, 100.0]);
    hover(&mut h, [-500.0, 900.0]);
    save(&mut h, "move");
    // Copy with Multiple: two copies made, the third under way.
    shortcut(&mut h, "CO");
    ed(&mut h).views.modify_options.multiple = true;
    click(&mut h, [-1000.0, 100.0]);
    click(&mut h, [-1000.0, 700.0]);
    hover(&mut h, [-1000.0, 1300.0]);
    save(&mut h, "copy-multiple");
    // Rotate: the box and the plane Edge about their centre, the first ray east, the pointer at 60°.
    let mut h = start();
    pick(&mut h, &[Picked::Form(1)]);
    shortcut(&mut h, "RO");
    click(&mut h, [-400.0, 300.0]);
    hover(&mut h, [-800.0 + 400.0 * 0.5, 300.0 + 400.0 * 0.866]);
    save(&mut h, "rotate");
    // Mirror - Pick Axis over Edge.
    let mut h = start();
    pick(&mut h, &[Picked::Form(1)]);
    shortcut(&mut h, "MM");
    hover(&mut h, [1000.0, 600.0]);
    save(&mut h, "mirror-pick");
    click(&mut h, [1000.0, 600.0]);
    save(&mut h, "mirror-picked");
    // Mirror - Draw Axis: the axis' first point, the pointer at its second.
    let mut h = start();
    pick(&mut h, &[Picked::Form(1)]);
    shortcut(&mut h, "DM");
    click(&mut h, [-300.0, -400.0]);
    hover(&mut h, [-300.0, 1000.0]);
    save(&mut h, "mirror-draw");
    // Scale, Graphical, on Edge and the reference line: origin, a point, the pointer half as far.
    let mut h = start();
    pick(&mut h, &[Picked::Plane(g(EDGE)), Picked::Line(g(super::family_drag_move::LINE))]);
    shortcut(&mut h, "RE");
    click(&mut h, [0.0, 0.0]);
    click(&mut h, [1000.0, 0.0]);
    hover(&mut h, [1400.0, 0.0]);
    save(&mut h, "scale");
    // Scale on a form: greyed, the reason in the status bar.
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "RE");
    save(&mut h, "scale-form-refused");
    // Offset: Edge copied 300 west.
    let mut h = start();
    shortcut(&mut h, "OF");
    hover(&mut h, [990.0, 600.0]);
    save(&mut h, "offset");
    click(&mut h, [990.0, 600.0]);
    save(&mut h, "offset-done");
    // Rotate the box its label ties to Center (Left/Right): asked.
    let mut h = start();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "RO");
    click(&mut h, [700.0, 300.0]);
    click(&mut h, [300.0, 700.0]);
    save(&mut h, "constraints");
    // In a sketch: the lines picked moved, then Split and Trim/Extend Single Element's prompts.
    let mut h = start();
    ed(&mut h).edit_form_sketch(1).unwrap();
    h.run();
    ed(&mut h).sketch.as_mut().unwrap().sketch.picked = (0..4).collect();
    shortcut(&mut h, "RO");
    click(&mut h, [-400.0, 300.0]);
    hover(&mut h, [-500.0, 700.0]);
    save(&mut h, "sketch-rotate");
    key(&mut h, Key::Escape);
    key(&mut h, Key::Escape);
    shortcut(&mut h, "SL");
    hover(&mut h, [-800.0, 100.0]);
    save(&mut h, "sketch-split");
    click(&mut h, [-800.0, 100.0]);
    ed(&mut h).modify_tool_on(M::TrimTo { boundary: None, many: true }).unwrap();
    click(&mut h, [-1000.0, 300.0]);
    hover(&mut h, [-700.0, 500.0]);
    save(&mut h, "sketch-trim-multiple");
    shortcut(&mut h, "OF");
    hover(&mut h, [-800.0, 480.0]);
    save(&mut h, "sketch-offset");
}
