//! GEO-2g-2: a plan finds its components through the model's index of them (`component_geometry/index.rs`): their keys,
//! envelopes and lines' boxes found once and kept by revision, and a grid of where they stand, brought up to date by what an
//! edit changed. Each test holds the indexed plan to the plan drawn without the index, or to one drawn on a fresh thread
//! (which keeps nothing from this one's drawings).

use serde_json::{Value, json};

use crate::component_geometry as cg;
use crate::engine::Session;
use crate::model::{Element, Model};
use crate::render::{self, Drawing, View};

const LEVEL: u64 = 100000;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn id_of(v: &Value) -> u64 {
    v["id"].as_u64().or_else(|| v["ids"][0].as_u64()).unwrap_or_else(|| panic!("{v}"))
}

/// A box family, its width a type parameter, its height each instance's own; on a wall's face, it follows it.
fn boxes() -> Session {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Box", "category": "generic", "options": {"work_plane_based": true, "always_vertical": false},
               "parameters": [{"name": "W", "value": 800}, {"name": "Height", "instance": true, "value": 600}],
               "types": {"T": {}, "U": {"W": 500}}, "forms": [{"profile": [[0, 0], ["W", 0], ["W", 400], [0, 400]], "to": "Height"}]}),
    );
    s
}

/// `side` by `side` boxes 2 m apart from the origin, each its own height: one placed by the command, the rest put straight in.
fn grid(side: usize) -> Session {
    let mut s = boxes();
    let first = id_of(&run(&mut s, "component", json!({"family": "Box", "at": [0, 0]})));
    let Some(Element::Component(one)) = s.model.elements.get(first).cloned() else { panic!("a box") };
    let model = std::sync::Arc::make_mut(&mut s.model);
    model.elements.remove(first);
    for k in 0..side * side {
        let mut c = one.clone();
        c.params = [("Height".to_string(), json!(600 + k))].into_iter().collect();
        c.at = crate::geom::P2((k % side) as f64 * 2000.0, (k / side) as f64 * 2000.0);
        model.insert(Element::Component(c));
    }
    s
}

/// A screen 10 m by 8 m, its middle at (`cx`, `cy`).
fn at(cx: f64, cy: f64) -> View {
    View { angle: 0.0, scale: 0.1, cx, cy, width: 1000, height: 800 }
}

fn same(a: &Drawing, b: &Drawing) -> bool {
    a.prims.len() == b.prims.len()
        && a.owners == b.owners
        && a.parts == b.parts
        && a.prims.iter().zip(&b.prims).all(|(x, y)| x.points() == y.points())
}

/// `m`'s plan of `level` through `view` drawn on a thread of its own: nothing kept from this thread's drawings.
fn fresh(m: &std::sync::Arc<Model>, level: u64, view: View) -> Drawing {
    let m = std::sync::Arc::clone(m);
    std::thread::spawn(move || render::draw(&m, level, view)).join().unwrap_or_else(|_| panic!("drawn"))
}

/// Drawn through the index, as drawn without it and as a fresh thread draws it; and the index kept is the one made afresh.
fn check(m: &std::sync::Arc<Model>, level: u64, view: View, what: &str) -> Drawing {
    let indexed = render::draw(m, level, view);
    let unindexed = cg::without_index(|| render::draw(m, level, view));
    assert!(same(&indexed, &unindexed), "{what}: the indexed plan isn't the plan drawn among every component");
    assert!(same(&indexed, &fresh(m, level, view)), "{what}: the plan isn't the one drawn afresh");
    assert!(cg::index_is_made_afresh(m), "{what}: the index kept isn't the one made afresh");
    indexed
}

