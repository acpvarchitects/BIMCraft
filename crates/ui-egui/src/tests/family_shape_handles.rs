//! FE-5: shape handles on a selected form's profile. In plan, an L-shaped extrusion shows a handle at the middle of each edge,
//! pointing out of it; a drag offsets the edge, its neighbours following, as one step of the history; an arc's handle changes
//! its radius. A locked edge, an edge a labelled dimension sets and a drag that would leave no profile are refused, the form
//! left as it was. A void shows them too, and an elevation shows the sides it sees edge-on, beside the Start and End handles.
use super::family_views::{editor, open, view};
use super::*;
use bimcraft_core::family::sketch::FormCurve;
use bimcraft_core::family::views::{Side, Which};
use bimcraft_core::family::{Expr, Family};

const L: &str = "55555555-5555-4555-8555-000000000001";
const WEST: &str = "55555555-5555-4555-8555-000000000002";
const LINE: &str = "55555555-5555-4555-8555-0000000000";

/// The L: 1000 by 1000 less its top right quarter, 300 high, its lines `<LINE>10` to `15` counter-clockwise from the
/// origin; a void box beside it; a slot whose right end is a half circle about (-500, -700); and a plane West on its west
/// side (x 0), the west line Align-locked to it and the east line's distance from it labelled `Len`.
fn family() -> String {
    let pts = [(0, 0), (1000, 0), (1000, 500), (500, 500), (500, 1000), (0, 1000)];
    let lines: Vec<String> = (0..6)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % 6]);
            format!(r#"{{"guid": "{LINE}1{i}", "start": [{}, {}], "end": [{}, {}]}}"#, a.0, a.1, b.0, b.1)
        })
        .collect();
    let lines = lines.join(", ");
    format!(
        r#"family {{"name": "Ell", "category": "generic", "parameters": [{{"name": "Len", "value": 1000}}], "types": {{"T": {{}}}}, "ref_planes": [{{"guid": "{WEST}", "name": "West", "start": [0, -2000, 0], "end": [0, 2000, 0], "up": [0, 0, 1]}}], "forms": [{{"guid": "{L}", "sketch": [{lines}], "from": 0, "to": 300}}, {{"profile": [[1500, 0], [1900, 0], [1900, 400], [1500, 400]], "from": 0, "to": 200, "void": true}}, {{"sketch": [{{"start": [-1000, -800], "end": [-500, -800]}}, {{"start": [-500, -800], "end": [-500, -600], "turn": 180}}, {{"start": [-500, -600], "end": [-1000, -600]}}, {{"start": [-1000, -600], "end": [-1000, -800]}}], "from": 0, "to": 100}}], "dimensions": [{{"refs": [{{"plane": "{WEST}"}}, {{"form": "{L}", "line": "{LINE}15"}}], "at": [0, -300, 0], "locked": [{{"segment": 0, "length": 0}}], "alignment": true}}, {{"refs": [{{"plane": "{WEST}"}}, {{"form": "{L}", "line": "{LINE}11"}}], "at": [500, -400, 0], "label": "Len"}}]}}"#
    )
}

fn in_plan() -> Harness<'static, BimApp> {
    let (mut h, _) = open(&family(), "Ell");
    view(&mut h, Some(Which::Plan));
    h
}

fn select(h: &mut Harness<BimApp>, i: usize) {
    h.state_mut().active_family_mut().unwrap().select_form(i);
    h.run();
}

/// The handles shown: where on screen, and whether each can be dragged.
fn shown(h: &Harness<BimApp>) -> Vec<(egui::Pos2, bool)> {
    editor(h).views.shapes_shown.iter().map(|(p, s)| (*p, s.blocked.is_none())).collect()
}

/// The handle shown nearest family point `p`, which must be within a few points of it.
fn handle_at(h: &Harness<BimApp>, p: [f64; 3]) -> (egui::Pos2, bool) {
    let want = editor(h).views.screen_of(p).unwrap();
    let best = shown(h).into_iter().min_by(|a, b| a.0.distance(want).total_cmp(&b.0.distance(want))).expect("handles shown");
    assert!(best.0.distance(want) < 2.0, "no handle at {p:?}: {:?}", shown(h));
    best
}

