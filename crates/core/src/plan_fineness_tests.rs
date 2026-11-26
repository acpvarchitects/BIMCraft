//! GEO-2c-3: a plan zoomed in draws a component's curves finer (GEO-2c), and a finer build reaches past its default build's
//! box by as much as [`crate::family::fineness::finer_reach`] says. What the plan chooses by that box (what it builds finer,
//! its window, its cut plane and view depth) reaches as far, so a finer curve past the box is drawn, and a click or a hover
//! there finds its component.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::geom::P2;
use crate::ids::Id;
use crate::render::{self, Drawing, Prim, View};
use crate::section_fineness_tests::{FRONT, apex, default_box, family, place, ring};

/// Level 1, at 0: its plan cuts at 1200 and sees down to 0.
const LEVEL: Id = 100000;
/// Level 2, at 3000: its plan sees down to 3000.
const LEVEL_2: Id = 100001;

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

/// The radius of [`arch`]'s arc: 30° through 1000 mm.
fn arch_radius() -> f64 {
    500.0 / 15f64.to_radians().sin()
}

/// How far [`arch`]'s arc bulges past its ends: up for one standing, down for one hanging.
fn sagitta() -> f64 {
    arch_radius() * (1.0 - 15f64.to_radians().cos())
}

/// A slab 1000 mm wide (x from -500 to 500), 1000 deep (y from 0 to 1000), its top (`up`) or its bottom an arc of 30° whose
/// ends are at `ends` up, bulging away from the slab's other face, which is at 0 for one standing and at 2000 for one hanging.
/// Three chords at the default, its farthest midway along the middle one: the default stops 7.35 mm short of it.
fn arch(ends: f64, up: bool) -> Value {
    let line = |a: [f64; 2], b: [f64; 2]| json!({"start": a, "end": b});
    let face = if up { 0.0 } else { 2000.0 };
    // Each arc counter-clockwise, so it bulges out of the slab: west along its top, east along its bottom.
    let (a, b) = if up { ([500.0, ends], [-500.0, ends]) } else { ([-500.0, ends], [500.0, ends]) };
    json!({"sketch": [
        {"start": a, "end": b, "turn": 30},
        line(b, [b[0], face]),
        line([b[0], face], [a[0], face]),
        line([a[0], face], a)],
        "host": {"plane": FRONT}, "from": -1000, "to": 0})
}

/// A slab hung from 2000 whose arched bottom dips 5.8 mm under Level 1's cut plane, its default build's 1.5 over it. Zoomed in
/// to 2 px a mm, the plan draws it finer: under the cut, so drawn, and a click on its edge over the arc's lowest finds it.
#[test]
fn a_finer_curve_dipping_under_the_cut_plane_short_of_its_default_bottom_is_drawn_and_picked() {
    let mut s = Session::new();
    family(&mut s, "Vault", vec![arch(1260.0, false)]);
    let c = place(&mut s, "Vault", [0.0, 0.0], 0.0);
    let lowest = 1260.0 - sagitta();
    let [lo, _] = default_box(&s, c);
    assert!(lo[2] > 1201.0 && lowest < 1195.0, "its default bottom {:.2}, its lowest {lowest:.2}", lo[2]);
    let view = View { angle: 0.0, scale: 2.0, cx: 0.0, cy: 0.0, width: 200, height: 200 };
    let d = render::draw(&s.model, LEVEL, view);
    assert_eq!(picked(&d, &view, P2(0.0, 0.0)), Some(c), "a click on its south edge, over its lowest");
}

/// A slab on Level 1 whose arched top rises 3.8 mm past Level 2, where Level 2's plan sees down to, its default build's top
/// 3.5 short of it. Zoomed in to 2 px a mm, Level 2's plan draws it finer: over its view depth, so drawn, and a click on its
/// edge under the arc's highest finds it.
#[test]
fn a_finer_curve_rising_past_the_view_depth_short_of_its_default_top_is_drawn_and_picked() {
    let mut s = Session::new();
    family(&mut s, "Arch", vec![arch(2938.0, true)]);
    let c = place(&mut s, "Arch", [5000.0, 0.0], 0.0);
    let highest = 2938.0 + sagitta();
    let [_, hi] = default_box(&s, c);
    assert!(hi[2] < 2999.0 && highest > 3003.0, "its default top {:.2}, its highest {highest:.2}", hi[2]);
    let view = View { angle: 0.0, scale: 2.0, cx: 5000.0, cy: 0.0, width: 200, height: 200 };
    let d = render::draw(&s.model, LEVEL_2, view);
    assert_eq!(picked(&d, &view, P2(5000.0, 0.0)), Some(c), "a click on its south edge, under its highest");
}
