//! FE-17: Pin and Unpin in a family's plans and elevations, and Select Pinned Elements. A part pinned keeps its place: a drag,
//! Delete and the Modify tools refuse it, saying it is pinned; Ctrl+drag copies it, the copy unpinned. PE (the status bar's
//! toggle) off, a click and a box pass over what is pinned.
use super::family_box_select::{boxed, selected};
use super::family_drag_move::{EDGE, drag, ed, family, g, near, pick, plan, points, shift, status};
use super::family_views::{click_family, editor};
use super::*;
use crate::editor::select_box::Picked;

fn pinned(h: &Harness<BimApp>, p: Picked) -> bool {
    bimcraft_core::family::select::pinned(&family(h), p)
}

#[test]
fn pn_and_up_pin_and_unpin_what_is_selected_each_a_step_of_the_history() {
    let mut h = plan();
    let line = Picked::Line(g(super::family_drag_move::LINE));
    pick(&mut h, &[Picked::Form(0), Picked::Plane(g(EDGE)), line, Picked::Symbol(0)]);
    shortcut(&mut h, "PN");
    for p in [Picked::Form(0), Picked::Plane(g(EDGE)), line, Picked::Symbol(0)] {
        assert!(pinned(&h, p), "{p:?} {}", status(&h));
    }
    assert!(!pinned(&h, Picked::Form(1)), "B, not selected");
    assert_eq!(editor(&h).steps(), 1);
    assert_eq!(editor(&h).selection.items().len(), 4, "the selection stays");
    // PN again changes nothing, and is no step.
    shortcut(&mut h, "PN");
    assert_eq!(editor(&h).steps(), 1);
    // The form's pin is kept through an edit of the draft and in the family it saves.
    assert!(family(&h).forms[0].pinned);
    shortcut(&mut h, "UP");
    assert!(!pinned(&h, Picked::Form(0)) && !pinned(&h, Picked::Symbol(0)));
    assert_eq!(editor(&h).steps(), 2);
    // Undo pins them again.
    h.state_mut().family_undo();
    h.run();
    assert!(pinned(&h, Picked::Form(0)) && pinned(&h, line));
}

#[test]
fn the_modify_panels_pin_turns_to_unpin_with_what_is_selected_pinned() {
    let mut h = plan();
    // Wide enough for the whole Modify panel.
    h.set_size(egui::vec2(3200.0, 1000.0));
    h.run();
    pick(&mut h, &[Picked::Form(0)]);
    let enabled = |h: &Harness<BimApp>, label: &str| !egui_kittest::kittest::NodeT::accesskit_node(&h.get_by_label(label)).is_disabled();
    assert!(enabled(&h, "Pin") && h.query_by_label("Unpin").is_none(), "A not pinned: Pin");
    press(&mut h, "Pin");
    assert!(pinned(&h, Picked::Form(0)));
    assert!(h.query_by_label("Pin").is_none() && enabled(&h, "Unpin"), "A pinned: Unpin in its place");
    press(&mut h, "Unpin");
    assert!(!pinned(&h, Picked::Form(0)));
    // A datum alone: pinned always, neither changes it.
    pick(&mut h, &[Picked::Plane(g(super::family_drag_move::CENTER_LR))]);
    assert!(!enabled(&h, "Pin"), "a datum: greyed");
}

#[test]
fn a_drag_of_a_pinned_form_moves_nothing_and_says_so_and_ctrl_copies_it_unpinned() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "PN");
    let a0 = points(&h, 0);
    // From A's edge, off its handle at the edge's middle (which says it is pinned too).
    drag(&mut h, [500.0, 150.0, 0.0], [0.0, -400.0, 0.0], Modifiers::NONE);
    assert_eq!(points(&h, 0), a0, "A stays");
    assert!(status(&h).contains("pinned"), "{}", status(&h));
    assert_eq!(editor(&h).steps(), 1, "the pin alone");
    // Ctrl, held as the button comes up: a copy, unpinned, and selected.
    super::family_drag_copy::copy_drag(&mut h, [500.0, 150.0, 0.0], [0.0, -700.0, 0.0]);
    let f = family(&h);
    assert_eq!(f.forms.len(), 3, "{}", status(&h));
    assert_eq!(points(&h, 0), a0, "the original stays");
    let d = shift(&a0, &points(&h, 1));
    assert!(near(d[0], 0.0) && near(d[1], -700.0), "{d:?}");
    assert!(f.forms[0].pinned && !f.forms[1].pinned, "the copy is unpinned");
    assert_eq!(editor(&h).selection.items(), &[Picked::Form(1)]);
}

#[test]
fn a_pinned_plane_holds_a_drag_of_the_selection_it_is_in() {
    let mut h = plan();
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    shortcut(&mut h, "PN");
    pick(&mut h, &[Picked::Form(0), Picked::Plane(g(EDGE))]);
    let a0 = points(&h, 0);
    drag(&mut h, [500.0, 150.0, 0.0], [200.0, 0.0, 0.0], Modifiers::NONE);
    assert_eq!(points(&h, 0), a0, "nothing moves");
    assert!(status(&h).contains("Edge") && status(&h).contains("pinned"), "{}", status(&h));
}