/// A drag from the handle at family point `p` by `by` (mm, family axes).
fn drag_handle(h: &mut Harness<BimApp>, p: [f64; 3], by: [f64; 3]) {
    let (a, _) = handle_at(h, p);
    let s = editor(h).views.screen_of(p).unwrap();
    let e = editor(h).views.screen_of([p[0] + by[0], p[1] + by[1], p[2] + by[2]]).unwrap();
    drag_screen(h, a, a + (e - s), PointerButton::Primary);
}

/// Form `i`'s lines' ends as numbers (a formula NaN).
fn ends(h: &Harness<BimApp>, i: usize) -> Vec<[f64; 4]> {
    let f: Family = editor(h).draft_family().unwrap();
    let v = |e: &Expr| match e {
        Expr::Number(x) => *x,
        _ => f64::NAN,
    };
    f.forms[i]
        .sketch
        .iter()
        .map(|l| match &l.curve {
            FormCurve::Line { start, end, .. } => [v(&start[0]), v(&start[1]), v(&end[0]), v(&end[1])],
            FormCurve::Circle { centre, radius } => [v(&centre[0]), v(&centre[1]), v(radius), 0.0],
        })
        .collect()
}

fn near(a: [f64; 4], b: [f64; 4], tol: f64) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

fn status(h: &Harness<BimApp>) -> String {
    h.state().status.text.clone()
}

#[test]
fn an_l_shapes_handles_sit_at_its_edges_middles_and_its_locked_and_labelled_edges_are_greyed() {
    let mut h = in_plan();
    assert!(shown(&h).is_empty(), "nothing selected, no handles");
    select(&mut h, 0);
    assert_eq!(shown(&h).len(), 6, "{:?}", shown(&h));
    let mids = [[500.0, 0.0], [1000.0, 250.0], [750.0, 500.0], [500.0, 750.0], [250.0, 1000.0], [0.0, 500.0]];
    let free: Vec<bool> = mids.iter().map(|m| handle_at(&h, [m[0], m[1], 0.0]).1).collect();
    // The east side's place is Len's, the west side is locked to West: greyed.
    assert_eq!(free, [true, false, true, true, true, false]);
    // A tool in hand: no handles.
    shortcut(&mut h, "DI");
    assert!(editor(&h).views.dimensioning.is_some());
    assert!(shown(&h).is_empty(), "a tool in hand");
}

#[test]
fn a_drag_offsets_the_edge_its_neighbours_follow_and_it_is_one_step_undone_and_redone() {
    let mut h = in_plan();
    select(&mut h, 0);
    let before = ends(&h, 0);
    // The notch's floor (y 500) up 200: the east side runs to 700, the notch's wall starts there.
    drag_handle(&mut h, [750.0, 500.0, 0.0], [0.0, 200.0, 0.0]);
    let after = ends(&h, 0);
    let rise = after[2][1] - 500.0;
    assert!((rise - 200.0).abs() < 15.0, "{after:?} {}", status(&h));
    assert!(near(after[2], [1000.0, 500.0 + rise, 500.0, 500.0 + rise], 1e-6), "{after:?}");
    assert!(near(after[1], [1000.0, 0.0, 1000.0, 500.0 + rise], 1e-6));
    assert!(near(after[3], [500.0, 500.0 + rise, 500.0, 1000.0], 1e-6));
    assert_eq!([after[0], after[4], after[5]], [before[0], before[4], before[5]]);
    // The bottom down 100: its ends slide along the west side (locked to West) and the east (Len from it), which still hold.
    drag_handle(&mut h, [500.0, 0.0, 0.0], [0.0, -100.0, 0.0]);
    let low = ends(&h, 0);
    assert!(low[0][1] < -50.0 && low[0][0] == 0.0 && low[0][2] == 1000.0, "{low:?} {}", status(&h));
    assert_eq!(low[5][0], 0.0);
    // Undo takes back the bottom's drag alone, then the floor's; Redo does the floor's again.
    h.state_mut().family_undo();
    h.run();
    assert_eq!(ends(&h, 0), after);
    h.state_mut().family_undo();
    h.run();
    assert_eq!(ends(&h, 0), before);
    h.state_mut().family_redo();
    h.run();
    assert_eq!(ends(&h, 0), after);
}

