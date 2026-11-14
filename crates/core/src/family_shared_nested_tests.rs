//! Shared nested instances as elements of their own (T6b): each component of a family placing an instance of a shared nested
//! family exposes it as an element, turned and mirrored with its host, scheduled and counted among the components, tagged,
//! written to IFC aggregated into its host, and found again by its GUID after the file is saved and read; nothing of it is
//! in the file. An instance of a family that isn't shared stays part of its host.

use serde_json::{Value, json};

use crate::engine::Session;
use crate::ids::Id;
use crate::model::{Element, Model};
use crate::nested_elements::NestedElement;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// A handle `W` wide, as a family file holds it, shared or not.
fn handle(shared: bool) -> Value {
    json!({"category": "generic", "shared": shared, "parameters": [{"name": "W", "value": 40}], "types": {"Lever": {}},
           "forms": [{"profile": [[0, 0], ["W", 0], ["W", 20], [0, 20]], "to": 30}]})
}

/// A cabinet 600 wide whose handle (shared, or not) is placed at (100, 50), as wide as a tenth of the cabinet, and a
/// component of it at (5000, 0), turned 90 degrees.
fn cabinet(shared: bool) -> (Session, Id) {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "category": "furniture", "parameters": [{"name": "Width", "value": 600}],
               "types": {"Small": {"Width": 600}},
               "forms": [{"profile": [[0, 0], ["Width", 0], ["Width", 400], [0, 400]], "to": 800}]}),
    );
    run(&mut s, "family", json!({"name": "Cabinet", "nest": {"definition": handle(shared), "name": "Handle"}}));
    run(
        &mut s,
        "family",
        json!({"name": "Cabinet", "nested_instance": {"family": "Handle", "type": "Lever", "at": [100, 50], "params": {"W": "Width / 10"}}}),
    );
    let id = run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [5000, 0], "angle": 90}))["id"].as_u64().unwrap();
    (s, id)
}

/// The nested elements of the model, by id.
fn nested(m: &Model) -> Vec<(Id, NestedElement)> {
    m.elements
        .iter()
        .filter_map(|(id, e)| match e {
            Element::Nested(n) => Some((*id, n.clone())),
            _ => None,
        })
        .collect()
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

#[test]
fn a_shared_nested_instance_is_an_element_turned_and_mirrored_with_its_host() {
    let (mut s, host) = cabinet(true);
    let found = nested(&s.model);
    assert_eq!(found.len(), 1, "one element for the handle of the one cabinet");
    let (id, n) = &found[0];
    assert_eq!(n.host, s.model.guid(host));
    assert_eq!((n.family.as_str(), n.type_name.as_str()), ("Handle", "Lever"));
    assert_eq!(n.path, vec![s.model.families["Cabinet"].nested_instances[0].guid]);
    // (100, 50) in the cabinet's axes, turned 90 degrees about (5000, 0): (4950, 100); its x along the cabinet's y.
    assert!(near(n.origin, [4950.0, 100.0, 0.0]), "{:?}", n.origin);
    assert!(near(n.axes[0], [0.0, 1.0, 0.0]), "{:?}", n.axes);
    // Its own solids: the handle, 60 wide (a tenth of 600), where it stands; the cabinet's aren't among them.
    let pieces = s.model.nested_pieces(n);
    assert_eq!(pieces.len(), 1);
    let ys: Vec<f64> = pieces[0].tris.iter().flatten().map(|p| p[1]).collect();
    let (lo, hi) = ys.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), y| (a.min(*y), b.max(*y)));
    assert!((lo - 100.0).abs() < 1e-6 && (hi - 160.0).abs() < 1e-6, "{lo} to {hi}");
    // Its element is in the selection's words: a component of family Handle.
    let json = crate::commands::element_json(&s.model, *id);
    assert_eq!((json["category"].as_str(), json["family"].as_str()), (Some("nested"), Some("Handle")));
    // Mirrored across y = 0, the host takes it along: (4950, -100), its axes left-handed. The same element, by its GUID.
    let guid = s.model.guid(*id);
    run(&mut s, "mirror", json!({"ids": [host], "axis": [[0, 0], [10000, 0]]}));
    let (after, n) = nested(&s.model).pop().unwrap();
    assert_eq!(s.model.guid(after), guid);
    assert!(near(n.origin, [4950.0, -100.0, 0.0]), "{:?}", n.origin);
    let [x, y, z] = n.axes;
    let cross = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
    assert!(cross[0] * z[0] + cross[1] * z[1] + cross[2] * z[2] < 0.0, "mirrored: {:?}", n.axes);
    // Undone, it is where it was.
    run(&mut s, "undo", json!({}));
    assert!(near(nested(&s.model)[0].1.origin, [4950.0, 100.0, 0.0]));
}

