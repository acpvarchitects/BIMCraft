//! FE-17: Pin and Unpin in a family, as Revit's family editor has them. A part of the family (a form, a reference plane or
//! line, a dimension, a symbol, a connector, a nested instance) pinned keeps its place: a move, a rotation, a mirror, a
//! scale and Align refuse it, naming it, and copies of it come out unpinned. The datums are pinned always.

use std::collections::BTreeMap;

use serde_json::json;

use crate::engine::Session;
use crate::family::constraints::{Copying, MoveRefused, Moving, Named, Stays, Transform, TransformRefused, Transforming};
use crate::family::select::{self, Picked};
use crate::family::{CENTER_LEFT_RIGHT, Family};
use crate::family_constraint_tests::{BAR, LEFT, POST, TOP, g, run, the_box};

/// The box of the constraint tests, with Left and the post pinned as the file says.
fn pinned_box() -> Session {
    let mut fam = the_box();
    fam["ref_planes"][0]["pinned"] = json!(true);
    fam["forms"][1]["pinned"] = json!(true);
    fam["dimensions"][0]["pinned"] = json!(true);
    let mut s = Session::new();
    run(&mut s, "family", fam);
    s
}

fn family(s: &Session) -> &Family {
    &s.model.families["Box"]
}

fn post(f: &Family) -> usize {
    f.forms.iter().position(|x| x.guid == g(POST)).unwrap()
}

#[test]
fn a_pin_is_read_from_a_family_and_written_back_only_where_it_is() {
    let s = pinned_box();
    let f = family(&s);
    let p = post(f);
    let bar = f.forms.iter().position(|x| x.guid == g(BAR)).unwrap();
    assert!(select::pinned(f, Picked::Form(p)), "the post");
    assert!(!select::pinned(f, Picked::Form(bar)), "not the bar");
    assert!(select::pinned(f, Picked::Plane(g(LEFT))), "Left");
    assert!(!select::pinned(f, Picked::Plane(g(TOP))), "not Top");
    let width = f.dimensions[0].guid;
    assert!(select::pinned(f, Picked::Dimension(width)), "Width's dimension");
    // The datums are pinned, always.
    assert!(select::pinned(f, Picked::Plane(CENTER_LEFT_RIGHT)), "a datum");
    // Written back where it is, and left out where it isn't.
    let back = serde_json::to_value(f).unwrap();
    assert_eq!(back["forms"][p]["pinned"], json!(true));
    assert_eq!(back["forms"][bar].get("pinned"), None, "{}", back["forms"][bar]);
    // A file written again reads the same.
    let again: Family = serde_json::from_value(back).unwrap();
    assert!(select::pinned(&again, Picked::Form(p)) && select::pinned(&again, Picked::Plane(g(LEFT))));
}

#[test]
fn pin_and_unpin_say_what_they_changed_and_leave_the_datums_pinned() {
    let s = pinned_box();
    let mut f = family(&s).clone();
    let (p, bar) = (post(&f), f.forms.iter().position(|x| x.guid == g(BAR)).unwrap());
    let all = [Picked::Form(p), Picked::Form(bar), Picked::Plane(g(TOP)), Picked::Plane(CENTER_LEFT_RIGHT)];
    // The post is pinned already, a datum always: only the bar and Top change.
    assert_eq!(select::pin(&mut f, &all, true), vec![Picked::Form(bar), Picked::Plane(g(TOP))]);
    assert!(all.iter().all(|x| select::pinned(&f, *x)));
    assert_eq!(select::pinned_among(&f, &all), Some(Picked::Form(p)), "the first pinned, in the order given");
    // Unpinned: a datum stays pinned, and isn't among what changed.
    assert_eq!(select::pin(&mut f, &all, false), vec![Picked::Form(p), Picked::Form(bar), Picked::Plane(g(TOP))]);
    assert!(select::pinned(&f, Picked::Plane(CENTER_LEFT_RIGHT)));
    assert_eq!(select::pinned_among(&f, &all[..3]), None);
    // What the family hasn't is neither pinned nor pinned by it.
    assert!(!select::pinned(&f, Picked::Form(99)));
    assert!(select::pin(&mut f, &[Picked::Symbol(0), Picked::Connector(0), Picked::Nested(0)], true).is_empty());
}

