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
    assert!(e.ends_with(&format!("part {first}: part {} is that of wall {wall} too; delete one of them", first + 1)), "{e}");
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

// ---------- what a part is of its host now ----------

use crate::parts::solid;
use crate::view3d::P3;

/// The box round a solid's corners: its lowest and its highest, each way.
fn extent(tris: &[[P3; 3]]) -> ([f64; 3], [f64; 3]) {
    tris.iter().flatten().fold(([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]), |(lo, hi), p| {
        (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k])))
    })
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * (1.0 + b.abs())
}

/// Part `id` as `elements` lists it.
fn shown(s: &mut Session, id: Id) -> Value {
    let listed = run(s, "elements", json!({"category": "part"}));
    listed["elements"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("no part {id} listed")).clone()
}

/// The three parts of the historic wall, exterior plaster first, put in the project.
fn three(s: &mut Session, wall: Id) -> [Id; 3] {
    [0, 1, 2].map(|k| put(s, |m| layer_part(m, wall, k)))
}

/// Part `id`'s solid, which it must have, checked to be closed; with its net area and its volume.
fn measured(m: &Model, id: Id) -> (Vec<[P3; 3]>, f64, f64) {
    let p = part(m, id);
    let tris = m.part_solid(p).unwrap_or_else(|e| panic!("part {id}: {e}"));
    assert!(solid::closed(&tris), "part {id}'s solid is closed");
    let (area, volume) = m.part_quantities(p).unwrap();
    assert!(near(volume, solid::volume(&tris)));
    (tris, area.unwrap_or(f64::NAN), volume)
}

#[test]
fn a_walls_part_is_its_layer_between_the_layers_faces() {
    let (mut s, wall) = historic();
    let [outer, core, inner] = three(&mut s, wall);
    let whole = put(&mut s, |m| BuildingPart::new(m.guid(wall), Source::Whole));
    let m = &s.model;
    // The wall runs east, so its exterior side is north: 250 mm from its centre line, the plaster's first 20 of them.
    for (id, (y0, y1), thick) in [(outer, (230.0, 250.0), 20.0), (core, (-230.0, 230.0), 460.0), (inner, (-250.0, -230.0), 20.0)] {
        let (tris, area, volume) = measured(m, id);
        let (lo, hi) = extent(&tris);
        assert!(near(lo[1], y0) && near(hi[1], y1), "part {id} lies from {} to {} across the wall", lo[1], hi[1]);
        assert_eq!(([lo[0], lo[2]], [hi[0], hi[2]]), ([0.0, 0.0], [4000.0, 3000.0]));
        assert!(near(volume, 4000.0 * 3000.0 * thick), "{volume}");
        assert!(near(area, 4000.0 * 3000.0), "its face toward the exterior: {area}");
        assert_eq!(m.part_thickness(part(m, id)), Some(thick));
    }
    assert_eq!(m.part_material(part(m, outer)), Some("Lime Plaster"));
    assert_eq!(m.part_material(part(m, core)), Some("Rubble Masonry"));
    // All of it: the wall's own thickness, the material of its structure, the sum of its layers.
    let (tris, area, volume) = measured(m, whole);
    assert_eq!(extent(&tris), ([0.0, -250.0, 0.0], [4000.0, 250.0, 3000.0]));
    assert!(near(volume, 4000.0 * 3000.0 * 500.0) && near(area, 4000.0 * 3000.0));
    assert_eq!((m.part_thickness(part(m, whole)), m.part_material(part(m, whole))), (Some(500.0), Some("Rubble Masonry")));
    let state = m.part_state(core).unwrap();
    assert_eq!(
        (state.layer, state.material.as_str(), state.thickness),
        (Some((1, crate::model::Function::Structure)), "Rubble Masonry", Some(460.0))
    );
    assert_eq!((state.net_area.map(f64::round), state.net_volume.round(), state.conflict), (Some(12e6), 5.52e9, None));
    assert_eq!(m.part_state(wall), None, "a wall is no part");
    assert_eq!(m.parts_of(wall), [outer, core, inner, whole]);
    // Triangles turned out: a corner of the exterior plaster's north face, its normal north.
    let (tris, ..) = measured(m, outer);
    let north = tris.iter().filter(|t| t.iter().all(|p| near(p[1], 250.0))).count();
    assert_eq!(north, 2, "its exterior face is two triangles");
    assert!(near(solid::facing(&tris, &solid::Facing::Along(vec![[0.0, 1.0, 0.0]])), 12e6));
}

