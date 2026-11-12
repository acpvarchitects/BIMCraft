//! FE-14: the Clipboard panel in a family that isn't adaptive. Ctrl+C and Ctrl+X put the selection on the clipboard (Cut taking
//! it away as one step); Ctrl+V has the copies follow the pointer in a plan or an elevation, a click putting them there as one
//! step and Escape giving it up; Paste Aligned puts them back in the same place, or dimensions copied alone into the view shown
//! or the views picked. In a sketch the same on the lines picked. What can't be done says why, nothing changed.
use super::family_drag_move::{
    A, A_LINE, CENTER_LR, DIM, EDGE, ed, family, family_with, g, near, pick, plan, plan_of, points, shift, status, untouched,
};
use super::family_views::{editor, view};
use super::*;
use crate::editor::clipboard::{Command, Paste};
use crate::editor::select_box::Picked;
use bimcraft_core::family::constraints::DimView;
use bimcraft_core::family::views::{Side, Which};
use bimcraft_core::family::{DimRef, Expr};

fn click3(h: &mut Harness<BimApp>, p: [f64; 3]) {
    let at = editor(h).views.screen_of(p).expect("a view shown");
    click_at(h, at);
}

fn hover3(h: &mut Harness<BimApp>, p: [f64; 3]) {
    let at = editor(h).views.screen_of(p).expect("a view shown");
    h.hover_at(at);
    h.run();
}

/// Ctrl and `k`, as the window gets Ctrl+C, Ctrl+X or Ctrl+V.
fn ctrl(h: &mut Harness<BimApp>, k: Key) {
    h.key_press_modifiers(Modifiers::COMMAND, k);
    h.run();
}

fn paste(h: &mut Harness<BimApp>, how: Paste) {
    h.state_mut().family_clipboard(Command::Paste(how));
    h.run();
    h.run();
}

/// A lock from box A's east line (x 500) to Edge (x 1000), and Len labelling Center (Left/Right) to its west line.
fn constrained() -> String {
    family_with(&format!(
        r#", {{"refs": [{{"form": "{A}", "line": "{A_LINE}1"}}, {{"plane": "{EDGE}"}}], "view": "plan", "at": [700, 0, 0], "locked": [{{"segment": 0, "length": 500}}]}}, {{"refs": [{{"plane": "{CENTER_LR}"}}, {{"form": "{A}", "line": "{A_LINE}3"}}], "view": "plan", "at": [50, 0, 0], "label": "Len"}}"#
    ))
}

#[test]
fn ctrl_c_and_ctrl_v_in_plan_put_a_copy_where_the_click_is_as_one_step() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    let a = points(&h, 0);
    ctrl(&mut h, Key::C);
    assert!(status(&h).contains("1 element"), "{}", status(&h));
    assert!(untouched(&h), "a copy changes nothing");
    ctrl(&mut h, Key::V);
    assert!(editor(&h).pasting());
    assert!(h.state().prompt().contains("click where the copies go"), "{}", h.state().prompt());
    // The box's middle (300, 300) under the pointer: the click puts it at (1300, 1100).
    hover3(&mut h, [1300.0, 1100.0, 0.0]);
    click3(&mut h, [1300.0, 1100.0, 0.0]);
    let f = family(&h);
    assert_eq!(f.forms.len(), 3, "{}", status(&h));
    assert_eq!(points(&h, 0), a, "the original stays");
    let d = shift(&a, &points(&h, 2));
    assert!(near(d[0], 1000.0) && near(d[1], 800.0), "{d:?}");
    assert!(f.forms[2].guid != g(A) && !f.forms[2].guid.is_nil());
    assert_eq!(editor(&h).selection.items(), &[Picked::Form(2)], "the copy is selected");
    assert_eq!(editor(&h).steps(), 1);
    assert!(!editor(&h).pasting());
    // The clipboard pastes again, with GUIDs of its own.
    ctrl(&mut h, Key::V);
    click3(&mut h, [1300.0, -1100.0, 0.0]);
    let f = family(&h);
    assert_eq!(f.forms.len(), 4, "{}", status(&h));
    assert_ne!(f.forms[3].guid, f.forms[2].guid);
    assert_eq!(editor(&h).steps(), 2);
}

