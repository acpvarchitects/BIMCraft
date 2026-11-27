//! GEO-2c-2e: in the family document's plan, zoomed in on a curve the plan draws finer than its default chords (GEO-2c), a
//! click selects the form where the plan draws it: inside the circle but outside a default chord, by its face; on the circle
//! drawn, by its edge. Neither click is near a default chord.
use super::family_box_select::selected;
use super::family_views::{click_family, editor, open, view};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::views::Which;

/// A drum 2 m across, through the plan's cut.
const ROUND: &str = r#"family {"name": "Round", "category": "generic", "types": {"T": {}}, "forms": [{"sketch": [{"centre": [0, 0], "radius": 1000}], "from": 0, "to": 2000}]}"#;

/// How many points on screen a millimetre of the plan shown is, at `q`.
fn scale_at(h: &Harness<BimApp>, q: [f64; 3]) -> f32 {
    let views = &editor(h).views;
    let (a, b) = (views.screen_of(q).expect("the plan shown"), views.screen_of([q[0] + 1.0, q[1], q[2]]).expect("the plan shown"));
    a.distance(b)
}

#[test]
fn zoomed_in_on_a_curve_a_click_where_the_plan_draws_it_selects_the_form() {
    let mut h = if std::env::var_os("BIMCRAFT_FAMILY_PICK_DUMP").is_some() {
        let mut h = app(ROUND);
        h.set_size(vec2(1600.0, 1200.0));
        h.state_mut().edit_family("Round");
        h.run();
        h
    } else {
        open(ROUND, "Round").0
    };
    view(&mut h, Some(Which::Plan));
    // A default chord's middle is 4.8 mm inside the circle: halfway across that gap, and on the circle.
    let (sin, cos) = (std::f64::consts::PI / 32.0).sin_cos();
    let (inside, on) = ([cos * 997.6, sin * 997.6, 0.0], [cos * 1000.0, sin * 1000.0, 0.0]);
    // Zoom In Region round it, as far as the view zooms (50 times its fit): a millimetre some 8 points on screen, so the default
    // chord is some 19 points from either click, well past the reach of a pick.
    for _ in 0..4 {
        if scale_at(&h, inside) > 7.5 {
            break;
        }
        let c = editor(&h).views.screen_of(inside).expect("the plan shown");
        shortcut(&mut h, "ZR");
        drag_screen(&mut h, c + vec2(-60.0, -45.0), c + vec2(60.0, 45.0), PointerButton::Primary);
        h.run();
    }
    let scale = scale_at(&h, inside);
    assert!(scale > 6.0, "zoomed in: {scale} points a mm");
    for (by_face, at) in [(true, inside), (false, on)] {
        h.state_mut().select_by_face = by_face;
        h.run();
        key(&mut h, Key::Escape);
        click_family(&mut h, at);
        assert_eq!(selected(&h), vec![Picked::Form(0)], "a click at {at:?}, by face {by_face}, at {scale} points a mm");
        capture_pick(&mut h, if by_face { "round-plan-inside" } else { "round-plan-edge" });
    }
}

/// Save the actual desktop view while it still shows the result of the click or crossing.
fn capture_pick(h: &mut Harness<BimApp>, state: &str) {
    let Some(folder) = std::env::var_os("BIMCRAFT_FAMILY_PICK_DUMP") else { return };
    let folder = std::path::Path::new(&folder);
    std::fs::create_dir_all(folder).unwrap();
    for (name, theme, light_canvas) in
        [("light", egui::Theme::Light, false), ("dark", egui::Theme::Dark, false), ("dark-light", egui::Theme::Dark, true)]
    {
        h.ctx.set_theme(theme);
        h.state_mut().light_canvas = light_canvas;
        h.run();
        h.run();
        h.render().unwrap().save(folder.join(format!("{state}-{name}.png"))).unwrap();
    }
}

fn open_pick_view(script: &str, name: &str, which: Which) -> Harness<'static, BimApp> {
    let mut h = app(script);
    h.set_size(vec2(1600.0, 1000.0));
    h.state_mut().edit_family(name);
    h.run();
    view(&mut h, Some(which));
    h
}

