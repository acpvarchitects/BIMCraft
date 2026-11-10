//! FE-3: Revit's zoom commands in the family document's views, its plans, ceiling plan, elevations and {3D}. Zoom to Fit (ZF,
//! ZE, ZX, ZA) fits the view shown; Zoom Out (2x) (ZO, ZV) halves it about its middle; Zoom In Region (ZR, ZZ) fits the
//! rectangle dragged, and Escape gives it up; Previous Pan/Zoom (ZP, ZC) steps back through the cameras the commands left in
//! that view, the wheel's ticks not among them. The view control bar has them beside Zoom to Fit. None of them touches the
//! project's views, and a key typed on the project's tab never touches the family's.
use super::family_views::{editor, view};
use super::*;
use bimcraft_core::family::views::{Side, Which};
use bimcraft_core::view3d::Camera;

/// A box from Ref. Level up to 600, 400 by 300.
const FAMILY: &str = r#"family {"name": "Box", "category": "generic", "types": {"T": {}}, "forms": [{"profile": [[0, 0], [400, 0], [400, 300], [0, 300]], "from": 0, "to": 600}]}"#;

/// Two of the family's points, apart in plan and in every elevation: where they are on screen says how the view is looked at.
const A: [f64; 3] = [0.0, 0.0, 0.0];
const B: [f64; 3] = [400.0, 300.0, 600.0];

/// The views of a family that isn't adaptive, each kind once: none is {3D}.
const VIEWS: [Option<Which>; 4] = [Some(Which::Plan), Some(Which::Ceiling), Some(Which::Elevation(Side::Front)), None];

/// The project's four walls, one of them selected, and its family Box open in its document over them.
fn open() -> Harness<'static, BimApp> {
    let mut h = app(&format!("{BOX}\n{FAMILY}"));
    h.set_size(vec2(1600.0, 1100.0));
    h.run();
    let wall = ids(&h, "wall")[0];
    h.state_mut().selection = vec![wall];
    h.state_mut().edit_family("Box");
    h.run();
    h
}

/// Where the family's point `q` is on screen, in the view shown: a plan's or an elevation's drawing, or {3D}'s picture.
fn on_screen(h: &Harness<BimApp>, q: [f64; 3]) -> Pos2 {
    let ed = editor(h);
    match ed.views.shown {
        Some(_) => ed.views.screen_of(q).expect("a plan or an elevation drawn"),
        None => ed.drawn_3d().expect("{3D} drawn").screen(h.state().view_rect, q),
    }
}

/// How the view shown is looked at: where [`A`] and [`B`] are on screen.
fn look(h: &Harness<BimApp>) -> [Pos2; 2] {
    [on_screen(h, A), on_screen(h, B)]
}

/// The middle of the view, which Zoom Out keeps.
fn middle(h: &Harness<BimApp>) -> Pos2 {
    h.state().view_rect.center()
}

fn near(a: Pos2, b: Pos2, slack: f32, what: &str) {
    assert!(a.distance(b) <= slack, "{what}: {a:?} is not {b:?}");
}

fn same(a: [Pos2; 2], b: [Pos2; 2], what: &str) {
    for (p, q) in a.into_iter().zip(b) {
        near(p, q, 1.5, what);
    }
}

/// The project as a family's command must leave it: its plan and its 3D view looked at as before, its selection and tool.
#[derive(Debug, PartialEq)]
struct Project {
    plan: [Pos2; 2],
    orbit: Option<Camera>,
    selection: usize,
    tool: Tool,
}

fn project(h: &Harness<BimApp>) -> Project {
    let app = h.state();
    Project {
        plan: [app.screen(P2(0.0, 0.0)), app.screen(P2(8000.0, 6000.0))],
        orbit: app.orbit.camera,
        selection: app.selection.len(),
        tool: app.tool.clone(),
    }
}

/// A rectangle in the view, clear of its tabs, its ViewCube and its control bar, a little left of and above its middle.
fn rectangle(h: &Harness<BimApp>) -> (Pos2, Pos2) {
    let c = middle(h);
    (c + vec2(-160.0, -90.0), c + vec2(90.0, 50.0))
}

/// Zoom In Region, dragged from `a` to `b`.
fn region(h: &mut Harness<BimApp>, keys: &str, a: Pos2, b: Pos2) {
    shortcut(h, keys);
    drag_screen(h, a, b, PointerButton::Primary);
    h.run();
}

/// The wheel turned over the view's middle: no step of Previous Pan/Zoom.
fn wheel(h: &mut Harness<BimApp>) {
    h.hover_at(middle(h) + vec2(40.0, 30.0));
    h.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, 120.0),
        phase: TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    h.run();
}

