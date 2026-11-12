//! FE-17: Measure and Create Similar in a family's plans and elevations. Measure (ME, as the project's) lays a ruler by two
//! clicks and reads it, changing nothing; Create Similar (CS) starts the tool that makes what is selected, as it is.
use super::family_drag_move::{CENTER_LR, DIM, EDGE, family, g, pick, plan, status, untouched};
use super::family_views::{click_family, editor};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::DimKind;
use bimcraft_core::family::forms::FormKind;

#[test]
fn measure_lays_a_ruler_by_two_clicks_reads_it_and_changes_nothing() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "ME");
    assert!(editor(&h).measuring(), "ME");
    assert!(h.state().prompt().contains("Measure"), "{}", h.state().prompt());
    assert!(editor(&h).selection.items().is_empty(), "the selection goes, as with any tool");
    click_family(&mut h, [0.0, 0.0, 0.0]);
    click_family(&mut h, [300.0, -400.0, 0.0]);
    let r = editor(&h).measured().expect("a ruler laid");
    assert!((r.length - 500.0).abs() < 1e-6 && (r.dx - 300.0).abs() < 1e-6 && (r.dy + 400.0).abs() < 1e-6, "{r:?}");
    assert!(status(&h).contains("500"), "the status bar reads it: {}", status(&h));
    assert!(untouched(&h), "it changes nothing");
    // Another first click lays another; Escape puts the tool down.
    click_family(&mut h, [0.0, 0.0, 0.0]);
    assert!(editor(&h).measured().is_none());
    // Escape lets the first click go, then puts the tool down.
    key(&mut h, Key::Escape);
    assert!(editor(&h).measuring());
    key(&mut h, Key::Escape);
    assert!(!editor(&h).measuring());
    // The Measure panel's button picks it up again (wide enough for the ribbon to show it).
    h.set_size(egui::vec2(3200.0, 1000.0));
    pick(&mut h, &[Picked::Form(0)]);
    press(&mut h, "Measure");
    assert!(editor(&h).measuring());
}

#[test]
fn create_similar_starts_the_tool_that_makes_what_is_selected() {
    let mut h = plan();
    // Nothing selected: nothing.
    shortcut(&mut h, "CS");
    assert!(editor(&h).sketch.is_none() && editor(&h).views.drawing.is_none());
    // A solid extrusion: a new one sketched, solid.
    pick(&mut h, &[Picked::Form(0)]);
    shortcut(&mut h, "CS");
    let s = editor(&h).sketch.as_ref().expect("a sketch");
    assert_eq!((s.kind, s.void), (FormKind::Extrusion, false));
    h.state_mut().active_family_mut().unwrap().cancel_form_sketch();
    h.run();
    // A plane: the Reference Plane tool.
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    shortcut(&mut h, "CS");
    assert_eq!(editor(&h).views.drawing, Some(None), "{}", status(&h));
    key(&mut h, Key::Escape);
    // A datum too.
    pick(&mut h, &[Picked::Plane(g(CENTER_LR))]);
    shortcut(&mut h, "CS");
    assert_eq!(editor(&h).views.drawing, Some(None));
    key(&mut h, Key::Escape);
    // An aligned dimension: Aligned Dimension.
    pick(&mut h, &[Picked::Dimension(g(DIM))]);
    shortcut(&mut h, "CS");
    assert!(editor(&h).views.dimensioning.is_some() && editor(&h).views.dim_kind == DimKind::Aligned);
    key(&mut h, Key::Escape);
    // A symbolic line: another, as the ribbon's Symbolic Line adds one.
    pick(&mut h, &[Picked::Symbol(0)]);
    shortcut(&mut h, "CS");
    assert_eq!(family(&h).symbols.len(), 2, "{}", status(&h));
    // Two things selected: Create Similar takes one, so it does nothing.
    let n = family(&h).symbols.len();
    pick(&mut h, &[Picked::Form(0), Picked::Form(1)]);
    shortcut(&mut h, "CS");
    assert!(editor(&h).sketch.is_none() && family(&h).symbols.len() == n);
}