#[test]
fn a_walls_parts_are_cut_by_its_doors_and_windows_and_measure_net_of_them() {
    let (mut s, wall) = historic();
    let ids = three(&mut s, wall);
    run(&mut s, "type", json!({"category": "window", "name": "W 1000x1200", "width": 1000, "height": 1200, "sill": 900}));
    run(&mut s, "type", json!({"category": "door", "name": "D 900x2100", "width": 900, "height": 2100}));
    run(&mut s, "window", json!({"at": [1500, 0], "type": "W 1000x1200"}));
    run(&mut s, "door", json!({"at": [3000, 0], "type": "D 900x2100"}));
    let net = 4000.0 * 3000.0 - 1000.0 * 1200.0 - 900.0 * 2100.0;
    for (id, thick) in ids.into_iter().zip([20.0, 460.0, 20.0]) {
        let (_, area, volume) = measured(&s.model, id);
        assert!(near(area, net), "part {id}: {area}");
        assert!(near(volume, net * thick), "part {id}: {volume}");
    }
    // The window's reveals are faces of the masonry that look along the wall, not toward its exterior.
    let (tris, ..) = measured(&s.model, ids[1]);
    let along = solid::facing(&tris, &solid::Facing::Along(vec![[1.0, 0.0, 0.0]]));
    assert!(near(along, 460.0 * (3000.0 + 1200.0 + 2100.0)), "{along}");
    // Taken out again, the parts are whole again: nothing was stored.
    let openings: Vec<Id> =
        s.model.elements.iter().filter(|(_, e)| matches!(e, Element::Door(_) | Element::Window(_))).map(|(id, _)| *id).collect();
    run(&mut s, "delete", json!({"ids": openings}));
    assert!(near(measured(&s.model, ids[0]).1, 12e6));
}

#[test]
fn a_part_follows_its_wall_when_it_moves_stretches_or_its_layers_change() {
    let (mut s, wall) = historic();
    let [outer, core, _] = three(&mut s, wall);
    let before = part(&s.model, outer).clone();
    // Moved: its solid is where the wall is.
    run(&mut s, "move", json!({"ids": [wall], "by": [1000, 2000]}));
    let (tris, area, _) = measured(&s.model, outer);
    assert_eq!(extent(&tris), ([1000.0, 2230.0, 0.0], [5000.0, 2250.0, 3000.0]));
    assert!(near(area, 12e6));
    // Stretched and raised: as long and as tall as it is now.
    run(&mut s, "modify", json!({"id": wall, "end": [7000, 2000], "height": 3500}));
    let (_, area, volume) = measured(&s.model, outer);
    assert!(near(area, 6000.0 * 3500.0) && near(volume, 6000.0 * 3500.0 * 20.0));
    // Its type's layers changed where they are: a thicker plaster, a thinner wall behind it.
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 500mm", "layers": [
            {"function": "finish", "material": "Lime Plaster", "thickness": 35},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 400},
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
        ]}),
    );
    assert_eq!(s.model.part_thickness(part(&s.model, outer)), Some(35.0));
    assert!(near(measured(&s.model, outer).2, 6000.0 * 3500.0 * 35.0));
    assert!(near(measured(&s.model, core).2, 6000.0 * 3500.0 * 400.0));
    // The part itself is as it was: a recipe, with nothing of the wall's shape in it.
    assert_eq!(*part(&s.model, outer), before);
    run(&mut s, "undo", json!({"count": 3}));
    assert!(near(measured(&s.model, outer).2, 4000.0 * 3000.0 * 20.0));
}

#[test]
fn walls_that_join_give_their_parts_their_ends_and_curved_ones_their_bow() {
    let (mut s, wall) = historic();
    // A second wall of the type from the first's end, turning north-west: a mitre.
    let other = run(&mut s, "wall", json!({"points": [[4000, 0], [2000, 3000]], "type": "Historic - 500mm", "height": 3000}))["ids"][0]
        .as_u64()
        .unwrap();
    // And a curved one on its own.
    let bow =
        run(&mut s, "wall", json!({"points": [[0, 8000], [4000, 8000]], "arc": 90, "type": "Historic - 500mm", "height": 3000}))["ids"][0]
            .as_u64()
            .unwrap();
    for host in [wall, other, bow] {
        let outline = crate::geom::area(&crate::walls::outline(&s.model, host)).abs();
        let parts = three(&mut s, host);
        let whole = put(&mut s, |m| BuildingPart::new(m.guid(host), Source::Whole));
        let layers: f64 = parts.iter().map(|id| measured(&s.model, *id).2).sum();
        let (_, _, all) = measured(&s.model, whole);
        assert!((layers - all).abs() <= 1e-6 * all, "wall {host}: its layers hold {layers}, all of it {all}");
        assert!((all - outline * 3000.0).abs() <= 1e-6 * all, "wall {host}: {all} is its outline, {outline} mm², swept up");
    }
    // At the mitre the first wall's exterior face (north, inside the corner) is shorter than its interior one.
    let (_, outer, _) = measured(&s.model, s.model.parts_of(wall)[0]);
    assert!(outer < 12e6 - 1e5, "{outer}");
    // The curved wall's exterior plaster: its face is the arc at its exterior side, a little short of it in flat pieces.
    let w = s.model.wall(bow).unwrap();
    let axis = crate::walls::Axis::of(w);
    let (radius, sweep) = (axis.radius().unwrap(), 90f64.to_radians());
    let (_, area, volume) = measured(&s.model, s.model.parts_of(bow)[0]);
    let face = (radius - 250.0) * sweep * 3000.0;
    assert!(area <= face && area > 0.999 * face, "{area} of {face}");
    let ring = (radius - 240.0) * sweep * 20.0 * 3000.0;
    assert!((volume - ring).abs() < 0.002 * ring, "{volume} of {ring}");
}

