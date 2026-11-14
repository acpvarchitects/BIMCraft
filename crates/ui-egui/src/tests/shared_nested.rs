//! T6b: a shared nested instance in the project, Tab-selected inside its host in a plan; and a nested instance dragged in
//! its family's plan and elevation, as the family's other parts are (FE-7), honouring a pin and a formula, one step undone.
use super::family_drag_move::{drag, ed, family, family_with, near, pick, plan_of, press_and_move, release, status, untouched};
use super::family_views::{editor, view};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::Expr;
use bimcraft_core::family::views::{Side, Which};

/// A cabinet 600 by 400 whose handle, a shared family, is placed at (100, 50), as wide as a tenth of the cabinet; a
/// component of it at (5000, 0), turned 90 degrees.
const CABINET: &str = r#"family {"name": "Cabinet", "category": "furniture", "parameters": [{"name": "Width", "value": 600}], "types": {"Small": {"Width": 600}}, "forms": [{"profile": [[0, 0], ["Width", 0], ["Width", 400], [0, 400]], "to": 800}]}
family {"name": "Cabinet", "nest": {"name": "Handle", "definition": {"category": "generic", "shared": true, "parameters": [{"name": "W", "value": 40}], "types": {"Lever": {}}, "forms": [{"profile": [[0, 0], ["W", 0], ["W", 20], [0, 20]], "to": 30}]}}}
family {"name": "Cabinet", "nested_instance": {"family": "Handle", "type": "Lever", "at": [100, 50], "params": {"W": "Width / 10"}}}
component {"family": "Cabinet", "type": "Small", "at": [5000, 0], "angle": 90}"#;

/// The handle's middle in the project: (130, 60) in the cabinet's axes, turned 90 degrees about (5000, 0).
const HANDLE: P2 = P2(4940.0, 130.0);

fn handle(h: &Harness<BimApp>) -> Id {
    find(h, |e| matches!(e, Element::Nested(_)))
}

#[test]
fn a_click_on_a_shared_nested_instance_takes_its_host_and_tab_reaches_it() {
    let mut h = app(CABINET);
    let cabinet = find(&h, |e| matches!(e, Element::Component(_)));
    let handle = handle(&h);
    click(&mut h, HANDLE);
    assert_eq!(h.state().selection, [cabinet], "a click takes the cabinet");
    // Over it, Tab moves on to the handle, and a click takes that.
    h.hover_at(h.state().screen(HANDLE));
    h.run();
    key(&mut h, Key::Tab);
    click(&mut h, HANDLE);
    assert_eq!(h.state().selection, [handle]);
    // Its properties say what it is; Delete leaves it, saying it goes with its host.
    assert!(h.query_by_label("Handle").is_some() || h.query_by_label("Lever").is_some(), "its family or type shown");
    key(&mut h, Key::Delete);
    assert!(h.state().session.model.elements.get(handle).is_some());
    assert!(h.state().status.text.contains("part of component"), "{}", h.state().status.text);
}

