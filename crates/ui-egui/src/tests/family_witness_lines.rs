//! FE-23c: Edit Witness Lines on a family's dimension (keyless: the project's EW is Wire), as Revit's: with an aligned dimension selected, a click on a
//! reference the Dimension tool takes adds it, in order along the dimension; a click on one it has takes it off; a click on
//! nothing, or Esc, ends. Each click is one step. What it can't do it refuses, saying why: a dimension keeps two references,
//! a labelled one measures one segment, a reference must be parallel to the dimension's, a lock or EQ isn't lost silently.
use super::family_views::{click_family, editor, open, view};
use super::*;
use bimcraft_core::family::DimRef;
use bimcraft_core::family::views::Which;

const LEFT: &str = "11111111-1111-4111-8111-000000000001";
const RIGHT: &str = "11111111-1111-4111-8111-000000000002";
const CENTER_LR: &str = "9e3c0a10-1f0b-4d8c-8a5e-0000000000a3";

/// A box family: Left and Right planes at x ±500; `dims`, its dimensions as JSON.
fn family(dims: &str) -> String {
    format!(
        r#"family {{"name": "Box", "category": "generic", "parameters": [{{"name": "Width", "value": 1000}}], "types": {{"T": {{}}}}, "ref_planes": [{{"guid": "{LEFT}", "name": "Left", "start": [-500, -2000, 0], "end": [-500, 2000, 0], "up": [0, 0, 1]}}, {{"guid": "{RIGHT}", "name": "Right", "start": [500, -2000, 0], "end": [500, 2000, 0], "up": [0, 0, 1]}}], "dimensions": [{dims}]}}"#
    )
}

fn plane(g: &str) -> String {
    format!(r#"{{"plane": "{g}"}}"#)
}

/// A dimension along x at y -700 between `refs`, with `more` JSON fields.
fn dim(refs: &[&str], more: &str) -> String {
    let refs: Vec<String> = refs.iter().map(|g| plane(g)).collect();
    format!(r#"{{"refs": [{}], "at": [0, -700, 0]{more}}}"#, refs.join(", "))
}

fn g(s: &str) -> bimcraft_core::ids::Guid {
    serde_json::from_value(json!(s)).unwrap()
}

/// The box open in its floor plan, its one dimension selected and Edit Witness Lines on.
fn editing(dims: &str) -> Harness<'static, BimApp> {
    let (mut h, _) = open(&family(dims), "Box");
    view(&mut h, Some(Which::Plan));
    click_family(&mut h, [200.0, -700.0, 0.0]);
    assert!(editor(&h).selected_dimension().is_some(), "the dimension is selected");
    // Keyless, as the project's EW is Wire: the Options Bar's button.
    press(&mut h, "Edit Witness Lines");
    assert!(editor(&h).views.witnessing.is_some(), "Edit Witness Lines is on");
    h
}

fn refs(h: &Harness<BimApp>) -> Vec<DimRef> {
    editor(h).dimensions[0].refs.clone()
}

fn planes(gs: &[&str]) -> Vec<DimRef> {
    gs.iter().map(|s| DimRef::Plane(g(s))).collect()
}

fn said(h: &Harness<BimApp>) -> (String, bool) {
    let s = &h.state().status;
    (s.text.clone(), s.error)
}

#[test]
fn ew_adds_a_reference_in_order_and_takes_it_off_each_click_one_step() {
    let mut h = editing(&dim(&[LEFT, RIGHT], ""));
    let steps = editor(&h).steps();
    click_family(&mut h, [0.0, 600.0, 0.0]);
    assert_eq!(refs(&h), planes(&[LEFT, CENTER_LR, RIGHT]), "in order along the dimension");
    assert_eq!(editor(&h).steps(), steps + 1);
    click_family(&mut h, [0.0, 600.0, 0.0]);
    assert_eq!(refs(&h), planes(&[LEFT, RIGHT]), "taken off again");
    assert_eq!(editor(&h).steps(), steps + 2);
    // Two references at least: the dimension keeps them, saying why.
    click_family(&mut h, [-500.0, 600.0, 0.0]);
    assert_eq!(refs(&h), planes(&[LEFT, RIGHT]));
    let (text, error) = said(&h);
    assert!(error && text.contains("two references"), "{text}");
    // Esc ends the tool.
    key(&mut h, Key::Escape);
    assert!(editor(&h).views.witnessing.is_none());
}

#[test]
fn ew_refuses_a_reference_not_parallel_and_a_labelled_dimension() {
    let mut h = editing(&dim(&[LEFT, RIGHT], ""));
    // Center (Front/Back) runs along x: square to the dimension's references.
    click_family(&mut h, [-1500.0, 0.0, 0.0]);
    assert_eq!(refs(&h), planes(&[LEFT, RIGHT]));
    let (text, error) = said(&h);
    assert!(error && text.contains("parallel"), "{text}");
    let mut h = editing(&dim(&[LEFT, RIGHT], r#", "label": "Width""#));
    click_family(&mut h, [0.0, 600.0, 0.0]);
    assert_eq!(refs(&h), planes(&[LEFT, RIGHT]));
    let (text, error) = said(&h);
    assert!(error && text.contains("label"), "{text}");
}

#[test]
fn a_lock_or_eq_is_never_lost_without_a_word() {
    // Taking off the middle reference merges two segments: refused while only one of them is locked.
    let mut h = editing(&dim(&[LEFT, CENTER_LR, RIGHT], r#", "locked": [{"segment": 0, "length": 500}]"#));
    click_family(&mut h, [0.0, 600.0, 0.0]);
    assert_eq!(refs(&h).len(), 3);
    let (text, error) = said(&h);
    assert!(error && text.contains("locked"), "{text}");
    // Both locked: the merged segment is locked at their sum.
    let mut h = editing(&dim(&[LEFT, CENTER_LR, RIGHT], r#", "locked": [{"segment": 0, "length": 500}, {"segment": 1, "length": 500}]"#));
    click_family(&mut h, [0.0, 600.0, 0.0]);
    let d = editor(&h).dimensions[0].clone();
    assert_eq!(d.refs, planes(&[LEFT, RIGHT]));
    assert_eq!(d.locked.iter().map(|l| (l.segment, l.length)).collect::<Vec<_>>(), [(0, 1000.0)]);
    // Splitting a locked segment drops its lock, and says so.
    click_family(&mut h, [0.0, 600.0, 0.0]);
    let d = editor(&h).dimensions[0].clone();
    assert_eq!(d.refs, planes(&[LEFT, CENTER_LR, RIGHT]));
    assert!(d.locked.is_empty());
    let (text, error) = said(&h);
    assert!(!error && text.contains("lock"), "{text}");
    // An EQ dimension of two segments left with one is no longer EQ, and says so.
    let mut h = editing(&dim(&[LEFT, CENTER_LR, RIGHT], r#", "equal": true"#));
    click_family(&mut h, [0.0, 600.0, 0.0]);
    let d = editor(&h).dimensions[0].clone();
    assert_eq!((d.refs.len(), d.equal), (2, false));
    let (text, _) = said(&h);
    assert!(text.contains("EQ"), "{text}");
    // Added back: two segments, not EQ until it is asked for again.
    click_family(&mut h, [0.0, 600.0, 0.0]);
    assert_eq!(editor(&h).dimensions[0].refs.len(), 3);
}
