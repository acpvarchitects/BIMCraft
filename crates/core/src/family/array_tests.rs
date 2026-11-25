use super::arrays::{ArrayLayout, ArrayPattern, MoveTo};
use super::{Expr, Val};
use std::collections::BTreeMap;
fn pattern(move_to: MoveTo) -> ArrayPattern {
    ArrayPattern { number: Expr::Number(4.), move_to, layout: ArrayLayout::Linear { displacement: [300., 0., 0.] } }
}
#[test]
fn array_second_keeps_spacing_and_last_keeps_total_span_when_count_changes() {
    let mut second = pattern(MoveTo::Second);
    let mut last = pattern(MoveTo::Last);
    let values = BTreeMap::new();
    assert_eq!(second.placements(&values).unwrap()[3].point([0.; 3]), [900., 0., 0.]);
    assert_eq!(last.placements(&values).unwrap()[3].point([0.; 3]), [300., 0., 0.]);
    second.number = Expr::Number(7.);
    last.number = Expr::Number(7.);
    assert_eq!(second.placements(&values).unwrap()[6].point([0.; 3]), [1800., 0., 0.]);
    assert_eq!(last.placements(&values).unwrap()[6].point([0.; 3]), [300., 0., 0.]);
}
#[test]
fn array_resolves_count_parameters_and_rejects_fractional_unbounded_or_missing_counts() {
    let mut a = pattern(MoveTo::Second);
    a.number = Expr::Formula("Count".into());
    assert_eq!(a.placements(&[("Count".into(), Val::Number(5.))].into()).unwrap().len(), 5);
    assert!(a.placements(&BTreeMap::new()).is_err());
    for n in [0., 1., 2.5, 1001., f64::NAN, f64::INFINITY] {
        a.number = Expr::Number(n);
        assert!(a.placements(&BTreeMap::new()).is_err(), "{n}");
    }
}
#[test]
fn radial_array_uses_axis_and_second_or_last_angle() {
    let mut a = pattern(MoveTo::Last);
    a.layout = ArrayLayout::Radial { centre: [10., 20., 30.], axis: [0., 0., 1.], degrees: 90. };
    let placements = a.placements(&BTreeMap::new()).unwrap();
    let end = placements[3].point([110., 20., 30.]);
    assert!((end[0] - 10.).abs() < 1e-8);
    assert!((end[1] - 120.).abs() < 1e-8);
    assert!((end[2] - 30.).abs() < 1e-8);
    a.layout = ArrayLayout::Radial { centre: [0.; 3], axis: [0.; 3], degrees: 90. };
    assert!(a.placements(&BTreeMap::new()).is_err());
}

#[test]
fn associated_array_rebuilds_real_members_from_edited_seed_with_stable_identities() {
    use super::{Family, arrays::FamilyArray};
    use crate::ids::Guid;
    let seed = Guid::new();
    let array = Guid::new();
    let mut f: Family = serde_json::from_value(
        serde_json::json!({"types":{"A":{}},"forms":[{"guid":seed,"profile":[[0,0],[10,0],[10,10],[0,10]],"to":100}]}),
    )
    .unwrap();
    f.forms[0].array = Some((array, 0));
    f.arrays.push(FamilyArray { guid: array, members: vec![seed], pattern: pattern(MoveTo::Second), ..Default::default() });
    let built = f.rebuild_arrays(&BTreeMap::new()).unwrap();
    assert_eq!(built.forms.len(), 4);
    assert_eq!(built.forms[3].array, Some((array, 3)));
    assert_ne!(built.forms[0].guid, built.forms[1].guid);
    let again = built.rebuild_arrays(&BTreeMap::new()).unwrap();
    assert_eq!(built, again);
    let mut edited = again;
    edited.forms[0].to = Expr::Number(200.);
    edited.arrays[0].pattern.number = Expr::Number(3.);
    let refreshed = edited.rebuild_arrays(&BTreeMap::new()).unwrap();
    assert_eq!(refreshed.forms.len(), 3);
    assert_eq!(refreshed.forms[2].to, Expr::Number(200.));
    assert_eq!(refreshed.forms[1].guid, built.forms[1].guid);
}

#[test]
fn family_array_command_persists_undoes_and_flexes_integer_count_and_seed_formula() {
    use crate::engine::Session;
    use crate::ids::Guid;
    use serde_json::json;
    let seed = Guid::new();
    let array = Guid::new();
    let mut s = Session::new();
    s.execute("family",&json!({"name":"Leg array","types":{"A":{},"B":{"Length":2000}},"parameters":[{"name":"Count","kind":"integer","formula":"if(Length > 1500, 5, 3)"},{"name":"Length","value":1000},{"name":"Width","value":20}],"forms":[{"guid":seed,"array":[array,0],"profile":[[0,0],["Width",0],["Width",10],[0,10]],"to":100}],"arrays":[{"guid":array,"members":[seed],"pattern":{"number":"Count","layout":{"kind":"linear","displacement":[100,0,0]}}}]})).unwrap();
    let f = &s.model.families["Leg array"];
    assert_eq!(f.arrays.len(), 1);
    assert!(f.arrays[0].id > 0);
    let a = f.values("A", &BTreeMap::new()).unwrap();
    let b = f.values("B", &BTreeMap::new()).unwrap();
    assert_eq!(f.flexed(&a).unwrap().forms.len(), 3);
    assert_eq!(f.flexed(&b).unwrap().forms.len(), 5);
    assert_eq!(f.flexed(&a).unwrap().forms.len(), 3);
    let file = s.model.to_json().unwrap();
    let (loaded, left) = crate::model::Model::read(&file).unwrap();
    assert!(left.is_empty());
    loaded.check().unwrap();
    assert_eq!(loaded.families["Leg array"].arrays, f.arrays);
    s.execute("family", &json!({"name":"Leg array","types":{"A":{"Length":1800}}})).unwrap();
    let f = &s.model.families["Leg array"];
    assert_eq!(f.flexed(&f.values("A", &BTreeMap::new()).unwrap()).unwrap().forms.len(), 5);
    s.execute("undo", &json!({})).unwrap();
    let f = &s.model.families["Leg array"];
    assert_eq!(f.flexed(&f.values("A", &BTreeMap::new()).unwrap()).unwrap().forms.len(), 3);
}
#[test]
fn array_of_nested_instances_and_radial_full_circle_have_real_stable_slots() {
    use super::{Family, arrays::FamilyArray};
    use crate::ids::Guid;
    use serde_json::json;
    let seed = Guid::new();
    let array = Guid::new();
    let mut f:Family=serde_json::from_value(json!({"types":{"A":{}},"nested":[{"name":"Leg","family":{"types":{"A":{}},"forms":[{"profile":[[0,0],[10,0],[10,10],[0,10]],"to":100}]}}],"nested_instances":[{"guid":seed,"array":[array,0],"family":"Leg","type":"A","at":[100,0]}]})).unwrap();
    f.arrays.push(FamilyArray {
        guid: array,
        members: vec![seed],
        pattern: ArrayPattern {
            number: Expr::Number(4.),
            move_to: MoveTo::Last,
            layout: ArrayLayout::Radial { centre: [0.; 3], axis: [0., 0., 1.], degrees: 360. },
        },
        ..Default::default()
    });
    let built = f.rebuild_arrays(&BTreeMap::new()).unwrap();
    assert_eq!(built.nested_instances.len(), 4);
    assert_eq!(built.nested_instances[1].array, Some((array, 1)));
    assert_eq!(built.nested_instances[2].at[0], Expr::Number(-100.));
    assert_eq!(built.rebuild_arrays(&BTreeMap::new()).unwrap(), built);
}
