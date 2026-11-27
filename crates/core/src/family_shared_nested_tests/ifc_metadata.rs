use super::*;
use crate::ifc::{Origin, Schema};
use crate::step::{Entity, File, V};

fn related(file: &File, child: u64, class: &str) -> Vec<u64> {
    file.entities
        .values()
        .filter_map(|e| {
            if e.name != class {
                return None;
            }
            let (V::List(children), V::Ref(to)) = (&e.args[4], &e.args[5]) else { return None };
            children.contains(&V::Ref(child)).then_some(*to)
        })
        .collect()
}

fn properties(file: &File, sets: &[u64]) -> std::collections::BTreeMap<String, V> {
    let mut props = std::collections::BTreeMap::new();
    for set in sets {
        assert_eq!(file.entities[set].name, "IFCPROPERTYSET");
        let V::List(items) = &file.entities[set].args[4] else { panic!("a set's properties") };
        for item in items {
            let V::Ref(id) = item else { panic!("a property reference") };
            let Entity { name, args } = &file.entities[id];
            if name == "IFCPROPERTYSINGLEVALUE"
                && let V::Str(key) = &args[0]
            {
                props.insert(key.clone(), args[2].clone());
            }
        }
    }
    props
}

fn occurrence(file: &File, m: &Model, id: Id) -> u64 {
    let global = V::Str(crate::ifc::compress(m.guid(id).as_u128()));
    *file.entities.iter().find(|(_, e)| e.args.first() == Some(&global)).unwrap().0
}

fn length(n: f64) -> V {
    V::Typed("IFCLENGTHMEASURE".into(), Box::new(V::Real(n)))
}

#[test]
fn nested_ifc_metadata_follows_the_bound_values_and_shares_one_type() {
    let mut definition = handle(true);
    definition["omniclass"] = json!({"number":"23.40.20","title":"Hardware"});
    let (mut s, host) = cabinet_with(definition);
    run(
        &mut s,
        "family",
        json!({"name":"Cabinet","nested_instance":{
        "family":"Handle","type":"Lever","at":[200,50],"params":{"W":"Width / 5"}}}),
    );
    let mut children = nested(&s.model);
    children.sort_by_key(|(_, n)| n.path.clone());
    assert_eq!(children.len(), 2);
    let first = s.model.families["Cabinet"].nested_instances[0].guid;
    for schema in [Schema::Ifc4, Schema::Ifc2x3, Schema::Ifc4x3] {
        let exchange = crate::ifc::export_as(&s.model, Origin::Internal, schema);
        let file = crate::step::parse(exchange.as_bytes()).unwrap();
        let imported = crate::import::ifc(exchange.as_bytes(), "Nested handles").unwrap().model;
        imported.check().unwrap();
        let host = occurrence(&file, &s.model, host);
        let mut types = Vec::new();
        for (id, n) in &children {
            let part = occurrence(&file, &s.model, *id);
            let assigned = related(&file, part, "IFCRELDEFINESBYTYPE");
            assert_eq!(assigned.len(), 1, "{schema:?}: one type for Handle:Lever");
            let typ = &file.entities[&assigned[0]];
            assert_eq!(typ.name, "IFCBUILDINGELEMENTPROXYTYPE");
            assert_eq!(typ.args[2], V::Str("Handle:Lever".into()));
            types.push(assigned[0]);
            let own = properties(&file, &related(&file, part, "IFCRELDEFINESBYPROPERTIES"));
            let width = if n.path[0] == first { 60. } else { 120. };
            assert_eq!(own.get("W"), Some(&length(width)));
            let imported_child = imported.elements.id(s.model.guid(*id)).unwrap();
            assert_eq!(imported.elements.get(imported_child).unwrap().params().unwrap()["W"], json!(width));
            let classes = related(&file, assigned[0], "IFCRELASSOCIATESCLASSIFICATION");
            assert_eq!(classes.len(), 1);
            assert_eq!(file.entities[&classes[0]].args[1], V::Str("23.40.20".into()));
            let V::List(sets) = &typ.args[5] else { panic!("the type's declared parameters") };
            let refs = sets.iter().filter_map(|p| if let V::Ref(id) = p { Some(*id) } else { None }).collect::<Vec<_>>();
            assert_eq!(properties(&file, &refs).get("W"), Some(&length(40.)));
            assert!(file.entities.values().any(|r| r.name == "IFCRELAGGREGATES"
                && r.args[4] == V::Ref(host)
                && matches!(&r.args[5], V::List(items) if items.contains(&V::Ref(part)))));
        }
        assert_eq!(types[0], types[1]);
        assert!(!properties(&file, &related(&file, host, "IFCRELDEFINESBYPROPERTIES")).contains_key("W"));
    }
}

