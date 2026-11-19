//! GEO-2g: a plan of many unique components. What a plan builds of its components is kept across drawings (the default
//! builds in a tier of their own), only what may show in its window is built, what its heights leave out isn't, and what is
//! built is built on several threads, drawn in the model's order.

use serde_json::{Value, json};

use crate::component_geometry as cg;
use crate::engine::Session;
use crate::render::{self, Drawing, View};

const LEVEL: u64 = 100000;
/// How far apart the boxes stand, and how wide each is, mm.
const SPACING: f64 = 2000.0;
const WIDE: f64 = 800.0;

/// A box family whose envelope is known without building it, its height each instance's own.
fn boxes() -> Session {
    let mut s = Session::new();
    s.execute(
        "family",
        &json!({"name": "Box", "category": "generic", "parameters": [{"name": "Height", "instance": true, "value": 600}],
                "types": {"T": {}}, "forms": [{"profile": [[0, 0], [WIDE, 0], [WIDE, 400], [0, 400]], "to": "Height"}]}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    s
}

fn place(s: &mut Session, at: [f64; 2], params: Value) -> u64 {
    let v = s.execute("component", &json!({"family": "Box", "at": at, "params": params})).unwrap_or_else(|e| panic!("{e}"));
    v["id"].as_u64().unwrap_or_else(|| panic!("{v}"))
}

/// `n` boxes along x from the origin, `SPACING` apart, each its own height (so each its own build).
fn row(n: usize) -> (Session, Vec<u64>) {
    let mut s = boxes();
    let ids = (0..n).map(|k| place(&mut s, [k as f64 * SPACING, 0.0], json!({"Height": 600 + k}))).collect();
    (s, ids)
}

/// A screen 10 m by 8 m, its middle at (`cx`, 0).
fn at(cx: f64) -> View {
    View { angle: 0.0, scale: 0.1, cx, cy: 0.0, width: 1000, height: 800 }
}

/// Whether box `k` of a row meets the window of `view`: its screen and one more screen each way.
fn meets(k: usize, view: View) -> bool {
    let x = k as f64 * SPACING;
    let half = 1.5 * f64::from(view.width) / view.scale;
    x + WIDE >= view.cx - half && x <= view.cx + half
}

/// How many components drawing `view` builds.
fn built(s: &Session, view: View) -> usize {
    let before = cg::builds();
    render::draw(&s.model, LEVEL, view);
    cg::builds() - before
}

fn same(a: &Drawing, b: &Drawing) -> bool {
    a.prims.len() == b.prims.len()
        && a.owners == b.owners
        && a.parts == b.parts
        && a.prims.iter().zip(&b.prims).all(|(x, y)| x.points() == y.points())
}

/// The primitives of `d` that reach onto its screen, and their owners.
/// A primitive's owner and its points.
type Owned = (Option<u64>, Vec<(f32, f32)>);

fn on_screen(d: &Drawing) -> Vec<Owned> {
    let (w, h) = (d.view.width as f32, d.view.height as f32);
    d.prims
        .iter()
        .zip(&d.owners)
        .filter(|(p, _)| {
            let pts = p.points();
            let (x0, x1) = pts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), q| (a.min(q.0), b.max(q.0)));
            let (y0, y1) = pts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), q| (a.min(q.1), b.max(q.1)));
            x1 >= 0.0 && x0 <= w && y1 >= 0.0 && y0 <= h
        })
        .map(|(p, o)| (*o, p.points()))
        .collect()
}

#[test]
fn a_plan_drawn_again_builds_nothing() {
    let (s, _) = row(40);
    let view = at(40_100.0);
    assert!(built(&s, view) > 0, "the first drawing builds what it shows");
    assert_eq!(built(&s, view), 0, "drawn again, nothing");
}

#[test]
fn a_pan_of_half_a_screen_builds_only_what_came_into_the_window() {
    cg::without_fineness(|| {
        let (s, _) = row(60);
        let (before, after) = (at(40_100.0), at(45_100.0));
        let first = built(&s, before);
        assert_eq!(first, (0..60).filter(|k| meets(*k, before)).count(), "the first drawing builds its window's");
        let came = (0..60).filter(|k| meets(*k, after) && !meets(*k, before)).count();
        assert!(came > 0);
        assert_eq!(built(&s, after), came);
    });
}