/// Frames for {3D} to fit itself on its worker.
fn settle(h: &mut Harness<BimApp>) {
    for _ in 0..20 {
        h.run();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn zoom_out_halves_each_view_about_its_middle() {
    let mut h = open();
    for which in VIEWS {
        view(&mut h, which);
        settle(&mut h);
        for keys in ["ZO", "ZV"] {
            let before = look(&h);
            shortcut(&mut h, keys);
            let c = middle(&h);
            let after = look(&h);
            for (p, q) in before.into_iter().zip(after) {
                near(q, c + (p - c) * 0.5, 1.5, &format!("{keys} in {which:?}"));
            }
        }
    }
}

#[test]
fn zoom_in_region_fits_the_rectangle_dragged_in_each_view() {
    let mut h = open();
    for which in VIEWS {
        for keys in ["ZR", "ZZ"] {
            view(&mut h, which);
            shortcut(&mut h, "ZF");
            settle(&mut h);
            let before = look(&h);
            let (a, b) = rectangle(&h);
            let azimuth = editor(&h).orbit.camera.map(|c| c.azimuth);
            region(&mut h, keys, a, b);
            // The rectangle's middle comes to the view's, and the rectangle fills it as far as it can without being cut.
            let pane = h.state().view_rect;
            let m = a + (b - a) / 2.0;
            let f = (pane.width() / (b.x - a.x)).min(pane.height() / (b.y - a.y));
            let after = look(&h);
            for (p, q) in before.into_iter().zip(after) {
                near(q, pane.center() + (p - m) * f, 2.0 + 0.01 * (p - m).length() * f, &format!("{keys} in {which:?}"));
            }
            assert_eq!(editor(&h).orbit.camera.map(|c| c.azimuth), azimuth, "{keys}: the drag doesn't turn {which:?}");
            // Once fitted, the region is done: another drag is the view's own.
            let again = look(&h);
            drag_screen(&mut h, a, b, PointerButton::Middle);
            assert_ne!(look(&h), again, "{keys}: the middle button pans {which:?} again");
        }
    }
}

#[test]
fn escape_gives_up_the_region_and_nothing_else() {
    let mut h = open();
    view(&mut h, Some(Which::Plan));
    super::family_views::select_form(&mut h, 1);
    assert!(editor(&h).selected_form().is_some());
    let before = look(&h);
    shortcut(&mut h, "ZR");
    key(&mut h, Key::Escape);
    assert!(editor(&h).selected_form().is_some(), "the form stays selected: Escape gave up the region alone");
    let (a, b) = rectangle(&h);
    drag_screen(&mut h, a, b, PointerButton::Primary);
    same(look(&h), before, "a drag after Escape zooms nothing");
}

#[test]
fn previous_steps_back_through_the_zooms_of_the_view_shown_and_not_the_wheels() {
    let mut h = open();
    for which in VIEWS {
        view(&mut h, which);
        shortcut(&mut h, "ZF");
        settle(&mut h);
        let fitted = look(&h);
        shortcut(&mut h, "ZO");
        let out = look(&h);
        let (a, b) = rectangle(&h);
        region(&mut h, "ZR", a, b);
        let zoomed = look(&h);
        assert_ne!(zoomed, out);
        wheel(&mut h);
        assert_ne!(look(&h), zoomed, "the wheel zooms {which:?}");
        shortcut(&mut h, "ZP");
        same(look(&h), out, &format!("ZP in {which:?}: before the region"));
        shortcut(&mut h, "ZC");
        same(look(&h), fitted, &format!("ZC in {which:?}: before Zoom Out"));
        // Fit, the first command of all here, is where it stops.
        for _ in 0..3 {
            shortcut(&mut h, "ZP");
        }
        let first = look(&h);
        shortcut(&mut h, "ZP");
        same(look(&h), first, &format!("ZP in {which:?} with nothing before"));
    }
    // Each view keeps its own: Zoom Out in the plan is no step of the elevation's.
    view(&mut h, Some(Which::Elevation(Side::Front)));
    let elevation = look(&h);
    view(&mut h, Some(Which::Plan));
    let plan = look(&h);
    shortcut(&mut h, "ZO");
    view(&mut h, Some(Which::Elevation(Side::Front)));
    shortcut(&mut h, "ZP");
    same(look(&h), elevation, "ZP in the elevation after Zoom Out in the plan");
    view(&mut h, Some(Which::Plan));
    shortcut(&mut h, "ZP");
    same(look(&h), plan, "ZP in the plan");
}

#[test]
fn zoom_to_fit_fits_the_view_shown() {
    let mut h = open();
    for which in VIEWS {
        view(&mut h, which);
        shortcut(&mut h, "ZF");
        settle(&mut h);
        let fitted = look(&h);
        for keys in ["ZF", "ZE", "ZX", "ZA"] {
            // Zoomed by the wheel, which a plan's own fit used to leave as it was.
            wheel(&mut h);
            assert_ne!(look(&h), fitted, "the wheel zooms {which:?}");
            shortcut(&mut h, keys);
            settle(&mut h);
            same(look(&h), fitted, &format!("{keys} in {which:?}"));
        }
    }
}

#[test]
fn the_view_control_bar_has_them_beside_zoom_to_fit() {
    let mut h = open();
    view(&mut h, Some(Which::Plan));
    let fitted = look(&h);
    press(&mut h, "Zoom Out (2x)");
    let c = middle(&h);
    near(look(&h)[1], c + (fitted[1] - c) * 0.5, 1.5, "Zoom Out (2x)");
    let out = look(&h);
    press(&mut h, "Zoom In Region");
    let (a, b) = rectangle(&h);
    drag_screen(&mut h, a, b, PointerButton::Primary);
    assert!((look(&h)[1] - look(&h)[0]).length() > (out[1] - out[0]).length() * 2.0, "Zoom In Region zoomed in");
    press(&mut h, "Previous Pan/Zoom");
    same(look(&h), out, "Previous Pan/Zoom");
    // In a row, after Zoom to Fit, in the bar at the view's bottom left.
    let bar: Vec<_> = ["Zoom to Fit", "Zoom In Region", "Zoom Out (2x)", "Previous Pan/Zoom"].map(|l| h.get_by_label(l).rect()).into();
    for w in bar.windows(2) {
        assert!(w[1].left() >= w[0].right() - 0.5 && (w[1].center().y - w[0].center().y).abs() < 1.0, "{bar:?}");
    }
    assert!(bar[0].bottom() > h.state().view_rect.bottom() - 40.0, "at the bottom");
}

#[test]
fn an_adaptive_familys_modelling_view_zooms_too() {
    let (mut h, ..) = super::family_temporary_hide::adaptive();
    h.run();
    let cam = |h: &Harness<BimApp>| editor(h).modeller.camera.expect("the modelling view drawn");
    let before = cam(&h);
    shortcut(&mut h, "ZO");
    let out = cam(&h);
    assert!((out.scale / before.scale - 0.5).abs() < 1e-6, "ZO halves it: {} then {}", before.scale, out.scale);
    let c = middle(&h);
    region(&mut h, "ZR", c + vec2(-100.0, -60.0), c + vec2(60.0, 40.0));
    let zoomed = cam(&h);
    assert!(zoomed.scale > out.scale * 3.0, "ZR fits the rectangle: {} then {}", out.scale, zoomed.scale);
    assert_eq!((zoomed.azimuth, zoomed.elevation), (out.azimuth, out.elevation), "the drag doesn't turn it");
    shortcut(&mut h, "ZP");
    assert_eq!(cam(&h), out, "ZP");
    shortcut(&mut h, "ZP");
    assert_eq!(cam(&h), before, "ZP again");
}

#[test]
fn a_family_zoom_never_touches_the_projects_views_nor_a_projects_key_the_familys() {
    let mut h = open();
    let untouched = project(&h);
    for which in VIEWS {
        view(&mut h, which);
        for keys in ["ZO", "ZV", "ZF", "ZP", "ZC"] {
            shortcut(&mut h, keys);
            assert_eq!(project(&h), untouched, "{keys} in the family's {which:?}");
        }
        let (a, b) = rectangle(&h);
        region(&mut h, "ZR", a, b);
        assert_eq!(project(&h), untouched, "ZR in the family's {which:?}");
    }
    // On the project's tab, the keys leave the family's plan as it was, and arm nothing there.
    view(&mut h, Some(Which::Plan));
    let family = look(&h);
    press(&mut h, "Level 1");
    assert!(h.state().active_family().is_none(), "the project's tab");
    for keys in ["ZO", "ZV", "ZR", "ZZ", "ZP", "ZC"] {
        h.state_mut().typed.clear();
        shortcut(&mut h, keys);
    }
    press(&mut h, "Box - Floor Plan: Ref. Level");
    same(look(&h), family, "the family's plan after the project's keys");
    let (a, b) = rectangle(&h);
    drag_screen(&mut h, a, b, PointerButton::Primary);
    same(look(&h), family, "no region was armed from the project's tab");
}

/// The new buttons in the view control bar and a region being dragged, in the plan and in {3D}, saved where
/// `BIMCRAFT_FE3_DUMP` says (only then).
#[test]
fn family_zoom_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_FE3_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let mut h = open();
    for (name, which) in [("plan", Some(Which::Plan)), ("3d", None)] {
        view(&mut h, which);
        settle(&mut h);
        shortcut(&mut h, "ZR");
        let (a, b) = rectangle(&h);
        h.hover_at(a);
        h.run();
        h.event(Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        h.run();
        for t in [0.5, 1.0] {
            h.hover_at(a + (b - a) * t);
            h.run();
        }
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe3-{name}-region.png"))).unwrap();
        h.event(Event::PointerButton { pos: b, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        h.run();
        settle(&mut h);
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("fe3-{name}-zoomed.png"))).unwrap();
    }
    // The bar with the pointer over Zoom Out (2x), its tip shown.
    let at = h.get_by_label("Zoom Out (2x)").rect().center();
    h.hover_at(at);
    for _ in 0..6 {
        h.run();
    }
    h.render().unwrap().save(std::path::Path::new(&folder).join("fe3-bar.png")).unwrap();
}
