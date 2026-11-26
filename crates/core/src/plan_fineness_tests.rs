//! GEO-2c-3: a plan zoomed in draws a component's curves finer (GEO-2c), and a finer build reaches past its default build's
//! box by as much as [`crate::family::fineness::finer_reach`] says. What the plan chooses by that box (what it builds finer,
//! and what meets its window) reaches as far, so a finer curve past the box is drawn, and a click or a hover there finds its
//! component.

use crate::engine::Session;
use crate::geom::P2;
use crate::ids::Id;
use crate::render::{self, Drawing, Prim, View};
use crate::section_fineness_tests::{apex, default_box, family, place, ring};

/// Level 1, at 0: its plan cuts at 1200 and sees down to 0.
const LEVEL: Id = 100000;

/// The farthest east `d` draws `owner`, mm.
fn east(d: &Drawing, view: &View, owner: Id) -> Option<f64> {
    d.prims
        .iter()
        .zip(&d.owners)
        .filter(|(_, o)| **o == Some(owner))
        .flat_map(|(p, _)| match p {
            Prim::Line { pts, .. } => pts.clone(),
            Prim::Fill { rings, .. } => rings.iter().flatten().copied().collect(),
            Prim::Image { .. } => Vec::new(),
        })
        .map(|q| view.mm(q).0)
        .reduce(f64::max)
}

/// What a click or a hover at `at` finds in `d`: the element drawn within 2 px of it.
fn picked(d: &Drawing, view: &View, at: P2) -> Option<Id> {
    d.pick(view.px(at), 2.0)
}

/// [`ring`] zoomed in to 2 px a mm on a plan 100 px square whose screen begins 5 mm short of its apex. What the plan builds
/// finer, half the screen's diagonal round its middle (35 mm), begins 15 mm short of the apex: past the ring's default box,
/// which stops 21 mm short, and within the reach of a finer build of it. Built finer, its curve shows on the screen out to its
/// apex, within the two curves' chords (half a mm), and a click or a hover there finds it.
#[test]
fn a_finer_curve_reaching_onto_a_zoomed_plan_from_past_its_default_box_is_drawn_and_picked() {
    let mut s = Session::new();
    family(&mut s, "Ring", vec![ring()]);
    let c = place(&mut s, "Ring", [0.0, 0.0], 0.0);
    let view = View { angle: 0.0, scale: 2.0, cx: apex() + 20.0, cy: 0.0, width: 100, height: 100 };
    let [_, hi] = default_box(&s, c);
    // In the window (a screen past each side: 75 mm round the middle), short of what is built finer.
    assert!(hi[0] > view.cx - 75.0 && hi[0] < view.cx - 35.4, "its default box ends at {:.2}", hi[0]);
    let d = render::draw(&s.model, LEVEL, view);
    let Some(east) = east(&d, &view, c) else { panic!("nothing of the ring drawn") };
    assert!(east > apex() - 0.5 && east <= apex() + 0.01, "drawn out to {east:.2}, its apex at {:.2}", apex());
    assert_eq!(picked(&d, &view, P2(apex(), 0.0)), Some(c), "a click at its apex");
}

/// [`ring`] on a plan 20 px square at 2 px a mm, its middle on the apex: the plan builds and draws only what meets its window,
/// a screen past each side (15 mm round the middle), and the ring's default box stops 21 mm short of the apex, out of it. Its
/// finer build reaches in (and is what the plan shows finer, 7 mm round the middle), so it is drawn out to its apex, and
/// picked there.
#[test]
fn a_finer_curve_reaching_into_the_window_its_default_box_misses_is_drawn_and_picked() {
    let mut s = Session::new();
    family(&mut s, "Ring", vec![ring()]);
    let c = place(&mut s, "Ring", [0.0, 0.0], 0.0);
    let view = View { angle: 0.0, scale: 2.0, cx: apex(), cy: 0.0, width: 20, height: 20 };
    let [_, hi] = default_box(&s, c);
    assert!(hi[0] < view.cx - 15.0, "its default box ends at {:.2}, out of the window", hi[0]);
    let d = render::draw(&s.model, LEVEL, view);
    let Some(east) = east(&d, &view, c) else { panic!("nothing of the ring drawn") };
    assert!(east > apex() - 0.5 && east <= apex() + 0.01, "drawn out to {east:.2}, its apex at {:.2}", apex());
    assert_eq!(picked(&d, &view, P2(apex(), 0.0)), Some(c), "a click at its apex");
}
