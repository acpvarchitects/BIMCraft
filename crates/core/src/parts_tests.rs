//! Parts of building elements (`parts.rs`): what the model asks of one, what goes with what, and how a file holds them.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::model::{Element, Guid, Id, Model};
use crate::parts::{BuildingPart, Decay, FILE_KEY, Heritage, PartType, Source};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn fails(s: &mut Session, id: &str, p: Value) -> String {
    let before = s.model.clone();
    let e = s.execute(id, &p).expect_err(&format!("{id} {p} should fail"));
    assert_eq!(*s.model, *before, "a refused {id} changes nothing");
    e
}

/// A project with a historic wall, 4 m long and 3 m tall on Level 1: lime plaster, rubble masonry and lime plaster again
/// (20 + 460 + 20 mm). The session, and the wall's id.
fn historic() -> (Session, Id) {
    let mut s = Session::new();
    for (name, color) in [("Lime Plaster", "#e8e0d0"), ("Rubble Masonry", "#9a8f80")] {
        run(&mut s, "material", json!({"name": name, "color": color}));
    }
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 500mm", "layers": [
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 460},
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
        ]}),
    );
    let wall =
        run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "type": "Historic - 500mm", "height": 3000}))["ids"][0].as_u64().unwrap();
    (s, wall)
}

/// The GUIDs of the layers of wall type `name`, from its exterior in.
fn layers(m: &Model, name: &str) -> Vec<Guid> {
    m.types.wall[name].layers.iter().map(|l| l.guid).collect()
}

/// A part of layer `k` of wall `wall`'s type.
fn layer_part(m: &Model, wall: Id, k: usize) -> BuildingPart {
    let type_name = m.wall(wall).unwrap().type_name.clone();
    BuildingPart::new(m.guid(wall), Source::Layer(layers(m, &type_name)[k]))
}

/// What the model says of itself with part `p` put in it.
fn checked(s: &Session, p: BuildingPart) -> Result<(), String> {
    let mut m = (*s.model).clone();
    m.insert(Element::Part(p));
    m.check()
}

/// The part `make` makes of the project put in it, which must take it: its id.
fn put(s: &mut Session, make: impl FnOnce(&Model) -> BuildingPart) -> Id {
    let p = make(&s.model);
    let id = s.model_mut().insert(Element::Part(p));
    assert_eq!(s.model.check(), Ok(()));
    id
}

fn part(m: &Model, id: Id) -> &BuildingPart {
    let Some(Element::Part(p)) = m.elements.get(id) else { panic!("no part {id}") };
    p
}

#[test]
fn a_part_is_an_element_with_an_id_and_a_guid_that_stands_on_its_hosts_level() {
    let (mut s, wall) = historic();
    let p = layer_part(&s.model, wall, 0);
    let id = put(&mut s, |_| p);
    let m = &s.model;
    assert!(id > wall && m.elements.guid(id).is_some_and(|g| g.pending().is_none()));
    let e = m.elements.get(id).unwrap();
    assert_eq!(e.category(), "part");
    assert_eq!(e.refs(), [m.guid(wall)], "it points at its host; its layer is a part of a type, no element");
    assert_eq!(m.level_of(e), Some(m.wall(wall).unwrap().level));
    // It is listed and read as any element: its own fields, its id, its GUID and its level.
    let listed = run(&mut s, "elements", json!({"category": "part"}));
    assert_eq!(listed["count"], 1);
    let one = &listed["elements"][0];
    assert_eq!((one["id"].as_u64(), one["category"].as_str()), (Some(id), Some("part")));
    assert_eq!(one["host"], json!(s.model.guid(wall)));
    assert_eq!(one["level"], json!(s.model.wall(wall).unwrap().level));
}