#[test]
fn a_paste_in_an_elevation_moves_a_copy_up_by_its_start_and_end() {
    let mut h = plan();
    view(&mut h, Some(Which::Elevation(Side::Front)));
    pick(&mut h, &[Picked::Form(0)]);
    ctrl(&mut h, Key::C);
    ctrl(&mut h, Key::V);
    // The box (x 100 to 500, 0 to 300 high) seen from the front: its middle (300, 150) taken 1000 up and 2000 east.
    hover3(&mut h, [2300.0, 0.0, 1150.0]);
    click3(&mut h, [2300.0, 0.0, 1150.0]);
    let f = family(&h);
    assert_eq!(f.forms.len(), 3, "{}", status(&h));
    let num = |e: &Expr| match e {
        Expr::Number(x) => *x,
        _ => f64::NAN,
    };
    assert!(near(num(&f.forms[2].from), 1000.0) && near(num(&f.forms[2].to), 1300.0), "{:?} {:?}", f.forms[2].from, f.forms[2].to);
    assert!(near(shift(&points(&h, 0), &points(&h, 2))[0], 2000.0));
    assert_eq!(editor(&h).steps(), 1);
}

#[test]
fn escape_gives_a_paste_up_with_nothing_changed() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(0)]);
    ctrl(&mut h, Key::C);
    ctrl(&mut h, Key::V);
    hover3(&mut h, [1300.0, 300.0, 0.0]);
    key(&mut h, Key::Escape);
    assert!(!editor(&h).pasting());
    click3(&mut h, [1300.0, 300.0, 0.0]);
    assert_eq!(family(&h).forms.len(), 2);
    assert!(untouched(&h));
}

#[test]
fn cut_takes_the_selection_away_as_one_step_and_undo_brings_it_back() {
    let mut h = plan();
    pick(&mut h, &[Picked::Form(1)]);
    let b = family(&h).forms[1].clone();
    ctrl(&mut h, Key::X);
    assert_eq!(family(&h).forms.len(), 1, "{}", status(&h));
    assert_eq!(editor(&h).steps(), 1);
    // Pasted in the same place: a box where it was, a new one.
    paste(&mut h, Paste::SamePlace);
    let f = family(&h);
    assert_eq!(f.forms.len(), 2, "{}", status(&h));
    assert_eq!(
        f.forms[1].sketch.iter().map(|l| l.curve.clone()).collect::<Vec<_>>(),
        b.sketch.iter().map(|l| l.curve.clone()).collect::<Vec<_>>()
    );
    assert_ne!(f.forms[1].guid, b.guid);
    assert_eq!(editor(&h).steps(), 2);
    // Undo twice: the paste, then the cut, taken back.
    h.state_mut().family_undo();
    h.run();
    h.state_mut().family_undo();
    h.run();
    assert_eq!(family(&h).forms[1], b);
}

#[test]
fn cut_refuses_what_delete_refuses_and_copy_what_a_copy_refuses() {
    let mut h = plan();
    // A datum: neither copied nor taken away.
    pick(&mut h, &[Picked::Plane(g(CENTER_LR))]);
    ctrl(&mut h, Key::X);
    assert!(!status(&h).is_empty() && h.state().status.error, "{}", status(&h));
    ctrl(&mut h, Key::C);
    assert!(status(&h).contains("datum"), "{}", status(&h));
    assert!(untouched(&h));
    assert!(h.state().clipboard.is_none());
    // Nothing on the clipboard: Paste is greyed, saying why.
    let doc = h.state().active_doc().unwrap();
    let why = editor(&h).clipboard_blocked(Command::Paste(Paste::Pointer), None, doc);
    assert!(why.is_some_and(|w| w.contains("Nothing has been copied")));
}

