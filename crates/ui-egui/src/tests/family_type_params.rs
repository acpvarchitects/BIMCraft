//! T24 in the window: a nested instance labelled with a family type parameter from the Options Bar's Label (or a parameter
//! made there, Create Parameter…), Family Types picking the nested family's type that parameter holds, the labelled instance's
//! Type greyed in Properties, unlabelling writing back what it showed; and in the project, a component's family type picked
//! in Properties and in Type Properties.
use super::family_drag_move::{ed, pick};
use super::family_views::{editor, view};
use super::*;
use crate::editor::select_box::Picked;
use bimcraft_core::family::views::Which;
use egui_kittest::kittest::NodeT;

/// A cabinet 600 by 400, 800 high, types Big (900) and Small (600), with family type parameters `Handle` (generic) and `Unit`
/// (furniture); it nests a Lever (40 by 20, 30 high) and a Knob (10 by 10, 50 high), both generic, and a Shelf of furniture,
/// with a lever on its top at (100, 50), not labelled.
const CABINET: &str = r#"family {"name": "Cabinet", "category": "furniture", "parameters": [{"name": "Width", "value": 600}, {"name": "Handle", "kind": "familytype", "of": "generic"}, {"name": "Unit", "kind": "familytype", "of": "furniture"}], "types": {"Small": {"Width": 600}, "Big": {"Width": 900}}, "forms": [{"profile": [[0, 0], ["Width", 0], ["Width", 400], [0, 400]], "to": 800}]}
family {"name": "Cabinet", "nest": {"name": "Lever", "definition": {"category": "generic", "types": {"L": {}}, "forms": [{"profile": [[0, 0], [40, 0], [40, 20], [0, 20]], "to": 30}]}}}
family {"name": "Cabinet", "nest": {"name": "Knob", "definition": {"category": "generic", "types": {"K": {}}, "forms": [{"profile": [[0, 0], [10, 0], [10, 10], [0, 10]], "to": 50}]}}}
family {"name": "Cabinet", "nest": {"name": "Shelf", "definition": {"category": "furniture", "types": {"S": {}}, "forms": [{"profile": [[0, 0], [500, 0], [500, 300], [0, 300]], "to": 20}]}}}
family {"name": "Cabinet", "nested_instance": {"family": "Lever", "type": "L", "at": [100, 50], "offset": 800}}"#;

/// The cabinet's document, its floor plan shown, the lever selected.
fn opened() -> Harness<'static, BimApp> {
    let mut h = app(CABINET);
    h.set_size(vec2(1600.0, 1000.0));
    h.state_mut().edit_family("Cabinet");
    h.run();
    view(&mut h, Some(Which::Plan));
    pick(&mut h, &[Picked::Nested(0)]);
    h
}

/// The Options Bar's Label drop-down opened.
fn open_label(h: &mut Harness<BimApp>) {
    h.get_by_label("Label").click();
    h.run();
}

/// The drop-down labelled `label` scrolled to and opened.
fn open_combo(h: &mut Harness<BimApp>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run();
    h.get_by_label(label).click();
    h.run();
    h.run();
}

/// The last node labelled `label` clicked: a drop-down's entry, over what else says it.
fn choose(h: &mut Harness<BimApp>, label: &str) {
    let n = h.get_all_by_label(label).last().unwrap_or_else(|| panic!("an entry `{label}`"));
    n.click();
    h.run();
    h.run();
}

/// The draft's top for type `ty`: 830 for a lever on the cabinet, 850 for a knob.
fn top(h: &Harness<BimApp>, ty: &str) -> f64 {
    let f = editor(h).draft_family().unwrap();
    let v = f.values(ty, &Default::default()).unwrap();
    f.pieces(&v).unwrap().iter().flat_map(|p| p.tris.iter().flatten()).map(|p| p[2]).fold(f64::NEG_INFINITY, f64::max)
}

fn param(h: &Harness<BimApp>, name: &str) -> Option<bimcraft_core::family::Parameter> {
    editor(h).draft_family().unwrap().parameters.into_iter().find(|p| p.name == name)
}

