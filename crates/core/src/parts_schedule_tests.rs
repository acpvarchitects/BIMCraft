//! Schedules of parts (`schedule.rs` over `parts.rs`): parts as a category schedules are of, and a schedule of them as
//! the restoration's bill of quantities: net areas and volumes by what is to be done, with their totals, to a workbook.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::model::{Element, Id, Model};

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn fails(s: &mut Session, id: &str, p: Value) -> String {
    let before = s.model.clone();
    let e = s.execute(id, &p).expect_err(&format!("{id} {p} should fail"));
    assert_eq!(*s.model, *before, "a refused {id} changes nothing");
    e
}

fn ids(v: &Value) -> Vec<Id> {
    v["ids"].as_array().unwrap().iter().map(|id| id.as_u64().unwrap()).collect()
}

/// A surveyed corner of a historic building, taken apart: a wall 4 m long and 3 m tall of lime plaster on rubble masonry
/// (20 + 460 mm) and a floor of 4 by 5 m, terracotta on a timber deck (30 + 170 mm).
struct Site {
    wall: Id,
    floor: Id,
    plaster: Id,
    masonry: Id,
    tiles: Id,
    deck: Id,
}

/// The site, and what the survey says of its parts: the plaster and the tiles are to be consolidated, the masonry
/// repointed, and of the deck nothing is said.
fn site() -> (Session, Site) {
    let mut s = Session::new();
    for name in ["Lime Plaster", "Rubble Masonry", "Terracotta", "Timber"] {
        run(&mut s, "material", json!({"name": name}));
    }
    run(
        &mut s,
        "type",
        json!({"category": "wall", "name": "Historic - 480mm", "layers": [
            {"function": "finish", "material": "Lime Plaster", "thickness": 20},
            {"function": "structure", "material": "Rubble Masonry", "thickness": 460},
        ]}),
    );
    run(
        &mut s,
        "type",
        json!({"category": "floor", "name": "Historic Floor", "layers": [
            {"function": "finish", "material": "Terracotta", "thickness": 30},
            {"function": "structure", "material": "Timber", "thickness": 170},
        ]}),
    );
    let wall =
        run(&mut s, "wall", json!({"points": [[0, 0], [4000, 0]], "type": "Historic - 480mm", "height": 3000}))["ids"][0].as_u64().unwrap();
    let floor =
        run(&mut s, "floor", json!({"boundary": [[0, 1000], [4000, 1000], [4000, 6000], [0, 6000]], "type": "Historic Floor"}))["id"]
            .as_u64()
            .unwrap();
    let made = ids(&run(&mut s, "create_parts", json!({"ids": [wall, floor]})));
    let [plaster, masonry, tiles, deck] = [made[0], made[1], made[2], made[3]];
    run(
        &mut s,
        "part",
        json!({"id": plaster, "su_code": "USR 1", "historical_period": "18th century", "decay_pathology": "detachment",
            "decay_severity": 4, "intervention_type": "Consolidation"}),
    );
    run(
        &mut s,
        "part",
        json!({"id": masonry, "su_code": "USM 1", "historical_period": "Late medieval", "decay_pathology": "crack",
            "decay_severity": 2, "intervention_type": "Repointing"}),
    );
    run(
        &mut s,
        "part",
        json!({"id": tiles, "su_code": "USR 2", "decay_pathology": "erosion", "decay_severity": 3, "intervention_type": "Consolidation"}),
    );
    (s, Site { wall, floor, plaster, masonry, tiles, deck })
}

/// What part `id` measures, as the model works it out: its net area, m², and its net volume, m³.
fn measured(m: &Model, id: Id) -> (f64, f64) {
    let Some(Element::Part(p)) = m.elements.get(id) else { panic!("no part {id}") };
    let (area, volume) = m.part_quantities(p).unwrap();
    (area.unwrap() / 1e6, volume / 1e9)
}

