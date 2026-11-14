//! T6b: a shared nested instance in the project, Tab-selected inside its host in a plan; and a nested instance dragged in
//! its family's plan and elevation, as the family's other parts are (FE-7), honouring a pin and a formula, one step undone.
use super::family_drag_move::{drag, ed, family, family_with, near, pick, plan_of, press_and_move, release, status, untouched};
use super::family_views::{editor, view};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::Expr;
use bimcraft_core::family::views::{Side, Which};
use egui_kittest::kittest::NodeT;

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

/// The cabinet at (5000, 0), not turned, its handle (a shared family) standing proud of its front at (250, -20), 400 up, a
/// quarter of its width wide and 100 high; an elevation looking north at it from 2 m south.
const FRONT: &str = r#"family {"name": "Cabinet", "category": "furniture", "parameters": [{"name": "Width", "value": 600}], "types": {"Small": {"Width": 600}}, "forms": [{"profile": [[0, 0], ["Width", 0], ["Width", 400], [0, 400]], "to": 800}]}
family {"name": "Cabinet", "nest": {"name": "Handle", "definition": {"category": "generic", "shared": true, "parameters": [{"name": "W", "value": 40}], "types": {"Lever": {}}, "forms": [{"profile": [[0, 0], ["W", 0], ["W", 20], [0, 20]], "to": 100}]}}}
family {"name": "Cabinet", "nested_instance": {"family": "Handle", "type": "Lever", "at": [250, -20], "offset": 400, "params": {"W": "Width / 4"}}}
component {"family": "Cabinet", "type": "Small", "at": [5000, 0]}
elevation {"start": [4000, -2000], "end": [7000, -2000], "name": "Front"}"#;

/// The handle's middle on its front face, in the model.
const FRONT_HANDLE: [f64; 3] = [5325.0, -20.0, 450.0];

#[test]
fn in_an_elevation_tab_reaches_a_shared_nested_instance_after_its_host() {
    let mut h = app(FRONT);
    let cabinet = find(&h, |e| matches!(e, Element::Component(_)));
    let handle = handle(&h);
    let front = find(&h, |e| matches!(e, Element::Elevation(_)));
    h.state_mut().open_sheet(Sheet::Section(front));
    h.run();
    h.run();
    // Along its line from its start, and up.
    let at = P2(FRONT_HANDLE[0] - 4000.0, FRONT_HANDLE[2]);
    click(&mut h, at);
    assert_eq!(h.state().selection, [cabinet], "a click takes the cabinet");
    h.hover_at(h.state().screen(at));
    h.run();
    key(&mut h, Key::Tab);
    click(&mut h, at);
    assert_eq!(h.state().selection, [handle], "Tab, then a click, takes the handle");
}

#[test]
fn in_3d_tab_reaches_a_shared_nested_instance_after_its_host() {
    let mut h = app(FRONT);
    let cabinet = find(&h, |e| matches!(e, Element::Component(_)));
    let handle = handle(&h);
    press(&mut h, "Default 3D View");
    h.run();
    let at = h.state().screen3(FRONT_HANDLE);
    click_at(&mut h, at);
    assert_eq!(h.state().selection, [cabinet], "a click takes the cabinet");
    h.hover_at(at);
    h.run();
    key(&mut h, Key::Tab);
    click_at(&mut h, at);
    assert_eq!(h.state().selection, [handle], "Tab, then a click, takes the handle");
    // Its host as people read it.
    assert!(h.query_by_label("Cabinet : Small").is_some(), "its host by family and type");
}