/// Centre a detail on the canvas so the ordinary pick tolerance can distinguish close parallel edges.
fn zoom_to_detail(h: &mut Harness<BimApp>, at: [f64; 3]) {
    let point = editor(h).views.screen_of(at).unwrap();
    shortcut(h, "ZR");
    drag_screen(h, point - vec2(130.0, 100.0), point + vec2(130.0, 100.0), PointerButton::Primary);
    h.run();
}

#[test]
fn a_wall_in_the_front_elevation_hides_the_flange_until_its_category_is_hidden() {
    let script = r#"family {"name":"Jamb and flange","category":"window","parameters":[{"name":"Width","value":1200},{"name":"Height","value":1500}],"types":{"A":{}},"forms":[{"profile":[[440,-30],[560,-30],[560,30],[440,30]],"from":400,"to":1000},{"profile":[[420,240],[860,240],[860,280],[420,280]],"from":400,"to":1000}]}"#;
    let mut h = open_pick_view(script, "Jamb and flange", Which::Elevation(bimcraft_core::family::views::Side::Front));
    h.state_mut().select_by_face = true;
    zoom_to_detail(&mut h, [620., 0., 700.]);
    click_family(&mut h, [500., 0., 700.]);
    let jamb = selected(&h);
    capture_pick(&mut h, "visible-jamb");
    click_family(&mut h, [750., 0., 700.]);
    let concealed = selected(&h);
    capture_pick(&mut h, "flange-behind-wall");
    shortcut(&mut h, "VG");
    pick(&mut h, Role::CheckBox, "Walls");
    press(&mut h, "OK");
    click_family(&mut h, [750., 0., 700.]);
    let revealed = selected(&h);
    capture_pick(&mut h, "flange-with-wall-hidden");
    assert_eq!(jamb, [Picked::Form(0)], "the material inside the opening");
    assert!(!concealed.contains(&Picked::Form(1)), "the host covers the rear flange");
    assert_eq!(revealed, [Picked::Form(1)], "the same click after hiding Walls");
}

#[test]
fn a_crossing_over_a_rear_forms_edge_selects_its_exposed_portion_only() {
    let script = r#"family {"name":"Panels","category":"generic","types":{"A":{}},"forms":[{"profile":[[-200,-80],[140,-80],[140,-50],[-200,-50]],"from":-100,"to":140},{"profile":[[-100,350],[300,350],[300,375],[-100,375]],"from":-75,"to":115}]}"#;
    let mut h = open_pick_view(script, "Panels", Which::Elevation(bimcraft_core::family::views::Side::Front));
    h.state_mut().select_by_face = false;
    zoom_to_detail(&mut h, [110., 0., 100.]);
    let mut selections = Vec::new();
    for (name, x) in [("rear-edge-covered", 30.), ("rear-edge-exposed", 205.)] {
        key(&mut h, Key::Escape);
        super::family_box_select::boxed(&mut h, [x + 18., 350., 132.], [x - 18., 350., 99.], Modifiers::NONE);
        selections.push(selected(&h));
        capture_pick(&mut h, name);
    }
    assert!(!selections[0].contains(&Picked::Form(1)), "the crossing is entirely behind the front panel");
    assert_eq!(selections[1], [Picked::Form(1)], "the rear edge beyond the front panel");
}

#[test]
fn a_window_plan_can_pick_its_sill_between_the_two_closed_tiers() {
    let script = r#"family {"name":"Window tiers","category":"window","parameters":[{"name":"Width","value":1200},{"name":"Height","value":1800}],"types":{"T":{}},"forms":[{"profile":[[-300,-80],[300,-80],[300,80],[-300,80]],"from":100,"to":1700},{"profile":[[-500,-150],[500,-150],[500,150],[-500,150]],"from":800,"to":1450,"void":true,"cuts":"forms"},{"profile":[[-450,-15],[450,-15],[450,15],[-450,15]],"from":950,"to":1000}]}"#;
    let mut h = open_pick_view(script, "Window tiers", Which::Plan);
    h.state_mut().select_by_face = true;
    let inside = [160., 8., 0.];
    zoom_to_detail(&mut h, inside);
    zoom_to_detail(&mut h, inside);
    click_family(&mut h, inside);
    let hit = selected(&h);
    capture_pick(&mut h, "sill-in-gap");
    assert_eq!(hit, [Picked::Form(2)], "the sill takes the click in the gap");
}