#[test]
fn a_pan_over_the_indexed_model_looks_only_near_the_window() {
    // 900 boxes over 60 m; a screen of 10 m by 8 m, its window three times that: some 15 by 12 of them.
    let s = grid(30);
    let view = at(20_000.0, 20_000.0);
    render::draw(&s.model, LEVEL, view);
    let (_, cells, candidates) = cg::index_counts();
    let panned = at(25_000.0, 20_000.0);
    let keys = cg::keys_made();
    // Drawn once, as a window draws it (a test's twin looks at every component).
    let d = render::without_twins(|| render::draw(&s.model, LEVEL, panned));
    assert_eq!(cg::keys_made(), keys, "no component's key made again: each from the index");
    let (_, cells_now, candidates_now) = cg::index_counts();
    let (looked_in, looked_at) = (cells_now - cells, candidates_now - candidates);
    // The window, 30 m by 24 m, over cells of 4,096 mm: no more than 9 by 8 of them; the boxes in or touching those cells.
    assert!(looked_in <= 9 * 8, "{looked_in} cells looked in");
    let near = |x: f64, lo: f64, hi: f64| x + 800.0 >= (lo / 4096.0).floor() * 4096.0 && x <= ((hi / 4096.0).floor() + 1.0) * 4096.0;
    let in_cells = (0..900)
        .filter(|k| {
            let (x, y) = ((k % 30) as f64 * 2000.0, (k / 30) as f64 * 2000.0);
            near(x, 10_000.0, 40_000.0) && near(y, 8_000.0, 32_000.0)
        })
        .count();
    assert!(looked_at <= in_cells, "{looked_at} candidates looked at, {in_cells} boxes in the window's cells");
    assert!(looked_at < 300, "{looked_at} of 900");
    assert!(d.owners.iter().flatten().count() > 0);
    check(&s.model, LEVEL, panned, "panned");
}

#[test]
fn an_edit_finds_again_only_that_component_in_the_index() {
    let mut s = grid(10);
    let view = at(8_000.0, 8_000.0);
    render::draw(&s.model, LEVEL, view);
    let one =
        s.model.elements.iter().find(|(_, e)| matches!(e, Element::Component(_))).map(|(id, _)| *id).unwrap_or_else(|| panic!("a box"));
    run(&mut s, "modify", json!({"id": one, "params": {"Height": 999}}));
    let (before, ..) = cg::index_counts();
    render::draw(&s.model, LEVEL, view);
    let (after, ..) = cg::index_counts();
    assert_eq!(after - before, 1, "the one edited, alone");
    check(&s.model, LEVEL, view, "edited");
}

#[test]
fn the_indexed_plan_is_the_plan_among_every_component_at_random_views_and_through_edits() {
    let mut s = grid(12);
    // A wall through them, a second type, and one turned.
    run(&mut s, "wall", json!({"points": [[0, 1000], [24000, 1000]]}));
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for k in 0..10 {
        let view = View { angle: rand() * 360.0, scale: 0.02 + rand() * 0.3, cx: rand() * 24_000.0, cy: rand() * 24_000.0, ..at(0.0, 0.0) };
        check(&s.model, LEVEL, view, &format!("random view {k}"));
    }
    let view = at(6_000.0, 6_000.0);
    check(&s.model, LEVEL, view, "before the edits");
    let ids: Vec<u64> = s.model.elements.iter().filter(|(_, e)| matches!(e, Element::Component(_))).map(|(id, _)| *id).collect();
    run(&mut s, "move", json!({"ids": [ids[3]], "by": [30_000, 0]}));
    check(&s.model, LEVEL, view, "moved out of the window");
    run(&mut s, "move", json!({"ids": [ids[100]], "by": [-14_000, -14_000]}));
    check(&s.model, LEVEL, view, "moved into the window");
    run(&mut s, "delete", json!({"ids": [ids[14]]}));
    check(&s.model, LEVEL, view, "deleted");
    run(&mut s, "component", json!({"family": "Box", "at": [5_000, 7_000], "angle": 30}));
    check(&s.model, LEVEL, view, "added");
    run(&mut s, "modify", json!({"id": ids[15], "type": "U"}));
    assert!(matches!(s.model.elements.get(ids[15]), Some(Element::Component(c)) if c.type_name == "U"));
    check(&s.model, LEVEL, view, "its type changed");
    for k in 0..5 {
        run(&mut s, "undo", json!({}));
        check(&s.model, LEVEL, view, &format!("undone {k}"));
    }
    run(&mut s, "redo", json!({}));
    check(&s.model, LEVEL, view, "redone");
}

/// A column-like box two storeys tall, on Level 1: in Level 2's plan, as drawn without the index.
#[test]
fn a_component_of_level_1_reaching_into_level_2s_range_is_drawn_in_its_plan() {
    let mut s = boxes();
    let upper = id_of(&run(&mut s, "level", json!({"elevation": 3000})));
    let tall = id_of(&run(&mut s, "component", json!({"family": "Box", "at": [0, 0], "params": {"Height": 5000}})));
    let short = id_of(&run(&mut s, "component", json!({"family": "Box", "at": [2000, 0], "params": {"Height": 900}})));
    let view = at(1_000.0, 0.0);
    let d = check(&s.model, upper, view, "Level 2");
    assert!(d.owners.contains(&Some(tall)), "the tall one, in Level 2's plan");
    assert!(!d.owners.contains(&Some(short)), "the short one, not");
}

