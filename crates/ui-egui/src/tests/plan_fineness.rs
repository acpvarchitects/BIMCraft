//! GEO-2c-3: a plan zoomed in draws a component's curves finer, and a finer curve that reaches past its default build is
//! there to hover and to click in the canvas, which picks on the plan the core draws (`plan_fineness_tests` in the core).
use super::*;

/// A slab hung from 2000 mm whose bottom is an arc of 30° 1000 mm wide, bulging down to 5.8 mm under Level 1's cut plane: its
/// default build's three chords stop 1.5 mm over it.
const VAULT: &str = r#"family {"name": "Vault", "category": "generic", "types": {"T": {}}, "forms": [{"sketch": [{"start": [-500, 1260], "end": [500, 1260], "turn": 30}, {"start": [500, 1260], "end": [500, 2000]}, {"start": [500, 2000], "end": [-500, 2000]}, {"start": [-500, 2000], "end": [-500, 1260]}], "host": {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2"}, "from": -1000, "to": 0}]}
component {"family": "Vault", "type": "T", "at": [0, 0]}"#;

/// Zoomed in to 2 px a mm on its south edge over its lowest, the plan draws it finer, under the cut: the pointer there hovers
/// it, and a click takes it.
#[test]
fn a_finer_curve_dipping_under_the_cut_plane_is_hovered_and_picked_zoomed_in() {
    let mut h = app(VAULT);
    let vault = find(&h, |e| matches!(e, Element::Component(_)));
    let level = match h.state().sheet {
        Sheet::Plan(level) => level,
        _ => panic!("a plan"),
    };
    h.state_mut().canvas.set_view(level, 0.0, 0.0, 2.0);
    h.run();
    hover(&mut h, P2(0.0, 0.0));
    assert_eq!(h.state().canvas.under, [vault], "the pointer on its edge hovers it");
    click(&mut h, P2(0.0, 0.0));
    assert_eq!(h.state().selection, [vault], "a click there takes it");
}
