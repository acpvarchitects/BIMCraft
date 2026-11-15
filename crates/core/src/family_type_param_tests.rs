//! T24: a nested family chosen by a family type parameter. A host's parameter of kind `familytype` holds a family it nests
//! (of the parameter's category) and one of its types, `{"family", "type"}`; a nested instance `label`led with it shows that
//! family and type, per type, per component, or as a choice formula picks. Three levels down, a host's parameter associated
//! with a nested family's own picks inside it. The derived element of a shared instance keeps its GUID, and its tags,
//! through a swap.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::ids::Id;
use crate::model::{Element, Model};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn err(s: &mut Session, id: &str, p: Value) -> String {
    match s.execute(id, &p) {
        Ok(v) => panic!("{id} {p} should be refused, not give {v}"),
        Err(e) => e,
    }
}

/// A box `w` by `d` and `h` high, of category `category`, its one type `ty`; `shared` or not.
fn part(category: &str, ty: &str, w: f64, d: f64, h: f64, shared: bool) -> Value {
    json!({"category": category, "shared": shared, "types": {ty: {}},
           "forms": [{"profile": [[0, 0], [w, 0], [w, d], [0, d]], "to": h}]})
}

fn choice(family: &str, ty: &str) -> Value {
    json!({"family": family, "type": ty})
}

/// A cabinet 600 by 400 and 800 high, types Small and Big, with a type parameter `Handle` (a generic family type); it nests a
/// Lever (40 by 20, 30 high) and a Knob (10 by 10, 50 high), shared as `shared` says, and a Shelf of furniture; an instance of
/// the Lever stands on its top at (100, 50), labelled `Handle`.
fn cabinet(shared: bool) -> Session {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "category": "furniture",
               "parameters": [{"name": "Width", "value": 600}, {"name": "Handle", "kind": "familytype", "of": "generic"}],
               "types": {"Small": {"Width": 600}, "Big": {"Width": 900}},
               "forms": [{"profile": [[0, 0], ["Width", 0], ["Width", 400], [0, 400]], "to": 800}]}),
    );
    for (name, def) in [
        ("Lever", part("generic", "L", 40.0, 20.0, 30.0, shared)),
        ("Knob", part("generic", "K", 10.0, 10.0, 50.0, shared)),
        ("Shelf", part("furniture", "S", 500.0, 300.0, 20.0, false)),
    ] {
        run(&mut s, "family", json!({"name": "Cabinet", "nest": {"definition": def, "name": name}}));
    }
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nested_instance": {"family": "Lever", "type": "L", "at": [100, 50], "offset": 800, "label": "Handle"}}),
    );
    s
}

/// The top of what a component of `type_name` builds: 830 for a lever on the cabinet, 850 for a knob.
fn top(s: &mut Session, type_name: &str, params: Value) -> f64 {
    let id = run(s, "component", json!({"family": "Cabinet", "type": type_name, "at": [0, 0], "params": params}))["id"].as_u64().unwrap();
    let top = component_top(&s.model, id);
    run(s, "undo", json!({}));
    top
}

fn component_top(m: &Model, id: Id) -> f64 {
    let Some(Element::Component(c)) = m.elements.get(id) else { panic!("a component") };
    m.component_pieces(c).iter().flat_map(|p| p.tris.iter().flatten()).map(|p| p[2]).fold(f64::NEG_INFINITY, f64::max)
}

fn types(s: &Session) -> Value {
    serde_json::to_value(&s.model.families["Cabinet"].types).unwrap()
}

#[test]
fn labelling_fills_null_values_with_the_instances_type() {
    let s = cabinet(false);
    // The default holds it; the types, set nothing, follow the default.
    let t = types(&s);
    assert!(t["Small"].get("Handle").is_none() && t["Big"].get("Handle").is_none(), "{t}");
    let handle = s.model.families["Cabinet"].parameters.iter().find(|p| p.name == "Handle").unwrap().clone();
    assert_eq!(handle.value, choice("Lever", "L"));
    // The family's reply says what it may pick.
    let mut s = s;
    let r = run(&mut s, "family", json!({"name": "Cabinet"}));
    assert_eq!(r["choices"]["Handle"], json!([choice("Lever", "L"), choice("Knob", "K")]), "{}", r["choices"]);
    assert_eq!(r["values"]["Small"]["Handle"], choice("Lever", "L"));
}