fn choice(f: &str, t: &str) -> Value {
    json!({"family": f, "type": t})
}

#[test]
fn options_bar_label_lists_family_type_parameters_of_the_category() {
    let mut h = opened();
    open_label(&mut h);
    assert!(h.query_all_by_label("Handle").count() > 0, "Handle, a generic family type, is offered");
    assert!(h.query_by_label("Create Parameter…").is_some());
    assert!(h.query_by_label("Unit").is_none(), "Unit picks furniture, and the lever is generic");
    assert!(h.query_by_label("Width").is_none(), "a length labels no instance");
    // The drop-down has the keys: Delete takes nothing away while it is open.
    key(&mut h, Key::Delete);
    assert_eq!(editor(&h).nested_instances.len(), 1, "the lever is still there");
    choose(&mut h, "Handle");
    assert_eq!(editor(&h).nested_instances[0].label.as_deref(), Some("Handle"));
    assert_eq!(param(&h, "Handle").unwrap().value, choice("Lever", "L"), "labelling fills its value with what it shows");
    assert_eq!(top(&h, "Big"), 830.0, "nothing changed in the picture");
}

#[test]
fn options_bar_create_parameter_presets_kind_and_category() {
    let mut h = opened();
    open_label(&mut h);
    choose(&mut h, "Create Parameter…");
    assert!(h.query_by_label("Parameter Properties").is_some(), "Parameter Properties opens");
    assert!(h.query_by_label("Family Type of Generic").is_some(), "its kind and category set, as the instance's");
    type_into(&mut h, "Name", "Pull");
    press(&mut h, "OK");
    let p = param(&h, "Pull").expect("Pull made");
    assert_eq!((p.kind, p.of), (bimcraft_core::family::Kind::FamilyType, Some(bimcraft_core::family::Category::Generic)));
    assert_eq!(p.value, choice("Lever", "L"));
    assert_eq!(editor(&h).nested_instances[0].label.as_deref(), Some("Pull"));
    assert!(editor(&h).views.creating.is_none(), "the dialog closed");
}

/// The lever labelled `Handle` in the draft, as the Options Bar labels it.
fn labelled() -> Harness<'static, BimApp> {
    let mut h = opened();
    open_label(&mut h);
    choose(&mut h, "Handle");
    h
}

#[test]
fn family_types_picks_a_nested_type_and_the_view_swaps() {
    let mut h = labelled();
    ed(&mut h).preview_type("Big");
    h.run();
    press(&mut h, "Family Types");
    let value = "Parameter Value Handle";
    assert!(!h.get_by_label("Parameter Formula Handle").accesskit_node().is_disabled(), "a family type takes a formula");
    h.get_by_label(value).click();
    h.run();
    assert!(h.query_by_label("Shelf : S").is_none(), "furniture isn't offered for a generic family type");
    choose(&mut h, "Knob : K");
    press(&mut h, "OK");
    assert_eq!(top(&h, "Big"), 850.0, "Big shows the knob");
    assert_eq!(top(&h, "Small"), 830.0, "Small the lever still");
    let f = editor(&h).draft_family().unwrap();
    assert_eq!(f.types["Big"]["Handle"], choice("Knob", "K"));
    // Family Types opened and closed with OK, nothing changed: the draft as it was.
    let before = editor(&h).draft_family().unwrap();
    press(&mut h, "Family Types");
    press(&mut h, "OK");
    assert_eq!(editor(&h).draft_family().unwrap(), before);
}

#[test]
fn labelled_instance_type_is_greyed_in_properties() {
    let mut h = labelled();
    ed(&mut h).preview_type("Big");
    h.run();
    press(&mut h, "Family Types");
    h.get_by_label("Parameter Value Handle").click();
    h.run();
    choose(&mut h, "Knob : K");
    press(&mut h, "OK");
    pick(&mut h, &[Picked::Nested(0)]);
    h.run();
    let ty = h.get_by_label("Type");
    assert!(ty.accesskit_node().is_disabled(), "its label picks its type");
    assert!(h.query_by_label("Labelled by Handle").is_some());
    assert_eq!(ty.accesskit_node().value().as_deref(), Some("Knob : K"), "it shows what Big picks");
}

