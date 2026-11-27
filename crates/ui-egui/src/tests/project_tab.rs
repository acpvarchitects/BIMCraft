//! Ordinary overlapping project instances can each be preselected in a model view.
use super::*;

const BOXES: &str = r#"family {"name":"Front Box","category":"generic","types":{"Standard":{}},"forms":[{"profile":[[0,0],[600,0],[600,400],[0,400]],"to":800}]}
family {"name":"Rear Box","category":"generic","types":{"Standard":{}},"forms":[{"profile":[[0,0],[600,0],[600,400],[0,400]],"to":800}]}
component {"family":"Front Box","type":"Standard","at":[5000,0]}"#;

fn other_after_tab(h: &mut Harness<BimApp>, at: Pos2, view: &str) {
    let boxes =
        h.state().session.model.elements.iter().filter_map(|(id, e)| matches!(e, Element::Component(_)).then_some(*id)).collect::<Vec<_>>();
    assert_eq!(boxes.len(), 2);
    click_at(h, at);
    assert_eq!(h.state().selection.len(), 1, "{view} has an ordinary first pick");
    let first = h.state().selection[0];
    if view != "3d" {
        assert_eq!(first, find(h, |e| matches!(e, Element::Component(c) if c.family == "Front Box")), "the nearer instance comes first");
    }
    assert!(boxes.contains(&first), "{view} clicked {first}");
    let other = *boxes.iter().find(|id| **id != first).unwrap();
    h.hover_at(at);
    h.run();
    key(h, Key::Tab);
    h.run();
    assert_eq!(h.state().nested_tab.current(), Some(other), "the candidate is preselected before the click");
    let label = crate::canvas::named(&h.state().session.model, other);
    assert!(h.query_by_label(label.as_str()).is_some(), "the status bar names the candidate");
    if let Some(dir) = std::env::var_os("BIMCRAFT_PROJECT_TAB_CAPTURE") {
        h.run();
        h.render().unwrap().save(std::path::Path::new(&dir).join(format!("tab-{view}.png"))).unwrap();
    }
    click_at(h, at);
    assert_eq!(h.state().selection, [other], "Tab and click take the second projected owner in {view}");
    key(h, Key::Tab);
    click_at(h, at);
    assert_eq!(h.state().selection, [first], "cycling wraps to the ordinary first pick in {view}");
    key(h, Key::Tab);
    h.hover_at(at + vec2(12., 0.));
    h.run();
    h.hover_at(at);
    h.run();
    assert_eq!(h.state().nested_tab.current(), Some(first), "moving the pointer starts the cycle again");
}

#[test]
fn tab_reaches_a_second_overlapping_project_instance_in_3d() {
    let mut h = app(&format!("{BOXES}\n{}", r#"component {"family":"Rear Box","type":"Standard","at":[5000,0]}"#));
    press(&mut h, "Default 3D View");
    h.run();
    let at = h.state().screen3([5300.0, 200.0, 400.0]);
    other_after_tab(&mut h, at, "3d");
}

fn vertical(view: &str) {
    let mut h = app(&format!(
        "{BOXES}\n{}\n{view} {}",
        r#"component {"family":"Rear Box","type":"Standard","at":[5000,600]}"#,
        r#"{"start":[4000,-2000],"end":[7000,-2000],"name":"Front"}"#
    ));
    let id = find(&h, |e| matches!(e, Element::Section(_) | Element::Elevation(_)));
    h.state_mut().open_sheet(Sheet::Section(id));
    h.run();
    h.run();
    let at = h.state().screen(P2(1300., 400.));
    other_after_tab(&mut h, at, view);
}

#[test]
fn tab_reaches_the_instance_behind_the_front_one_in_a_section() {
    vertical("section");
}

#[test]
fn tab_reaches_the_instance_behind_the_front_one_in_an_elevation() {
    vertical("elevation");
}
