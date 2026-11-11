//! Schedules of parts in the desktop: Parts in the New Schedule dialog's categories, the ready-made Restoration
//! Quantities in the Schedules list, and what a schedule says of parts in conflict, driven as a user drives them.

use super::*;

/// The box, its south wall taken apart into parts, and what is to be done to the first of them.
const SURVEYED: &str = r#"type {"category": "wall", "name": "Historic", "layers": [{"function": "finish", "thickness": 20}, {"function": "structure", "thickness": 460}]}
wall {"points": [[0,-3000],[4000,-3000]], "type": "Historic", "height": 3000}
create_parts {"id": 100006}
part {"id": 100007, "intervention_type": "Consolidation", "decay_pathology": "detachment"}"#;

fn schedule_of(h: &Harness<BimApp>, id: Id) -> bimcraft_core::model::Schedule {
    match h.state().session.model.elements.get(id) {
        Some(Element::Schedule(s)) => s.clone(),
        _ => panic!("schedule {id} is gone"),
    }
}

#[test]
fn a_schedule_of_parts_is_made_from_the_new_schedule_dialog_and_totals_its_net_volume() {
    let mut h = app_at(&format!("{BOX}\n{SURVEYED}"), 0.05);
    assert_eq!(ids(&h, "part").len(), 2, "the wall's plaster and its masonry");
    press(&mut h, "View");
    press(&mut h, "Schedules");
    press(&mut h, "Schedule/Quantities…");
    // The list scrolls: its entry is there for a screen reader if not under the pointer.
    h.get_by_role_and_label(Role::Button, "Parts").click_accesskit();
    h.run();
    press(&mut h, "OK");
    let sc = ids(&h, "schedule")[0];
    assert_eq!(h.state().sheet, Sheet::Table(sc));
    assert_eq!((schedule_of(&h, sc).of.as_str(), schedule_of(&h, sc).name.as_str()), ("part", "Part Schedule"));
    // Its properties open on its fields: a volume totals, as an area does.
    h.get_by_role_and_label(Role::Button, "Net Volume").click_accesskit();
    h.run();
    pick(&mut h, Role::CheckBox, "Calculate totals");
    let s = schedule_of(&h, sc);
    assert!(s.fields.iter().any(|f| f.name == "Net Volume" && f.total), "{:?}", s.fields);
    // The tab says each quantity's unit, and a volume to the litre: 4 by 3 m of plaster 20 mm thick.
    h.get_by_label("Net Volume (m³)");
    h.get_by_label("Net Area (m²)");
    assert!(h.query_all_by_label("0.240").count() >= 1, "the plaster's volume, and with no other row over a litre short of it");
}

#[test]
fn restoration_quantities_is_made_from_the_schedules_list_and_says_which_parts_are_in_conflict() {
    let mut h = app_at(&format!("{BOX}\n{SURVEYED}"), 0.05);
    press(&mut h, "View");
    press(&mut h, "Schedules");
    press(&mut h, "Restoration Quantities");
    let sc = ids(&h, "schedule")[0];
    assert_eq!(h.state().sheet, Sheet::Table(sc));
    let s = schedule_of(&h, sc);
    assert_eq!((s.of.as_str(), s.name.as_str(), s.grouped), ("part", "Restoration Quantities", true));
    h.get_by_label("Intervention_Type");
    h.get_by_label("Consolidation");
    assert!(h.query_by_label_contains("in conflict").is_none());
    // Made, it is one of the project's schedules: the list offers it once, to open, and no more to make (its tab and
    // the Project Browser name it too).
    let offered = |h: &Harness<BimApp>| {
        use egui_kittest::kittest::NodeT;
        h.query_all_by_label("Restoration Quantities").filter(|n| n.accesskit_node().role() == Role::Button).count()
    };
    let before = offered(&h);
    press(&mut h, "Schedules");
    assert_eq!(offered(&h), before + 1);
    key(&mut h, Key::Escape);
    // The wall given another type, its parts are in conflict: the schedule says so over its rows.
    let wall = 100006;
    h.state_mut().session.execute("modify", &json!({"id": wall, "type": "Generic - 200mm"})).unwrap();
    h.run();
    h.get_by_label_contains("2 of its parts are in conflict");
}
