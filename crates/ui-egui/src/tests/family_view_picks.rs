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
    let (mut h, _) = open(ROUND, "Round");
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
    }
}