#[test]
fn a_type_value_swaps_the_nested_geometry() {
    let mut s = cabinet(false);
    assert_eq!(top(&mut s, "Big", json!({})), 830.0, "the lever");
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    assert_eq!(top(&mut s, "Big", json!({})), 850.0, "the knob");
    assert_eq!(top(&mut s, "Small", json!({})), 830.0, "Small still the lever");
}

#[test]
fn an_instance_value_in_the_project_swaps_the_component() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Handle", "instance": true}}));
    let id = run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [0, 0]}))["id"].as_u64().unwrap();
    assert_eq!(component_top(&s.model, id), 830.0);
    run(&mut s, "modify", json!({"id": id, "params": {"Handle": choice("Knob", "K")}}));
    assert_eq!(component_top(&s.model, id), 850.0);
    // One of furniture, one it doesn't nest, a type its family hasn't: refused, the component as it was.
    for (v, says) in [(choice("Shelf", "S"), "Shelf"), (choice("Nope", "K"), "Nope"), (choice("Knob", "Z"), "`Z`")] {
        let e = err(&mut s, "modify", json!({"id": id, "params": {"Handle": v}}));
        assert!(e.contains("Handle") && e.contains(says), "{e}");
    }
    assert_eq!(component_top(&s.model, id), 850.0);
    // Its schedule shows it as Family : Type.
    assert_eq!(crate::schedule::field_text(&s.model, id, "Handle").as_deref(), Some("Knob : K"));
}

#[test]
fn a_family_type_value_must_name_a_nested_family_of_its_category() {
    let mut s = cabinet(false);
    let e = err(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Shelf", "S")}}}));
    assert!(e.contains("Handle") && e.contains("Shelf") && e.contains("generic"), "{e}");
    let e = err(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Nope", "K")}}}));
    assert!(e.contains("Handle") && e.contains("Nope"), "{e}");
}

#[test]
fn a_family_type_value_naming_a_missing_type_is_refused() {
    let mut s = cabinet(false);
    let e = err(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "Round")}}}));
    assert!(e.contains("Handle") && e.contains("Round"), "{e}");
}

