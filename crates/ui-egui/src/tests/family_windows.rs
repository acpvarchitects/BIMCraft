//! FE-22: family window commands operate on tabs and tools, never on the draft.
use super::*;
use bimcraft_core::family::views::Which;

const FAMILIES: &str = r#"family {"name": "Box", "category": "generic", "types": {"T": {}}, "forms": [{"profile": [[0, 0], [500, 0], [500, 300], [0, 300]], "to": 400}]}
family {"name": "Crate", "category": "generic", "types": {"T": {}}, "forms": [{"profile": [[0, 0], [800, 0], [800, 600], [0, 600]], "to": 600}]}"#;

fn open() -> Harness<'static, BimApp> {
    let mut h = app(&format!("{BOX}\n{FAMILIES}"));
    h.set_size(vec2(1600.0, 1000.0));
    h.run();
    h.state_mut().edit_family("Box");
    h.run();
    super::family_views::view(&mut h, Some(Which::Plan));
    h
}

#[test]
fn close_inactive_keeps_the_active_view_and_one_tab_per_other_document() {
    let mut h = open();
    let project = h.state().session.model.to_json().unwrap();
    let box_doc = h.state().active_doc().unwrap();
    h.state_mut().edit_family("Crate");
    h.run();
    let crate_doc = h.state().active_doc().unwrap();
    super::family_views::view(&mut h, Some(Which::Plan));
    h.state_mut().open_sheet(Sheet::Family(box_doc, Some(Which::Plan)));
    h.run();
    let draft = h.state().active_family().unwrap().draft();
    assert_eq!(h.state().doc_tabs(box_doc).len(), 2);
    assert_eq!(h.state().doc_tabs(crate_doc).len(), 2);
    shortcut(&mut h, "CH");
    assert_eq!(h.state().sheet, Sheet::Family(box_doc, Some(Which::Plan)));
    assert_eq!(h.state().doc_tabs(box_doc).len(), 1);
    assert_eq!(h.state().doc_tabs(crate_doc).len(), 1);
    assert_eq!(h.state().families.len(), 2);
    assert!(h.state().active_family().unwrap().draft() == draft);
    assert_eq!(h.state().session.model.to_json().unwrap(), project);
    assert!(h.state().doc_closing.is_none());
}

#[test]
fn repeat_last_command_rearms_a_tool_without_replaying_an_edit() {
    let mut h = open();
    let before = h.state().active_family().unwrap().draft();
    shortcut(&mut h, "RP");
    assert!(h.state().active_family().unwrap().views.drawing.is_some());
    shortcut(&mut h, "MD");
    assert!(h.state().active_family().unwrap().views.drawing.is_none());
    shortcut(&mut h, "RC");
    assert!(h.state().active_family().unwrap().views.drawing.is_some());
    assert!(h.state().active_family().unwrap().draft() == before);
    shortcut(&mut h, "MD");
    key(&mut h, Key::Enter);
    assert!(h.state().active_family().unwrap().views.drawing.is_some(), "Enter repeats on an idle canvas");
}

#[test]
fn repeat_last_command_is_document_local_and_ignores_mutations() {
    let mut h = open();
    shortcut(&mut h, "RP");
    let box_doc = h.state().active_doc().unwrap();
    h.state_mut().edit_family("Crate");
    h.run();
    super::family_views::view(&mut h, Some(Which::Plan));
    shortcut(&mut h, "RC");
    assert!(h.state().active_family().unwrap().views.drawing.is_none(), "no tool recorded in Crate");
    h.state_mut().open_sheet(Sheet::Family(box_doc, Some(Which::Plan)));
    h.run();
    shortcut(&mut h, "MD");
    shortcut(&mut h, "RC");
    assert!(h.state().active_family().unwrap().views.drawing.is_some(), "Box remembers its own tool");
}