#[test]
fn only_a_wall_a_floor_a_roof_a_ceiling_or_a_component_has_parts() {
    let (mut s, wall) = historic();
    let floor = run(&mut s, "floor", json!({"boundary": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]]}))["id"].as_u64().unwrap();
    let grid = run(&mut s, "grid", json!({"start": [0, -1000], "end": [4000, -1000]}))["id"].as_u64().unwrap();
    let level = s.model.levels()[0].0;
    for host in [wall, floor] {
        assert_eq!(checked(&s, BuildingPart::new(s.model.guid(host), Source::Whole)), Ok(()));
    }
    for (host, cat) in [(grid, "grid"), (level, "level")] {
        let e = checked(&s, BuildingPart::new(s.model.guid(host), Source::Whole)).unwrap_err();
        assert!(e.contains(&format!("{cat} {host} has no parts; a wall, a floor, a roof, a ceiling or a component has")), "{e}");
    }
    // A part of a part is a division of it, which names it as its parent, never as its host.
    let whole = put(&mut s, |m| BuildingPart::new(m.guid(wall), Source::Whole));
    let e = checked(&s, BuildingPart::new(s.model.guid(whole), Source::Whole)).unwrap_err();
    assert!(e.contains(&format!("part {whole} has no parts")), "{e}");
    let e = checked(&s, BuildingPart::new(Guid::new(), Source::Whole)).unwrap_err();
    assert!(e.contains("its host") && e.contains("doesn't exist"), "{e}");
}

#[test]
fn a_part_divides_from_another_part_of_its_host_and_never_from_itself() {
    let (mut s, wall) = historic();
    let other = run(&mut s, "wall", json!({"points": [[0, 5000], [4000, 5000]]}))["ids"][0].as_u64().unwrap();
    let (host, far) = (s.model.guid(wall), s.model.guid(other));
    let top = put(&mut s, |_| BuildingPart::new(host, Source::Whole));
    let elsewhere = put(&mut s, |_| BuildingPart::new(far, Source::Whole));
    let child = put(&mut s, |m| BuildingPart { parent: Some(m.guid(top)), ..BuildingPart::new(host, Source::Whole) });
    assert_eq!(part(&s.model, child).parent, Some(s.model.guid(top)));
    assert_eq!(s.model.elements.get(child).unwrap().refs(), [host, s.model.guid(top)]);
    // From a wall, from a part of another wall: no.
    let e = checked(&s, BuildingPart { parent: Some(far), ..BuildingPart::new(host, Source::Whole) }).unwrap_err();
    assert!(e.contains(&format!("its parent {other} isn't a part")), "{e}");
    let e = checked(&s, BuildingPart { parent: Some(s.model.guid(elsewhere)), ..BuildingPart::new(host, Source::Whole) }).unwrap_err();
    assert!(e.contains(&format!("its parent {elsewhere} is a part of another element")), "{e}");
    // From itself, or round in a ring: no.
    let mut m = (*s.model).clone();
    let (top_guid, child_guid) = (m.guid(top), m.guid(child));
    if let Some(Element::Part(p)) = m.elements.get_mut(top) {
        p.parent = Some(top_guid);
    }
    assert!(m.check().unwrap_err().contains("can't be divided from itself"), "{:?}", m.check());
    if let Some(Element::Part(p)) = m.elements.get_mut(top) {
        p.parent = Some(child_guid);
    }
    assert!(m.check().unwrap_err().contains("nor from a part divided from it"), "{:?}", m.check());
}

#[test]
fn a_layer_is_named_by_its_guid() {
    let (s, wall) = historic();
    let host = s.model.guid(wall);
    assert_eq!(checked(&s, layer_part(&s.model, wall, 1)), Ok(()));
    for not_one in [Guid::default(), Guid::of_id(wall)] {
        let e = checked(&s, BuildingPart::new(host, Source::Layer(not_one))).unwrap_err();
        assert!(e.contains("`source` names a layer by its GUID"), "{e}");
    }
    // A GUID no layer of the host's type has is no error: the part is in conflict, and the model lives with it.
    assert_eq!(checked(&s, BuildingPart::new(host, Source::Layer(Guid::new()))), Ok(()));
}