#[test]
fn hostile_family_type_values_are_refused_not_panicked() {
    let mut s = cabinet(false);
    let long = "x".repeat(10_000);
    for v in [
        json!(5),
        json!("Knob : K"),
        json!({"family": "Knob"}),
        json!({"family": "Knob", "type": "K", "x": 1}),
        json!({"family": long, "type": "K"}),
        json!({"family": "Kn\nob", "type": "K"}),
        json!({"family": 5, "type": []}),
        json!([1, 2]),
    ] {
        let e = err(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": v}}}));
        assert!(e.contains("Handle"), "{v}: {e}");
    }
    s.model.check().unwrap();
}

#[test]
fn a_label_must_be_a_family_type_parameter_of_the_instances_category() {
    let mut s = cabinet(false);
    let g = s.model.families["Cabinet"].nested_instances[0].guid;
    let e = err(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": "Width"}}));
    assert!(e.contains("Width") && e.contains("family type"), "{e}");
    let e = err(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": "Nope"}}));
    assert!(e.contains("Nope"), "{e}");
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Unit", "kind": "familytype", "of": "furniture"}}));
    let e = err(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": "Unit"}}));
    assert_eq!(
        e,
        "family `Cabinet`: nested instance 1: it is labelled `Unit`, a family type of furniture, and `Lever` is of generic: \
         label it with one of its category",
        "the label's own error, not a value's"
    );
    // Unlabelled, it shows its own family and type, whatever the parameter says.
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": null}}));
    assert_eq!(top(&mut s, "Big", json!({})), 830.0);
    // A label's parameter can't be deleted while it labels.
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": "Handle"}}));
    let e = err(&mut s, "family", json!({"name": "Cabinet", "delete_parameter": ["Handle"]}));
    assert!(e.contains("Handle"), "{e}");
}

#[test]
fn a_choice_formula_picks_by_a_yes_no() {
    let mut s = cabinet(false);
    for p in [
        json!({"name": "Round", "kind": "yesno", "value": false}),
        json!({"name": "Long Handle", "kind": "familytype", "of": "generic", "value": choice("Lever", "L")}),
        json!({"name": "Round Handle", "kind": "familytype", "of": "generic", "value": choice("Knob", "K")}),
    ] {
        run(&mut s, "family", json!({"name": "Cabinet", "parameter": p}));
    }
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Small": {"Round": false}, "Big": {"Round": true}}}));
    // Its formula decides: the types no longer set it.
    let t = json!({"Small": {"Handle": null}, "Big": {"Handle": null}});
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "types": t, "parameter": {"name": "Handle", "formula": "if(Round, Round Handle, Long Handle)"}}),
    );
    assert_eq!(top(&mut s, "Small", json!({})), 830.0);
    assert_eq!(top(&mut s, "Big", json!({})), 850.0);
    // Renamed, the formula and the label follow.
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Round Handle", "rename": "Knobbed"}}));
    let f = &s.model.families["Cabinet"];
    assert_eq!(f.parameters.iter().find(|p| p.name == "Handle").unwrap().formula.as_deref(), Some("if(Round, Knobbed, Long Handle)"));
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Handle", "rename": "Pull"}}));
    assert_eq!(s.model.families["Cabinet"].nested_instances[0].label.as_deref(), Some("Pull"));
    assert_eq!(top(&mut s, "Big", json!({})), 850.0);
}

#[test]
fn a_choice_formula_with_a_literal_or_another_category_or_a_cycle_is_refused() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Unit", "kind": "familytype", "of": "furniture"}}));
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Other", "kind": "familytype", "of": "generic"}}));
    let t = json!({"Small": {"Handle": null}, "Big": {"Handle": null}});
    for (formula, says) in [
        (
            "if(Width > 700, 5, Other)",
            "parameter `Handle`: a family type's formula picks among family type parameters of its category: another's name, or \
             `if(condition, one, other)`",
        ),
        ("Width", "parameter `Handle`: its formula picks `Width`, which isn't a family type parameter"),
        ("Unit", "parameter `Handle`: its formula picks `Unit`, a family type of another category"),
        ("if(Width > 700, Handle, Other)", "`Handle`'s formula comes back to itself"),
        ("if(Other, Other, Other)", "`Other` isn't a number"),
    ] {
        let e = err(&mut s, "family", json!({"name": "Cabinet", "types": t, "parameter": {"name": "Handle", "formula": formula}}));
        assert!(e.contains(says), "{formula}: {e}");
    }
}

#[test]
fn deleting_a_nested_family_a_value_names_is_refused() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    let e = err(&mut s, "family", json!({"name": "Cabinet", "delete_nested": ["Knob"]}));
    assert!(e.contains("Knob") && e.contains("Handle"), "{e}");
}

/// The cabinet's definition as a file holds it, `change`d.
fn definition(s: &mut Session, change: impl Fn(&mut Value)) -> Value {
    let mut d = run(s, "family", json!({"name": "Cabinet"}))["family"].clone();
    change(&mut d);
    d
}

#[test]
fn reload_keeps_family_type_values_that_still_exist() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    // A version that still nests the knob keeps Big's.
    let d = definition(&mut s, |d| d["types"]["Big"]["Handle"] = choice("Lever", "L"));
    run(&mut s, "family", json!({"name": "Cabinet", "definition": d, "overwrite": "version"}));
    assert_eq!(types(&s)["Big"]["Handle"], choice("Knob", "K"));
    // One without the knob takes its own.
    let d = definition(&mut s, |d| {
        d["types"]["Big"]["Handle"] = choice("Lever", "L");
        let nested = d["nested"].as_array_mut().unwrap();
        nested.retain(|n| n["name"] != "Knob");
    });
    run(&mut s, "family", json!({"name": "Cabinet", "definition": d, "overwrite": "version"}));
    assert_eq!(types(&s)["Big"]["Handle"], choice("Lever", "L"));
}