#[test]
fn a_wall_under_a_roof_gives_its_parts_its_top() {
    let mut s = Session::new();
    s.script(
        r#"wall {"points": [[0, 0], [8000, 0], [8000, 6000], [0, 6000]], "closed": true}
           roof {"gable": true}"#,
    )
    .unwrap();
    let (gable_wall, roof) = (100003, 100006);
    assert!(matches!(s.model.elements.get(roof), Some(Element::Roof(_))));
    run(&mut s, "modify", json!({"id": gable_wall, "attach_top": roof}));
    let id = put(&mut s, |m| BuildingPart::new(m.guid(gable_wall), Source::Whole));
    let (tris, area, volume) = measured(&s.model, id);
    let prisms: f64 = crate::walls::prisms(&s.model, gable_wall, true)
        .iter()
        .map(|p| {
            let c = crate::geom::centroid(&p.ring);
            crate::geom::area(&p.ring).abs() * (p.top.at(c) - p.bottom.at(c))
        })
        .sum();
    assert!((volume - prisms).abs() <= 1e-6 * prisms, "{volume} of {prisms}");
    // Taller in the middle than at its ends, its side a pentagon.
    let (_, hi) = extent(&tris);
    assert!(hi[2] > 4000.0, "{hi:?}");
    assert!(area > 6000.0 * 3000.0 && area < 6000.0 * hi[2], "{area}");
}

/// A floor of a type of three layers (20 mm finish, 60 mm screed, 200 mm slab), 5 by 4 m with a hole a metre square, on
/// Level 1: the session, and the floor's id.
fn slab(more: Value) -> (Session, Id) {
    let mut s = Session::new();
    run(&mut s, "material", json!({"name": "Cocciopesto", "color": "#b5651d"}));
    run(
        &mut s,
        "type",
        json!({"category": "floor", "name": "Historic Floor", "layers": [
            {"function": "finish", "material": "Cocciopesto", "thickness": 20},
            {"function": "substrate", "thickness": 60},
            {"function": "structure", "thickness": 200},
        ]}),
    );
    let mut p = json!({"boundary": [[0, 0], [5000, 0], [5000, 4000], [0, 4000]], "type": "Historic Floor",
        "openings": [[[1000, 1000], [2000, 1000], [2000, 2000], [1000, 2000]]]});
    p.as_object_mut().unwrap().extend(more.as_object().cloned().unwrap_or_default());
    let floor = run(&mut s, "floor", p)["id"].as_u64().unwrap();
    (s, floor)
}

/// A part of layer `k` of floor `floor`'s type, put in the project.
fn floor_part(s: &mut Session, floor: Id, k: usize) -> Id {
    put(s, |m| {
        let Some(Element::Floor(f)) = m.elements.get(floor) else { panic!("no floor {floor}") };
        BuildingPart::new(m.guid(floor), Source::Layer(m.types.floor[&f.type_name].layers[k].guid))
    })
}

#[test]
fn a_floors_part_is_its_layer_from_the_top_down_less_its_openings_and_shafts() {
    let (mut s, floor) = slab(json!({}));
    let ids = [0, 1, 2].map(|k| floor_part(&mut s, floor, k));
    let plan = 5000.0 * 4000.0 - 1000.0 * 1000.0;
    for (id, (z0, z1)) in ids.into_iter().zip([(-20.0, 0.0), (-80.0, -20.0), (-280.0, -80.0)]) {
        let (tris, area, volume) = measured(&s.model, id);
        let (lo, hi) = extent(&tris);
        assert!(near(lo[2], z0) && near(hi[2], z1), "part {id} lies from {} to {}", lo[2], hi[2]);
        assert!(near(area, plan) && near(volume, plan * (z1 - z0)), "part {id}: {area} mm², {volume} mm³");
    }
    assert_eq!(s.model.part_material(part(&s.model, ids[0])), Some("Cocciopesto"));
    assert_eq!(s.model.part_material(part(&s.model, ids[2])), None, "its layer names no material");
    // A shaft from the level up only meets its top, and cuts nothing of it.
    let shaft = run(&mut s, "shaft", json!({"boundary": [[3000, 2500], [4000, 2500], [4000, 3500], [3000, 3500]]}))["id"].as_u64().unwrap();
    assert!(near(measured(&s.model, ids[1]).1, plan));
    // One that reaches into it, be it only into its finish, cuts all of it: its plan out of every layer.
    run(&mut s, "modify", json!({"id": shaft, "base_offset": -10}));
    let (_, area, volume) = measured(&s.model, ids[1]);
    assert!(near(area, plan - 1e6) && near(volume, (plan - 1e6) * 60.0), "{area} mm², {volume} mm³");
    // All of it holds what the floor does.
    let whole = put(&mut s, |m| BuildingPart::new(m.guid(floor), Source::Whole));
    let Some(Element::Floor(f)) = s.model.elements.get(floor) else { panic!() };
    assert!(near(measured(&s.model, whole).2, s.model.floor_volume(f)));
    assert_eq!(s.model.part_thickness(part(&s.model, whole)), Some(280.0));
}