#[test]
fn delete_refuses_what_is_pinned_and_takes_nothing() {
    let mut h = plan();
    pick(&mut h, &[Picked::Symbol(0)]);
    shortcut(&mut h, "PN");
    pick(&mut h, &[Picked::Form(1), Picked::Symbol(0)]);
    key(&mut h, Key::Delete);
    let f = family(&h);
    assert_eq!((f.forms.len(), f.symbols.len()), (2, 1), "nothing deleted");
    assert!(status(&h).contains("pinned"), "{}", status(&h));
    // Unpinned, it goes.
    shortcut(&mut h, "UP");
    key(&mut h, Key::Delete);
    let f = family(&h);
    assert_eq!((f.forms.len(), f.symbols.len()), (1, 0));
}

#[test]
fn the_move_tool_refuses_a_pinned_symbol() {
    let mut h = plan();
    pick(&mut h, &[Picked::Symbol(0)]);
    shortcut(&mut h, "PN");
    let before = family(&h).symbols.clone();
    shortcut(&mut h, "MV");
    click_family(&mut h, [0.0, -800.0, 0.0]);
    click_family(&mut h, [0.0, -1000.0, 0.0]);
    assert_eq!(family(&h).symbols, before, "it stays");
    assert!(status(&h).contains("pinned"), "{}", status(&h));
}

#[test]
fn select_pinned_elements_off_a_click_and_a_box_pass_over_what_is_pinned() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "PN");
    ed(&mut h).selection.apply(&[], crate::editor::select_box::How::Replace);
    h.run();
    shortcut(&mut h, "PE");
    assert!(!h.state().select_pinned, "PE turns it off");
    // A click on A's edge selects nothing; a box over both takes B alone.
    click_family(&mut h, [500.0, 300.0, 0.0]);
    assert!(selected(&h).is_empty(), "{:?}", selected(&h));
    boxed(&mut h, [-1200.0, 700.0, 0.0], [700.0, 0.0, 0.0], Modifiers::NONE);
    assert_eq!(selected(&h), vec![Picked::Form(1)]);
    // A drag from A's edge draws a box instead: A stays.
    let a0 = points(&h, 0);
    drag(&mut h, [500.0, 300.0, 0.0], [0.0, -300.0, 0.0], Modifiers::NONE);
    assert_eq!(points(&h, 0), a0);
    // On again: a click selects it.
    press(&mut h, "Select Pinned Elements");
    assert!(h.state().select_pinned);
    click_family(&mut h, [500.0, 300.0, 0.0]);
    assert_eq!(selected(&h), vec![Picked::Form(0)]);
}

#[test]
fn align_refuses_to_move_a_pinned_form_onto_a_reference() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "PN");
    key(&mut h, Key::Escape);
    let a0 = points(&h, 0);
    // Edge (x 1000) as the reference, A's east line (x 500) as what moves onto it.
    shortcut(&mut h, "AL");
    click_family(&mut h, [1000.0, -1000.0, 0.0]);
    click_family(&mut h, [500.0, 150.0, 0.0]);
    assert_eq!(points(&h, 0), a0, "A stays");
    assert!(status(&h).contains("pinned"), "{}", status(&h));
    assert_eq!(editor(&h).steps(), 1, "the pin alone");
}

#[test]
fn cut_refuses_what_is_pinned_as_delete_does() {
    let mut h = plan();
    pick(&mut h, &[Picked::Symbol(0)]);
    shortcut(&mut h, "PN");
    h.key_press_modifiers(Modifiers::COMMAND, Key::X);
    h.run();
    assert_eq!(family(&h).symbols.len(), 1, "the symbol stays");
    assert!(status(&h).contains("pinned"), "{}", status(&h));
}

/// The Modify panel's Pin and Unpin, the status bar's Select Pinned Elements beside Select Elements by Face and Drag Elements
/// on Selection, and a Measure ruler in plan, saved where `BIMCRAFT_FE17_DUMP` says (only then).
#[test]
fn family_pin_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE17_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let mut h = app(&super::family_drag_move::family_with(""));
    h.set_size(egui::vec2(1600.0, 1000.0));
    h.state_mut().edit_family("Movers");
    h.run();
    super::family_views::view(&mut h, Some(bimcraft_core::family::views::Which::Plan));
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe17-{name}.png"))).unwrap();
    };
    pick(&mut h, &[Picked::Form(0)]);
    save(&mut h, "pin");
    shortcut(&mut h, "PN");
    save(&mut h, "unpin");
    shortcut(&mut h, "PE");
    save(&mut h, "pe-off");
    shortcut(&mut h, "PE");
    shortcut(&mut h, "ME");
    click_family(&mut h, [-1000.0, -300.0, 0.0]);
    click_family(&mut h, [500.0, 500.0, 0.0]);
    save(&mut h, "measure");
}