#[test]
fn reload_refuses_an_instance_override_naming_a_vanished_family() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Handle", "instance": true}}));
    run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [0, 0], "params": {"Handle": choice("Knob", "K")}}));
    let d = definition(&mut s, |d| d["nested"].as_array_mut().unwrap().retain(|n| n["name"] != "Knob"));
    let e = err(&mut s, "family", json!({"name": "Cabinet", "definition": d, "overwrite": "version"}));
    assert!(e.contains("Handle") && e.contains("Knob"), "{e}");
    assert!(s.model.families["Cabinet"].nested.iter().any(|n| n.name == "Knob"), "the reload refused whole");
}

#[test]
fn insert_from_file_keeps_family_type_values() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    run(&mut s, "component", json!({"family": "Cabinet", "type": "Big", "at": [0, 0]}));
    let path = std::env::temp_dir().join(format!("bimcraft-t24-insert-{}.bimcraft", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    run(&mut s, "save", json!({"path": path}));
    let mut other = Session::new();
    run(&mut other, "insert", json!({"path": path}));
    let _ = std::fs::remove_file(&path);
    let id = other.model.elements.iter().find(|(_, e)| matches!(e, Element::Component(_))).map(|(id, _)| *id).unwrap();
    assert_eq!(component_top(&other.model, id), 850.0, "the knob, in the project it came into");
    other.model.check().unwrap();
}

#[test]
fn a_swapped_shared_instance_keeps_its_element_guid_and_tag() {
    let mut s = cabinet(true);
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Handle", "instance": true}}));
    let id = run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [0, 0]}))["id"].as_u64().unwrap();
    let nested = |m: &Model| -> Vec<(Id, crate::nested_elements::NestedElement)> {
        m.elements
            .iter()
            .filter_map(|(id, e)| match e {
                Element::Nested(n) => Some((*id, n.clone())),
                _ => None,
            })
            .collect()
    };
    let (handle, n) = nested(&s.model).pop().unwrap();
    assert_eq!(n.family, "Lever");
    let guid = s.model.guid(handle);
    assert_eq!(guid, crate::nested_elements::guid_of(s.model.guid(id), &[s.model.families["Cabinet"].nested_instances[0].guid]));
    let tag = run(&mut s, "tag", json!({"ids": [handle]}))["ids"][0].as_u64().unwrap();
    run(&mut s, "modify", json!({"id": id, "params": {"Handle": choice("Knob", "K")}}));
    let after = nested(&s.model);
    assert_eq!(after.len(), 1);
    assert_eq!(s.model.guid(after[0].0), guid, "the same element");
    assert_eq!((after[0].1.family.as_str(), after[0].1.type_name.as_str()), ("Knob", "K"));
    let Some(Element::Tag(t)) = s.model.elements.get(tag) else { panic!("the tag stays") };
    assert_eq!(t.host, guid);
    assert_eq!(s.model.tag_text(t), "K");
    // IFC names it by what it shows now.
    assert!(crate::ifc::export(&s.model).contains("'Knob:K'"));
}

#[test]
fn a_file_with_family_type_values_says_format_21() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    assert_eq!(crate::model::FORMAT, 21);
    let text = s.model.to_json().unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    // Above 20: what 0.12 reads up to, so 0.12 says it was saved by a newer BIMCraft rather than misreading it.
    assert_eq!(file["format"], json!(21));
    assert!(text.contains("\"label\": \"Handle\"") || text.contains("\"label\":\"Handle\""), "the label is in the file");
    let (back, left_out) = Model::read(&text).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    back.check().unwrap();
    assert_eq!(serde_json::to_value(&back.families["Cabinet"].types).unwrap()["Big"]["Handle"], choice("Knob", "K"));
    let mut old = file.clone();
    old["format"] = json!(22);
    assert!(Model::read(&old.to_string()).is_err(), "a newer file is refused");
}