/// An id as a schedule's cell holds it: a number.
fn n(id: Id) -> f64 {
    id as f64
}

fn near(a: &Value, b: f64) -> bool {
    a.as_f64().is_some_and(|a| (a - b).abs() < 1e-3)
}

/// A schedule of parts by `by`, a row for each of its values, with the totals a bill of quantities has.
fn bill(by: &[&str]) -> Value {
    let mut fields: Vec<Value> = by.iter().map(|f| json!(f)).collect();
    fields.extend([
        json!({"name": "Net Area", "total": true}),
        json!({"name": "Net Volume", "total": true}),
        json!({"name": "Count", "total": true}),
    ]);
    json!({"name": "Bill", "category": "parts", "fields": fields, "sort": by, "grouped": true})
}

#[test]
fn parts_are_a_category_schedules_are_of_with_a_row_for_each_part() {
    let (mut s, at) = site();
    let t = run(&mut s, "schedule", json!({"name": "Part Schedule", "category": "Parts"}));
    assert_eq!(t["category"], "part");
    assert_eq!(
        t["columns"],
        json!([
            "Id",
            "Host",
            "Host Category",
            "Host Type",
            "Level",
            "Layer",
            "Function",
            "Material",
            "Thickness",
            "Net Area (m²)",
            "Net Volume (m³)"
        ])
    );
    assert_eq!(
        t["rows"],
        json!([
            [n(at.plaster), n(at.wall), "wall", "Historic - 480mm", "Level 1", 1, "finish", "Lime Plaster", 20.0, 12.0, 0.24],
            [n(at.masonry), n(at.wall), "wall", "Historic - 480mm", "Level 1", 2, "structure", "Rubble Masonry", 460.0, 12.0, 5.52],
            [n(at.tiles), n(at.floor), "floor", "Historic Floor", "Level 1", 1, "finish", "Terracotta", 30.0, 20.0, 0.6],
            [n(at.deck), n(at.floor), "floor", "Historic Floor", "Level 1", 2, "structure", "Timber", 170.0, 20.0, 3.4],
        ])
    );
    // Each row lists its part, which is what a click on it selects.
    assert_eq!(t["elements"], json!([[at.plaster], [at.masonry], [at.tiles], [at.deck]]));
    let available: Vec<&str> = t["available"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    for name in crate::parts::Heritage::NAMES.iter().chain(&["Conflict", "Excluded", "Count", "Mass"]) {
        assert!(available.contains(name), "{name} of {available:?}");
    }
    assert!(t.get("totals").is_none() && t.get("notes").is_none(), "{t}");
    // It is the project's, as any schedule made: read again by its name, and kept by a file.
    assert_eq!(run(&mut s, "schedule", json!({"name": "Part Schedule"}))["id"], t["id"]);
    assert!(crate::schedule::categories().contains(&"part"));
    let (again, left_out) = Model::read(&s.model.to_json().unwrap()).unwrap();
    assert_eq!((again.check(), left_out), (Ok(()), Vec::new()));
    let Some(Element::Schedule(kept)) = again.elements.get(t["id"].as_u64().unwrap()) else { panic!("the file kept no schedule") };
    assert_eq!(crate::schedule::table(&again, kept).unwrap().rows.len(), 4);
}

#[test]
fn a_bill_of_quantities_groups_parts_and_totals_their_net_areas_volumes_and_counts() {
    let (mut s, at) = site();
    let t = run(&mut s, "schedule", bill(&["Intervention_Type"]));
    assert_eq!(t["columns"], json!(["Intervention_Type", "Net Area (m²)", "Net Volume (m³)", "Count"]));
    // What nobody said what to do with comes first, under no heading: neither dropped nor counted as nothing.
    assert_eq!(t["rows"], json!([[null, 20.0, 3.4, 1.0], ["Consolidation", 32.0, 0.84, 2.0], ["Repointing", 12.0, 5.52, 1.0]]));
    assert_eq!(t["elements"], json!([[at.deck], [at.plaster, at.tiles], [at.masonry]]));
    assert_eq!(t["totals"], json!([null, 64.0, 9.76, 4.0]));
    // The totals are the parts' own net areas and volumes, as the model measures them.
    let all = [at.plaster, at.masonry, at.tiles, at.deck].map(|id| measured(&s.model, id));
    assert!(near(&t["totals"][1], all.iter().map(|q| q.0).sum()), "{t}");
    assert!(near(&t["totals"][2], all.iter().map(|q| q.1).sum()), "{t}");
    // By two fields, a row for each pair of their values; by what it is made of, or its host, the same way.
    let t = run(&mut s, "schedule", bill(&["Intervention_Type", "Decay_Pathology"]));
    assert_eq!(
        t["rows"],
        json!([
            [null, null, 20.0, 3.4, 1.0],
            ["Consolidation", "Detachment", 12.0, 0.24, 1.0],
            ["Consolidation", "Erosion", 20.0, 0.6, 1.0],
            ["Repointing", "Crack", 12.0, 5.52, 1.0],
        ])
    );
    let t = run(&mut s, "schedule", bill(&["Host Category"]));
    assert_eq!(t["rows"], json!([["floor", 40.0, 4.0, 2.0], ["wall", 24.0, 5.76, 2.0]]));
    let t = run(&mut s, "schedule", bill(&["Historical_Period"]));
    assert_eq!(t["rows"], json!([[null, 40.0, 4.0, 2.0], ["18th century", 12.0, 0.24, 1.0], ["Late medieval", 12.0, 5.52, 1.0]]));
    let t = run(&mut s, "schedule", bill(&["Material"]));
    assert_eq!(t["rows"][0], json!(["Lime Plaster", 12.0, 0.24, 1.0]));
    // Itemized, each part is a row and the totals are the same.
    let mut each = bill(&["Intervention_Type"]);
    each["grouped"] = json!(false);
    let t = run(&mut s, "schedule", each);
    assert_eq!((t["rows"].as_array().map(Vec::len), &t["totals"]), (Some(4), &json!([null, 64.0, 9.76, 4.0])));
}

#[test]
fn a_bill_of_quantities_follows_the_host_as_it_is_stretched_and_cut() {
    let (mut s, at) = site();
    run(&mut s, "schedule", bill(&["Intervention_Type"]));
    // The wall half as long again, and a window through it: nothing of a part is stored, so the bill is what they are now.
    run(&mut s, "modify", json!({"id": at.wall, "end": [6000, 0]}));
    let t = run(&mut s, "schedule", json!({"name": "Bill"}));
    assert_eq!(t["rows"], json!([[null, 20.0, 3.4, 1.0], ["Consolidation", 38.0, 0.96, 2.0], ["Repointing", 18.0, 8.28, 1.0]]));
    run(&mut s, "window", json!({"wall": at.wall, "offset": 3000}));
    let t = run(&mut s, "schedule", json!({"name": "Bill"}));
    let all = [at.plaster, at.masonry, at.tiles, at.deck].map(|id| measured(&s.model, id));
    let [plaster, masonry, tiles, _] = all;
    assert!(masonry.0 < 18.0 && masonry.1 < 8.28, "the window is out of the wall's parts: {masonry:?}");
    assert!(near(&t["rows"][1][1], plaster.0 + tiles.0) && near(&t["rows"][1][2], plaster.1 + tiles.1), "{t}");
    assert!(near(&t["rows"][2][1], masonry.0) && near(&t["rows"][2][2], masonry.1), "{t}");
    assert!(near(&t["totals"][1], all.iter().map(|q| q.0).sum()), "{t}");
    assert!(near(&t["totals"][2], all.iter().map(|q| q.1).sum()), "{t}");
}

#[test]
fn a_schedule_of_parts_leaves_out_the_excluded_and_says_which_are_in_conflict() {
    let (mut s, at) = site();
    run(&mut s, "schedule", bill(&["Intervention_Type"]));
    // Excluded, a part is in no schedule, as Revit leaves it out: the bill is of what is left.
    run(&mut s, "part", json!({"id": at.tiles, "excluded": true}));
    let t = run(&mut s, "schedule", json!({"name": "Bill"}));
    assert_eq!(t["rows"], json!([[null, 20.0, 3.4, 1.0], ["Consolidation", 12.0, 0.24, 1.0], ["Repointing", 12.0, 5.52, 1.0]]));
    assert_eq!(t["totals"], json!([null, 44.0, 9.16, 3.0]));
    // A schedule that filters by Excluded says for itself which it lists: so the excluded can be listed.
    let out = run(
        &mut s,
        "schedule",
        json!({"name": "Left out", "category": "part", "fields": ["Id", "Excluded", "Net Area"], "filters": [{"field": "Excluded", "op": "equals", "value": true}]}),
    );
    assert_eq!(out["rows"], json!([[n(at.tiles), true, 20.0]]));
    run(&mut s, "part", json!({"id": at.tiles, "excluded": false}));
    // The wall given a type of another make: its parts' recipes no longer apply. They stay in the bill, counted, with no
    // area and no volume (never 0), and the schedule says so.
    run(&mut s, "modify", json!({"id": at.wall, "type": "Generic - 200mm"}));
    let t = run(&mut s, "schedule", json!({"name": "Bill"}));
    assert_eq!(t["rows"], json!([[null, 20.0, 3.4, 1.0], ["Consolidation", 20.0, 0.6, 2.0], ["Repointing", null, null, 1.0]]));
    assert_eq!(t["totals"], json!([null, 40.0, 4.0, 4.0]));
    let notes = t["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 1, "{t}");
    let note = notes[0].as_str().unwrap();
    assert!(note.contains("2 of its parts are in conflict") && note.contains(&format!("{}, {}", at.plaster, at.masonry)), "{note}");
    // Listed with why, by their Conflict; or left out by it.
    let listed = run(
        &mut s,
        "schedule",
        json!({"name": "Conflicts", "category": "part", "fields": ["Id", "Conflict", "Net Area", "Net Volume"],
            "filters": [{"field": "Conflict", "op": "has-value"}]}),
    );
    let rows = listed["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows[0][1].as_str().is_some_and(|c| c.contains("which has no such layer")), "{listed}");
    assert_eq!((&rows[0][2], &rows[0][3]), (&Value::Null, &Value::Null), "in conflict it measures nothing, which is no value");
    let sound = run(&mut s, "schedule", json!({"name": "Bill", "filters": [{"field": "Conflict", "op": "has-no-value"}]}));
    assert_eq!(sound["rows"], json!([[null, 20.0, 3.4, 1.0], ["Consolidation", 20.0, 0.6, 1.0]]));
    assert!(sound.get("notes").is_none(), "{sound}");
}

#[test]
fn a_schedule_of_parts_filters_by_what_the_survey_says() {
    let (mut s, at) = site();
    let listed = |s: &mut Session, filters: Value| -> Vec<Id> {
        let t = run(s, "schedule", json!({"category": "parts", "fields": ["Id", "Decay_Severity"], "filters": filters}));
        t["rows"].as_array().unwrap().iter().map(|r| r[0].as_f64().unwrap() as Id).collect()
    };
    assert_eq!(listed(&mut s, json!([{"field": "Decay_Severity", "op": "greater-or-equal", "value": 3}])), [at.plaster, at.tiles]);
    assert_eq!(listed(&mut s, json!([{"field": "Decay_Severity", "op": "equals", "value": 2}])), [at.masonry]);
    assert_eq!(listed(&mut s, json!([{"field": "Decay_Severity", "op": "has-no-value"}])), [at.deck]);
    assert_eq!(listed(&mut s, json!([{"field": "Decay_Severity", "op": "less", "value": 4}])), [at.masonry, at.tiles], "none isn't 0");
    assert_eq!(
        listed(
            &mut s,
            json!([{"field": "Decay_Pathology", "op": "equals", "value": "detachment"}, {"field": "Host Category", "op": "equals", "value": "Wall"}])
        ),
        [at.plaster]
    );
    // The worst first, under what has no severity: no value sorts after every number, so before them going down.
    let t = run(
        &mut s,
        "schedule",
        json!({"category": "parts", "fields": ["Decay_Severity", "Id"], "sort": [{"field": "Decay_Severity", "descending": true}]}),
    );
    assert_eq!(t["rows"], json!([[null, n(at.deck)], [4, n(at.plaster)], [3, n(at.tiles)], [2, n(at.masonry)]]));
    let e =
        fails(&mut s, "schedule", json!({"category": "parts", "filters": [{"field": "Decay_Severity", "op": "contains", "value": "4"}]}));
    assert!(e.contains("it holds a number"), "{e}");
    let e = fails(&mut s, "schedule", json!({"name": "S", "category": "parts", "fields": ["Decay"]}));
    assert!(e.contains("`Decay` isn't a field of parts; there are: Id, Host, Host Category"), "{e}");
}

#[test]
fn a_group_none_of_whose_rows_has_a_number_shows_none_and_a_total_of_nothing_is_none() {
    let (mut s, at) = site();
    // A project parameter of parts, which only the plaster has a value of.
    run(&mut s, "parameter", json!({"name": "Sampled", "kind": "number", "categories": ["part"]}));
    run(&mut s, "modify", json!({"id": at.plaster, "params": {"Sampled": 3}}));
    let t = run(
        &mut s,
        "schedule",
        json!({"name": "Samples", "category": "parts", "fields": ["Host Category", {"name": "Sampled", "total": true}, "Count"],
            "sort": ["Host Category"], "grouped": true}),
    );
    assert_eq!(t["rows"], json!([["floor", null, 2.0], ["wall", 3.0, 2.0]]), "no number in the floor's parts: none, not 0");
    assert_eq!(t["totals"], json!([null, 3.0, null]));
    // The same of any category: a wall schedule's total of a parameter no wall has a value of.
    run(&mut s, "parameter", json!({"name": "Cost", "kind": "number", "categories": ["wall"]}));
    let t = run(&mut s, "schedule", json!({"category": "walls", "fields": ["Id", {"name": "Cost", "total": true}]}));
    assert_eq!(t["totals"], json!([null, null]));
}

#[test]
fn volumes_are_a_kind_of_their_own_shown_to_the_litre_in_the_projects_units() {
    use crate::schedule::{Column, Kind, heading, shown};
    use crate::units::Units;
    let (mut s, at) = site();
    let kind = |name: &str| crate::schedule::fields(&s.model, "part").into_iter().find(|(n, _)| n == name).map(|(_, k)| k);
    assert_eq!((kind("Net Area"), kind("Net Volume")), (Some(Kind::Area), Some(Kind::Volume)));
    let volume = Column { heading: "Net Volume".into(), kind: Kind::Volume };
    let (m, ft) = (Units::Metres, Units::FeetInches);
    assert_eq!((heading(m, &volume), shown(m, Kind::Volume, &json!(0.0154))), ("Net Volume (m³)".into(), "0.015".into()));
    assert_eq!((heading(ft, &volume), shown(ft, Kind::Volume, &json!(1.0))), ("Net Volume (ft³)".into(), "35.315".into()));
    // As a tag or Filter on a selection shows a part's.
    assert_eq!(crate::schedule::field_text(&s.model, at.plaster, "Net Volume").as_deref(), Some("0.240"));
    assert_eq!(crate::schedule::field_text(&s.model, at.plaster, "Net Area").as_deref(), Some("12.00"));
    // A calculated field may come to one, and totals as one; a filter compares cubic metres.
    let t = run(
        &mut s,
        "schedule",
        json!({"name": "Rubble", "category": "parts", "fields": ["Id", "Net Volume",
            {"name": "Waste", "formula": "Net Volume * 0.1", "kind": "volume", "total": true}],
            "filters": [{"field": "Net Volume", "op": "greater", "value": 1}]}),
    );
    assert_eq!(t["columns"], json!(["Id", "Net Volume (m³)", "Waste (m³)"]));
    assert_eq!(t["rows"], json!([[n(at.masonry), 5.52, 0.552], [n(at.deck), 3.4, 0.34]]));
    assert_eq!(t["totals"], json!([null, null, 0.892]));
    let e = fails(&mut s, "schedule", json!({"name": "Rubble", "filters": [{"field": "Net Volume", "op": "greater", "value": "big"}]}));
    assert!(e.contains("it compares a volume (m³), not \"big\""), "{e}");
}

#[test]
fn the_restoration_quantities_starter_is_the_bill_of_quantities_made_in_one_go() {
    let (mut s, at) = site();
    let t = run(&mut s, "schedule", json!({"starter": "restoration-quantities"}));
    assert_eq!((&t["name"], &t["category"]), (&json!("Restoration Quantities"), &json!("part")));
    assert_eq!(
        t["columns"],
        json!(["Intervention_Type", "Decay_Pathology", "SU_Code", "Host Type", "Material", "Net Area (m²)", "Net Volume (m³)", "Count"])
    );
    assert_eq!(
        t["rows"],
        json!([
            [null, null, null, "Historic Floor", "Timber", 20.0, 3.4, 1.0],
            ["Consolidation", "Detachment", "USR 1", "Historic - 480mm", "Lime Plaster", 12.0, 0.24, 1.0],
            ["Consolidation", "Erosion", "USR 2", "Historic Floor", "Terracotta", 20.0, 0.6, 1.0],
            ["Repointing", "Crack", "USM 1", "Historic - 480mm", "Rubble Masonry", 12.0, 5.52, 1.0],
        ])
    );
    assert_eq!(t["totals"], json!([null, null, null, null, null, 64.0, 9.76, 4.0]));
    assert_eq!(t["elements"], json!([[at.deck], [at.plaster], [at.tiles], [at.masonry]]));
    // It is the project's own schedule from then on: read by the starter again or by its name, changed as any other.
    let id = t["id"].as_u64().unwrap();
    let Some(Element::Schedule(sc)) = s.model.elements.get(id) else { panic!("no schedule {id}") };
    assert_eq!((sc.of.as_str(), sc.grouped, sc.sort.len()), ("part", true, 2));
    assert_eq!(run(&mut s, "schedule", json!({"starter": "restoration-quantities"}))["id"], t["id"]);
    let by_severity = run(
        &mut s,
        "schedule",
        json!({"name": "Restoration Quantities", "filters": [{"field": "Decay_Severity", "op": "greater", "value": 2}]}),
    );
    assert_eq!((&by_severity["id"], by_severity["rows"].as_array().map(Vec::len)), (&t["id"], Some(2)));
    assert_eq!(by_severity["totals"], json!([null, null, null, null, null, 32.0, 0.84, 2.0]));
    // Under another name, another one; of another category, or one there isn't, it says what there is.
    let other = run(&mut s, "schedule", json!({"starter": "restoration-quantities", "name": "Works, north wing"}));
    assert_ne!(other["id"], t["id"]);
    assert_eq!(other["rows"].as_array().map(Vec::len), Some(4));
    let e = fails(&mut s, "schedule", json!({"starter": "demolition"}));
    assert!(e.contains("`demolition` isn't a starter schedule; there are: restoration-quantities"), "{e}");
    let e = fails(&mut s, "schedule", json!({"starter": "restoration-quantities", "name": "Walls", "category": "walls"}));
    assert!(e.contains("parts"), "{e}");
    for hostile in [json!(5), json!([]), json!({"x": 1}), json!("x".repeat(100_000))] {
        assert!(s.execute("schedule", &json!({"starter": hostile})).is_err());
    }
}

#[test]
fn the_bill_of_quantities_goes_to_a_workbook_with_its_numbers_as_numbers_and_to_csv() {
    let (mut s, _) = site();
    let dir = std::env::temp_dir().join(format!("bimcraft-parts-schedule-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (xlsx, csv) = (dir.join("restoration.xlsx"), dir.join("restoration.csv"));
    run(&mut s, "schedule", json!({"starter": "restoration-quantities", "path": xlsx}));
    run(&mut s, "schedule", json!({"starter": "restoration-quantities", "path": csv}));
    let parts = crate::xlsx::unzip(&std::fs::read(&xlsx).unwrap());
    assert!(parts["xl/workbook.xml"].contains(r#"name="Restoration Quantities""#));
    let sheet = &parts["xl/worksheets/sheet1.xml"];
    // The headings, in bold, say the units; text is text.
    for (at, heading) in [("A1", "Intervention_Type"), ("F1", "Net Area (m²)"), ("G1", "Net Volume (m³)"), ("H1", "Count")] {
        let cell = format!(r#"<c r="{at}" s="1" t="inlineStr"><is><t xml:space="preserve">{heading}</t></is></c>"#);
        assert!(sheet.contains(&cell), "{cell} in {sheet}");
    }
    assert!(sheet.contains(r#"<c r="A3" t="inlineStr"><is><t xml:space="preserve">Consolidation</t></is></c>"#), "{sheet}");
    // The quantities are numbers, to be priced: the plaster's row, and the totals under the last, in bold.
    assert!(sheet.contains(r#"<c r="F3"><v>12</v></c><c r="G3"><v>0.24</v></c><c r="H3"><v>1</v></c>"#), "{sheet}");
    assert!(
        sheet.contains(r#"<row r="6"><c r="F6" s="1"><v>64</v></c><c r="G6" s="1"><v>9.76</v></c><c r="H6" s="1"><v>4</v></c></row>"#),
        "{sheet}"
    );
    // What has no value is no cell: the deck's row starts at its host's type.
    assert!(sheet.contains(r#"<row r="2"><c r="D2" t="inlineStr">"#), "{sheet}");
    assert_eq!(
        std::fs::read_to_string(&csv).unwrap(),
        "Intervention_Type,Decay_Pathology,SU_Code,Host Type,Material,Net Area (m²),Net Volume (m³),Count\n\
         ,,,Historic Floor,Timber,20.0,3.4,1.0\n\
         Consolidation,Detachment,USR 1,Historic - 480mm,Lime Plaster,12.0,0.24,1.0\n\
         Consolidation,Erosion,USR 2,Historic Floor,Terracotta,20.0,0.6,1.0\n\
         Repointing,Crack,USM 1,Historic - 480mm,Rubble Masonry,12.0,5.52,1.0\n\
         ,,,,,64.0,9.76,4.0\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_multi_category_schedule_lists_no_parts_beside_what_they_are_parts_of() {
    let (mut s, _) = site();
    let t = run(&mut s, "schedule", json!({"name": "Everything", "category": "multicategory", "fields": ["Category", "Count"]}));
    assert_eq!(t["rows"], json!([["floor", 1.0], ["wall", 1.0]]));
    assert!(!crate::schedule::built().contains(&"part"));
    let e = fails(&mut s, "schedule", json!({"name": "Both", "category": "multicategory", "categories": ["walls", "parts"]}));
    assert!(e.contains("`part` isn't a category a multi-category schedule lists"), "{e}");
}