#[test]
fn unlabelling_writes_back_what_the_instance_showed() {
    let mut h = labelled();
    ed(&mut h).preview_type("Big");
    h.run();
    press(&mut h, "Family Types");
    h.get_by_label("Parameter Value Handle").click();
    h.run();
    choose(&mut h, "Knob : K");
    press(&mut h, "OK");
    pick(&mut h, &[Picked::Nested(0)]);
    open_label(&mut h);
    choose(&mut h, "<None>");
    let inst = &editor(&h).nested_instances[0];
    assert_eq!(inst.label, None);
    assert_eq!((inst.family.as_str(), inst.type_name.as_str()), ("Knob", "K"), "what Big showed");
    assert_eq!(top(&h, "Small"), 850.0, "unlabelled, every type shows the knob");
}

#[test]
fn an_inner_family_type_is_set_only_by_associating() {
    // The Lever with a family type of its own: in the cabinet, its row has "=" alone, offering the cabinet's generic ones.
    let script = CABINET.replace(
        r#""definition": {"category": "generic", "types": {"L": {}},"#,
        r#""definition": {"category": "generic", "parameters": [{"name": "Grip", "kind": "familytype", "of": "generic"}], "types": {"L": {}},"#,
    );
    let mut h = app(&script);
    h.set_size(vec2(1600.0, 1000.0));
    h.state_mut().edit_family("Cabinet");
    h.run();
    view(&mut h, Some(Which::Plan));
    pick(&mut h, &[Picked::Nested(0)]);
    assert!(h.query_by_label("Grip").is_some(), "its family type is listed");
    let label = "Associate Family Parameter: Grip";
    h.get_by_label(label).scroll_to_me();
    h.run();
    press(&mut h, label);
    assert!(h.query_all_by_label("Handle").count() > 0, "a generic family type of the cabinet's");
    assert!(h.query_by_label("Unit").is_none(), "not one of furniture");
    choose(&mut h, "Handle");
    press(&mut h, "OK");
    assert_eq!(editor(&h).nested_instances[0].params.get("Grip"), Some(&bimcraft_core::family::Expr::Formula("Handle".into())));
}

/// A project with the cabinet: `Handle` an instance family type labelling its lever, `Base` a type one, and a component.
const PROJECT: &str = r#"family {"name": "Cabinet", "category": "furniture", "parameters": [{"name": "Handle", "kind": "familytype", "of": "generic", "instance": true}, {"name": "Base", "kind": "familytype", "of": "generic"}], "types": {"Small": {}}, "forms": [{"profile": [[0, 0], [600, 0], [600, 400], [0, 400]], "to": 800}]}
family {"name": "Cabinet", "nest": {"name": "Lever", "definition": {"category": "generic", "types": {"L": {}}, "forms": [{"profile": [[0, 0], [40, 0], [40, 20], [0, 20]], "to": 30}]}}}
family {"name": "Cabinet", "nest": {"name": "Knob", "definition": {"category": "generic", "types": {"K": {}}, "forms": [{"profile": [[0, 0], [10, 0], [10, 10], [0, 10]], "to": 50}]}}}
family {"name": "Cabinet", "nested_instance": {"family": "Lever", "type": "L", "at": [100, 50], "offset": 800, "label": "Handle"}}
component {"family": "Cabinet", "type": "Small", "at": [5000, 0]}"#;

fn component_top(h: &Harness<BimApp>) -> f64 {
    let m = &h.state().session.model;
    let c = m.elements.values().find_map(|e| if let Element::Component(c) = e { Some(c) } else { None }).unwrap();
    m.component_pieces(c).iter().flat_map(|p| p.tris.iter().flatten()).map(|p| p[2]).fold(f64::NEG_INFINITY, f64::max)
}