#[test]
fn a_shared_nested_element_is_scheduled_and_counted_among_the_components() {
    let (mut s, _) = cabinet(true);
    run(&mut s, "component", json!({"family": "Cabinet", "type": "Small", "at": [0, 0]}));
    let r = run(&mut s, "schedule", json!({"name": "By Family", "category": "component", "fields": ["Family", "Type", "Count"]}));
    let rows = r["rows"].as_array().unwrap().clone();
    let count = |family: &str| rows.iter().filter(|row| row[0] == json!(family)).map(|row| row[2].as_f64().unwrap()).sum::<f64>();
    assert_eq!(count("Cabinet"), 2.0, "{rows:?}");
    assert_eq!(count("Handle"), 2.0, "a handle for each cabinet: {rows:?}");
    // The rows pick what they list: the handles' are the nested elements.
    let handles: Vec<Id> = nested(&s.model).into_iter().map(|(id, _)| id).collect();
    let listed: Vec<Id> =
        r["elements"].as_array().unwrap().iter().flat_map(|ids| ids.as_array().unwrap().iter().map(|v| v.as_u64().unwrap())).collect();
    assert!(handles.iter().all(|h| listed.contains(h)), "{listed:?} lists {handles:?}");
}

#[test]
fn a_tag_on_a_shared_nested_element_survives_saving_and_reading_the_file() {
    let (mut s, host) = cabinet(true);
    let (id, _) = nested(&s.model)[0].clone();
    let guid = s.model.guid(id);
    let tag = run(&mut s, "tag", json!({"ids": [id]}))["ids"][0].as_u64().unwrap();
    assert_eq!(crate::schedule::field_text(&s.model, id, "Type").as_deref(), Some("Lever"), "what its tag shows");
    // Nothing of the element is in the file; the tag holds its GUID.
    let text = s.model.to_json().unwrap();
    assert!(!text.contains("\"category\":\"nested\""), "no nested element written");
    assert!(text.contains(&guid.to_string()), "the tag holds the element's GUID");
    let (back, left_out) = Model::read(&text).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    back.check().unwrap();
    let again = back.elements.id(guid).expect("the element, by the same GUID");
    assert!(matches!(back.elements.get(again), Some(Element::Nested(_))));
    let Some(Element::Tag(t)) = back.elements.get(back.elements.id(s.model.guid(tag)).unwrap()) else { panic!("the tag") };
    assert_eq!(t.host, guid);
    // Its host deleted, it goes, and its tag with it.
    run(&mut s, "delete", json!({"ids": [host]}));
    assert!(nested(&s.model).is_empty());
    assert!(!s.model.elements.values().any(|e| matches!(e, Element::Tag(_))));
}

#[test]
fn a_shared_nested_element_cant_be_deleted_apart_from_its_host() {
    let (mut s, _) = cabinet(true);
    let (id, _) = nested(&s.model)[0].clone();
    let e = s.execute("delete", &json!({"ids": [id]})).unwrap_err();
    assert!(e.contains("part of component"), "{e}");
}