#[test]
fn edit_family_opens_a_shared_nested_family_the_project_has_and_is_greyed_for_one_it_hasnt() {
    // In the plan, the handle's middle: Tab past the cabinet, and a click.
    let take_handle = |h: &mut Harness<BimApp>| {
        let at = P2(5325.0, -10.0);
        h.hover_at(h.state().screen(at));
        h.run();
        key(h, Key::Tab);
        click(h, at);
        assert_eq!(h.state().selection, [handle(h)]);
    };
    // Loading the cabinet loaded its shared Handle into the project too: Edit Family opens it.
    let mut h = app(FRONT);
    assert!(h.state().session.model.families.contains_key("Handle"));
    take_handle(&mut h);
    assert!(!h.get_by_label("Edit Family").accesskit_node().is_disabled());
    press(&mut h, "Edit Family");
    assert!(matches!(h.state().sheet, Sheet::Family(..)), "{:?}", h.state().sheet);
    let opened = editor(&h).draft_family().unwrap();
    assert!(opened.types.contains_key("Lever"), "the Handle family");
    // A project that hasn't it on its own (a file from elsewhere): greyed, saying why.
    let mut h = app(FRONT);
    std::sync::Arc::make_mut(&mut h.state_mut().session.model).families.remove("Handle");
    h.run();
    take_handle(&mut h);
    assert!(h.get_by_label("Edit Family").accesskit_node().is_disabled());
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
    // The cabinet in Existing, its plan at 1 : 20, as furniture is drawn.
    let script = format!(
        "{FRONT}\nscale {{\"view\": \"Level 1\", \"scale\": 20}}\nmodify {{\"id\": 100002, \"params\": {{\"Phase Created\": \"Existing\"}}}}"
    );
    let mut h = app(&script);
    h.set_size(vec2(1600.0, 1000.0));
    h.run();
    let cabinet = find(&h, |e| matches!(e, Element::Component(_)));
    let handle = handle(&h);
    let level = match h.state().sheet {
        Sheet::Plan(level) => level,
        _ => panic!("a plan"),
    };
    h.state_mut().canvas.set_view(level, 5150.0, 150.0, 0.6);
    h.run();
    let at = P2(5325.0, -10.0);
    h.hover_at(h.state().screen(at));
    h.run();
    key(&mut h, Key::Tab);
    save(&mut h, "tab-hover");
    click(&mut h, at);
    save(&mut h, "tab-selected");
    // A tag on the cabinet and one on its handle: the same size.
    h.state_mut().run("tag", json!({"ids": [cabinet], "offset": [-500, 450], "leader": true}));
    h.state_mut().run("tag", json!({"ids": [handle], "offset": [500, -350], "leader": true}));
    save(&mut h, "tag");
    let id = h.state_mut().run(
        "schedule",
        json!({"name": "Furniture and Parts", "category": "component", "fields": ["Family", "Type", "Level", "Phase Created", "Count"]}),
    );
    let id = id.and_then(|v| v["id"].as_u64()).unwrap();
    h.state_mut().open_sheet(Sheet::Table(id));
    save(&mut h, "schedule");
    // The elevation looking north at it: Tab over the cabinet reaches the handle.
    let front = find(&h, |e| matches!(e, Element::Elevation(_)));
    h.state_mut().open_sheet(Sheet::Section(front));
    h.run();
    h.run();
    h.state_mut().canvas.set_view(front, 1300.0, 450.0, 0.6);
    h.run();
    let at = P2(FRONT_HANDLE[0] - 4000.0, FRONT_HANDLE[2]);
    // Nothing selected and the pointer away: the handle as the elevation draws it.
    h.state_mut().selection.clear();
    h.hover_at(h.state().screen(P2(500.0, 1500.0)));
    h.run();
    save(&mut h, "elevation-plain");
    h.hover_at(h.state().screen(at));
    h.run();
    key(&mut h, Key::Tab);
    save(&mut h, "elevation-tab");
    click(&mut h, at);
    save(&mut h, "elevation-selected");
    // The default 3D view.
    press(&mut h, "Default 3D View");
    h.run();
    // In close on the cabinet, nothing selected and the pointer away: the handle as the 3D view draws it.
    h.state_mut().selection.clear();
    h.hover_at(h.state().screen3([5300.0, 200.0, 400.0]));
    h.run();
    for _ in 0..4 {
        h.event(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, 120.0),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        h.run();
    }
    h.hover_at(h.state().orbit.rect.min + vec2(10.0, 10.0));
    h.run();
    save(&mut h, "3d-plain");
    let at = h.state().screen3(FRONT_HANDLE);
    h.hover_at(at);
    h.run();
    key(&mut h, Key::Tab);
    save(&mut h, "3d-tab");
    click_at(&mut h, at);
    save(&mut h, "3d-selected");
    // The same project with the handle not shared: the 3D view's work plane patch (the small square by the model's
    // south-west corner) is there just the same; it is no part of the handle.
    let mut h = app(&FRONT.replace(r#""shared": true"#, r#""shared": false"#));
    h.set_size(vec2(1600.0, 1000.0));
    press(&mut h, "Default 3D View");
    h.run();
    h.hover_at(h.state().orbit.rect.min + vec2(10.0, 10.0));
    save(&mut h, "3d-not-shared");
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