#[test]
fn a_sloped_floors_part_measures_its_top_along_the_slope() {
    let (mut s, floor) = slab(json!({"slope": 10, "arrow": [[0, 0], [5000, 0]]}));
    let finish = floor_part(&mut s, floor, 0);
    let plan = 5000.0 * 4000.0 - 1000.0 * 1000.0;
    let cos = 10f64.to_radians().cos();
    let (tris, area, volume) = measured(&s.model, finish);
    assert!(near(area, plan / cos), "{area}");
    // 20 mm across its slope, so more than that plumb.
    assert!(near(volume, plan * 20.0 / cos), "{volume}");
    let (lo, hi) = extent(&tris);
    assert!(near(hi[2], 5000.0 * 10f64.to_radians().tan()) && lo[2] < 0.0, "{lo:?} {hi:?}");
}

#[test]
fn over_a_variable_layer_a_shaped_floors_parts_follow_its_top_and_its_level_underside() {
    let mut s = Session::new();
    run(
        &mut s,
        "type",
        json!({"category": "floor", "name": "Terrace", "layers": [
            {"function": "finish", "thickness": 10},
            {"function": "substrate", "thickness": 60, "variable": true},
            {"function": "structure", "thickness": 200},
        ]}),
    );
    let floor =
        run(&mut s, "floor", json!({"boundary": [[0, 0], [4000, 0], [4000, 4000], [0, 4000]], "type": "Terrace"}))["id"].as_u64().unwrap();
    run(&mut s, "edit_shape", json!({"id": floor, "points": [{"at": [2000, 2000], "z": -30}]}));
    let [finish, screed, slab] = [0, 1, 2].map(|k| floor_part(&mut s, floor, k));
    let plan = 4000.0 * 4000.0;
    // A funnel 30 mm deep at its middle: a pyramid's worth of screed less, and all of the slab under it.
    let (_, _, v) = measured(&s.model, slab);
    assert!(near(v, plan * 200.0), "{v}");
    let (tris, area, v) = measured(&s.model, screed);
    assert!(near(v, plan * 60.0 - plan * 30.0 / 3.0), "{v}");
    assert!(area > plan && area < plan * 1.001, "its top, folded: {area}");
    assert!(near(extent(&tris).0[2], -270.0 + 200.0));
    // The finish follows the top, 10 mm plumb all over.
    let (tris, area, v) = measured(&s.model, finish);
    assert!(near(v, plan * 10.0) && area > plan, "{v}, {area}");
    assert!(near(extent(&tris).0[2], -40.0) && near(extent(&tris).1[2], 0.0));
    let Some(Element::Floor(f)) = s.model.elements.get(floor) else { panic!() };
    let all: f64 = [finish, screed, slab].iter().map(|id| measured(&s.model, *id).2).sum();
    assert!(near(all, s.model.floor_volume(f)), "{all}");
}