/// A type's value changed: every one of the type drawn anew, none from what was found of it before.
#[test]
fn a_type_parameter_changed_redraws_every_one_of_its_type() {
    let mut s = boxes();
    for x in [0, 3000, 6000] {
        run(&mut s, "component", json!({"family": "Box", "at": [x, 0]}));
    }
    let view = at(3_000.0, 0.0);
    let before = check(&s.model, LEVEL, view, "before");
    run(&mut s, "family", json!({"name": "Box", "types": {"T": {"W": 1500}}}));
    let after = check(&s.model, LEVEL, view, "its type's width changed");
    assert!(!same(&before, &after), "drawn anew");
}

/// ec's review of #88: what places a component, through whatever moves it, comes to the index: a host wall moved, its type
/// changed, a level raised, a group moved, a link reloaded.
#[test]
fn what_places_a_component_brings_its_index_up_to_date() {
    let mut s = boxes();
    let wall = id_of(&run(&mut s, "wall", json!({"points": [[0, 0], [6000, 0]]})));
    let hosted = id_of(&run(
        &mut s,
        "component",
        json!({"family": "Box", "at": [2000, 0, 500], "host": {"face": {"element": wall, "face": "exterior"}}}),
    ));
    let view = at(3_000.0, 0.0);
    let was = check(&s.model, LEVEL, view, "hosted");
    assert!(was.owners.contains(&Some(hosted)));
    run(&mut s, "move", json!({"ids": [wall], "by": [0, 30_000]}));
    let moved = check(&s.model, LEVEL, view, "its wall moved off the window");
    assert!(!moved.owners.contains(&Some(hosted)), "it went with its wall");
    check(&s.model, LEVEL, at(3_000.0, 30_000.0), "where its wall went");
    run(&mut s, "type", json!({"category": "wall", "name": "Generic - 200mm", "thickness": 600}));
    check(&s.model, LEVEL, at(3_000.0, 30_000.0), "its wall's type thicker");
    // A level raised: what stands on it rises with it.
    let upper = id_of(&run(&mut s, "level", json!({"elevation": 3000})));
    let on_upper = id_of(&run(&mut s, "component", json!({"family": "Box", "at": [0, 5000], "level": upper})));
    check(&s.model, upper, view, "on Level 2");
    run(&mut s, "modify", json!({"id": upper, "elevation": 9000}));
    check(&s.model, upper, view, "Level 2 raised");
    check(&s.model, LEVEL, view, "Level 1 under it");
    // A group of it, moved.
    let made = run(&mut s, "create_group", json!({"ids": [on_upper]}));
    let group = s
        .model
        .elements
        .iter()
        .find(|(_, e)| matches!(e, Element::ModelGroup(_)))
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("a model group: {made}"));
    run(&mut s, "move", json!({"ids": [group], "by": [4000, 0]}));
    check(&s.model, upper, view, "its group moved");
    // A link of a project with a box in it, reloaded once the box moved.
    let path = std::env::temp_dir().join(format!("bimcraft-index-link-{}.bimcraft", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    let mut other = boxes();
    let linked_box = id_of(&run(&mut other, "component", json!({"family": "Box", "at": [1000, -3000]})));
    run(&mut other, "save", json!({"path": path}));
    let mut s = boxes();
    let link = id_of(&run(&mut s, "link", json!({"path": path})));
    let seen = crate::link::seen(&s.model);
    check(&seen, LEVEL, view, "linked");
    run(&mut other, "move", json!({"ids": [linked_box], "by": [3000, 0]}));
    run(&mut other, "save", json!({"path": path}));
    run(&mut s, "reload", json!({"id": link}));
    let _ = std::fs::remove_file(&path);
    let again = crate::link::seen(&s.model);
    let moved = check(&again, LEVEL, view, "the link reloaded");
    assert!(!same(&moved, &check(&seen, LEVEL, view, "as linked before")), "the linked box moved");
}

/// ec's review of #88: a family whose solid is a small box at its origin and whose model line runs 30 m from it, viewed at
/// the line's far end: the line is drawn, the box neither built nor drawn.
#[test]
fn a_component_whose_lines_alone_reach_the_window_draws_its_lines_not_its_solids() {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Long", "category": "generic", "types": {"T": {}},
               "forms": [{"profile": [[0, 0], [200, 0], [200, 200], [0, 200]], "to": 300}],
               "model_lines": [{"start": [0, 100, 0], "end": [30000, 100, 0]}]}),
    );
    let id = id_of(&run(&mut s, "component", json!({"family": "Long", "at": [0, 0]})));
    let view = at(29_000.0, 0.0);
    let before = cg::builds();
    let d = render::draw(&s.model, LEVEL, view);
    assert_eq!(cg::builds() - before, 0, "its solid isn't built");
    let owned: Vec<&render::Prim> = d.prims.iter().zip(&d.owners).filter(|(_, o)| **o == Some(id)).map(|(p, _)| p).collect();
    assert_eq!(owned.len(), 1, "its line alone");
    assert!(matches!(owned[0], render::Prim::Line { closed: false, .. }));
    check(&s.model, LEVEL, view, "its line far off");
    // Near its solid, both.
    let near = render::draw(&s.model, LEVEL, at(0.0, 0.0));
    assert!(near.owners.iter().filter(|o| **o == Some(id)).count() > 1);
}