#[test]
fn one_component_edited_builds_that_one_again() {
    cg::without_fineness(|| {
        let (mut s, ids) = row(40);
        let view = at(40_100.0);
        built(&s, view);
        s.execute("modify", &json!({"id": ids[20], "params": {"Height": 999}})).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(built(&s, view), 1, "the one changed");
        // One out of the window: nothing.
        s.execute("modify", &json!({"id": ids[0], "params": {"Height": 998}})).unwrap_or_else(|e| panic!("{e}"));
        assert!(!meets(0, view));
        assert_eq!(built(&s, view), 0);
    });
}

#[test]
fn a_component_off_the_window_costs_no_build() {
    cg::without_fineness(|| {
        let (mut s, _) = row(3);
        let far = place(&mut s, [1e6, 0.0], json!({"Height": 700}));
        let view = at(2000.0);
        assert_eq!(built(&s, view), 3, "the three near, not the far one");
        assert!(!render::draw(&s.model, LEVEL, view).owners.contains(&Some(far)));
    });
}

/// Boxes all over, a wall through them, a plan zoomed in on a few: culled, it draws on its screen just what it draws unculled.
#[test]
fn culled_and_unculled_plans_are_alike_inside_the_window() {
    let (mut s, _) = row(60);
    s.execute("wall", &json!({"points": [[0, 1000], [120000, 1000]]})).unwrap_or_else(|e| panic!("{e}"));
    for view in [at(40_100.0), View { angle: 30.0, ..at(70_000.0) }, View { scale: 0.4, ..at(20_000.0) }] {
        let culled = render::draw(&s.model, LEVEL, view);
        let whole = cg::without_culling(|| render::draw(&s.model, LEVEL, view));
        assert!(culled.owners.len() < whole.owners.len(), "something is left out at {view:?}");
        assert_eq!(on_screen(&culled), on_screen(&whole), "at {view:?}");
        assert_eq!(render::png(&culled), render::png(&whole), "pixel for pixel at {view:?}");
    }
}

/// A view cropped to a few boxes, exported: as its own view culls it, pixel for pixel and owner for owner the unculled one.
#[test]
fn an_export_of_a_zoomed_view_is_the_unculled_one() {
    let (mut s, _) = row(60);
    // A sheet's viewport of the plan, at paper size.
    let view = at(40_100.0);
    let culled = render::draw_at(&s.model, LEVEL, view, 1.0);
    let whole = cg::without_culling(|| render::draw_at(&s.model, LEVEL, view, 1.0));
    assert!(culled.owners.len() < whole.owners.len(), "something is left out");
    assert_eq!(on_screen(&culled), on_screen(&whole));
    assert_eq!(render::png(&culled), render::png(&whole));
    // And the plan cropped to a few boxes, exported as an image of it.
    s.execute("crop", &json!({"view": LEVEL, "region": [[38_000, -2000], [44_000, 2000]]})).unwrap_or_else(|e| panic!("{e}"));
    let culled = render::plan(&s.model, LEVEL, 1200, 800);
    let whole = cg::without_culling(|| render::plan(&s.model, LEVEL, 1200, 800));
    assert_eq!(on_screen(&culled), on_screen(&whole));
    assert_eq!(render::png(&culled), render::png(&whole));
}

/// Under the cut plane, past the view range's bottom and still within its view depth: drawn, and so built. Wholly under
/// the view depth, or over the cut plane: left out, and never built.
#[test]
fn a_component_only_beyond_is_drawn_and_one_past_the_view_depth_is_never_built() {
    cg::without_fineness(|| {
        let mut s = boxes();
        s.execute("modify", &json!({"id": LEVEL, "view_range": {"top": 2300, "cut": 1200, "bottom": 0, "depth": -3000}}))
            .unwrap_or_else(|e| panic!("{e}"));
        let at_ = |s: &mut Session, x: f64, offset: f64, h: f64| {
            let v = s
                .execute("component", &json!({"family": "Box", "at": [x, 0], "offset": offset, "params": {"Height": h}}))
                .unwrap_or_else(|e| panic!("{e}"));
            v["id"].as_u64().unwrap_or_else(|| panic!("{v}"))
        };
        let beyond = at_(&mut s, 0.0, -2000.0, 600.0);
        let under = at_(&mut s, 2000.0, -5000.0, 601.0);
        let over = at_(&mut s, 4000.0, 3000.0, 602.0);
        let view = at(2000.0);
        assert_eq!(built(&s, view), 1, "only the one beyond");
        let d = render::draw(&s.model, LEVEL, view);
        assert!(d.owners.contains(&Some(beyond)), "beyond, drawn");
        assert!(!d.owners.contains(&Some(under)) && !d.owners.contains(&Some(over)));
    });
}