/// Three levels: the cabinet's `Handle` is associated, through its instance of a Plate (casework, shared), with the plate's own
/// `Handle`, which labels the plate's instance of the Lever; the cabinet nests the Lever and the Knob too (shared, they come
/// with the plate). The cabinet's type picks what stands on the plate.
#[test]
fn a_host_parameter_associated_into_b_swaps_c_three_levels_deep() {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "category": "furniture",
               "parameters": [{"name": "Handle", "kind": "familytype", "of": "generic"}],
               "types": {"Small": {}, "Big": {}},
               "forms": [{"profile": [[0, 0], [600, 0], [600, 400], [0, 400]], "to": 800}]}),
    );
    let mut plate = part("casework", "P", 100.0, 100.0, 10.0, true);
    plate["parameters"] = json!([{"name": "Handle", "kind": "familytype", "of": "generic"}]);
    plate["nested"] = json!([{"name": "Lever", "family": part("generic", "L", 40.0, 20.0, 30.0, true)}, {"name": "Knob", "family": part("generic", "K", 10.0, 10.0, 50.0, true)}]);
    plate["nested_instances"] = json!([{"family": "Lever", "type": "L", "offset": 10, "label": "Handle"}]);
    plate["types"]["P"]["Handle"] = choice("Lever", "L");
    run(&mut s, "family", json!({"name": "Cabinet", "nest": {"definition": plate, "name": "Plate"}}));
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nested_instance": {"family": "Plate", "type": "P", "offset": 800, "params": {"Handle": "Handle"}}}),
    );
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "types": {"Small": {"Handle": choice("Lever", "L")}, "Big": {"Handle": choice("Knob", "K")}}}),
    );
    let small = run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [0, 0]}))["id"].as_u64().unwrap();
    let big = run(&mut s, "component", json!({"family": "Cabinet", "type": "Big", "at": [2000, 0]}))["id"].as_u64().unwrap();
    assert_eq!(component_top(&s.model, small), 840.0, "a lever on the plate");
    assert_eq!(component_top(&s.model, big), 860.0, "a knob on the plate");
    let on = |m: &Model, host: Id| -> Vec<(Id, crate::nested_elements::NestedElement)> {
        m.elements
            .iter()
            .filter_map(|(id, e)| match e {
                Element::Nested(n) if n.host == m.guid(host) && n.path.len() == 2 => Some((*id, n.clone())),
                _ => None,
            })
            .collect()
    };
    let (c, n) = on(&s.model, big).pop().expect("the knob, three levels down, an element of its own");
    assert_eq!(n.family, "Knob");
    let guid = s.model.guid(c);
    // The big cabinet's type picks the lever again: the same element, by its GUID.
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Lever", "L")}}}));
    let (c, n) = on(&s.model, big).pop().unwrap();
    assert_eq!((s.model.guid(c), n.family.as_str()), (guid, "Lever"));
    // Associated with a parameter of another category, or with a literal: refused.
    let g = s.model.families["Cabinet"].nested_instances[0].guid;
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Unit", "kind": "familytype", "of": "furniture"}}));
    let e = err(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "params": {"Handle": "Unit"}}}));
    assert!(e.contains("Handle") && e.contains("Unit"), "{e}");
    // A generic family the cabinet nests and the plate doesn't: the cabinet's Handle could pick what the plate can't.
    let e = err(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nest": {"definition": part("generic", "B", 5.0, 5.0, 5.0, false), "name": "Bolt"}}),
    );
    assert!(e.contains("Bolt") && e.contains("Plate"), "{e}");
}

#[test]
fn filling_happens_only_when_a_label_is_set_or_changed() {
    let mut s = cabinet(false);
    let g = s.model.families["Cabinet"].nested_instances[0].guid;
    let handle = |s: &Session| s.model.families["Cabinet"].parameters.iter().find(|p| p.name == "Handle").unwrap().value.clone();
    // Taken back to none after labelling: it stays none through later edits, and the instance shows its own.
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Handle", "value": null}}));
    run(&mut s, "family", json!({"name": "Cabinet", "parameter": {"name": "Width", "value": 650}}));
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "at": [120, 50]}}));
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": "Handle"}}));
    assert_eq!(handle(&s), Value::Null, "the same label again changes nothing");
    let r = run(&mut s, "family", json!({"name": "Cabinet"}));
    assert_eq!(r["values"]["Big"]["Handle"], Value::Null);
    assert_eq!(top(&mut s, "Big", json!({})), 830.0, "its own lever");
    // A type set to the knob keeps it, and one set back to none follows the default again.
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": null}}}));
    assert!(types(&s)["Big"].get("Handle").is_none());
    // Labelled anew (taken off, then on), it fills again.
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": null}}));
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "label": "Handle"}}));
    assert_eq!(handle(&s), choice("Lever", "L"));
}