#[test]
fn a_parts_own_material_is_the_librarys_follows_its_rename_and_keeps_it_in_the_library() {
    let (mut s, wall) = historic();
    let base = layer_part(&s.model, wall, 0);
    let e = checked(&s, BuildingPart { material: Some("Cocciopesto".into()), ..base.clone() }).unwrap_err();
    assert!(e.contains("material `Cocciopesto` isn't in the library; add it with `material`"), "{e}");
    let e = checked(&s, BuildingPart { material: Some("  ".into()), ..base.clone() }).unwrap_err();
    assert!(e.contains("leave `material` out to take its layer's"), "{e}");
    run(&mut s, "material", json!({"name": "Cocciopesto", "color": "#b5651d"}));
    let id = put(&mut s, |_| BuildingPart { material: Some("Cocciopesto".into()), ..base });
    // In use, it stays; renamed, the part names it as it is now.
    let e = fails(&mut s, "material", json!({"name": "Cocciopesto", "delete": true}));
    assert!(e.contains(&format!("`Cocciopesto` is in part {id}; take it out of it first")), "{e}");
    let unused = run(&mut s, "purge", json!({"kinds": ["material"], "dry_run": true}));
    assert!(unused["removed"].as_array().unwrap().iter().all(|x| x["name"] != "Cocciopesto"), "{unused}");
    run(&mut s, "material", json!({"name": "Cocciopesto", "rename": "Opus Signinum"}));
    assert_eq!(part(&s.model, id).material.as_deref(), Some("Opus Signinum"));
    // A file holds it by its GUID, as a layer's material.
    let file: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    let held = file[FILE_KEY][id.to_string()]["material"].as_str().unwrap();
    assert!(Guid::parse(held).is_some_and(|g| g.pending().is_none()), "{held}");
    let back = Model::from_json(&s.model.to_json().unwrap()).unwrap();
    assert_eq!(part(&back, id).material.as_deref(), Some("Opus Signinum"));
}

#[test]
fn a_user_defined_part_says_what_it_is() {
    let (s, wall) = historic();
    let base = layer_part(&s.model, wall, 1);
    let e = checked(&s, BuildingPart { predefined_type: PartType::UserDefined, ..base.clone() }).unwrap_err();
    assert!(e.contains("a user-defined part says what it is; give it an `object_type`"), "{e}");
    assert_eq!(checked(&s, BuildingPart { predefined_type: PartType::UserDefined, object_type: "USM".into(), ..base.clone() }), Ok(()));
    // Any part may say; IFC's own kinds need not.
    assert_eq!(checked(&s, BuildingPart { predefined_type: PartType::PrecastPanel, ..base.clone() }), Ok(()));
    assert_eq!(checked(&s, BuildingPart { object_type: "Infill".into(), ..base.clone() }), Ok(()));
    let e = checked(&s, BuildingPart { object_type: "x".repeat(257), ..base }).unwrap_err();
    assert!(e.contains("its object type is longer than 256 characters"), "{e}");
    for t in PartType::ALL {
        assert_eq!((PartType::parse(t.name()), PartType::parse(t.ifc())), (Some(t), Some(t)));
    }
    assert_eq!(PartType::parse("panel"), None);
}

#[test]
fn the_heritage_set_holds_five_values_and_a_severity_of_1_to_5() {
    let (s, wall) = historic();
    let base = layer_part(&s.model, wall, 0);
    let full = Heritage {
        su_code: "USM 12".into(),
        historical_period: "Late medieval".into(),
        decay_pathology: Some(Decay::Efflorescence),
        decay_severity: Some(3),
        intervention_type: "Cleaning".into(),
    };
    assert_eq!(checked(&s, BuildingPart { heritage: full.clone(), ..base.clone() }), Ok(()));
    assert_eq!(Heritage::NAMES, ["SU_Code", "Historical_Period", "Decay_Pathology", "Decay_Severity", "Intervention_Type"]);
    for (severity, ok) in [(0, false), (1, true), (5, true), (6, false), (255, false)] {
        let got = checked(&s, BuildingPart { heritage: Heritage { decay_severity: Some(severity), ..full.clone() }, ..base.clone() });
        assert_eq!(got.is_ok(), ok, "{severity}: {got:?}");
        if let Err(e) = got {
            assert!(e.contains(&format!("Decay_Severity is 1 (slight) to 5 (the worst), not {severity}")), "{e}");
        }
    }
    // A decay of the project's own carries its own words; text has its limit.
    let own = Heritage { decay_pathology: Some(Decay::Custom("Lacuna in the fresco".into())), ..full.clone() };
    assert_eq!(checked(&s, BuildingPart { heritage: own, ..base.clone() }), Ok(()));
    let blank = Heritage { decay_pathology: Some(Decay::Custom(" ".into())), ..full.clone() };
    assert!(checked(&s, BuildingPart { heritage: blank, ..base.clone() }).unwrap_err().contains("a decay of the project's own has a name"));
    for long in [
        Heritage { su_code: "x".repeat(257), ..full.clone() },
        Heritage { historical_period: "x".repeat(257), ..full.clone() },
        Heritage { decay_pathology: Some(Decay::Custom("x".repeat(257))), ..full.clone() },
        Heritage { intervention_type: "x".repeat(257), ..full.clone() },
    ] {
        let e = checked(&s, BuildingPart { heritage: long, ..base.clone() }).unwrap_err();
        assert!(e.contains("is longer than 256 characters"), "{e}");
    }
}