/// A family whose nested Leg is placed at (1500, -500), as `at` and `extra` (more fields of the instance) say.
fn legged(at: &str, extra: &str) -> String {
    let leg = r#"{"types": {"A": {}}, "forms": [{"profile": [[-100, -100], [100, -100], [100, 100], [-100, 100]], "to": 500}]}"#;
    family_with("").replace(
        r#""forms": ["#,
        &format!(r#""nested": [{{"name": "Leg", "family": {leg}}}], "nested_instances": [{{"family": "Leg", "type": "A", "at": {at}{extra}}}], "forms": ["#),
    )
}

fn number(e: &Expr) -> f64 {
    match e {
        Expr::Number(x) => *x,
        _ => f64::NAN,
    }
}

#[test]
fn a_nested_instance_dragged_in_plan_moves_as_one_step() {
    let mut h = plan_of(&legged("[1500, -500]", ""));
    pick(&mut h, &[Picked::Nested(0)]);
    drag(&mut h, [1500.0, -500.0, 0.0], [300.0, 200.0, 0.0], Modifiers::NONE);
    let at = family(&h).nested_instances[0].at.clone();
    assert!(near(number(&at[0]), 1800.0) && near(number(&at[1]), -300.0), "{at:?} {}", status(&h));
    assert_eq!(editor(&h).steps(), 1);
    assert_eq!(editor(&h).selection.items(), &[Picked::Nested(0)], "still selected");
    h.state_mut().family_undo();
    h.run();
    assert_eq!(family(&h).nested_instances[0].at, [Expr::Number(1500.0), Expr::Number(-500.0)]);
    // One not selected is selected by the press and moved.
    ed(&mut h).selection.clear();
    drag(&mut h, [1600.0, -500.0, 0.0], [0.0, -400.0, 0.0], Modifiers::NONE);
    assert!(near(number(&family(&h).nested_instances[0].at[1]), -900.0), "{}", status(&h));
}

#[test]
fn a_nested_instance_dragged_in_an_elevation_moves_along_its_plane_and_off_it() {
    let mut h = plan_of(&legged("[1500, -500]", ""));
    view(&mut h, Some(Which::Elevation(Side::Front)));
    pick(&mut h, &[Picked::Nested(0)]);
    // Seen from the front, a drag up lifts it off Ref. Level (its offset), one across slides it along x.
    drag(&mut h, [1500.0, -500.0, 250.0], [200.0, 0.0, 300.0], Modifiers::NONE);
    let n = &family(&h).nested_instances[0];
    assert!(near(number(&n.at[0]), 1700.0) && near(number(&n.offset), 300.0), "{:?} {:?} {}", n.at, n.offset, status(&h));
    assert_eq!(number(&n.at[1]), -500.0, "the depth it is seen along is kept");
    assert_eq!(editor(&h).steps(), 1);
}

#[test]
fn a_pinned_nested_instance_or_one_placed_by_a_formula_isnt_dragged() {
    let mut h = plan_of(&legged("[1500, -500]", r#", "pinned": true"#));
    pick(&mut h, &[Picked::Nested(0)]);
    drag(&mut h, [1500.0, -500.0, 0.0], [300.0, 0.0, 0.0], Modifiers::NONE);
    assert_eq!(family(&h).nested_instances[0].at, [Expr::Number(1500.0), Expr::Number(-500.0)]);
    assert!(untouched(&h));
    assert!(status(&h).to_lowercase().contains("pinned"), "{}", status(&h));
    // Its x a formula: a drag across would change it, and is refused, saying so; one along y keeps the formula.
    let mut h = plan_of(&legged(r#"["Len * 15", -500]"#, ""));
    pick(&mut h, &[Picked::Nested(0)]);
    drag(&mut h, [1500.0, -500.0, 0.0], [300.0, 0.0, 0.0], Modifiers::NONE);
    assert_eq!(family(&h).nested_instances[0].at[0], Expr::Formula("Len * 15".into()));
    assert!(untouched(&h));
    assert!(status(&h).contains("formula"), "{}", status(&h));
    let at = press_and_move(&mut h, [1500.0, -500.0, 0.0], [0.0, 300.0, 0.0], Modifiers::SHIFT);
    release(&mut h, at);
    let n = &family(&h).nested_instances[0];
    assert_eq!(n.at[0], Expr::Formula("Len * 15".into()));
    assert!(near(number(&n.at[1]), -200.0), "{:?} {}", n.at, status(&h));
}

/// The screenshots T6b is looked at by, saved where `BIMCRAFT_T6B_DUMP` says (only then): the handle Tab-selected in the
/// project's plan, under the cursor and selected; a schedule listing it; a tag on it; and a nested instance dragged in its
/// family's plan.
#[test]
fn shared_nested_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_T6B_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let save = |h: &mut Harness<BimApp>, name: &str| {
        h.run();
        h.run();
        h.render().unwrap().save(std::path::Path::new(&folder).join(format!("t6b-{name}.png"))).unwrap();
    };
    let mut h = app(CABINET);
    h.set_size(vec2(1600.0, 1000.0));
    h.run();
    let handle = handle(&h);
    // In close on the cabinet, so its handle shows.
    let level = match h.state().sheet {
        Sheet::Plan(level) => level,
        _ => panic!("a plan"),
    };
    h.state_mut().canvas.look_at(level, P2(5300.0, 400.0), 0.35);
    h.run();
    h.hover_at(h.state().screen(HANDLE));
    h.run();
    key(&mut h, Key::Tab);
    save(&mut h, "tab-hover");
    click(&mut h, HANDLE);
    save(&mut h, "tab-selected");
    h.state_mut().run("tag", json!({"ids": [handle], "offset": [700, 250], "leader": true}));
    save(&mut h, "tag");
    let id = h
        .state_mut()
        .run("schedule", json!({"name": "Furniture and Parts", "category": "component", "fields": ["Family", "Type", "Level", "Count"]}));
    let id = id.and_then(|v| v["id"].as_u64()).unwrap();
    h.state_mut().open_sheet(Sheet::Table(id));
    save(&mut h, "schedule");
    // In the family: the leg dragged, mid-drag.
    let mut h = app(&legged("[1500, -500]", ""));
    h.set_size(vec2(1600.0, 1000.0));
    h.state_mut().edit_family("Movers");
    h.run();
    view(&mut h, Some(Which::Plan));
    pick(&mut h, &[Picked::Nested(0)]);
    let at = press_and_move(&mut h, [1500.0, -500.0, 0.0], [300.0, 1100.0, 0.0], Modifiers::NONE);
    save(&mut h, "family-drag");
    release(&mut h, at);
    save(&mut h, "family-dropped");
}