#[test]
fn a_locked_or_labelled_edge_and_a_drag_leaving_no_profile_are_refused_and_the_form_stays() {
    let mut h = in_plan();
    select(&mut h, 0);
    let before = ends(&h, 0);
    drag_handle(&mut h, [0.0, 500.0, 0.0], [-200.0, 0.0, 0.0]);
    assert!(status(&h).contains("locked"), "{}", status(&h));
    assert_eq!(ends(&h, 0), before);
    drag_handle(&mut h, [1000.0, 250.0, 0.0], [200.0, 0.0, 0.0]);
    assert!(status(&h).contains("Len"), "{}", status(&h));
    assert_eq!(ends(&h, 0), before);
    // The notch's wall (x 500) right past the east side: the floor would turn back on itself.
    drag_handle(&mut h, [500.0, 750.0, 0.0], [800.0, 0.0, 0.0]);
    assert!(status(&h).contains("can't take"), "{}", status(&h));
    assert_eq!(ends(&h, 0), before);
}

#[test]
fn a_voids_edge_and_an_arcs_radius_drag() {
    let mut h = in_plan();
    select(&mut h, 1);
    assert_eq!(shown(&h).len(), 4, "a void's handles");
    drag_handle(&mut h, [1900.0, 200.0, 0.0], [300.0, 0.0, 0.0]);
    let v = ends(&h, 1);
    assert!((v[1][0] - 2200.0).abs() < 15.0 && v[1][0] == v[1][2], "{v:?} {}", status(&h));
    // The slot's round end, its handle at the arc's middle (-400, -700): out 50, the radius 150 about the same centre.
    select(&mut h, 2);
    drag_handle(&mut h, [-400.0, -700.0, 0.0], [50.0, 0.0, 0.0]);
    let s = ends(&h, 2);
    let r = (s[1][1] - s[1][3]).abs() / 2.0;
    assert!((r - 150.0).abs() < 15.0 && s[1][0] == -500.0 && s[1][2] == -500.0, "{s:?} {}", status(&h));
    // The straight sides' shared ends follow the arc's, their far ends staying.
    assert!(near(s[0], [-1000.0, -800.0, -500.0, -700.0 - r], 1e-3), "{s:?}");
    assert!(near(s[2], [-500.0, -700.0 + r, -1000.0, -600.0], 1e-3), "{s:?}");
}

#[test]
fn an_elevation_shows_the_sides_it_sees_edge_on_and_they_drag_the_same_edge() {
    let mut h = in_plan();
    view(&mut h, Some(Which::Elevation(Side::Front)));
    select(&mut h, 0);
    // Front looks north: the sides facing east and west are seen edge-on, at mid-depth (150): the west (locked) and the east
    // (Len's); the notch's wall is behind the L's foot.
    assert_eq!(shown(&h).len(), 2, "{:?}", shown(&h));
    assert!(!handle_at(&h, [0.0, 0.0, 150.0]).1 && !handle_at(&h, [1000.0, 0.0, 150.0]).1);
    // The Start and End handles show as well.
    assert_eq!(editor(&h).views.ends_shown.len(), 2);
    // Back looks south, across the notch: its wall too, which drags the same edge as in plan.
    view(&mut h, Some(Which::Elevation(Side::Back)));
    select(&mut h, 0);
    assert_eq!(shown(&h).len(), 3, "{:?}", shown(&h));
    drag_handle(&mut h, [500.0, 1000.0, 150.0], [-200.0, 0.0, 0.0]);
    let e = ends(&h, 0);
    assert!((e[3][0] - 300.0).abs() < 15.0 && e[3][0] == e[3][2] && e[2][2] == e[3][0], "{e:?} {}", status(&h));
}