/// A family's plan's own pass over its components is windowed as the project's plan is: one far off it isn't built.
#[test]
fn a_familys_plan_builds_nothing_far_off_its_window() {
    cg::without_fineness(|| {
        let mut s = boxes();
        let near = id_of(&run(&mut s, "component", json!({"family": "Box", "at": [0, 0]})));
        let far = id_of(&run(&mut s, "component", json!({"family": "Box", "at": [500_000, 0], "params": {"Height": 700}})));
        let before = cg::builds();
        let (d, _) = render::family_plan(&s.model, LEVEL, at(0.0, 0.0), None);
        assert_eq!(cg::builds() - before, 1, "the near one alone");
        assert!(d.owners.contains(&Some(near)) && !d.owners.contains(&Some(far)));
    });
}

/// ec's review of #88: the default builds an eviction takes out unshare their finer builds once a pass, each key's looked
/// up by key: an unsharing pass looks at no more finer builds than are kept, however many default builds go.
#[test]
fn an_eviction_unshares_its_finer_builds_once_a_pass_by_key() {
    // Each too fine to cut at a close zoom, its finer build its default one, shared (#84); each its own size.
    let rounds = || {
        let mut s = Session::new();
        let mut forms = vec![json!({"sketch": [{"centre": [0, 0], "radius": "R"}], "from": 0, "to": 3000})];
        for x in [-10000, 10000] {
            forms.push(json!({"sketch": [{"centre": [x, 0], "radius": 6000}], "from": 1000, "to": 4000, "void": true}));
        }
        run(
            &mut s,
            "family",
            json!({"name": "Round", "category": "generic", "parameters": [{"name": "R", "instance": true, "value": 20000}],
                   "types": {"T": {}}, "forms": forms}),
        );
        for k in 0..6 {
            run(&mut s, "component", json!({"family": "Round", "at": [0, 0], "params": {"R": 20000 + k}}));
        }
        s
    };
    let view = View { angle: 0.0, scale: 2.0, cx: 20_000.0, cy: 0.0, width: 1000, height: 800 };
    let s = rounds();
    render::draw(&s.model, LEVEL, view);
    let (shared, _) = cg::kept_finer();
    assert!(shared >= 6, "a finer build kept as its default one, each: {shared}");
    let (passes, looks) = cg::unshares();
    // Another model's drawing, with no room for a default build: the first one's go, in passes.
    let other = rounds();
    cg::with_budgets([1, 1 << 30], || render::draw(&other.model, LEVEL, view));
    let (passes_now, looks_now) = cg::unshares();
    assert!(passes_now > passes, "an unsharing pass");
    // Each pass looks only at the finer builds of the keys that went, not at all the generation holds for each of them: the
    // two models' twelve finer builds each looked at no more than once, though twelve default builds go.
    assert!(looks_now - looks <= 12, "{} finer builds looked at in {} passes", looks_now - looks, passes_now - passes);
    assert!(cg::kept_finer_honest());
    let (counted, summed) = cg::kept_bytes();
    assert_eq!(counted, summed);
}