#[test]
fn a_decay_is_one_of_the_lists_words_however_it_is_written_or_the_projects_own() {
    assert_eq!(Decay::NAMED.len(), 26);
    for d in Decay::NAMED {
        assert_eq!(Decay::named(d.label()), Some(d.clone()), "{}", d.label());
        // In a file, its name in lower case with hyphens; read back, the same.
        let written = serde_json::to_value(&d).unwrap();
        assert_eq!(Decay::named(written.as_str().unwrap()), Some(d.clone()));
        assert_eq!(serde_json::from_value::<Decay>(written).unwrap(), d);
    }
    assert_eq!(serde_json::to_value(Decay::MissingPart).unwrap(), json!("missing-part"));
    assert_eq!(serde_json::to_value(Decay::Custom("Lacuna".into())).unwrap(), json!({"custom": "Lacuna"}));
    for (text, d) in [
        ("MISSING_PART", Decay::MissingPart),
        ("moist area", Decay::MoistArea),
        ("Discoloration", Decay::Discolouration),
        ("biological colonisation", Decay::BiologicalColonization),
        (" Crack ", Decay::Crack),
    ] {
        assert_eq!(Decay::named(text), Some(d), "{text}");
    }
    assert_eq!(Decay::named("Lacuna"), None);
    assert_eq!(Decay::Custom("Lacuna".into()).label(), "Lacuna");
}

#[test]
fn two_parts_of_one_layer_of_a_host_are_refused() {
    let (mut s, wall) = historic();
    let plaster = layer_part(&s.model, wall, 0);
    let first = put(&mut s, |_| plaster.clone());
    let e = checked(&s, plaster.clone()).unwrap_err();
    assert!(e.contains(&format!("part {first} is that of wall {wall} already; delete one of them")), "{e}");
    // The other layers, and what is divided from it, are other things.
    assert_eq!(checked(&s, layer_part(&s.model, wall, 1)), Ok(()));
    assert_eq!(checked(&s, layer_part(&s.model, wall, 2)), Ok(()));
    assert_eq!(checked(&s, BuildingPart { parent: Some(s.model.guid(first)), ..plaster }), Ok(()));
}

#[test]
fn a_part_goes_with_its_host_and_with_the_part_it_was_divided_from() {
    let (mut s, wall) = historic();
    let other = run(&mut s, "wall", json!({"points": [[0, 5000], [4000, 5000]]}))["ids"][0].as_u64().unwrap();
    let plaster = put(&mut s, |m| layer_part(m, wall, 0));
    let masonry = put(&mut s, |m| layer_part(m, wall, 1));
    let patch = put(&mut s, |m| BuildingPart { parent: Some(m.guid(plaster)), ..layer_part(m, wall, 0) });
    let stays = put(&mut s, |m| BuildingPart::new(m.guid(other), Source::Whole));
    // A part alone, with what was divided from it.
    let gone = run(&mut s, "delete", json!({"ids": [plaster]}));
    assert_eq!(gone["deleted"], json!([plaster, patch]));
    run(&mut s, "undo", json!({}));
    assert!(s.model.elements.contains_key(patch));
    // Its host, as Revit deletes the parts with the original: all of its parts, and no other's.
    let gone = run(&mut s, "delete", json!({"ids": [wall]}));
    assert_eq!(gone["deleted"], json!([wall, plaster, masonry, patch]));
    assert!(s.model.elements.contains_key(stays));
    run(&mut s, "undo", json!({}));
    assert_eq!(part(&s.model, patch).parent, Some(s.model.guid(plaster)));
    run(&mut s, "redo", json!({}));
    assert!(!s.model.elements.contains_key(masonry));
}