#[test]
fn project_properties_swap_a_components_nested_family() {
    let mut h = app(PROJECT);
    h.set_size(vec2(1600.0, 1000.0));
    let cabinet = find(&h, |e| matches!(e, Element::Component(_)));
    h.state_mut().selection = vec![cabinet];
    h.run();
    h.run();
    assert_eq!(component_top(&h), 830.0);
    let value = h.get_by_label("Handle").accesskit_node().value().map(|v| v.to_string());
    assert_eq!(value.as_deref(), Some("Lever : L"), "what it holds, as Family : Type");
    open_combo(&mut h, "Handle");
    choose(&mut h, "Knob : K");
    assert_eq!(component_top(&h), 850.0, "the knob");
    let Some(Element::Component(c)) = h.state().session.model.elements.get(cabinet) else { panic!() };
    assert_eq!(c.params.get("Handle"), Some(&choice("Knob", "K")));
    // Undone, the lever.
    h.state_mut().run("undo", json!({}));
    h.run();
    assert_eq!(component_top(&h), 830.0);
}

#[test]
fn type_properties_pick_a_family_type_and_ok_with_nothing_edited_changes_nothing() {
    let mut h = app(PROJECT);
    h.set_size(vec2(1600.0, 1000.0));
    let cabinet = find(&h, |e| matches!(e, Element::Component(_)));
    h.state_mut().selection = vec![cabinet];
    h.run();
    let before = h.state().session.model.to_json().unwrap();
    press(&mut h, "Edit Type");
    press(&mut h, "OK");
    assert_eq!(h.state().session.model.to_json().unwrap(), before, "OK with nothing edited changes nothing");
    press(&mut h, "Edit Type");
    open_combo(&mut h, "Base");
    choose(&mut h, "Knob : K");
    press(&mut h, "OK");
    let f = &h.state().session.model.families["Cabinet"];
    assert_eq!(f.types["Small"]["Base"], choice("Knob", "K"));
}

/// The Options Bar's Label open, Family Types with the drop-down open, and the project's Properties with the swap, in the
/// three looks and both profiles, when `BIMCRAFT_T24_UI_DUMP` names a folder.
#[test]
fn family_type_params_screenshots() {
    let Ok(folder) = std::env::var("BIMCRAFT_T24_UI_DUMP") else { return };
    std::fs::create_dir_all(&folder).unwrap();
    let looks = [("light", false, false), ("dark", true, false), ("dark-ui-light-canvas", true, true)];
    for profile in [crate::style::Profile::Rev, crate::style::Profile::BimCraft] {
        for (look, dark, light_canvas) in looks {
            let name = format!("{}-{look}", if profile == crate::style::Profile::Rev { "rev" } else { "bimcraft" });
            let setup = |script: &str| {
                let mut h = harness(script, 0.25, profile);
                h.set_size(vec2(1600.0, 1000.0));
                h.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
                h.state_mut().light_canvas = light_canvas;
                h.run();
                h
            };
            let save = |h: &mut Harness<BimApp>, what: &str| {
                for _ in 0..3 {
                    h.run();
                }
                h.render().unwrap().save(std::path::Path::new(&folder).join(format!("t24-{what}-{name}.png"))).unwrap();
            };
            let mut h = setup(CABINET);
            h.state_mut().edit_family("Cabinet");
            h.run();
            view(&mut h, Some(Which::Plan));
            pick(&mut h, &[Picked::Nested(0)]);
            open_label(&mut h);
            save(&mut h, "options-bar-label");
            choose(&mut h, "Handle");
            ed(&mut h).preview_type("Big");
            h.run();
            press(&mut h, "Family Types");
            h.get_by_label("Parameter Value Handle").click();
            h.run();
            save(&mut h, "family-types-combo");
            choose(&mut h, "Knob : K");
            press(&mut h, "OK");
            pick(&mut h, &[Picked::Nested(0)]);
            save(&mut h, "nested-properties-labelled");
            let mut h = setup(PROJECT);
            let cabinet = find(&h, |e| matches!(e, Element::Component(_)));
            h.state_mut().selection = vec![cabinet];
            h.run();
            open_combo(&mut h, "Handle");
            save(&mut h, "project-properties-combo");
            choose(&mut h, "Knob : K");
            save(&mut h, "project-properties-swapped");
        }
    }
}