#[test]
fn a_pinned_part_refuses_a_move_and_a_transform_naming_it() {
    let s = pinned_box();
    let f = family(&s);
    let v = f.values("Small", &BTreeMap::new()).unwrap();
    let p = post(f);
    let moving = Moving { forms: vec![p], ..Moving::default() };
    assert_eq!(f.move_by(&v, &moving, [100.0, 0.0, 0.0]), Err(MoveRefused::Stays(Stays::Pinned(Named::Form(p)))));
    let left = Moving { planes: vec![g(LEFT)], ..Moving::default() };
    assert_eq!(f.move_by(&v, &left, [0.0, 100.0, 0.0]), Err(MoveRefused::Stays(Stays::Pinned(Named::Plane(g(LEFT))))));
    let turn = Transform::Rotate { centre: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 90.0 };
    let taking = Transforming { forms: vec![p], ..Transforming::default() };
    assert!(
        matches!(f.transformed(&v, &taking, &turn), Err(TransformRefused::Stays(Stays::Pinned(Named::Form(i)))) if i == p),
        "a rotation"
    );
    let mirror = Transform::Mirror { point: [0.0; 3], normal: [1.0, 0.0, 0.0] };
    let taking = Transforming { planes: vec![g(LEFT)], ..Transforming::default() };
    assert!(matches!(f.transformed(&v, &taking, &mirror), Err(TransformRefused::Stays(Stays::Pinned(Named::Plane(_))))));
    // Unpinned, the same move is done.
    let mut free = f.clone();
    select::pin(&mut free, &[Picked::Form(p)], false);
    let r = free.move_by(&v, &moving, [100.0, 0.0, 0.0]);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn copies_of_pinned_parts_come_out_unpinned() {
    let s = pinned_box();
    let f = family(&s);
    let copying = Copying { forms: vec![post(f)], planes: vec![g(LEFT)], ..Copying::default() };
    let copies = f.copies(&copying).unwrap();
    assert_eq!((copies.forms.len(), copies.planes.len()), (1, 1));
    assert!(copies.forms.iter().all(|x| !x.pinned) && copies.planes.iter().all(|x| !x.pinned), "{copies:?}");
}

#[test]
fn a_move_that_would_drag_a_pinned_part_along_is_refused_naming_it() {
    use crate::family_constraint_tests::RIGHT;
    // Width's label and its EQ gone, Right moves freely; the bar's end is locked to it and follows.
    let mut fam = the_box();
    let dims = fam["dimensions"].as_array().unwrap()[2..].to_vec();
    fam["dimensions"] = json!(dims);
    let mut s = Session::new();
    run(&mut s, "family", fam);
    let mut f = family(&s).clone();
    let v = f.values("Small", &BTreeMap::new()).unwrap();
    let right = Moving { planes: vec![g(RIGHT)], ..Moving::default() };
    let bar = f.forms.iter().position(|x| x.guid == g(BAR)).unwrap();
    assert!(f.move_by(&v, &right, [100.0, 0.0, 0.0]).is_ok(), "the bar's end follows Right");
    select::pin(&mut f, &[Picked::Form(bar)], true);
    assert_eq!(f.move_by(&v, &right, [100.0, 0.0, 0.0]), Err(MoveRefused::Stays(Stays::Pinned(Named::Form(bar)))));
    // Pinned Left, which nothing here takes along, holds nothing.
    select::pin(&mut f, &[Picked::Form(bar), Picked::Plane(g(LEFT))], false);
    select::pin(&mut f, &[Picked::Plane(g(LEFT))], true);
    assert!(f.move_by(&v, &right, [100.0, 0.0, 0.0]).is_ok());
}

#[test]
fn a_pinned_dimension_held_loose_on_the_clipboard_pastes_unpinned() {
    let s = pinned_box();
    let f = family(&s);
    let width = f.dimensions[0].guid;
    assert!(select::pinned(f, Picked::Dimension(width)));
    let clip = f.clip(&Copying { dimensions: vec![width], ..Copying::default() }).unwrap();
    let pasted = f.paste_into_view(&clip, crate::family::DimView::Plan).unwrap();
    assert_eq!(pasted.len(), 1);
    assert!(!pasted[0].pinned, "the copy is unpinned");
}