#[test]
fn a_part_has_no_place_of_its_own_to_move_copy_turn_or_mirror() {
    let (mut s, wall) = historic();
    let id = put(&mut s, |m| layer_part(m, wall, 0));
    for (cmd, p) in [
        ("move", json!({"ids": [id], "by": [1000, 0]})),
        ("copy", json!({"ids": [id], "by": [1000, 0]})),
        ("rotate", json!({"ids": [id], "about": [0, 0], "angle": 90})),
        ("mirror", json!({"ids": [id], "axis": [[0, 0], [0, 1000]]})),
    ] {
        let e = fails(&mut s, cmd, p);
        assert!(e.contains(&format!("part {id} is where the element it is a part of is")), "{cmd}: {e}");
    }
    // With its host it goes, changed in nothing: it is of the wall wherever the wall is.
    let before = part(&s.model, id).clone();
    run(&mut s, "move", json!({"ids": [wall, id], "by": [1000, 500]}));
    assert_eq!(*part(&s.model, id), before);
    run(&mut s, "move", json!({"ids": [wall], "by": [-1000, -500]}));
    assert_eq!(*part(&s.model, id), before);
}

#[test]
fn nothing_but_a_part_points_at_a_part_yet() {
    let (mut s, wall) = historic();
    let id = put(&mut s, |m| layer_part(m, wall, 0));
    let level = s.model.levels()[0].0;
    // A view's own override of it, which a file would hold outside the parts: an older BIMCraft, leaving the parts out,
    // would find it pointing at nothing.
    let mut m = (*s.model).clone();
    let guid = m.guid(id);
    let Some(Element::Level(l)) = m.elements.get_mut(level) else { panic!("no level") };
    l.graphics.elements.insert(guid, crate::graphics::Override { hidden: true, ..Default::default() });
    let e = m.check().unwrap_err();
    assert_eq!(e, format!("level {level} points at part {id}; nothing but a part points at a part yet"));
    assert_eq!(m.check_changed(&s.model), Err(e));
    // The command says it its own way: a part is drawn in no view yet. Its wall may be hidden, as ever.
    let e = fails(&mut s, "graphics", json!({"view": level, "hide_elements": [id]}));
    assert!(e.contains(&format!("element {id} is a part, and views don't draw those")), "{e}");
    run(&mut s, "graphics", json!({"view": level, "hide_elements": [wall]}));
}

#[test]
fn a_file_keeps_its_parts_under_a_key_of_their_own_and_reads_them_back_as_elements() {
    let (mut s, wall) = historic();
    let heritage = Heritage {
        su_code: "USR 3".into(),
        historical_period: "18th century".into(),
        decay_pathology: Some(Decay::Detachment),
        decay_severity: Some(4),
        intervention_type: "Consolidation".into(),
    };
    let plaster = put(&mut s, |m| BuildingPart { heritage, excluded: true, ..layer_part(m, wall, 0) });
    let masonry =
        put(&mut s, |m| BuildingPart { predefined_type: PartType::UserDefined, object_type: "USM".into(), ..layer_part(m, wall, 1) });
    let patch = put(&mut s, |m| BuildingPart { parent: Some(m.guid(plaster)), ..layer_part(m, wall, 0) });
    let text = s.model.to_json().unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    // The format is the one a BIMCraft from before parts reads, and no element is of a category it doesn't know.
    assert_eq!(file["format"], 18);
    let elements = file["elements"].as_object().unwrap();
    assert!(elements.values().all(|e| e["category"] != "part"));
    assert_eq!(elements.len() + 3, s.model.elements.len());
    let parts = file[FILE_KEY].as_object().unwrap();
    assert_eq!(parts.keys().map(String::as_str).collect::<Vec<_>>(), [plaster.to_string(), masonry.to_string(), patch.to_string()]);
    // Each as an element is written: its GUID, what it points at by GUID, and only what it says.
    let p = &parts[&plaster.to_string()];
    assert_eq!(p["guid"], json!(s.model.guid(plaster)));
    assert_eq!(p["host"], json!(s.model.guid(wall)));
    assert_eq!(p["source"], json!({"layer": layers(&s.model, "Historic - 500mm")[0]}));
    assert_eq!(p["excluded"], true);
    assert_eq!(
        p["heritage"],
        json!({"su_code": "USR 3", "historical_period": "18th century", "decay_pathology": "detachment", "decay_severity": 4,
            "intervention_type": "Consolidation"})
    );
    let m = &parts[&masonry.to_string()];
    assert_eq!((&m["predefined_type"], &m["object_type"]), (&json!("userdefined"), &json!("USM")));
    assert!(m.get("heritage").is_none() && m.get("excluded").is_none() && m.get("parent").is_none() && m.get("params").is_none());
    assert_eq!(parts[&patch.to_string()]["parent"], json!(s.model.guid(plaster)));
    // Read back, they are elements again, each with its id and its GUID, and the project is what it was.
    let (back, left_out) = Model::read(&text).unwrap();
    assert_eq!(left_out, Vec::<String>::new());
    assert_eq!(back.check(), Ok(()));
    assert_eq!(back.elements, s.model.elements);
    assert_eq!(back.to_json().unwrap(), text);
    // A project with no parts is written as it always was: no key for them.
    run(&mut s, "delete", json!({"ids": [wall]}));
    let bare: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    assert!(bare.get(FILE_KEY).is_none());
}