/// A component's model lines are culled by their own heights, as the plan draws them (FE-15), not by its forms': a family
/// whose box stands wholly above the cut plane and whose model line lies on the floor draws its line alone. Before GEO-2g-2
/// the plan's filter left the component out by its forms' heights, so the plan drawn among its candidates missed the line
/// the plan drawn among every element has (its twin check failed here).
#[test]
fn a_components_line_under_the_cut_is_drawn_though_its_forms_are_above_it() {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "High", "category": "generic", "types": {"T": {}},
               "forms": [{"profile": [[0, 0], [200, 0], [200, 200], [0, 200]], "from": 2000, "to": 2500}],
               "model_lines": [{"start": [0, 100, 0], "end": [3000, 100, 0]}]}),
    );
    let id = id_of(&run(&mut s, "component", json!({"family": "High", "at": [0, 0]})));
    let view = at(1500.0, 0.0);
    let d = render::draw(&s.model, LEVEL, view);
    let owned: Vec<&render::Prim> = d.prims.iter().zip(&d.owners).filter(|(_, o)| **o == Some(id)).map(|(p, _)| p).collect();
    assert_eq!(owned.len(), 1, "its line alone");
    assert!(matches!(owned[0], render::Prim::Line { closed: false, .. }));
    check(&s.model, LEVEL, view, "its box above the cut");
}

/// ec's review of #90: a component whose host is gone keeps waiting on the host's GUID, so a host that comes back (an edit of
/// the model itself, a link's or a paste's element made after the component) places it again: its cells and box follow.
#[test]
fn a_host_gone_and_back_with_its_guid_places_its_component_again() {
    let mut s = boxes();
    let wall = id_of(&run(&mut s, "wall", json!({"points": [[0, 0], [6000, 0]]})));
    run(&mut s, "component", json!({"family": "Box", "at": [2000, 0, 500], "host": {"face": {"element": wall, "face": "exterior"}}}));
    check(&s.model, LEVEL, at(3_000.0, 0.0), "hosted");
    let mut m: Model = (*s.model).clone();
    let guid = m.elements.guid(wall).unwrap();
    let Some(Element::Wall(mut w)) = m.elements.remove(wall) else { panic!("the wall") };
    let gone = std::sync::Arc::new(m.clone());
    check(&gone, LEVEL, at(3_000.0, 0.0), "its host gone");
    // Back with its GUID, 30 m north.
    w.start.1 += 30_000.0;
    w.end.1 += 30_000.0;
    m.elements.insert_as(wall, guid, Element::Wall(w)).unwrap();
    let back = std::sync::Arc::new(m);
    // Its index entry is the one made afresh (it was stale before), and the plan the one drawn among every component.
    check(&back, LEVEL, at(3_000.0, 30_000.0), "its host back, moved");
    check(&back, LEVEL, at(3_000.0, 0.0), "where its host was");
}

/// ec's review of #90: a component taken out of a crowded cell costs the same however many share it, so a level's change
/// that puts many in one cell again isn't quadratic: 3,000 components in one cell, all put again, look at a few entries
/// each, not at every one beside them.
#[test]
fn many_components_in_one_cell_are_put_again_in_linear_time() {
    let mut s = boxes();
    let first = id_of(&run(&mut s, "component", json!({"family": "Box", "at": [10, 10]})));
    let mut m: Model = (*s.model).clone();
    let Some(Element::Component(one)) = m.elements.get(first).cloned() else { panic!("a component") };
    const N: u64 = 3_000;
    for k in 0..N {
        let mut c = one.clone();
        c.at = crate::geom::P2(10.0 + (k % 50) as f64, 10.0 + (k / 50) as f64);
        m.elements.insert(500_000 + k, Element::Component(c));
    }
    let crowded = std::sync::Arc::new(m.clone());
    check(&crowded, LEVEL, at(0.0, 0.0), "crowded");
    // Every one put again: the level they stand on raised.
    if let Some(Element::Level(l)) = m.elements.get_mut(LEVEL) {
        l.elevation += 100.0;
    }
    let raised = std::sync::Arc::new(m);
    let before = cg::index_scanned();
    check(&raised, LEVEL, at(0.0, 0.0), "its level raised");
    let scanned = cg::index_scanned() - before;
    assert!(scanned <= 4 * N as usize, "{scanned} entries looked at to take {N} out of one cell");
}

