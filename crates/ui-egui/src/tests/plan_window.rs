//! GEO-2g: a plan builds and draws only the components that meet its window, its screen and one more screen each way. After
//! a pan, what is by the screen's edge is still there to hover, pick and reach with Tab, and what is just past it to snap to.
use super::*;
use bimcraft_core::snap::Kind;

/// A row of tables 4 m apart along a wall, far wider than the screen.
fn row() -> Harness<'static, BimApp> {
    let mut script = String::from("wall {\"points\": [[-2000, 2600], [400000, 2600]]}\n");
    for k in 0..100 {
        script += &format!("component {{\"family\": \"Table\", \"type\": \"1200 x 800\", \"at\": [{}, 3000]}}\n", k * 4000);
    }
    let mut h = app(&script);
    // Zoomed in on the row's start, some five tables to a screen.
    let rect = h.state().canvas.rect;
    for _ in 0..60 {
        let span = h.state().screen(P2(4000.0, 3000.0)).x - h.state().screen(P2(0.0, 3000.0)).x;
        if span > rect.width() / 5.0 {
            break;
        }
        h.hover_at(h.state().screen(P2(0.0, 3000.0)));
        h.event(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, 120.0),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        h.run();
    }
    let span = h.state().screen(P2(4000.0, 3000.0)).x - h.state().screen(P2(0.0, 3000.0)).x;
    assert!(span > rect.width() / 5.0 && span < rect.width() / 2.0, "zoomed in: {span} px a table");
    h
}

fn tables(h: &Harness<BimApp>) -> Vec<(Id, P2)> {
    h.state()
        .session
        .model
        .elements
        .iter()
        .filter_map(|(id, e)| match e {
            Element::Component(c) => Some((*id, c.at)),
            _ => None,
        })
        .collect()
}

#[test]
fn after_a_pan_a_table_by_the_screens_edge_is_hovered_picked_and_reached_with_tab() {
    let mut h = row();
    let rect = h.state().canvas.rect;
    // Half a screen to the left: what was off the screen's right comes on it.
    drag_screen(&mut h, rect.center(), rect.center() - vec2(rect.width() / 2.0, 0.0), PointerButton::Middle);
    h.run();
    // The table whose middle is nearest inside the screen's right edge.
    let (table, at) = tables(&h)
        .into_iter()
        .filter(|(_, at)| h.state().screen(*at).x < rect.max.x - 20.0)
        .max_by(|a, b| a.1.0.total_cmp(&b.1.0))
        .unwrap_or_else(|| panic!("a table on the screen"));
    assert!(h.state().screen(at).x > rect.max.x - rect.width() / 4.0, "by the edge: {:?} of {rect:?}", h.state().screen(at));
    // Its south edge, along the wall's middle: the table and the wall under the pointer.
    let edge = P2(at.0, 2600.0);
    h.hover_at(h.state().screen(edge));
    h.run();
    let wall = find(&h, |e| matches!(e, Element::Wall(_)));
    let under = h.state().canvas.under.clone();
    assert!(under.contains(&table) && under.contains(&wall), "hovered: {under:?}");
    click(&mut h, edge);
    let first = h.state().selection.clone();
    h.hover_at(h.state().screen(edge));
    h.run();
    key(&mut h, Key::Tab);
    click(&mut h, edge);
    let tabbed = h.state().selection.clone();
    let mut both = [first, tabbed].concat();
    both.sort_unstable();
    let mut expected = vec![table, wall];
    expected.sort_unstable();
    assert_eq!(both, expected, "a click takes one, Tab the other");
}

#[test]
fn after_a_pan_a_table_just_past_the_screens_edge_is_snapped_to() {
    let mut h = row();
    h.state_mut().snaps.mark_on_hover = false;
    let rect = h.state().canvas.rect;
    // A table well off the screen's right, brought by a pan to just past its edge.
    let (_, at) = tables(&h)
        .into_iter()
        .find(|(_, at)| h.state().screen(*at).x > rect.max.x + rect.width() / 2.0)
        .unwrap_or_else(|| panic!("a table off the screen"));
    let from = h.state().screen(at);
    let by = from.x - (rect.max.x + 3.0);
    drag_screen(&mut h, rect.center(), rect.center() - vec2(by, 0.0), PointerButton::Middle);
    h.run();
    let now = h.state().screen(at);
    assert!(now.x > rect.max.x && now.x < rect.max.x + 6.0, "just past the edge: {now:?} of {rect:?}");
    tool(&mut h, "Wall");
    h.hover_at(pos2(rect.max.x - 2.0, now.y));
    h.run();
    let found = h.state().canvas.found.get().map(|f| (f.kind, f.at));
    assert_eq!(found, Some((Kind::Centre, at)), "its middle");
}