#[test]
fn select_all_instances_takes_what_the_label_shows() {
    let mut s = cabinet(false);
    run(&mut s, "family", json!({"name": "Cabinet", "types": {"Big": {"Handle": choice("Knob", "K")}}}));
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"family": "Knob", "type": "K", "at": [300, 50], "offset": 800}}));
    let f = s.model.families["Cabinet"].clone();
    use crate::family::select::{Picked, instances};
    let big = f.values("Big", &std::collections::BTreeMap::new()).unwrap();
    let small = f.values("Small", &std::collections::BTreeMap::new()).unwrap();
    assert_eq!(instances(&f, &big, &[Picked::Nested(1)], None), [Picked::Nested(0), Picked::Nested(1)], "Big shows knobs on both");
    assert_eq!(instances(&f, &small, &[Picked::Nested(1)], None), [Picked::Nested(1)], "Small, a lever on the first");
}

#[test]
fn a_swap_between_a_shared_and_an_unshared_family_takes_the_element_away_and_brings_it_back() {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "category": "furniture",
               "parameters": [{"name": "Handle", "kind": "familytype", "of": "generic", "instance": true}],
               "types": {"Small": {}}, "forms": [{"profile": [[0, 0], [600, 0], [600, 400], [0, 400]], "to": 800}]}),
    );
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nest": {"definition": part("generic", "L", 40.0, 20.0, 30.0, true), "name": "Lever"}}),
    );
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nest": {"definition": part("generic", "K", 10.0, 10.0, 50.0, false), "name": "Knob"}}),
    );
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nested_instance": {"family": "Lever", "type": "L", "at": [100, 50], "offset": 800, "label": "Handle"}}),
    );
    let id = run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [0, 0]}))["id"].as_u64().unwrap();
    let nested = |m: &Model| -> Vec<Id> { m.elements.iter().filter(|(_, e)| matches!(e, Element::Nested(_))).map(|(id, _)| *id).collect() };
    let tags = |m: &Model| m.elements.values().filter(|e| matches!(e, Element::Tag(_))).count();
    let lever = nested(&s.model)[0];
    let guid = s.model.guid(lever);
    run(&mut s, "tag", json!({"ids": [lever]}));
    // To the knob, which isn't shared: no element, no tag.
    run(&mut s, "modify", json!({"id": id, "params": {"Handle": choice("Knob", "K")}}));
    assert!(nested(&s.model).is_empty());
    assert_eq!(tags(&s.model), 0);
    // Undone, both are back, by the same GUID.
    run(&mut s, "undo", json!({}));
    assert_eq!(nested(&s.model).iter().map(|id| s.model.guid(*id)).collect::<Vec<_>>(), [guid]);
    assert_eq!(tags(&s.model), 1);
    // To the knob and back to the lever: the element comes back by the same GUID; the tag went with it.
    run(&mut s, "modify", json!({"id": id, "params": {"Handle": choice("Knob", "K")}}));
    run(&mut s, "modify", json!({"id": id, "params": {"Handle": choice("Lever", "L")}}));
    assert_eq!(nested(&s.model).iter().map(|id| s.model.guid(*id)).collect::<Vec<_>>(), [guid]);
    assert_eq!(tags(&s.model), 0);
}