#[test]
fn a_roofs_a_ceilings_and_a_components_part_have_their_solids_too() {
    let mut s = Session::new();
    s.script(
        r#"wall {"points": [[0, 0], [8000, 0], [8000, 6000], [0, 6000]], "closed": true}
           type {"category": "roof", "name": "Tiled", "layers": [{"function": "finish", "thickness": 50}, {"function": "structure", "thickness": 150}]}
           roof {"type": "Tiled", "slope": 30}
           type {"category": "ceiling", "name": "Lath and Plaster", "layers": [{"function": "structure", "thickness": 25}, {"function": "finish", "thickness": 15}]}
           ceiling {"boundary": [[100, 100], [7900, 100], [7900, 5900], [100, 5900]], "type": "Lath and Plaster"}
           component {"family": "WC", "at": [2000, 2000]}"#,
    )
    .unwrap();
    let find = |s: &Session, cat: &str| *s.model.elements.iter().find(|(_, e)| e.category() == cat).unwrap().0;
    let (roof, ceiling, wc) = (find(&s, "roof"), find(&s, "ceiling"), find(&s, "component"));
    // A hipped roof's tiles and its structure: together all of it, each its share, its top the four slopes.
    let layer = |m: &Model, cat: &str, name: &str, k: usize| m.type_layers(cat, name)[k].guid;
    let tiles = put(&mut s, |m| BuildingPart::new(m.guid(roof), Source::Layer(layer(m, "roof", "Tiled", 0))));
    let boards = put(&mut s, |m| BuildingPart::new(m.guid(roof), Source::Layer(layer(m, "roof", "Tiled", 1))));
    let all = put(&mut s, |m| BuildingPart::new(m.guid(roof), Source::Whole));
    let (_, top, v_tiles) = measured(&s.model, tiles);
    let (_, _, v_boards) = measured(&s.model, boards);
    let (_, top_all, v_all) = measured(&s.model, all);
    assert!(near(v_tiles + v_boards, v_all) && near(v_boards, 3.0 * v_tiles), "{v_tiles} + {v_boards} of {v_all}");
    let Some(Element::Roof(r)) = s.model.elements.get(roof) else { panic!() };
    let plan = crate::geom::area(&s.model.roof_outline(r)).abs();
    assert!(near(top, plan / 30f64.to_radians().cos()) && near(top_all, top), "{top} over {plan}");
    // A ceiling's plaster: its lower 15 mm, its top its plan.
    let plaster = put(&mut s, |m| BuildingPart::new(m.guid(ceiling), Source::Layer(layer(m, "ceiling", "Lath and Plaster", 1))));
    let (tris, area, v) = measured(&s.model, plaster);
    assert!(near(area, 7800.0 * 5800.0) && near(v, 7800.0 * 5800.0 * 15.0));
    assert!(near(extent(&tris).0[2], 2600.0) && near(extent(&tris).1[2], 2615.0));
    // A component is all of itself: its family's forms where it stands, a volume and no face to name.
    let fixture = put(&mut s, |m| BuildingPart::new(m.guid(wc), Source::Whole));
    let p = part(&s.model, fixture);
    let tris = s.model.part_solid(p).unwrap();
    let (lo, hi) = extent(&tris);
    assert!(lo[0] < 2000.0 && hi[0] > 2000.0 && lo[2] >= -1e-6 && hi[2] > 100.0, "{lo:?} {hi:?}");
    let (area, volume) = s.model.part_quantities(p).unwrap();
    assert!(area.is_none() && volume > 1e6, "{volume}");
    assert_eq!((s.model.part_thickness(p), s.model.part_prisms(p).unwrap().is_none()), (None, true));
    assert!(s.model.part_prisms(part(&s.model, tiles)).unwrap().is_some_and(|prisms| !prisms.is_empty()));
}

#[test]
fn a_part_whose_recipe_no_longer_applies_is_in_conflict_and_says_what_to_fix() {
    let (mut s, wall) = historic();
    let [outer, core, inner] = three(&mut s, wall);
    let patch = put(&mut s, |m| BuildingPart { parent: Some(m.guid(inner)), ..layer_part(m, wall, 2) });
    assert!([outer, core, inner, patch].iter().all(|id| s.model.part_state(*id).unwrap().conflict.is_none()));
    // The wall given another type: its parts are layers of the type it was of.
    run(&mut s, "modify", json!({"id": wall, "type": "Generic - 200mm"}));
    assert_eq!(s.model.check(), Ok(()), "a conflict is the part's to report, never the model's to refuse");
    let state = s.model.part_state(core).unwrap();
    let said = state.conflict.clone().unwrap();
    assert_eq!(
        said,
        format!(
            "it is layer 2 of wall type Historic - 500mm, and wall {wall} is of wall type `Generic - 200mm` now, which has no such \
             layer: delete it and make the wall's parts again with `create_parts`, or give the wall its type back"
        )
    );
    assert_eq!((state.layer, state.thickness, state.net_area, state.net_volume), (None, None, None, 0.0));
    assert_eq!(s.model.part_solid(part(&s.model, core)), Err(said.clone()));
    assert_eq!(s.model.part_conflict(part(&s.model, core)), Some(said));
    // What was divided from a part in conflict is in conflict with it.
    let said = s.model.part_conflict(part(&s.model, patch)).unwrap();
    assert!(said.starts_with(&format!("part {inner}, which it was divided from, is in conflict: it is layer 3 of wall type")), "{said}");
    // A part of all of the wall has no layer to lose.
    let whole = put(&mut s, |m| BuildingPart::new(m.guid(wall), Source::Whole));
    assert!(near(measured(&s.model, whole).2, 4000.0 * 3000.0 * 200.0));
    // Its type given back, they are what they were.
    run(&mut s, "modify", json!({"id": wall, "type": "Historic - 500mm"}));
    assert!(near(measured(&s.model, core).2, 4000.0 * 3000.0 * 460.0));
    assert_eq!(s.model.part_conflict(part(&s.model, patch)), None);
    // The type losing its last layer: the part of it is in conflict, the others follow the two that stay.
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 500mm", "layers": [
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 480},
        ]}),
    );
    assert!(near(measured(&s.model, core).2, 4000.0 * 3000.0 * 480.0));
    let said = s.model.part_conflict(part(&s.model, inner)).unwrap();
    assert_eq!(
        said,
        format!(
            "the layer it was made of is gone from wall type `Historic - 500mm`, which wall {wall} is of: delete it and make the \
             wall's parts again with `create_parts`"
        )
    );
    // A layer as thin as nothing (a membrane) has nothing to it.
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 500mm", "layers": [
            {"function": "membrane", "thickness": 0},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 500},
        ]}),
    );
    let said = s.model.part_conflict(part(&s.model, outer)).unwrap();
    assert!(said.starts_with("layer 1 of its type has no thickness, so there is nothing of it"), "{said}");
    assert_eq!(s.model.check(), Ok(()));
}