#[test]
fn ifc_writes_a_shared_nested_element_of_its_own_aggregated_into_its_host() {
    let (s, host) = cabinet(true);
    let (id, _) = nested(&s.model)[0].clone();
    let file = crate::ifc::export(&s.model);
    let global = |id: Id| crate::ifc::compress(s.model.guid(id).as_u128());
    let entity = |gid: &str| {
        file.lines()
            .find(|l| l.contains(&format!("'{gid}'")) && !l.contains("IFCREL"))
            .and_then(|l| l.split('=').next())
            .map(str::to_string)
            .unwrap_or_else(|| panic!("an entity with GlobalId {gid}"))
    };
    let (whole, part) = (entity(&global(host)), entity(&global(id)));
    let aggregates = file.lines().filter(|l| l.contains("IFCRELAGGREGATES")).find(|l| l.contains(&format!(",{whole},(")));
    let aggregates = aggregates.unwrap_or_else(|| panic!("the cabinet aggregates its parts:\n{file}"));
    assert!(
        aggregates.contains(&format!("({part})")) || aggregates.contains(&format!("{part},")) || aggregates.contains(&format!(",{part})"))
    );
    // A part is in its whole, not in the storey.
    let contained = file
        .lines()
        .filter(|l| l.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE"))
        .any(|l| l.contains(&format!("{part},")) || l.contains(&format!("{part})")));
    assert!(!contained, "the handle is in the cabinet, not in the storey");
    // The handle's triangles are in its own body, not in the cabinet's too: one face set each.
    assert_eq!(file.lines().filter(|l| l.contains("IFCTRIANGULATEDFACESET")).count(), 2);
    let line = file.lines().find(|l| l.starts_with(&format!("{part}="))).unwrap();
    assert!(line.contains("'Handle:Lever'"), "{line}");
}

#[test]
fn an_instance_of_a_family_that_isnt_shared_stays_part_of_its_host() {
    let (s, host) = cabinet(false);
    assert!(nested(&s.model).is_empty());
    let Some(Element::Component(c)) = s.model.elements.get(host) else { panic!("a component") };
    assert!(s.model.component_pieces(c).iter().all(|p| p.nested.is_empty()));
    let file = crate::ifc::export(&s.model);
    assert!(!file.contains("'Handle:Lever'"));
    let r = crate::schedule::table(&s.model, &crate::model::Schedule { of: "component".into(), ..Default::default() }).unwrap();
    assert_eq!(r.ids.len(), 1, "the cabinet alone");
}

/// A project saved before T6b, with a component of a family that nests a shared one: it opens as it was, nothing left out,
/// its elements as the file has them, with the handle's element worked out beside them.
#[test]
fn a_file_from_before_opens_unchanged() {
    let (s, _) = cabinet(true);
    let mut old: Value = serde_json::from_str(&s.model.to_json().unwrap()).unwrap();
    // What a file from before holds: no nested elements, which no file ever does.
    assert!(old["elements"].as_object().unwrap().values().all(|e| e["category"] != "nested"));
    old["format"] = json!(crate::model::FORMAT);
    let text = old.to_string();
    let (m, left_out) = Model::read(&text).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    m.check().unwrap();
    for k in old["elements"].as_object().unwrap().keys() {
        assert!(m.elements.get(k.parse::<Id>().unwrap()).is_some_and(|e| !matches!(e, Element::Nested(_))), "element {k}");
    }
    assert_eq!(nested(&m).len(), 1);
    let again: Value = serde_json::from_str(&m.to_json().unwrap()).unwrap();
    assert_eq!(again["elements"], old["elements"], "written back as it was");
    assert_eq!(again["families"], old["families"]);
}

/// In the plan, a click on the handle takes the cabinet; the handle comes next, for Tab to reach.
#[test]
fn a_plan_picks_the_host_first_and_its_shared_nested_instance_next() {
    let (s, host) = cabinet(true);
    let (id, n) = nested(&s.model)[0].clone();
    let view = crate::render::View { scale: 0.5, cx: 5000.0, cy: 300.0, width: 800, height: 800, angle: 0.0 };
    let d = crate::render::draw(&s.model, n.level, view);
    assert_eq!(d.nested.len(), 1);
    assert_eq!((d.nested[0].id, d.nested[0].host), (id, host));
    // The handle's middle: (100 + 30, 50 + 10) in the cabinet's axes, turned 90 degrees about (5000, 0).
    let at = view.px(crate::geom::P2(4940.0, 130.0));
    assert_eq!(d.pick_all(at, 3.0), vec![host, id]);
    assert_eq!(d.pick(at, 3.0), Some(host));
    // Away from it, only the cabinet.
    let off = view.px(crate::geom::P2(4700.0, 500.0));
    assert!(!d.pick_all(off, 3.0).contains(&id));
}

/// A tag's prims in a drawing: the box round them, their width and height in pixels.
fn tag_size(d: &crate::render::Drawing, tag: Id) -> (f32, f32) {
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for (prim, owner) in d.prims.iter().zip(&d.owners) {
        if *owner != Some(tag) {
            continue;
        }
        let pts: Vec<(f32, f32)> = match prim {
            crate::render::Prim::Fill { rings, .. } => rings.iter().flatten().copied().collect(),
            crate::render::Prim::Line { pts, .. } => pts.clone(),
            crate::render::Prim::Image { .. } => Vec::new(),
        };
        for (x, y) in pts {
            (lo[0], lo[1], hi[0], hi[1]) = (lo[0].min(x), lo[1].min(y), hi[0].max(x), hi[1].max(y));
        }
    }
    (hi[0] - lo[0], hi[1] - lo[1])
}

/// A tag on the handle is drawn as a tag on the cabinet is, at the same scale: the same height, its words its own.
#[test]
fn a_tag_on_a_shared_nested_element_is_drawn_as_one_on_a_component() {
    let (mut s, host) = cabinet(true);
    let (id, n) = nested(&s.model)[0].clone();
    let on_cabinet = run(&mut s, "tag", json!({"ids": [host], "offset": [0, 1500]}))["ids"][0].as_u64().unwrap();
    let on_handle = run(&mut s, "tag", json!({"ids": [id], "offset": [0, -1500]}))["ids"][0].as_u64().unwrap();
    for scale in [0.05, 0.35] {
        let view = crate::render::View { scale, cx: 5000.0, cy: 0.0, width: 1600, height: 1600, angle: 0.0 };
        let d = crate::render::draw(&s.model, n.level, view);
        let (a, b) = (tag_size(&d, on_cabinet), tag_size(&d, on_handle));
        assert!(a.1 > 0.0 && (a.1 - b.1).abs() < 0.01, "at {scale} px a mm: the cabinet's {a:?}, the handle's {b:?}");
    }
    let text = |tag: Id| match s.model.elements.get(tag) {
        Some(Element::Tag(t)) => s.model.tag_text(t),
        _ => String::new(),
    };
    assert_eq!((text(on_cabinet), text(on_handle)), ("Small".to_string(), "Lever".to_string()));
}

/// The handle is in the cabinet's phases and design option: a schedule's phase fields say them, as the cabinet's do.
#[test]
fn a_shared_nested_element_is_in_its_hosts_phases() {
    let (mut s, host) = cabinet(true);
    run(&mut s, "modify", json!({"id": host, "params": {"Phase Created": "Existing", "Phase Demolished": "New Construction"}}));
    let (id, _) = nested(&s.model)[0].clone();
    let params = s.model.elements.get(id).and_then(Element::params).cloned().unwrap_or_default();
    assert_eq!(params.get("Phase Created"), Some(&json!("Existing")));
    let r = run(
        &mut s,
        "schedule",
        json!({"name": "Phased", "category": "component", "fields": ["Family", "Phase Created", "Phase Demolished"]}),
    );
    let rows = r["rows"].as_array().unwrap();
    assert!(
        rows.iter().all(|row| row[1] == json!("Existing") && row[2] == json!("New Construction")),
        "the cabinet's and the handle's: {rows:?}"
    );
    // Its phase is its host's: set on it, it is refused, or changes nothing.
    let _ = s.execute("modify", &json!({"id": id, "params": {"Phase Created": "New Construction"}}));
    let params = s.model.elements.get(id).and_then(Element::params).cloned().unwrap_or_default();
    assert_eq!(params.get("Phase Created"), Some(&json!("Existing")));
}

/// A project inserted into itself: the cabinet comes in as a copy with a GUID of its own, and so does its handle, which
/// the copied tag follows.
#[test]
fn insert_from_file_brings_a_tag_on_a_shared_nested_element_onto_the_copys() {
    let (mut s, _) = cabinet(true);
    let (id, _) = nested(&s.model)[0].clone();
    run(&mut s, "tag", json!({"ids": [id]}));
    let path = std::env::temp_dir().join(format!("bimcraft-t6b-insert-{}.bimcraft", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    run(&mut s, "save", json!({"path": path}));
    run(&mut s, "insert", json!({"path": path}));
    let _ = std::fs::remove_file(&path);
    let handles = nested(&s.model);
    assert_eq!(handles.len(), 2, "a handle for each cabinet");
    let tags: Vec<crate::ids::Guid> = s
        .model
        .elements
        .values()
        .filter_map(|e| match e {
            Element::Tag(t) => Some(t.host),
            _ => None,
        })
        .collect();
    assert_eq!(tags.len(), 2, "both tags kept");
    for (h, _) in &handles {
        assert!(tags.contains(&s.model.guid(*h)), "a tag on each handle");
    }
    assert_eq!(s.model.check(), Ok(()));
}

/// A linked project's cabinet, moved where the link puts it, takes its handle along.
#[test]
fn a_linked_cabinets_handle_stands_where_the_link_puts_it() {
    let (mut s, _) = cabinet(true);
    let path = std::env::temp_dir().join(format!("bimcraft-t6b-link-{}.bimcraft", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    run(&mut s, "save", json!({"path": path}));
    let mut host = Session::new();
    run(&mut host, "link", json!({"path": path, "at": [1000, 2000]}));
    let _ = std::fs::remove_file(&path);
    let seen = crate::link::seen(&host.model);
    let found = nested(&seen);
    assert_eq!(found.len(), 1);
    assert!(near(found[0].1.origin, [5950.0, 2100.0, 0.0]), "{:?}", found[0].1.origin);
}

/// A shared nested family of plan symbols alone: a plan picks it by its lines, after its host.
#[test]
fn a_shared_nested_family_of_symbols_alone_is_picked_by_its_lines() {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Desk", "category": "furniture", "types": {"D": {}},
               "forms": [{"profile": [[0, 0], [1200, 0], [1200, 600], [0, 600]], "to": 750}]}),
    );
    let mark = json!({"category": "generic", "shared": true, "types": {"X": {}}, "symbols": [{"line": [[0, 0], [300, 0]]}]});
    run(&mut s, "family", json!({"name": "Desk", "nest": {"definition": mark, "name": "Mark"}}));
    run(&mut s, "family", json!({"name": "Desk", "nested_instance": {"family": "Mark", "type": "X", "at": [100, 300]}}));
    let host = run(&mut s, "component", json!({"family": "Desk", "type": "D", "at": [0, 0]}))["id"].as_u64().unwrap();
    let (id, n) = nested(&s.model)[0].clone();
    let view = crate::render::View { scale: 0.5, cx: 600.0, cy: 300.0, width: 800, height: 800, angle: 0.0 };
    let d = crate::render::draw(&s.model, n.level, view);
    let at = view.px(crate::geom::P2(250.0, 300.0));
    assert_eq!(d.pick_all(at, 3.0), vec![host, id]);
}

/// A cabinet whose handle, shared, stands proud of its front, 400 up: seen in an elevation looking north at it, the handle is
/// where it is drawn, after the cabinet.
#[test]
fn an_elevation_sees_a_shared_nested_instance_where_it_draws_it() {
    let (mut s, host) = cabinet(true);
    let g = s.model.families["Cabinet"].nested_instances[0].guid;
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "at": [250, -20], "offset": 400}}));
    run(&mut s, "rotate", json!({"ids": [host], "about": [5000, 0], "angle": -90}));
    let id = run(&mut s, "elevation", json!({"start": [4000, -2000], "end": [7000, -2000]}))["id"].as_u64().unwrap();
    let view = crate::render::View { scale: 0.5, cx: 1500.0, cy: 400.0, width: 800, height: 800, angle: 0.0 };
    let seen = s.model.nested_seen(host, &|p| crate::render::section_px(&s.model, id, view, p));
    assert_eq!(seen.len(), 1);
    // The handle, 60 wide (x 5250 to 5310), 400 to 430 up: 1280 along the line, 415 up.
    let at = view.px(crate::geom::P2(1280.0, 415.0));
    assert_eq!(crate::render::nested_at(&seen, at, 3.0), vec![seen[0].id]);
    assert!(crate::render::nested_at(&seen, view.px(crate::geom::P2(1100.0, 700.0)), 3.0).is_empty());
}