#[test]
fn a_paste_a_formula_would_have_to_change_is_refused_with_nothing_changed() {
    let script = family_with("").replace(&super::family_drag_move::square(-1000, 100, -600, 500, None), r#"[{"start": ["-W", 100], "end": ["-W / 2", 100]}, {"start": ["-W / 2", 100], "end": ["-W / 2", 500]}, {"start": ["-W / 2", 500], "end": ["-W", 500]}, {"start": ["-W", 500], "end": ["-W", 100]}]"#).replace(r#""parameters": ["#, r#""parameters": [{"name": "W", "value": 1000}, "#);
    let mut h = plan_of(&script);
    pick(&mut h, &[Picked::Form(1)]);
    ctrl(&mut h, Key::C);
    ctrl(&mut h, Key::V);
    // East, its x would change: refused, nothing made, still pasting.
    click3(&mut h, [250.0, 300.0, 0.0]);
    assert!(status(&h).contains("formula"), "{}", status(&h));
    assert_eq!(family(&h).forms.len(), 2);
    assert!(untouched(&h));
    // North it keeps its formulas.
    click3(&mut h, [-750.0, 1300.0, 0.0]);
    assert_eq!(family(&h).forms.len(), 3, "{}", status(&h));
}

#[test]
fn constraints_among_the_copies_come_along_and_those_to_what_stays_are_dropped() {
    let mut h = plan_of(&constrained());
    let dims = family(&h).dimensions.len();
    // The box and Edge: the lock between them comes along, Len to Center (Left/Right) is dropped.
    pick(&mut h, &[Picked::Form(0), Picked::Plane(g(EDGE))]);
    ctrl(&mut h, Key::C);
    ctrl(&mut h, Key::V);
    click3(&mut h, [800.0, 1300.0, 0.0]);
    let f = family(&h);
    assert_eq!(f.forms.len(), 3, "{}", status(&h));
    assert_eq!(f.dimensions.len(), dims + 1, "{:?}", f.dimensions);
    let lock = f.dimensions.last().unwrap();
    let edge = f.planes.last().unwrap().guid;
    assert_eq!(lock.refs, vec![DimRef::Sketch(f.forms[2].guid, f.forms[2].sketch[1].guid), DimRef::Plane(edge)]);
    assert!(status(&h).contains("1 constraint"), "{}", status(&h));
    assert!(!h.state().status.error);
    // The box alone: both are dropped, and said.
    pick(&mut h, &[Picked::Form(0)]);
    ctrl(&mut h, Key::C);
    paste(&mut h, Paste::SamePlace);
    assert_eq!(family(&h).dimensions.len(), dims + 1);
    assert!(status(&h).contains("2 constraint"), "{}", status(&h));
}

#[test]
fn dimensions_copied_alone_paste_into_the_view_shown_or_the_views_picked() {
    let mut h = plan();
    pick(&mut h, &[Picked::Dimension(g(DIM))]);
    ctrl(&mut h, Key::C);
    let doc = h.state().active_doc().unwrap();
    // Not where they go: into views.
    let held = h.state().clipboard.clone();
    assert!(editor(&h).clipboard_blocked(Command::Paste(Paste::Pointer), held.as_ref(), doc).is_some());
    assert!(editor(&h).clipboard_blocked(Command::Paste(Paste::SamePlace), held.as_ref(), doc).is_some());
    view(&mut h, Some(Which::Ceiling));
    paste(&mut h, Paste::CurrentView);
    let f = family(&h);
    let made = f.dimensions.last().unwrap();
    assert_eq!(made.view, DimView::Ceiling, "{}", status(&h));
    assert_ne!(made.guid, g(DIM));
    assert_eq!(made.refs, f.dimensions.iter().find(|d| d.guid == g(DIM)).unwrap().refs);
    assert_eq!(editor(&h).steps(), 1);
    // Selected Views: a dialog of the views that can draw it (plans: the elevations see Edge flat or edge-on along it).
    paste(&mut h, Paste::SelectedViews);
    let views: Vec<Which> = editor(&h).views.paste_views.as_ref().expect("the dialog").views.iter().map(|(w, _)| *w).collect();
    assert!(views.contains(&Which::Plan) && views.contains(&Which::Ceiling), "{views:?}");
    h.get_by_label("Floor Plan: Ref. Level").click();
    h.run();
    press(&mut h, "OK");
    h.run();
    let f = family(&h);
    assert_eq!(f.dimensions.iter().filter(|d| d.refs == made.refs).count(), 3, "{}", status(&h));
    assert_eq!(editor(&h).steps(), 2);
    // Model elements don't paste into views.
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    ctrl(&mut h, Key::C);
    let held = h.state().clipboard.clone();
    assert!(editor(&h).clipboard_blocked(Command::Paste(Paste::CurrentView), held.as_ref(), doc).is_some());
    assert!(editor(&h).clipboard_blocked(Command::Paste(Paste::PickedLevel), held.as_ref(), doc).is_some());
}

#[test]
fn aligned_to_same_place_works_from_3d_where_paste_by_the_pointer_is_greyed() {
    let mut h = plan();
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    ctrl(&mut h, Key::C);
    view(&mut h, None);
    let doc = h.state().active_doc().unwrap();
    let held = h.state().clipboard.clone();
    assert!(editor(&h).clipboard_blocked(Command::Paste(Paste::Pointer), held.as_ref(), doc).is_some_and(|w| w.contains("{3D}")));
    paste(&mut h, Paste::SamePlace);
    let f = family(&h);
    let copy = f.planes.last().unwrap();
    assert!(copy.guid != g(EDGE) && copy.start[0] == 1000.0, "{copy:?} {}", status(&h));
    assert_eq!(editor(&h).selection.items(), &[Picked::Plane(copy.guid)]);
    // Another document's clipboard is refused, saying where to paste it.
    let other = crate::editor::tabs::DocId(doc.0 + 1);
    assert!(editor(&h).clipboard_blocked(Command::Paste(Paste::SamePlace), held.as_ref(), other).is_some_and(|w| w.contains("Movers")));
}

#[test]
fn the_ribbons_paste_aligned_drops_its_choices_and_one_pastes() {
    let mut h = plan();
    pick(&mut h, &[Picked::Plane(g(EDGE))]);
    press(&mut h, "Copy to Clipboard");
    assert!(h.state().clipboard.is_some(), "{}", status(&h));
    press(&mut h, "Paste Aligned");
    press(&mut h, "Aligned to Same Place");
    h.run();
    assert_eq!(family(&h).planes.iter().filter(|p| p.start[0] == 1000.0).count(), 2, "{}", status(&h));
}

#[test]
fn in_a_sketch_lines_paste_into_it_with_the_locks_among_them() {
    let mut h = plan();
    ed(&mut h).edit_form_sketch(0).unwrap();
    h.run();
    let form = ed(&mut h).sketch.as_ref().unwrap().guid;
    let (l0, l1, l2) = {
        let s = &ed(&mut h).sketch.as_ref().unwrap().sketch;
        (s.lines[0].guid, s.lines[1].guid, s.lines[2].guid)
    };
    // A lock between lines 0 and 1, and one from 1 to 2.
    let lock = |a, b| bimcraft_core::family::FamilyDimension {
        id: 0,
        guid: bimcraft_core::ids::Guid::new(),
        view: DimView::Plan,
        refs: vec![DimRef::Sketch(form, a), DimRef::Sketch(form, b)],
        at: [0.0; 3],
        label: None,
        locked: vec![bimcraft_core::model::Lock { segment: 0, length: 400.0 }],
        equal: false,
        alignment: false,
        kind: Default::default(),
    };
    {
        let s = ed(&mut h).sketch.as_mut().unwrap();
        s.dims.push(lock(l0, l2));
        s.dims.push(lock(l1, l2));
        s.sketch.picked = [0, 2].into();
    }
    h.run();
    let n = ed(&mut h).sketch.as_ref().unwrap().sketch.lines.len();
    let dims = ed(&mut h).sketch.as_ref().unwrap().dims.len();
    ctrl(&mut h, Key::C);
    assert!(status(&h).contains("2 line"), "{}", status(&h));
    ctrl(&mut h, Key::V);
    assert!(h.state().prompt().contains("click where the copies go"), "{}", h.state().prompt());
    // Escape gives it up; then again, and a click: the two lines' middle (300, 300) put at (300, 1300).
    key(&mut h, Key::Escape);
    assert!(!editor(&h).pasting());
    assert_eq!(ed(&mut h).sketch.as_ref().unwrap().sketch.lines.len(), n);
    ctrl(&mut h, Key::V);
    hover3(&mut h, [300.0, 1300.0, 0.0]);
    click3(&mut h, [300.0, 1300.0, 0.0]);
    let s = ed(&mut h).sketch.as_ref().unwrap();
    assert_eq!(s.sketch.lines.len(), n + 2, "{}", status(&h));
    assert_eq!(s.sketch.picked, [n, n + 1].into(), "the copies picked");
    let made = &s.sketch.lines[n];
    assert!(made.guid != l0 && !made.guid.is_nil());
    let p = made.curve.points();
    assert!(near(p[0].0, 100.0) && near(p[0].1, 1100.0), "{p:?}");
    // The lock between them came along, naming the copies; the one to line 1 was dropped, and said.
    assert_eq!(s.dims.len(), dims + 1);
    assert_eq!(
        s.dims.last().unwrap().refs,
        vec![DimRef::Sketch(form, s.sketch.lines[n].guid), DimRef::Sketch(form, s.sketch.lines[n + 1].guid)]
    );
    assert!(status(&h).contains("1 constraint"), "{}", status(&h));
    // Ctrl+Z, the sketch's own, takes the paste back in one go.
    ctrl(&mut h, Key::Z);
    assert_eq!(ed(&mut h).sketch.as_ref().unwrap().sketch.lines.len(), n);
    // Elements on the clipboard don't paste into a sketch.
    let doc = h.state().active_doc().unwrap();
    let held = Some(crate::editor::clipboard::Held::Elements { doc, family: "Movers".into(), clip: Default::default() });
    assert!(editor(&h).clipboard_blocked(Command::Paste(Paste::Pointer), held.as_ref(), doc).is_some());
}

#[test]
fn the_clipboards_commands_are_the_family_documents_and_keyboard_shortcuts_lists_them() {
    for c in Command::ALL {
        assert!(crate::shortcuts::ACTIONS.contains(&c.label()), "{}", c.label());
        assert!(crate::shortcuts::family_only(c.label()));
        assert!(crate::editor::keys::in_family(c.label()));
        assert!(!crate::editor::keys::FAMILY_SHORTCUTS.iter().any(|(_, a)| *a == c.label()), "no key of Revit's");
    }
}

/// The Clipboard panel with Paste Aligned dropped down, and a paste following the pointer in plan, saved where
/// `BIMCRAFT_FE14_DUMP` says (only then).
#[test]
fn family_clipboard_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE14_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let mut h = app(&constrained());
    h.set_size(vec2(1600.0, 1000.0));
    h.state_mut().edit_family("Movers");
    h.run();
    view(&mut h, Some(Which::Plan));
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe14-{name}.png"))).unwrap();
    };
    pick(&mut h, &[Picked::Form(0), Picked::Plane(g(EDGE))]);
    ctrl(&mut h, Key::C);
    press(&mut h, "Paste Aligned");
    save(&mut h, "paste-aligned");
    key(&mut h, Key::Escape);
    ctrl(&mut h, Key::V);
    hover3(&mut h, [1300.0, -900.0, 0.0]);
    save(&mut h, "pasting");
    click3(&mut h, [1300.0, -900.0, 0.0]);
    save(&mut h, "pasted");
    // In a sketch: the lines following the pointer.
    ed(&mut h).edit_form_sketch(1).unwrap();
    h.run();
    ed(&mut h).sketch.as_mut().unwrap().sketch.picked = (0..4).collect();
    ctrl(&mut h, Key::C);
    ctrl(&mut h, Key::V);
    hover3(&mut h, [-800.0, 1100.0, 0.0]);
    save(&mut h, "sketch-pasting");
}
