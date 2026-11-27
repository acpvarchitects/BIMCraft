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
    let (mut s, host) = cabinet(true);
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
        let file = crate::step::parse(crate::ifc::export_as(&s.model, Origin::Internal, schema).as_bytes()).unwrap();
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
            assert_eq!(own.get("W"), Some(&length(if n.path[0] == first { 60. } else { 120. })));
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
