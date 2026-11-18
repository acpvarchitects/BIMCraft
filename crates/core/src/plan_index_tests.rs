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
        let d = render::family_plan(&s.model, LEVEL, at(0.0, 0.0), None);
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