#[test]
fn nested_ifc_metadata_keeps_distinct_embedded_types_and_reuses_the_matching_project_type() {
    let (mut s, cabinet) = cabinet(true);
    let h = s.model.families["Cabinet"].nested[0].family.clone();
    s.model_mut().families.insert("Handle".into(), h);
    let direct = run(&mut s, "component", json!({"family":"Handle","type":"Lever","at":[7000,0]}))["id"].as_u64().unwrap();
    let other = run(&mut s, "component", json!({"family":"Cabinet","type":"Small","at":[9000,0]}))["id"].as_u64().unwrap();

    // An embedded definition in an older file can differ from the library under the same family/type name.
    let m = s.model_mut();
    let mut alternate = m.families["Cabinet"].clone();
    alternate.nested[0].family.parameters[0].value = json!(90);
    m.families.insert("Other cabinet".into(), alternate);
    let Some(Element::Component(cp)) = m.elements.get_mut(other) else { panic!("component") };
    cp.family = "Other cabinet".into();
    m.sync_nested();
    let file = crate::step::parse(crate::ifc::export(m).as_bytes()).unwrap();
    let type_of = |id| {
        let ids = related(&file, occurrence(&file, m, id), "IFCRELDEFINESBYTYPE");
        assert_eq!(ids.len(), 1);
        ids[0]
    };
    let nested_type = |id| type_of(m.nested_in(m.guid(id))[0].0);
    let first = nested_type(cabinet);
    let second = nested_type(other);
    assert_eq!(first, type_of(direct));
    assert_ne!(file.entities[&first].args[0], file.entities[&second].args[0]);
    for (t, w) in [(first, 40.), (second, 90.)] {
        let typ = &file.entities[&t];
        assert_eq!(typ.args[2], V::Str("Handle:Lever".into()));
        let V::List(sets) = &typ.args[5] else { panic!("declared values of this definition") };
        let V::Ref(set) = sets[0] else { panic!("type property set") };
        assert_eq!(properties(&file, &[set]).get("W"), Some(&length(w)));
    }
}

#[test]
fn nested_ifc_metadata_preserves_a_parameterless_childs_class_and_uses_only_available_type_classes() {
    let (mut s, host) = cabinet(true);
    let m = s.model_mut();
    let f = m.families.get_mut("Cabinet").unwrap();
    f.nested_instances[0].params.clear();
    let child = &mut f.nested[0].family;
    child.parameters.clear();
    child.forms = serde_json::from_value(json!([{"profile":[[0,0],[50,0],[50,20],[0,20]],"to":30}])).unwrap();
    child.ifc = Some(serde_json::from_value(json!({"class":"IFCBUILDINGELEMENTPART"})).unwrap());
    let id = m.nested_in(m.guid(host))[0].0;
    for schema in [Schema::Ifc4, Schema::Ifc2x3, Schema::Ifc4x3] {
        let file = crate::step::parse(crate::ifc::export_as(m, Origin::Internal, schema).as_bytes()).unwrap();
        let occ = occurrence(&file, m, id);
        assert_eq!(file.entities[&occ].name, "IFCBUILDINGELEMENTPART");
        let types = related(&file, occ, "IFCRELDEFINESBYTYPE");
        if schema == Schema::Ifc2x3 {
            assert!(types.is_empty(), "IFC2X3 has no IfcBuildingElementPartType");
        } else {
            assert_eq!(types.len(), 1);
            assert_eq!(file.entities[&types[0]].name, "IFCBUILDINGELEMENTPARTTYPE");
        }
        let own = properties(&file, &related(&file, occ, "IFCRELDEFINESBYPROPERTIES"));
        assert_eq!(own.get("Family"), Some(&V::Typed("IFCTEXT".into(), Box::new(V::Str("Handle".into())))));
        assert_eq!(own.get("Type"), Some(&V::Typed("IFCTEXT".into(), Box::new(V::Str("Lever".into())))));
    }
}

#[test]
fn nested_ifc_metadata_keeps_the_family_name_boundary_in_equal_display_labels() {
    let (mut s, host) = cabinet(true);
    let m = s.model_mut();
    let cabinet = m.families.get_mut("Cabinet").unwrap();
    let mut left = cabinet.nested[0].clone();
    left.name = "Handle:Fixture".into();
    let mut right = cabinet.nested[0].clone();
    right.family.types = [("Fixture:Lever".into(), Default::default())].into();
    right.family.parameters[0].value = json!(90);
    let mut instance = cabinet.nested_instances[0].clone();
    instance.guid = crate::ids::Guid::new();
    instance.type_name = "Fixture:Lever".into();
    cabinet.nested_instances[0].family = left.name.clone();
    cabinet.nested = vec![left.clone()];
    m.families.insert(left.name.clone(), left.family);
    let read = |m: &Model| {
        let file = crate::step::parse(crate::ifc::export(m).as_bytes()).unwrap();
        let mut ids = std::collections::BTreeMap::new();
        for (id, _) in m.nested_in(m.guid(host)) {
            let Some(Element::Nested(n)) = m.elements.get(id) else { panic!("child") };
            let part = occurrence(&file, m, id);
            let assigned = related(&file, part, "IFCRELDEFINESBYTYPE");
            assert_eq!(assigned.len(), 1);
            let typ = &file.entities[&assigned[0]];
            assert_eq!(typ.args[2], V::Str("Handle:Fixture:Lever".into()));
            let rel = file.entities.values().find(|e| e.name == "IFCRELDEFINESBYTYPE" && e.args[5] == V::Ref(assigned[0])).unwrap();
            ids.insert(n.family.clone(), (typ.args[0].clone(), rel.args[0].clone()));
        }
        ids
    };
    m.sync_nested();
    let alone = read(m);
    let cabinet = m.families.get_mut("Cabinet").unwrap();
    cabinet.nested_instances.push(instance);
    cabinet.nested.push(right.clone());
    m.families.insert(right.name.clone(), right.family);
    m.sync_nested();
    let both = read(m);
    assert_eq!(both.len(), 2);
    assert_eq!(alone[&left.name], both[&left.name], "adding a colliding label must preserve both original GlobalIds");
    assert_ne!(both[&left.name].0, both[&right.name].0);
    assert_ne!(both[&left.name].1, both[&right.name].1);
}