/// ec's review of #90: a component on a reference plane, the plane moved and tilted with the component itself unchanged:
/// its index entry follows the plane, as one on a wall follows the wall.
#[test]
fn a_component_on_a_reference_plane_follows_the_plane() {
    let mut s = Session::new();
    run(
        &mut s,
        "family",
        json!({"name": "Sign", "category": "generic", "types": {"A": {}}, "options": {"work_plane_based": true},
               "forms": [{"profile": [[0, 0], [1000, 0], [1000, 1000], [0, 1000]], "to": 100}]}),
    );
    let plane = id_of(&run(&mut s, "reference_plane", json!({"name": "Ramp", "start": [0, 0, 0], "end": [10000, 0, 0], "up": [0, 1, 0]})));
    let sign = id_of(&run(&mut s, "component", json!({"family": "Sign", "at": [2000, 2000], "host": {"plane": "Ramp"}})));
    let view = at(2_500.0, 2_500.0);
    let flat = check(&s.model, LEVEL, view, "on a flat plane");
    assert!(flat.owners.contains(&Some(sign)));
    // The plane alone moved 40 m east and tilted: the sign goes with it, its own element unchanged.
    let mut m: Model = (*s.model).clone();
    let Some(Element::RefPlane(p)) = m.elements.get_mut(plane) else { panic!("the plane") };
    p.start[0] += 40_000.0;
    p.end[0] += 40_000.0;
    p.up = [0.0, 0.8, 0.6];
    let moved = std::sync::Arc::new(m);
    check(&moved, LEVEL, view, "its plane moved off");
    check(&moved, LEVEL, at(42_500.0, 2_500.0), "where its plane went");
}

/// `count` boxes placed in a row along x, 2 m apart.
fn placed(s: &mut Session, count: usize) -> Vec<u64> {
    (0..count).map(|k| id_of(&run(s, "component", json!({"family": "Box", "at": [k as f64 * 2000.0, 0]})))).collect()
}

/// ec's review of #90: an instance parameter dragged makes a new key each step; the index keeps the local shape of each
/// key its components have now, not of every key they had, so a drag doesn't heap them up until the index is wiped.
#[test]
fn a_dragged_instance_parameter_leaves_only_the_keys_its_components_have() {
    let mut s = boxes();
    let ids = placed(&mut s, 2);
    // Each its height given, so one brought back to the other's has the other's key (a value left to the family's
    // default is a key of its own).
    run(&mut s, "modify", json!({"ids": ids, "params": {"Height": 600}}));
    let view = at(0.0, 0.0);
    check(&s.model, LEVEL, view, "two alike");
    assert_eq!(cg::index_locals(&s.model), 1, "two boxes alike share one key");
    for step in 0..40 {
        run(&mut s, "modify", json!({"id": ids[0], "params": {"Height": 601 + step}}));
        render::draw(&s.model, LEVEL, view);
    }
    assert_eq!(cg::index_locals(&s.model), 2, "the one dragged and the one left: two keys, not one a step");
    check(&s.model, LEVEL, view, "dragged");
    // Brought back to its twin's height: one key again.
    run(&mut s, "modify", json!({"id": ids[0], "params": {"Height": 600}}));
    check(&s.model, LEVEL, view, "brought back");
    assert_eq!(cg::index_locals(&s.model), 1, "alike again");
}

/// ec's review of #90: two threads asking for the index of one model at once (two plans on a sheet drawn side by side): it is
/// made once, the second waiting for the first's, not made again beside it. Tests keep the store on each thread, so this
/// holds an `IndexSlot` itself, as `index_of` holds the one kept with a generation; that `index_of` takes it from the store
/// and doesn't take the index out of it again is read, not tested here (cp's review of #97).
#[test]
fn two_threads_asking_for_the_index_at_once_make_it_once() {
    use std::sync::{Arc, Barrier};
    let s = grid(40);
    let components = s.model.elements.iter().filter(|(_, e)| matches!(e, Element::Component(_))).count();
    for round in 0..5 {
        let slot = Arc::new(cg::IndexSlot::default());
        let barrier = Arc::new(Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let (slot, barrier, m) = (Arc::clone(&slot), Arc::clone(&barrier), Arc::clone(&s.model));
                std::thread::spawn(move || {
                    barrier.wait();
                    let index = slot.brought_up(&m);
                    (cg::index_counts().0, index.same_as_made(&m))
                })
            })
            .collect();
        let mut found = 0;
        for t in threads {
            let (n, made) = t.join().unwrap_or_else(|_| panic!("a thread"));
            assert!(made, "round {round}: an index isn't the one made afresh");
            found += n;
        }
        assert_eq!(found, components, "round {round}: entries found by both threads");
    }
}