/// A choice formula whose condition is no number a float holds, an `if` picking itself on the branch not taken, and an
/// association three levels down, all past reading: worked out or refused, never a panic, the model as it was.
#[test]
fn a_hostile_choice_formula_and_association_are_worked_out_or_refused() {
    let mut s = cabinet(false);
    let before = s.model.families["Cabinet"].clone();
    for p in [
        json!({"name": "Cabinet", "parameter": {"name": "Other", "kind": "familytype", "of": "generic", "value": choice("Knob", "K")}}),
        json!({"name": "Cabinet", "types": {"Small": {"Handle": null}, "Big": {"Handle": null}},
               "parameter": {"name": "Handle", "formula": "if(1e308 * 1e308 - 1e308 * 1e308, Other, if(0 / 0, Handle, Other))"}}),
    ] {
        let _ = s.execute("family", &p);
    }
    s.model.check().unwrap();
    let mut plate = part("casework", "P", 100.0, 100.0, 10.0, false);
    plate["parameters"] = json!([{"name": "Pick", "kind": "familytype", "of": "generic"}]);
    plate["nested"] = json!([{"name": "Lever", "family": part("generic", "L", 40.0, 20.0, 30.0, false)},
                             {"name": "Knob", "family": part("generic", "K", 10.0, 10.0, 50.0, false)}]);
    plate["nested_instances"] = json!([{"family": "Lever", "type": "L", "label": "Pick"}]);
    plate["types"]["P"]["Pick"] = choice("Knob", "Z");
    let _ = s.execute("family", &json!({"name": "Cabinet", "nest": {"definition": plate, "name": "Plate"}}));
    plate["types"]["P"]["Pick"] = choice("Knob", "K");
    let _ = s.execute("family", &json!({"name": "Cabinet", "nest": {"definition": plate, "name": "Plate"}}));
    let _ = s.execute(
        "family",
        &json!({"name": "Cabinet", "nested_instance": {"family": "Plate", "type": "P", "params": {"Pick": "Other", "Nope": "Width / 0"}}}),
    );
    let _ =
        s.execute("family", &json!({"name": "Cabinet", "nested_instance": {"family": "Plate", "type": "P", "params": {"Pick": "Handle"}}}));
    s.model.check().unwrap();
    assert_ne!(s.model.families["Cabinet"], before, "something got past reading");
    top(&mut s, "Big", json!({}));
}

/// The association check compares every choice of the host with the nested family's once: four families of `n` types each,
/// checked at two sizes, grow as the choices do, not as their square.
#[test]
fn the_association_check_grows_with_the_choices_not_their_square() {
    fn with_types(n: usize) -> Session {
        let names: serde_json::Map<String, Value> = (0..n).map(|i| (format!("T{i}"), json!({}))).collect();
        let leaf = json!({"category": "generic", "types": names});
        let fams: Vec<Value> = (0..4).map(|k| json!({"name": format!("F{k}"), "family": leaf})).collect();
        let mut s = Session::new();
        let mut plate = json!({"category": "casework", "types": {"P": {}}, "nested": fams,
                               "parameters": [{"name": "Pick", "kind": "familytype", "of": "generic"}]});
        plate["nested_instances"] = json!([{"family": "F0", "type": "T0", "label": "Pick"}]);
        run(
            &mut s,
            "family",
            json!({"name": "Cabinet", "category": "furniture", "types": {"A": {}}, "nested": fams.iter().cloned().chain([json!({"name": "Plate", "family": plate})]).collect::<Vec<_>>(),
                   "parameters": [{"name": "Pick", "kind": "familytype", "of": "generic"}],
                   "nested_instances": [{"family": "Plate", "type": "P", "params": {"Pick": "Pick"}}]}),
        );
        s
    }
    let timed = |n: usize| {
        let mut s = with_types(n);
        (0..3)
            .map(|i| {
                let t = std::time::Instant::now();
                run(
                    &mut s,
                    "family",
                    json!({"name": "Cabinet", "parameter": {"name": "Pick", "group": if i % 2 == 0 { "data" } else { "other" }}}),
                );
                t.elapsed().as_secs_f64()
            })
            .fold(f64::INFINITY, f64::min)
    };
    let (small, large) = (timed(250), timed(1000));
    assert!(large < 20.0, "4 × 1000 types took {large} s a command");
    assert!(large < small * 9.0 + 0.05, "four times the choices took {:.1} times as long ({small} s, {large} s)", large / small);
}