#[test]
fn a_blends_base_has_handles_in_plan_its_top_staying_and_its_ends_in_an_elevation() {
    let base = r#"[{"start": [0, 0], "end": [600, 0]}, {"start": [600, 0], "end": [600, 600]}, {"start": [600, 600], "end": [0, 600]}, {"start": [0, 600], "end": [0, 0]}]"#;
    let top = r#"[{"start": [200, 200], "end": [400, 200]}, {"start": [400, 200], "end": [400, 400]}, {"start": [400, 400], "end": [200, 400]}, {"start": [200, 400], "end": [200, 200]}]"#;
    let script = format!(
        r#"family {{"name": "Hopper", "category": "generic", "types": {{"T": {{}}}}, "forms": [{{"sketch": {base}, "top": {top}, "from": 0, "to": 800}}]}}"#
    );
    let (mut h, _) = open(&script, "Hopper");
    view(&mut h, Some(Which::Plan));
    select(&mut h, 0);
    assert_eq!(shown(&h).len(), 4, "the base's edges");
    let top_before = editor(&h).draft_family().unwrap().forms[0].top.clone();
    drag_handle(&mut h, [600.0, 300.0, 0.0], [200.0, 0.0, 0.0]);
    let b = ends(&h, 0);
    assert!((b[1][0] - 800.0).abs() < 15.0 && b[1][0] == b[1][2] && b[0][2] == b[1][0], "{b:?} {}", status(&h));
    assert_eq!(editor(&h).draft_family().unwrap().forms[0].top, top_before, "the top stays");
    // In an elevation: its First and Second End's handles, no side handles (its sides lean).
    view(&mut h, Some(Which::Elevation(Side::Front)));
    assert!(shown(&h).is_empty());
    assert_eq!(editor(&h).views.ends_shown.len(), 2);
}

#[test]
fn an_end_a_lock_holds_is_greyed_and_refuses_a_drag() {
    let script = format!(
        r#"family {{"name": "Slab", "category": "generic", "types": {{"T": {{}}}}, "ref_planes": [{{"guid": "{WEST}", "name": "Top", "start": [-2000, 0, 300], "end": [2000, 0, 300], "up": [0, 1, 0]}}], "forms": [{{"guid": "{L}", "profile": [[0, 0], [400, 0], [400, 400], [0, 400]], "from": 0, "to": 300}}], "dimensions": [{{"refs": [{{"plane": "{WEST}"}}, {{"form": "{L}", "end": "end"}}], "view": "front", "at": [600, 0, 300], "locked": [{{"segment": 0, "length": 0}}], "alignment": true}}]}}"#
    );
    let (mut h, _) = open(&script, "Slab");
    view(&mut h, Some(Which::Elevation(Side::Front)));
    select(&mut h, 0);
    let ends_shown = editor(&h).views.ends_shown.clone();
    assert_eq!(ends_shown.iter().map(|e| e.1).collect::<Vec<_>>(), [true, false], "Start free, End locked to Top");
    drag_screen(&mut h, ends_shown[1].0, ends_shown[1].0 - vec2(0.0, 40.0), PointerButton::Primary);
    assert!(status(&h).contains("locked"), "{}", status(&h));
    assert_eq!(editor(&h).draft_family().unwrap().forms[0].to, Expr::Number(300.0));
}

/// The L selected in plan with its handles, the same after its notch's floor is dragged up, and Front with its side and end
/// handles (Back, which sees the notch's wall across it), saved where `BIMCRAFT_FE5_DUMP` says (only then).
#[test]
fn family_shape_handles_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE5_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let mut h = app(&family());
    h.set_size(vec2(1600.0, 1000.0));
    h.state_mut().edit_family("Ell");
    h.run();
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe5-{name}.png"))).unwrap();
    };
    view(&mut h, Some(Which::Plan));
    select(&mut h, 0);
    save(&mut h, "plan");
    drag_handle(&mut h, [750.0, 500.0, 0.0], [0.0, 200.0, 0.0]);
    save(&mut h, "plan-dragged");
    view(&mut h, Some(Which::Elevation(Side::Back)));
    select(&mut h, 0);
    save(&mut h, "elevation");
}