#[test]
fn a_listing_shows_what_a_part_is_now_beside_what_it_holds() {
    let (mut s, wall) = historic();
    let heritage = Heritage {
        su_code: "USR 3".into(),
        historical_period: "18th century".into(),
        decay_pathology: Some(Decay::BiologicalColonization),
        decay_severity: Some(2),
        intervention_type: "Cleaning".into(),
    };
    let id = put(&mut s, |m| BuildingPart { heritage, ..layer_part(m, wall, 0) });
    let core = put(&mut s, |m| layer_part(m, wall, 1));
    let listed = run(&mut s, "elements", json!({"category": "part"}));
    let one = &listed["elements"][0];
    assert_eq!(one["id"], id);
    assert_eq!((&one["host_category"], &one["layer"], &one["function"]), (&json!("wall"), &json!(1), &json!("finish")));
    assert_eq!((&one["material"], &one["own_material"], &one["thickness"]), (&json!("Lime Plaster"), &json!(false), &json!(20.0)));
    assert_eq!((&one["area_m2"], &one["volume_m3"]), (&json!(12.0), &json!(0.24)));
    assert_eq!(
        [&one["SU_Code"], &one["Historical_Period"], &one["Decay_Pathology"], &one["Decay_Severity"], &one["Intervention_Type"]],
        [&json!("USR 3"), &json!("18th century"), &json!("Biological colonization"), &json!(2), &json!("Cleaning")]
    );
    assert!(one.get("conflict").is_none());
    // What it holds is there as it is held, for `modify` to change: its heritage set, by its fields.
    assert_eq!(one["heritage"]["decay_pathology"], "biological-colonization");
    let other = &listed["elements"][1];
    assert_eq!((&other["id"], &other["volume_m3"]), (&json!(core), &json!(5.52)));
    assert!(other.get("SU_Code").is_none() && other.get("Decay_Severity").is_none(), "nothing said, nothing shown");
    // `modify` changes a part as any element: its own material, which the listing then shows as what it is made of.
    run(&mut s, "modify", json!({"id": core, "material": "Lime Plaster", "heritage": {"su_code": "USM 1", "decay_severity": 5}}));
    let one = shown(&mut s, core);
    assert_eq!(
        (&one["material"], &one["own_material"], &one["SU_Code"], &one["Decay_Severity"]),
        (&json!("Lime Plaster"), &json!(true), &json!("USM 1"), &json!(5))
    );
    let e = fails(&mut s, "modify", json!({"id": core, "heritage": {"decay_severity": 6}}));
    assert!(e.contains("Decay_Severity is 1 (slight) to 5 (the worst), not 6"), "{e}");
    // In conflict, it says so, and measures nothing.
    run(&mut s, "modify", json!({"id": wall, "type": "Generic - 200mm"}));
    let one = shown(&mut s, id);
    assert!(one["conflict"].as_str().unwrap().contains("which has no such layer"), "{one}");
    assert!(one.get("area_m2").is_none() && one["volume_m3"] == 0.0 && one.get("layer").is_none(), "{one}");
}

// ---------- a part's values by name ----------

use crate::schedule::{self, Filter, Kind, Op};

/// The historic wall and a plain one beside it, each taken apart, and the survey's word on the first one's parts: the
/// session, the historic wall's three parts, exterior plaster first, and the plain wall's one.
fn surveyed() -> (Session, Id, [Id; 3], Id) {
    let (mut s, wall) = historic();
    let plain = run(&mut s, "wall", json!({"points": [[0, 5000], [4000, 5000]], "type": "Generic - 200mm"}))["ids"][0].as_u64().unwrap();
    let ids = three(&mut s, wall);
    let whole = put(&mut s, |m| BuildingPart::new(m.guid(plain), Source::Whole));
    let say = |s: &mut Session, id: Id, heritage: Heritage| {
        run(s, "modify", json!({"id": id, "heritage": heritage}));
    };
    say(
        &mut s,
        ids[0],
        Heritage {
            su_code: "USR 3".into(),
            historical_period: "18th century".into(),
            decay_pathology: Some(Decay::Efflorescence),
            decay_severity: Some(4),
            intervention_type: "Cleaning".into(),
        },
    );
    say(
        &mut s,
        ids[1],
        Heritage { su_code: "USM 1".into(), historical_period: "Late medieval".into(), decay_severity: Some(2), ..Heritage::default() },
    );
    say(&mut s, ids[2], Heritage { decay_pathology: Some(Decay::Custom("Lacuna in the fresco".into())), ..Heritage::default() });
    (s, wall, ids, whole)
}