/// The handle stands proud of the cabinet's front, 400 up; the cabinet at (5000, 0), not turned. With `shared` or not.
fn front(shared: bool) -> (Session, Id) {
    let (mut s, host) = cabinet(shared);
    let g = s.model.families["Cabinet"].nested_instances[0].guid;
    run(&mut s, "family", json!({"name": "Cabinet", "nested_instance": {"guid": g, "at": [250, -20], "offset": 400}}));
    run(&mut s, "rotate", json!({"ids": [host], "about": [5000, 0], "angle": -90}));
    (s, host)
}

/// A shared nested instance is still drawn as part of its host, as one that isn't shared is: its lines in an elevation, its
/// triangles in the 3D scene. Only IFC writes it apart.
#[test]
fn a_shared_nested_instance_is_drawn_in_elevations_and_in_3d() {
    for shared in [true, false] {
        let (mut s, host) = front(shared);
        let id = run(&mut s, "elevation", json!({"start": [4000, -2000], "end": [7000, -2000]}))["id"].as_u64().unwrap();
        let view = crate::render::View { scale: 1.0, cx: 1300.0, cy: 450.0, width: 600, height: 600, angle: 0.0 };
        let pm = tiny_skia::Pixmap::decode_png(&crate::render::section_png(&s.model, id, view).unwrap()).unwrap();
        // Along the handle's top edge, 430 up from 1250 to 1310 along the line (60 wide): ink, not paper.
        let dark = (1255..1305)
            .filter(|x| {
                let (u, v) = view.px(crate::geom::P2(f64::from(*x), 430.0));
                (-1..=1).any(|dv| pm.pixel(u as u32, (v as i32 + dv) as u32).is_some_and(|c| c.red() < 128))
            })
            .count();
        assert!(dark > 40, "shared {shared}: the handle's top edge is drawn in the elevation ({dark} of 50)");
        // In 3D, the cabinet's triangles reach the handle's front, 20 in front of its own.
        let reaches = crate::view3d::solids(&s.model)
            .iter()
            .filter(|x| x.owner() == host)
            .flat_map(|x| x.triangles())
            .any(|t| t.iter().any(|p| (p[1] + 20.0).abs() < 1e-6 && (p[2] - 430.0).abs() < 1e-6));
        assert!(reaches, "shared {shared}: the handle is in the 3D scene");
    }
}