#[test]
fn a_store_too_small_for_the_plan_draws_it_the_same_and_only_builds_more() {
    let (s, _) = row(40);
    let view = at(40_100.0);
    let roomy = render::draw(&s.model, LEVEL, view);
    let again = built(&s, view);
    assert_eq!(again, 0);
    cg::with_budget(1, || {
        let (s, _) = row(40);
        let first = render::draw(&s.model, LEVEL, view);
        assert!(same(&first, &roomy));
        assert!(built(&s, view) > 0, "nothing kept: built again");
        assert!(same(&render::draw(&s.model, LEVEL, view), &roomy));
    });
}

/// GEO-2f's facade panel, curved every way a family chords a curve, `n` of them each its own size in a grid.
fn panels(n: usize, failing: usize) -> Session {
    let line = |a: Value, b: Value| json!({"start": a, "end": b});
    let knob = vec![
        line(json!(["W / 2 - R", 0]), json!(["W / 2 - R / 2", 0])),
        line(json!(["W / 2 - R / 2", 0]), json!(["W / 2 - R / 2", 80])),
        line(json!(["W / 2 - R / 2", 80]), json!(["W / 2 - R", 80])),
        line(json!(["W / 2 - R", 80]), json!(["W / 2 - R", 0])),
        json!({"start": ["W / 2", 0], "end": ["W / 2", 100], "axis": true}),
    ];
    let mut s = Session::new();
    s.execute(
        "family",
        &json!({"name": "Panel", "category": "generic",
            "parameters": [{"name": "W", "value": 1200, "instance": true}, {"name": "R", "value": 120, "instance": true}],
            "types": {"T": {}}, "forms": [
                {"profile": [[0, 0], ["W", 0], ["W", 150], [0, 150]], "to": 300},
                {"sketch": knob, "host": {"plane": "9e3c0a10-1f0b-4d8c-8a5e-0000000000a2"}, "from": 0, "to": 360},
                {"sketch": [{"centre": [75, 75], "radius": "R / 3"}], "from": -10, "to": 310, "void": true}]}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    // One placed by the command, the rest copies of it put straight in, unchecked: one that fails to build too.
    let first = s.execute("component", &json!({"family": "Panel", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"));
    let first = first["id"].as_u64().unwrap_or_else(|| panic!("{first}"));
    let Some(crate::model::Element::Component(one)) = s.model.elements.get(first).cloned() else { panic!("a panel") };
    let model = std::sync::Arc::make_mut(&mut s.model);
    model.elements.remove(first);
    for k in 0..n {
        // A knob of no width lies along its axis, and a void of no radius: that one doesn't build.
        let r = if k < failing { 0.0 } else { 100.0 + (k % 11) as f64 * 4.0 };
        let mut c = one.clone();
        c.params = [("W".to_string(), json!(1000.0 + k as f64)), ("R".to_string(), json!(r))].into_iter().collect();
        c.at = crate::geom::P2((k % 10) as f64 * 1500.0, (k / 10) as f64 * 1500.0);
        model.insert(crate::model::Element::Component(c));
    }
    s
}

#[test]
fn one_thread_and_eight_draw_the_same_plan() {
    let view = View { angle: 0.0, scale: 0.08, cx: 7000.0, cy: 4000.0, width: 1600, height: 1000 };
    let one = cg::with_threads(1, || render::draw(&panels(60, 0).model, LEVEL, view));
    let s = panels(60, 0);
    let eight = cg::with_threads(8, || {
        let d = render::draw(&s.model, LEVEL, view);
        assert!(cg::build_threads() > 1, "built on several threads");
        d
    });
    assert!(same(&one, &eight));
    assert_eq!(render::png(&one), render::png(&eight));
}

/// A build that fails is no geometry, on one thread or eight; and one that panics panics the drawing the same way.
#[test]
fn a_failed_build_comes_back_as_it_does_on_one_thread() {
    let view = View { angle: 0.0, scale: 0.08, cx: 7000.0, cy: 4000.0, width: 1600, height: 1000 };
    let one = cg::with_threads(1, || render::draw(&panels(40, 5).model, LEVEL, view));
    let eight = cg::with_threads(8, || render::draw(&panels(40, 5).model, LEVEL, view));
    assert!(same(&one, &eight));
    let s = panels(40, 5);
    let failed: Vec<u64> = s
        .model
        .elements
        .iter()
        .filter(|(_, e)| matches!(e, crate::model::Element::Component(c) if c.params.get("R") == Some(&json!(0.0))))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(failed.len(), 5);
    assert!(failed.iter().all(|id| !eight.owners.contains(&Some(*id))), "a failed build draws nothing");
    // A build that panics: the same panic out of the drawing, however many threads built it.
    let panicked = |threads: usize| {
        let s = panels(40, 0);
        let mut model = (*s.model).clone();
        for e in model.elements.values_mut() {
            if let crate::model::Element::Component(c) = e
                && c.params.get("W") == Some(&json!(1007.0))
            {
                c.params.insert("R".into(), json!(cg::PANIC_PROBE));
            }
        }
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cg::with_threads(threads, || render::draw(&model, LEVEL, view))));
        match caught {
            Err(e) => e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())),
            Ok(_) => None,
        }
    };
    let serial = panicked(1);
    assert!(serial.is_some(), "the probe panics");
    assert_eq!(panicked(8), serial);
}

/// ec's review of #84: what a shape takes is counted with its spare room and its nested instances' paths.
#[test]
fn the_bytes_a_shape_takes_count_its_spare_room_and_its_nested_paths() {
    let s = panels(1, 0);
    let Some(crate::model::Element::Component(c)) = s.model.elements.values().find(|e| matches!(e, crate::model::Element::Component(_)))
    else {
        panic!("a panel")
    };
    let pieces = s.model.component_local_pieces(c);
    let mut pieces = pieces;
    pieces[0].tris.shrink_to_fit();
    let plain = cg::estimate(&pieces);
    let mut roomy = pieces.clone();
    roomy[0].tris.reserve_exact(1000);
    assert!(cg::estimate(&roomy) >= plain + 1000 * 72);
    let mut nested = pieces.clone();
    nested[0].nested = vec![crate::ids::Guid::new(); 100];
    assert!(cg::estimate(&nested) >= plain + 100 * std::mem::size_of::<crate::ids::Guid>());
}

/// ec's review of #84: a family's plan builds each of its components once, though it draws the project's plan and then its own
/// pass over them.
#[test]
fn a_familys_plan_builds_each_component_once() {
    cg::without_fineness(|| {
        let (s, _) = row(5);
        let before = cg::builds();
        render::family_plan(&s.model, 100000, at(4000.0), None);
        assert_eq!(cg::builds() - before, 5);
    });
}

/// The facade panel's envelope (an L, a knob turned about an axis, a round void) is known without building it, and holds
/// all it builds, at the default and finer: so a plan leaves the panels far from its window out without building them.
#[test]
fn a_panels_envelope_is_known_and_holds_all_it_builds() {
    let s = panels(30, 0);
    let f = &s.model.families["Panel"];
    for e in s.model.elements.values() {
        let crate::model::Element::Component(c) = e else { continue };
        let v = f.values(&c.type_name, &c.params).unwrap_or_else(|e| panic!("{e}"));
        let [lo, hi] = f.conservative_bounds(&v).unwrap_or_else(|| panic!("no envelope for {:?}", c.params));
        for fineness in [crate::family::Fineness::DEFAULT, crate::family::Fineness::for_view(2.0)] {
            let pieces = f.pieces_fine(&v, fineness).unwrap_or_else(|e| panic!("{e}"));
            assert!(!pieces.is_empty());
            for p in pieces.iter().flat_map(|p| p.tris.iter().flatten()) {
                assert!((0..3).all(|i| lo[i] <= p[i] && p[i] <= hi[i]), "{p:?} outside {lo:?}..{hi:?}");
            }
        }
    }
    // Ten by three of them, a screen on the first few: those far off it aren't built.
    cg::without_fineness(|| {
        let view = View { angle: 0.0, scale: 0.5, cx: 0.0, cy: 0.0, width: 400, height: 300 };
        assert_eq!(built(&s, view), 1, "the one in the window");
    });
}

/// ec's review of #88: a family whose solid is a small box at its origin and whose model line runs 30 m from it, viewed at the
/// line's far end: the line is drawn, on its screen culled as unculled, though the box is far off the window (and, culled,
/// isn't drawn: `plan_index_tests.rs`).
#[test]
fn a_model_line_far_from_its_familys_solid_is_drawn_where_it_reaches_into_the_window() {
    let mut s = Session::new();
    s.execute(
        "family",
        &json!({"name": "Long", "category": "generic", "types": {"T": {}},
                "forms": [{"profile": [[0, 0], [200, 0], [200, 200], [0, 200]], "to": 300}],
                "model_lines": [{"start": [0, 100, 0], "end": [30000, 100, 0]}]}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let id = s.execute("component", &json!({"family": "Long", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"))["id"]
        .as_u64()
        .unwrap_or_else(|| panic!("an id"));
    for view in [at(29_000.0), View { angle: 30.0, ..at(29_000.0) }] {
        let culled = render::draw(&s.model, LEVEL, view);
        let whole = cg::without_culling(|| render::draw(&s.model, LEVEL, view));
        assert!(whole.owners.contains(&Some(id)), "unculled, its line is drawn");
        assert!(culled.owners.contains(&Some(id)), "culled, its line is drawn");
        assert_eq!(on_screen(&culled), on_screen(&whole), "culled, the same on its screen");
    }
}

/// ec's review of #88: a finer build kept as its default one, shared, is charged what it takes once its default one goes.
#[test]
fn a_finer_build_shared_with_its_default_is_counted_in_full_once_the_default_goes() {
    // Too fine to cut at a close zoom: its finer build is its default one, shared (#84).
    let round = || {
        let mut s = Session::new();
        let mut forms = vec![json!({"sketch": [{"centre": [0, 0], "radius": 20000}], "from": 0, "to": 3000})];
        for x in [-10000, 10000] {
            forms.push(json!({"sketch": [{"centre": [x, 0], "radius": 6000}], "from": 1000, "to": 4000, "void": true}));
        }
        s.execute("family", &json!({"name": "Round", "category": "generic", "types": {"T": {}}, "forms": forms}))
            .unwrap_or_else(|e| panic!("{e}"));
        s.execute("component", &json!({"family": "Round", "at": [0, 0]})).unwrap_or_else(|e| panic!("{e}"));
        s
    };
    let view = View { angle: 0.0, scale: 2.0, cx: 20_000.0, cy: 0.0, width: 1000, height: 800 };
    let s = round();
    render::draw(&s.model, LEVEL, view);
    assert!(cg::kept_finer().0 > 0, "a finer build kept as its default one");
    // Another model's drawing, with no room for a default build: the first one's goes, its finer one stays.
    let other = round();
    cg::with_budgets([1, 1 << 30], || render::draw(&other.model, LEVEL, view));
    assert!(cg::kept_finer().1 > 0, "a finer build kept with no default one");
    assert!(cg::kept_finer_honest(), "each finer build no default shares is charged in full");
    let (counted, summed) = cg::kept_bytes();
    assert_eq!(counted, summed);
}

/// ec's review of #88: two builds that panic, on one thread or eight: the first in the model's order comes out.
#[test]
fn of_two_panics_the_first_in_the_models_order_comes_out() {
    let view = View { angle: 0.0, scale: 0.08, cx: 7000.0, cy: 4000.0, width: 1600, height: 1000 };
    let panicked = |threads: usize| {
        let s = panels(40, 0);
        let mut model = (*s.model).clone();
        for e in model.elements.values_mut() {
            if let crate::model::Element::Component(c) = e
                && [json!(1003.0), json!(1031.0)].contains(&c.params["W"])
            {
                c.params.insert("R".into(), json!(cg::PANIC_PROBE));
            }
        }
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cg::with_threads(threads, || render::draw(&model, LEVEL, view))));
        match caught {
            Err(e) => e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())),
            Ok(_) => None,
        }
    };
    let serial = panicked(1);
    assert!(serial.as_deref().is_some_and(|m| m.contains("1003")), "the first one: {serial:?}");
    for _ in 0..8 {
        assert_eq!(panicked(8), serial);
    }
}

/// cp's review of #88: a plan zoomed to fit a facade asks for a fineness its panels' curves are already within at the default,
/// so it builds nothing finer and draws as it would at the default.
#[test]
fn a_facade_fitted_builds_nothing_finer() {
    let s = panels(100, 0);
    let fit = render::plan_fit(&s.model, LEVEL, 2000, 1200, false);
    assert_ne!(crate::family::Fineness::for_view(fit.scale), crate::family::Fineness::DEFAULT, "a fineness of its own");
    let before = cg::fine_builds();
    let fitted = render::draw(&s.model, LEVEL, fit);
    assert_eq!(cg::fine_builds() - before, 0, "nothing built finer");
    let s = panels(100, 0);
    assert!(same(&fitted, &cg::without_fineness(|| render::draw(&s.model, LEVEL, fit))));
}