#[test]
fn a_parts_values_are_fields_read_by_name_as_any_elements() {
    let (s, wall, ids, whole) = surveyed();
    let m = &s.model;
    let fields = schedule::fields(m, "part");
    let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "Id",
            "Host",
            "Host Category",
            "Host Type",
            "Level",
            "Layer",
            "Function",
            "Material",
            "Thickness",
            "Net Area",
            "Net Volume",
            "Predefined Type",
            "Object Type",
            "Excluded",
            "SU_Code",
            "Historical_Period",
            "Decay_Pathology",
            "Decay_Severity",
            "Intervention_Type",
            "Conflict",
            "Count",
            "Phase Created",
            "Phase Demolished",
            "Design Option",
        ]
    );
    let kind = |name: &str| fields.iter().find(|(n, _)| n == name).unwrap().1;
    assert_eq!(
        [kind("SU_Code"), kind("Decay_Severity"), kind("Net Area"), kind("Net Volume"), kind("Thickness"), kind("Excluded")],
        [Kind::Text, Kind::Number, Kind::Area, Kind::Number, Kind::Length, Kind::YesNo]
    );
    // The heritage set's names are the ones it is shown by everywhere.
    assert!(Heritage::NAMES.iter().all(|n| names.contains(n)));
    // Each value as a schedule would show it; none where the part has none.
    let text = |id: Id, name: &str| schedule::field_text(m, id, name);
    let of = |id: Id, names: &[&str]| -> Vec<Option<String>> { names.iter().map(|n| text(id, n)).collect() };
    let some = |values: &[&str]| -> Vec<Option<String>> { values.iter().map(|v| Some(v.to_string())).collect() };
    assert_eq!(
        of(ids[0], &["SU_Code", "Historical_Period", "Decay_Pathology", "Decay_Severity", "Intervention_Type"]),
        some(&["USR 3", "18th century", "Efflorescence", "4", "Cleaning"])
    );
    assert_eq!(
        of(ids[0], &["Host", "Host Category", "Host Type", "Level", "Layer", "Function", "Material"]),
        some(&[&wall.to_string(), "wall", "Historic - 500mm", "Level 1", "1", "finish", "Lime Plaster"])
    );
    assert_eq!(
        of(ids[0], &["Thickness", "Net Area", "Net Volume", "Predefined Type", "Excluded"]),
        some(&["20", "12.00", "0.24", "notdefined", "No"])
    );
    assert_eq!(of(ids[0], &["Object Type", "Conflict", "Phase Demolished"]), [None, None, None]);
    assert_eq!(text(ids[2], "Decay_Pathology").as_deref(), Some("Lacuna in the fresco"));
    assert_eq!(of(ids[2], &["SU_Code", "Decay_Severity", "Intervention_Type"]), [None, None, None], "nobody said: none, never 0");
    // A part of all of a plain wall: no layer, no function, no material its type names.
    assert_eq!(
        of(whole, &["Layer", "Function", "Material", "Thickness", "Net Volume"]),
        [None, None, None, Some("200".into()), Some("2.4".into())]
    );
    assert_eq!(text(wall, "SU_Code"), None, "a wall has no such field");
    // A name a part has of its own is no name for a project parameter of parts.
    let mut t = s.clone();
    let e = fails(&mut t, "parameter", json!({"name": "SU_Code", "categories": ["part"]}));
    assert!(e.contains("parts have a `SU_Code` of their own; name it otherwise"), "{e}");
    // There is no schedule of parts yet: only their values by name.
    assert!(!schedule::categories().contains(&"part") && !schedule::built().contains(&"part"));
}