#[test]
fn a_reader_that_leaves_the_parts_out_loses_nothing_else() {
    let (mut s, wall) = historic();
    run(&mut s, "door", json!({"at": [2000, 0]}));
    let plaster = put(&mut s, |m| layer_part(m, wall, 0));
    put(&mut s, |m| BuildingPart { parent: Some(m.guid(plaster)), ..layer_part(m, wall, 0) });
    let without = {
        let mut m = (*s.model).clone();
        m.elements.retain(|_, e| !matches!(e, Element::Part(_)));
        m
    };
    // As a BIMCraft from before parts reads the file: the key is one it doesn't know, so it leaves it out, names it,
    // and what it reads is the project without its parts, which stands on its own.
    let mut file: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    let parts = file.as_object_mut().unwrap().remove(FILE_KEY).unwrap();
    file["parts_of_a_later_bimcraft"] = parts;
    let (older, left_out) = Model::read(&file.to_string()).unwrap();
    assert_eq!(left_out, ["/parts_of_a_later_bimcraft"]);
    assert_eq!(older.check(), Ok(()));
    assert_eq!(older.elements, without.elements);
    assert_eq!(older.to_json().unwrap(), without.to_json().unwrap());
    // The same with the key taken out altogether.
    file.as_object_mut().unwrap().remove("parts_of_a_later_bimcraft");
    let (bare, left_out) = Model::read(&file.to_string()).unwrap();
    assert_eq!((left_out, bare.check()), (Vec::new(), Ok(())));
    assert_eq!(bare.elements, without.elements);
}

#[test]
fn a_file_is_read_kindly_whatever_its_parts_key_holds() {
    let (mut s, wall) = historic();
    let id = put(&mut s, |m| layer_part(m, wall, 0));
    let text = s.model.to_json().unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    // A field of a part this BIMCraft doesn't know is left out and named, as any element's.
    let mut later = file.clone();
    later[FILE_KEY][id.to_string()]["weathering_index"] = json!(7);
    let (m, left_out) = Model::read(&later.to_string()).unwrap();
    assert_eq!(left_out, [format!("/elements/{id}/weathering_index")]);
    assert_eq!(m.elements, s.model.elements);
    // The key holding something else, or a part whose id an element has: left out and named, the rest opens.
    for (hostile, named) in
        [(json!([1, 2]), true), (json!("parts"), true), (json!({wall.to_string(): {"host": 1}}), true), (json!({}), false)]
    {
        let mut f = file.clone();
        f[FILE_KEY] = hostile.clone();
        let (m, left_out) = Model::read(&f.to_string()).unwrap_or_else(|e| panic!("{hostile}: {e}"));
        assert_eq!(left_out, if named { vec![format!("/{FILE_KEY}")] } else { Vec::new() }, "{hostile}");
        assert!(m.wall(wall).is_some() && !m.elements.contains_key(id), "{hostile}");
    }
    // What is no part at all is refused as a hostile element is, with its id.
    let mut f = file.clone();
    f[FILE_KEY][id.to_string()] = json!({"host": "not a guid"});
    let e = Model::read(&f.to_string()).unwrap_err();
    assert!(e.contains(&format!("element {id}")), "{e}");
    // In `elements`, as this BIMCraft writes one to anything but a file, a part is read too.
    let mut inline = file.clone();
    let part = inline[FILE_KEY].as_object_mut().unwrap().remove(&id.to_string()).unwrap();
    inline.as_object_mut().unwrap().remove(FILE_KEY);
    inline["elements"][id.to_string()] = part;
    inline["elements"][id.to_string()]["category"] = json!("part");
    assert_eq!(Model::read(&inline.to_string()).unwrap().0.elements, s.model.elements);
}