#[test]
fn rules_pick_parts_by_what_the_survey_says_of_them() {
    let (mut s, wall, ids, whole) = surveyed();
    let parts = ["part".to_string()];
    let rule = |field: &str, op: Op, value: Value| Filter { field: field.into(), op, value };
    let picked = |m: &Model, rules: &[Filter]| -> Vec<Id> { schedule::picks(m, &parts, rules).into_iter().collect() };
    let m = &s.model;
    assert_eq!(picked(m, &[]), [ids[0], ids[1], ids[2], whole]);
    assert_eq!(picked(m, &[rule("Decay_Pathology", Op::Equals, json!("efflorescence"))]), [ids[0]]);
    assert_eq!(picked(m, &[rule("Decay_Pathology", Op::HasValue, Value::Null)]), [ids[0], ids[2]]);
    assert_eq!(picked(m, &[rule("Decay_Severity", Op::GreaterOrEqual, json!(3))]), [ids[0]]);
    assert_eq!(picked(m, &[rule("Decay_Severity", Op::HasNoValue, Value::Null)]), [ids[2], whole]);
    assert_eq!(picked(m, &[rule("Historical_Period", Op::Contains, json!("medieval"))]), [ids[1]]);
    assert_eq!(picked(m, &[rule("SU_Code", Op::BeginsWith, json!("US")), rule("Net Area", Op::Greater, json!(10))]), [ids[0], ids[1]]);
    assert_eq!(picked(m, &[rule("Host", Op::Equals, json!(wall)), rule("Function", Op::Equals, json!("structure"))]), [ids[1]]);
    assert_eq!(picked(m, &[rule("Thickness", Op::Less, json!(100))]), [ids[0], ids[2]]);
    assert_eq!(picked(m, &[rule("Conflict", Op::HasValue, Value::Null)]), Vec::<Id>::new());
    // Sound rules name a part's fields and compare what they hold.
    assert_eq!(schedule::check_rules(m, &parts, &[rule("Decay_Severity", Op::Greater, json!(2))]), Ok(()));
    let e = schedule::check_rules(m, &parts, &[rule("Decay", Op::Equals, json!("crack"))]).unwrap_err();
    assert!(e.contains("`Decay` isn't a field of parts; there are: Id, Host, Host Category"), "{e}");
    let e = schedule::check_rules(m, &parts, &[rule("Decay_Severity", Op::Contains, json!("4"))]).unwrap_err();
    assert!(e.contains("it holds a number, and only text contains, begins or ends"), "{e}");
    // The wall given another type: its parts are in conflict, and a rule finds them by it.
    run(&mut s, "modify", json!({"id": wall, "type": "Generic - 200mm"}));
    assert_eq!(picked(&s.model, &[rule("Conflict", Op::HasValue, Value::Null)]), ids);
    assert_eq!(picked(&s.model, &[rule("Net Area", Op::HasValue, Value::Null)]), [whole]);
    assert!(schedule::field_text(&s.model, ids[0], "Conflict").is_some_and(|c| c.contains("which has no such layer")));
}

#[test]
fn filter_on_a_selection_groups_parts_by_their_values_and_a_part_reads_its_hosts_phase() {
    let (mut s, wall, ids, whole) = surveyed();
    let all = json!([ids[0], ids[1], ids[2], whole]);
    let by = run(&mut s, "filter_selection", json!({"ids": all, "by": "Historical_Period"}));
    assert_eq!(
        by["groups"],
        json!([
            {"value": "18th century", "count": 1, "ids": [ids[0]]},
            {"value": "Late medieval", "count": 1, "ids": [ids[1]]},
            {"value": "", "count": 2, "ids": [ids[2], whole]},
        ])
    );
    assert!(by["fields"].as_array().unwrap().contains(&json!("Decay_Pathology")));
    let kept = run(&mut s, "filter_selection", json!({"ids": all, "by": "Material", "keep": ["Lime Plaster"]}));
    assert_eq!(kept["ids"], json!([ids[0], ids[2]]));
    // With their wall, what they share with it: its level, a thickness, and its phases.
    let mixed = run(&mut s, "filter_selection", json!({"ids": [wall, ids[0]]}));
    assert_eq!(mixed["fields"], json!(["Category", "Id", "Level", "Thickness", "Phase Created", "Phase Demolished", "Design Option"]));
    // A part with no phase of its own reads its host's, as a door its wall's; its own, once it has one.
    run(&mut s, "modify", json!({"id": wall, "params": {"Phase Demolished": "New Construction"}}));
    assert_eq!(schedule::field_text(&s.model, ids[0], "Phase Demolished").as_deref(), Some("New Construction"));
    assert_eq!(schedule::field_text(&s.model, whole, "Phase Demolished"), None);
    assert_eq!(schedule::field_text(&s.model, ids[0], "Phase Created").as_deref(), Some("Existing"));
    // A project parameter for parts is a field of theirs too.
    run(&mut s, "parameter", json!({"name": "Surveyed By", "categories": ["parts"]}));
    run(&mut s, "modify", json!({"id": ids[1], "params": {"Surveyed By": "A. Rossi"}}));
    assert_eq!(schedule::field_text(&s.model, ids[1], "Surveyed By").as_deref(), Some("A. Rossi"));
    let picked =
        schedule::picks(&s.model, &["part".to_string()], &[Filter { field: "Surveyed By".into(), op: Op::HasValue, value: Value::Null }]);
    assert_eq!(picked.into_iter().collect::<Vec<_>>(), [ids[1]]);
}
